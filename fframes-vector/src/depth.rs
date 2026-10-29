//! A 3D figure from a turntable video by depth: every pixel gets its depth from how it moves
//! through neighbouring frames in front of a still camera. An initial even-turn calibration
//! is refined from silhouettes before stereo uses the measured per-frame angles.
//!
//! 1. Mask: background is the screen colour (most common in the frame borders) within the
//!    spread the video gives it (measured in the borders), everywhere, so gaps between parts (an
//!    arm and the body, a weapon and the rider) are background too; every other colour is
//!    figure. Specks are cleaned.
//! 2. Half turn and axis: seen at 180°, a figure's outline is the mirror image of its outline
//!    (orthographic view). The frame distance at which every frame's row edges mirror best is
//!    half a turn; the mirror line is the axis.
//! 3. Depth: a point at `X'` from the axis and depth `D` in frame `k` shows at
//!    `axis + X' cos(a) + D sin(a)` in frame `f`, where `a = turn[f] - turn[k]`.
//!    For every pixel each depth is tried
//!    against several neighbouring frames at once (`±views`), colours compared over a small
//!    window; the best depth is refined between hypotheses. A depth whose best match is not
//!    clearly better than the average stays unknown (flat colour).
//! 4. Direction: a mirrored figure turning the other way shows the same outlines and colours;
//!    only occlusion differs. The direction is the one in which points measured in one frame
//!    do not end up in front of the surface another frame measured.
//! 5. Surface: a voxel is empty when it falls outside the mask in more than a few frames or
//!    lies in front of a measured surface (signed distances averaged over all frames); the
//!    rest is solid. Its surface is painted from the frame that faces it most directly and
//!    sees it.

use crate::figure::{surface_mesh_field, write_glb};
use rayon::prelude::*;

#[derive(Clone, Debug)]
pub struct DepthParams {
    /// Height of the figure in metres.
    pub height: f32,
    /// Neighbouring frames on each side compared for every pixel.
    pub views: usize,
    /// Share of those views that count for a depth: the best-matching ones, so views in which
    /// the point is hidden do not count.
    pub kept: f32,
    /// Half-width of the colour window, pixels.
    pub window: usize,
    /// Voxel edge, pixels.
    pub voxel: f32,
    /// How far in front of a measured surface a point still counts as on it, pixels (the
    /// depth maps' noise, not the voxel size).
    pub surface: f32,
    /// Best match cost against the mean over all depths; above it the depth stays unknown.
    pub sharpness: f32,
    /// Frames a voxel may fall outside the mask in and stay; `None` is one in forty.
    pub outside: Option<usize>,
    /// The figure turns clockwise seen from above; `None` finds it.
    pub clockwise: Option<bool>,
    /// Frames to draw loss pictures of (see [`LossPicture`]).
    pub check: Vec<usize>,
    /// The figure from the stacked outlines alone (the envelope): no depth check; depths
    /// still choose which frame paints a face.
    pub hull: bool,
    /// Rounds of measuring every frame's own angle against the envelope (0: even turn).
    pub refine: usize,
    /// Rounds of smoothing of the surface (each moves a corner halfway to its neighbours'
    /// mean); the exported surface is measured after them.
    pub smooth: usize,
    /// Every `holdout`-th frame is excluded from geometry calibration, angle fitting,
    /// stereo matching and carving, and its angle is inferred from the retained frames
    /// (0 or 1: none). Held-out images may still supply texture colours.
    pub holdout: usize,
    /// Also a reduced model with at most this many vertices and one baked texture.
    pub low: Option<usize>,
    /// How far each frame's angle is searched around its current value, degrees.
    pub search: f32,
    /// Also a dataset for Gaussian splatting (Nerfstudio format) in this folder.
    pub splat: Option<std::path::PathBuf>,
}

impl DepthParams {
    fn held(&self, frame: usize) -> bool {
        self.holdout > 1 && frame % self.holdout == self.holdout / 2
    }
}

impl Default for DepthParams {
    fn default() -> Self {
        DepthParams {
            height: 1.7,
            views: 5,
            kept: 1.0,
            window: 2,
            voxel: 1.5,
            surface: 4.5,
            sharpness: 0.6,
            outside: None,
            clockwise: None,
            check: Vec::new(),
            hull: false,
            refine: 2,
            smooth: 2,
            holdout: 8,
            low: Some(5000),
            search: 3.0,
            splat: None,
        }
    }
}

/// What the depth reconstruction found.
#[derive(Clone, Debug)]
pub struct DepthTurn {
    pub frames: usize,
    /// Frames per half turn (fractional).
    pub half_turn: f32,
    /// Uniform-turn step in degrees, before per-frame angle refinement.
    pub step_degrees: f32,
    /// Axis column, pixels.
    pub axis: f32,
    /// Mean mirror error of the row edges at the half turn, pixels.
    pub mirror_error: f32,
    pub clockwise: bool,
    /// Share of measured points that end up in front of another frame's surface, for the
    /// direction used and the other one; `None` when the direction was given.
    pub violations: Option<(f32, f32)>,
    /// Share of retained-frame figure pixels with a known depth.
    pub known: f32,
    pub voxels: usize,
    /// Ten height bands, top to bottom: voxels kept, removed by the outline check (outside
    /// the mask in too many frames), removed by the depth check (in front of a surface).
    pub bands: Vec<[usize; 3]>,
    pub checks: Vec<LossPicture>,
    /// Per round of angle measurement: mean and largest correction, degrees.
    pub corrections: Vec<(f32, f32)>,
    /// Ten height bands, top to bottom: how far the outline disagrees with itself half a turn
    /// later (pixels); each outline is widened by half of it for the shape.
    pub disagreement: Vec<f32>,
    /// Loose fragments removed.
    pub fragments: usize,
    /// Fit of the model's shadow to the frames that shaped it, and to the held-out frames.
    pub fit_used: FrameFit,
    pub fit_held: FrameFit,
    /// The same for the exported surface itself (its triangles, after smoothing), against the
    /// frames that shaped it and the held-out ones.
    pub fit_mesh_used: FrameFit,
    pub fit_mesh_held: FrameFit,
    /// The reduced model (binary glTF) and what the reduction gave, when asked for.
    pub low: Option<(Vec<u8>, crate::lowpoly::LowPoly)>,
}

/// A frame with its lost figure pixels painted: red removed by the outline check, blue by the
/// depth check; with the counts.
#[derive(Clone, Debug)]
pub struct LossPicture {
    pub frame: usize,
    pub picture: image::RgbaImage,
    pub figure: usize,
    pub by_outline: usize,
    pub by_depth: usize,
    /// Background pixels the model's shadow covers (painted magenta).
    pub excess: usize,
}

/// How well the model's shadow fits a frame's outline (or the mean over several frames).
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameFit {
    pub frames: usize,
    /// Figure pixels the shadow misses, share of the figure.
    pub missed: f32,
    /// Background pixels the shadow covers, share of the figure.
    pub excess: f32,
    /// Overlap over union of shadow and figure.
    pub iou: f32,
    /// Mean distance between the two outlines, both ways, pixels.
    pub boundary: f32,
}

impl FrameFit {
    pub(crate) fn mean(fits: &[&FrameFit]) -> FrameFit {
        let n = fits.len().max(1) as f32;
        FrameFit {
            frames: fits.len(),
            missed: fits.iter().map(|f| f.missed).sum::<f32>() / n,
            excess: fits.iter().map(|f| f.excess).sum::<f32>() / n,
            iou: fits.iter().map(|f| f.iou).sum::<f32>() / n,
            boundary: fits.iter().map(|f| f.boundary).sum::<f32>() / n,
        }
    }
}

/// Distance of every pixel to the nearest pixel where `set` is true (city block, capped).
fn distance_to(set: &[bool], w: usize, h: usize) -> Vec<u16> {
    let mut d: Vec<u16> = set.iter().map(|&s| if s { 0 } else { u16::MAX }).collect();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if x > 0 {
                d[i] = d[i].min(d[i - 1].saturating_add(1));
            }
            if y > 0 {
                d[i] = d[i].min(d[i - w].saturating_add(1));
            }
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let i = y * w + x;
            if x + 1 < w {
                d[i] = d[i].min(d[i + 1].saturating_add(1));
            }
            if y + 1 < h {
                d[i] = d[i].min(d[i + w].saturating_add(1));
            }
        }
    }
    d
}

/// Outline pixels: inside, with a 4-neighbour outside.
fn outline(set: &[bool], w: usize, h: usize) -> Vec<bool> {
    (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            set[i]
                && (x == 0 || y == 0 || x + 1 == w || y + 1 == h || !set[i - 1] || !set[i + 1] || !set[i - w] || !set[i + w])
        })
        .collect()
}

/// A frame with the screen's colour taken out along the figure's edge. On a coloured screen
/// (channel `c` well above the others) the edge picks up the screen colour: on the test video
/// 94 % of tinted figure pixels lie within 3 px of the edge, 2 % further in than 8 px. Within
/// SPILL_BAND pixels of the edge channel `c` is kept to the stronger of the other two; the
/// inside keeps its colours, green or not.
#[must_use]
pub fn despill(img: &image::RgbaImage, mask: &[bool], c: usize) -> image::RgbaImage {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let mut dist: Vec<u8> = mask.iter().map(|&m| if m { u8::MAX } else { 0 }).collect();
    for step in 1..=SPILL_BAND {
        let before = dist.clone();
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if before[i] != u8::MAX {
                    continue;
                }
                let near = (x > 0 && before[i - 1] == step - 1)
                    || (x + 1 < w && before[i + 1] == step - 1)
                    || (y > 0 && before[i - w] == step - 1)
                    || (y + 1 < h && before[i + w] == step - 1);
                if near {
                    dist[i] = step;
                }
            }
        }
    }
    let mut out = img.clone();
    for (i, px) in out.pixels_mut().enumerate() {
        if (1..=SPILL_BAND).contains(&dist[i]) {
            let others = (0..3).filter(|&o| o != c).map(|o| px.0[o]).max().unwrap_or(0);
            px.0[c] = px.0[c].min(others);
        }
    }
    out
}

/// The fit of a shadow to a mask.
#[must_use]
pub fn fit(shadow: &[bool], mask: &[bool], w: usize, h: usize) -> FrameFit {
    frame_fit(shadow, mask, w, h)
}

pub(crate) fn frame_fit(shadow: &[bool], mask: &[bool], w: usize, h: usize) -> FrameFit {
    let (mut both, mut missed, mut excess, mut figure) = (0usize, 0usize, 0usize, 0usize);
    for (&s, &m) in shadow.iter().zip(mask) {
        both += usize::from(s && m);
        missed += usize::from(m && !s);
        excess += usize::from(s && !m);
        figure += usize::from(m);
    }
    let (so, mo) = (outline(shadow, w, h), outline(mask, w, h));
    let (to_mask, to_shadow) = (distance_to(&mo, w, h), distance_to(&so, w, h));
    let one_way = |from: &[bool], to: &[u16]| {
        let (sum, k) = from.iter().zip(to).filter(|(o, _)| **o).fold((0u64, 0u64), |(s, k), (_, &d)| (s + u64::from(d), k + 1));
        if k == 0 { 0.0 } else { sum as f32 / k as f32 }
    };
    let f = figure.max(1) as f32;
    FrameFit {
        frames: 1,
        missed: missed as f32 / f,
        excess: excess as f32 / f,
        iou: both as f32 / (both + missed + excess).max(1) as f32,
        boundary: (one_way(&so, &to_mask) + one_way(&mo, &to_shadow)) / 2.0,
    }
}

/// Depth maps of all frames: `D` per pixel (pixels, towards the camera for the clockwise
/// sign), NaN where unknown, background or held out.
pub struct DepthMaps {
    pub width: u32,
    pub height: u32,
    pub axis: f32,
    pub reach: f32,
    pub maps: Vec<Vec<f32>>,
}

const TRUNC: f32 = 90.0;

/// Width of the band along the figure's edge where screen colour is taken out, pixels.
const SPILL_BAND: u8 = 4;

/// Width of the background border strips, pixels.
const BORDER: u32 = 8;

/// The screen colour and the video's spread of it: the most common colour in the border
/// strips of all frames, and the largest distance of any border pixel from it. The screen is
/// one colour; the video stores it as a scatter of neighbouring values.
#[must_use]
pub fn screen_colour(frames: &[image::RgbaImage]) -> ([u8; 3], f32) {
    screen_colour_in(frames.iter())
}

fn screen_colour_in<'a>(frames: impl Iterator<Item = &'a image::RgbaImage>) -> ([u8; 3], f32) {
    let mut count: std::collections::HashMap<[u8; 3], usize> = std::collections::HashMap::new();
    let border = |img: &image::RgbaImage| {
        let (w, h) = img.dimensions();
        (0..h).flat_map(move |y| (0..w).map(move |x| (x, y))).filter(move |&(x, y)| {
            y < BORDER || y >= h - BORDER || x < BORDER || x >= w - BORDER
        })
    };
    for img in frames {
        for (x, y) in border(img) {
            let p = img.get_pixel(x, y).0;
            *count.entry([p[0], p[1], p[2]]).or_default() += 1;
        }
    }
    let colour = count.iter().max_by_key(|e| e.1).map_or([0, 0, 0], |e| *e.0);
    let spread = count.keys().map(|c| distance(*c, colour)).fold(0.0, f32::max);
    (colour, spread)
}

fn distance(a: [u8; 3], b: [u8; 3]) -> f32 {
    (0..3).map(|c| (f32::from(a[c]) - f32::from(b[c])).powi(2)).sum::<f32>().sqrt()
}

/// Background is the screen colour within the video's measured spread, everywhere (so gaps
/// between parts are background too); every other colour is figure. Figure specks and
/// background specks under `min` pixels are flipped.
#[must_use]
pub fn key_mask(img: &image::RgbaImage, (colour, spread): ([u8; 3], f32), min: usize) -> Vec<bool> {
    let (w, h) = img.dimensions();
    let mut mask: Vec<bool> = img
        .pixels()
        .map(|p| p.0[3] >= 128 && distance([p.0[0], p.0[1], p.0[2]], colour) > spread)
        .collect();
    flip_small(&mut mask, w as usize, h as usize, true, min);
    flip_small(&mut mask, w as usize, h as usize, false, min);
    mask
}

/// Components (4-connected) of `value` smaller than `min` pixels become `!value`.
fn flip_small(mask: &mut [bool], w: usize, h: usize, value: bool, min: usize) {
    let mut seen = vec![false; mask.len()];
    let mut stack = Vec::new();
    let mut part = Vec::new();
    for start in 0..mask.len() {
        if seen[start] || mask[start] != value {
            continue;
        }
        part.clear();
        stack.push(start);
        seen[start] = true;
        while let Some(i) = stack.pop() {
            part.push(i);
            let (x, y) = (i % w, i / w);
            let mut go = |j: usize| {
                if !seen[j] && mask[j] == value {
                    seen[j] = true;
                    stack.push(j);
                }
            };
            if x > 0 {
                go(i - 1);
            }
            if x + 1 < w {
                go(i + 1);
            }
            if y > 0 {
                go(i - w);
            }
            if y + 1 < h {
                go(i + w);
            }
        }
        if part.len() < min {
            for &i in &part {
                mask[i] = !value;
            }
        }
    }
}

/// Left and right figure pixel of every row, per frame (`None` for rows without figure).
fn row_edges(mask: &[bool], w: usize, h: usize) -> Vec<Option<(f32, f32)>> {
    (0..h)
        .map(|y| {
            let row = &mask[y * w..(y + 1) * w];
            let l = row.iter().position(|&m| m)?;
            let r = row.iter().rposition(|&m| m)?;
            Some((l as f32, r as f32))
        })
        .collect()
}

/// Frames per half turn (fractional), the axis column and the mean mirror error, from the
/// row edges: at half a turn the left edge of one frame mirrors the right edge of the other.
fn half_turn(edges: &[Vec<Option<(f32, f32)>>]) -> Result<(f32, f32, f32), String> {
    let n = edges.len();
    let score = |p: usize| -> Option<(f32, f32)> {
        let mut sums = Vec::new();
        for k in 0..n - p {
            for (a, b) in edges[k].iter().zip(&edges[k + p]).step_by(2) {
                if let (Some((la, ra)), Some((lb, rb))) = (a, b) {
                    sums.push(lb + ra);
                    sums.push(la + rb);
                }
            }
        }
        if sums.len() < 200 {
            return None;
        }
        let mut sorted = sums.clone();
        sorted.sort_by(f32::total_cmp);
        let c2 = sorted[sorted.len() / 2];
        let err = sums.iter().map(|s| (s - c2).abs()).sum::<f32>() / sums.len() as f32 / 2.0;
        Some((err, c2 / 2.0))
    };
    let lo = (n / 8).max(4);
    let hi = n.saturating_sub(n / 8).max(lo + 1);
    let scores: Vec<(usize, f32, f32)> =
        (lo..hi).filter_map(|p| score(p).map(|(e, c)| (p, e, c))).collect();
    let &(p, e, c) = scores
        .iter()
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .ok_or("the video turns less than half a turn: no frame mirrors another")?;
    // refine between frames: a parabola through the errors around the best frame distance
    let at = |q: usize| scores.iter().find(|s| s.0 == q).map(|s| s.1);
    let pf = match (p.checked_sub(1).and_then(at), at(p + 1)) {
        (Some(a), Some(b)) if a + b - 2.0 * e > 1e-6 => p as f32 + 0.5 * (a - b) / (a + b - 2.0 * e),
        _ => p as f32,
    };
    Ok((pf, c, e))
}

fn sample(img: &image::RgbaImage, x: f32, y: u32) -> Option<[f32; 3]> {
    let x0 = x.floor();
    if x0 < 0.0 || x0 as u32 + 1 >= img.width() {
        return None;
    }
    let t = x - x0;
    let a = img.get_pixel(x0 as u32, y).0;
    let b = img.get_pixel(x0 as u32 + 1, y).0;
    Some(std::array::from_fn(|c| f32::from(a[c]) * (1.0 - t) + f32::from(b[c]) * t))
}

/// Horizontal image variation is needed to measure an orthographic horizontal disparity.
/// Low-contrast windows retain their silhouette geometry instead of arbitrary sweep depths.
fn has_horizontal_texture(img: &image::RgbaImage, x: usize, y: usize, window: usize) -> bool {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let window = window.max(1);
    let (mut gradient, mut samples) = (0u64, 0u64);
    for yy in y.saturating_sub(window)..=(y + window).min(h - 1) {
        for xx in x.saturating_sub(window)..(x + window).min(w - 1) {
            let a = img.get_pixel(xx as u32, yy as u32).0;
            let b = img.get_pixel(xx as u32 + 1, yy as u32).0;
            gradient += (0..3).map(|c| u64::from(a[c].abs_diff(b[c]))).sum::<u64>();
            samples += 1;
        }
    }
    samples > 0 && gradient > 3 * samples
}

/// Depth of every figure pixel of frame `k` by sweeping depths `D` and comparing each against
/// the retained frames `k ± 1..=views` at once, using their measured `turn` angles.
/// `D` is towards the camera for the clockwise sign.
#[allow(clippy::too_many_arguments)]
fn frame_depth(
    frames: &[image::RgbaImage],
    masks: &[Vec<bool>],
    k: usize,
    axis: f32,
    reach: f32,
    turn: &[f32],
    p: &DepthParams,
) -> Vec<f32> {
    let img = &frames[k];
    let (w, h) = (img.width() as usize, img.height() as usize);
    let mask = &masks[k];
    let mut depth = vec![f32::NAN; w * h];
    if p.held(k) {
        return depth;
    }
    // the mask's box, widened by the window
    let (mut l, mut t, mut r, mut b) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if mask[y * w + x] {
                (l, t, r, b) = (l.min(x), t.min(y), r.max(x), b.max(y));
            }
        }
    }
    if r < l {
        return depth;
    }
    let win = p.window;
    let (l, t) = (l.saturating_sub(win), t.saturating_sub(win));
    let (r, b) = ((r + win).min(w - 1), (b + win).min(h - 1));
    let (bw, bh) = (r - l + 1, b - t + 1);
    let views: Vec<(usize, f32, f32)> = (1..=p.views as i64)
        .flat_map(|m| [m, -m])
        .filter_map(|m| {
            let f = k as i64 + m;
            if !(0..frames.len() as i64).contains(&f) || p.held(f as usize) {
                return None;
            }
            let f = f as usize;
            let delta = turn[f] - turn[k];
            Some((f, delta.cos(), delta.sin()))
        })
        .collect();
    if views.is_empty() {
        return depth;
    }
    let kept = ((views.len() as f32 * p.kept).ceil() as usize).clamp(1, views.len());
    let mut costs: Vec<f32> = Vec::with_capacity(views.len());
    let hyps = (2.0 * reach).ceil() as usize + 1;
    let n = bw * bh;
    let mut raw = vec![0.0_f32; n];
    let mut tmp = vec![0.0_f32; n];
    let mut agg = vec![0.0_f32; n];
    let mut prev = vec![f32::INFINITY; n];
    let mut best = vec![f32::INFINITY; n];
    let mut best_h = vec![0usize; n];
    let mut left = vec![f32::INFINITY; n];
    let mut right = vec![f32::INFINITY; n];
    let mut pending = vec![false; n];
    let mut sum = vec![0.0_f32; n];
    let area = ((2 * win + 1) * (2 * win + 1)) as f32;
    for hi in 0..hyps {
        let d = -reach + hi as f32;
        for yy in 0..bh {
            let y = t + yy;
            for xx in 0..bw {
                let x = l + xx;
                let xp = x as f32 - axis;
                if xp * xp + d * d > reach * reach {
                    raw[yy * bw + xx] = TRUNC;
                    continue;
                }
                let here = img.get_pixel(x as u32, y as u32).0;
                let inside = mask[y * w + x];
                costs.clear();
                for &(f, c, s) in &views {
                    let xm = axis + xp * c + d * s;
                    costs.push(match sample(&frames[f], xm, y as u32) {
                        Some(q) if !inside || masks[f][y * w + xm.round() as usize] => {
                            ((0..3).map(|i| (q[i] - f32::from(here[i])).abs()).sum::<f32>()).min(TRUNC)
                        }
                        _ => TRUNC,
                    });
                }
                // the best-matching share of the views: a view in which the point is hidden
                // (another part in front) does not count against the right depth
                costs.sort_unstable_by(f32::total_cmp);
                raw[yy * bw + xx] = costs[..kept].iter().sum::<f32>() / kept as f32;
            }
        }
        // box filter: rows, then columns
        for yy in 0..bh {
            let row = &raw[yy * bw..(yy + 1) * bw];
            let mut s: f32 = row[..win.min(bw)].iter().sum();
            for xx in 0..bw {
                if xx + win < bw {
                    s += row[xx + win];
                }
                if xx > win {
                    s -= row[xx - win - 1];
                }
                tmp[yy * bw + xx] = s;
            }
        }
        for xx in 0..bw {
            let mut s: f32 = (0..win.min(bh)).map(|yy| tmp[yy * bw + xx]).sum();
            for yy in 0..bh {
                if yy + win < bh {
                    s += tmp[(yy + win) * bw + xx];
                }
                if yy > win {
                    s -= tmp[(yy - win - 1) * bw + xx];
                }
                agg[yy * bw + xx] = s / area;
            }
        }
        for i in 0..n {
            let a = agg[i];
            sum[i] += a;
            if pending[i] {
                right[i] = a;
                pending[i] = false;
            }
            if a < best[i] {
                best[i] = a;
                best_h[i] = hi;
                left[i] = prev[i];
                right[i] = f32::INFINITY;
                pending[i] = true;
            }
        }
        std::mem::swap(&mut prev, &mut agg);
    }
    for yy in 0..bh {
        for xx in 0..bw {
            let (x, y) = (l + xx, t + yy);
            let i = yy * bw + xx;
            // only where the whole window lies on the figure: in parts narrower than the
            // window (legs) the background in it matches at any depth, so their shape comes
            // from the outlines alone
            let whole = x >= win && y >= win && win < w - x && win < h - y
                && (y - win..=y + win).all(|yy| (x - win..=x + win).all(|xx| mask[yy * w + xx]));
            if !whole || !has_horizontal_texture(img, x, y, win) {
                continue;
            }
            let mean = sum[i] / hyps as f32;
            if !(mean > 0.0 && best[i] <= p.sharpness * mean) {
                continue;
            }
            let (a, c, e) = (left[i], right[i], best[i]);
            // Flat minima and minima at the sweep limits do not establish a depth.
            if !(a.is_finite() && c.is_finite() && a > e && c > e) {
                continue;
            }
            let off = if a.is_finite() && c.is_finite() && a + c - 2.0 * e > 1e-6 {
                (0.5 * (a - c) / (a + c - 2.0 * e)).clamp(-0.5, 0.5)
            } else {
                0.0
            };
            depth[y * w + x] = -reach + best_h[i] as f32 + off;
        }
    }
    depth
}

/// Depth maps of all frames (see the module notes, steps 1 to 3), with the half turn found.
pub fn depth_maps(
    frames: &[image::RgbaImage],
    p: &DepthParams,
) -> Result<(DepthMaps, Vec<Vec<bool>>, f32, f32), String> {
    let (mut d, masks, half, mirror) = prepare_depth(frames, p)?;
    let turn: Vec<f32> = (0..frames.len()).map(|k| k as f32 * std::f32::consts::PI / half).collect();
    d.maps = (0..frames.len())
        .into_par_iter()
        .map(|k| frame_depth(frames, &masks, k, d.axis, d.reach, &turn, p))
        .collect();
    d.maps = consistent_depths(&d, &turn, p);
    Ok((d, masks, half, mirror))
}

/// Calibrate and segment before running stereo, so silhouette refinement uses no depth maps.
fn prepare_depth(
    frames: &[image::RgbaImage],
    p: &DepthParams,
) -> Result<(DepthMaps, Vec<Vec<bool>>, f32, f32), String> {
    let n = frames.len();
    if n < 4 || p.views > n.saturating_sub(2) / 2 {
        return Err(format!("{n} frames; need at least four and enough for the requested neighbouring views"));
    }
    if !p.height.is_finite() || p.height <= 0.0 || !p.voxel.is_finite() || p.voxel <= 0.0
        || !p.surface.is_finite() || p.surface <= 0.0
        || !p.kept.is_finite() || p.kept <= 0.0 || p.kept > 1.0
        || !p.sharpness.is_finite() || !(0.0..=1.0).contains(&p.sharpness)
    {
        return Err("height, voxel and surface must be finite and positive; kept must be in (0, 1], sharpness in [0, 1]".into());
    }
    let (w, h) = frames[0].dimensions();
    if frames.iter().any(|f| f.dimensions() != (w, h)) {
        return Err("frames differ in size".into());
    }
    if w == 0 || h == 0 || p.window > (w.min(h) as usize - 1) / 2 {
        return Err("frames must be nonempty and fit the full colour window".into());
    }
    let screen = screen_colour_in(frames.iter().enumerate().filter(|(f, _)| !p.held(*f)).map(|(_, img)| img));
    let masks: Vec<Vec<bool>> = frames.par_iter().map(|f| key_mask(f, screen, 30)).collect();
    let edges: Vec<_> = masks.iter().enumerate()
        .map(|(f, m)| if p.held(f) { vec![None; h as usize] } else { row_edges(m, w as usize, h as usize) })
        .collect();
    let (half, axis, mirror) = half_turn(&edges)?;
    let reach = edges
        .iter()
        .flatten()
        .flatten()
        .map(|&(l, r)| (axis - l).max(r - axis))
        .fold(0.0_f32, f32::max)
        + 2.0;
    let figure = masks.iter().map(|m| m.iter().filter(|&&v| v).count()).sum::<usize>() / n;
    eprintln!(
        "screen colour {:?}, spread {:.1}, {figure} figure pixels per frame, half turn {half:.2} frames, axis x {axis:.1}, turning radius {reach:.0} px",
        screen.0, screen.1
    );
    Ok((DepthMaps { width: w, height: h, axis, reach, maps: Vec::new() }, masks, half, mirror))
}

/// Share of measured points (sampled) that land more than `tol` pixels in front of the
/// surface another frame measured, for one direction `sign` (+1 clockwise).
fn violations(d: &DepthMaps, turn: &[f32], sign: f32, tol: f32) -> f32 {
    let (w, h) = (d.width as usize, d.height as usize);
    let n = d.maps.len();
    let step = (turn[n - 1] - turn[0]) / (n - 1) as f32;
    let quarter = ((std::f32::consts::FRAC_PI_2 / step).round() as usize).max(1);
    let (mut bad, mut all) = (0usize, 0usize);
    for k in (0..n).step_by(4) {
        let tk = sign * turn[k];
        for y in (0..h).step_by(4) {
            for x in (0..w).step_by(4) {
                let dk = d.maps[k][y * w + x];
                if dk.is_nan() {
                    continue;
                }
                let (xp, zp) = (x as f32 - d.axis, sign * dk);
                let (ox, oz) = (xp * tk.cos() - zp * tk.sin(), xp * tk.sin() + zp * tk.cos());
                for f in [k.wrapping_sub(quarter / 2), k + quarter / 2, k.wrapping_sub(quarter), k + quarter] {
                    if f >= n {
                        continue;
                    }
                    let tf = sign * turn[f];
                    let (fx, fz) = (ox * tf.cos() + oz * tf.sin(), -ox * tf.sin() + oz * tf.cos());
                    let px = (d.axis + fx).round();
                    if px < 0.0 || px as usize >= w {
                        continue;
                    }
                    let df = d.maps[f][y * w + px as usize];
                    if df.is_nan() {
                        continue;
                    }
                    all += 1;
                    if fz > sign * df + tol {
                        bad += 1;
                    }
                }
            }
        }
    }
    if all == 0 { 0.0 } else { bad as f32 / all as f32 }
}

/// Keep depths supported by other independently measured maps. Occluded and unknown
/// samples supply no support; one isolated match cannot carve the hull at the default view count.
fn consistent_depths(d: &DepthMaps, turn: &[f32], p: &DepthParams) -> Vec<Vec<f32>> {
    let (w, h) = (d.width as usize, d.height as usize);
    d.maps.par_iter().enumerate().map(|(k, map)| {
        let views: Vec<(usize, f32, f32)> = (k.saturating_sub(p.views)..(k + p.views + 1).min(d.maps.len()))
            .filter(|&f| f != k && !p.held(f))
            .map(|f| (f, (turn[f] - turn[k]).cos(), (turn[f] - turn[k]).sin()))
            .collect();
        let required = p.views.clamp(1, 2);
        (0..w * h).map(|i| {
            let dk = map[i];
            if !dk.is_finite() {
                return f32::NAN;
            }
            let (x, y) = ((i % w) as f32 - d.axis, i / w);
            let mut support = 0;
            for &(f, c, s) in &views {
                let px = (d.axis + x * c + dk * s).round();
                if px < 0.0 || px >= w as f32 {
                    continue;
                }
                let df = d.maps[f][y * w + px as usize];
                if df.is_finite() && (df - (-x * s + dk * c)).abs() <= p.surface {
                    support += 1;
                    if support >= required {
                        return dk;
                    }
                }
            }
            f32::NAN
        }).collect()
    }).collect()
}

/// Damped angle updates with a fixed reference and a positive angular increment.
/// The upper bound uses the next old angle, so the forward lower bound stays feasible.
fn refine_turn(turn: &mut [f32], found: &[f32], step: f32, p: &DepthParams) -> (f32, f32) {
    let previous = turn.to_vec();
    let minimum = 0.1 * step;
    let anchor = found[0];
    for f in 1..turn.len() {
        let lower = turn[f - 1] + minimum;
        let upper = previous.get(f + 1).map_or(f32::INFINITY, |&t| t - minimum);
        turn[f] = (previous[f] + 0.5 * (found[f] - anchor)).clamp(lower, upper);
    }
    // Held-out angles depend only on their retained neighbours, including at the end.
    for f in 1..turn.len() {
        if p.held(f) {
            turn[f] = if f + 1 < turn.len() {
                0.5 * (turn[f - 1] + turn[f + 1])
            } else {
                turn[f - 1] + step
            };
        }
    }
    let degrees: Vec<f32> = turn.iter().zip(&previous).map(|(a, b)| (a - b).to_degrees().abs()).collect();
    (degrees.iter().sum::<f32>() / degrees.len() as f32, degrees.iter().copied().fold(0.0, f32::max))
}

/// Conservative orthographic front depths of the occupied voxel cubes.
/// A rotated cube spans v * (|cos| + |sin|) in both image x and camera depth.
#[allow(clippy::too_many_arguments)]
fn front_depth(
    shell: &[(f32, f32, usize)],
    angle: (f32, f32),
    axis: f32,
    top: f32,
    w: usize,
    h: usize,
    v: f32,
) -> Vec<f32> {
    let (c, s) = angle;
    let radius = 0.5 * v * (c.abs() + s.abs());
    let mut front = vec![f32::NEG_INFINITY; w * h];
    for &(x, z, j) in shell {
        let px = axis + x * c + z * s;
        let fz = -x * s + z * c + radius;
        let (x0, x1) = (
            (px - radius).ceil().clamp(0.0, w as f32) as usize,
            (px + radius).ceil().clamp(0.0, w as f32) as usize,
        );
        let (y0, y1) = (
            (top + (j as f32 - 1.0) * v).ceil().clamp(0.0, h as f32) as usize,
            (top + j as f32 * v).ceil().clamp(0.0, h as f32) as usize,
        );
        for y in y0..y1 {
            for x in x0..x1 {
                front[y * w + x] = front[y * w + x].max(fz);
            }
        }
    }
    front
}

/// The figure as a textured binary glTF (see the module notes), and what was found.
pub fn depth_glb(frames: &[image::RgbaImage], p: &DepthParams) -> Result<(Vec<u8>, DepthTurn, DepthMaps), String> {
    let n = frames.len();
    let (mut d, masks, half, mirror) = prepare_depth(frames, p)?;
    let step = std::f32::consts::PI / half;
    let (w, h) = (d.width as usize, d.height as usize);
    let (axis, reach) = (d.axis, d.reach);
    // Fit a positive turn first; choosing its handedness later only mirrors the hull.
    let mut turn: Vec<f32> = (0..n).map(|f| f as f32 * step).collect();

    // rows the figure spans in the retained frames
    let rows: Vec<usize> = masks
        .iter()
        .enumerate()
        .filter(|(f, _)| !p.held(*f))
        .flat_map(|(_, m)| (0..h).filter(move |&y| m[y * w..(y + 1) * w].iter().any(|&v| v)))
        .collect();
    let (top, bottom) = (*rows.iter().min().ok_or("no figure")?, *rows.iter().max().ok_or("no figure")?);
    let v = p.voxel;
    let nh = (d.reach / v).ceil() as usize + 2;
    let (nx, ny, nz) = (2 * nh + 1, ((bottom - top + 1) as f32 / v).ceil() as usize + 2, 2 * nh + 1);
    let idx = |i: usize, j: usize, k: usize| (k * ny + j) * nx + i;
    let used = (0..n).filter(|&f| !p.held(f)).count();
    let allowed = p.outside.unwrap_or(used / 40);
    // Held-out pixels take no part in the geometry or its fitted angles.
    let held = |f: usize| p.held(f);
    let mu = p.surface;

    // how well every row keeps its outline over the turn: frame k's left edge against the
    // right edge half a turn later and the other way round (pixels, median over the frames,
    // the worse of the two edges), smoothed over neighbouring rows
    let edges: Vec<_> = masks.iter().enumerate()
        .map(|(f, m)| if held(f) { vec![None; h] } else { row_edges(m, w, h) })
        .collect();
    let pairs = half.round() as usize;
    let raw: Vec<f32> = (0..h)
        .map(|y| {
            let mut errs: Vec<f32> = (0..n.saturating_sub(pairs))
                .filter_map(|k| match (edges[k][y], edges[k + pairs][y]) {
                    (Some((la, ra)), Some((lb, rb))) => {
                        Some((la + rb - 2.0 * d.axis).abs().max((ra + lb - 2.0 * d.axis).abs()))
                    }
                    _ => None,
                })
                .collect();
            if errs.is_empty() {
                return 0.0;
            }
            errs.sort_by(f32::total_cmp);
            errs[errs.len() / 2]
        })
        .collect();
    let disagreement: Vec<f32> = (0..h)
        .map(|y| {
            let mut near: Vec<f32> = raw[y.saturating_sub(5)..(y + 6).min(h)].to_vec();
            near.sort_by(f32::total_cmp);
            near[near.len() / 2]
        })
        .collect();
    // every outline widened in each row by its share of that row's disagreement (the two
    // views of a pair share it), so parts the video draws a little differently from side to
    // side are not cut away; rows that agree stay sharp
    let hull_masks: Vec<Vec<bool>> = masks
        .par_iter()
        .map(|m| {
            let mut out = vec![false; w * h];
            for y in 0..h {
                let r = (disagreement[y] / 2.0).ceil() as usize;
                let row = &m[y * w..(y + 1) * w];
                // running count of figure pixels in [x - r, x + r]
                let mut count = row[..r.min(w)].iter().filter(|&&v| v).count();
                for x in 0..w {
                    if x + r < w && row[x + r] {
                        count += 1;
                    }
                    if x > r && row[x - r - 1] {
                        count -= 1;
                    }
                    out[y * w + x] = count > 0;
                }
            }
            out
        })
        .collect();
    let band_disagreement: Vec<f32> = (0..10)
        .map(|b| {
            let (y0, y1) = (top + (bottom - top + 1) * b / 10, top + (bottom - top + 1) * (b + 1) / 10);
            disagreement[y0..y1.max(y0 + 1)].iter().sum::<f32>() / (y1.max(y0 + 1) - y0) as f32
        })
        .collect();

    // one z slice of the grid: per voxel its kind (0 kept, 1 removed by outline, 2 removed by
    // depth, 3 outside the turning space) and its signed distance (averaged, in units of
    // `surface`; negative inside, +1 where the outlines exclude it, NaN where no depth was
    // measured); per row, [kept, removed by outline, removed by depth]
    struct Slice {
        kinds: Vec<u8>,
        field: Vec<f32>,
        tally: Vec<[usize; 3]>,
    }
    let carve = |angle: &[(f32, f32)], allowed: usize, depth: Option<(&DepthMaps, f32)>| -> Vec<Slice> {
        (0..nz)
        .into_par_iter()
        .map(|k| {
            let mut out = vec![3u8; nx * ny];
            let mut field = vec![f32::NAN; nx * ny];
            let mut tally = vec![[0usize; 3]; ny];
            if k == 0 || k == nz - 1 {
                return Slice { kinds: out, field, tally };
            }
            let z = (k as f32 - nh as f32) * v;
            for j in 1..ny - 1 {
                let y = (top as f32 + (j as f32 - 0.5) * v).round();
                if y < 0.0 || y as usize >= h {
                    continue;
                }
                let y = y as usize;
                for i in 1..nx - 1 {
                    let x = (i as f32 - nh as f32) * v;
                    if x * x + z * z > reach * reach {
                        continue;
                    }
                    let (mut outside, mut tsdf, mut weight) = (0usize, 0.0_f32, 0.0_f32);
                    for (f, &(c, s)) in angle.iter().enumerate() {
                        if held(f) {
                            continue;
                        }
                        let (fx, fz) = (x * c + z * s, -x * s + z * c);
                        let px = (axis + fx).round();
                        if px < 0.0 || px as usize >= w || !hull_masks[f][y * w + px as usize] {
                            outside += 1;
                            if outside > allowed {
                                break;
                            }
                            continue;
                        }
                        if let Some((d, sign)) = depth {
                            let df = d.maps[f][y * w + px as usize];
                            if df.is_nan() {
                                continue;
                            }
                            let phi = fz - sign * df;
                            if phi > -mu {
                                tsdf += (phi / mu).min(1.0);
                                weight += 1.0;
                            }
                        }
                    }
                    let kind = if outside > allowed {
                        1
                    } else if weight > 0.0 && tsdf / weight >= 0.0 {
                        2
                    } else {
                        0
                    };
                    tally[j][usize::from(kind)] += 1;
                    out[j * nx + i] = kind;
                    field[j * nx + i] = if kind == 1 {
                        1.0
                    } else if weight > 0.0 {
                        tsdf / weight
                    } else {
                        f32::NAN
                    };
                }
            }
            Slice { kinds: out, field, tally }
        })
        .collect()
    };

    // each frame's own angle: turned within ±search degrees until the shadow of the envelope (the
    // stacked outlines, loosely) covers its outline best; compared on the voxel grid
    let to_cs = |t: &[f32]| t.iter().map(|a| (a.cos(), a.sin())).collect::<Vec<(f32, f32)>>();
    let bin = |px: f32| ((px - d.axis) / v + nh as f32).round();
    let mask_bins: Vec<Vec<bool>> = masks
        .par_iter()
        .map(|m| {
            let mut g = vec![false; nx * ny];
            for y in top..=bottom {
                let j = ((y as f32 - top as f32) / v + 0.5).round() as usize;
                for x in 0..w {
                    let b = bin(x as f32);
                    if m[y * w + x] && j < ny && b >= 0.0 && (b as usize) < nx {
                        g[j * nx + b as usize] = true;
                    }
                }
            }
            g
        })
        .collect();
    let mut corrections = Vec::new();
    for _ in 0..p.refine {
        let env = carve(&to_cs(&turn), (used / 10).max(allowed), None);
        let solid_at = |i: usize, j: usize, k: usize| env[k].kinds[j * nx + i] == 0;
        let shell: Vec<(f32, f32, usize)> = (1..nz - 1)
            .flat_map(|k| (1..ny - 1).flat_map(move |j| (1..nx - 1).map(move |i| (i, j, k))))
            .filter(|&(i, j, k)| {
                solid_at(i, j, k)
                    && (!solid_at(i + 1, j, k) || !solid_at(i - 1, j, k) || !solid_at(i, j, k + 1) || !solid_at(i, j, k - 1))
            })
            .map(|(i, j, k)| ((i as f32 - nh as f32) * v, (k as f32 - nh as f32) * v, j))
            .collect();
        let found: Vec<f32> = (0..n)
            .into_par_iter()
            .map(|f| {
                if held(f) {
                    return 0.0;
                }
                let mut best = (f32::MIN, 0.0_f32);
                let mut shadow = vec![false; nx * ny];
                let range = (p.search / 0.25).round() as i32;
                for q in -range..=range {
                    let delta = (q as f32 * 0.25).to_radians();
                    let (c, s) = ((turn[f] + delta).cos(), (turn[f] + delta).sin());
                    shadow.iter_mut().for_each(|b| *b = false);
                    for &(x, z, j) in &shell {
                        let b = bin(d.axis + x * c + z * s);
                        if b >= 0.0 && (b as usize) < nx {
                            shadow[j * nx + b as usize] = true;
                        }
                    }
                    let (mut both, mut either) = (0usize, 0usize);
                    for (a, b) in shadow.iter().zip(&mask_bins[f]) {
                        both += usize::from(*a && *b);
                        either += usize::from(*a || *b);
                    }
                    let fit = both as f32 / either.max(1) as f32;
                    if fit > best.0 || (fit == best.0 && delta.abs() < best.1.abs()) {
                        best = (fit, delta);
                    }
                }
                best.1
            })
            .collect();
        // a frame's angle cannot jump away from its neighbours': median over five frames
        let found: Vec<f32> = (0..n)
            .map(|f| {
                let mut near: Vec<f32> = (f.saturating_sub(2)..(f + 3).min(n))
                    .filter(|&k| !held(k)).map(|k| found[k]).collect();
                near.sort_by(f32::total_cmp);
                near.get(near.len() / 2).copied().unwrap_or(0.0)
            })
            .collect();
        corrections.push(refine_turn(&mut turn, &found, step, p));
    }
    // Stereo and direction checks use exactly the angles that will carve and texture.
    d.maps = (0..n).into_par_iter()
        .map(|k| frame_depth(frames, &masks, k, axis, reach, &turn, p)).collect();
    d.maps = consistent_depths(&d, &turn, p);
    let (clockwise, viol) = match p.clockwise {
        Some(c) => (c, None),
        None => {
            let (a, b) = (violations(&d, &turn, 1.0, 3.0), violations(&d, &turn, -1.0, 3.0));
            if a <= b { (true, Some((a, b))) } else { (false, Some((b, a))) }
        }
    };
    let sign = if clockwise { 1.0 } else { -1.0 };
    for t in &mut turn {
        *t *= sign;
    }
    let figure_px: usize = masks.iter().enumerate().filter(|(f, _)| !held(*f))
        .map(|(_, m)| m.iter().filter(|&&m| m).count()).sum();
    let known_px: usize = d.maps.iter().flatten().filter(|v| v.is_finite()).count();
    let angle = to_cs(&turn);
    let slices = carve(&angle, allowed, if p.hull { None } else { Some((&d, sign)) });
    let mut kinds = vec![3u8; nx * ny * nz];
    let mut field = vec![f32::NAN; nx * ny * nz];
    let mut bands = vec![[0usize; 3]; 10];
    for (k, s) in slices.iter().enumerate() {
        for j in 0..ny {
            for i in 0..nx {
                kinds[idx(i, j, k)] = s.kinds[j * nx + i];
                field[idx(i, j, k)] = s.field[j * nx + i];
            }
            let band = (j * 10 / ny).min(9);
            for c in 0..3 {
                bands[band][c] += s.tally[j][c];
            }
        }
    }
    drop(slices);
    let mut solid: Vec<bool> = kinds.iter().map(|&c| c == 0).collect();
    // loose fragments: connected pieces (6-neighbours) under a hundredth of the largest go
    let mut piece = vec![u32::MAX; solid.len()];
    let mut sizes: Vec<usize> = Vec::new();
    let mut stack = Vec::new();
    for start in 0..solid.len() {
        if !solid[start] || piece[start] != u32::MAX {
            continue;
        }
        let id = sizes.len() as u32;
        let mut size = 0;
        piece[start] = id;
        stack.push(start);
        while let Some(c) = stack.pop() {
            size += 1;
            let (i, rest) = (c % nx, c / nx);
            let (j, k) = (rest % ny, rest / ny);
            let mut go = |q: usize| {
                if solid[q] && piece[q] == u32::MAX {
                    piece[q] = id;
                    stack.push(q);
                }
            };
            if i > 0 { go(c - 1); }
            if i + 1 < nx { go(c + 1); }
            if j > 0 { go(c - nx); }
            if j + 1 < ny { go(c + nx); }
            if k > 0 { go(c - nx * ny); }
            if k + 1 < nz { go(c + nx * ny); }
        }
        sizes.push(size);
    }
    let largest = sizes.iter().copied().max().unwrap_or(0);
    let fragments = sizes.iter().filter(|&&s| s * 100 < largest).count();
    for (c, s) in solid.iter_mut().enumerate() {
        if *s && sizes[piece[c] as usize] * 100 < largest {
            *s = false;
        }
    }
    drop(piece);
    let voxels = solid.iter().filter(|&&s| s).count();

    // the model's shadow in a frame: its outer voxels (the inside casts no other shadow),
    // each covering its voxel's span of columns and rows
    let shell: Vec<(f32, f32, usize)> = (1..nz - 1)
        .flat_map(|k| (1..ny - 1).flat_map(move |j| (1..nx - 1).map(move |i| (i, j, k))))
        .filter(|&(i, j, k)| {
            solid[idx(i, j, k)]
                && [idx(i + 1, j, k), idx(i - 1, j, k), idx(i, j + 1, k), idx(i, j - 1, k), idx(i, j, k + 1), idx(i, j, k - 1)]
                    .iter()
                    .any(|&q| !solid[q])
        })
        .map(|(i, j, k)| ((i as f32 - nh as f32) * v, (k as f32 - nh as f32) * v, j))
        .collect();
    let shadow = |f: usize| -> Vec<bool> {
        front_depth(&shell, angle[f], axis, top as f32, w, h, v)
            .into_iter().map(|z| z.is_finite()).collect()
    };
    // how well the model's shadow fits every frame's outline
    let fits: Vec<(bool, FrameFit)> = (0..n)
        .into_par_iter()
        .map(|f| (held(f), frame_fit(&shadow(f), &masks[f], w, h)))
        .collect();
    let average = |which: bool| {
        let chosen: Vec<&FrameFit> = fits.iter().filter(|(hf, _)| *hf == which).map(|(_, x)| x).collect();
        FrameFit::mean(&chosen)
    };
    let (fit_used, fit_held) = (average(false), average(true));

    // check pictures: figure pixels of a frame with no solid voxel behind them, red where
    // the outline check removed the voxels along their ray, blue where the depth check did;
    // magenta where the model's shadow covers background (excess)
    let checks: Vec<LossPicture> = p
        .check
        .iter()
        .filter(|&&f| f < n)
        .map(|&f| {
            let (c, s) = angle[f];
            let mut picture = frames[f].clone();
            let (mut figure, mut by_outline, mut by_depth) = (0, 0, 0);
            let cast = shadow(f);
            let mut excess = 0;
            for (i, (&m, &sh)) in masks[f].iter().zip(&cast).enumerate() {
                if sh && !m {
                    excess += 1;
                    picture.get_pixel_mut((i % w) as u32, (i / w) as u32).0 = [255, 0, 255, 255];
                }
            }
            for y in 0..h {
                let j = ((y as f32 - top as f32) / v + 0.5).round();
                if j < 0.0 || j as usize >= ny {
                    continue;
                }
                let j = j as usize;
                for x in 0..w {
                    if !masks[f][y * w + x] {
                        continue;
                    }
                    figure += 1;
                    let fx = x as f32 - d.axis;
                    let (mut kept, mut outline, mut depth) = (false, 0, 0);
                    let mut fz = -d.reach;
                    while fz <= d.reach {
                        let (ox, oz) = (fx * c - fz * s, fx * s + fz * c);
                        let (i, k) = ((ox / v + nh as f32).round(), (oz / v + nh as f32).round());
                        if i >= 0.0 && k >= 0.0 && (i as usize) < nx && (k as usize) < nz {
                            let q = idx(i as usize, j, k as usize);
                            if solid[q] {
                                kept = true;
                                break;
                            }
                            match kinds[q] {
                                1 => outline += 1,
                                2 => depth += 1,
                                _ => {}
                            }
                        }
                        fz += v * 0.5;
                    }
                    if !kept {
                        let px = picture.get_pixel_mut(x as u32, y as u32);
                        if depth > outline {
                            by_depth += 1;
                            px.0 = [40, 90, 255, 255];
                        } else {
                            by_outline += 1;
                            px.0 = [255, 40, 40, 255];
                        }
                    }
                }
            }
            LossPicture { frame: f, picture, figure, by_outline, by_depth, excess }
        })
        .collect();

    // texture: every frame cropped to the frames' common box, in a grid of tiles
    let (mut l0, mut r0) = (w, 0);
    for m in &masks {
        for y in top..=bottom {
            for x in 0..w {
                if m[y * w + x] {
                    (l0, r0) = (l0.min(x), r0.max(x));
                }
            }
        }
    }
    let (tw, th) = ((r0 - l0 + 1) as u32, (bottom - top + 1) as u32);
    let cols = (n as f32).sqrt().ceil() as u32;
    let mut atlas = image::RgbaImage::new(tw * cols, th * (n as u32).div_ceil(cols));
    let (screen, _) = screen_colour_in(frames.iter().enumerate().filter(|(f, _)| !held(*f)).map(|(_, img)| img));
    let spill = (0..3).find(|&c| (0..3).filter(|&o| o != c).all(|o| i32::from(screen[c]) > i32::from(screen[o]) + 40));
    let clean: Vec<image::RgbaImage> = frames
        .par_iter()
        .enumerate()
        .map(|(f, img)| match spill {
            Some(c) => despill(img, &masks[f], c),
            None => img.clone(),
        })
        .collect();
    // the screen around the figure filled with the nearest figure colours, so that no texture
    // filtering (between pixels, or smaller copies for distant views) ever mixes in the screen
    let tiles: Vec<image::RgbaImage> = clean
        .par_iter()
        .enumerate()
        .map(|(f, img)| {
            let mut tile = image::imageops::crop_imm(img, l0 as u32, top as u32, tw, th).to_image();
            let (tw, th) = (tw as usize, th as usize);
            let mut filled: Vec<bool> =
                (0..tw * th).map(|i| masks[f][(top + i / tw) * w + l0 + i % tw]).collect();
            let mut todo = true;
            while todo {
                todo = false;
                let before = filled.clone();
                for y in 0..th {
                    for x in 0..tw {
                        if before[y * tw + x] {
                            continue;
                        }
                        let (mut sum, mut k) = ([0u32; 3], 0u32);
                        for (dx, dy) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
                            let (qx, qy) = (x as i64 + dx, y as i64 + dy);
                            if qx >= 0 && qy >= 0 && (qx as usize) < tw && (qy as usize) < th && before[qy as usize * tw + qx as usize] {
                                let q = tile.get_pixel(qx as u32, qy as u32).0;
                                for c in 0..3 {
                                    sum[c] += u32::from(q[c]);
                                }
                                k += 1;
                            }
                        }
                        if k > 0 {
                            tile.put_pixel(x as u32, y as u32, image::Rgba([(sum[0] / k) as u8, (sum[1] / k) as u8, (sum[2] / k) as u8, 255]));
                            filled[y * tw + x] = true;
                            todo = true;
                        }
                    }
                }
            }
            tile
        })
        .collect();
    for (f, tile) in tiles.iter().enumerate() {
        image::imageops::overlay(&mut atlas, tile, i64::from(f as u32 % cols * tw), i64::from(f as u32 / cols * th));
    }
    drop(tiles);
    let mut png = Vec::new();
    atlas
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    let (aw, ah) = (atlas.width() as f32, atlas.height() as f32);
    // Geometry visibility is available even where stereo cannot measure depth.
    // Store only the cropped foreground rectangle, rather than the whole source frame.
    let visibility: Vec<Vec<f32>> = angle.par_iter()
        .map(|&a| front_depth(&shell, a, axis - l0 as f32, 0.0, tw as usize, th as usize, v))
        .collect();
    let to_camera: Vec<[f32; 2]> = angle.iter().map(|&(c, s)| [-s, c]).collect();
    // a grid point in frame f: image column, depth and image row
    let at = |f: usize, g: [f32; 3]| {
        let (x, z) = ((g[0] - nh as f32) * v, (g[2] - nh as f32) * v);
        let (c, s) = angle[f];
        let (fx, fz) = (x * c + z * s, -x * s + z * c);
        (d.axis + fx, fz, (top as f32 + (g[1] - 0.5) * v).clamp(0.0, h as f32 - 1.0))
    };
    let uv = |nrm: [f32; 3], _axis: usize, centre: [f32; 3], g: [f32; 3]| -> [f32; 2] {
        let mut order: Vec<usize> = (0..n).collect();
        let facing = |f: usize| nrm[0] * to_camera[f][0] + nrm[2] * to_camera[f][1];
        order.sort_by(|&a, &b| facing(b).total_cmp(&facing(a)));
        // the frame for the whole face, by its centre: facing it most directly, the centre
        // inside the figure there, visible in the hull, and preferably stereo-supported
        let inside = |f: usize| {
            let (px, _, y) = at(f, centre);
            let pi = px.round();
            pi >= 1.0 && (pi as usize) + 1 < w && {
                let row = &masks[f][y as usize * w..(y as usize + 1) * w];
                row[pi as usize - 1] && row[pi as usize] && row[pi as usize + 1]
            }
        };
        let visible = |f: usize| {
            let (px, fz, y) = at(f, centre);
            let (x, y) = ((px - l0 as f32).round(), (y - top as f32).floor());
            x >= 0.0 && x < tw as f32 && y >= 0.0 && y < th as f32 && {
                let front = visibility[f][y as usize * tw as usize + x as usize];
                front.is_finite() && (front - fz).abs() <= 2.0 * v
            }
        };
        let on_surface = |f: usize| {
            let (px, fz, y) = at(f, centre);
            let df = d.maps[f][y as usize * w + px.round() as usize];
            !df.is_nan() && (fz - sign * df).abs() < 2.0 * mu
        };
        let f = order
            .iter()
            .copied()
            .find(|&f| inside(f) && visible(f) && on_surface(f))
            .or_else(|| order.iter().copied().find(|&f| inside(f) && visible(f)))
            .unwrap_or(order[0]);
        let (px, _, y) = at(f, g);
        let (sx, sy) = (px - l0 as f32, y - top as f32);
        let (tx, ty) = ((f as u32 % cols * tw) as f32, (f as u32 / cols * th) as f32);
        [(tx + sx.clamp(0.0, tw as f32 - 1.0) + 0.5) / aw, (ty + sy.clamp(0.0, th as f32 - 1.0) + 0.5) / ah]
    };
    let scale = p.height / ((bottom - top + 1) as f32 / v);
    // the surface: from the signed distances where depth measured them, else the voxels'
    // boundary; smoothed `p.smooth` times
    let mesh = surface_mesh_field(&solid, (!p.hull).then_some(field.as_slice()), [nx, ny, nz], scale, &uv, p.smooth);
    drop(field);
    // world (metres, as the surface is placed) to grid to frame
    let project = |f: usize, world: [f32; 3]| {
        let g = [
            world[0] / scale + nx as f32 / 2.0,
            ny as f32 - 1.0 - world[1] / scale,
            world[2] / scale + nz as f32 / 2.0,
        ];
        at(f, g)
    };
    let views = crate::lowpoly::Views { frames: &clean, masks: &masks, width: w, height: h, to_camera: &to_camera, held: &held };
    // the exported surface itself against every frame's outline (after smoothing)
    let (fit_mesh_used, fit_mesh_held) = crate::lowpoly::mesh_fit(&mesh.positions, &mesh.indices, &views, &project);
    if let Some(dir) = &p.splat {
        let points: Vec<([f32; 3], [u8; 3])> = shell
            .iter()
            .step_by(2)
            .filter_map(|&(x, z, j)| {
                // coloured from the frame facing the point's direction from the axis
                let f = (0..n).max_by(|&a, &b| {
                    let fa = x * to_camera[a][0] + z * to_camera[a][1];
                    let fb = x * to_camera[b][0] + z * to_camera[b][1];
                    fa.total_cmp(&fb)
                })?;
                let (c, s) = angle[f];
                let (col, row) = ((d.axis + x * c + z * s).round(), (top as f32 + (j as f32 - 0.5) * v).round());
                if col < 0.0 || row < 0.0 || col as usize >= w || row as usize >= h || !masks[f][row as usize * w + col as usize] {
                    return None;
                }
                let q = clean[f].get_pixel(col as u32, row as u32).0;
                Some(([x, (top as f32 + (j as f32 - 0.5) * v), z], [q[0], q[1], q[2]]))
            })
            .collect();
        crate::splat::write_dataset(dir, &clean, &masks, &angle, d.axis, p.height / (bottom - top + 1) as f32, &points)?;
    }
    // the reduced model with one baked texture
    let low = match p.low {
        Some(budget) => Some(crate::lowpoly::low_poly_glb(&mesh, budget, &views, &project)?),
        None => None,
    };
    Ok((
        write_glb(&mesh, &png),
        DepthTurn {
            frames: n,
            half_turn: half,
            step_degrees: 180.0 / half,
            axis: d.axis,
            mirror_error: mirror,
            clockwise,
            violations: viol,
            known: known_px as f32 / figure_px.max(1) as f32,
            voxels,
            bands,
            checks,
            corrections,
            disagreement: band_disagreement,
            fragments,
            fit_used,
            fit_held,
            fit_mesh_used,
            fit_mesh_held,
            low,
        },
        d,
    ))
}

/// A depth map as a grey picture for checking: mid grey at the axis, lighter towards the
/// camera, black where unknown.
#[must_use]
pub fn depth_picture(d: &DepthMaps, f: usize) -> image::GrayImage {
    image::GrayImage::from_fn(d.width, d.height, |x, y| {
        let v = d.maps[f][(y * d.width + x) as usize];
        if v.is_nan() {
            image::Luma([0])
        } else {
            image::Luma([(128.0 + v / d.reach * 126.0).clamp(1.0, 255.0) as u8])
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A box with a striped texture turning about the image centre, seen orthographically.
    fn turning_box(n: usize, step: f32) -> Vec<image::RgbaImage> {
        turning_box_at(&(0..n).map(|f| f as f32 * step).collect::<Vec<_>>())
    }

    fn turning_box_at(turn: &[f32]) -> Vec<image::RgbaImage> {
        let (w, h) = (160u32, 60u32);
        let (hx, hz) = (30.0_f32, 14.0_f32);
        turn.iter()
            .map(|&a| {
                let (c, s) = (a.cos(), a.sin());
                image::RgbaImage::from_fn(w, h, |x, y| {
                    if !(10..50).contains(&y) {
                        return image::Rgba([0, 177, 64, 255]);
                    }
                    let xp = x as f32 + 0.5 - 80.0;
                    // nearest surface along the view ray: largest depth inside the box
                    let mut found = None;
                    let mut dz = 60.0;
                    while dz > -60.0 {
                        let (ox, oz) = (xp * c - dz * s, xp * s + dz * c);
                        if ox.abs() <= hx && oz.abs() <= hz {
                            found = Some((ox, oz));
                            break;
                        }
                        dz -= 0.25;
                    }
                    match found {
                        None => image::Rgba([0, 177, 64, 255]),
                        Some((ox, oz)) => {
                            let t = ((ox * 0.9).sin() * 0.5 + (oz * 1.3).cos() * 0.5 + 1.0) * 0.5;
                            let v = (40.0 + 180.0 * t) as u8;
                            image::Rgba([v, v / 2 + 60, 255 - v, 255])
                        }
                    }
                })
            })
            .collect()
    }

    #[test]
    fn fit_counts_missed_excess_and_outline_distance() {
        let (w, h) = (40, 30);
        let square = |x0: usize, x1: usize| -> Vec<bool> {
            (0..w * h).map(|i| (x0..x1).contains(&(i % w)) && (10..20).contains(&(i / w))).collect()
        };
        let same = frame_fit(&square(10, 20), &square(10, 20), w, h);
        assert_eq!((same.missed, same.excess, same.iou, same.boundary), (0.0, 0.0, 1.0, 0.0));
        // a shadow 4 columns too wide on the right: 40 % excess, nothing missed
        let wide = frame_fit(&square(10, 24), &square(10, 20), w, h);
        assert_eq!(wide.missed, 0.0);
        assert!((wide.excess - 0.4).abs() < 1e-6, "{wide:?}");
        assert!((wide.iou - 100.0 / 140.0).abs() < 1e-6, "{wide:?}");
        assert!(wide.boundary > 0.5 && wide.boundary < 4.0, "{wide:?}");
    }

    #[test]
    fn finds_half_turn_axis_and_depth() {
        let step = std::f32::consts::PI / 30.0;
        let frames = turning_box(48, step);
        let p = DepthParams { views: 3, ..DepthParams::default() };
        let (d, _, half, _) = depth_maps(&frames, &p).expect("depth");
        assert!((half - 30.0).abs() < 0.6, "half turn {half}");
        assert!((d.axis - 80.0).abs() < 1.0, "axis {}", d.axis);
        // frame 0: the box faces the camera; its front is 14 px in front of the axis
        let row = 30 * d.width as usize;
        let known: Vec<f32> = (60..100).map(|x| d.maps[0][row + x]).filter(|v| !v.is_nan()).collect();
        assert!(known.len() > 10, "{} known", known.len());
        let mean = known.iter().sum::<f32>() / known.len() as f32;
        assert!((mean.abs() - 14.0).abs() < 2.0, "front depth {mean}");
    }

    #[test]
    fn angle_updates_keep_reference_order_and_infer_held_angles() {
        let step = 0.04;
        let mut turn: Vec<f32> = (0..20).map(|f| f as f32 * step).collect();
        let found: Vec<f32> = (0..20).map(|f| if f % 2 == 0 { 0.1 } else { -0.1 }).collect();
        let p = DepthParams { holdout: 4, ..DepthParams::default() };
        for _ in 0..6 {
            refine_turn(&mut turn, &found, step, &p);
            assert_eq!(turn[0], 0.0);
            assert!(turn.windows(2).all(|w| w[1] - w[0] >= 0.099 * step));
            for f in 1..turn.len() - 1 {
                if p.held(f) {
                    assert!((turn[f] - 0.5 * (turn[f - 1] + turn[f + 1])).abs() < 1e-6);
                }
            }
        }
    }

    #[test]
    fn flat_colour_and_vertical_only_variation_have_no_stereo_depth() {
        for vertical in [false, true] {
            let img = image::RgbaImage::from_fn(32, 16, |_, y| {
                image::Rgba([if vertical { (y * 10) as u8 } else { 80 }, 60, 40, 255])
            });
            let frames = vec![img; 3];
            let masks = vec![vec![true; 32 * 16]; 3];
            let p = DepthParams { views: 1, window: 1, holdout: 0, ..DepthParams::default() };
            let depth = frame_depth(&frames, &masks, 1, 15.5, 6.0, &[0.0, 0.1, 0.2], &p);
            assert!(depth.iter().all(|d| d.is_nan()));
        }
    }

    #[test]
    fn stereo_uses_nonuniform_frame_angles() {
        let turn = [0.0, 0.08, 0.4, 0.65, 1.0];
        let frames = turning_box_at(&turn);
        let screen = screen_colour(&frames);
        let masks: Vec<_> = frames.iter().map(|f| key_mask(f, screen, 30)).collect();
        let p = DepthParams { views: 4, window: 1, kept: 0.75, holdout: 0, ..DepthParams::default() };
        let depth = frame_depth(&frames, &masks, 0, 79.5, 35.0, &turn, &p);
        let known: Vec<f32> = (72..88).map(|x| depth[30 * 160 + x]).filter(|d| d.is_finite()).collect();
        assert!(known.len() >= 4, "{} known", known.len());
        let mean = known.iter().sum::<f32>() / known.len() as f32;
        assert!((mean - 14.0).abs() < 2.0, "front depth {mean}");
    }

    #[test]
    fn held_out_frame_cannot_change_training_stereo() {
        let turn = [0.0, 0.1, 0.2, 0.3, 0.4];
        let mut frames = turning_box_at(&turn);
        let screen = screen_colour(&frames);
        let mut masks: Vec<_> = frames.iter().map(|f| key_mask(f, screen, 30)).collect();
        let p = DepthParams { views: 2, window: 1, holdout: 2, ..DepthParams::default() };
        let before = frame_depth(&frames, &masks, 0, 79.5, 35.0, &turn, &p);
        assert!(before.iter().any(|d| d.is_finite()));
        frames[1] = image::RgbaImage::from_pixel(160, 60, image::Rgba([255, 0, 255, 255]));
        masks[1].fill(true);
        let after = frame_depth(&frames, &masks, 0, 79.5, 35.0, &turn, &p);
        assert!(before.iter().zip(&after).all(|(a, b)| a.to_bits() == b.to_bits()));
        assert!(frame_depth(&frames, &masks, 1, 79.5, 35.0, &turn, &p).iter().all(|d| d.is_nan()));
    }

    #[test]
    fn voxel_visibility_keeps_the_front_surface() {
        let shell = [(0.0, -6.0, 2), (0.0, 6.0, 2)];
        let front = front_depth(&shell, (1.0, 0.0), 5.0, 0.0, 11, 5, 1.0);
        assert_eq!(front[11 + 5], 6.5);
        assert!((front[11 + 5] - 6.0).abs() <= 2.0);
        assert!((front[11 + 5] + 6.0).abs() > 2.0);
        assert!(!front[0].is_finite());
        let a = std::f32::consts::FRAC_PI_4;
        let wide = front_depth(&[(0.0, 0.0, 1)], (a.cos(), a.sin()), 4.0, 0.0, 9, 3, 2.0);
        assert_eq!(wide[..9].iter().filter(|z| z.is_finite()).count(), 3);
    }

    #[test]
    fn invalid_grid_parameters_return_an_error() {
        let frames = turning_box(12, 0.3);
        for voxel in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let p = DepthParams { voxel, ..DepthParams::default() };
            assert!(prepare_depth(&frames, &p).is_err());
        }
    }

    #[test]
    fn cross_view_depth_filter_rejects_an_isolated_wrong_surface() {
        let turn: Vec<f32> = (0..5).map(|k| k as f32 * 0.05).collect();
        let maps: Vec<Vec<f32>> = turn.iter().map(|&a| {
            (0..41 * 3).map(|i| (10.0 - ((i % 41) as f32 - 20.0) * a.sin()) / a.cos()).collect()
        }).collect();
        let mut d = DepthMaps { width: 41, height: 3, axis: 20.0, reach: 70.0, maps };
        let p = DepthParams { views: 2, surface: 0.6, holdout: 0, ..DepthParams::default() };
        let good = consistent_depths(&d, &turn, &p);
        assert!((good[2][41 + 20] - d.maps[2][41 + 20]).abs() < 1e-6);
        d.maps[2][41 + 20] = 60.0;
        let checked = consistent_depths(&d, &turn, &p);
        assert!(checked[2][41 + 20].is_nan());
    }

    #[test]
    fn stereo_preserves_solid_box_volume_and_held_out_fit() {
        let frames = turning_box(24, std::f32::consts::PI / 14.0);
        let mut p = DepthParams { hull: true, voxel: 3.0, views: 2, window: 1, kept: 0.75,
            refine: 1, check: vec![0, 4], ..DepthParams::default() };
        let (_, hull, _) = depth_glb(&frames, &p).expect("hull reconstruction");
        p.hull = false;
        let (_, stereo, depths) = depth_glb(&frames, &p).expect("stereo reconstruction");
        assert!(hull.voxels > 100);
        assert!(stereo.voxels * 100 >= hull.voxels * 95,
            "stereo removed solid-box volume: {} / {}", stereo.voxels, hull.voxels);
        assert!(hull.fit_held.iou > 0.9, "{:?}", hull.fit_held);
        assert!(stereo.fit_held.iou > 0.9, "{:?}", stereo.fit_held);
        assert_eq!(stereo.fit_used.frames + stereo.fit_held.frames, frames.len());
        assert_eq!(stereo.fit_held.frames, 3);
        assert_eq!(stereo.checks.len(), 2);
        // the exported surface (its triangles) is measured too: unsmoothed it fits the held-out
        // views; smoothing only takes away (on a box 20 voxels wide nearly every voxel is
        // an edge voxel, so it takes away a lot)
        assert_eq!(hull.fit_mesh_used.frames + hull.fit_mesh_held.frames, frames.len());
        let (_, sharp, _) = depth_glb(&frames, &DepthParams { hull: true, smooth: 0, low: None, ..p.clone() }).expect("unsmoothed");
        eprintln!("exported box surface, held-out views: unsmoothed {:?}; smoothed twice {:?}", sharp.fit_mesh_held, hull.fit_mesh_held);
        assert!(sharp.fit_mesh_held.iou > 0.9, "unsmoothed exported surface {:?}", sharp.fit_mesh_held);
        assert!(hull.fit_mesh_held.missed >= sharp.fit_mesh_held.missed, "smoothing grew the surface");
        assert!(depths.maps[4].iter().all(|d| d.is_nan()));
        eprintln!("solid box: hull {} voxels, stereo {} voxels; held IoU {:.3}",
            hull.voxels, stereo.voxels, stereo.fit_held.iou);

        // the reduced model: tangents, a normal map, both textures 2048 x 2048, and the box's
        // flat faces give a nearly neutral normal map
        let (low, info) = hull.low.expect("reduced model");
        let (doc, _, images) = gltf::import_slice(&low).expect("glTF");
        let prim = doc.meshes().next().and_then(|m| m.primitives().next()).expect("primitive");
        assert!(prim.get(&gltf::Semantic::Tangents).is_some(), "no tangents");
        assert!(prim.material().normal_texture().is_some(), "no normal map");
        assert!(images.iter().all(|i| (i.width, i.height) == (2048, 2048)), "texture sizes");
        assert!(info.normal_detail.0 < 10.0, "median normal detail {:?}", info.normal_detail);
        eprintln!("reduced box: {} vertices, normal detail median {:.1}°, p90 {:.1}°",
            info.vertices, info.normal_detail.0, info.normal_detail.1);
    }
}
