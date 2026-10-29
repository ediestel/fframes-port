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
//! 3. Carving: a voxel is kept when it lies inside the front outline (seen along z) and inside
//!    the side outline (seen along x). Views share their top and bottom, as on a sheet with
//!    one scale.
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
}

impl Default for FigureParams {
    fn default() -> Self {
        FigureParams {
            height: 1.7,
            resolution: 256,
            tolerance: 30.0,
            order: None,
            side_faces_left: true,
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

struct Mesh {
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
    let mut solid = vec![false; nx * ny * nz];
    let idx = |i: usize, j: usize, k: usize| (k * ny + j) * nx + i;
    // a row of a view as runs of figure: (from, to) in grid units
    let runs = |b: ViewBox, y: f32, n: usize, at: &dyn Fn(usize) -> f32| -> Vec<(f32, f32)> {
        let mut out = Vec::new();
        let mut open: Option<usize> = None;
        for c in 1..=n {
            let on = c < n - 1 && inside_view(b, at(c), y);
            match (open, on) {
                (None, true) => open = Some(c),
                (Some(a), false) => {
                    out.push((a as f32 - 0.5, c as f32 - 0.5));
                    open = None;
                }
                _ => {}
            }
        }
        out
    };
    // pairs of the row above (front run, side run), for continuity
    let mut above: Vec<((f32, f32), (f32, f32))> = Vec::new();
    for j in 1..ny - 1 {
        let y = (j as f32 - 0.5) * px;
        // every slice is round, not square: ellipses fitted to pairs of a front run (width)
        // and a side run (depth). Not every pair is a part: the head in front and the lance
        // beside it are seen from the side as head and lance too, and crossing them would
        // make ghosts. A run pairs with the run of the other view closest to it in size;
        // where several share one (two arms in front, one body from the side), the smaller
        // ones are round limbs inside it.
        let across = runs(front, y, nx, &|i| (i as f32 - 0.5) * px);
        let deep = runs(side, y, nz, &|k| side_u(nz - 1 - k));
        let (ellipses, pairs) = pair_runs(&across, &deep, &above);
        above = pairs;
        for (cx, rx, cz, rz) in ellipses {
            {
                for k in 1..nz - 1 {
                    let dz = (k as f32 - cz) / rz.max(0.5);
                    if dz.abs() > 1.0 {
                        continue;
                    }
                    for i in 1..nx - 1 {
                        let dx = (i as f32 - cx) / rx.max(0.5);
                        if dx * dx + dz * dz <= 1.0 {
                            solid[idx(i, j, k)] = true;
                        }
                    }
                }
            }
        }
    }

    // surface nets: a vertex in every cell where the solid changes, quads across every
    // boundary face between a solid and an empty voxel
    let mut cell_vertex = vec![u32::MAX; nx * ny * nz];
    let mut verts: Vec<[f32; 3]> = Vec::new();
    for k in 0..nz - 1 {
        for j in 0..ny - 1 {
            for i in 0..nx - 1 {
                let mut sum = [0.0_f32; 3];
                let mut n = 0.0;
                let corner = |a: usize, b: usize, c: usize| solid[idx(i + a, j + b, k + c)];
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
                    verts.push([
                        i as f32 + sum[0] / n,
                        j as f32 + sum[1] / n,
                        k as f32 + sum[2] / n,
                    ]);
                }
            }
        }
    }
    // one smoothing pass towards the neighbours' average keeps the outline, softens steps
    let faces = collect_quads(&solid, &cell_vertex, nx, ny, nz, idx);
    let quads: Vec<[u32; 4]> = faces.iter().map(|f| f.0).collect();
    let verts = smooth(&verts, &quads, 2);

    // positions in metres: feet at y = 0, centred; y of the grid runs down the sheet
    let s = p.height / (p.resolution as f32);
    let (cx, cz) = (nx as f32 / 2.0, nz as f32 / 2.0);
    let world = |v: [f32; 3]| [(v[0] - cx) * s, (ny as f32 - 1.0 - v[1]) * s, (v[2] - cz) * s];
    let mut mesh = Mesh {
        positions: Vec::new(),
        normals: Vec::new(),
        uvs: Vec::new(),
        indices: Vec::new(),
    };
    let (sw, sh) = (w as f32, h as f32);
    // smooth shading: every vertex the average normal of its faces, oriented outwards
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
    for (q, axis, positive) in &faces {
        let pts: Vec<[f32; 3]> = q.iter().map(|&v| world(verts[v as usize])).collect();
        // outward in world space: the grid's y runs down the sheet, the world's up
        let mut out = [0.0_f32; 3];
        out[*axis] = if *positive { 1.0 } else { -1.0 };
        if *axis == 1 {
            out[1] = -out[1];
        }
        let mut n = normal(&pts);
        if n[0] * out[0] + n[1] * out[1] + n[2] * out[2] < 0.0 {
            n = [-n[0], -n[1], -n[2]];
        }
        // the view this face looks into: +z the front, -z the back, x the side; tops and
        // bottoms by their smoothed normal
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
        let q = if n_dot_winding(&pts, n) { *q } else { [q[3], q[2], q[1], q[0]] };
        let pts: Vec<[f32; 3]> = q.iter().map(|&v| world(verts[v as usize])).collect();
        let base = mesh.positions.len() as u32;
        for (&v, pt) in q.iter().zip(&pts) {
            let g = verts[v as usize];
            let y = top as f32 + (g[1] - 0.5) * px;
            let x = match view.view {
                View::Front => view.x as f32 + (g[0] - 0.5) * px,
                View::Back => view.x as f32 + view.w as f32 - (g[0] - 0.5) * px,
                View::Side => view.x as f32 + side_u(nz - 1 - g[2].round() as usize).clamp(0.0, view.w as f32),
            };
            mesh.positions.push(*pt);
            mesh.normals.push(vertex_normal[v as usize]);
            mesh.uvs.push([x / sw, y / sh]);
        }
        mesh.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    Ok((write_glb(&mesh, sheet_png), views))
}

/// Whether the winding of `pts` (counter-clockwise seen from outside) agrees with `n`.
fn n_dot_winding(pts: &[[f32; 3]], n: [f32; 3]) -> bool {
    let w = normal(pts);
    w[0] * n[0] + w[1] * n[1] + w[2] * n[2] >= 0.0
}

/// Boundary quads between solid and empty voxels, with the axis they face along and
/// whether the solid side is the lower one (the face points to +axis).
/// Pairs the runs of one row of the front view (`across`) with those of the side view
/// (`deep`) and returns the ellipses (centre and radius across, centre and radius in depth)
/// and the pairs, to pass as `above` to the next row down.
///
/// A pair costs the size mismatch of its runs (width against depth), and more when it does
/// not continue a pair of the row above (both runs overlapping that pair's runs): parts
/// continue from row to row, a horse's head from its neck, legs from the hips. With as many
/// runs in both views, every run is used once, at the least total cost (two legs side by side
/// in front and one behind the other from the side are two legs, not four). Otherwise each run
/// takes its cheapest partner, and a run that is not its partner's own choice is a limb
/// within it: as deep as it is wide (or as wide as deep), centred on the partner.
#[allow(clippy::type_complexity)]
fn pair_runs(
    across: &[(f32, f32)],
    deep: &[(f32, f32)],
    above: &[((f32, f32), (f32, f32))],
) -> (Vec<(f32, f32, f32, f32)>, Vec<((f32, f32), (f32, f32))>) {
    if across.is_empty() || deep.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let size = |r: &(f32, f32)| (r.1 - r.0).max(0.5);
    let overlaps = |a: &(f32, f32), b: &(f32, f32)| a.0 < b.1 && b.0 < a.1;
    let cost = |f: usize, s: usize| {
        let mismatch = (size(&across[f]) / size(&deep[s])).ln().abs();
        let continues = above.is_empty()
            || above
                .iter()
                .any(|(a, d)| overlaps(&across[f], a) && overlaps(&deep[s], d));
        mismatch + if continues { 0.0 } else { 3.0 }
    };
    let mut pairs: Vec<(usize, usize, bool)> = Vec::new(); // (front, side, a limb in its partner)
    if across.len() == deep.len() && across.len() <= 6 {
        // every run once: the cheapest permutation
        let n = across.len();
        let mut perm: Vec<usize> = (0..n).collect();
        let mut best = (f32::MAX, perm.clone());
        permutations(&mut perm, 0, &mut |p| {
            let c: f32 = p.iter().enumerate().map(|(f, &s)| cost(f, s)).sum();
            if c < best.0 {
                best = (c, p.to_vec());
            }
        });
        pairs.extend(best.1.iter().enumerate().map(|(f, &s)| (f, s, false)));
    } else {
        let cheapest = |f: usize| (0..deep.len()).min_by(|&a, &b| cost(f, a).total_cmp(&cost(f, b))).unwrap_or(0);
        let cheapest_front = |s: usize| (0..across.len()).min_by(|&a, &b| cost(a, s).total_cmp(&cost(b, s))).unwrap_or(0);
        for f in 0..across.len() {
            let s = cheapest(f);
            pairs.push((f, s, cheapest_front(s) != f));
        }
        for s in 0..deep.len() {
            let f = cheapest_front(s);
            if !pairs.iter().any(|&(pf, ps, _)| pf == f && ps == s) {
                // a side run that is not its front run's own choice: a limb across
                pairs.push((f, s, cheapest(f) != s));
            }
        }
    }
    let ellipses = pairs
        .iter()
        .map(|&(f, s, limb)| {
            let (a, d) = (across[f], deep[s]);
            let (cx, mut rx) = ((a.0 + a.1) / 2.0, (a.1 - a.0) / 2.0);
            let (cz, mut rz) = ((d.0 + d.1) / 2.0, (d.1 - d.0) / 2.0);
            if limb {
                // a round limb, as thick as its smaller dimension, not a slab
                let r = rx.min(rz);
                (rx, rz) = (r, r);
            }
            (cx, rx, cz, rz)
        })
        .collect();
    let kept = pairs.iter().map(|&(f, s, _)| (across[f], deep[s])).collect();
    (ellipses, kept)
}

/// Calls `visit` with every ordering of `p[k..]` (Heap-style swaps), `p[..k]` fixed.
fn permutations(p: &mut Vec<usize>, k: usize, visit: &mut dyn FnMut(&[usize])) {
    if k == p.len() {
        visit(p);
        return;
    }
    for i in k..p.len() {
        p.swap(k, i);
        permutations(p, k + 1, visit);
        p.swap(k, i);
    }
}

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
fn write_glb(m: &Mesh, png: &[u8]) -> Vec<u8> {
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

    #[test]
    fn a_head_and_a_lance_make_two_parts_not_four() {
        // front: head 20 wide at 10..30, lance 2 wide at 50..52; side: head 18 deep at
        // 0..18, lance 2 deep at 30..32
        let (e, _) = pair_runs(&[(10.0, 30.0), (50.0, 52.0)], &[(0.0, 18.0), (30.0, 32.0)], &[]);
        assert_eq!(e.len(), 2, "{e:?}");
        assert!(e.iter().any(|&(cx, _, cz, _)| cx == 20.0 && cz == 9.0), "head");
        assert!(e.iter().any(|&(cx, _, cz, _)| cx == 51.0 && cz == 31.0), "lance");
    }

    #[test]
    fn arms_beside_a_body_are_round_limbs_not_slabs() {
        // front: arm 8, body 30, arm 8; side: one body 20 deep
        let (e, _) = pair_runs(&[(0.0, 8.0), (10.0, 40.0), (42.0, 50.0)], &[(0.0, 20.0)], &[]);
        assert_eq!(e.len(), 3);
        let arm = e.iter().find(|p| p.0 == 4.0).unwrap();
        assert_eq!((arm.1, arm.3), (4.0, 4.0), "an arm as deep as it is wide");
        let body = e.iter().find(|p| p.0 == 25.0).unwrap();
        assert_eq!((body.1, body.3), (15.0, 10.0), "the body keeps its depth");
    }

    #[test]
    fn two_legs_seen_twice_are_two_legs_not_four() {
        // front: legs side by side; side: one leg ahead of the other; hips above
        let hips = [((0.0, 30.0), (0.0, 20.0))];
        let (e, _) = pair_runs(&[(2.0, 12.0), (18.0, 28.0)], &[(0.0, 9.0), (11.0, 20.0)], &hips);
        assert_eq!(e.len(), 2, "{e:?}");
        let zs: Vec<f32> = e.iter().map(|p| p.2).collect();
        assert!(zs.contains(&4.5) && zs.contains(&15.5), "one ahead, one behind: {zs:?}");
    }

    #[test]
    fn a_part_continues_its_pairing_from_the_row_above() {
        // a long neck and head from the side (40 deep) and narrow from the front (10 wide)
        // continue the neck above, although a lance (2 x 2) is closer in size to nothing
        let neck = [((20.0, 30.0), (0.0, 40.0))];
        let (e, _) = pair_runs(&[(20.0, 30.0), (60.0, 62.0)], &[(0.0, 40.0), (50.0, 52.0)], &neck);
        assert!(e.iter().any(|&(cx, _, cz, _)| cx == 25.0 && cz == 20.0), "head on the neck: {e:?}");
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
        // round slices: no surface near the corners of the 1.0 x 0.5 m rectangle
        let (_, buffers, _) = gltf::import_slice(&glb).unwrap();
        let data = &buffers[0];
        let acc = prim.get(&gltf::Semantic::Positions).unwrap();
        let view = acc.view().unwrap();
        let start = view.offset() + acc.offset();
        let corner = (0..acc.count()).any(|i| {
            let o = start + i * 12;
            let f = |k: usize| f32::from_le_bytes(data[o + k * 4..o + k * 4 + 4].try_into().unwrap());
            let (x, z) = (f(0) - (b.min[0] + b.max[0]) / 2.0, f(2) - (b.min[2] + b.max[2]) / 2.0);
            (x / (wd / 2.0)).abs() > 0.85 && (z / (dp / 2.0)).abs() > 0.85
        });
        assert!(!corner, "a slice of the figure has square corners");
    }
}
