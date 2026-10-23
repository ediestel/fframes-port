//! VideoStreamEncoder — wraps AVCodecContext for video, owns PTS counter,
//! pixel format dispatch, and frame format conversion. Extracted from the
//! legacy Encoder. Caches FrameFormatConvertor at stream level.

use super::encoder::av_error_to_string;
use super::encoder_frame::EncoderFrame;
use crate::encoding_backend::{EncodedPacket, StreamKind};
use super::format_muxer::FormatMuxer;
use super::renderer_error::{RenderEncodingError, RenderEncodingResult};
use super::stream::Stream;
use super::EncoderOptions;

unsafe fn packet_from_avpacket(
    packet: *mut AVPacket,
    stream_kind: StreamKind,
    stream_index: u32,
) -> EncodedPacket {
    unsafe {
        let data = if (*packet).data.is_null() || (*packet).size <= 0 {
            Vec::new()
        } else {
            std::slice::from_raw_parts((*packet).data, (*packet).size as usize).to_vec()
        };
        EncodedPacket {
            stream_kind,
            stream_index,
            pts: (*packet).pts,
            dts: (*packet).dts,
            data,
            is_key: ((*packet).flags & AV_PKT_FLAG_KEY) != 0,
        }
    }
}
use crate::color_space::ColorSpaceDescriptor;
use crate::frame_rate::FrameRate;
use crate::media::ffmpeg_sys_fframes::*;
use crate::pixel_format::{AlphaAwarePixelConfig, AlphaMode, PixelFormat};

pub struct VideoStreamEncoder {
    pub stream: Stream,
    pub pixel_config: AlphaAwarePixelConfig,
    pub frame_rate: FrameRate,
    format_convertor: Option<super::encoder_frame::FrameFormatConvertor>,
    pts: i64,
    width: i32,
    height: i32,
}

unsafe impl Send for VideoStreamEncoder {}
unsafe impl Sync for VideoStreamEncoder {}

impl VideoStreamEncoder {
    pub unsafe fn new(
        muxer: &FormatMuxer,
        width: i32,
        height: i32,
        frame_rate: FrameRate,
        options: &EncoderOptions,
    ) -> RenderEncodingResult<Self> {
        unsafe {
            let stream = Stream::make_video_with_rate(width, height, frame_rate, muxer.as_ptr(), options)?;
            let pf = options.effective_typed_pixel_format();
            let enc_fmt = (*stream.enc).pix_fmt;
            let native = super::encoder_frame::is_native_yuv_format(enc_fmt, pf);
            let format_convertor = (!native)
                .then(|| super::encoder_frame::FrameFormatConvertor::new(&stream))
                .transpose()?;
            Ok(Self {
                stream,
                pixel_config: AlphaAwarePixelConfig {
                    pixel_format: pf,
                    alpha_mode: options.alpha_mode,
                    color_space: options.color_space,
                },
                format_convertor,
                frame_rate,
                pts: 0,
                width,
                height,
            })
        }
    }

    /// Codec-only constructor (step 1 in the README): allocates only an
    /// AVCodecContext without an AVStream or muxer. The caller binds to a
    /// muxer later via [`Stream::bind_to_muxer`] before writing packets
    /// through a FormatMuxer-backed sink.
    pub unsafe fn new_codec_only(
        width: i32,
        height: i32,
        frame_rate: FrameRate,
        options: &EncoderOptions,
        codec_id: crate::media::ffmpeg_sys_fframes::AVCodecID,
    ) -> RenderEncodingResult<Self> {
        unsafe {
            let stream = Stream::make_video_with_rate_and_codec(
                width,
                height,
                frame_rate,
                std::ptr::null_mut(),
                options,
                Some(codec_id),
            )?;
            let pf = options.effective_typed_pixel_format();
            let enc_fmt = (*stream.enc).pix_fmt;
            let native = super::encoder_frame::is_native_yuv_format(enc_fmt, pf);
            let format_convertor = (!native)
                .then(|| super::encoder_frame::FrameFormatConvertor::new(&stream))
                .transpose()?;
            Ok(Self {
                stream,
                pixel_config: AlphaAwarePixelConfig {
                    pixel_format: pf,
                    alpha_mode: options.alpha_mode,
                    color_space: options.color_space,
                },
                format_convertor,
                frame_rate,
                pts: 0,
                width,
                height,
            })
        }
    }

    pub fn new_encoder_frame(&self) -> RenderEncodingResult<EncoderFrame> {
        unsafe {
            EncoderFrame::new_with_shared_convertor(
                &self.stream,
                self.pixel_config,
                self.format_convertor.as_ref(),
            )
        }
    }

    /// Compat wrapper: encode an RGBA8 pixmap and write packets through the
    /// supplied FormatMuxer. Preserved for the existing Encoder-owns-muxer
    /// path; new callers use [`Self::encode_rgba_frame`] to receive
    /// `Vec<EncodedPacket>` directly.
    pub unsafe fn send_rgba_frame(
        &mut self,
        muxer: &FormatMuxer,
        frame: &mut EncoderFrame,
        rgba_pixels: &[u8],
    ) -> RenderEncodingResult<()> {
        unsafe {
            let packets = self.encode_rgba_frame(frame, rgba_pixels)?;
            for pkt in packets {
                self.write_packet_through_muxer(muxer, &pkt)?;
            }
            Ok(())
        }
    }

    /// Decoupled encode: fills the frame from the RGBA pixmap, drives the
    /// codec, returns freshly encoded packets. Packet pts/dts are in the
    /// AVCodecContext's time base; the sink rescales to its stream tb.
    pub unsafe fn encode_rgba_frame(
        &mut self,
        frame: &mut EncoderFrame,
        rgba_pixels: &[u8],
    ) -> RenderEncodingResult<Vec<EncodedPacket>> {
        unsafe {
            debug_assert!(
                (*frame.av_frame).width == self.width && (*frame.av_frame).height == self.height,
                "VideoStreamEncoder dimensions mismatch"
            );
            let pts = self
                .frame_rate
                .pts_for_frame(self.pts as u64, self.frame_rate.time_base_den() as u64);
            frame.fill_from_rgba_pixmap(rgba_pixels)?;
            frame.set_pts(pts);
            self.pts += 1;
            self.write_frame_packets(frame)
        }
    }

    /// Compat muxer-writing variant. Kept because Encoder's drop + tests
    /// still route through a FormatMuxer.
    pub unsafe fn write_frame(
        &self,
        muxer: &FormatMuxer,
        frame: &EncoderFrame,
    ) -> RenderEncodingResult<()> {
        unsafe {
            let packets = self.write_frame_packets(frame)?;
            for pkt in packets {
                self.write_packet_through_muxer(muxer, &pkt)?;
            }
            Ok(())
        }
    }

    /// Decoupled drive: send a frame into the encoder, drain any packets,
    /// return them. No muxer involvement.
    pub unsafe fn write_frame_packets(
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
                out.push(packet_from_avpacket(
                    frame.packet,
                    StreamKind::Video,
                    stream_index,
                ));
                av_packet_unref(frame.packet);
            }
            Ok(out)
        }
    }

    /// Write a single decoded EncodedPacket through the given muxer,
    /// rescaling pts/dts from codec-tb to stream-tb. Used by the compat
    /// send_rgba_frame / write_frame path.
    unsafe fn write_packet_through_muxer(
        &self,
        muxer: &FormatMuxer,
        packet: &EncodedPacket,
    ) -> RenderEncodingResult<()> {
        unsafe {
            if self.stream.st.is_null() {
                return Err(RenderEncodingError::Internal(
                    "write_packet_through_muxer: stream not bound to a muxer".to_owned(),
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

    /// Wrap a stream-copy `Stream` (no encoder context) for the remux path.
    /// Used when packets are copied byte-for-byte from an input file rather
    /// than re-encoded. The caller is responsible for setting the AVStream's
    /// codecpar and time_base before muxing.
    pub fn for_remux(
        stream: Stream,
        width: i32,
        height: i32,
        frame_rate: FrameRate,
    ) -> Self {
        Self {
            stream,
            pixel_config: AlphaAwarePixelConfig::default_sdr(),
            format_convertor: None,
            frame_rate,
            pts: 0,
            width,
            height,
        }
    }

    /// Compat flush: drains remaining packets and writes them to the muxer.
    pub unsafe fn flush(&mut self, muxer: &FormatMuxer) -> RenderEncodingResult<()> {
        unsafe {
            let packets = self.flush_packets()?;
            for pkt in packets {
                self.write_packet_through_muxer(muxer, &pkt)?;
            }
            Ok(())
        }
    }

    /// Decoupled flush: sends EOF marker to codec and drains trailing
    /// packets. Safe to call multiple times (subsequent calls return an
    /// empty Vec). Caller routes packets through an OutputSink.
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
                out.push(packet_from_avpacket(packet, StreamKind::Video, stream_index));
                av_packet_unref(packet);
            }
            av_packet_free(&mut packet);
            Ok(out)
        }
    }
}

impl Drop for VideoStreamEncoder {
    fn drop(&mut self) {
        self.stream.free();
    }
}
