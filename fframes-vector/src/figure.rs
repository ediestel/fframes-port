//! A 3D figure from a sheet of three views (front, side, back) on a plain background, as an
//! image model draws them: the figure's volume is carved from its outlines, its surface is
//! painted with the views.
//!
//! 1. The background is the colour of the corners; everything not connected to the border
//!    through background colour is figure, so figure parts in the background colour stay
//!    figure.
//! 2. The views are the column runs of figure pixels, left to right. Front and back are the
//!    two of about equal width (mirror images of one outline); the third is the side, unless
//!    an order is given.
//! 3. Carving: a voxel is kept when it lies inside every outline (front and back seen along
//!    z, the side along x). Then photo-consistency (space carving): every surface voxel that
//!    two or more views see must show the same colour in them, else it is carved, until all
//!    agree. Ghost parts (a head's width crossed with a lance's depth) and square corners show
//!    different colours to different views and go; outlines are never carved. Views share
//!    their top and bottom, as on a sheet with one scale.
//! 4. The surface of the kept voxels (surface nets), each face textured from the view it faces:
//!    front, back, or the side for both flanks. The sheet itself is the texture.
//!
//! Outlines cannot see hollows that no view shows (between an arm and the body seen only from
//! above); those stay filled.

use serde_json::json;

/// Which view a sheet region is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Front,
    Side,
    Back,
}

/// A view's box on the sheet, in pixels: x, y, width, height.
#[derive(Clone, Copy, Debug)]
pub struct ViewBox {
    pub view: View,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Clone, Debug)]
pub struct FigureParams {
    /// Height of the figure in metres.
    pub height: f32,
    /// Voxels along the height.
    pub resolution: u32,
    /// Colour distance (0..441) that still counts as background.
    pub tolerance: f32,
    /// View order left to right; `None` finds the side as the odd one out.
    pub order: Option<[View; 3]>,
    /// The side view shows the figure facing left (its front towards the image's left).
    pub side_faces_left: bool,
    /// Largest colour distance (0..441, blurred) between views that see the same surface
    /// point; beyond it the point is carved.
    pub consistency: f32,
    /// Most carving passes.
    pub passes: usize,
}

impl Default for FigureParams {
    fn default() -> Self {
        FigureParams {
            height: 1.7,
            resolution: 256,
            tolerance: 30.0,
            order: None,
            side_faces_left: true,
            consistency: 70.0,
            passes: 200,
        }
    }
}

/// The figure mask: true for figure pixels.
#[must_use]
pub fn figure_mask(img: &image::RgbaImage, tolerance: f32) -> (Vec<bool>, u32, u32) {
    let (w, h) = img.dimensions();
    let bg = img.get_pixel(2, 2).0;
    let near = |x: u32, y: u32| {
        let p = img.get_pixel(x, y).0;
        let d: f32 = (0..3)
            .map(|i| (f32::from(p[i]) - f32::from(bg[i])).powi(2))
            .sum::<f32>()
            .sqrt();
        d <= tolerance || p[3] < 128
    };
    // background = background-coloured pixels connected to the border
    let mut background = vec![false; (w * h) as usize];
    let mut stack: Vec<(u32, u32)> = Vec::new();
    for x in 0..w {
        stack.push((x, 0));
        stack.push((x, h - 1));
    }
    for y in 0..h {
        stack.push((0, y));
        stack.push((w - 1, y));
    }
    while let Some((x, y)) = stack.pop() {
        let i = (y * w + x) as usize;
        if background[i] || !near(x, y) {
            continue;
        }
        background[i] = true;
        if x > 0 {
            stack.push((x - 1, y));
        }
        if x + 1 < w {
            stack.push((x + 1, y));
        }
        if y > 0 {
            stack.push((x, y - 1));
        }
        if y + 1 < h {
            stack.push((x, y + 1));
        }
    }
    (background.iter().map(|b| !b).collect(), w, h)
}

/// The views of a sheet: runs of columns with figure pixels (at least 2 % of the width,
/// separated by empty columns), their vertical extent, and which view each is.
pub fn find_views(
    mask: &[bool],
    w: u32,
    h: u32,
    order: Option<[View; 3]>,
) -> Result<Vec<ViewBox>, String> {
    let col = |x: u32| (0..h).filter(|&y| mask[(y * w + x) as usize]).count();
    let mut runs: Vec<(u32, u32)> = Vec::new();
    let mut start: Option<u32> = None;
    for x in 0..=w {
        let filled = x < w && col(x) > 2;
        match (start, filled) {
            (None, true) => start = Some(x),
            (Some(s), false) => {
                if x - s >= w / 50 {
                    runs.push((s, x));
                }
                start = None;
            }
            _ => {}
        }
    }
    if runs.len() != 3 {
        return Err(format!(
            "found {} views, need 3 (front, side, back) side by side with background between them",
            runs.len()
        ));
    }
    let views = match order {
        Some(o) => o,
        None => {
            // front and back mirror one outline: the pair of most equal width
            let wd: Vec<i64> = runs.iter().map(|(a, b)| i64::from(b - a)).collect();
            let odd = (0..3)
                .min_by_key(|&i| {
                    let (j, k) = ((i + 1) % 3, (i + 2) % 3);
                    (wd[j] - wd[k]).abs()
                })
                .unwrap_or(1);
            let mut v = [View::Front; 3];
            v[odd] = View::Side;
            let mut pair = (0..3).filter(|&i| i != odd);
            if let (Some(a), Some(b)) = (pair.next(), pair.next()) {
                v[a] = View::Front;
                v[b] = View::Back;
            }
            v
        }
    };
    Ok(runs
        .iter()
        .zip(views)
        .map(|(&(x0, x1), view)| {
            let rows: Vec<u32> = (0..h)
                .filter(|&y| (x0..x1).any(|x| mask[(y * w + x) as usize]))
                .collect();
            let (y0, y1) = (rows.first().copied().unwrap_or(0), rows.last().copied().unwrap_or(0));
            ViewBox {
                view,
                x: x0,
                y: y0,
                w: x1 - x0,
                h: y1 - y0 + 1,
            }
        })
        .collect())
}

pub(crate) struct Mesh {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

/// Carves the figure and returns its textured mesh as a binary glTF, with the sheet as the
/// texture. Also returns the views found.
pub fn figure_glb(
    sheet_png: &[u8],
    p: &FigureParams,
) -> Result<(Vec<u8>, Vec<ViewBox>), String> {
    let img = image::load_from_memory(sheet_png)
        .map_err(|e| e.to_string())?
        .to_rgba8();
    let (mask, w, h) = figure_mask(&img, p.tolerance);
    let views = find_views(&mask, w, h, p.order)?;
    let get = |v: View| views.iter().find(|b| b.view == v).copied();
    let (front, side) = match (get(View::Front), get(View::Side)) {
        (Some(f), Some(s)) => (f, s),
        _ => return Err("need a front and a side view".into()),
    };
    let back = get(View::Back);

    // the voxel grid: y along the height, x across the front view, z across the side view;
    // one voxel = `px` sheet pixels
    let top = front.y.min(side.y);
    let bottom = (front.y + front.h).max(side.y + side.h);
    let px = (bottom - top) as f32 / p.resolution as f32;
    let nx = (front.w as f32 / px).ceil() as usize + 2;
    let ny = p.resolution as usize + 2;
    let nz = (side.w as f32 / px).ceil() as usize + 2;
    let inside_view = |b: ViewBox, u: f32, y: f32| -> bool {
        // u, y in sheet pixels relative to the view's box
        let (sx, sy) = (b.x as f32 + u, top as f32 + y);
        if sx < b.x as f32 || sx >= (b.x + b.w) as f32 || sy < 0.0 || sy >= h as f32 {
            return false;
        }
        mask[(sy as u32 * w + sx as u32) as usize]
    };
    let side_u = |k: usize| {
        // z index to the side view's column: the figure's front is +z
        let t = (k as f32 - 0.5) * px;
        if p.side_faces_left { t } else { side.w as f32 - t }
    };
    let idx = |i: usize, j: usize, k: usize| (k * ny + j) * nx + i;
    // the side view sees the flank that faces it: the figure's left (+x) when it faces left
    let side_from_plus_x = p.side_faces_left;
    let back_u = |i: usize| back.map(|b| b.w as f32 - (i as f32 - 0.5) * px * b.w as f32 / front.w as f32);

    // 1. exact hull: inside every outline
    let mut solid = vec![false; nx * ny * nz];
    for k in 1..nz - 1 {
        for j in 1..ny - 1 {
            let y = (j as f32 - 0.5) * px;
            if !inside_view(side, side_u(nz - 1 - k), y) {
                continue;
            }
            for i in 1..nx - 1 {
                let in_back = match (back, back_u(i)) {
                    (Some(b), Some(u)) => inside_view(b, u, y),
                    _ => true,
                };
                if in_back && inside_view(front, (i as f32 - 0.5) * px, y) {
                    solid[idx(i, j, k)] = true;
                }
            }
        }
    }

    // 2. photo-consistency: a surface voxel seen by two or more views must show the same
    // colour in them; one that does not cannot be on the figure and is carved, until every
    // seen voxel agrees. Colours are compared blurred over about a voxel, so lead lines and
    // small drift between the views do not count. The last voxel of a column or row is never
    // carved, so the outlines stay exactly as drawn.
    let blurred = image::imageops::blur(&img, (px * 0.75).max(1.0));
    let colour = |b: ViewBox, u: f32, y: f32| -> [f32; 3] {
        let sx = (b.x as f32 + u).clamp(0.0, (w - 1) as f32) as u32;
        let sy = (top as f32 + y).clamp(0.0, (h - 1) as f32) as u32;
        let q = blurred.get_pixel(sx, sy).0;
        [f32::from(q[0]), f32::from(q[1]), f32::from(q[2])]
    };
    for _pass in 0..p.passes {
        let mut carve: Vec<usize> = Vec::new();
        for j in 1..ny - 1 {
            let y = (j as f32 - 0.5) * px;
            // front-most and back-most solid of every column (i, j), side-most of every row
            let mut front_k = vec![usize::MAX; nx];
            let mut back_k = vec![usize::MAX; nx];
            let mut count_ij = vec![0u32; nx];
            let mut side_i = vec![usize::MAX; nz];
            let mut count_jk = vec![0u32; nz];
            for k in 1..nz - 1 {
                for i in 1..nx - 1 {
                    if solid[idx(i, j, k)] {
                        if back_k[i] == usize::MAX {
                            back_k[i] = k;
                        }
                        front_k[i] = k;
                        count_ij[i] += 1;
                        count_jk[k] += 1;
                        let better = side_i[k] == usize::MAX
                            || if side_from_plus_x { i > side_i[k] } else { i < side_i[k] };
                        if better {
                            side_i[k] = i;
                        }
                    }
                }
            }
            for k in 1..nz - 1 {
                for i in 1..nx - 1 {
                    if !solid[idx(i, j, k)] {
                        continue;
                    }
                    let mut seen: Vec<[f32; 3]> = Vec::new();
                    if front_k[i] == k {
                        seen.push(colour(front, (i as f32 - 0.5) * px, y));
                    }
                    if back_k[i] == k
                        && let (Some(b), Some(u)) = (back, back_u(i))
                    {
                        seen.push(colour(b, u, y));
                    }
                    if side_i[k] == i {
                        seen.push(colour(side, side_u(nz - 1 - k), y));
                    }
                    if seen.len() < 2 || count_ij[i] <= 1 || count_jk[k] <= 1 {
                        continue;
                    }
                    let mut worst = 0.0_f32;
                    for a in 0..seen.len() {
                        for b in a + 1..seen.len() {
                            let d = (0..3).map(|c| (seen[a][c] - seen[b][c]).powi(2)).sum::<f32>().sqrt();
                            worst = worst.max(d);
                        }
                    }
                    if worst > p.consistency {
                        carve.push(idx(i, j, k));
                    }
                }
            }
        }
        if carve.is_empty() {
            break;
        }
        for c in carve {
            solid[c] = false;
        }
    }

    // painted from the view each face looks into: +z the front, -z the back, x the side;
    // tops and bottoms by their normal
    let (sw, sh) = (w as f32, h as f32);
    let uv = |n: [f32; 3], axis: usize, _centre: [f32; 3], g: [f32; 3]| -> [f32; 2] {
        let facing_z = match axis {
            2 => true,
            0 => false,
            _ => n[2].abs() >= n[0].abs(),
        };
        let view = if facing_z {
            if n[2] >= 0.0 { front } else { back.unwrap_or(front) }
        } else {
            side
        };
        let y = top as f32 + (g[1] - 0.5) * px;
        let x = match view.view {
            View::Front => view.x as f32 + (g[0] - 0.5) * px,
            View::Back => view.x as f32 + view.w as f32 - (g[0] - 0.5) * px,
            View::Side => {
                view.x as f32 + side_u(nz - 1 - g[2].round() as usize).clamp(0.0, view.w as f32)
            }
        };
        [x / sw, y / sh]
    };
    let mesh = surface_mesh(&solid, [nx, ny, nz], p.height / p.resolution as f32, &uv);
    Ok((write_glb(&mesh, sheet_png), views))
}

#[derive(Clone, Debug)]
pub struct TurntableParams {
    /// Height of the figure in metres.
    pub height: f32,
    /// Voxels along the height.
    pub resolution: u32,
    /// Colour distance (0..441) that still counts as background.
    pub tolerance: f32,
    /// Frames a point must lie inside to stay; `None` is all but one in twelve, so drift and
    /// perspective in a few frames do not cut the figure.
    pub keep: Option<usize>,
    /// The figure turns clockwise seen from above; `None` finds it. Outlines cannot tell (the
    /// other direction gives the same figure mirrored front to back), colours can: the
    /// direction in which surface points show the same colour in the frames that see them.
    pub clockwise: Option<bool>,
}

impl Default for TurntableParams {
    fn default() -> Self {
        TurntableParams {
            height: 1.7,
            resolution: 192,
            tolerance: 30.0,
            keep: None,
            clockwise: None,
        }
    }
}

/// What a turntable carving found.
#[derive(Clone, Debug)]
pub struct Turntable {
    pub frames: usize,
    pub clockwise: bool,
    pub keep: usize,
    /// Kept voxels.
    pub volume: usize,
    /// Colour disagreement of surface points between frames (mean distance, 0..441) with the
    /// direction used and with the other one; `None` when the direction was given.
    pub disagreement: Option<(f32, f32)>,
}

/// A 3D figure from the frames of one full, even turn (a turntable video: frame `f` of `n` is
/// the figure turned by `f / n` of a circle, seen by a still camera, plain background).
///
/// Every frame is masked (background connected to the border), its feet put on one line and
/// its height evened out to the median; the axis is the mean of the frames' centres. A voxel
/// stays where it lies inside the outline in at least `keep` frames. Each face is painted from
/// the frame that looks at it most directly and sees it (no solid voxel between face and
/// camera); the frames, cropped, are the texture.
pub fn turntable_glb(
    frames: &[image::RgbaImage],
    p: &TurntableParams,
) -> Result<(Vec<u8>, Turntable), String> {
    use rayon::prelude::*;
    let n = frames.len();
    if n < 8 {
        return Err(format!("{n} frames; a turn needs at least 8"));
    }
    // masks and boxes: (left, top, right, bottom), inclusive
    let mut masks = Vec::with_capacity(n);
    let mut boxes = Vec::with_capacity(n);
    for (f, img) in frames.iter().enumerate() {
        let (m, w, h) = figure_mask(img, p.tolerance);
        let (mut l, mut t, mut r, mut b) = (w, h, 0, 0);
        for y in 0..h {
            for x in 0..w {
                if m[(y * w + x) as usize] {
                    (l, t, r, b) = (l.min(x), t.min(y), r.max(x), b.max(y));
                }
            }
        }
        if r < l {
            return Err(format!("frame {f}: no figure against the background"));
        }
        masks.push((m, w, h));
        boxes.push((l, t, r, b));
    }
    let mut heights: Vec<f32> = boxes.iter().map(|b| (b.3 - b.1 + 1) as f32).collect();
    heights.sort_by(f32::total_cmp);
    let tall = heights[n / 2];
    let axis = boxes.iter().map(|b| (b.0 + b.2) as f32 / 2.0).sum::<f32>() / n as f32;
    // reference pixels per frame pixel, so every frame has the median height
    let zoom: Vec<f32> = boxes.iter().map(|b| tall / (b.3 - b.1 + 1) as f32).collect();
    let reach = boxes
        .iter()
        .zip(&zoom)
        .map(|(b, z)| (axis - b.0 as f32).max(b.2 as f32 - axis) * z)
        .fold(0.0, f32::max);
    let px = tall / p.resolution as f32;
    let nh = (reach / px).ceil() as usize + 2;
    let (nx, ny, nz) = (2 * nh + 1, p.resolution as usize + 2, 2 * nh + 1);
    let idx = |i: usize, j: usize, k: usize| (k * ny + j) * nx + i;
    // a point (x, z) in reference pixels from the axis, `v` reference pixels above the feet,
    // in frame f: inside the figure?
    let inside = |f: usize, u: f32, v: f32| -> bool {
        let (m, w, h) = &masks[f];
        let sx = axis + u / zoom[f];
        let sy = boxes[f].3 as f32 - v / zoom[f];
        if sx < 0.0 || sy < 0.0 || sx >= *w as f32 || sy >= *h as f32 {
            return false;
        }
        m[(sy as u32 * w + sx as u32) as usize]
    };
    let angles = |clockwise: bool| -> Vec<(f32, f32)> {
        (0..n)
            .map(|f| {
                let a = std::f32::consts::TAU * f as f32 / n as f32 * if clockwise { 1.0 } else { -1.0 };
                (a.cos(), a.sin())
            })
            .collect()
    };
    let keep = p.keep.unwrap_or(n - n.div_ceil(12)).clamp(1, n);
    let carve = |clockwise: bool| -> Vec<bool> {
        let rot = angles(clockwise);
        let mut solid = vec![false; nx * ny * nz];
        let rows: Vec<Vec<(usize, usize)>> = (1..ny - 1)
            .into_par_iter()
            .map(|j| {
                let v = tall - (j as f32 - 0.5) * px;
                let mut out = Vec::new();
                for k in 1..nz - 1 {
                    let z = (k as f32 - nh as f32) * px;
                    for i in 1..nx - 1 {
                        let x = (i as f32 - nh as f32) * px;
                        if x * x + z * z > reach * reach {
                            continue;
                        }
                        // u in frame f: the point turned by the frame's angle
                        let mut votes = 0;
                        for (f, &(c, s)) in rot.iter().enumerate() {
                            if inside(f, x * c + z * s, v) {
                                votes += 1;
                            } else if f + 1 - votes > n - keep {
                                break;
                            }
                        }
                        if votes >= keep {
                            out.push((i, k));
                        }
                    }
                }
                out
            })
            .collect();
        for (r, cells) in rows.iter().enumerate() {
            for &(i, k) in cells {
                solid[idx(i, r + 1, k)] = true;
            }
        }
        solid
    };
    // how much the frames disagree on the colours of surface points, for one direction
    let disagreement = |clockwise: bool, solid: &[bool]| -> f32 {
        let rot = angles(clockwise);
        let mut total = 0.0_f64;
        let mut count = 0usize;
        for j in (2..ny - 2).step_by(3) {
            for k in (2..nz - 2).step_by(2) {
                for i in (2..nx - 2).step_by(2) {
                    if !solid[idx(i, j, k)] {
                        continue;
                    }
                    // outward: towards the empty neighbours
                    let (mut ox, mut oz) = (0.0_f32, 0.0_f32);
                    for (di, dk) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                        if !solid[idx((i as i32 + di) as usize, j, (k as i32 + dk) as usize)] {
                            ox += di as f32;
                            oz += dk as f32;
                        }
                    }
                    let l = (ox * ox + oz * oz).sqrt();
                    if l == 0.0 {
                        continue;
                    }
                    let (x, z) = ((i as f32 - nh as f32) * px, (k as f32 - nh as f32) * px);
                    let v = tall - (j as f32 - 0.5) * px;
                    let mut seen: Vec<[f32; 3]> = Vec::new();
                    for (f, &(c, sn)) in rot.iter().enumerate() {
                        // facing this frame's camera closely
                        if (ox * -sn + oz * c) / l < 0.85 {
                            continue;
                        }
                        let sx = axis + (x * c + z * sn) / zoom[f];
                        let sy = boxes[f].3 as f32 - v / zoom[f];
                        let img = &frames[f];
                        if sx >= 0.0 && sy >= 0.0 && (sx as u32) < img.width() && (sy as u32) < img.height() {
                            let q = img.get_pixel(sx as u32, sy as u32).0;
                            seen.push([f32::from(q[0]), f32::from(q[1]), f32::from(q[2])]);
                        }
                    }
                    if seen.len() < 2 {
                        continue;
                    }
                    let m: [f32; 3] = std::array::from_fn(|c| seen.iter().map(|s| s[c]).sum::<f32>() / seen.len() as f32);
                    for s in &seen {
                        total += f64::from((0..3).map(|c| (s[c] - m[c]).powi(2)).sum::<f32>().sqrt());
                        count += 1;
                    }
                }
            }
        }
        if count == 0 { 0.0 } else { (total / count as f64) as f32 }
    };
    let (clockwise, solid, disagree) = match p.clockwise {
        Some(c) => (c, carve(c), None),
        None => {
            let (a, b) = (carve(true), carve(false));
            let (da, db) = (disagreement(true, &a), disagreement(false, &b));
            if da <= db { (true, a, Some((da, db))) } else { (false, b, Some((db, da))) }
        }
    };
    let volume = solid.iter().filter(|&&x| x).count();

    // the texture: every frame cropped to the frames' common box, in a grid of tiles
    let (l0, t0) = (boxes.iter().map(|b| b.0).min().unwrap_or(0), boxes.iter().map(|b| b.1).min().unwrap_or(0));
    let (r0, b0) = (boxes.iter().map(|b| b.2).max().unwrap_or(0), boxes.iter().map(|b| b.3).max().unwrap_or(0));
    let (tw, th) = (r0 - l0 + 1, b0 - t0 + 1);
    let cols = (n as f32).sqrt().ceil() as u32;
    let rows_n = (n as u32).div_ceil(cols);
    let mut atlas = image::RgbaImage::new(tw * cols, th * rows_n);
    for (f, img) in frames.iter().enumerate() {
        let tile = image::imageops::crop_imm(img, l0, t0, tw, th).to_image();
        image::imageops::overlay(&mut atlas, &tile, i64::from(f as u32 % cols * tw), i64::from(f as u32 / cols * th));
    }
    let mut png = Vec::new();
    atlas
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;

    let rot = angles(clockwise);
    // the direction from the figure to the camera of frame f, in figure coordinates
    let to_camera: Vec<[f32; 2]> = rot.iter().map(|&(c, s)| [-s, c]).collect();
    let solid_at = |x: f32, y: f32, z: f32| -> bool {
        let (i, j, k) = (x.round(), y.round(), z.round());
        if i < 0.0 || j < 0.0 || k < 0.0 {
            return false;
        }
        let (i, j, k) = (i as usize, j as usize, k as usize);
        i < nx && j < ny && k < nz && solid[idx(i, j, k)]
    };
    let sees = |f: usize, g: [f32; 3]| -> bool {
        // walk from just outside the face towards the camera
        let [dx, dz] = to_camera[f];
        (2..(nx.max(nz) as i32)).all(|t| !solid_at(g[0] + dx * t as f32, g[1], g[2] + dz * t as f32))
    };
    let (aw, ah) = (atlas.width() as f32, atlas.height() as f32);
    let uv = |nrm: [f32; 3], _axis: usize, centre: [f32; 3], g: [f32; 3]| -> [f32; 2] {
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| {
            let da = nrm[0] * to_camera[a][0] + nrm[2] * to_camera[a][1];
            let db = nrm[0] * to_camera[b][0] + nrm[2] * to_camera[b][1];
            db.total_cmp(&da)
        });
        let f = order.iter().take(6).copied().find(|&f| sees(f, centre)).unwrap_or(order[0]);
        let (c, s) = rot[f];
        let (x, z) = ((g[0] - nh as f32) * px, (g[2] - nh as f32) * px);
        let v = tall - (g[1] - 0.5) * px;
        let sx = axis + (x * c + z * s) / zoom[f] - l0 as f32;
        let sy = boxes[f].3 as f32 - v / zoom[f] - t0 as f32;
        let (tx, ty) = ((f as u32 % cols * tw) as f32, (f as u32 / cols * th) as f32);
        [
            (tx + sx.clamp(0.0, tw as f32 - 1.0)) / aw,
            (ty + sy.clamp(0.0, th as f32 - 1.0)) / ah,
        ]
    };
    let mesh = surface_mesh(&solid, [nx, ny, nz], p.height / p.resolution as f32, &uv);
    Ok((
        write_glb(&mesh, &png),
        Turntable {
            frames: n,
            clockwise,
            keep,
            volume,
            disagreement: disagree,
        },
    ))
}

/// The surface of the solid voxels of an `[nx, ny, nz]` grid (y running down) as a textured
/// mesh: surface nets, smoothed twice, smooth vertex normals, feet at y = 0 and centred, `scale`
/// metres per voxel. `uv` maps a face's outward normal (world), the axis it faces along, the
/// face's centre and a corner (grid points) to texture coordinates; it should choose the
/// picture by the centre, so all corners of a face come from the same picture.
pub(crate) fn surface_mesh(
    solid: &[bool],
    [nx, ny, nz]: [usize; 3],
    scale: f32,
    uv: &dyn Fn([f32; 3], usize, [f32; 3], [f32; 3]) -> [f32; 2],
) -> Mesh {
    let idx = |i: usize, j: usize, k: usize| (k * ny + j) * nx + i;
    // a vertex in every cell where the solid changes, quads across every boundary face
    let mut cell_vertex = vec![u32::MAX; nx * ny * nz];
    let mut verts: Vec<[f32; 3]> = Vec::new();
    let edges = [
        ((0, 0, 0), (1, 0, 0)),
        ((0, 1, 0), (1, 1, 0)),
        ((0, 0, 1), (1, 0, 1)),
        ((0, 1, 1), (1, 1, 1)),
        ((0, 0, 0), (0, 1, 0)),
        ((1, 0, 0), (1, 1, 0)),
        ((0, 0, 1), (0, 1, 1)),
        ((1, 0, 1), (1, 1, 1)),
        ((0, 0, 0), (0, 0, 1)),
        ((1, 0, 0), (1, 0, 1)),
        ((0, 1, 0), (0, 1, 1)),
        ((1, 1, 0), (1, 1, 1)),
    ];
    for k in 0..nz - 1 {
        for j in 0..ny - 1 {
            for i in 0..nx - 1 {
                let corner = |a: usize, b: usize, c: usize| solid[idx(i + a, j + b, k + c)];
                let mut sum = [0.0_f32; 3];
                let mut n = 0.0;
                for ((a0, b0, c0), (a1, b1, c1)) in edges {
                    if corner(a0, b0, c0) != corner(a1, b1, c1) {
                        sum[0] += (a0 + a1) as f32 / 2.0;
                        sum[1] += (b0 + b1) as f32 / 2.0;
                        sum[2] += (c0 + c1) as f32 / 2.0;
                        n += 1.0;
                    }
                }
                if n > 0.0 {
                    cell_vertex[idx(i, j, k)] = verts.len() as u32;
                    verts.push([i as f32 + sum[0] / n, j as f32 + sum[1] / n, k as f32 + sum[2] / n]);
                }
            }
        }
    }
    let faces = collect_quads(solid, &cell_vertex, nx, ny, nz, idx);
    let quads: Vec<[u32; 4]> = faces.iter().map(|f| f.0).collect();
    let verts = smooth(&verts, &quads, 2);

    let (cx, cz) = (nx as f32 / 2.0, nz as f32 / 2.0);
    let world = |v: [f32; 3]| [(v[0] - cx) * scale, (ny as f32 - 1.0 - v[1]) * scale, (v[2] - cz) * scale];
    // outward normal of a face: from the side its solid voxel is on (the grid's y runs down)
    let oriented = |q: &[u32; 4], axis: usize, positive: bool| {
        let pts: Vec<[f32; 3]> = q.iter().map(|&v| world(verts[v as usize])).collect();
        let mut out = [0.0_f32; 3];
        out[axis] = if positive { 1.0 } else { -1.0 };
        if axis == 1 {
            out[1] = -out[1];
        }
        let n = normal(&pts);
        if n[0] * out[0] + n[1] * out[1] + n[2] * out[2] < 0.0 { [-n[0], -n[1], -n[2]] } else { n }
    };
    let mut vertex_normal = vec![[0.0_f32; 3]; verts.len()];
    for (q, axis, positive) in &faces {
        let n = oriented(q, *axis, *positive);
        for &v in q {
            for c in 0..3 {
                vertex_normal[v as usize][c] += n[c];
            }
        }
    }
    for n in &mut vertex_normal {
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
        *n = [n[0] / l, n[1] / l, n[2] / l];
    }
    let mut mesh = Mesh {
        positions: Vec::new(),
        normals: Vec::new(),
        uvs: Vec::new(),
        indices: Vec::new(),
    };
    for (q, axis, positive) in &faces {
        let n = oriented(q, *axis, *positive);
        let pts: Vec<[f32; 3]> = q.iter().map(|&v| world(verts[v as usize])).collect();
        let q = if n_dot_winding(&pts, n) { *q } else { [q[3], q[2], q[1], q[0]] };
        let base = mesh.positions.len() as u32;
        let centre: [f32; 3] = std::array::from_fn(|c| q.iter().map(|&v| verts[v as usize][c]).sum::<f32>() / 4.0);
        for &v in &q {
            let g = verts[v as usize];
            mesh.positions.push(world(g));
            mesh.normals.push(vertex_normal[v as usize]);
            mesh.uvs.push(uv(n, *axis, centre, g));
        }
        mesh.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    mesh
}

/// Whether the winding of `pts` (counter-clockwise seen from outside) agrees with `n`.
fn n_dot_winding(pts: &[[f32; 3]], n: [f32; 3]) -> bool {
    let w = normal(pts);
    w[0] * n[0] + w[1] * n[1] + w[2] * n[2] >= 0.0
}

/// Boundary quads between solid and empty voxels, with the axis they face along and
/// whether the solid side is the lower one (the face points to +axis).
fn collect_quads(
    solid: &[bool],
    cell_vertex: &[u32],
    nx: usize,
    ny: usize,
    nz: usize,
    idx: impl Fn(usize, usize, usize) -> usize,
) -> Vec<([u32; 4], usize, bool)> {
    let mut quads = Vec::new();
    for k in 1..nz - 1 {
        for j in 1..ny - 1 {
            for i in 1..nx - 1 {
                let here = solid[idx(i, j, k)];
                // faces towards +x, +y, +z; the four cells around the shared edge
                for axis in 0..3 {
                    let (a, b, c) = match axis {
                        0 => (i + 1, j, k),
                        1 => (i, j + 1, k),
                        _ => (i, j, k + 1),
                    };
                    if a >= nx || b >= ny || c >= nz || here == solid[idx(a, b, c)] {
                        continue;
                    }
                    let cells = match axis {
                        0 => [(i, j - 1, k - 1), (i, j, k - 1), (i, j, k), (i, j - 1, k)],
                        1 => [(i - 1, j, k - 1), (i - 1, j, k), (i, j, k), (i, j, k - 1)],
                        _ => [(i - 1, j - 1, k), (i, j - 1, k), (i, j, k), (i - 1, j, k)],
                    };
                    let v: Vec<u32> = cells.iter().map(|&(x, y, z)| cell_vertex[idx(x, y, z)]).collect();
                    if v.contains(&u32::MAX) {
                        continue;
                    }
                    quads.push(([v[0], v[1], v[2], v[3]], axis, here));
                }
            }
        }
    }
    quads
}

/// Moves every vertex halfway to the average of its quad neighbours, `passes` times.
fn smooth(verts: &[[f32; 3]], quads: &[[u32; 4]], passes: usize) -> Vec<[f32; 3]> {
    let mut v = verts.to_vec();
    for _ in 0..passes {
        let mut sum = vec![[0.0_f32; 3]; v.len()];
        let mut n = vec![0.0_f32; v.len()];
        for q in quads {
            for e in 0..4 {
                let (a, b) = (q[e] as usize, q[(e + 1) % 4] as usize);
                for c in 0..3 {
                    sum[a][c] += v[b][c];
                    sum[b][c] += v[a][c];
                }
                n[a] += 1.0;
                n[b] += 1.0;
            }
        }
        for (i, p) in v.iter_mut().enumerate() {
            if n[i] > 0.0 {
                for c in 0..3 {
                    p[c] = 0.5 * p[c] + 0.5 * sum[i][c] / n[i];
                }
            }
        }
    }
    v
}

fn normal(p: &[[f32; 3]]) -> [f32; 3] {
    let d1 = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
    let d2 = [p[3][0] - p[1][0], p[3][1] - p[1][1], p[3][2] - p[1][2]];
    let c = [
        d1[1] * d2[2] - d1[2] * d2[1],
        d1[2] * d2[0] - d1[0] * d2[2],
        d1[0] * d2[1] - d1[1] * d2[0],
    ];
    let l = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt().max(1e-9);
    [c[0] / l, c[1] / l, c[2] / l]
}

/// A binary glTF with one textured mesh; the PNG is embedded as its texture.
pub(crate) fn write_glb(m: &Mesh, png: &[u8]) -> Vec<u8> {
    let mut bin: Vec<u8> = Vec::new();
    let mut views = Vec::new();
    let mut push = |bin: &mut Vec<u8>, bytes: &[u8], target: Option<u32>| {
        while bin.len() % 4 != 0 {
            bin.push(0);
        }
        let offset = bin.len();
        bin.extend_from_slice(bytes);
        let mut v = json!({"buffer": 0, "byteOffset": offset, "byteLength": bytes.len()});
        if let Some(t) = target {
            v["target"] = json!(t);
        }
        views.push(v);
        views.len() - 1
    };
    let f32s = |v: &[[f32; 3]]| v.iter().flatten().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>();
    let pos = push(&mut bin, &f32s(&m.positions), Some(34962));
    let nrm = push(&mut bin, &f32s(&m.normals), Some(34962));
    let uv_bytes: Vec<u8> = m.uvs.iter().flatten().flat_map(|f| f.to_le_bytes()).collect();
    let uv = push(&mut bin, &uv_bytes, Some(34962));
    let idx_bytes: Vec<u8> = m.indices.iter().flat_map(|i| i.to_le_bytes()).collect();
    let ind = push(&mut bin, &idx_bytes, Some(34963));
    let img = push(&mut bin, png, None);
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in &m.positions {
        for c in 0..3 {
            lo[c] = lo[c].min(p[c]);
            hi[c] = hi[c].max(p[c]);
        }
    }
    let n = m.positions.len();
    let doc = json!({
        "asset": {"version": "2.0", "generator": "fframes-vector figure"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"name": "figure", "mesh": 0}],
        "meshes": [{"name": "figure", "primitives": [{
            "attributes": {"POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2},
            "indices": 3, "material": 0
        }]}],
        "materials": [{"name": "painted", "pbrMetallicRoughness": {
            "baseColorTexture": {"index": 0}, "metallicFactor": 0.0, "roughnessFactor": 0.8
        }, "doubleSided": true}],
        "textures": [{"source": 0, "sampler": 0}],
        "samplers": [{"magFilter": 9729, "minFilter": 9987, "wrapS": 33071, "wrapT": 33071}],
        "images": [{"bufferView": img, "mimeType": "image/png"}],
        "accessors": [
            {"bufferView": pos, "componentType": 5126, "count": n, "type": "VEC3", "min": lo, "max": hi},
            {"bufferView": nrm, "componentType": 5126, "count": n, "type": "VEC3"},
            {"bufferView": uv, "componentType": 5126, "count": n, "type": "VEC2"},
            {"bufferView": ind, "componentType": 5125, "count": m.indices.len(), "type": "SCALAR"}
        ],
        "bufferViews": views,
        "buffers": [{"byteLength": bin.len()}]
    });
    let mut json = serde_json::to_vec(&doc).unwrap_or_default();
    while json.len() % 4 != 0 {
        json.push(b' ');
    }
    let total = 12 + 8 + json.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&bin);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sheet with three views of a box-shaped figure: front 40 wide, side 20, back 40.
    fn sheet() -> Vec<u8> {
        let mut img = image::RgbaImage::from_pixel(200, 100, image::Rgba([200, 200, 200, 255]));
        for (x0, wd) in [(10u32, 40u32), (80, 20), (140, 40)] {
            for y in 10..90 {
                for x in x0..x0 + wd {
                    img.put_pixel(x, y, image::Rgba([180, 40, 30, 255]));
                }
            }
        }
        let mut png = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        png
    }

    /// A head (red) with a lance (yellow) beside it, in front, side and back view.
    fn head_and_lance() -> Vec<u8> {
        let grey = image::Rgba([200, 200, 200, 255]);
        let red = image::Rgba([200, 40, 30, 255]);
        let yellow = image::Rgba([230, 200, 40, 255]);
        let mut img = image::RgbaImage::from_pixel(300, 100, grey);
        let mut fill = |x0: u32, x1: u32, y0: u32, c: image::Rgba<u8>| {
            for y in y0..90 {
                for x in x0..x1 {
                    img.put_pixel(x, y, c);
                }
            }
        };
        // a hand on top joins head and lance in every view (rows 10..14)
        for (x0, x1) in [(10, 56), (110, 156), (200, 246)] {
            fill(x0, x1, 10, red);
        }
        let mut fill = |x0: u32, x1: u32, c: image::Rgba<u8>| fill(x0, x1, 14, c);
        // front: head 10..40, lance 50..56 (to the figure's left, image right)
        fill(10, 40, red);
        fill(50, 56, yellow);
        // side, facing left: head 110..140 (front at the left), lance 150..156 behind it
        fill(110, 140, red);
        fill(150, 156, yellow);
        // back: the front mirrored inside its own box 200..246
        fill(216, 246, red);
        fill(200, 206, yellow);
        let mut png = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        png
    }

    #[test]
    fn colours_carve_the_visible_ghost_of_crossed_parts() {
        let png = head_and_lance();
        let p = FigureParams {
            height: 1.0,
            resolution: 80,
            order: Some([View::Front, View::Side, View::Back]),
            ..FigureParams::default()
        };
        let (glb, _) = figure_glb(&png, &p).unwrap();
        let (doc, buffers, _) = gltf::import_slice(&glb).unwrap();
        let prim = doc.meshes().next().unwrap().primitives().next().unwrap();
        let reader = prim.reader(|b| Some(&buffers[b.index()]));
        let pts: Vec<[f32; 3]> = reader.read_positions().unwrap().collect();
        let b = prim.bounding_box();
        // grid fractions: across, head 0..0.65 and lance 0.87..1; in depth, head at the front
        // (high z) and the lance behind (low z); a ghost is head-width at lance-depth or
        // lance-width at head-depth
        let fx = |x: f32| (x - b.min[0]) / (b.max[0] - b.min[0]);
        let fz = |z: f32| (z - b.min[2]) / (b.max[2] - b.min[2]);
        let fy = |y: f32| (y - b.min[1]) / (b.max[1] - b.min[1]);
        // the lance's width at the head's depth is seen by front and side in different
        // colours and carved; the head's width at the lance's depth is hidden from every
        // view (head in front, lance at the side, head colour behind) and stays: carving
        // part by part is needed for it
        let lance_in_front = pts
            .iter()
            .filter(|q| fy(q[1]) < 0.85 && fx(q[0]) > 0.9 && fz(q[2]) > 0.5)
            .count();
        assert_eq!(lance_in_front, 0, "the visible ghost is carved");
        assert!(!pts.is_empty());
    }

    #[test]
    fn a_turning_elliptic_column_comes_out_round() {
        // 36 frames of an elliptic column (semi-axes 30 px across, 15 px deep at the start)
        // turning once: its outline is as wide as the ellipse seen from each angle
        let frames: Vec<image::RgbaImage> = (0..36)
            .map(|f| {
                let a = std::f32::consts::TAU * f as f32 / 36.0;
                let half = (30.0_f32.powi(2) * a.cos().powi(2) + 15.0_f32.powi(2) * a.sin().powi(2)).sqrt();
                let mut img = image::RgbaImage::from_pixel(120, 100, image::Rgba([200, 200, 200, 255]));
                for y in 10..90 {
                    for x in 0..120 {
                        if (x as f32 + 0.5 - 60.0).abs() < half {
                            img.put_pixel(x, y, image::Rgba([180, 40, 30, 255]));
                        }
                    }
                }
                img
            })
            .collect();
        let (glb, t) = turntable_glb(
            &frames,
            &TurntableParams { height: 1.0, resolution: 80, ..TurntableParams::default() },
        )
        .unwrap();
        assert_eq!(t.frames, 36);
        let (doc, buffers, _) = gltf::import_slice(&glb).unwrap();
        let prim = doc.meshes().next().unwrap().primitives().next().unwrap();
        let b = prim.bounding_box();
        let (wd, dp) = (b.max[0] - b.min[0], b.max[2] - b.min[2]);
        // 80 px high = 1 m: 60 x 30 px = 0.75 x 0.375 m
        assert!((wd - 0.75).abs() < 0.06, "width {wd}");
        assert!((dp - 0.375).abs() < 0.06, "depth {dp}");
        let reader = prim.reader(|bf| Some(&buffers[bf.index()]));
        let corner = reader.read_positions().unwrap().any(|q| {
            (q[0] / (wd / 2.0)).abs() > 0.8 && (q[2] / (dp / 2.0)).abs() > 0.8
        });
        assert!(!corner, "the column has square corners");
    }

    #[test]
    fn views_are_found_and_the_side_is_the_odd_one() {
        let png = sheet();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let (mask, w, h) = figure_mask(&img, 30.0);
        let v = find_views(&mask, w, h, None).unwrap();
        let kinds: Vec<View> = v.iter().map(|b| b.view).collect();
        assert_eq!(kinds, [View::Front, View::Side, View::Back]);
        assert_eq!((v[1].x, v[1].w, v[1].y, v[1].h), (80, 20, 10, 80));
    }

    #[test]
    fn a_box_figure_has_the_height_and_proportions_of_its_views() {
        let (glb, _) = figure_glb(
            &sheet(),
            &FigureParams {
                height: 2.0,
                resolution: 64,
                ..FigureParams::default()
            },
        )
        .unwrap();
        let (doc, _, _) = gltf::import_slice(&glb).unwrap();
        let mesh = doc.meshes().next().unwrap();
        let prim = mesh.primitives().next().unwrap();
        let b = prim.bounding_box();
        let (wd, ht, dp) = (b.max[0] - b.min[0], b.max[1] - b.min[1], b.max[2] - b.min[2]);
        assert!((ht - 2.0).abs() < 0.1, "height {ht}");
        // front 40 px wide, side 20 px, height 80 px: width 1.0 m, depth 0.5 m
        assert!((wd - 1.0).abs() < 0.1, "width {wd}");
        assert!((dp - 0.5).abs() < 0.1, "depth {dp}");
        assert!(b.min[1].abs() < 0.05, "feet at y = 0, got {}", b.min[1]);
    }
}
