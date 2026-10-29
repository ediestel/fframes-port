use crate::error::FFramesMediaError;
use image::GenericImageView;
use std::marker::PhantomData;
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub struct ImageMetadata {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct ImageData<'a> {
    inner: Arc<usvgr::PreloadedImageData>,
    pub metadata: ImageMetadata,
    _phantom: PhantomData<&'a ()>,
}

// Fix 16: added Debug and Clone derives
#[derive(Debug, Clone)]
pub enum RawMediaFile {
    Data(Vec<u8>),
    Stream(std::path::PathBuf),
}

impl<'a> ImageData<'a> {
    pub fn new_from_bytes(filename: &str, bytes: &[u8]) -> Result<Self, FFramesMediaError> {
        let img = image::load_from_memory(bytes)?;
        let (width, height) = img.dimensions();
        let rgba = img.to_rgba8();

        // Fix 5: use PreloadedImageData::new() which pre-blends semi-transparent pixels
        let preloaded = Arc::new(usvgr::PreloadedImageData::new(
            filename.to_string(),
            width,
            height,
            &rgba.into_raw(),
        ));

        Ok(Self {
            inner: preloaded,
            metadata: ImageMetadata { width, height },
            _phantom: PhantomData,
        })
    }

    pub fn new_from_raw_data(
        data: Arc<usvgr::PreloadedImageData>,
        metadata: ImageMetadata,
    ) -> Self {
        Self {
            inner: data,
            metadata,
            _phantom: PhantomData,
        }
    }

    pub fn href(&self) -> Arc<usvgr::PreloadedImageData> {
        self.inner.clone() // Arc clone — O(1) refcount bump; avoid calling in tight per-frame loops
    }
}
