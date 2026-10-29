//! Abstract encoding surface — video and audio backends are split.
//!
//! The native path builds on FFmpeg (`AVCodecContext` / `AVFormatContext` /
//! `SwrContext`); other targets (WebCodecs, openh264, rav1e) plug in via
//! the same split traits. Keeping video and audio on separate traits makes
//! audio-capable outputs explicit: the renderer constructs one video
//! backend and, when requested, one audio backend — a single-stream codec
//! (rav1e, openh264) cannot sit in an audio slot because it only
//! implements [`VideoEncodingBackend`].

use crate::audio_spec::AudioSpec;
use crate::color_space::ColorSpaceDescriptor;
use crate::frame_rate::FrameRate;
use crate::pixel_format::{AlphaMode, PixelFormat};

/// Which stream a packet belongs to.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StreamKind {
    Video,
    Audio,
}

/// Encoded bitstream packet produced by any backend. Pure-Rust — no FFmpeg
/// types cross this boundary.
#[derive(Clone, Debug)]
pub struct EncodedPacket {
    pub stream_kind: StreamKind,
    pub stream_index: u32,
    pub pts: i64,
    pub dts: i64,
    pub data: Vec<u8>,
    pub is_key: bool,
}

/// Raw video frame handed to a video backend.
#[derive(Debug)]
pub struct RawVideoFrame<'a> {
    pub width: u32,
    pub height: u32,
    pub pts: i64,
    pub pixel_format: PixelFormat,
    pub data: &'a [u8],
}

/// Interleaved float32 audio samples handed to an audio backend.
#[derive(Debug)]
pub struct AudioSamples<'a> {
    pub samples: &'a [f32],
    pub channels: u16,
    pub sample_rate: u32,
    pub pts: i64,
}

/// Video-only encoder configuration. Pure-Rust; portable to WebCodecs /
/// rav1e / openh264.
#[derive(Clone, Debug)]
pub struct VideoEncoderConfig {
    pub width: u32,
    pub height: u32,
    pub frame_rate: FrameRate,
    pub pixel_format: PixelFormat,
    pub alpha_mode: AlphaMode,
    pub color_space: ColorSpaceDescriptor,
    pub gop_size: u32,
    pub target_bitrate: Option<u64>,
    pub codec_hint: Option<String>,
}

/// Audio-only encoder configuration.
#[derive(Clone, Debug)]
pub struct AudioEncoderConfig {
    pub spec: AudioSpec,
    pub target_bitrate: Option<u64>,
    pub codec_hint: Option<String>,
}

/// Errors shared by every backend.
#[derive(Debug, thiserror::Error)]
pub enum EncodingError {
    #[error("encoder not yet opened")]
    NotOpen,
    #[error("encoder already open")]
    AlreadyOpen,
    #[error("codec unavailable: {0}")]
    CodecUnavailable(String),
    #[error("invalid config: {0}")]
    InvalidConfig(&'static str),
    #[error("unsupported pixel format: {0:?}")]
    UnsupportedPixelFormat(PixelFormat),
    #[error("unsupported operation: {0}")]
    Unsupported(&'static str),
    #[error("backend error: {0}")]
    Backend(String),
}

pub type EncodingResult<T> = Result<T, EncodingError>;

/// Abstract video-encoding backend. Owns one video stream. On the FFmpeg
/// native path the backend shares a muxer with an optional sibling
/// [`AudioEncodingBackend`] via the concrete `bind_to_file_sink` method.
pub trait VideoEncodingBackend: Send {
    fn open(&mut self, config: &VideoEncoderConfig) -> EncodingResult<()>;
    fn push_video_frame(&mut self, frame: &RawVideoFrame<'_>) -> EncodingResult<Vec<EncodedPacket>>;
    fn flush(&mut self) -> EncodingResult<Vec<EncodedPacket>>;
    fn close(&mut self) -> EncodingResult<()>;
}

/// Abstract audio-encoding backend. Owns one audio stream.
pub trait AudioEncodingBackend: Send {
    fn open(&mut self, config: &AudioEncoderConfig) -> EncodingResult<()>;
    fn push_audio_samples(
        &mut self,
        samples: &AudioSamples<'_>,
    ) -> EncodingResult<Vec<EncodedPacket>>;
    fn flush(&mut self) -> EncodingResult<Vec<EncodedPacket>>;
    fn close(&mut self) -> EncodingResult<()>;
}

// ---------------------------------------------------------------------------
// Native (FFmpeg) adapters — split into video and audio backends.
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
mod ffmpeg_adapter {
    use super::*;
    use crate::EncoderOptions;
    use crate::RenderOptions;
    use crate::media::ffmpeg_sys_fframes as ff;
    use crate::renderer::audio_stream_encoder::AudioStreamEncoder;
    use crate::renderer::video_stream_encoder::VideoStreamEncoder;
    use std::ffi::CString;

    pub(super) struct AudioResampler {
        swr: *mut ff::SwrContext,
        in_sample_rate: u32,
        in_channels: u16,
        out_sample_fmt: ff::AVSampleFormat,
        out_channels: i32,
        out_frame_size: i32,
    }

    unsafe impl Send for AudioResampler {}

    impl AudioResampler {
        unsafe fn new(
            in_sample_rate: u32,
            in_channels: u16,
            out_sample_rate: i32,
            out_layout: &ff::AVChannelLayout,
            out_sample_fmt: ff::AVSampleFormat,
            out_frame_size: i32,
        ) -> EncodingResult<Self> {
            unsafe {
                let swr = ff::swr_alloc();
                if swr.is_null() {
                    return Err(EncodingError::Backend("swr_alloc returned null".into()));
                }
                let in_layout = default_layout_for_channel_count(in_channels)?;

                set_swr_int(swr, "in_sample_rate", in_sample_rate as i64);
                set_swr_int(swr, "out_sample_rate", out_sample_rate as i64);
                ff::av_opt_set_chlayout(
                    swr as *mut std::ffi::c_void,
                    CString::new("in_chlayout").unwrap().as_ptr(),
                    &in_layout as *const ff::AVChannelLayout,
                    0,
                );
                ff::av_opt_set_chlayout(
                    swr as *mut std::ffi::c_void,
                    CString::new("out_chlayout").unwrap().as_ptr(),
                    out_layout as *const ff::AVChannelLayout,
                    0,
                );
                ff::av_opt_set_sample_fmt(
                    swr as *mut std::ffi::c_void,
                    CString::new("in_sample_fmt").unwrap().as_ptr(),
                    ff::AVSampleFormat::AV_SAMPLE_FMT_FLT,
                    0,
                );
                ff::av_opt_set_sample_fmt(
                    swr as *mut std::ffi::c_void,
                    CString::new("out_sample_fmt").unwrap().as_ptr(),
                    out_sample_fmt,
                    0,
                );

                let init = ff::swr_init(swr);
                if init < 0 {
                    let mut raw = swr;
                    ff::swr_free(&mut raw);
                    return Err(EncodingError::Backend(format!(
                        "swr_init failed: {}",
                        crate::renderer::av_error_to_string(init)
                    )));
                }

                Ok(Self {
                    swr,
                    in_sample_rate,
                    in_channels,
                    out_sample_fmt,
                    out_channels: out_layout.nb_channels,
                    out_frame_size,
                })
            }
        }

        fn matches_input(&self, sample_rate: u32, channels: u16) -> bool {
            self.in_sample_rate == sample_rate && self.in_channels == channels
        }
    }

    impl Drop for AudioResampler {
        fn drop(&mut self) {
            unsafe {
                let mut raw = self.swr;
                if !raw.is_null() {
                    ff::swr_free(&mut raw);
                }
            }
        }
    }

    unsafe fn set_swr_int(swr: *mut ff::SwrContext, name: &str, val: i64) {
        unsafe {
            if let Ok(cname) = CString::new(name) {
                ff::av_opt_set_int(swr as *mut std::ffi::c_void, cname.as_ptr(), val, 0);
            }
        }
    }

    fn default_layout_for_channel_count(
        channels: u16,
    ) -> EncodingResult<ff::AVChannelLayout> {
        // Default-layout table follows FFmpeg's own conventions:
        //   1ch mono, 2ch stereo, 3ch 2.1 (FL+FR+LFE — the most common
        //   3-channel consumer layout; callers wanting plain surround
        //   3.0 should configure AudioSpec.layout explicitly), 4ch quad,
        //   5ch 5.0, 6ch 5.1, 7ch 6.1, 8ch 7.1. Zero channels is
        //   rejected because SwrContext has nothing to resample.
        let mask = match channels {
            1 => ff::AV_CH_LAYOUT_MONO,
            2 => ff::AV_CH_LAYOUT_STEREO,
            3 => ff::AV_CH_LAYOUT_2POINT1,
            4 => ff::AV_CH_LAYOUT_QUAD,
            5 => ff::AV_CH_LAYOUT_5POINT0,
            6 => ff::AV_CH_LAYOUT_5POINT1,
            7 => ff::AV_CH_LAYOUT_6POINT1,
            8 => ff::AV_CH_LAYOUT_7POINT1,
            0 => return Err(EncodingError::Unsupported("zero channels")),
            _ => {
                return Err(EncodingError::Unsupported(
                    "channel count > 8 has no default layout — set AudioSpec.layout",
                ));
            }
        };
        Ok(ff::AVChannelLayout {
            order: ff::AVChannelOrder::AV_CHANNEL_ORDER_NATIVE,
            nb_channels: channels as i32,
            u: ff::AVChannelLayout__bindgen_ty_1 { mask },
            opaque: std::ptr::null_mut(),
        })
    }

    // ---- FFmpegVideoBackend --------------------------------------------

    pub struct FFmpegVideoBackend {
        video: Option<VideoStreamEncoder>,
        pending_codec_id: Option<ff::AVCodecID>,
        opened: bool,
        /// Non-owning AVFormatContext pointer after `bind_to_file_sink`.
        /// The sink owns the muxer.
        bound_oc: *mut ff::AVFormatContext,
        /// Non-owning pointer to the shared `FileOutputSink` set by
        /// `bind_to_file_sink`. Packets emitted by the codec are routed
        /// through the sink so the single shared muxer sees both streams.
        sink_ptr: *mut crate::renderer::output_sink::FileOutputSink,
    }

    unsafe impl Send for FFmpegVideoBackend {}

    impl FFmpegVideoBackend {
        pub fn from_codec_id(codec_id: ff::AVCodecID) -> Self {
            Self {
                video: None,
                pending_codec_id: Some(codec_id),
                opened: false,
                bound_oc: std::ptr::null_mut(),
                sink_ptr: std::ptr::null_mut(),
            }
        }

        /// Bind this backend to a `FileOutputSink` (the same sink the
        /// companion audio backend will bind to). Must be called after
        /// `open()` and before any `push_video_frame`.
        pub fn bind_to_file_sink(
            &mut self,
            sink: &mut crate::renderer::output_sink::FileOutputSink,
        ) -> EncodingResult<()> {
            let video = self.video.as_mut().ok_or(EncodingError::NotOpen)?;
            unsafe {
                let (stream_index, codec_tb, stream_tb) = {
                    let sink_muxer = sink.muxer_mut().ok_or_else(|| {
                        EncodingError::Backend("bind_to_file_sink: sink has no muxer".into())
                    })?;
                    self.bound_oc = sink_muxer.as_ptr();
                    video
                        .stream
                        .bind_to_muxer(sink_muxer, None)
                        .map_err(|e| EncodingError::Backend(format!("video bind_to_muxer: {e}")))?;
                    (
                        (*video.stream.st).index as u32,
                        (*video.stream.enc).time_base,
                        (*video.stream.st).time_base,
                    )
                };
                sink.register_stream_binding(
                    stream_index,
                    (codec_tb.num, codec_tb.den),
                    (stream_tb.num, stream_tb.den),
                );
            }
            self.sink_ptr = sink as *mut _;
            Ok(())
        }

        fn route_packets_through_sink(
            &mut self,
            packets: Vec<EncodedPacket>,
        ) -> EncodingResult<Vec<EncodedPacket>> {
            use crate::renderer::output_sink::OutputSink;
            if self.sink_ptr.is_null() {
                return Ok(packets);
            }
            let sink = unsafe { &mut *self.sink_ptr };
            for pkt in &packets {
                sink.write_packet(pkt)?;
            }
            Ok(Vec::new())
        }

        pub fn stream_encoder_mut(&mut self) -> Option<&mut VideoStreamEncoder> {
            self.video.as_mut()
        }

        pub fn stream_encoder(&self) -> Option<&VideoStreamEncoder> {
            self.video.as_ref()
        }

        pub fn bound_format_ctx(&self) -> *mut ff::AVFormatContext {
            self.bound_oc
        }
    }

    fn build_render_options_for_video(config: &VideoEncoderConfig) -> RenderOptions {
        let mut render_options = RenderOptions::default();
        render_options.video_encoder_options.frame_rate = Some(config.frame_rate);
        render_options.video_encoder_options.typed_pixel_format = Some(config.pixel_format);
        render_options.video_encoder_options.color_space = config.color_space;
        render_options.video_encoder_options.alpha_mode = config.alpha_mode;
        render_options.video_encoder_options.gop_size = config.gop_size as i32;
        if let Some(br) = config.target_bitrate {
            render_options.video_encoder_options.bitrate = Some(br as i64);
        }
        render_options.video_encoder_options.preferred_encoder_owned = config.codec_hint.clone();
        render_options
    }

    fn build_audio_encoder_options(config: &AudioEncoderConfig) -> EncoderOptions<'static> {
        let mut opts = EncoderOptions::default();
        opts.sample_rate = config.spec.sample_rate as usize;
        opts.audio_spec = Some(config.spec.clone());
        opts.preferred_encoder_owned = config.codec_hint.clone();
        if let Some(br) = config.target_bitrate {
            opts.bitrate = Some(br as i64);
        }
        opts
    }

    impl VideoEncodingBackend for FFmpegVideoBackend {
        fn open(&mut self, config: &VideoEncoderConfig) -> EncodingResult<()> {
            if self.opened {
                return Err(EncodingError::AlreadyOpen);
            }
            let codec_id = self
                .pending_codec_id
                .take()
                .ok_or_else(|| EncodingError::Backend("video backend has no codec id".into()))?;
            let render_options = build_render_options_for_video(config);
            let video = unsafe {
                VideoStreamEncoder::new_codec_only(
                    config.width as i32,
                    config.height as i32,
                    config.frame_rate,
                    &render_options.video_encoder_options,
                    codec_id,
                )
                .map_err(|e| {
                    EncodingError::Backend(format!("VideoStreamEncoder::new_codec_only: {e}"))
                })?
            };
            self.video = Some(video);
            self.opened = true;
            Ok(())
        }

        fn push_video_frame(
            &mut self,
            frame: &RawVideoFrame<'_>,
        ) -> EncodingResult<Vec<EncodedPacket>> {
            if !self.opened {
                return Err(EncodingError::NotOpen);
            }
            let packets = {
                let video = self.video.as_mut().ok_or(EncodingError::NotOpen)?;
                let mut av_frame = video
                    .new_encoder_frame()
                    .map_err(|e| EncodingError::Backend(format!("new_encoder_frame: {e}")))?;
                match frame.pixel_format {
                    // RGBA-in paths: the bytes hold RGBA (8- or 16-bit);
                    // the EncoderFrame internals convert to the codec's
                    // target format.
                    PixelFormat::Rgba8 => {
                        let expected = (frame.width as usize) * (frame.height as usize) * 4;
                        if frame.data.len() < expected {
                            return Err(EncodingError::Backend(format!(
                                "rgba8 buffer short: got {} bytes, need {}",
                                frame.data.len(),
                                expected
                            )));
                        }
                        unsafe {
                            av_frame.set_pts(frame.pts);
                            video
                                .encode_rgba_frame(&mut av_frame, frame.data)
                                .map_err(|e| {
                                    EncodingError::Backend(format!("encode_rgba_frame: {e}"))
                                })?
                        }
                    }
                    PixelFormat::Rgba16 => {
                        let expected_bytes =
                            (frame.width as usize) * (frame.height as usize) * 8;
                        if frame.data.len() < expected_bytes {
                            return Err(EncodingError::Backend(format!(
                                "rgba16 buffer short: got {} bytes, need {}",
                                frame.data.len(),
                                expected_bytes
                            )));
                        }
                        if frame.data.as_ptr() as usize % std::mem::align_of::<u16>() != 0 {
                            return Err(EncodingError::Backend(
                                "rgba16 buffer not u16-aligned".into(),
                            ));
                        }
                        let samples = unsafe {
                            std::slice::from_raw_parts(
                                frame.data.as_ptr() as *const u16,
                                frame.data.len() / 2,
                            )
                        };
                        unsafe {
                            av_frame
                                .fill_from_rgba16_pixmap(samples)
                                .map_err(|e| {
                                    EncodingError::Backend(format!(
                                        "fill_from_rgba16_pixmap: {e}"
                                    ))
                                })?;
                            av_frame.set_pts(frame.pts);
                            video.write_frame_packets(&av_frame).map_err(|e| {
                                EncodingError::Backend(format!("write_frame_packets: {e}"))
                            })?
                        }
                    }
                    // YUV-in paths: the bytes are already laid out as
                    // planar YUV (and optionally alpha). Copy straight
                    // into the AVFrame's plane buffers — no color
                    // conversion — so callers with pre-computed YUV
                    // (video decoders, preprocessors) skip the
                    // YUV→RGBA→YUV round-trip.
                    PixelFormat::Yuv420P
                    | PixelFormat::Yuv444P
                    | PixelFormat::Yuv420P10Le
                    | PixelFormat::Yuv444P10Le
                    | PixelFormat::Yuva444P10Le => unsafe {
                        av_frame
                            .fill_from_yuv_planes(frame.pixel_format, frame.data)
                            .map_err(|e| {
                                EncodingError::Backend(format!(
                                    "fill_from_yuv_planes: {e}"
                                ))
                            })?;
                        av_frame.set_pts(frame.pts);
                        video.write_frame_packets(&av_frame).map_err(|e| {
                            EncodingError::Backend(format!("write_frame_packets: {e}"))
                        })?
                    },
                    other => return Err(EncodingError::UnsupportedPixelFormat(other)),
                }
            };
            self.route_packets_through_sink(packets)
        }

        fn flush(&mut self) -> EncodingResult<Vec<EncodedPacket>> {
            let packets = {
                let video = self.video.as_mut().ok_or(EncodingError::NotOpen)?;
                unsafe {
                    video
                        .flush_packets()
                        .map_err(|e| EncodingError::Backend(format!("video flush: {e}")))?
                }
            };
            self.route_packets_through_sink(packets)
        }

        fn close(&mut self) -> EncodingResult<()> {
            drop(self.video.take());
            self.opened = false;
            self.bound_oc = std::ptr::null_mut();
            Ok(())
        }
    }

    impl Drop for FFmpegVideoBackend {
        fn drop(&mut self) {
            if let Some(video) = self.video.as_mut() {
                video.stream.st = std::ptr::null_mut();
            }
            let _ = self.close();
        }
    }

    // ---- FFmpegAudioBackend --------------------------------------------

    pub struct FFmpegAudioBackend {
        audio: Option<AudioStreamEncoder>,
        pending_codec_id: Option<ff::AVCodecID>,
        pending_options: Option<EncoderOptions<'static>>,
        opened: bool,
        bound_oc: *mut ff::AVFormatContext,
        sink_ptr: *mut crate::renderer::output_sink::FileOutputSink,
        resampler: Option<AudioResampler>,
    }

    unsafe impl Send for FFmpegAudioBackend {}

    impl FFmpegAudioBackend {
        pub fn from_codec_id(codec_id: ff::AVCodecID) -> Self {
            Self {
                audio: None,
                pending_codec_id: Some(codec_id),
                pending_options: None,
                opened: false,
                bound_oc: std::ptr::null_mut(),
                sink_ptr: std::ptr::null_mut(),
                resampler: None,
            }
        }

        pub fn bind_to_file_sink(
            &mut self,
            sink: &mut crate::renderer::output_sink::FileOutputSink,
        ) -> EncodingResult<()> {
            let audio = self.audio.as_mut().ok_or(EncodingError::NotOpen)?;
            unsafe {
                let (stream_index, codec_tb, stream_tb) = {
                    let sink_muxer = sink.muxer_mut().ok_or_else(|| {
                        EncodingError::Backend(
                            "bind_to_file_sink: sink has no muxer (audio)".into(),
                        )
                    })?;
                    self.bound_oc = sink_muxer.as_ptr();
                    audio
                        .stream
                        .bind_to_muxer(sink_muxer, None)
                        .map_err(|e| EncodingError::Backend(format!("audio bind_to_muxer: {e}")))?;
                    audio
                        .apply_bwf_metadata_to_muxer(sink_muxer)
                        .map_err(|e| EncodingError::Backend(format!("BWF metadata: {e}")))?;
                    (
                        (*audio.stream.st).index as u32,
                        (*audio.stream.enc).time_base,
                        (*audio.stream.st).time_base,
                    )
                };
                sink.register_stream_binding(
                    stream_index,
                    (codec_tb.num, codec_tb.den),
                    (stream_tb.num, stream_tb.den),
                );
            }
            self.sink_ptr = sink as *mut _;
            Ok(())
        }

        fn route_packets_through_sink(
            &mut self,
            packets: Vec<EncodedPacket>,
        ) -> EncodingResult<Vec<EncodedPacket>> {
            use crate::renderer::output_sink::OutputSink;
            if self.sink_ptr.is_null() {
                return Ok(packets);
            }
            let sink = unsafe { &mut *self.sink_ptr };
            for pkt in &packets {
                sink.write_packet(pkt)?;
            }
            Ok(Vec::new())
        }

        pub fn stream_encoder_mut(&mut self) -> Option<&mut AudioStreamEncoder> {
            self.audio.as_mut()
        }

        pub fn stream_encoder(&self) -> Option<&AudioStreamEncoder> {
            self.audio.as_ref()
        }

        pub fn bound_format_ctx(&self) -> *mut ff::AVFormatContext {
            self.bound_oc
        }

        fn ensure_resampler(
            &mut self,
            in_rate: u32,
            in_channels: u16,
        ) -> EncodingResult<()> {
            if let Some(r) = self.resampler.as_ref() {
                if r.matches_input(in_rate, in_channels) {
                    return Ok(());
                }
                self.resampler = None;
            }
            let audio = self.audio.as_ref().ok_or(EncodingError::NotOpen)?;
            let (out_rate, out_layout, out_fmt, out_frame_size) = unsafe {
                let codec_ctx = audio.stream.enc;
                (
                    (*codec_ctx).sample_rate,
                    (*codec_ctx).ch_layout,
                    (*codec_ctx).sample_fmt,
                    if (*codec_ctx).frame_size > 0 {
                        (*codec_ctx).frame_size
                    } else {
                        1024
                    },
                )
            };
            let resampler = unsafe {
                AudioResampler::new(
                    in_rate,
                    in_channels,
                    out_rate,
                    &out_layout,
                    out_fmt,
                    out_frame_size,
                )?
            };
            self.resampler = Some(resampler);
            Ok(())
        }

        fn resample_and_send(
            &mut self,
            samples: &AudioSamples<'_>,
        ) -> EncodingResult<Vec<EncodedPacket>> {
            let resampler = self.resampler.as_ref().ok_or_else(|| {
                EncodingError::Backend("resampler not initialised".into())
            })?;
            let swr = resampler.swr;
            let out_channels = resampler.out_channels as usize;
            let out_fmt = resampler.out_sample_fmt;
            let out_frame_size = resampler.out_frame_size;
            let out_is_planar = unsafe { ff::av_sample_fmt_is_planar(out_fmt) != 0 };
            let bytes_per_sample = unsafe { ff::av_get_bytes_per_sample(out_fmt) as usize };
            if bytes_per_sample == 0 {
                return Err(EncodingError::Backend(
                    "av_get_bytes_per_sample returned 0".into(),
                ));
            }

            let audio = self.audio.as_mut().ok_or(EncodingError::NotOpen)?;

            let in_samples_total = samples.samples.len() / samples.channels.max(1) as usize;
            let mut remaining_in = in_samples_total as i32;
            let mut in_cursor = samples.samples.as_ptr();
            let mut out_pts = samples.pts;
            let mut collected = Vec::new();

            loop {
                let mut audio_frame = audio.new_encoder_frame().map_err(|e| {
                    EncodingError::Backend(format!("audio new_encoder_frame: {e}"))
                })?;

                let out_ptrs: [*mut u8; 8] = unsafe {
                    let mut arr: [*mut u8; 8] = [std::ptr::null_mut(); 8];
                    let data = (*audio_frame.av_frame).data;
                    for i in 0..8 {
                        arr[i] = data[i];
                    }
                    arr
                };

                let in_ptrs: [*const u8; 8] = {
                    let mut arr: [*const u8; 8] = [std::ptr::null(); 8];
                    if remaining_in > 0 {
                        arr[0] = in_cursor as *const u8;
                    }
                    arr
                };

                let out_written = unsafe {
                    ff::swr_convert(
                        swr,
                        out_ptrs.as_ptr() as *mut *mut u8,
                        out_frame_size,
                        if remaining_in > 0 {
                            in_ptrs.as_ptr() as *mut *const u8
                        } else {
                            std::ptr::null_mut()
                        },
                        remaining_in,
                    )
                };

                if out_written < 0 {
                    return Err(EncodingError::Backend(format!(
                        "swr_convert failed: {}",
                        crate::renderer::av_error_to_string(out_written)
                    )));
                }

                remaining_in = 0;
                in_cursor = std::ptr::null();

                if out_written == 0 {
                    break;
                }

                unsafe {
                    (*audio_frame.av_frame).nb_samples = out_written;
                    (*audio_frame.av_frame).pts = out_pts;
                    let _ = (out_channels, out_is_planar, bytes_per_sample);
                }

                let packets = unsafe { audio.send_frame_packets(&audio_frame) }.map_err(|e| {
                    EncodingError::Backend(format!("audio send_frame_packets: {e}"))
                })?;
                collected.extend(packets);

                out_pts = out_pts.saturating_add(out_written as i64);

                if out_written < out_frame_size {
                    break;
                }
            }

            Ok(collected)
        }
    }

    impl AudioEncodingBackend for FFmpegAudioBackend {
        fn open(&mut self, config: &AudioEncoderConfig) -> EncodingResult<()> {
            if self.opened {
                return Err(EncodingError::AlreadyOpen);
            }
            let codec_id = self.pending_codec_id.take().ok_or_else(|| {
                EncodingError::Backend("audio backend has no codec id".into())
            })?;
            let options = build_audio_encoder_options(config);
            self.pending_options = Some(options);
            let audio = unsafe {
                AudioStreamEncoder::new_codec_only(
                    self.pending_options.as_ref().unwrap(),
                    codec_id,
                )
                .map_err(|e| {
                    EncodingError::Backend(format!("AudioStreamEncoder::new_codec_only: {e}"))
                })?
            };
            self.audio = Some(audio);
            self.opened = true;
            Ok(())
        }

        fn push_audio_samples(
            &mut self,
            samples: &AudioSamples<'_>,
        ) -> EncodingResult<Vec<EncodedPacket>> {
            if !self.opened {
                return Err(EncodingError::NotOpen);
            }
            self.ensure_resampler(samples.sample_rate, samples.channels)?;
            let packets = self.resample_and_send(samples)?;
            self.route_packets_through_sink(packets)
        }

        fn flush(&mut self) -> EncodingResult<Vec<EncodedPacket>> {
            let packets = {
                let audio = self.audio.as_mut().ok_or(EncodingError::NotOpen)?;
                unsafe {
                    audio
                        .flush_packets()
                        .map_err(|e| EncodingError::Backend(format!("audio flush: {e}")))?
                }
            };
            self.route_packets_through_sink(packets)
        }

        fn close(&mut self) -> EncodingResult<()> {
            drop(self.audio.take());
            self.opened = false;
            self.bound_oc = std::ptr::null_mut();
            self.resampler = None;
            Ok(())
        }
    }

    impl Drop for FFmpegAudioBackend {
        fn drop(&mut self) {
            if let Some(audio) = self.audio.as_mut() {
                audio.stream.st = std::ptr::null_mut();
            }
            let _ = self.close();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use ffmpeg_adapter::{FFmpegAudioBackend, FFmpegVideoBackend};
