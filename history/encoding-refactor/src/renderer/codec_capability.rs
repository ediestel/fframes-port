//! Trait-based codec registry — split into video and audio capabilities.
//!
//! Each backend registers its codecs as either a [`VideoCodecCapability`]
//! or an [`AudioCodecCapability`]. The registry keeps the two kinds in
//! separate vectors: [`CodecRegistry::find_best_video`] only ever returns
//! a video codec, [`CodecRegistry::find_best_audio`] only audio. A
//! single-stream codec (rav1e, openh264) is registered as
//! `VideoCodecCapability` only — it cannot be mis-selected for an audio
//! slot because the trait surface doesn't expose audio construction.

use crate::encoding_backend::{
    AudioEncoderConfig, AudioEncodingBackend, EncodingResult, VideoEncoderConfig,
    VideoEncodingBackend,
};

/// Portable codec identifier.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum CodecId {
    // Video
    H264,
    H265,
    Vp8,
    Vp9,
    Av1,
    ProRes,
    DnxHd,
    OpenExr,
    // Audio
    Aac,
    Opus,
    Mp3,
    Pcm16,
    Flac,
}

/// Which stream kind a codec produces.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MediaKind {
    Video,
    Audio,
}

impl CodecId {
    pub const fn media_kind(&self) -> MediaKind {
        match self {
            CodecId::H264
            | CodecId::H265
            | CodecId::Vp8
            | CodecId::Vp9
            | CodecId::Av1
            | CodecId::ProRes
            | CodecId::DnxHd
            | CodecId::OpenExr => MediaKind::Video,
            CodecId::Aac
            | CodecId::Opus
            | CodecId::Mp3
            | CodecId::Pcm16
            | CodecId::Flac => MediaKind::Audio,
        }
    }
}

/// Portable container / stream format for output selection.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum OutputFormat {
    Mp4,
    Mov,
    WebM,
    Mkv,
    Mp3,
    Wav,
    Flac,
    ImageSequencePng,
    ImageSequenceTiff,
    ImageSequenceOpenExr,
}

/// Short description of a codec.
#[derive(Clone, Debug)]
pub struct CodecDescription {
    pub codec_id: CodecId,
    pub backend_name: String,
    pub hardware_accelerated: bool,
}

/// Shared base for every codec capability — identity and format filter.
pub trait CodecCapability: Send + Sync {
    fn codec_id(&self) -> CodecId;
    fn supports_format(&self, fmt: OutputFormat) -> bool;
    fn priority(&self) -> i32 {
        0
    }
    fn description(&self) -> CodecDescription;
}

/// Produces a [`VideoEncodingBackend`] on demand.
pub trait VideoCodecCapability: CodecCapability {
    fn create_video_encoder(
        &self,
        config: &VideoEncoderConfig,
    ) -> EncodingResult<Box<dyn VideoEncodingBackend>>;
}

/// Produces an [`AudioEncodingBackend`] on demand.
pub trait AudioCodecCapability: CodecCapability {
    fn create_audio_encoder(
        &self,
        config: &AudioEncoderConfig,
    ) -> EncodingResult<Box<dyn AudioEncodingBackend>>;
}

/// Trait-based codec registry with separate video and audio vectors.
pub struct CodecRegistry {
    video: Vec<Box<dyn VideoCodecCapability>>,
    audio: Vec<Box<dyn AudioCodecCapability>>,
}

impl CodecRegistry {
    pub fn new() -> Self {
        Self {
            video: Vec::new(),
            audio: Vec::new(),
        }
    }

    pub fn register_video(&mut self, cap: Box<dyn VideoCodecCapability>) {
        self.video.push(cap);
    }

    pub fn register_audio(&mut self, cap: Box<dyn AudioCodecCapability>) {
        self.audio.push(cap);
    }

    pub fn populated_for_target() -> Self {
        let mut registry = Self::new();
        #[cfg(not(target_arch = "wasm32"))]
        ffmpeg_caps::register_all(&mut registry);
        #[cfg(all(target_arch = "wasm32", feature = "webcodecs"))]
        wasm_caps::register_webcodecs(&mut registry);
        #[cfg(all(target_arch = "wasm32", feature = "wasm-openh264"))]
        wasm_caps::register_openh264(&mut registry);
        #[cfg(all(target_arch = "wasm32", feature = "wasm-rav1e"))]
        wasm_caps::register_rav1e(&mut registry);
        registry
    }

    pub fn find_best_video(&self, fmt: OutputFormat) -> Option<&dyn VideoCodecCapability> {
        self.video
            .iter()
            .filter(|c| c.supports_format(fmt))
            .max_by_key(|c| c.priority())
            .map(|b| b.as_ref())
    }

    pub fn find_best_audio(&self, fmt: OutputFormat) -> Option<&dyn AudioCodecCapability> {
        self.audio
            .iter()
            .filter(|c| c.supports_format(fmt))
            .max_by_key(|c| c.priority())
            .map(|b| b.as_ref())
    }

    pub fn find_video_by_codec(&self, codec: CodecId) -> Option<&dyn VideoCodecCapability> {
        self.video
            .iter()
            .filter(|c| c.codec_id() == codec)
            .max_by_key(|c| c.priority())
            .map(|b| b.as_ref())
    }

    pub fn find_audio_by_codec(&self, codec: CodecId) -> Option<&dyn AudioCodecCapability> {
        self.audio
            .iter()
            .filter(|c| c.codec_id() == codec)
            .max_by_key(|c| c.priority())
            .map(|b| b.as_ref())
    }

    pub fn probe(&self) -> Vec<CodecDescription> {
        self.video
            .iter()
            .map(|c| c.description())
            .chain(self.audio.iter().map(|c| c.description()))
            .collect()
    }

    pub fn video_len(&self) -> usize {
        self.video.len()
    }

    pub fn audio_len(&self) -> usize {
        self.audio.len()
    }

    pub fn len(&self) -> usize {
        self.video.len() + self.audio.len()
    }

    pub fn is_empty(&self) -> bool {
        self.video.is_empty() && self.audio.is_empty()
    }
}

impl Default for CodecRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// FFmpeg-backed capabilities (native).
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
mod ffmpeg_caps {
    use super::*;
    use crate::media::ffmpeg_sys_fframes::AVCodecID;
    use crate::renderer::codec_registry::FFmpegEncoderCatalog;
    use crate::renderer::encoding_backend::{FFmpegAudioBackend, FFmpegVideoBackend};

    struct FFmpegVideoCap {
        codec_id: CodecId,
        ffmpeg_codec_id: AVCodecID,
        formats: &'static [OutputFormat],
        priority: i32,
    }

    impl CodecCapability for FFmpegVideoCap {
        fn codec_id(&self) -> CodecId {
            self.codec_id
        }

        fn supports_format(&self, fmt: OutputFormat) -> bool {
            if !FFmpegEncoderCatalog::global().is_available(self.ffmpeg_codec_id) {
                return false;
            }
            self.formats.contains(&fmt)
        }

        fn priority(&self) -> i32 {
            self.priority
        }

        fn description(&self) -> CodecDescription {
            let name = format!("ffmpeg:{:?}", self.ffmpeg_codec_id);
            let hw = FFmpegEncoderCatalog::global()
                .capabilities_for_codec(self.ffmpeg_codec_id)
                .map(|infos| infos.iter().any(|i| i.hardware_accelerated))
                .unwrap_or(false);
            CodecDescription {
                codec_id: self.codec_id,
                backend_name: name,
                hardware_accelerated: hw,
            }
        }
    }

    impl VideoCodecCapability for FFmpegVideoCap {
        fn create_video_encoder(
            &self,
            _config: &VideoEncoderConfig,
        ) -> EncodingResult<Box<dyn VideoEncodingBackend>> {
            Ok(Box::new(FFmpegVideoBackend::from_codec_id(
                self.ffmpeg_codec_id,
            )))
        }
    }

    struct FFmpegAudioCap {
        codec_id: CodecId,
        ffmpeg_codec_id: AVCodecID,
        formats: &'static [OutputFormat],
        priority: i32,
    }

    impl CodecCapability for FFmpegAudioCap {
        fn codec_id(&self) -> CodecId {
            self.codec_id
        }

        fn supports_format(&self, fmt: OutputFormat) -> bool {
            if !FFmpegEncoderCatalog::global().is_available(self.ffmpeg_codec_id) {
                return false;
            }
            self.formats.contains(&fmt)
        }

        fn priority(&self) -> i32 {
            self.priority
        }

        fn description(&self) -> CodecDescription {
            let name = format!("ffmpeg:{:?}", self.ffmpeg_codec_id);
            let hw = FFmpegEncoderCatalog::global()
                .capabilities_for_codec(self.ffmpeg_codec_id)
                .map(|infos| infos.iter().any(|i| i.hardware_accelerated))
                .unwrap_or(false);
            CodecDescription {
                codec_id: self.codec_id,
                backend_name: name,
                hardware_accelerated: hw,
            }
        }
    }

    impl AudioCodecCapability for FFmpegAudioCap {
        fn create_audio_encoder(
            &self,
            _config: &AudioEncoderConfig,
        ) -> EncodingResult<Box<dyn AudioEncodingBackend>> {
            Ok(Box::new(FFmpegAudioBackend::from_codec_id(
                self.ffmpeg_codec_id,
            )))
        }
    }

    pub(super) fn register_all(registry: &mut CodecRegistry) {
        // H.264 is the MP4/MOV default codec; bump its priority above
        // other candidates that also claim MP4 (e.g. AV1) so that
        // `find_best_video(Mp4)` returns H.264 without an explicit hint.
        registry.register_video(Box::new(FFmpegVideoCap {
            codec_id: CodecId::H264,
            ffmpeg_codec_id: AVCodecID::AV_CODEC_ID_H264,
            formats: &[OutputFormat::Mp4, OutputFormat::Mov, OutputFormat::Mkv],
            priority: 15,
        }));
        registry.register_video(Box::new(FFmpegVideoCap {
            codec_id: CodecId::H265,
            ffmpeg_codec_id: AVCodecID::AV_CODEC_ID_HEVC,
            formats: &[OutputFormat::Mp4, OutputFormat::Mov, OutputFormat::Mkv],
            priority: 10,
        }));
        registry.register_video(Box::new(FFmpegVideoCap {
            codec_id: CodecId::Vp9,
            ffmpeg_codec_id: AVCodecID::AV_CODEC_ID_VP9,
            formats: &[OutputFormat::WebM, OutputFormat::Mkv],
            priority: 10,
        }));
        registry.register_video(Box::new(FFmpegVideoCap {
            codec_id: CodecId::Av1,
            ffmpeg_codec_id: AVCodecID::AV_CODEC_ID_AV1,
            formats: &[OutputFormat::Mp4, OutputFormat::WebM, OutputFormat::Mkv],
            priority: 10,
        }));
        registry.register_video(Box::new(FFmpegVideoCap {
            codec_id: CodecId::ProRes,
            ffmpeg_codec_id: AVCodecID::AV_CODEC_ID_PRORES,
            formats: &[OutputFormat::Mov],
            priority: 10,
        }));
        registry.register_video(Box::new(FFmpegVideoCap {
            codec_id: CodecId::DnxHd,
            ffmpeg_codec_id: AVCodecID::AV_CODEC_ID_DNXHD,
            formats: &[OutputFormat::Mov, OutputFormat::Mkv],
            priority: 10,
        }));

        registry.register_audio(Box::new(FFmpegAudioCap {
            codec_id: CodecId::Aac,
            ffmpeg_codec_id: AVCodecID::AV_CODEC_ID_AAC,
            formats: &[OutputFormat::Mp4, OutputFormat::Mov, OutputFormat::Mkv],
            priority: 10,
        }));
        registry.register_audio(Box::new(FFmpegAudioCap {
            codec_id: CodecId::Opus,
            ffmpeg_codec_id: AVCodecID::AV_CODEC_ID_OPUS,
            formats: &[OutputFormat::WebM, OutputFormat::Mkv],
            priority: 10,
        }));
        registry.register_audio(Box::new(FFmpegAudioCap {
            codec_id: CodecId::Mp3,
            ffmpeg_codec_id: AVCodecID::AV_CODEC_ID_MP3,
            formats: &[OutputFormat::Mp3, OutputFormat::Mkv],
            priority: 10,
        }));
    }
}

// ---------------------------------------------------------------------------
// WASM-backed capabilities (wasm32 only).
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
mod wasm_caps {
    use super::*;

    #[cfg(feature = "webcodecs")]
    pub(super) struct WebCodecsVideoCap {
        codec_id: CodecId,
        codec_string: &'static str,
        formats: &'static [OutputFormat],
    }

    #[cfg(feature = "webcodecs")]
    impl CodecCapability for WebCodecsVideoCap {
        fn codec_id(&self) -> CodecId {
            self.codec_id
        }
        fn supports_format(&self, fmt: OutputFormat) -> bool {
            self.formats.contains(&fmt)
        }
        fn priority(&self) -> i32 {
            20
        }
        fn description(&self) -> CodecDescription {
            CodecDescription {
                codec_id: self.codec_id,
                backend_name: format!("webcodecs:{}", self.codec_string),
                hardware_accelerated: true,
            }
        }
    }

    #[cfg(feature = "webcodecs")]
    impl VideoCodecCapability for WebCodecsVideoCap {
        fn create_video_encoder(
            &self,
            config: &VideoEncoderConfig,
        ) -> EncodingResult<Box<dyn VideoEncodingBackend>> {
            let mut cfg = config.clone();
            if cfg.codec_hint.is_none() {
                cfg.codec_hint = Some(self.codec_string.to_owned());
            }
            let _ = cfg;
            Ok(Box::new(crate::wasm_encode::WasmVideoBackend::new_sinkless()))
        }
    }

    #[cfg(feature = "webcodecs")]
    pub(super) struct WebCodecsAudioCap {
        codec_id: CodecId,
        codec_string: &'static str,
        formats: &'static [OutputFormat],
    }

    #[cfg(feature = "webcodecs")]
    impl CodecCapability for WebCodecsAudioCap {
        fn codec_id(&self) -> CodecId {
            self.codec_id
        }
        fn supports_format(&self, fmt: OutputFormat) -> bool {
            self.formats.contains(&fmt)
        }
        fn priority(&self) -> i32 {
            20
        }
        fn description(&self) -> CodecDescription {
            CodecDescription {
                codec_id: self.codec_id,
                backend_name: format!("webcodecs:{}", self.codec_string),
                hardware_accelerated: true,
            }
        }
    }

    #[cfg(feature = "webcodecs")]
    impl AudioCodecCapability for WebCodecsAudioCap {
        fn create_audio_encoder(
            &self,
            config: &AudioEncoderConfig,
        ) -> EncodingResult<Box<dyn AudioEncodingBackend>> {
            let mut cfg = config.clone();
            if cfg.codec_hint.is_none() {
                cfg.codec_hint = Some(self.codec_string.to_owned());
            }
            let _ = cfg;
            Ok(Box::new(crate::wasm_encode::WasmAudioBackend::new_sinkless()))
        }
    }

    #[cfg(feature = "webcodecs")]
    pub(super) fn register_webcodecs(registry: &mut CodecRegistry) {
        registry.register_video(Box::new(WebCodecsVideoCap {
            codec_id: CodecId::H264,
            codec_string: "avc1.42E01E",
            formats: &[OutputFormat::Mp4, OutputFormat::Mov],
        }));
        registry.register_video(Box::new(WebCodecsVideoCap {
            codec_id: CodecId::Vp9,
            codec_string: "vp09.00.10.08",
            formats: &[OutputFormat::WebM, OutputFormat::Mkv],
        }));
        registry.register_video(Box::new(WebCodecsVideoCap {
            codec_id: CodecId::Av1,
            codec_string: "av01.0.04M.08",
            formats: &[OutputFormat::Mp4, OutputFormat::WebM, OutputFormat::Mkv],
        }));
        registry.register_audio(Box::new(WebCodecsAudioCap {
            codec_id: CodecId::Aac,
            codec_string: "mp4a.40.2",
            formats: &[OutputFormat::Mp4, OutputFormat::Mov, OutputFormat::Mkv],
        }));
        registry.register_audio(Box::new(WebCodecsAudioCap {
            codec_id: CodecId::Opus,
            codec_string: "opus",
            formats: &[OutputFormat::WebM, OutputFormat::Mkv],
        }));
    }

    #[cfg(feature = "wasm-openh264")]
    pub(super) struct OpenH264Cap;

    #[cfg(feature = "wasm-openh264")]
    impl CodecCapability for OpenH264Cap {
        fn codec_id(&self) -> CodecId {
            CodecId::H264
        }
        fn supports_format(&self, fmt: OutputFormat) -> bool {
            matches!(fmt, OutputFormat::Mp4 | OutputFormat::Mov | OutputFormat::Mkv)
        }
        fn priority(&self) -> i32 {
            5
        }
        fn description(&self) -> CodecDescription {
            CodecDescription {
                codec_id: CodecId::H264,
                backend_name: "openh264:wasm32".into(),
                hardware_accelerated: false,
            }
        }
    }

    #[cfg(feature = "wasm-openh264")]
    impl VideoCodecCapability for OpenH264Cap {
        fn create_video_encoder(
            &self,
            _config: &VideoEncoderConfig,
        ) -> EncodingResult<Box<dyn VideoEncodingBackend>> {
            // `openh264` requires a C++ toolchain, which isn't available
            // on wasm32-unknown-unknown. The cap still registers so
            // `probe()` reports the build intent, but we can't hand out
            // a working encoder here — fall through to the error path.
            #[cfg(not(target_arch = "wasm32"))]
            {
                return Ok(Box::new(
                    crate::wasm_encode_openh264::OpenH264Backend::new(),
                ));
            }
            #[cfg(target_arch = "wasm32")]
            {
                Err(EncodingError::CodecUnavailable(
                    "openh264 is unavailable on wasm32-unknown-unknown \
                     (requires a C++ toolchain — rebuild for emscripten/wasi)"
                        .into(),
                ))
            }
        }
    }

    #[cfg(feature = "wasm-openh264")]
    pub(super) fn register_openh264(registry: &mut CodecRegistry) {
        registry.register_video(Box::new(OpenH264Cap));
    }

    #[cfg(feature = "wasm-rav1e")]
    pub(super) struct Rav1eCap;

    #[cfg(feature = "wasm-rav1e")]
    impl CodecCapability for Rav1eCap {
        fn codec_id(&self) -> CodecId {
            CodecId::Av1
        }
        fn supports_format(&self, fmt: OutputFormat) -> bool {
            matches!(fmt, OutputFormat::Mp4 | OutputFormat::WebM | OutputFormat::Mkv)
        }
        fn priority(&self) -> i32 {
            5
        }
        fn description(&self) -> CodecDescription {
            CodecDescription {
                codec_id: CodecId::Av1,
                backend_name: "rav1e:wasm32".into(),
                hardware_accelerated: false,
            }
        }
    }

    #[cfg(feature = "wasm-rav1e")]
    impl VideoCodecCapability for Rav1eCap {
        fn create_video_encoder(
            &self,
            _config: &VideoEncoderConfig,
        ) -> EncodingResult<Box<dyn VideoEncodingBackend>> {
            Ok(Box::new(crate::wasm_encode_rav1e::Rav1eBackend::new()))
        }
    }

    #[cfg(feature = "wasm-rav1e")]
    pub(super) fn register_rav1e(registry: &mut CodecRegistry) {
        registry.register_video(Box::new(Rav1eCap));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_registry_finds_nothing() {
        let r = CodecRegistry::new();
        assert!(r.find_best_video(OutputFormat::Mp4).is_none());
        assert!(r.find_best_audio(OutputFormat::Mp4).is_none());
        assert_eq!(r.probe().len(), 0);
    }

    #[test]
    fn populated_registry_has_entries() {
        let r = CodecRegistry::populated_for_target();
        assert!(!r.is_empty());
        assert!(r.find_best_video(OutputFormat::Mp4).is_some());
    }
}
