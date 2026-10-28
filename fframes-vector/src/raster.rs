//! Pieces filled into a label raster: for every pixel, the piece drawn last on it.
//!
//! The raster is the common ground of everything that asks "which pieces lie where":
//! assigning pieces to object zones or to a segmentation mask (majority of a piece's
//! pixels), and finding which pieces touch across the outline (adjacency for clustering).
//! It is built once at a resolution of the caller's choice; 4096 pixels on the longest
//! side is plenty for grouping and keeps the raster around 50 MB.

use crate::document::PieceDocument;
use rayon::prelude::*;
use svgr::tiny_skia::{FillRule, Mask, Path, PathBuilder, Transform};
use usvgr::svgtree::svgrtypes::{SimplePathSegment, SimplifyingPathParser};

/// Any SVG path data (relative or absolute commands, arcs, shorthand curves) as a
/// tiny-skia path. `None` when the data does not parse or is empty.
#[must_use]
pub fn skia_path(d: &str) -> Option<Path> {
    let mut pb = PathBuilder::new();
    for seg in SimplifyingPathParser::from(d) {
        match seg.ok()? {
            SimplePathSegment::MoveTo { x, y } => pb.move_to(x as f32, y as f32),
            SimplePathSegment::LineTo { x, y } => pb.line_to(x as f32, y as f32),
            SimplePathSegment::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => pb.cubic_to(
                x1 as f32, y1 as f32, x2 as f32, y2 as f32, x as f32, y as f32,
            ),
            SimplePathSegment::Quadratic { x1, y1, x, y } => {
                pb.quad_to(x1 as f32, y1 as f32, x as f32, y as f32);
            }
            SimplePathSegment::ClosePath => pb.close(),
        }
    }
    pb.finish()
}

/// The pixel indices (`y * width + x`) of a `width x height` raster that `path`, mapped by
/// `transform`, covers (nonzero rule, no anti-aliasing: a pixel counts when its centre is
/// inside).
#[must_use]
pub fn covered_pixels(path: &Path, transform: Transform, width: u32, height: u32) -> Vec<u32> {
    let Some(path) = path.clone().transform(transform) else {
        return Vec::new();
    };
    let b = path.bounds();
    let x0 = b.left().floor().max(0.0) as i64;
    let y0 = b.top().floor().max(0.0) as i64;
    let x1 = (b.right().ceil() as i64).min(i64::from(width));
    let y1 = (b.bottom().ceil() as i64).min(i64::from(height));
    if x1 <= x0 || y1 <= y0 {
        return Vec::new();
    }
    let (mw, mh) = ((x1 - x0) as u32, (y1 - y0) as u32);
    let Some(mut mask) = Mask::new(mw, mh) else {
        return Vec::new();
    };
    mask.fill_path(
        &path,
        FillRule::Winding,
        false,
        Transform::from_translate(-x0 as f32, -y0 as f32),
    );
    let mut out = Vec::new();
    for (i, &v) in mask.data().iter().enumerate() {
        if v > 127 {
            let (mx, my) = (i as u32 % mw, i as u32 / mw);
            out.push((y0 as u32 + my) * width + x0 as u32 + mx);
        }
    }
    out
}

/// For every raster pixel the index + 1 of the piece drawn last on it (0 = no piece).
pub struct PieceRaster {
    pub width: u32,
    pub height: u32,
    /// Raster pixels per document pixel.
    pub res: f32,
    pub labels: Vec<u32>,
}

impl PieceRaster {
    /// The raster at `res` raster pixels per document pixel.
    #[must_use]
    pub fn new(doc: &PieceDocument, res: f32) -> Self {
        let width = ((doc.width as f32 * res).ceil() as u32).max(1);
        let height = ((doc.height as f32 * res).ceil() as u32).max(1);
        let ts = Transform::from_scale(res, res);
        // fill in parallel, write in draw order so later pieces cover earlier ones
        let covered: Vec<Vec<u32>> = doc
            .pieces
            .par_iter()
            .map(|p| {
                skia_path(&p.d)
                    .map_or_else(Vec::new, |path| covered_pixels(&path, ts, width, height))
            })
            .collect();
        let mut labels = vec![0u32; width as usize * height as usize];
        for (i, px) in covered.iter().enumerate() {
            for &j in px {
                labels[j as usize] = i as u32 + 1;
            }
        }
        PieceRaster {
            width,
            height,
            res,
            labels,
        }
    }

    /// The raster scaled so its longest side is at most `max_side` pixels (never above the
    /// document's own resolution).
    #[must_use]
    pub fn fit(doc: &PieceDocument, max_side: u32) -> Self {
        let longest = doc.width.max(doc.height).max(1) as f32;
        Self::new(doc, (max_side as f32 / longest).min(1.0))
    }

    /// Visible pixels per piece (index = piece index).
    #[must_use]
    pub fn areas(&self, pieces: usize) -> Vec<u32> {
        let mut a = vec![0u32; pieces];
        for &l in &self.labels {
            if l > 0 {
                a[l as usize - 1] += 1;
            }
        }
        a
    }
}
