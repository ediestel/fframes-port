//! Pictures and drawings to piece-based vector art.
//!
//! 1. [`flatten`]: outlines become flat black, every area between them one even colour.
//! 2. [`quantize`]: the colours reduced to a palette, without dithering.
//! 3. [`trace`]: every colour traced with `potrace` into closed pieces.
//! 4. [`document::PieceDocument`]: the pieces with ids, colours, bounds and groups; saved as
//!    JSON, exported as SVG.
//! 5. [`group`]: pieces named as objects from hints (zone SVG, colour mask), piece by
//!    piece or by automatic [`cluster`]s, on a [`raster::PieceRaster`]; [`cut`] first splits
//!    pieces that cross a zone border, for parts that move on their own.
//! 6. [`tree::PieceTree`]: the document as a static, cached `svgr` tree for fframes, whole,
//!    per object, or everything but one object.
//! 7. [`atlas`]: sprite atlases for game engines.
//!
//! [`vectorize`] runs 1 to 4 on an RGB image. Settings come as presets
//! ([`FlattenParams::outlined`], [`TraceParams::outlined`]) for pictures with drawn outlines
//! (stained glass, comics, flat illustration).

pub mod atlas;
pub mod cluster;
pub mod cut;
pub mod depth;
pub mod document;
pub mod drawing;
pub mod figure;
pub mod flatten;
pub mod gltf;
pub mod group;
pub mod lowpoly;
pub mod quantize;
pub mod raster;
pub mod report;
pub mod trace;
pub mod tree;

pub use cluster::ClusterParams;
pub use document::{Piece, PieceDocument};
pub use flatten::FlattenParams;
pub use group::{AssignParams, GroupMap, LabelLayer};
pub use raster::PieceRaster;
pub use trace::TraceParams;
pub use tree::PieceTree;

/// Settings of one vectorisation run.
#[derive(Clone, Copy, Debug)]
pub struct VectorizeParams {
    /// Upscale before flattening: 2 for most pictures, 3 for close-ups of small faces.
    pub scale: u32,
    /// Palette size including the outline colour.
    pub colors: usize,
    pub flatten: FlattenParams,
    pub trace: TraceParams,
    /// Clean a scanned or photographed drawing first (even white paper).
    pub scan: Option<drawing::ScanParams>,
}

impl VectorizeParams {
    /// The preset for pictures with drawn outlines.
    #[must_use]
    pub fn outlined(scale: u32) -> Self {
        VectorizeParams {
            scale,
            colors: 96,
            flatten: FlattenParams::outlined(scale),
            trace: TraceParams::outlined(scale),
            scan: None,
        }
    }

    /// The preset for scans and photos of drawings: paper levelled to white, gaps in the
    /// lines closed, a smaller palette.
    #[must_use]
    pub fn drawing(scale: u32) -> Self {
        VectorizeParams {
            colors: 32,
            flatten: FlattenParams::drawing(scale),
            scan: Some(drawing::ScanParams::new(1)),
            ..Self::outlined(scale)
        }
    }
}

/// Upscales with a Catmull-Rom filter (close to Mitchell) and returns RGB bytes.
#[must_use]
pub fn upscale(img: &image::RgbImage, scale: u32) -> image::RgbImage {
    if scale <= 1 {
        return img.clone();
    }
    image::imageops::resize(
        img,
        img.width() * scale,
        img.height() * scale,
        image::imageops::FilterType::CatmullRom,
    )
}

/// The flattened picture, for review and as the puzzle piece map.
#[must_use]
pub fn flat_image(img: &image::RgbImage, p: &VectorizeParams) -> image::RgbImage {
    let cleaned;
    let img = match &p.scan {
        Some(s) => {
            cleaned = drawing::clean_scan(img, s);
            &cleaned
        }
        None => img,
    };
    let big = upscale(img, p.scale);
    let (w, h) = (big.width() as usize, big.height() as usize);
    let flat = flatten::flatten(big.as_raw(), w, h, &p.flatten);
    image::RgbImage::from_raw(w as u32, h as u32, flat).unwrap_or_else(|| big.clone())
}

/// Runs the whole pipeline on an RGB picture.
pub fn vectorize(img: &image::RgbImage, p: &VectorizeParams) -> Result<PieceDocument, String> {
    let flat = flat_image(img, p);
    let (w, h) = (flat.width() as usize, flat.height() as usize);
    let (palette, indices) = quantize::quantize(flat.as_raw(), p.colors);
    trace::trace(&palette, &indices, w, h, p.scale, &p.trace)
}
