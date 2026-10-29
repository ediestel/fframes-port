use std::path::{Path, PathBuf};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ContainerFormat {
    Mp4,
    Mov,
    Mkv,
    Webm,
    WavAudio,
}

impl ContainerFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            ContainerFormat::Mp4 => "mp4",
            ContainerFormat::Mov => "mov",
            ContainerFormat::Mkv => "mkv",
            ContainerFormat::Webm => "webm",
            ContainerFormat::WavAudio => "wav",
        }
    }

    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .and_then(|ext| match ext.as_str() {
                "mp4" | "m4v" => Some(Self::Mp4),
                "mov" => Some(Self::Mov),
                "mkv" => Some(Self::Mkv),
                "webm" => Some(Self::Webm),
                "wav" => Some(Self::WavAudio),
                _ => None,
            })
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ImageSequenceFormat {
    Png,
    Tiff,
    OpenExr,
}

impl ImageSequenceFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            ImageSequenceFormat::Png => "png",
            ImageSequenceFormat::Tiff => "tif",
            ImageSequenceFormat::OpenExr => "exr",
        }
    }
}

/// Per-frame raw pixel callback. Boxed because the closure type is
/// erased; not `Clone` (the closure may carry !Clone state). When this
/// variant is selected the renderer skips both the muxer and the image
/// writer, handing the rendered RGBA8 buffer directly to the callback.
pub type FrameCallback = std::sync::Arc<
    parking_lot::Mutex<dyn FnMut(u64, u32, u32, &[u8]) -> Result<(), String> + Send + 'static>,
>;

#[derive(Clone)]
pub enum OutputTarget {
    Container {
        path: PathBuf,
        format: ContainerFormat,
    },
    ImageSequence {
        dir: PathBuf,
        format: ImageSequenceFormat,
        zero_pad: usize,
        base_name: String,
    },
    /// Per-frame callback receives `(frame_index, width, height, rgba8)`.
    /// Use this for inline pipelines (preview UIs, custom encoders) that
    /// want raw pixels without a container or file-system roundtrip.
    FrameCallback(FrameCallback),
}

impl std::fmt::Debug for OutputTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputTarget::Container { path, format } => f
                .debug_struct("Container")
                .field("path", path)
                .field("format", format)
                .finish(),
            OutputTarget::ImageSequence { dir, format, zero_pad, base_name } => f
                .debug_struct("ImageSequence")
                .field("dir", dir)
                .field("format", format)
                .field("zero_pad", zero_pad)
                .field("base_name", base_name)
                .finish(),
            OutputTarget::FrameCallback(_) => f.write_str("FrameCallback(<closure>)"),
        }
    }
}

impl OutputTarget {
    pub fn container_mp4(path: impl Into<PathBuf>) -> Self {
        OutputTarget::Container {
            path: path.into(),
            format: ContainerFormat::Mp4,
        }
    }

    pub fn container_from_path(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let format = ContainerFormat::from_path(&path).unwrap_or(ContainerFormat::Mp4);
        OutputTarget::Container { path, format }
    }

    pub fn png_sequence(dir: impl Into<PathBuf>) -> Self {
        OutputTarget::ImageSequence {
            dir: dir.into(),
            format: ImageSequenceFormat::Png,
            zero_pad: 6,
            base_name: "frame".to_string(),
        }
    }

    pub fn tiff_sequence(dir: impl Into<PathBuf>) -> Self {
        OutputTarget::ImageSequence {
            dir: dir.into(),
            format: ImageSequenceFormat::Tiff,
            zero_pad: 6,
            base_name: "frame".to_string(),
        }
    }

    pub fn exr_sequence(dir: impl Into<PathBuf>) -> Self {
        OutputTarget::ImageSequence {
            dir: dir.into(),
            format: ImageSequenceFormat::OpenExr,
            zero_pad: 6,
            base_name: "frame".to_string(),
        }
    }

    pub fn frame_filename(&self, frame_index: u64) -> Option<PathBuf> {
        match self {
            OutputTarget::ImageSequence {
                dir,
                format,
                zero_pad,
                base_name,
            } => {
                let pad = *zero_pad;
                let name = format!("{base_name}_{frame_index:0>pad$}.{ext}", ext = format.extension(), pad = pad);
                Some(dir.join(name))
            }
            OutputTarget::Container { .. } | OutputTarget::FrameCallback(_) => None,
        }
    }

    pub fn is_container(&self) -> bool {
        matches!(self, OutputTarget::Container { .. })
    }

    pub fn is_image_sequence(&self) -> bool {
        matches!(self, OutputTarget::ImageSequence { .. })
    }

    pub fn is_frame_callback(&self) -> bool {
        matches!(self, OutputTarget::FrameCallback(_))
    }

    pub fn frame_callback<F>(callback: F) -> Self
    where
        F: FnMut(u64, u32, u32, &[u8]) -> Result<(), String> + Send + 'static,
    {
        OutputTarget::FrameCallback(std::sync::Arc::new(parking_lot::Mutex::new(callback)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_detection() {
        assert_eq!(
            ContainerFormat::from_path(Path::new("out.mp4")),
            Some(ContainerFormat::Mp4)
        );
        assert_eq!(
            ContainerFormat::from_path(Path::new("out.MOV")),
            Some(ContainerFormat::Mov)
        );
        assert_eq!(
            ContainerFormat::from_path(Path::new("out.foo")),
            None
        );
    }

    #[test]
    fn sequence_filename() {
        let t = OutputTarget::png_sequence("/tmp/out");
        let f = t.frame_filename(42).unwrap();
        assert_eq!(f.file_name().unwrap(), "frame_000042.png");
    }
}
