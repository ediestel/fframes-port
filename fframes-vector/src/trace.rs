//! Tracing: every palette colour becomes a bitmap mask, `potrace` turns each mask into
//! closed curves, and every closed outer curve with its holes becomes a [`Piece`].
//!
//! `potrace` is called as an external program (`potrace` on the `PATH`, or `POTRACE`), so
//! nothing of it is linked into this crate.

use crate::document::{Piece, PieceDocument};
use rayon::prelude::*;
use std::io::Write;
use std::process::{Command, Stdio};
use usvgr::svgtree::svgrtypes::{PathParser, PathSegment};

/// One colour's trace: its palette index and (path data, bbox, area) of every shape.
type Traced = (usize, Vec<(String, [f32; 4], f32)>);

#[derive(Clone, Copy, Debug)]
pub struct TraceParams {
    /// Speckles of fewer pixels than this are dropped (potrace `-t`).
    pub turd: u32,
    /// Speckle size for the outline colour; outlines are never dropped by default.
    pub turd_outline: u32,
    /// Corner threshold (potrace `-a`), 1.0 = smooth curves.
    pub alpha: f32,
    /// Curve optimisation tolerance (potrace `-O`).
    pub opt: f32,
}

impl TraceParams {
    #[must_use]
    pub fn outlined(scale: u32) -> Self {
        let s = scale.max(1);
        TraceParams {
            turd: 4 * s * s,
            turd_outline: 8 * s * s,
            alpha: 1.0,
            opt: 0.4,
        }
    }
}

fn potrace_svg(
    mask: &[bool],
    w: usize,
    h: usize,
    turd: u32,
    alpha: f32,
    opt: f32,
) -> Result<String, String> {
    let exe = std::env::var("POTRACE").unwrap_or_else(|_| "potrace".to_string());
    // PBM P4: 1 bits are black = the shape
    let row = w.div_ceil(8);
    let mut pbm = format!("P4\n{w} {h}\n").into_bytes();
    let mut bits = vec![0u8; row * h];
    for y in 0..h {
        for x in 0..w {
            if mask[y * w + x] {
                bits[y * row + x / 8] |= 0x80 >> (x % 8);
            }
        }
    }
    pbm.extend_from_slice(&bits);
    let mut child = Command::new(&exe)
        .args([
            "-s",
            "-u",
            "10",
            "-t",
            &turd.to_string(),
            "-a",
            &alpha.to_string(),
            "-O",
            &opt.to_string(),
            "-o",
            "-",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{exe}: {e} (install potrace or set POTRACE)"))?;
    {
        let mut stdin = child.stdin.take().ok_or("potrace stdin")?;
        stdin.write_all(&pbm).map_err(|e| e.to_string())?;
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    String::from_utf8(out.stdout).map_err(|e| e.to_string())
}

/// Rewrites potrace path data (relative, in 0.1 px units, y flipped by the group transform
/// `translate(0,H) scale(0.1,-0.1)`) as absolute `M L C Z` in pixels, and returns the
/// outer-contour bounds and area.
fn absolute_path(d: &str, height: f64) -> Option<(String, [f32; 4], f32)> {
    use std::fmt::Write;
    let map = |x: f64, y: f64| (x * 0.1, height - y * 0.1);
    let mut out = String::with_capacity(d.len());
    let (mut cx, mut cy) = (0.0f64, 0.0f64);
    let (mut sx, mut sy) = (0.0f64, 0.0f64);
    let mut bb = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    let mut area = 0.0f64;
    let mut first_contour = true;
    let mut contour_area = 0.0f64;
    let mut in_first = false;
    for seg in PathParser::from(d) {
        let seg = seg.ok()?;
        match seg {
            PathSegment::MoveTo { abs, x, y } => {
                if in_first {
                    first_contour = false;
                }
                let (nx, ny) = if abs { (x, y) } else { (cx + x, cy + y) };
                cx = nx;
                cy = ny;
                sx = nx;
                sy = ny;
                let (px, py) = map(nx, ny);
                let _ = write!(out, "M{px:.1} {py:.1}");
                in_first = first_contour;
                if in_first {
                    contour_area = 0.0;
                }
            }
            PathSegment::LineTo { abs, x, y } => {
                let (nx, ny) = if abs { (x, y) } else { (cx + x, cy + y) };
                if in_first {
                    contour_area += cx * ny - nx * cy;
                }
                cx = nx;
                cy = ny;
                let (px, py) = map(nx, ny);
                let _ = write!(out, "L{px:.1} {py:.1}");
            }
            PathSegment::CurveTo {
                abs,
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                let (ax1, ay1, ax2, ay2, nx, ny) = if abs {
                    (x1, y1, x2, y2, x, y)
                } else {
                    (cx + x1, cy + y1, cx + x2, cy + y2, cx + x, cy + y)
                };
                if in_first {
                    contour_area += cx * ny - nx * cy;
                }
                cx = nx;
                cy = ny;
                let (p1x, p1y) = map(ax1, ay1);
                let (p2x, p2y) = map(ax2, ay2);
                let (px, py) = map(nx, ny);
                let _ = write!(out, "C{p1x:.1} {p1y:.1} {p2x:.1} {p2y:.1} {px:.1} {py:.1}");
            }
            PathSegment::ClosePath { .. } => {
                if in_first {
                    contour_area += cx * sy - sx * cy;
                    area = contour_area.abs() / 2.0 * 0.01;
                }
                cx = sx;
                cy = sy;
                out.push('Z');
            }
            _ => return None, // potrace emits nothing else
        }
        let (px, py) = map(cx, cy);
        bb[0] = bb[0].min(px);
        bb[1] = bb[1].min(py);
        bb[2] = bb[2].max(px);
        bb[3] = bb[3].max(py);
    }
    if bb[0] == f64::MAX {
        return None;
    }
    Some((
        out,
        [
            bb[0] as f32,
            bb[1] as f32,
            (bb[2] - bb[0]) as f32,
            (bb[3] - bb[1]) as f32,
        ],
        area as f32,
    ))
}

fn paths_in(svg: &str) -> Vec<&str> {
    let mut v = Vec::new();
    let mut rest = svg;
    while let Some(i) = rest.find(" d=\"") {
        let s = &rest[i + 4..];
        let Some(j) = s.find('"') else { break };
        v.push(&s[..j]);
        rest = &s[j + 1..];
    }
    v
}

/// Traces an indexed image (`indices` into `palette`, index 0 = outline colour) into pieces.
/// Pieces come out ordered by colour (larger first), the outline colour last.
pub fn trace(
    palette: &[[u8; 3]],
    indices: &[u16],
    w: usize,
    h: usize,
    scale: u32,
    p: &TraceParams,
) -> Result<PieceDocument, String> {
    // colour order: by pixel count descending, outline (0) last
    let mut counts = vec![0usize; palette.len()];
    for &i in indices {
        counts[i as usize] += 1;
    }
    let mut order: Vec<usize> = (1..palette.len()).filter(|&c| counts[c] > 0).collect();
    order.sort_by_key(|&c| (std::cmp::Reverse(counts[c]), c));
    order.push(0);

    let traced: Vec<Result<Traced, String>> = order
        .par_iter()
        .map(|&c| {
            let mask: Vec<bool> = indices.iter().map(|&i| i as usize == c).collect();
            let turd = if c == 0 { p.turd_outline } else { p.turd };
            let svg = potrace_svg(&mask, w, h, turd, p.alpha, p.opt)?;
            let pieces = paths_in(&svg)
                .into_iter()
                .filter_map(|d| absolute_path(d, h as f64))
                .collect();
            Ok((c, pieces))
        })
        .collect();

    let mut pieces = Vec::new();
    let mut id = 0u32;
    for r in traced {
        let (c, ps) = r?;
        for (d, bbox, area) in ps {
            id += 1;
            pieces.push(Piece {
                id,
                color: c as u16,
                d,
                bbox,
                area,
                group: String::new(),
            });
        }
    }
    Ok(PieceDocument {
        width: w as u32,
        height: h as u32,
        scale,
        palette: palette
            .iter()
            .map(|c| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]))
            .collect(),
        pieces,
    })
}
