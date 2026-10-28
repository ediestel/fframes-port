//! Sprite atlases from a piece document: every piece (or every piece of one group) is
//! rasterized at a density relative to the source picture, packed into pages of at most
//! `max_size` square, and written as PNG pages plus a JSON file per page in the
//! `TexturePacker` "hash" layout that `PixiJS`, Phaser, Godot and Unity importers read.
//!
//! Each frame also carries the piece's `group`, `color` and its place in the source
//! picture (`source: [x, y, w, h]` in source pixels), so a game can lay the pieces back
//! into the picture or group them by object.

use crate::document::{Piece, PieceDocument};
use rayon::prelude::*;
use std::fmt::Write as _;
use std::path::Path;
use svgr::tiny_skia;

#[derive(Clone, Debug)]
pub struct AtlasParams {
    /// Pixels per source-picture pixel: 1 = the picture's own size, 2 = twice.
    pub density: f32,
    /// Page edge in pixels.
    pub max_size: u32,
    /// Transparent border around every sprite, in atlas pixels.
    pub padding: u32,
    /// Only pieces of this group; `None` = every piece.
    pub group: Option<String>,
    /// Leave out the outline colour (palette 0); the lead lines then stay on the board.
    pub skip_outline: bool,
}

impl Default for AtlasParams {
    fn default() -> Self {
        AtlasParams {
            density: 1.0,
            max_size: 4096,
            padding: 2,
            group: None,
            skip_outline: false,
        }
    }
}

/// One rasterized piece before packing.
struct Raster {
    id: u32,
    group: String,
    color: String,
    /// Place in the source picture, source pixels.
    source: [f32; 4],
    w: u32,
    h: u32,
    rgba: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct Sprite {
    pub id: u32,
    pub group: String,
    pub color: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub source: [f32; 4],
}

pub struct AtlasPage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub sprites: Vec<Sprite>,
}

fn rasterize(doc: &PieceDocument, piece: &Piece, scale: f32, pad: u32) -> Option<Raster> {
    let [bx, by, bw, bh] = piece.bbox;
    let w = (bw * scale).ceil() as u32 + 2 * pad;
    let h = (bh * scale).ceil() as u32 + 2 * pad;
    if w == 0 || h == 0 || w > 16384 || h > 16384 {
        return None;
    }
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{bw}" height="{bh}" viewBox="{bx} {by} {bw} {bh}"><path fill="{}" d="{}"/></svg>"#,
        doc.color_hex(piece),
        piece.d
    );
    let fontdb = usvgr::fontdb::Database::new();
    let tree = usvgr::Tree::from_str(&svg, &usvgr::Options::default(), &fontdb).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(w, h)?;
    let ctx = svgr::Context::new_from_pixmap_unsafe(&pixmap);
    let mut cache = svgr::SvgrCache::new(0);
    let pool = svgr::PixmapPool::new();
    let transform =
        tiny_skia::Transform::from_translate(pad as f32, pad as f32).pre_scale(scale, scale);
    svgr::render(
        &tree,
        transform,
        &mut pixmap.as_mut(),
        &mut cache,
        &pool,
        &ctx,
    );
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for p in pixmap.pixels() {
        let c = p.demultiply();
        rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    let k = doc.scale.max(1) as f32;
    Some(Raster {
        id: piece.id,
        group: piece.group.clone(),
        color: doc.color_hex(piece).to_string(),
        source: [bx / k, by / k, bw / k, bh / k],
        w,
        h,
        rgba,
    })
}

/// Shelf packing: sprites sorted by height, rows filled left to right, new page when full.
fn pack(mut rasters: Vec<Raster>, max_size: u32) -> Vec<AtlasPage> {
    rasters.sort_by(|a, b| b.h.cmp(&a.h).then(b.w.cmp(&a.w)).then(a.id.cmp(&b.id)));
    let mut pages: Vec<AtlasPage> = Vec::new();
    let mut placements: Vec<(usize, u32, u32)> = Vec::new(); // page, x, y per raster
    let (mut page, mut x, mut y, mut row_h) = (0usize, 0u32, 0u32, 0u32);
    let mut page_w = vec![0u32];
    let mut page_h = vec![0u32];
    for r in &rasters {
        if r.w > max_size || r.h > max_size {
            placements.push((usize::MAX, 0, 0));
            continue;
        }
        if x + r.w > max_size {
            x = 0;
            y += row_h;
            row_h = 0;
        }
        if y + r.h > max_size {
            page += 1;
            page_w.push(0);
            page_h.push(0);
            x = 0;
            y = 0;
            row_h = 0;
        }
        placements.push((page, x, y));
        x += r.w;
        row_h = row_h.max(r.h);
        page_w[page] = page_w[page].max(x);
        page_h[page] = page_h[page].max(y + row_h);
    }
    for p in 0..=page {
        // power-of-two page sizes, the safe choice for every GPU
        let w = page_w[p].max(1).next_power_of_two().min(max_size);
        let h = page_h[p].max(1).next_power_of_two().min(max_size);
        pages.push(AtlasPage {
            width: w,
            height: h,
            rgba: vec![0; (w * h * 4) as usize],
            sprites: Vec::new(),
        });
    }
    for (r, (p, px, py)) in rasters.iter().zip(&placements) {
        if *p == usize::MAX {
            continue;
        }
        let page = &mut pages[*p];
        for row in 0..r.h {
            let src = (row * r.w * 4) as usize;
            let dst = (((py + row) * page.width + px) * 4) as usize;
            page.rgba[dst..dst + (r.w * 4) as usize]
                .copy_from_slice(&r.rgba[src..src + (r.w * 4) as usize]);
        }
        page.sprites.push(Sprite {
            id: r.id,
            group: r.group.clone(),
            color: r.color.clone(),
            x: *px,
            y: *py,
            w: r.w,
            h: r.h,
            source: r.source,
        });
    }
    pages
}

/// Rasterizes and packs the pieces. Pieces larger than a page are left out.
#[must_use]
pub fn build(doc: &PieceDocument, p: &AtlasParams) -> Vec<AtlasPage> {
    let scale = p.density / doc.scale.max(1) as f32;
    let rasters: Vec<Raster> = doc
        .pieces
        .par_iter()
        .filter(|piece| p.group.as_ref().is_none_or(|g| &piece.group == g))
        .filter(|piece| !(p.skip_outline && piece.color == 0))
        .filter_map(|piece| rasterize(doc, piece, scale, p.padding))
        .collect();
    pack(rasters, p.max_size)
}

/// Writes `stem_N.png` and `stem_N.json` per page into `dir`.
pub fn write(
    pages: &[AtlasPage],
    dir: impl AsRef<Path>,
    stem: &str,
    density: f32,
) -> std::io::Result<Vec<String>> {
    let dir = dir.as_ref();
    std::fs::create_dir_all(dir)?;
    let mut written = Vec::new();
    for (n, page) in pages.iter().enumerate() {
        let png = format!("{stem}_{n}.png");
        let img = image::RgbaImage::from_raw(page.width, page.height, page.rgba.clone())
            .ok_or_else(|| std::io::Error::other("page buffer"))?;
        img.save(dir.join(&png)).map_err(std::io::Error::other)?;
        let mut json = String::new();
        json.push_str("{\"frames\":{");
        for (i, s) in page.sprites.iter().enumerate() {
            if i > 0 {
                json.push(',');
            }
            let _ = write!(
                json,
                r#""p{id}":{{"frame":{{"x":{x},"y":{y},"w":{w},"h":{h}}},"rotated":false,"trimmed":false,"spriteSourceSize":{{"x":0,"y":0,"w":{w},"h":{h}}},"sourceSize":{{"w":{w},"h":{h}}},"group":"{g}","color":"{c}","source":[{sx:.1},{sy:.1},{sw:.1},{sh:.1}]}}"#,
                id = s.id,
                x = s.x,
                y = s.y,
                w = s.w,
                h = s.h,
                g = s.group,
                c = s.color,
                sx = s.source[0],
                sy = s.source[1],
                sw = s.source[2],
                sh = s.source[3]
            );
        }
        let _ = write!(
            json,
            r#"}},"meta":{{"app":"fframes-vector","version":"1.3.0","image":"{png}","format":"RGBA8888","size":{{"w":{w},"h":{h}}},"scale":"{density}"}}}}"#,
            w = page.width,
            h = page.height
        );
        std::fs::write(dir.join(format!("{stem}_{n}.json")), json)?;
        written.push(png);
    }
    Ok(written)
}
