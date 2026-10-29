//! FFmpeg-backed video and audio encoder.
//!
//! ## Codec availability
//!
//! ⚠️  **Codec and container support depends on your FFmpeg installation, not
//! on fframes.**  The fframes feature flags (`h264`, `h265`, `aac`, `vpx`,
//! `videotoolbox`, …) gate *link-time* codec selection; they do **not** build
//! or bundle FFmpeg.  Formats such as Apple ProRes (`prores_ks`), Avid DNxHR
//! (`dnxhd`), and OpenEXR sequences are only available if the linked FFmpeg
//! was compiled with the corresponding encoder enabled.  Verify at runtime:
//!
//! ```sh
//! ffmpeg -encoders 2>/dev/null | grep prores
//! ```
//!

use super::{
    FFramesLogger,
    audio_stream_encoder::AudioStreamEncoder,
    encoding_backend::{EncodingError, EncodingResult},
    format_muxer::FormatMuxer,
    output_sink::FileOutputSink,
    renderer_error::{self, RenderEncodingError},
    stream,
    stream::Stream,
    video_stream_encoder::VideoStreamEncoder,
};
pub use super::{encoder_frame::EncoderFrame, renderer_error::RenderEncodingResult};
use crate::audio_spec::AudioSpec;
use crate::color_space::ColorSpaceDescriptor;
use crate::ffmpeg_sys_fframes::*;
use super::output_target::OutputTarget;
use crate::frame_rate::FrameRate;
use crate::pixel_format::{AlphaMode, PixelFormat};
use crate::video_spec::VideoSpec;
use crate::{RenderOptions, ffmpeg_action};
use std::ops::Range;
use std::path::Path;
use std::{
    ffi::{CStr, CString},
    os::raw::c_char,
    sync::Arc,
};

pub use crate::media::ffmpeg_sys_fframes::{AVPixelFormat, MKTAG};

/// Negate an FFmpeg error code, matching the C `AVERROR()` macro convention.
/// Named in all-caps to match the upstream C macro it mirrors.
#[inline(always)]
pub const fn ffmpeg_averror(e: std::os::raw::c_int) -> std::os::raw::c_int {
    -e
}

pub fn av_error_to_string(errnum: i32) -> String {
    let mut errbuf = [0 as c_char; AV_ERROR_MAX_STRING_SIZE];
    unsafe {
        if av_strerror(errnum, errbuf.as_mut_ptr(), AV_ERROR_MAX_STRING_SIZE) < 0 {
            return "Unknown error".to_string();
        }

        CStr::from_ptr(errbuf.as_ptr())
            .to_string_lossy()
            .to_string()
    }
}

#[derive(Debug, Clone)]
pub struct EncoderOptions<'a> {
    /// Force the codec to be used for encoding audio/video stream. If not provided the codec will
    /// be inferred from the output file extensions + preferred encoder (if specified).
    pub codec: Option<AVCodecID>,
    /// If several encoders available for the specified codec and/or container format here you can
    /// specify the ffmpeg-compatible name of the encoder that should be used. If nothing provided
    /// fallback toe the first available encoder for the specified output format.
    pub preferred_encoder: Option<&'a str>,
    /// Owned sibling of `preferred_encoder`. The registry-driven construction
    /// path produces an `EncoderConfig.codec_hint: Option<String>` that
    /// doesn't fit through the `Option<&'a str>` borrow. Callers may set
    /// either field; [`Self::effective_preferred_encoder`] returns
    /// whichever is present (owned takes precedence).
    pub preferred_encoder_owned: Option<String>,
    /// Pixel format used to store encoded frame. By default equals to AVPixelFormat::AV_PIX_FMT_YUV420P
    /// If not supported by the encoder the first supported pixel format will be used (which may
    /// lead to quality of alpha channel loss)
    ///
    /// Ignored for audio streams.
    /// @default AV_PIX_FMT_YUV420P
    pub pixel_format: AVPixelFormat,
    /// Sample format used to store encoded audio frame. By default equals to AvSampleFormat::AV_SAMPLE_FMT_FLTP
    /// Ignored for video streams.
    ///
    /// @default AV_SAMPLE_FMT_FLTP
    pub sample_format: AVSampleFormat,
    /// Target audio bitrate in bits,
    /// For video streams sometimes may not be needed and set dynamically based on the other codec
    /// params, like crf for libx264 and libx265.
    /// If not provided 192kb used for audios streams and set dynamically for video streams.
    pub bitrate: Option<i64>,
    /// Number of bits the bitstream is allowed to diverge from the reference.
    /// @default 0
    pub bitrate_tolerance: i32,
    /// Minimum quantizer
    /// @default 10
    pub qmin: i32,
    /// Maximum quantizer
    /// @default 51
    pub qmax: i32,
    ///  amount of qscale change between easy & hard scenes (0.0-1.0)
    pub qcompress: f32,
    /// maximum quantizer difference between frames
    /// @default 4
    pub max_qdiff: i32,
    /// Size of group of picture
    /// @default 12
    pub gop_size: i32,
    /// Specify the resulting sample rate of the audio stream (ignored for video streams)
    /// @default 44100
    pub sample_rate: usize,
    /// Dynamic set of options specific to encoder. Every encoder accepts its own purely dynamic set of options, e.g.
    /// the most popular example for h264 & h265 codecs are options like `-crf 18 -tune animation -preset ultrafast`.
    ///
    /// You can pass this set of options like:
    /// ```ignore
    /// let encoder_options = fframes::EncoderOptions { codec_params: Some(&[("crf", "23"), ("tune", "animation"), ("preset", "ultrafast")]),
    ///     ..Default::default()
    /// };
    /// ```
    ///
    /// Find available set of options for your codec using
    /// ```sh
    /// ffmpeg -h encoder={your_codec_name} -v quiet
    /// ```
    ///
    /// ## Safety
    /// ### Libav can have segmentation fault if some options are invalid or the value is not correct.
    /// ### So it is very important to validate the function parameters before usage
    /// ### because segfaults **won't be caught** by fframes.
    pub codec_params: Option<&'a [(&'a str, &'a str)]>,
    /// Tag used to identify video stream in the output file.
    /// to create a tag use the `MKTAG!` macro:
    ///
    /// ```ignore
    /// use fframes::MKTAG;
    /// let video_tag = MKTAG!('h', 'v', 'c', '1');
    /// ```
    pub tag: Option<isize>,
    /// High-level pixel format (bit depth + chroma + alpha). When set, this
    /// overrides `pixel_format` (the raw AVPixelFormat) for dispatch and
    /// codec selection.
    pub typed_pixel_format: Option<PixelFormat>,
    /// Color space metadata written into the output stream (`colr` atom, VUI).
    pub color_space: ColorSpaceDescriptor,
    /// Alpha handling. When not `None` the pipeline selects an
    /// alpha-carrying pixel format and enforces a compatible codec.
    pub alpha_mode: AlphaMode,
    /// Frame-rate as a rational (supports NTSC drop-frame precisely).
    pub frame_rate: Option<FrameRate>,
    /// Optional audio configuration. When `None` the legacy mono/FLTP
    /// behaviour is preserved.
    pub audio_spec: Option<AudioSpec>,
    /// When `true`, reject inputs that are not constant-frame-rate.
    pub enforce_cfr: bool,
    /// Optional output target. When `Some(OutputTarget::ImageSequence{..})`
    /// the renderer bypasses the muxer and writes per-frame image files
    /// (PNG/TIFF). When `None` or `Some(OutputTarget::Container{..})`,
    /// the legacy container path is used.
    pub output_target: Option<OutputTarget>,
}

impl EncoderOptions<'_> {
    /// Split video for concurrent rendering taking into account the GOP size
    /// to make sure that individual chunks are never less than 2xGOP size.
    pub fn split_gop_chunks(
        &self,
        duration_in_frames: usize,
        concurrency: usize,
    ) -> Vec<Range<usize>> {
        let gop_size = self.gop_size as usize;
        let min_chunk = 2 * gop_size;

        if duration_in_frames < min_chunk {
            #[allow(clippy::single_range_in_vec_init)]
            return vec![0..duration_in_frames];
        }

        let max_possible_chunks = duration_in_frames / min_chunk;
        let actual_chunks = concurrency.min(max_possible_chunks).max(1);
        let chunk_size = duration_in_frames.div_ceil(actual_chunks);

        let mut chunks = vec![];
        let mut prev_chunk = 0;

        while prev_chunk < duration_in_frames {
            let remaining = duration_in_frames - prev_chunk;
            if remaining > chunk_size {
                chunks.push(prev_chunk..prev_chunk + chunk_size);
                prev_chunk += chunk_size;
            } else {
                chunks.push(prev_chunk..prev_chunk + remaining);
                prev_chunk += remaining;
            }
        }
        chunks
    }
}

impl Default for EncoderOptions<'_> {
    fn default() -> Self {
        Self {
            bitrate: None,
            bitrate_tolerance: 0,
            codec: None,
            codec_params: None,
            gop_size: 24,
            max_qdiff: 4,
            pixel_format: AVPixelFormat::AV_PIX_FMT_YUV420P,
            preferred_encoder: None,
            preferred_encoder_owned: None,
            qcompress: 0.6,
            qmax: 60,
            qmin: 15,
            sample_format: AVSampleFormat::AV_SAMPLE_FMT_FLTP,
            sample_rate: 44100,
            tag: None,
            typed_pixel_format: None,
            color_space: ColorSpaceDescriptor::sdr_bt709(),
            alpha_mode: AlphaMode::None,
            frame_rate: None,
            audio_spec: None,
            enforce_cfr: false,
            output_target: None,
        }
    }
}

impl<'a> EncoderOptions<'a> {
    /// Resolve the effective preferred encoder name, preferring an owned
    /// value over the borrowed one. Stream-construction call sites should
    /// use this instead of reading `preferred_encoder` directly.
    pub fn effective_preferred_encoder(&self) -> Option<&str> {
        self.preferred_encoder_owned
            .as_deref()
            .or(self.preferred_encoder)
    }

    /// Adopt every video-stream-shaped field from a `VideoSpec` in one shot.
    /// This is the single source of truth: callers configure
    /// resolution / frame-rate / pixel-format / color-space / alpha via a
    /// `VideoSpec` and the encoder picks them up here without any scattered
    /// per-field plumbing.
    pub fn apply_video_spec(&mut self, spec: &VideoSpec) {
        self.typed_pixel_format = Some(spec.pixel_format());
        self.color_space = spec.color_space();
        self.alpha_mode = spec.alpha_mode();
        self.frame_rate = Some(spec.frame_rate);
    }

    /// Snapshot the encoder-side video parameters as a `VideoSpec`. Width
    /// and height aren't owned by `EncoderOptions` (they come from the
    /// `Video` trait at render time), so the caller supplies them.
    pub fn derive_video_spec(&self, width: u32, height: u32) -> VideoSpec {
        VideoSpec::new(width, height, self.frame_rate.unwrap_or_else(|| FrameRate::exact(30)))
            .with_pixel_format(self.effective_typed_pixel_format())
            .with_color_space(self.color_space)
            .with_alpha(self.alpha_mode)
    }

    /// Resolve the effective AVPixelFormat taking typed_pixel_format and
    /// alpha_mode into account. When alpha is enabled, force YUVA444P10LE.
    pub fn effective_pixel_format(&self) -> AVPixelFormat {
        if self.alpha_mode.carries_alpha() {
            return AVPixelFormat::AV_PIX_FMT_YUVA444P10LE;
        }
        if let Some(pf) = self.typed_pixel_format {
            return pf.to_av_pixel_format();
        }
        self.pixel_format
    }

    /// Resolve the effective PixelFormat type used for buffer conversion dispatch.
    pub fn effective_typed_pixel_format(&self) -> PixelFormat {
        if self.alpha_mode.carries_alpha() {
            return PixelFormat::Yuva444P10Le;
        }
        if let Some(pf) = self.typed_pixel_format {
            return pf;
        }
        match self.pixel_format {
            AVPixelFormat::AV_PIX_FMT_YUV420P10LE => PixelFormat::Yuv420P10Le,
            AVPixelFormat::AV_PIX_FMT_YUV444P => PixelFormat::Yuv444P,
            AVPixelFormat::AV_PIX_FMT_YUV444P10LE => PixelFormat::Yuv444P10Le,
            AVPixelFormat::AV_PIX_FMT_YUVA444P10LE => PixelFormat::Yuva444P10Le,
            AVPixelFormat::AV_PIX_FMT_RGBA => PixelFormat::Rgba8,
            AVPixelFormat::AV_PIX_FMT_RGBA64LE => PixelFormat::Rgba16,
            _ => PixelFormat::Yuv420P,
        }
    }
}

/// Decomposed encoder.
///
/// Two construction modes:
/// - `Encoder::new` (legacy / compat) — opens a file + FormatMuxer,
///   allocates AVStreams on the muxer, owns both codec contexts and the
///   muxer. `audio` → `video` → `muxer` drop order flushes through the
///   still-live muxer before destructors run.
/// - `Encoder::new_codec_only` (step 1 in the README) — allocates only the
///   AVCodecContext(s). `muxer` is `None`; a later
///   `Encoder::bind_to_file_sink` populates AVStreams on the sink's muxer.
pub struct Encoder {
    pub audio: Option<AudioStreamEncoder>,
    pub video: VideoStreamEncoder,
    /// Owned muxer on the legacy path; `None` in codec-only mode. When
    /// `None` the streams' `st` pointers live on a `FileOutputSink`'s
    /// muxer (bound via `bind_to_file_sink`).
    pub muxer: Option<FormatMuxer>,
    /// Non-owning AVFormatContext pointer set during
    /// `bind_to_file_sink`. Used by `send_frame` / `oc()` so the legacy
    /// write path keeps working against the sink-owned muxer.
    pub(crate) bound_oc: *mut AVFormatContext,
}

impl Encoder {
    /// Access the owned muxer if the encoder was built via the legacy
    /// `Encoder::new` path. Panics in codec-only mode.
    pub fn muxer(&self) -> &FormatMuxer {
        self.muxer
            .as_ref()
            .expect("Encoder::muxer called in codec-only mode (use bind_to_file_sink)")
    }
}

impl Encoder {
    /// Backward-compat accessor: the underlying video `Stream`.
    pub fn video_stream(&self) -> &Stream {
        &self.video.stream
    }

    /// Backward-compat accessor: the underlying audio `Stream`, if present.
    pub fn audio_stream(&self) -> Option<&Stream> {
        self.audio.as_ref().map(|a| &a.stream)
    }

    /// Backward-compat accessor: the AVFormatContext pointer. Returns the
    /// owned muxer's ptr on the legacy path, the sink-bound muxer's ptr
    /// after `bind_to_file_sink`, or null otherwise.
    pub(crate) fn oc(&self) -> *mut AVFormatContext {
        if let Some(m) = self.muxer.as_ref() {
            m.as_ptr()
        } else {
            self.bound_oc
        }
    }

    /// Construct an Encoder from already-built parts. Used by the remux path
    /// in `concatenator.rs` which copies codec parameters from an input file
    /// rather than allocating an encoder context.
    pub unsafe fn from_parts(
        muxer: FormatMuxer,
        video: VideoStreamEncoder,
        audio: Option<AudioStreamEncoder>,
    ) -> Self {
        Self {
            audio,
            video,
            muxer: Some(muxer),
            bound_oc: std::ptr::null_mut(),
        }
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe {
            // Flush both encoders. If we own the muxer (legacy path), drain
            // packets through it; in codec-only mode, just flush the codec
            // contexts (B-frames are discarded — callers must drain via
            // EncodingBackend::flush before Drop runs).
            if !self.video.stream.enc.is_null() {
                avcodec_flush_buffers(self.video.stream.enc);
                if let Some(muxer) = self.muxer.as_ref() {
                    let _ = self.video.flush(muxer);
                }
            }
            if let Some(audio) = self.audio.as_mut() {
                if !audio.stream.enc.is_null() {
                    avcodec_flush_buffers(audio.stream.enc);
                    if let Some(muxer) = self.muxer.as_ref() {
                        let _ = audio.flush(muxer);
                    }
                }
            }
            if let Some(muxer) = self.muxer.as_mut() {
                let _ = muxer.finish();
            }
        }
    }
}

impl Encoder {
    pub unsafe fn new(
        with_audio: bool,
        width: i32,
        height: i32,
        frame_rate: FrameRate,
        filename: &Path,
        render_options: &RenderOptions,
        logger: &Arc<dyn FFramesLogger>,
    ) -> RenderEncodingResult<Self> {
        unsafe {
            av_log_set_level(logger.get_libav_log_level());

            let mut muxer = FormatMuxer::open(filename)?;

            // Single source of truth: caller passes the resolved FrameRate.
            // EncoderOptions.frame_rate, when set, must agree — we honour it
            // strictly and surface a hard error if the caller-passed value
            // contradicts an explicit override. This eliminates the silent
            // drift between the two frame rate sources.
            let fr = match render_options.video_encoder_options.frame_rate {
                Some(opt_fr) if opt_fr != frame_rate => {
                    return Err(RenderEncodingError::InvalidArgument(format!(
                        "EncoderOptions.frame_rate ({:?}) disagrees with caller-passed FrameRate ({:?}); pick one source of truth",
                        opt_fr, frame_rate
                    )));
                }
                _ => frame_rate,
            };

            let video = VideoStreamEncoder::new(
                &muxer,
                width,
                height,
                fr,
                &render_options.video_encoder_options,
            )?;

            if logger.should_dump_format_info() {
                let c_filename = CString::new(filename.to_string_lossy().as_ref())
                    .map_err(RenderEncodingError::CStringError)?;
                av_dump_format(muxer.as_ptr(), 0, c_filename.as_ptr(), 1);
            }

            let audio = if with_audio {
                Some(AudioStreamEncoder::new(
                    &mut muxer,
                    &render_options.audio_encoder_options,
                )?)
            } else {
                None
            };

            muxer.write_header()?;

            Ok(Encoder {
                audio,
                video,
                muxer: Some(muxer),
                bound_oc: std::ptr::null_mut(),
            })
        }
    }

    /// Codec-only constructor (step 1 in the README): allocates AVCodecContexts
    /// for video (and optionally audio) without opening a file or owning a
    /// FormatMuxer. The caller pairs this encoder with a `FileOutputSink`
    /// via [`Encoder::bind_to_file_sink`] before writing packets through
    /// the sink.
    pub unsafe fn new_codec_only(
        with_audio: bool,
        width: i32,
        height: i32,
        frame_rate: FrameRate,
        render_options: &RenderOptions,
        video_codec_id: AVCodecID,
        audio_codec_id: Option<AVCodecID>,
    ) -> RenderEncodingResult<Self> {
        unsafe {
            let fr = match render_options.video_encoder_options.frame_rate {
                Some(opt_fr) if opt_fr != frame_rate => {
                    return Err(RenderEncodingError::InvalidArgument(format!(
                        "EncoderOptions.frame_rate ({:?}) disagrees with caller-passed FrameRate ({:?}); pick one source of truth",
                        opt_fr, frame_rate
                    )));
                }
                _ => frame_rate,
            };
            let video = VideoStreamEncoder::new_codec_only(
                width,
                height,
                fr,
                &render_options.video_encoder_options,
                video_codec_id,
            )?;
            let audio = if with_audio {
                let codec_id = audio_codec_id.ok_or_else(|| {
                    RenderEncodingError::Internal(
                        "Encoder::new_codec_only: with_audio requires audio_codec_id".to_owned(),
                    )
                })?;
                Some(AudioStreamEncoder::new_codec_only(
                    &render_options.audio_encoder_options,
                    codec_id,
                )?)
            } else {
                None
            };
            Ok(Self {
                audio,
                video,
                muxer: None,
                bound_oc: std::ptr::null_mut(),
            })
        }
    }

    /// Bind a codec-only encoder to a `FileOutputSink`: allocate AVStreams
    /// on the sink's muxer, copy codec params, apply any BWF metadata, and
    /// register codec-tb ↔ stream-tb bindings so the sink can rescale
    /// pts/dts at `write_packet` time.
    pub unsafe fn bind_to_file_sink(
        &mut self,
        sink: &mut FileOutputSink,
    ) -> EncodingResult<()> {
        unsafe {
            if self.muxer.is_some() {
                return Err(EncodingError::Backend(
                    "bind_to_file_sink: encoder already owns a muxer".into(),
                ));
            }
            {
                let sink_muxer = sink.muxer_mut().ok_or_else(|| {
                    EncodingError::Backend("bind_to_file_sink: sink has no muxer".into())
                })?;
                self.bound_oc = sink_muxer.as_ptr();
                self.video
                    .stream
                    .bind_to_muxer(sink_muxer, None)
                    .map_err(|e| EncodingError::Backend(format!("video bind_to_muxer: {e}")))?;
            }
            let video_stream_index = (*self.video.stream.st).index as u32;
            let video_codec_tb = (*self.video.stream.enc).time_base;
            let video_stream_tb = (*self.video.stream.st).time_base;
            sink.register_stream_binding(
                video_stream_index,
                (video_codec_tb.num, video_codec_tb.den),
                (video_stream_tb.num, video_stream_tb.den),
            );
            if let Some(audio) = self.audio.as_mut() {
                {
                    let sink_muxer = sink.muxer_mut().ok_or_else(|| {
                        EncodingError::Backend(
                            "bind_to_file_sink: sink has no muxer (audio)".into(),
                        )
                    })?;
                    audio
                        .stream
                        .bind_to_muxer(sink_muxer, None)
                        .map_err(|e| {
                            EncodingError::Backend(format!("audio bind_to_muxer: {e}"))
                        })?;
                    audio.apply_bwf_metadata_to_muxer(sink_muxer).map_err(|e| {
                        EncodingError::Backend(format!("BWF metadata: {e}"))
                    })?;
                }
                let audio_stream_index = (*audio.stream.st).index as u32;
                let audio_codec_tb = (*audio.stream.enc).time_base;
                let audio_stream_tb = (*audio.stream.st).time_base;
                sink.register_stream_binding(
                    audio_stream_index,
                    (audio_codec_tb.num, audio_codec_tb.den),
                    (audio_stream_tb.num, audio_stream_tb.den),
                );
            }
            Ok(())
        }
    }

    /// Registry-driven construction over the split
    /// [`crate::renderer::encoding_backend::VideoEncodingBackend`] /
    /// [`crate::renderer::encoding_backend::AudioEncodingBackend`] trait
    /// surface.
    ///
    /// The closure receives the video backend and, when audio was
    /// requested (`render_options.audio_encoder_options.audio_spec`), an
    /// audio backend. Both backends share a single [`FileOutputSink`] —
    /// the sink owns the muxer; each backend binds its stream via
    /// `bind_to_file_sink` so writes interleave correctly.
    ///
    /// Surfaces a clean error when no registered capability matches the
    /// requested container, rather than silently producing a file with a
    /// missing audio track.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn with_output_via_registry<T, F>(
        width: i32,
        height: i32,
        frame_rate: FrameRate,
        filename: &Path,
        output_format: crate::renderer::codec_capability::OutputFormat,
        render_options: &RenderOptions,
        logger: &Arc<dyn FFramesLogger>,
        inner_fn: &mut F,
    ) -> RenderEncodingResult<T>
    where
        T: 'static,
        F: FnMut(
            &mut dyn crate::renderer::encoding_backend::VideoEncodingBackend,
            Option<&mut dyn crate::renderer::encoding_backend::AudioEncodingBackend>,
        ) -> RenderEncodingResult<T>,
    {
        use crate::renderer::codec_capability::CodecRegistry;
        use crate::renderer::encoding_backend::{
            AudioEncoderConfig, AudioEncodingBackend, VideoEncoderConfig, VideoEncodingBackend,
        };

        unsafe {
            av_log_set_level(logger.get_libav_log_level());

            let fr = match render_options.video_encoder_options.frame_rate {
                Some(opt_fr) if opt_fr != frame_rate => {
                    return Err(RenderEncodingError::InvalidArgument(format!(
                        "EncoderOptions.frame_rate ({:?}) disagrees with caller-passed FrameRate ({:?}); pick one source of truth",
                        opt_fr, frame_rate
                    )));
                }
                _ => frame_rate,
            };

            let registry = CodecRegistry::populated_for_target();

            let video_cap = registry.find_best_video(output_format).ok_or_else(|| {
                RenderEncodingError::Internal(format!(
                    "CodecRegistry: no video capability for {output_format:?}"
                ))
            })?;

            let video_config = VideoEncoderConfig {
                width: width as u32,
                height: height as u32,
                frame_rate: fr,
                pixel_format: render_options
                    .video_encoder_options
                    .effective_typed_pixel_format(),
                alpha_mode: render_options.video_encoder_options.alpha_mode,
                color_space: render_options.video_encoder_options.color_space,
                gop_size: render_options.video_encoder_options.gop_size.max(0) as u32,
                target_bitrate: render_options
                    .video_encoder_options
                    .bitrate
                    .filter(|b| *b > 0)
                    .map(|b| b as u64),
                codec_hint: render_options
                    .video_encoder_options
                    .preferred_encoder_owned
                    .clone(),
            };

            let audio_spec = render_options.audio_encoder_options.audio_spec.clone();

            let audio_cap_and_config: Option<(
                &dyn crate::renderer::codec_capability::AudioCodecCapability,
                AudioEncoderConfig,
            )> = if let Some(spec) = audio_spec {
                let audio_cap = registry.find_best_audio(output_format).ok_or_else(|| {
                    RenderEncodingError::Internal(format!(
                        "CodecRegistry: audio requested but no audio capability for {output_format:?}"
                    ))
                })?;
                let audio_config = AudioEncoderConfig {
                    spec,
                    target_bitrate: render_options
                        .audio_encoder_options
                        .bitrate
                        .filter(|b| *b > 0)
                        .map(|b| b as u64),
                    codec_hint: render_options
                        .audio_encoder_options
                        .preferred_encoder_owned
                        .clone(),
                };
                Some((audio_cap, audio_config))
            } else {
                None
            };

            let mut sink = FileOutputSink::new(filename).map_err(|e| {
                RenderEncodingError::Internal(format!("FileOutputSink::new: {e}"))
            })?;

            let mut video_backend: Box<dyn VideoEncodingBackend> = video_cap
                .create_video_encoder(&video_config)
                .map_err(|e| RenderEncodingError::Internal(format!("create_video_encoder: {e}")))?;
            video_backend
                .open(&video_config)
                .map_err(|e| RenderEncodingError::Internal(format!("video open: {e}")))?;

            let mut audio_backend: Option<Box<dyn AudioEncodingBackend>> = match audio_cap_and_config {
                Some((cap, cfg)) => {
                    let mut backend = cap.create_audio_encoder(&cfg).map_err(|e| {
                        RenderEncodingError::Internal(format!("create_audio_encoder: {e}"))
                    })?;
                    backend
                        .open(&cfg)
                        .map_err(|e| RenderEncodingError::Internal(format!("audio open: {e}")))?;
                    Some(backend)
                }
                None => None,
            };

            // Native: bind the FFmpeg-backed stream encoders to the sink's
            // muxer. `with_output_via_registry` is the only caller of the
            // native registry today, and it always produces
            // `FFmpegVideoBackend` / `FFmpegAudioBackend` — downcast
            // directly via the concrete type.
            #[cfg(not(target_arch = "wasm32"))]
            {
                use crate::renderer::encoding_backend::{FFmpegAudioBackend, FFmpegVideoBackend};
                let video_ptr: *mut dyn VideoEncodingBackend = video_backend.as_mut();
                let ffvideo = video_ptr as *mut FFmpegVideoBackend;
                (*ffvideo).bind_to_file_sink(&mut sink).map_err(|e| {
                    RenderEncodingError::Internal(format!("video bind_to_file_sink: {e}"))
                })?;
                if let Some(audio_box) = audio_backend.as_mut() {
                    let audio_ptr: *mut dyn AudioEncodingBackend = audio_box.as_mut();
                    let ffaudio = audio_ptr as *mut FFmpegAudioBackend;
                    (*ffaudio).bind_to_file_sink(&mut sink).map_err(|e| {
                        RenderEncodingError::Internal(format!("audio bind_to_file_sink: {e}"))
                    })?;
                }
            }

            sink.write_header().map_err(|e| {
                RenderEncodingError::Internal(format!("sink.write_header: {e}"))
            })?;

            // Stash the audio backend as a raw pointer across the
            // inner_fn call so rustc doesn't extend a `&mut` reborrow
            // into the generic `T` return value. The Box<dyn ...> is
            // reconstructed afterwards for flush/close.
            let audio_raw: Option<*mut (dyn AudioEncodingBackend)> =
                audio_backend.take().map(|b| Box::into_raw(b));
            let result: RenderEncodingResult<T> = {
                let a_ref: Option<
                    &mut dyn crate::renderer::encoding_backend::AudioEncodingBackend,
                > = audio_raw.map(|p| &mut *p);
                inner_fn(video_backend.as_mut(), a_ref)
            };

            let _ = video_backend.flush();
            let _ = video_backend.close();
            if let Some(raw) = audio_raw {
                let mut a: Box<dyn AudioEncodingBackend> = Box::from_raw(raw);
                let _ = a.flush();
                let _ = a.close();
            }

            use crate::renderer::output_sink::OutputSink;
            sink.finalize().map_err(|e| {
                RenderEncodingError::Internal(format!("sink.finalize: {e}"))
            })?;

            result
        }
    }

    /// Run `inner_fn` with a muxer-bound Encoder whose output is a
    /// `FileOutputSink` at `filename`. Decoupled path (step 6 in the README):
    /// the sink owns the FormatMuxer, the encoder holds only codec
    /// contexts and borrows the sink's muxer via `bind_to_file_sink`.
    ///
    /// Signature unchanged from the legacy path so existing callers (tests,
    /// sf-video, cpu.rs) keep working; internally wired through
    /// FileOutputSink.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn with_output<T, F: FnMut(&mut Encoder) -> RenderEncodingResult<T>>(
        with_audio: bool,
        width: i32,
        height: i32,
        frame_rate: FrameRate,
        filename: &Path,
        render_options: &RenderOptions,
        logger: &Arc<dyn FFramesLogger>,
        inner_fn: &mut F,
    ) -> RenderEncodingResult<T> {
        unsafe {
            Self::with_output_using_codecs(
                with_audio,
                width,
                height,
                frame_rate,
                filename,
                render_options,
                logger,
                AVCodecID::AV_CODEC_ID_NONE,
                None,
                inner_fn,
            )
        }
    }

    /// Shared body for `with_output` and `with_output_via_registry`. When
    /// `video_codec_override == AV_CODEC_ID_NONE`, falls back to the
    /// container's default video codec (legacy behaviour). When
    /// `audio_codec_override` is `None` and `with_audio`, falls back to the
    /// container's default audio codec.
    #[allow(clippy::too_many_arguments)]
    unsafe fn with_output_using_codecs<
        T,
        F: FnMut(&mut Encoder) -> RenderEncodingResult<T>,
    >(
        with_audio: bool,
        width: i32,
        height: i32,
        frame_rate: FrameRate,
        filename: &Path,
        render_options: &RenderOptions,
        logger: &Arc<dyn FFramesLogger>,
        video_codec_override: AVCodecID,
        audio_codec_override: Option<AVCodecID>,
        inner_fn: &mut F,
    ) -> RenderEncodingResult<T> {
        unsafe {
            av_log_set_level(logger.get_libav_log_level());

            let fr = match render_options.video_encoder_options.frame_rate {
                Some(opt_fr) if opt_fr != frame_rate => {
                    return Err(RenderEncodingError::InvalidArgument(format!(
                        "EncoderOptions.frame_rate ({:?}) disagrees with caller-passed FrameRate ({:?}); pick one source of truth",
                        opt_fr, frame_rate
                    )));
                }
                _ => frame_rate,
            };

            // Open the sink (owns the FormatMuxer) separately from the
            // encoder (step 6 in the README).
            let mut sink = FileOutputSink::new(filename)
                .map_err(|e| RenderEncodingError::Internal(format!("FileOutputSink::new: {e}")))?;

            // Video codec: prefer the caller's override (registry-driven
            // path); fall back to the container default when unset.
            let video_codec_id = if video_codec_override != AVCodecID::AV_CODEC_ID_NONE {
                video_codec_override
            } else {
                let muxer_ptr = sink
                    .muxer_mut()
                    .ok_or_else(|| {
                        RenderEncodingError::Internal("sink has no muxer".to_owned())
                    })?
                    .as_ptr();
                (*(*muxer_ptr).oformat).video_codec
            };
            let audio_codec_id = if with_audio {
                Some(match audio_codec_override {
                    Some(id) => id,
                    None => {
                        let muxer_ptr = sink
                            .muxer_mut()
                            .ok_or_else(|| {
                                RenderEncodingError::Internal("sink has no muxer".to_owned())
                            })?
                            .as_ptr();
                        (*(*muxer_ptr).oformat).audio_codec
                    }
                })
            } else {
                None
            };

            let mut encoder = Encoder::new_codec_only(
                with_audio,
                width,
                height,
                fr,
                render_options,
                video_codec_id,
                audio_codec_id,
            )?;

            encoder
                .bind_to_file_sink(&mut sink)
                .map_err(|e| RenderEncodingError::Internal(format!("bind_to_file_sink: {e}")))?;

            if logger.should_dump_format_info() {
                let c_filename = CString::new(filename.to_string_lossy().as_ref())
                    .map_err(RenderEncodingError::CStringError)?;
                av_dump_format(encoder.oc(), 0, c_filename.as_ptr(), 1);
            }

            sink.write_header().map_err(|e| {
                RenderEncodingError::Internal(format!("sink.write_header: {e}"))
            })?;

            // Flip the sink-owned muxer's header_written flag too — the
            // existing compat write paths (`VideoStreamEncoder::write_frame`,
            // `Encoder::send_frame`) call `av_interleaved_write_frame`
            // against the AVFormatContext directly; the FormatMuxer's
            // header_written was set by sink.write_header().

            let result = inner_fn(&mut encoder);

            // Explicit flush + finalise sequence. The codec contexts are
            // drained into the sink-bound muxer via the encoder's compat
            // write path (which uses `oc()` → `bound_oc`). We must flush
            // before dropping the encoder, because Drop sees
            // `self.muxer.is_none()` and skips the flush.
            if !encoder.video.stream.enc.is_null() {
                avcodec_flush_buffers(encoder.video.stream.enc);
                let muxer_ref_for_flush = sink.muxer_mut().ok_or_else(|| {
                    RenderEncodingError::Internal("sink muxer gone before flush".to_owned())
                })?;
                let _ = encoder.video.flush(muxer_ref_for_flush);
            }
            if let Some(audio) = encoder.audio.as_mut() {
                if !audio.stream.enc.is_null() {
                    avcodec_flush_buffers(audio.stream.enc);
                    let muxer_ref_for_flush = sink.muxer_mut().ok_or_else(|| {
                        RenderEncodingError::Internal("sink muxer gone before flush".to_owned())
                    })?;
                    let _ = audio.flush(muxer_ref_for_flush);
                }
            }
            // bound_oc references sink's muxer; clear before the sink
            // finalises (and drops the muxer) to avoid dangling pointer.
            encoder.bound_oc = std::ptr::null_mut();
            // Null out the stream AVStream pointers too — they live on
            // the sink's muxer which is about to be freed.
            encoder.video.stream.st = std::ptr::null_mut();
            if let Some(audio) = encoder.audio.as_mut() {
                audio.stream.st = std::ptr::null_mut();
            }

            use crate::renderer::output_sink::OutputSink;
            sink.finalize().map_err(|e| {
                RenderEncodingError::Internal(format!("sink.finalize: {e}"))
            })?;

            result
        }
    }

    pub unsafe fn send_customizable_frame_packet<F: Fn(*mut AVPacket) -> i32>(
        &self,
        stream: &stream::Stream,
        EncoderFrame {
            av_frame: frame,
            packet,
            ..
        }: &EncoderFrame,
        customize_frame: F,
    ) -> RenderEncodingResult<()> {
        unsafe {
            let avcodec_send_frame = avcodec_send_frame(stream.enc, *frame);
            let mut status = avcodec_send_frame;

            if status < 0 {
                let error_description = av_error_to_string(status);

                return Err(renderer_error::RenderEncodingError::CantWriteFrame(
                    error_description,
                ));
            }

            while status >= 0 {
                status = avcodec_receive_packet(stream.enc, *packet);

                match status {
                    status if status == AVERROR_EOF => break,
                    status if status == ffmpeg_averror(EAGAIN) => {
                        break;
                    }
                    _ => status = customize_frame(*packet),
                }
            }

            Ok(())
        }
    }

    pub unsafe fn send_frame(
        &self,
        stream: &stream::Stream,
        frame: &EncoderFrame,
    ) -> RenderEncodingResult<()> {
        unsafe {
            let oc = self.oc();

            self.send_customizable_frame_packet(stream, frame, |packet| {
                av_packet_rescale_ts(packet, (*stream.enc).time_base, (*stream.st).time_base);

                (*packet).stream_index = (*stream.st).index;
                av_interleaved_write_frame(oc, packet)
            })
        }
    }

    pub unsafe fn submit_leftover_b_frames(
        &self,
        frame: &EncoderFrame,
        stream: &stream::Stream,
        expected_frames_in_stream: usize,
    ) -> RenderEncodingResult<()> {
        unsafe {
            let submitted_frames = self.video_stream().get_frames_in_stream() as usize;

            if submitted_frames < expected_frames_in_stream {
                for _ in 0..expected_frames_in_stream - submitted_frames {
                    self.send_frame(stream, frame)?;
                }
            }

            Ok(())
        }
    }
}

unsafe impl Send for Encoder {}
unsafe impl Sync for Encoder {}

