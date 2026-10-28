//! Piece-wise flattening of a picture with drawn outlines, in two steps.
//!
//! 1. Outlines, written as flat black: pixels darker than `dark` (strongest channel, 0..1) that
//!    belong to a structure thinner than about `2 * line_w` pixels (an opening with that radius
//!    removes it) and to a dark blob of at least `speck` pixels. Soft lines, thin structures
//!    darker than their surroundings by `soft_t` (a black-hat), keep their own colour darkened.
//! 2. Pieces: everything else is grown into regions of similar colour; a pixel joins a region
//!    while it is within `tol` (RGB distance) of the region's running mean, so a piece ends at an
//!    outline or at a colour change. Each piece becomes its mean colour; pieces under
//!    `min_piece` pixels take the colour of their largest neighbour.
//!
//! The result quantises and traces without islands.

use std::collections::HashMap;

/// Flattening settings, in pixels of the image being flattened.
#[derive(Clone, Copy, Debug)]
pub struct FlattenParams {
    pub dark: f32,
    pub min_piece: usize,
    pub line_w: usize,
    pub speck: usize,
    pub tol: f32,
    pub soft_t: u8,
    /// Close gaps in the outlines up to this many pixels ([`crate::drawing::bridge_gaps`]);
    /// 0 = off.
    pub gap: usize,
}

impl FlattenParams {
    /// The stained-glass / comic preset, tuned on the Golden Spurs window, for a picture
    /// upscaled by `scale` (areas grow with `scale²`, widths with `scale`).
    #[must_use]
    pub fn outlined(scale: u32) -> Self {
        let s = scale.max(1) as usize;
        FlattenParams {
            dark: 0.42,
            min_piece: 16 * s * s,
            line_w: 4 * s,
            speck: 20 * s * s,
            tol: 24.0,
            soft_t: 32,
            gap: 0,
        }
    }

    /// The preset for drawings (ink or pencil on paper, coloured or not): lines a little
    /// lighter than printed outlines count, and gaps up to 10 source pixels are closed.
    #[must_use]
    pub fn drawing(scale: u32) -> Self {
        let s = scale.max(1) as usize;
        FlattenParams {
            dark: 0.5,
            gap: 10 * s,
            speck: 12 * s * s,
            ..Self::outlined(scale)
        }
    }
}

/// Connected components (4-neighbourhood) of `mask`: labels and sizes.
fn components(mask: &[bool], w: usize, h: usize) -> (Vec<u32>, Vec<usize>) {
    let n = w * h;
    let mut label = vec![u32::MAX; n];
    let mut sizes: Vec<usize> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    for start in 0..n {
        if !mask[start] || label[start] != u32::MAX {
            continue;
        }
        let id = sizes.len() as u32;
        sizes.push(0);
        label[start] = id;
        stack.push(start);
        while let Some(i) = stack.pop() {
            sizes[id as usize] += 1;
            let (x, y) = (i % w, i / w);
            let mut visit = |j: usize| {
                if mask[j] && label[j] == u32::MAX {
                    label[j] = id;
                    stack.push(j);
                }
            };
            if x > 0 {
                visit(i - 1);
            }
            if x + 1 < w {
                visit(i + 1);
            }
            if y > 0 {
                visit(i - w);
            }
            if y + 1 < h {
                visit(i + w);
            }
        }
    }
    (label, sizes)
}

/// Morphological closing (max, then min) of a brightness map with a square of radius `r`.
fn closing_u8(v: &[u8], w: usize, h: usize, r: usize) -> Vec<u8> {
    let pass = |src: &[u8], max: bool| -> Vec<u8> {
        let mut tmp = vec![0u8; w * h];
        for y in 0..h {
            for x in 0..w {
                let (x0, x1) = (x.saturating_sub(r), (x + r).min(w - 1));
                let it = (x0..=x1).map(|xx| src[y * w + xx]);
                tmp[y * w + x] = if max {
                    it.max().unwrap_or(0)
                } else {
                    it.min().unwrap_or(0)
                };
            }
        }
        let mut out = vec![0u8; w * h];
        for y in 0..h {
            let (y0, y1) = (y.saturating_sub(r), (y + r).min(h - 1));
            for x in 0..w {
                let it = (y0..=y1).map(|yy| tmp[yy * w + x]);
                out[y * w + x] = if max {
                    it.max().unwrap_or(0)
                } else {
                    it.min().unwrap_or(0)
                };
            }
        }
        out
    };
    let dilated = pass(v, true);
    pass(&dilated, false)
}

/// Morphological opening of `mask` with a square of radius `r`.
fn opening(mask: &[bool], w: usize, h: usize, r: usize) -> Vec<bool> {
    let pass = |src: &[bool], all: bool| -> Vec<bool> {
        let mut tmp = vec![false; w * h];
        for y in 0..h {
            for x in 0..w {
                let (x0, x1) = (x.saturating_sub(r), (x + r).min(w - 1));
                tmp[y * w + x] = if all {
                    (x0..=x1).all(|xx| src[y * w + xx])
                } else {
                    (x0..=x1).any(|xx| src[y * w + xx])
                };
            }
        }
        let mut out = vec![false; w * h];
        for y in 0..h {
            let (y0, y1) = (y.saturating_sub(r), (y + r).min(h - 1));
            for x in 0..w {
                out[y * w + x] = if all {
                    (y0..=y1).all(|yy| tmp[yy * w + x])
                } else {
                    (y0..=y1).any(|yy| tmp[yy * w + x])
                };
            }
        }
        out
    };
    let eroded = pass(mask, true);
    pass(&eroded, false)
}

/// Flattens an RGB buffer (`w * h * 3` bytes). Outline pixels come out as pure black `[0,0,0]`.
#[must_use]
pub fn flatten(rgb: &[u8], w: usize, h: usize, p: &FlattenParams) -> Vec<u8> {
    let px = |i: usize| -> [u8; 3] { [rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2]] };
    let n = w * h;
    let thresh = (p.dark * 255.0) as u8;
    let darkm: Vec<bool> = (0..n)
        .map(|i| {
            let c = px(i);
            c[0].max(c[1]).max(c[2]) < thresh
        })
        .collect();

    // 1. hard outlines: dark, thin, not a speck
    let thick = opening(&darkm, w, h, p.line_w);
    let (dlabel, dsizes) = components(&darkm, w, h);
    let mut hard: Vec<bool> = (0..n)
        .map(|i| darkm[i] && !thick[i] && dsizes[dlabel[i] as usize] >= p.speck)
        .collect();
    // drawn outlines: close the gaps so every area becomes its own piece
    crate::drawing::bridge_gaps(&mut hard, w, h, p.gap);

    // soft lines: black-hat of the brightness
    let bright: Vec<u8> = (0..n)
        .map(|i| {
            let c = px(i);
            c[0].max(c[1]).max(c[2])
        })
        .collect();
    let closed = closing_u8(&bright, w, h, p.line_w);
    let softm: Vec<bool> = (0..n)
        .map(|i| !hard[i] && !darkm[i] && closed[i].saturating_sub(bright[i]) >= p.soft_t)
        .collect();
    let (slabel, ssizes) = components(&softm, w, h);
    let mut ssum = vec![[0u64; 3]; ssizes.len()];
    for i in 0..n {
        if softm[i] {
            let c = px(i);
            let s = &mut ssum[slabel[i] as usize];
            s[0] += u64::from(c[0]);
            s[1] += u64::from(c[1]);
            s[2] += u64::from(c[2]);
        }
    }
    let soft_col: Vec<[u8; 3]> = ssum
        .iter()
        .zip(&ssizes)
        .map(|(s, &k)| {
            let k = k.max(1) as u64;
            [
                (s[0] / k * 80 / 100) as u8,
                (s[1] / k * 80 / 100) as u8,
                (s[2] / k * 80 / 100) as u8,
            ]
        })
        .collect();
    let soft: Vec<bool> = (0..n)
        .map(|i| softm[i] && ssizes[slabel[i] as usize] >= p.speck)
        .collect();
    let line: Vec<bool> = (0..n).map(|i| hard[i] || soft[i]).collect();

    // 2. pieces grown by colour similarity between the outlines
    let tol2 = p.tol * p.tol;
    let mut label = vec![u32::MAX; n];
    let mut sums: Vec<[f64; 3]> = Vec::new();
    let mut sizes: Vec<usize> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    for start in 0..n {
        if line[start] || label[start] != u32::MAX {
            continue;
        }
        let id = sums.len() as u32;
        sums.push([0.0; 3]);
        sizes.push(0);
        label[start] = id;
        stack.push(start);
        while let Some(i) = stack.pop() {
            let c = px(i);
            let s = &mut sums[id as usize];
            s[0] += f64::from(c[0]);
            s[1] += f64::from(c[1]);
            s[2] += f64::from(c[2]);
            sizes[id as usize] += 1;
            let k = sizes[id as usize] as f64;
            let mean = [(s[0] / k) as f32, (s[1] / k) as f32, (s[2] / k) as f32];
            let (x, y) = (i % w, i / w);
            let mut visit = |j: usize| {
                if line[j] || label[j] != u32::MAX {
                    return;
                }
                let q = px(j);
                let d = (f32::from(q[0]) - mean[0]).powi(2)
                    + (f32::from(q[1]) - mean[1]).powi(2)
                    + (f32::from(q[2]) - mean[2]).powi(2);
                if d <= tol2 {
                    label[j] = id;
                    stack.push(j);
                }
            };
            if x > 0 {
                visit(i - 1);
            }
            if x + 1 < w {
                visit(i + 1);
            }
            if y > 0 {
                visit(i - w);
            }
            if y + 1 < h {
                visit(i + w);
            }
        }
    }
    let mean: Vec<[u8; 3]> = sums
        .iter()
        .zip(&sizes)
        .map(|(s, &k)| {
            let k = k.max(1) as f64;
            [(s[0] / k) as u8, (s[1] / k) as u8, (s[2] / k) as u8]
        })
        .collect();

    // grain: small pieces take the colour of the largest neighbouring piece
    let mut remap: HashMap<u32, u32> = HashMap::new();
    for i in 0..n {
        let l = label[i];
        if l == u32::MAX || sizes[l as usize] >= p.min_piece || remap.contains_key(&l) {
            continue;
        }
        let (x, y) = (i % w, i / w);
        let mut best: Option<u32> = None;
        for dy in -3i64..=3 {
            for dx in -3i64..=3 {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                    continue;
                }
                let m = label[ny as usize * w + nx as usize];
                if m != u32::MAX
                    && m != l
                    && best.is_none_or(|b| sizes[m as usize] > sizes[b as usize])
                {
                    best = Some(m);
                }
            }
        }
        if let Some(b) = best {
            remap.insert(l, b);
        }
    }
    let mut out = vec![0u8; n * 3];
    for i in 0..n {
        let c = if hard[i] {
            [0, 0, 0]
        } else if soft[i] {
            soft_col[slabel[i] as usize]
        } else {
            let mut l = label[i];
            let mut hops = 0;
            while let Some(&m) = remap.get(&l) {
                l = m;
                hops += 1;
                if hops > 8 {
                    break;
                }
            }
            mean[l as usize]
        };
        out[i * 3..i * 3 + 3].copy_from_slice(&c);
    }
    out
}
