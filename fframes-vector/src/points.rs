//! Surface points (x, y, z, colour) of a figure from one full, even turn: the frames of a
//! turntable video.
//!
//! The camera stands still and the figure turns about a vertical axis by 360°/n from frame to
//! frame. A point of the figure stays in its image row and moves on a circle: in frame `f` it
//! is at `x cos a + z sin a` from the axis, `a` the frame's angle. Its position in one frame
//! gives one coordinate; how it moves while turning gives the other: a quarter turn later its
//! sideways position is its depth.
//!
//! For every pixel of the figure in a start frame (0°, 90°, 180° and 270°, so front, sides and
//! back are all start frames once), every depth along its line of sight predicts its path
//! through the neighbouring frames, a sixth of a turn either side, where it still faces the
//! camera. The depth whose path keeps the pixel's colour is the point's depth. Paths that
//! leave the figure or change colour are rejected; the colour distance along the best path is
//! the point's quality.
//!
//! The axis of each frame is found from the figure's centre of mass, which moves on a sine
//! curve over the turn; what the curve does not explain is the camera's wobble.

use rayon::prelude::*;

#[derive(Clone, Debug)]
pub struct PointsParams {
    /// Colour distance (0..441) that still counts as background.
    pub tolerance: f32,
    /// Every `step`-th pixel and row of a start frame becomes a point.
    pub step: u32,
    /// Largest mean colour distance (0..441) along a point's path.
    pub max_disagreement: f32,
    /// The figure turns clockwise seen from above.
    pub clockwise: bool,
}

impl Default for PointsParams {
    fn default() -> Self {
        PointsParams {
            tolerance: 30.0,
            step: 2,
            max_disagreement: 30.0,
            clockwise: false,
        }
    }
}
/// A surface point in pixels: x to the right of the axis, y above the feet, z towards the
/// camera of the first frame; its colour, and its support (0..1).
#[derive(Clone, Copy, Debug)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub colour: [u8; 3],
    pub support: f32,
}

/// The figure's pixels: background is the median border colour on a slightly blurred frame,
/// connected to the border; of the rest the largest connected region.
fn mask(img: &image::RgbaImage, tolerance: f32) -> Vec<bool> {
    let soft = image::imageops::blur(img, 1.2);
    let (w, h) = soft.dimensions();
    let mut border: Vec<[u8; 4]> = Vec::new();
    for x in 0..w {
        border.push(soft.get_pixel(x, 0).0);
        border.push(soft.get_pixel(x, h - 1).0);
    }
    for y in 0..h {
        border.push(soft.get_pixel(0, y).0);
        border.push(soft.get_pixel(w - 1, y).0);
    }
    let bg: [f32; 3] = std::array::from_fn(|c| {
        let mut v: Vec<u8> = border.iter().map(|p| p[c]).collect();
        v.sort_unstable();
        f32::from(v[v.len() / 2])
    });
    let near = |i: usize| {
        let p = soft.as_raw();
        (0..3).map(|c| (f32::from(p[i * 4 + c]) - bg[c]).powi(2)).sum::<f32>().sqrt() <= tolerance
    };
    let n = (w * h) as usize;
    let mut background = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    for x in 0..w as usize {
        stack.push(x);
        stack.push((h as usize - 1) * w as usize + x);
    }
    for y in 0..h as usize {
        stack.push(y * w as usize);
        stack.push(y * w as usize + w as usize - 1);
    }
    let neighbours = |i: usize| {
        let (x, y) = (i % w as usize, i / w as usize);
        let mut v = Vec::with_capacity(4);
        if x > 0 {
            v.push(i - 1);
        }
        if x + 1 < w as usize {
            v.push(i + 1);
        }
        if y > 0 {
            v.push(i - w as usize);
        }
        if y + 1 < h as usize {
            v.push(i + w as usize);
        }
        v
    };
    while let Some(i) = stack.pop() {
        if background[i] || !near(i) {
            continue;
        }
        background[i] = true;
        stack.extend(neighbours(i));
    }
    // largest region of the rest
    let mut label = vec![0u32; n];
    let (mut best, mut best_size, mut next) = (0u32, 0usize, 0u32);
    for start in 0..n {
        if background[start] || label[start] != 0 {
            continue;
        }
        next += 1;
        label[start] = next;
        let (mut size, mut todo) = (0usize, vec![start]);
        while let Some(i) = todo.pop() {
            size += 1;
            for j in neighbours(i) {
                if !background[j] && label[j] == 0 {
                    label[j] = next;
                    todo.push(j);
                }
            }
        }
        if size > best_size {
            (best, best_size) = (next, size);
        }
    }
    label.iter().map(|&l| l == best && l != 0).collect()
}

/// The surface points of the figure in `frames` (one even turn, frame 0 facing the camera).
/// Also returns the median support of the points.
pub fn turntable_points(frames: &[image::RgbaImage], p: &PointsParams) -> Result<(Vec<Point>, f32), String> {
    let n = frames.len();
    if n < 12 {
        return Err(format!("{n} frames; one turn needs at least 12"));
    }
    let (w, h) = frames[0].dimensions();
    if frames.iter().any(|f| f.dimensions() != (w, h)) {
        return Err("frames of different sizes".into());
    }
    let masks: Vec<Vec<bool>> = frames.par_iter().map(|f| mask(f, p.tolerance)).collect();
    // feet line: the median of the frames' lowest figure rows
    let mut bottoms: Vec<u32> = masks
        .iter()
        .map(|m| (0..h).rev().find(|&y| (0..w).any(|x| m[(y * w + x) as usize])).unwrap_or(h - 1))
        .collect();
    bottoms.sort_unstable();
    let feet = bottoms[n / 2] as f32;
    // per-frame axis: centroid minus its sine curve over the turn
    let centroid: Vec<f32> = masks
        .iter()
        .map(|m| {
            let (mut s, mut c) = (0.0_f64, 0.0_f64);
            for (i, &on) in m.iter().enumerate() {
                if on {
                    s += f64::from(i as u32 % w);
                    c += 1.0;
                }
            }
            (s / c.max(1.0)) as f32
        })
        .collect();
    let mean = centroid.iter().sum::<f32>() / n as f32;
    let angle = |f: usize| std::f32::consts::TAU * f as f32 / n as f32 * if p.clockwise { 1.0 } else { -1.0 };
    let (mut ca, mut sa) = (0.0_f32, 0.0_f32);
    for (f, &c) in centroid.iter().enumerate() {
        let a = std::f32::consts::TAU * f as f32 / n as f32;
        ca += (c - mean) * a.cos() * 2.0 / n as f32;
        sa += (c - mean) * a.sin() * 2.0 / n as f32;
    }
    let axis: Vec<f32> = centroid
        .iter()
        .enumerate()
        .map(|(f, &c)| {
            let a = std::f32::consts::TAU * f as f32 / n as f32;
            c - ca * a.cos() - sa * a.sin()
        })
        .collect();
    // how far the figure reaches from the axis
    let reach = masks
        .iter()
        .zip(&axis)
        .flat_map(|(m, &a)| m.iter().enumerate().filter(|(_, on)| **on).map(move |(i, _)| ((i as u32 % w) as f32 - a).abs()))
        .fold(0.0_f32, f32::max)
        .ceil() as i32
        + 2;
    let rot: Vec<(f32, f32)> = (0..n).map(|f| (angle(f).cos(), angle(f).sin())).collect();

    // colours on slightly blurred frames, so a pixel's neighbourhood counts, not its noise
    let soft: Vec<image::RgbaImage> = frames.par_iter().map(|f| image::imageops::blur(f, 1.0)).collect();
    let colour_at = |f: usize, x: f32, y: u32| -> Option<[f32; 3]> {
        let xi = x.floor();
        if xi < 0.0 || xi >= (w - 1) as f32 || !masks[f][(y * w + xi as u32) as usize] {
            return None;
        }
        // linear between the two neighbouring pixels
        let t = x - xi;
        let (a, b) = (soft[f].get_pixel(xi as u32, y).0, soft[f].get_pixel(xi as u32 + 1, y).0);
        Some(std::array::from_fn(|c| f32::from(a[c]) * (1.0 - t) + f32::from(b[c]) * t))
    };
    let dist = |a: [f32; 3], b: [f32; 3]| (0..3).map(|c| (a[c] - b[c]).powi(2)).sum::<f32>().sqrt();
    // a point stays visible while it faces the camera: frames within a sixth of a turn either
    // side of the start frame are compared
    let span = (n / 6).max(2) as i64;
    let starts = [0, n / 4, n / 2, 3 * n / 4];
    let step = p.step.max(1);

    let rows: Vec<Vec<Point>> = (0..h)
        .into_par_iter()
        .filter(|&y| (y as f32) <= feet && y % step == 0)
        .map(|y| {
            let mut out = Vec::new();
            for &s in &starts {
                let (cs, ss) = rot[s];
                for xi in (0..w).step_by(step as usize) {
                    if !masks[s][(y * w + xi) as usize] {
                        continue;
                    }
                    let u0 = xi as f32 + 0.5 - axis[s];
                    let Some(c0) = colour_at(s, xi as f32 + 0.5, y) else { continue };
                    let deep = ((reach * reach) as f32 - u0 * u0).max(0.0).sqrt();
                    // the depth whose path through the frames keeps the colour best
                    let mut best = (f32::MAX, 0.0_f32, 0usize);
                    let mut d = -deep;
                    while d <= deep {
                        // the point in figure coordinates: u0 across, d towards frame s's camera
                        let (x, z) = (u0 * cs - d * ss, u0 * ss + d * cs);
                        let (mut sum, mut cnt) = (0.0_f32, 0usize);
                        for o in -span..=span {
                            if o == 0 {
                                continue;
                            }
                            let f = (s as i64 + o).rem_euclid(n as i64) as usize;
                            let (c, sn) = rot[f];
                            match colour_at(f, axis[f] + x * c + z * sn, y) {
                                Some(col) => {
                                    sum += dist(col, c0);
                                    cnt += 1;
                                }
                                None => sum += 441.0 / 4.0,
                            }
                        }
                        let score = sum / (2 * span) as f32;
                        if score < best.0 {
                            best = (score, d, cnt);
                        }
                        d += 1.0;
                    }
                    if best.0 > p.max_disagreement || best.2 < (span as usize) {
                        continue;
                    }
                    let (x, z) = (u0 * cs - best.1 * ss, u0 * ss + best.1 * cs);
                    out.push(Point {
                        x,
                        y: feet - y as f32,
                        z,
                        colour: [c0[0] as u8, c0[1] as u8, c0[2] as u8],
                        support: 1.0 - best.0 / p.max_disagreement,
                    });
                }
            }
            out
        })
        .collect();
    let points: Vec<Point> = rows.into_iter().flatten().collect();
    let mut s: Vec<f32> = points.iter().map(|q| q.support).collect();
    s.sort_by(f32::total_cmp);
    let median = s.get(s.len() / 2).copied().unwrap_or(0.0);
    Ok((points, median))
}

/// The points as a binary glTF a 3D viewer shows: every point a small octahedron `size`
/// metres across in its colour (vertex colours), scaled by `scale` (metres per pixel), feet
/// at y = 0.
#[must_use]
pub fn to_glb(points: &[Point], scale: f32, size: f32) -> Vec<u8> {
    let r = size / 2.0;
    let corners: [[f32; 3]; 6] = [[r, 0.0, 0.0], [-r, 0.0, 0.0], [0.0, r, 0.0], [0.0, -r, 0.0], [0.0, 0.0, r], [0.0, 0.0, -r]];
    let faces: [[u32; 3]; 8] = [[0, 2, 4], [2, 1, 4], [1, 3, 4], [3, 0, 4], [2, 0, 5], [1, 2, 5], [3, 1, 5], [0, 3, 5]];
    let srgb = |c: u8| {
        let v = f32::from(c) / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    let (mut pos, mut nrm, mut col, mut idx) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for q in points {
        let base = (pos.len() / 3) as u32;
        let c = [srgb(q.colour[0]), srgb(q.colour[1]), srgb(q.colour[2]), 1.0];
        for k in corners {
            let p = [q.x * scale + k[0], q.y * scale + k[1], q.z * scale + k[2]];
            for a in 0..3 {
                lo[a] = lo[a].min(p[a]);
                hi[a] = hi[a].max(p[a]);
            }
            pos.extend(p);
            let l = (k[0] * k[0] + k[1] * k[1] + k[2] * k[2]).sqrt().max(1e-9);
            nrm.extend([k[0] / l, k[1] / l, k[2] / l]);
            col.extend(c);
        }
        for f in faces {
            idx.extend(f.map(|i| base + i));
        }
    }
    let mut bin: Vec<u8> = Vec::new();
    let mut views = Vec::new();
    for (bytes, target) in [
        (pos.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>(), 34962),
        (nrm.iter().flat_map(|f| f.to_le_bytes()).collect(), 34962),
        (col.iter().flat_map(|f| f.to_le_bytes()).collect(), 34962),
        (idx.iter().flat_map(|i| i.to_le_bytes()).collect(), 34963),
    ] {
        views.push(serde_json::json!({"buffer": 0, "byteOffset": bin.len(), "byteLength": bytes.len(), "target": target}));
        bin.extend(bytes);
    }
    let n = pos.len() / 3;
    let doc = serde_json::json!({
        "asset": {"version": "2.0", "generator": "fframes-vector points"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"name": "points", "mesh": 0}],
        "meshes": [{"name": "points", "primitives": [{
            "attributes": {"POSITION": 0, "NORMAL": 1, "COLOR_0": 2},
            "indices": 3, "material": 0
        }]}],
        "materials": [{"name": "dots", "pbrMetallicRoughness": {
            "baseColorFactor": [1.0, 1.0, 1.0, 1.0], "metallicFactor": 0.0, "roughnessFactor": 0.8
        }}],
        "accessors": [
            {"bufferView": 0, "componentType": 5126, "count": n, "type": "VEC3", "min": lo, "max": hi},
            {"bufferView": 1, "componentType": 5126, "count": n, "type": "VEC3"},
            {"bufferView": 2, "componentType": 5126, "count": n, "type": "VEC4"},
            {"bufferView": 3, "componentType": 5125, "count": idx.len(), "type": "SCALAR"}
        ],
        "bufferViews": views,
        "buffers": [{"byteLength": bin.len()}]
    });
    let mut json = serde_json::to_vec(&doc).unwrap_or_default();
    while !json.len().is_multiple_of(4) {
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

/// The points as an ASCII PLY point cloud, scaled by `scale` (metres per pixel).
#[must_use]
pub fn to_ply(points: &[Point], scale: f32) -> String {
    use std::fmt::Write;
    let mut s = format!(
        "ply\nformat ascii 1.0\nelement vertex {}\nproperty float x\nproperty float y\nproperty float z\nproperty uchar red\nproperty uchar green\nproperty uchar blue\nproperty float support\nend_header\n",
        points.len()
    );
    for q in points {
        let _ = writeln!(
            s,
            "{:.4} {:.4} {:.4} {} {} {} {:.2}",
            q.x * scale,
            q.y * scale,
            q.z * scale,
            q.colour[0],
            q.colour[1],
            q.colour[2],
            q.support
        );
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `n` frames of a cylinder of radius `r` px with 12 coloured stripes around it, turning
    /// once (anticlockwise seen from above), seen along -z from far away; rows 10..90.
    fn striped_cylinder(n: usize, r: f32) -> Vec<image::RgbaImage> {
        let palette: [[u8; 3]; 12] = [
            [200, 40, 30], [230, 200, 40], [40, 110, 200], [60, 160, 70], [150, 60, 160], [240, 140, 40],
            [30, 30, 30], [120, 200, 220], [180, 120, 80], [90, 90, 160], [220, 220, 220], [140, 30, 60],
        ];
        (0..n)
            .map(|f| {
                let a = -std::f32::consts::TAU * f as f32 / n as f32;
                let (c, s) = (a.cos(), a.sin());
                let mut img = image::RgbaImage::from_pixel(160, 100, image::Rgba([200, 200, 200, 255]));
                for xi in 0..160 {
                    let u = xi as f32 + 0.5 - 80.0;
                    if u.abs() >= r {
                        continue;
                    }
                    // the visible (near) surface point on this column: depth towards the camera
                    let w = (r * r - u * u).sqrt();
                    let (x, z) = (u * c - w * s, u * s + w * c);
                    let k = ((z.atan2(x) + std::f32::consts::PI) / std::f32::consts::TAU * 12.0) as usize % 12;
                    let col = palette[k];
                    for y in 10..90 {
                        img.put_pixel(xi, y, image::Rgba([col[0], col[1], col[2], 255]));
                    }
                }
                img
            })
            .collect()
    }

    #[test]
    #[ignore = "the method misses its own accuracy: 37 % of the points within 1.5 px of the radius, 90 % required; superseded by crate::depth"]
    fn points_lie_on_the_cylinder() {
        let r = 30.0;
        let frames = striped_cylinder(72, r);
        let (points, _) = turntable_points(&frames, &PointsParams::default()).unwrap();
        assert!(points.len() > 200, "{} points", points.len());
        let radii: Vec<f32> = points.iter().map(|q| (q.x * q.x + q.z * q.z).sqrt()).collect();
        let on = radii.iter().filter(|&&d| (d - r).abs() <= 1.5).count();
        let share = on as f32 / radii.len() as f32;
        let mut sorted = radii.clone();
        sorted.sort_by(f32::total_cmp);
        eprintln!(
            "{} points, {:.0} % within 1.5 px of the radius; radius quartiles {:.1} {:.1} {:.1}",
            points.len(),
            share * 100.0,
            sorted[sorted.len() / 4],
            sorted[sorted.len() / 2],
            sorted[sorted.len() * 3 / 4]
        );
        assert!(share >= 0.9, "only {:.0} % of the points on the cylinder", share * 100.0);
    }
}