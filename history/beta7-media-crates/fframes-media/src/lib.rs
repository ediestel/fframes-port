#![warn(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// TODO: graduate to #![deny(...)] once all remaining occurrences carry
// explicit #[allow] with justification

mod error;
mod audio;
mod image_data;
mod subtitles;
mod video_media;

#[cfg(not(target_arch = "wasm32"))]
mod video_decoder;
#[cfg(not(target_arch = "wasm32"))]
pub mod ffi_safety;

pub extern crate bytemuck;

#[cfg(not(target_arch = "wasm32"))]
pub extern crate ffmpeg_sys_next as ffmpeg_sys_fframes;

#[cfg(feature = "exif")]
pub extern crate kamadak_exif as exif;

pub use error::FFramesMediaError;
pub use audio::PreloadedAudioData;
pub use image_data::{ImageData, ImageMetadata, RawMediaFile};
pub use subtitles::{Subtitles, FFramesSubtitles, FFramesSubtitlesCue};
pub use video_media::{VideoMedia, GeneralVideoFileMetadata};

#[cfg(not(target_arch = "wasm32"))]
pub use video_decoder::{FFmpegDecoder, FFmpegFrameBuf, FrameConvertOptions, ResizeVideoFrame};
