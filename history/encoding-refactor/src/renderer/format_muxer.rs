//! FormatMuxer — owns the AVFormatContext lifecycle (file I/O, header/trailer,
//! packet interleaving). Extracted from the legacy monolithic Encoder.

use super::renderer_error::{RenderEncodingError, RenderEncodingResult};
use crate::ffmpeg_action;
use crate::media::ffmpeg_sys_fframes::*;
use std::ffi::CString;
use std::path::Path;

pub struct FormatMuxer {
    pub(crate) oc: *mut AVFormatContext,
    header_written: bool,
    trailer_written: bool,
}

unsafe impl Send for FormatMuxer {}
unsafe impl Sync for FormatMuxer {}

impl FormatMuxer {
    pub unsafe fn open(filename: &Path) -> RenderEncodingResult<Self> {
        unsafe {
            let c_filename = CString::new(filename.to_string_lossy().as_ref())
                .map_err(RenderEncodingError::CStringError)?;
            let mut oc: *mut AVFormatContext = std::ptr::null_mut();
            ffmpeg_action!(
                avformat_alloc_output_context2(
                    &mut oc,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    c_filename.as_ptr(),
                ),
                RenderEncodingError::UnknownExtension(filename.to_owned())
            );
            ffmpeg_action!(
                avio_open(&mut (*oc).pb, c_filename.as_ptr(), 2),
                RenderEncodingError::CantOpenFile(filename.to_owned())
            );
            Ok(Self {
                oc,
                header_written: false,
                trailer_written: false,
            })
        }
    }

    pub fn as_ptr(&self) -> *mut AVFormatContext {
        self.oc
    }

    /// Mark the trailer as already written (used by paths that handed the
    /// muxer off to FFmpeg helpers which call `av_write_trailer` directly).
    pub fn mark_header_written(&mut self) {
        self.header_written = true;
    }

    pub unsafe fn write_header(&mut self) -> RenderEncodingResult<()> {
        unsafe {
            if !self.header_written {
                let r = avformat_write_header(self.oc, std::ptr::null_mut());
                if r < 0 {
                    return Err(RenderEncodingError::Internal(format!(
                        "avformat_write_header failed: {r}"
                    )));
                }
                self.header_written = true;
            }
            Ok(())
        }
    }

    pub unsafe fn set_dict(&mut self, key: &str, value: &str) -> RenderEncodingResult<()> {
        unsafe {
            let k = CString::new(key).map_err(RenderEncodingError::CStringError)?;
            let v = CString::new(value).map_err(RenderEncodingError::CStringError)?;
            av_dict_set(&mut (*self.oc).metadata, k.as_ptr(), v.as_ptr(), 0);
            Ok(())
        }
    }

    pub unsafe fn write_interleaved(
        &self,
        packet: *mut AVPacket,
    ) -> RenderEncodingResult<()> {
        unsafe {
            let r = av_interleaved_write_frame(self.oc, packet);
            if r < 0 {
                return Err(RenderEncodingError::CantWriteFrame(
                    super::encoder::av_error_to_string(r),
                ));
            }
            Ok(())
        }
    }

    pub unsafe fn finish(&mut self) -> RenderEncodingResult<()> {
        unsafe {
            if self.header_written && !self.trailer_written {
                av_write_trailer(self.oc);
                self.trailer_written = true;
            }
            Ok(())
        }
    }
}

impl Drop for FormatMuxer {
    fn drop(&mut self) {
        unsafe {
            if !self.oc.is_null() {
                if self.header_written && !self.trailer_written {
                    av_write_trailer(self.oc);
                }
                if !(*self.oc).metadata.is_null() {
                    av_dict_free(&mut (*self.oc).metadata);
                }
                if !(*self.oc).pb.is_null() {
                    avio_closep(&mut (*self.oc).pb);
                }
                avformat_free_context(self.oc);
                self.oc = std::ptr::null_mut();
            }
        }
    }
}
