//! FFmpeg-side encoder catalog.
//!
//! This module inspects the linked FFmpeg library at startup to discover
//! which encoders are available and reports their capabilities. It is used
//! by `FFmpegCodecCapability` (in `codec_capability.rs`) as the FFmpeg
//! backend's source of truth, and directly by internal FFmpeg plumbing in
//! `renderer/stream.rs` where per-encoder details (name, alpha support)
//! matter.
//!
//! For the portable trait-based codec registry exposed to callers, see
//! `codec_capability::CodecRegistry`.

use crate::media::ffmpeg_sys_fframes::{AVCodec, AVCodecID, av_codec_iterate, avcodec_find_encoder};
use std::collections::HashMap;
use std::ffi::CStr;
use std::sync::OnceLock;

#[derive(Clone, Debug)]
pub struct FFmpegEncoderInfo {
    pub name: String,
    pub supports_10bit: bool,
    pub supports_alpha: bool,
    pub hardware_accelerated: bool,
}

#[derive(Debug)]
pub struct FFmpegEncoderCatalog {
    by_id: HashMap<AVCodecID, Vec<FFmpegEncoderInfo>>,
    by_name: HashMap<String, FFmpegEncoderInfo>,
}

static GLOBAL_CATALOG: OnceLock<FFmpegEncoderCatalog> = OnceLock::new();

impl FFmpegEncoderCatalog {
    pub fn global() -> &'static FFmpegEncoderCatalog {
        GLOBAL_CATALOG.get_or_init(Self::init)
    }

    pub fn init() -> Self {
        let mut by_id: HashMap<AVCodecID, Vec<FFmpegEncoderInfo>> = HashMap::new();
        let mut by_name = HashMap::new();

        unsafe {
            let mut iter: *mut std::ffi::c_void = std::ptr::null_mut();
            loop {
                let codec: *const AVCodec = av_codec_iterate(&mut iter);
                if codec.is_null() {
                    break;
                }
                let is_encoder =
                    crate::media::ffmpeg_sys_fframes::av_codec_is_encoder(codec) != 0;
                if !is_encoder {
                    continue;
                }

                let name = match CStr::from_ptr((*codec).name).to_str() {
                    Ok(s) => s.to_string(),
                    Err(_) => continue,
                };
                let info = FFmpegEncoderInfo {
                    name: name.clone(),
                    supports_10bit: codec_supports_10bit(codec),
                    supports_alpha: codec_supports_alpha(codec),
                    hardware_accelerated: is_hardware_accelerated(&name),
                };
                by_id.entry((*codec).id).or_default().push(info.clone());
                by_name.insert(name, info);
            }
        }

        FFmpegEncoderCatalog { by_id, by_name }
    }

    pub fn is_available(&self, codec_id: AVCodecID) -> bool {
        self.by_id.contains_key(&codec_id)
            && unsafe { !avcodec_find_encoder(codec_id).is_null() }
    }

    pub fn is_encoder_available(&self, encoder_name: &str) -> bool {
        self.by_name.contains_key(encoder_name)
    }

    pub fn capabilities_for_encoder(&self, encoder_name: &str) -> Option<&FFmpegEncoderInfo> {
        self.by_name.get(encoder_name)
    }

    pub fn capabilities_for_codec(&self, codec_id: AVCodecID) -> Option<&[FFmpegEncoderInfo]> {
        self.by_id.get(&codec_id).map(|v| v.as_slice())
    }

    pub fn suggest_alternative(&self, codec_id: AVCodecID) -> Option<String> {
        match codec_id {
            AVCodecID::AV_CODEC_ID_PRORES => {
                if self.is_encoder_available("prores_ks") {
                    Some("prores_ks".to_string())
                } else if self.is_encoder_available("prores") {
                    Some("prores".to_string())
                } else {
                    None
                }
            }
            AVCodecID::AV_CODEC_ID_DNXHD => {
                if self.is_encoder_available("dnxhd") {
                    Some("dnxhd".to_string())
                } else {
                    None
                }
            }
            AVCodecID::AV_CODEC_ID_H264 => {
                for name in ["libx264", "h264_videotoolbox", "h264_nvenc", "h264_vaapi"] {
                    if self.is_encoder_available(name) {
                        return Some(name.to_string());
                    }
                }
                None
            }
            AVCodecID::AV_CODEC_ID_HEVC => {
                for name in ["libx265", "hevc_videotoolbox", "hevc_nvenc", "hevc_vaapi"] {
                    if self.is_encoder_available(name) {
                        return Some(name.to_string());
                    }
                }
                None
            }
            _ => None,
        }
    }
}

fn codec_supports_10bit(codec: *const AVCodec) -> bool {
    use crate::media::ffmpeg_sys_fframes::AVPixelFormat;
    unsafe {
        if (*codec).pix_fmts.is_null() {
            return false;
        }
        let mut index = 0_isize;
        loop {
            let fmt = *(*codec).pix_fmts.offset(index);
            if fmt == AVPixelFormat::AV_PIX_FMT_NONE {
                return false;
            }
            if matches!(
                fmt,
                AVPixelFormat::AV_PIX_FMT_YUV420P10LE
                    | AVPixelFormat::AV_PIX_FMT_YUV422P10LE
                    | AVPixelFormat::AV_PIX_FMT_YUV444P10LE
                    | AVPixelFormat::AV_PIX_FMT_YUVA444P10LE
            ) {
                return true;
            }
            index += 1;
        }
    }
}

fn codec_supports_alpha(codec: *const AVCodec) -> bool {
    use crate::media::ffmpeg_sys_fframes::AVPixelFormat;
    use crate::pixel_format::PixelFormat;
    unsafe {
        if (*codec).pix_fmts.is_null() {
            return false;
        }
        let mut index = 0_isize;
        loop {
            let fmt = *(*codec).pix_fmts.offset(index);
            if fmt == AVPixelFormat::AV_PIX_FMT_NONE {
                return false;
            }
            if PixelFormat::av_pixel_format_has_alpha(fmt) {
                return true;
            }
            index += 1;
        }
    }
}

fn is_hardware_accelerated(name: &str) -> bool {
    name.contains("videotoolbox")
        || name.contains("nvenc")
        || name.contains("qsv")
        || name.contains("vaapi")
        || name.contains("mediacodec")
        || name.contains("vulkan")
}
