/// RAII wrappers for FFmpeg raw pointers.
///
/// Each wrapper owns the underlying FFmpeg resource and frees it on drop.
/// All wrappers implement `Send` and `Sync` because FFmpeg's allocation and
/// deallocation functions are thread-safe when called on distinct objects.
use ffmpeg_sys_next as sys;

// ---------------------------------------------------------------------------
// SafeAvFrame
// ---------------------------------------------------------------------------

/// Owning RAII wrapper around `*mut sys::AVFrame`.
///
/// # Safety invariants
/// - `ptr` is non-null and was obtained from `av_frame_alloc()`.
/// - No other code aliases this pointer in a way that would outlive `SafeAvFrame`.
/// - `av_frame_free` is called exactly once in `Drop`.
pub struct SafeAvFrame {
    ptr: *mut sys::AVFrame,
}

impl SafeAvFrame {
    /// Allocate a new `AVFrame`.
    ///
    /// Returns `None` if `av_frame_alloc()` returns null (out of memory).
    pub fn new() -> Option<Self> {
        // SAFETY: av_frame_alloc is always safe to call; returns null on OOM.
        let ptr = unsafe { sys::av_frame_alloc() };
        if ptr.is_null() {
            None
        } else {
            Some(Self { ptr })
        }
    }

    /// Returns the raw pointer.
    ///
    /// # Safety
    /// The caller must not let the returned pointer outlive `self`,
    /// and must not call `av_frame_free` on it.
    pub unsafe fn as_ptr(&self) -> *mut sys::AVFrame {
        self.ptr
    }

    /// Provides shared access to the inner `AVFrame`.
    ///
    /// # Safety
    /// The caller must not use the reference after `self` is dropped.
    pub unsafe fn as_ref(&self) -> &sys::AVFrame {
        // SAFETY: ptr is non-null and valid for the lifetime of self.
        &*self.ptr
    }

    /// Provides mutable access to the inner `AVFrame`.
    ///
    /// # Safety
    /// The caller must not alias the mutable reference and must not use it
    /// after `self` is dropped.
    pub unsafe fn as_mut(&mut self) -> &mut sys::AVFrame {
        // SAFETY: ptr is non-null and valid for the lifetime of self.
        &mut *self.ptr
    }
}

impl Drop for SafeAvFrame {
    fn drop(&mut self) {
        // SAFETY: ptr is non-null (checked in `new`) and uniquely owned by this
        // struct.  av_frame_free sets the pointer to null and frees the frame
        // and all buffers it references.
        unsafe {
            sys::av_frame_free(&mut self.ptr);
        }
    }
}

// SAFETY: AVFrame is heap-allocated and not inherently thread-affine; we
// guarantee unique ownership through the Rust type system.
unsafe impl Send for SafeAvFrame {}
unsafe impl Sync for SafeAvFrame {}

// ---------------------------------------------------------------------------
// SafeSwsContext
// ---------------------------------------------------------------------------

/// Owning RAII wrapper around `*mut sys::SwsContext`.
///
/// # Safety invariants
/// - `ptr` is non-null and was obtained from `sws_getContext()`.
/// - `sws_freeContext` is called exactly once in `Drop`.
pub struct SafeSwsContext {
    ptr: *mut sys::SwsContext,
}

impl SafeSwsContext {
    /// Wraps an existing non-null `SwsContext` pointer.
    ///
    /// # Safety
    /// `ptr` must have been returned by `sws_getContext()` and must not be
    /// null.  The caller transfers ownership to `SafeSwsContext`.
    pub unsafe fn from_raw(ptr: *mut sys::SwsContext) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            Some(Self { ptr })
        }
    }

    /// Returns the raw pointer.
    ///
    /// # Safety
    /// The caller must not let the returned pointer outlive `self`,
    /// and must not call `sws_freeContext` on it.
    pub unsafe fn as_ptr(&self) -> *mut sys::SwsContext {
        self.ptr
    }
}

impl Drop for SafeSwsContext {
    fn drop(&mut self) {
        // SAFETY: ptr is non-null (guaranteed by `from_raw`) and uniquely
        // owned by this struct.  sws_freeContext does not set the pointer to
        // null, but we never use it after this point.
        unsafe {
            sys::sws_freeContext(self.ptr);
        }
    }
}

// SAFETY: SwsContext is heap-allocated; unique ownership is enforced by Rust.
unsafe impl Send for SafeSwsContext {}
unsafe impl Sync for SafeSwsContext {}

// ---------------------------------------------------------------------------
// SafeAvFormatContext
// ---------------------------------------------------------------------------

/// Owning RAII wrapper around `*mut sys::AVFormatContext`.
///
/// # Safety invariants
/// - `ptr` is non-null and was opened via `avformat_open_input()`.
/// - `avformat_close_input` is called exactly once in `Drop`.
pub struct SafeAvFormatContext {
    ptr: *mut sys::AVFormatContext,
}

impl SafeAvFormatContext {
    /// Wraps an existing non-null `AVFormatContext` pointer.
    ///
    /// # Safety
    /// `ptr` must have been successfully opened with `avformat_open_input()`
    /// and must not be null.  The caller transfers ownership to
    /// `SafeAvFormatContext`.
    pub unsafe fn from_raw(ptr: *mut sys::AVFormatContext) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            Some(Self { ptr })
        }
    }

    /// Returns the raw pointer.
    ///
    /// # Safety
    /// The caller must not let the returned pointer outlive `self`,
    /// and must not call `avformat_close_input` on it.
    pub unsafe fn as_ptr(&self) -> *mut sys::AVFormatContext {
        self.ptr
    }

    /// Provides shared access to the inner `AVFormatContext`.
    ///
    /// # Safety
    /// The caller must not use the reference after `self` is dropped.
    pub unsafe fn as_ref(&self) -> &sys::AVFormatContext {
        // SAFETY: ptr is non-null and valid for the lifetime of self.
        &*self.ptr
    }
}

impl Drop for SafeAvFormatContext {
    fn drop(&mut self) {
        // SAFETY: ptr is non-null (guaranteed by `from_raw`) and was opened
        // with avformat_open_input, so avformat_close_input is the correct
        // deallocation function.  avformat_close_input sets the pointer to
        // null internally.
        unsafe {
            sys::avformat_close_input(&mut self.ptr);
        }
    }
}

// SAFETY: AVFormatContext is heap-allocated; unique ownership is enforced by
// the Rust type system.
unsafe impl Send for SafeAvFormatContext {}
unsafe impl Sync for SafeAvFormatContext {}

// ---------------------------------------------------------------------------
// SafeAvCodecContext
// ---------------------------------------------------------------------------

/// Owning RAII wrapper around `*mut sys::AVCodecContext`.
///
/// # Safety invariants
/// - `ptr` is non-null and was obtained from `avcodec_alloc_context3()`.
/// - `avcodec_free_context` is called exactly once in `Drop`.
pub struct SafeAvCodecContext {
    ptr: *mut sys::AVCodecContext,
}

impl SafeAvCodecContext {
    /// Wraps an existing non-null `AVCodecContext` pointer.
    ///
    /// # Safety
    /// `ptr` must have been returned by `avcodec_alloc_context3()` and must not
    /// be null.  The caller transfers ownership to `SafeAvCodecContext`.
    pub unsafe fn from_raw(ptr: *mut sys::AVCodecContext) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            Some(Self { ptr })
        }
    }

    /// Returns the raw pointer.
    ///
    /// # Safety
    /// The caller must not let the returned pointer outlive `self`,
    /// and must not call `avcodec_free_context` on it.
    pub unsafe fn as_ptr(&self) -> *mut sys::AVCodecContext {
        self.ptr
    }

    /// Provides mutable access to the inner `AVCodecContext`.
    ///
    /// # Safety
    /// The caller must not alias the mutable reference and must not use it
    /// after `self` is dropped.
    pub unsafe fn as_mut(&mut self) -> &mut sys::AVCodecContext {
        // SAFETY: ptr is non-null and valid for the lifetime of self.
        &mut *self.ptr
    }
}

impl Drop for SafeAvCodecContext {
    fn drop(&mut self) {
        // SAFETY: ptr is non-null (guaranteed by `from_raw`) and was allocated
        // with avcodec_alloc_context3.  avcodec_free_context is the matching
        // release function and sets the pointer to null internally.
        unsafe {
            sys::avcodec_free_context(&mut self.ptr);
        }
    }
}

// SAFETY: AVCodecContext is heap-allocated; unique ownership is enforced by
// the Rust type system.
unsafe impl Send for SafeAvCodecContext {}
unsafe impl Sync for SafeAvCodecContext {}
