//! Renders a video into one image file per frame (PNG, TIFF or OpenEXR) instead of a video file,
//! for compositing and VFX pipelines that take image sequences.
//!
//! Frames are split into contiguous parts, one per thread; each thread renders its part through
//! its own [`Previewer`] and the backend's [`FrameRenderer`], so the pixels are exactly those of
//! a preview over the video's `BACKGROUND_COLOR`. The video file path of [`super::render`] is not
//! involved.

use super::{
    FFramesRenderBackend, FFramesRendererError, FFramesRendererResult, Previewer, RenderOptions,
    RgbaFrame, get_thread_count,
};
use crate::Video;
use std::path::{Path, PathBuf};

/// The file format of an image sequence.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ImageSequenceFormat {
    /// 8 bit straight RGBA.
    Png,
    /// 8 bit straight RGBA.
    Tiff,
    /// 32 bit float RGBA in linear light with premultiplied alpha, the OpenEXR convention.
    OpenExr,
}

impl ImageSequenceFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Tiff => "tif",
            Self::OpenExr => "exr",
        }
    }
}

/// Where and how [`render_image_sequence`] writes the frames: `dir/base_name_000042.png`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageSequence {
    pub dir: PathBuf,
    pub format: ImageSequenceFormat,
    pub base_name: String,
    /// Digits of the frame number, padded with zeros.
    pub zero_pad: usize,
}

impl ImageSequence {
    pub fn new(dir: impl Into<PathBuf>, format: ImageSequenceFormat) -> Self {
        Self {
            dir: dir.into(),
            format,
            base_name: "frame".to_owned(),
            zero_pad: 6,
        }
    }

    pub fn png(dir: impl Into<PathBuf>) -> Self {
        Self::new(dir, ImageSequenceFormat::Png)
    }

    pub fn tiff(dir: impl Into<PathBuf>) -> Self {
        Self::new(dir, ImageSequenceFormat::Tiff)
    }

    pub fn exr(dir: impl Into<PathBuf>) -> Self {
        Self::new(dir, ImageSequenceFormat::OpenExr)
    }

    /// The file of video frame `frame`.
    pub fn frame_path(&self, frame: usize) -> PathBuf {
        let pad = self.zero_pad;
        self.dir.join(format!(
            "{}_{frame:0>pad$}.{}",
            self.base_name,
            self.format.extension()
        ))
    }

    /// Writes `pixels` (straight RGBA) to [`Self::frame_path`] of `frame`.
    pub fn write_frame(&self, frame: usize, pixels: &RgbaFrame) -> FFramesRendererResult<PathBuf> {
        let path = self.frame_path(frame);
        match self.format {
            ImageSequenceFormat::Png => write_rgba8(&path, pixels, image::ImageFormat::Png)?,
            ImageSequenceFormat::Tiff => write_rgba8(&path, pixels, image::ImageFormat::Tiff)?,
            ImageSequenceFormat::OpenExr => write_exr(&path, pixels)?,
        }
        Ok(path)
    }
}

/// Renders the frames of `options.frame_range` (all frames by default) into `sequence` and
/// returns the written files in frame order. The output size follows `scale_resolution`.
///
/// Fails when the backend can not render single frames ([`FFramesRenderBackend::frame_renderer`]
/// returns `None`).
pub fn render_image_sequence<'a, 'media: 'a, TBackend, TVideo>(
    sequence: &ImageSequence,
    video: &'a TVideo,
    backend: TBackend,
    options: &RenderOptions<'a, 'media>,
) -> FFramesRendererResult<Vec<PathBuf>>
where
    TBackend: FFramesRenderBackend + Sync,
    TVideo: Video + Sync,
{
    let duration = Previewer::new(video, options)?
        .timeline()
        .duration_in_frames;
    let frames = options.output_frame_range(duration);
    if frames.is_empty() {
        return Err(FFramesRendererError::Custom(format!(
            "frame range {:?} is empty or outside the video (0..{duration})",
            options.frame_range
        )));
    }
    std::fs::create_dir_all(&sequence.dir)?;

    let part_len = frames.len().div_ceil(get_thread_count().max(1));
    let parts: Vec<_> = frames
        .clone()
        .step_by(part_len)
        .map(|start| start..(start + part_len).min(frames.end))
        .collect();

    let written = std::thread::scope(|scope| {
        let workers: Vec<_> = parts
            .into_iter()
            .map(|part| {
                let backend = &backend;
                scope.spawn(move || -> FFramesRendererResult<Vec<PathBuf>> {
                    let mut renderer = backend.frame_renderer().ok_or_else(|| {
                        FFramesRendererError::Custom(
                            "this rendering backend can not render single frames".to_owned(),
                        )
                    })?;
                    let mut previewer = Previewer::new(video, options)?
                        .with_svgr_rasterizer(backend.svgr_rasterizer());
                    part.map(|frame| {
                        let pixels = previewer.render(frame, &mut renderer)?;
                        sequence.write_frame(frame, &pixels)
                    })
                    .collect()
                })
            })
            .collect();

        workers
            .into_iter()
            .map(|worker| {
                worker.join().map_err(|_| {
                    FFramesRendererError::Internal("image sequence thread panicked".to_owned())
                })?
            })
            .collect::<FFramesRendererResult<Vec<_>>>()
    })?;

    Ok(written.into_iter().flatten().collect())
}

fn write_rgba8(
    path: &Path,
    pixels: &RgbaFrame,
    format: image::ImageFormat,
) -> FFramesRendererResult<()> {
    image::save_buffer_with_format(
        path,
        &pixels.pixels,
        pixels.width,
        pixels.height,
        image::ExtendedColorType::Rgba8,
        format,
    )
    .map_err(|err| FFramesRendererError::Custom(format!("{}: {err}", path.display())))
}

fn write_exr(path: &Path, pixels: &RgbaFrame) -> FFramesRendererResult<()> {
    let linear: Vec<f32> = pixels
        .pixels
        .chunks_exact(4)
        .flat_map(|px| {
            let alpha = f32::from(px[3]) / 255.;
            let channel = |value: u8| srgb_to_linear(f32::from(value) / 255.) * alpha;
            [channel(px[0]), channel(px[1]), channel(px[2]), alpha]
        })
        .collect();
    let image = image::Rgba32FImage::from_raw(pixels.width, pixels.height, linear)
        .ok_or_else(|| FFramesRendererError::Internal("EXR buffer size mismatch".to_owned()))?;
    image
        .save_with_format(path, image::ImageFormat::OpenExr)
        .map_err(|err| FFramesRendererError::Custom(format!("{}: {err}", path.display())))
}

/// The sRGB transfer function, inverted.
fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}
