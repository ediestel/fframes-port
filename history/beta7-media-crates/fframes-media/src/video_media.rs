use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct VideoMedia {
    pub path: PathBuf,
    pub metadata: Option<GeneralVideoFileMetadata>,
}

#[derive(Debug, Clone, Copy)]
pub struct GeneralVideoFileMetadata {
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub duration: f32,
}

impl VideoMedia {
    /// Fix 11: probe video file with FFmpeg to populate GeneralVideoFileMetadata.
    /// Falls back to metadata: None on any error.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let metadata = Self::probe_metadata(&path);
        Self { path, metadata }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            metadata: None,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn probe_metadata(path: &Path) -> Option<GeneralVideoFileMetadata> {
        use ffmpeg_sys_next as sys;

        let path_str = path.to_str()?;
        let c_path = std::ffi::CString::new(path_str).ok()?;

        // SAFETY:
        // - c_path is a valid null-terminated C string derived from a Rust Path.
        // - avformat_open_input is called with a null format and options, which
        //   causes FFmpeg to probe the format automatically.
        // - fmt_ctx starts as null; avformat_open_input sets it on success or
        //   leaves it null on failure.
        // - avformat_close_input must be called on the returned context to free
        //   it; we call it in all success and error paths below.
        // - Pointer arithmetic via .add(i) is safe because nb_streams bounds
        //   the loop and the streams array has exactly nb_streams elements.
        // - All dereferenced pointers (fmt_ctx, stream, params) are guaranteed
        //   non-null by FFmpeg after successful open + find_stream_info.
        unsafe {
            let mut fmt_ctx: *mut sys::AVFormatContext = std::ptr::null_mut();
            let ret = sys::avformat_open_input(
                &mut fmt_ctx,
                c_path.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            if ret < 0 {
                return None;
            }

            let ret = sys::avformat_find_stream_info(fmt_ctx, std::ptr::null_mut());
            if ret < 0 {
                sys::avformat_close_input(&mut fmt_ctx);
                return None;
            }

            let nb_streams = (*fmt_ctx).nb_streams as usize; // nb_streams is u32, cast to usize is safe on 32-bit and wider targets
            let mut result = None;

            for i in 0..nb_streams {
                let stream = *(*fmt_ctx).streams.add(i);
                let params = (*stream).codecpar;
                if (*params).codec_type == sys::AVMediaType::AVMEDIA_TYPE_VIDEO {
                    let width = (*params).width as u32;
                    let height = (*params).height as u32;

                    let r = (*stream).r_frame_rate;
                    let fps = if r.den != 0 {
                        r.num as f32 / r.den as f32
                    } else {
                        0.0
                    };

                    const MICROSECONDS_PER_SECOND: f32 = 1_000_000.0;
                    let duration = if (*fmt_ctx).duration != sys::AV_NOPTS_VALUE {
                        (*fmt_ctx).duration as f32 / MICROSECONDS_PER_SECOND
                    } else {
                        0.0
                    };

                    result = Some(GeneralVideoFileMetadata { width, height, fps, duration });
                    break;
                }
            }

            sys::avformat_close_input(&mut fmt_ctx);
            result
        }
    }
}
