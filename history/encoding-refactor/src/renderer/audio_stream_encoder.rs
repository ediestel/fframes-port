//! AudioStreamEncoder — wraps AVCodecContext for audio, owns the SwrContext
//! and applies AudioSpec (channel layout, LUFS normalisation, BWF metadata).
//! Extracted from the legacy Encoder.

use super::encoder::av_error_to_string;
use super::encoder_frame::EncoderFrame;
use crate::encoding_backend::{EncodedPacket, StreamKind};
use super::format_muxer::FormatMuxer;
use super::renderer_error::{RenderEncodingError, RenderEncodingResult};
use super::stream::Stream;
use super::EncoderOptions;
use crate::audio_spec::AudioSpec;
use crate::media::ffmpeg_sys_fframes::*;

unsafe fn packet_from_avpacket(packet: *mut AVPacket, stream_index: u32) -> EncodedPacket {
    unsafe {
        let data = if (*packet).data.is_null() || (*packet).size <= 0 {
            Vec::new()
        } else {
            std::slice::from_raw_parts((*packet).data, (*packet).size as usize).to_vec()
        };
        EncodedPacket {
            stream_kind: StreamKind::Audio,
            stream_index,
            pts: (*packet).pts,
            dts: (*packet).dts,
            data,
            is_key: ((*packet).flags & AV_PKT_FLAG_KEY) != 0,
        }
    }
}

pub struct AudioStreamEncoder {
    pub stream: Stream,
    pub spec: AudioSpec,
}

unsafe impl Send for AudioStreamEncoder {}
unsafe impl Sync for AudioStreamEncoder {}

impl AudioStreamEncoder {
    pub unsafe fn new(
        muxer: &mut FormatMuxer,
        options: &EncoderOptions,
    ) -> RenderEncodingResult<Self> {
        unsafe {
            Self::new_internal(Some(muxer), options, None)
        }
    }

    /// Codec-only constructor (step 1 in the README): allocates only an
    /// AVCodecContext without a muxer. BWF metadata can't be written until
    /// bind time; callers use [`Self::apply_bwf_metadata_to_muxer`] after
    /// binding to a FormatMuxer.
    pub unsafe fn new_codec_only(
        options: &EncoderOptions,
        codec_id: crate::media::ffmpeg_sys_fframes::AVCodecID,
    ) -> RenderEncodingResult<Self> {
        unsafe { Self::new_internal(None, options, Some(codec_id)) }
    }

    unsafe fn new_internal(
        muxer: Option<&mut FormatMuxer>,
        options: &EncoderOptions,
        codec_id_override: Option<AVCodecID>,
    ) -> RenderEncodingResult<Self> {
        unsafe {
            let oc = muxer
                .as_ref()
                .map(|m| m.as_ptr())
                .unwrap_or(std::ptr::null_mut());
            let stream = Stream::make_audio_with_codec(oc, options, codec_id_override)?;
            let spec = options.audio_spec.clone().unwrap_or_default();

            if let (Some(muxer), Some(bwf)) = (muxer, spec.bwf_metadata.as_ref()) {
                Self::write_bwf(muxer, bwf)?;
            }

            Ok(Self { stream, spec })
        }
    }

    /// Write BWF metadata entries onto the muxer's dictionary. Separated
    /// from construction so callers using codec-only mode can apply them at
    /// bind time.
    pub unsafe fn apply_bwf_metadata_to_muxer(
        &self,
        muxer: &mut FormatMuxer,
    ) -> RenderEncodingResult<()> {
        unsafe {
            if let Some(bwf) = self.spec.bwf_metadata.as_ref() {
                Self::write_bwf(muxer, bwf)?;
            }
            Ok(())
        }
    }

    unsafe fn write_bwf(
        muxer: &mut FormatMuxer,
        bwf: &crate::audio_spec::BwfMetadata,
    ) -> RenderEncodingResult<()> {
        unsafe {
            if let Some(desc) = bwf.description.as_ref() {
                muxer.set_dict("comment", desc)?;
            }
            if let Some(o) = bwf.originator.as_ref() {
                muxer.set_dict("originator", o)?;
            }
            if let Some(o) = bwf.originator_reference.as_ref() {
                muxer.set_dict("originator_reference", o)?;
            }
            if let Some(d) = bwf.origination_date.as_ref() {
                muxer.set_dict("origination_date", d)?;
            }
            if let Some(t) = bwf.origination_time.as_ref() {
                muxer.set_dict("origination_time", t)?;
            }
            if let Some(tr) = bwf.time_reference {
                muxer.set_dict("time_reference", &tr.to_string())?;
            }
            Ok(())
        }
    }

    pub fn new_encoder_frame(&self) -> RenderEncodingResult<EncoderFrame> {
        unsafe { EncoderFrame::new(&self.stream) }
    }

    /// Compat: encode one frame's worth of packets and write them through
    /// the muxer.
    pub unsafe fn send_frame(
        &self,
        muxer: &FormatMuxer,
        frame: &EncoderFrame,
    ) -> RenderEncodingResult<()> {
        unsafe {
            let packets = self.send_frame_packets(frame)?;
            for pkt in packets {
                self.write_packet_through_muxer(muxer, &pkt)?;
            }
            Ok(())
        }
    }

    /// Decoupled: drive the codec with one frame and return the resulting
    /// packets. Pts/dts are in the codec time base; sink rescales.
    pub unsafe fn send_frame_packets(
        &self,
        frame: &EncoderFrame,
    ) -> RenderEncodingResult<Vec<EncodedPacket>> {
        unsafe {
            let send = avcodec_send_frame(self.stream.enc, frame.av_frame);
            if send < 0 {
                return Err(RenderEncodingError::CantWriteFrame(av_error_to_string(send)));
            }
            let mut out = Vec::new();
            loop {
                let r = avcodec_receive_packet(self.stream.enc, frame.packet);
                if r == AVERROR_EOF || r == crate::renderer::encoder::ffmpeg_averror(EAGAIN) {
                    break;
                }
                if r < 0 {
                    return Err(RenderEncodingError::CantWriteFrame(av_error_to_string(r)));
                }
                let stream_index = if self.stream.st.is_null() {
                    0
                } else {
                    (*self.stream.st).index as u32
                };
                out.push(packet_from_avpacket(frame.packet, stream_index));
                av_packet_unref(frame.packet);
            }
            Ok(out)
        }
    }

    /// Compat flush writes packets through the muxer.
    pub unsafe fn flush(&mut self, muxer: &FormatMuxer) -> RenderEncodingResult<()> {
        unsafe {
            let packets = self.flush_packets()?;
            for pkt in packets {
                self.write_packet_through_muxer(muxer, &pkt)?;
            }
            Ok(())
        }
    }

    /// Decoupled flush: send EOF, drain trailing packets. Caller routes
    /// them through an OutputSink.
    pub unsafe fn flush_packets(&mut self) -> RenderEncodingResult<Vec<EncodedPacket>> {
        unsafe {
            let send = avcodec_send_frame(self.stream.enc, std::ptr::null_mut());
            if send < 0 && send != AVERROR_EOF {
                return Err(RenderEncodingError::CantWriteFrame(av_error_to_string(send)));
            }
            let mut packet = av_packet_alloc();
            if packet.is_null() {
                return Err(RenderEncodingError::Internal(
                    "av_packet_alloc returned null".to_owned(),
                ));
            }
            let mut out = Vec::new();
            loop {
                let r = avcodec_receive_packet(self.stream.enc, packet);
                if r == AVERROR_EOF || r == crate::renderer::encoder::ffmpeg_averror(EAGAIN) {
                    break;
                }
                if r < 0 {
                    av_packet_free(&mut packet);
                    return Err(RenderEncodingError::CantWriteFrame(av_error_to_string(r)));
                }
                let stream_index = if self.stream.st.is_null() {
                    0
                } else {
                    (*self.stream.st).index as u32
                };
                out.push(packet_from_avpacket(packet, stream_index));
                av_packet_unref(packet);
            }
            av_packet_free(&mut packet);
            Ok(out)
        }
    }

    unsafe fn write_packet_through_muxer(
        &self,
        muxer: &FormatMuxer,
        packet: &EncodedPacket,
    ) -> RenderEncodingResult<()> {
        unsafe {
            if self.stream.st.is_null() {
                return Err(RenderEncodingError::Internal(
                    "audio stream not bound to a muxer".to_owned(),
                ));
            }
            let av_packet = av_packet_alloc();
            if av_packet.is_null() {
                return Err(RenderEncodingError::Internal(
                    "av_packet_alloc returned null".to_owned(),
                ));
            }
            if av_new_packet(av_packet, packet.data.len() as i32) < 0 {
                let mut raw = av_packet;
                av_packet_free(&mut raw);
                return Err(RenderEncodingError::Internal(
                    "av_new_packet failed".to_owned(),
                ));
            }
            if !packet.data.is_empty() {
                std::ptr::copy_nonoverlapping(
                    packet.data.as_ptr(),
                    (*av_packet).data,
                    packet.data.len(),
                );
            }
            (*av_packet).pts = packet.pts;
            (*av_packet).dts = packet.dts;
            if packet.is_key {
                (*av_packet).flags |= AV_PKT_FLAG_KEY;
            }
            av_packet_rescale_ts(
                av_packet,
                (*self.stream.enc).time_base,
                (*self.stream.st).time_base,
            );
            (*av_packet).stream_index = (*self.stream.st).index;
            let result = muxer.write_interleaved(av_packet);
            let mut raw = av_packet;
            av_packet_free(&mut raw);
            result
        }
    }

    pub fn free(&mut self) {
        self.stream.free();
    }

    pub fn apply_lufs_gain(&self, samples: &mut [f32]) {
        if let Some(target) = self.spec.lufs_target {
            let current = crate::audio_spec::compute_integrated_lufs(
                samples,
                self.spec.sample_rate,
                self.spec.layout.channel_count(),
            );
            let gain = crate::audio_spec::lufs_normalization_gain(current, target);
            if (gain - 1.0).abs() > 0.001 {
                for s in samples.iter_mut() {
                    *s *= gain;
                }
            }
        }
    }
}

impl Drop for AudioStreamEncoder {
    fn drop(&mut self) {
        self.stream.free();
    }
}
