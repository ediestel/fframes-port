use crate::ffi_safety::{SafeAvCodecContext, SafeAvFormatContext, SafeSwsContext};
use crate::error::FFramesMediaError;
use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;

use ffmpeg_sys_next as sys;

#[derive(Debug, Clone, Copy)]
pub struct ResizeVideoFrame {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct FrameConvertOptions {
    pub resize: Option<ResizeVideoFrame>,
}

// Fix 15: removed dead sws_ctx field (always null, never stored, never freed)
// Fix 1: added time_base_num/den fields for correct timestamp_seconds() math
pub struct FFmpegFrameBuf {
    frame: *mut sys::AVFrame,
    stream_width: u32,
    stream_height: u32,
    stream_fps: f32,
    stream_duration_seconds: f32,
    output_width: u32,
    output_height: u32,
    time_base_num: i32,
    time_base_den: i32,
    frame_id: String,
}

// SAFETY: FFmpegFrameBuf contains a raw *mut AVFrame.  We guarantee unique
// ownership through Arc<FFmpegFrameBuf> and never expose the raw pointer
// without going through the documented unsafe methods.  FFmpeg's allocation
// and deallocation functions are thread-safe on distinct objects.
unsafe impl Send for FFmpegFrameBuf {}
// SAFETY: All access to the inner frame pointer goes through unsafe methods
// that require the caller to uphold aliasing invariants; no interior mutability
// is provided through shared references.
unsafe impl Sync for FFmpegFrameBuf {}

impl FFmpegFrameBuf {
    pub fn get_width(&self) -> u32 {
        self.output_width
    }

    pub fn get_height(&self) -> u32 {
        self.output_height
    }

    pub fn get_stream_width(&self) -> u32 {
        self.stream_width
    }

    pub fn get_stream_height(&self) -> u32 {
        self.stream_height
    }

    pub fn get_stream_fps(&self) -> f32 {
        self.stream_fps
    }

    /// # Safety
    /// Caller must ensure frame pointer is valid.
    // Fix 1: correct formula — pts is in stream timebase units, not frame units
    pub unsafe fn timestamp_seconds(&self) -> f32 {
        let pts = (*self.frame).pts;
        if pts == sys::AV_NOPTS_VALUE || self.time_base_den == 0 {
            return 0.0;
        }
        pts as f32 * self.time_base_num as f32 / self.time_base_den as f32
    }

    /// # Safety
    /// Caller must ensure frame pointer is valid.
    pub unsafe fn get_pts(&self) -> i64 {
        (*self.frame).pts
    }

    /// # Safety
    /// Caller must ensure frame pointer is valid.
    // Fix 7: renamed from get_stream_duration_in_frames — actually returns seconds
    pub unsafe fn get_stream_duration_in_seconds(&self) -> f32 {
        self.stream_duration_seconds
    }

    /// # Safety
    /// Caller must ensure frame pointer is valid and output dimensions are set.
    // Fix 5: RGBA data pre-blended via PreloadedImageData::new()
    // Fix 8: SWS_BILINEAR via SwsFlags enum instead of magic number 2
    pub unsafe fn convert_last_decoded_frame_into_svg_image(
        &self,
        options: Option<&FrameConvertOptions>,
    ) -> Result<Arc<usvgr::PreloadedImageData>, FFramesMediaError> {
        let (out_w, out_h) = match options.and_then(|o| o.resize.as_ref()) {
            Some(resize) => (resize.width, resize.height),
            None => (self.output_width, self.output_height),
        };

        let dst_format = sys::AVPixelFormat::AV_PIX_FMT_RGBA;
        let raw_format = (*self.frame).format;
        if raw_format < sys::AVPixelFormat::AV_PIX_FMT_NONE as i32 {
            return Err(FFramesMediaError::DecodingError(format!(
                "invalid AVPixelFormat value from FFmpeg: {raw_format}"
            )));
        }
        let src_format: sys::AVPixelFormat = std::mem::transmute(raw_format);
        // SAFETY: sws_getContext returns a non-null pointer on success; we wrap it
        // in SafeSwsContext which calls sws_freeContext on drop, ensuring the
        // context is freed even if the function returns early via `?`.
        let raw_sws = sys::sws_getContext(
            self.stream_width as i32,
            self.stream_height as i32,
            src_format,
            out_w as i32,
            out_h as i32,
            dst_format,
            sys::SwsFlags::SWS_BILINEAR as i32,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
        );
        // SAFETY: raw_sws is either null (handled below) or a valid pointer
        // returned by sws_getContext; SafeSwsContext takes ownership.
        let sws = SafeSwsContext::from_raw(raw_sws).ok_or_else(|| {
            FFramesMediaError::DecodingError("sws_getContext failed".into())
        })?;

        let buf_size = (out_w * out_h * 4) as usize;
        let mut rgba_buf: Vec<u8> = vec![0u8; buf_size];

        let dst_data: [*mut u8; 4] = [
            rgba_buf.as_mut_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ];
        let dst_linesize: [i32; 4] = [(out_w * 4) as i32, 0, 0, 0];

        // SAFETY: sws.as_ptr() is a valid non-null SwsContext. self.frame is
        // valid for the lifetime of this call (checked by callers). dst_data
        // and dst_linesize are properly sized for the output dimensions.
        sys::sws_scale(
            sws.as_ptr(),
            (*self.frame).data.as_ptr() as *const *const u8,
            (*self.frame).linesize.as_ptr(),
            0,
            self.stream_height as i32,
            dst_data.as_ptr() as *const *mut u8,
            dst_linesize.as_ptr(),
        );
        // `sws` is dropped here, which calls sws_freeContext automatically.

        // Fix 5: use PreloadedImageData::new() which pre-blends semi-transparent pixels
        let preloaded = Arc::new(usvgr::PreloadedImageData::new(
            self.frame_id.clone(),
            out_w,
            out_h,
            &rgba_buf,
        ));

        Ok(preloaded)
    }
}

impl Drop for FFmpegFrameBuf {
    fn drop(&mut self) {
        // SAFETY: self.frame is either null (checked) or a valid AVFrame
        // allocated by av_frame_alloc inside decode_up_to. We are the sole
        // owner of this pointer (wrapped in Arc<FFmpegFrameBuf>), so no
        // aliasing occurs. av_frame_free sets the pointer to null.
        unsafe {
            if !self.frame.is_null() {
                sys::av_frame_free(&mut self.frame);
            }
        }
    }
}

pub struct FFmpegDecoder {
    // fmt_ctx is declared first; Rust drops fields in reverse declaration order,
    // so codec_ctx (below) is dropped first — matching FFmpeg's required teardown
    // order (free codec context before closing format context).
    fmt_ctx: SafeAvFormatContext,
    codec_ctx: SafeAvCodecContext,
    stream_index: i32,
    fps: f32,
    stream_duration_seconds: f32,
    width: u32,
    height: u32,
    time_base_num: i32,
    time_base_den: i32,
    buffer: VecDeque<Arc<FFmpegFrameBuf>>,
    buffer_size: usize,
    pub current_loop: usize,
}

// SAFETY: SafeAvFormatContext and SafeAvCodecContext are Send + Sync;
// FFmpegFrameBuf is Send + Sync via its own unsafe impls.
unsafe impl Send for FFmpegDecoder {}
unsafe impl Sync for FFmpegDecoder {}

impl FFmpegDecoder {
    /// # Safety
    /// Opens the video file using raw FFmpeg APIs.
    pub unsafe fn new(
        path: &Path,
        video_fps: usize,
        buffer_size: usize,
    ) -> Result<Self, FFramesMediaError> {
        let path_str = path
            .to_str()
            .ok_or_else(|| FFramesMediaError::IoError("invalid path".into()))?;
        let c_path = std::ffi::CString::new(path_str)
            .map_err(|e| FFramesMediaError::IoError(e.to_string()))?;

        // Open the format context and immediately wrap it in RAII.
        // If any subsequent step fails, `fmt_ctx` is dropped automatically and
        // avformat_close_input is called — no manual cleanup needed.
        let mut raw_fmt: *mut sys::AVFormatContext = std::ptr::null_mut();
        let ret = sys::avformat_open_input(
            &mut raw_fmt,
            c_path.as_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        if ret < 0 {
            return Err(FFramesMediaError::FFmpegError(ret, "avformat_open_input failed".into()));
        }
        // SAFETY: ret >= 0 guarantees raw_fmt is non-null.
        // SAFETY: ret >= 0 guarantees raw_fmt is non-null per the FFmpeg API.
        let fmt_ctx = SafeAvFormatContext::from_raw(raw_fmt)
            .ok_or_else(|| FFramesMediaError::FFmpegError(-1, "avformat_open_input returned null despite ret >= 0".into()))?;

        let ret = sys::avformat_find_stream_info(fmt_ctx.as_ptr(), std::ptr::null_mut());
        if ret < 0 {
            // fmt_ctx drops here → avformat_close_input called automatically.
            return Err(FFramesMediaError::FFmpegError(ret, "avformat_find_stream_info failed".into()));
        }

        // Find best video stream
        let nb_streams = (*fmt_ctx.as_ptr()).nb_streams as usize;
        let mut stream_index: i32 = -1;
        for i in 0..nb_streams {
            let stream = *(*fmt_ctx.as_ptr()).streams.add(i);
            let params = (*stream).codecpar;
            if (*params).codec_type == sys::AVMediaType::AVMEDIA_TYPE_VIDEO {
                stream_index = i as i32;
                break;
            }
        }

        if stream_index < 0 {
            return Err(FFramesMediaError::DecodingError("no video stream found".into()));
        }

        let stream = *(*fmt_ctx.as_ptr()).streams.add(stream_index as usize);
        let params = (*stream).codecpar;
        let width = (*params).width as u32;
        let height = (*params).height as u32;

        let r_frame_rate = (*stream).r_frame_rate;
        let avg_frame_rate = (*stream).avg_frame_rate;
        let fps = if r_frame_rate.den != 0 {
            r_frame_rate.num as f32 / r_frame_rate.den as f32
        } else {
            video_fps as f32
        };

        // VFR detection: when avg_frame_rate disagrees with r_frame_rate by
        // more than ~0.5%, the source is likely variable-frame-rate. Surface
        // this so deterministic CFR pipelines can reject the input upstream.
        if r_frame_rate.den != 0 && avg_frame_rate.den != 0 {
            let r = r_frame_rate.num as f64 / r_frame_rate.den as f64;
            let a = avg_frame_rate.num as f64 / avg_frame_rate.den as f64;
            if a > 0.0 && (r - a).abs() / a > 0.005 {
                eprintln!(
                    "fframes-media: VFR-suspect input — r_frame_rate={r:.4} avg_frame_rate={a:.4}; \
                     enable EncoderOptions.enforce_cfr to reject upstream."
                );
            }
        }

        let time_base = (*stream).time_base;
        let time_base_num = time_base.num;
        let time_base_den = time_base.den;

        let duration_ts = (*fmt_ctx.as_ptr()).duration;
        let stream_duration_seconds = if duration_ts != sys::AV_NOPTS_VALUE {
            duration_ts as f32 / 1_000_000.0 // AV_TIME_BASE = 1_000_000
        } else {
            0.0
        };

        let codec_id = (*params).codec_id;
        let codec = sys::avcodec_find_decoder(codec_id);
        if codec.is_null() {
            return Err(FFramesMediaError::DecodingError("no decoder found".into()));
        }

        // Wrap codec context in RAII immediately after allocation.
        // If subsequent steps fail, avcodec_free_context is called on drop.
        let raw_codec = sys::avcodec_alloc_context3(codec);
        // SAFETY: raw_codec is either null (error) or a valid allocated context.
        let codec_ctx = SafeAvCodecContext::from_raw(raw_codec)
            .ok_or_else(|| FFramesMediaError::DecodingError("avcodec_alloc_context3 failed".into()))?;

        let ret = sys::avcodec_parameters_to_context(codec_ctx.as_ptr(), params);
        if ret < 0 {
            // codec_ctx and fmt_ctx both drop here → cleanup is automatic.
            return Err(FFramesMediaError::FFmpegError(ret, "avcodec_parameters_to_context failed".into()));
        }

        let ret = sys::avcodec_open2(codec_ctx.as_ptr(), codec, std::ptr::null_mut());
        if ret < 0 {
            return Err(FFramesMediaError::FFmpegError(ret, "avcodec_open2 failed".into()));
        }

        Ok(Self {
            fmt_ctx,
            codec_ctx,
            stream_index,
            fps,
            stream_duration_seconds,
            width,
            height,
            time_base_num,
            time_base_den,
            buffer: VecDeque::with_capacity(buffer_size),
            buffer_size,
            current_loop: 0,
        })
    }

    /// # Safety
    /// Seeks the decoder to a frame offset.
    pub unsafe fn seek_to_offset(&mut self, offset: i64) -> Result<(), FFramesMediaError> {
        // Fix 3: use f64 to avoid fps truncation for fractional rates like 29.97
        let pts = self.offset_to_pts(offset);
        let ret = sys::av_seek_frame(
            self.fmt_ctx.as_ptr(),
            self.stream_index,
            pts,
            sys::AVSEEK_FLAG_BACKWARD,
        );
        if ret < 0 {
            return Err(FFramesMediaError::FFmpegError(ret, "seek failed".into()));
        }

        sys::avcodec_flush_buffers(self.codec_ctx.as_ptr());
        self.buffer.clear();
        Ok(())
    }

    /// # Safety
    /// Adjusts offset for looping based on stream duration.
    pub unsafe fn adjust_offset_for_looping(&self, offset: i64) -> Result<i64, FFramesMediaError> {
        let total_frames = (self.stream_duration_seconds * self.fps) as i64;
        if total_frames == 0 {
            return Ok(offset);
        }
        Ok(offset % total_frames)
    }

    /// # Safety
    /// Decodes frames up to the given frame offset. Returns true if a frame is available.
    pub unsafe fn decode_up_to(&mut self, offset: i64) -> Result<bool, FFramesMediaError> {
        let target_pts = self.offset_to_pts(offset);

        // Fix 2: check buffer.back() (most recent frame), not buffer.front() (oldest)
        if let Some(frame) = self.buffer.back() {
            if frame.get_pts() >= target_pts {
                return Ok(true);
            }
        }

        // Fix 6: use av_packet_alloc() instead of zeroed stack struct
        let packet = sys::av_packet_alloc();
        if packet.is_null() {
            return Err(FFramesMediaError::DecodingError("av_packet_alloc failed".into()));
        }

        let mut found = false;

        'outer: loop {
            let ret = sys::av_read_frame(self.fmt_ctx.as_ptr(), packet);
            if ret < 0 {
                break;
            }

            if (*packet).stream_index != self.stream_index {
                sys::av_packet_unref(packet);
                continue;
            }

            let ret = sys::avcodec_send_packet(self.codec_ctx.as_ptr(), packet);
            sys::av_packet_unref(packet);
            if ret < 0 {
                continue;
            }

            loop {
                let avframe = sys::av_frame_alloc();
                if avframe.is_null() {
                    break;
                }

                let ret = sys::avcodec_receive_frame(self.codec_ctx.as_ptr(), avframe);
                if ret < 0 {
                    sys::av_frame_free(&mut (avframe as *mut sys::AVFrame));
                    break;
                }

                let frame_pts = (*avframe).pts;
                let frame_id = format!("frame_{}", frame_pts);

                let frame_buf = Arc::new(FFmpegFrameBuf {
                    frame: avframe,
                    stream_width: self.width,
                    stream_height: self.height,
                    stream_fps: self.fps,
                    stream_duration_seconds: self.stream_duration_seconds,
                    output_width: self.width,
                    output_height: self.height,
                    // Fix 1: populate timebase so timestamp_seconds() can compute correctly
                    time_base_num: self.time_base_num,
                    time_base_den: self.time_base_den,
                    frame_id,
                });

                if self.buffer.len() >= self.buffer_size {
                    self.buffer.pop_front();
                }
                self.buffer.push_back(frame_buf);

                if frame_pts >= target_pts {
                    found = true;
                    break 'outer;
                }
            }
        }

        sys::av_packet_free(&mut (packet as *mut sys::AVPacket));

        Ok(found || !self.buffer.is_empty())
    }

    /// # Safety
    /// Returns the most recently decoded frame from the buffer.
    pub unsafe fn get_raw_frame(&self) -> Arc<FFmpegFrameBuf> {
        // SAFETY: callers must only call this after decode_up_to returned true; buffer is non-empty.
        #[allow(clippy::expect_used)] // Precondition of unsafe fn — caller guarantees buffer is non-empty
        self.buffer.back().cloned().expect("get_raw_frame called with empty buffer — call decode_up_to first")
    }

    pub fn get_decoded_image_in_buf(
        &self,
        pts: i64,
    ) -> Option<Arc<usvgr::PreloadedImageData>> {
        self.buffer
            .iter()
            // SAFETY: get_pts reads the pts field of the AVFrame which is valid
            // as long as the FFmpegFrameBuf is alive (guaranteed by the Arc).
            .find(|f| unsafe { f.get_pts() } == pts)
            // SAFETY: convert_last_decoded_frame_into_svg_image requires the
            // frame pointer to be valid and output dimensions to be set – both
            // are guaranteed by the FFmpegFrameBuf invariants.
            .and_then(|f| unsafe { f.convert_last_decoded_frame_into_svg_image(None) }.ok())
    }

    // Fix 3: use f64 throughout to preserve fractional fps (e.g. 29.97)
    fn offset_to_pts(&self, offset: i64) -> i64 {
        if self.time_base_den == 0 || self.fps <= 0.0 {
            return offset;
        }
        (offset as f64 / self.fps as f64
            * self.time_base_den as f64
            / self.time_base_num as f64) as i64
    }
}

// Drop is not implemented manually: SafeAvCodecContext and SafeAvFormatContext
// each call the correct FFmpeg teardown function in their own Drop impls.
// Rust drops fields in declaration order (reversed), so codec_ctx is freed
// before fmt_ctx, matching FFmpeg's required teardown sequence.
