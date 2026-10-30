//! A turntable figure as a dataset for Gaussian splatting (Nerfstudio format: `transforms.json`,
//! `images/`, a starting point cloud), from the turn measured by [`crate::depth`].
//!
//! The figure turns in front of a still camera; here the camera goes round the still figure
//! instead, by each frame's measured angle, around the measured axis. The video looks almost
//! without perspective, so every camera stands `DISTANCE` metres away with a lens that shows
//! the figure at its pixel size: points 1 m in front of or behind the axis land within 2 %
//! of where a view without perspective puts them.
//!
//! World axes: x right and z towards the camera of the first frame, y up; metres. Image rows
//! map to y around the middle row; image columns around the axis.

use rayon::prelude::*;
use serde_json::json;

/// Camera distance, metres.
const DISTANCE: f32 = 30.0;

/// Writes `transforms.json`, `images/f_NNN.png` (the figure, background transparent) and
/// `points3D.ply` (starting points in image pixels: x and z from the axis, y the image row)
/// into `dir`. `metres_per_pixel` scales the image to the world.
pub fn write_dataset(
    dir: &std::path::Path,
    frames: &[image::RgbaImage],
    masks: &[Vec<bool>],
    angle: &[(f32, f32)],
    axis: f32,
    metres_per_pixel: f32,
    points: &[([f32; 3], [u8; 3])],
) -> Result<(), String> {
    let err = |e: std::io::Error| format!("{}: {e}", dir.display());
    std::fs::create_dir_all(dir.join("images")).map_err(err)?;
    let (w, h) = frames.first().map_or((0, 0), image::RgbaImage::dimensions);
    let mid = h as f32 / 2.0;
    let focal = DISTANCE / metres_per_pixel;
    let mut list = Vec::new();
    for (f, (img, mask)) in frames.iter().zip(masks).enumerate() {
        let mut cut = img.clone();
        for (px, &m) in cut.pixels_mut().zip(mask) {
            px.0[3] = if m { 255 } else { 0 };
        }
        let name = format!("images/f_{:03}.png", f + 1);
        cut.save(dir.join(&name)).map_err(|e| format!("{name}: {e}"))?;
        // camera axes in the world: right (c, 0, s), up (0, 1, 0), backwards (towards the
        // camera, OpenGL) (-s, 0, c); it stands DISTANCE along backwards from the axis
        let (c, s) = angle[f];
        let (bx, bz) = (-s, c);
        list.push(json!({
            "file_path": name,
            "transform_matrix": [
                [c, 0.0, bx, DISTANCE * bx],
                [0.0, 1.0, 0.0, 0.0],
                [s, 0.0, bz, DISTANCE * bz],
                [0.0, 0.0, 0.0, 1.0]
            ]
        }));
    }
    let doc = json!({
        "camera_model": "PINHOLE",
        "fl_x": focal, "fl_y": focal, "cx": axis, "cy": mid, "w": w, "h": h,
        "ply_file_path": "points3D.ply",
        "frames": list
    });
    std::fs::write(dir.join("transforms.json"), serde_json::to_vec_pretty(&doc).unwrap_or_default()).map_err(err)?;

    // starting points: image pixels to metres (y from the middle row, upwards)
    let mut ply = format!(
        "ply\nformat binary_little_endian 1.0\nelement vertex {}\nproperty float x\nproperty float y\nproperty float z\nproperty uchar red\nproperty uchar green\nproperty uchar blue\nend_header\n",
        points.len()
    )
    .into_bytes();
    for (p, rgb) in points {
        for v in [p[0] * metres_per_pixel, (mid - p[1]) * metres_per_pixel, p[2] * metres_per_pixel] {
            ply.extend_from_slice(&v.to_le_bytes());
        }
        ply.extend_from_slice(rgb);
    }
    std::fs::write(dir.join("points3D.ply"), ply).map_err(err)
}

/// One trained Gaussian: centre, rotation-scale covariance, opacity and colour as spherical
/// harmonics up to degree 3 (16 coefficients per channel), as the 3D Gaussian splatting
/// format stores them.
#[derive(Clone, Debug)]
pub struct Splat {
    pub centre: [f32; 3],
    /// Covariance, world axes (symmetric 3x3).
    pub cov: [[f32; 3]; 3],
    pub opacity: f32,
    pub sh: [[f32; 16]; 3],
}

/// Reads a binary 3D Gaussian splatting `.ply` (as Brush and the reference implementation
/// export it): `x y z`, `f_dc_*`, `f_rest_*` (channel by channel), `opacity` (logit),
/// `scale_*` (log), `rot_*` (quaternion w x y z).
pub fn read_splats(path: &std::path::Path) -> Result<Vec<Splat>, String> {
    let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let end = data.windows(11).position(|w| w == b"end_header\n").ok_or("no ply header")? + 11;
    let head = String::from_utf8_lossy(&data[..end]);
    let count: usize = head
        .lines()
        .find_map(|l| l.strip_prefix("element vertex "))
        .and_then(|n| n.trim().parse().ok())
        .ok_or("no vertex count")?;
    let names: Vec<&str> = head
        .lines()
        .filter_map(|l| l.strip_prefix("property float "))
        .collect();
    if head.lines().filter(|l| l.starts_with("property")).count() != names.len() {
        return Err("only float properties are read".into());
    }
    let col = |name: &str| names.iter().position(|n| *n == name);
    let need = |name: &str| col(name).ok_or(format!("no property {name}"));
    let (x, y, z) = (need("x")?, need("y")?, need("z")?);
    let dc = [need("f_dc_0")?, need("f_dc_1")?, need("f_dc_2")?];
    let rest: Vec<Option<usize>> = (0..45).map(|i| col(&format!("f_rest_{i}"))).collect();
    let (op, sc, rot) = (
        need("opacity")?,
        [need("scale_0")?, need("scale_1")?, need("scale_2")?],
        [need("rot_0")?, need("rot_1")?, need("rot_2")?, need("rot_3")?],
    );
    let stride = names.len() * 4;
    if data.len() < end + count * stride {
        return Err("ply shorter than its header says".into());
    }
    Ok((0..count)
        .map(|i| {
            let at = |c: usize| {
                let o = end + i * stride + c * 4;
                f32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]])
            };
            let mut sh = [[0.0_f32; 16]; 3];
            for c in 0..3 {
                sh[c][0] = at(dc[c]);
                for k in 1..16 {
                    sh[c][k] = rest[c * 15 + k - 1].map_or(0.0, at);
                }
            }
            let s = sc.map(|c| at(c).exp());
            let q = rot.map(at);
            let l = (q.iter().map(|v| v * v).sum::<f32>()).sqrt().max(1e-12);
            let (w, qx, qy, qz) = (q[0] / l, q[1] / l, q[2] / l, q[3] / l);
            let r = [
                [1.0 - 2.0 * (qy * qy + qz * qz), 2.0 * (qx * qy - w * qz), 2.0 * (qx * qz + w * qy)],
                [2.0 * (qx * qy + w * qz), 1.0 - 2.0 * (qx * qx + qz * qz), 2.0 * (qy * qz - w * qx)],
                [2.0 * (qx * qz - w * qy), 2.0 * (qy * qz + w * qx), 1.0 - 2.0 * (qx * qx + qy * qy)],
            ];
            // covariance R S S^T R^T
            let cov = std::array::from_fn(|a| {
                std::array::from_fn(|b| (0..3).map(|k| r[a][k] * s[k] * s[k] * r[b][k]).sum())
            });
            Splat { centre: [at(x), at(y), at(z)], cov, opacity: 1.0 / (1.0 + (-at(op)).exp()), sh }
        })
        .collect())
}

/// Colour of spherical harmonics (degree 3) seen along unit direction `d`, from the centre
/// towards the camera's opposite (the reference convention: direction from camera to point).
fn sh_colour(sh: &[[f32; 16]; 3], d: [f32; 3]) -> [f32; 3] {
    const C0: f32 = 0.282_094_8;
    const C1: f32 = 0.488_602_5;
    const C2: [f32; 5] = [1.092_548_5, -1.092_548_5, 0.315_391_57, -1.092_548_5, 0.546_274_2];
    const C3: [f32; 7] = [-0.590_043_6, 2.890_611_4, -0.457_045_8, 0.373_176_33, -0.457_045_8, 1.445_305_7, -0.590_043_6];
    let (x, y, z) = (d[0], d[1], d[2]);
    let (xx, yy, zz, xy, yz, xz) = (x * x, y * y, z * z, x * y, y * z, x * z);
    let basis = [
        C0,
        -C1 * y,
        C1 * z,
        -C1 * x,
        C2[0] * xy,
        C2[1] * yz,
        C2[2] * (2.0 * zz - xx - yy),
        C2[3] * xz,
        C2[4] * (xx - yy),
        C3[0] * y * (3.0 * xx - yy),
        C3[1] * xy * z,
        C3[2] * y * (4.0 * zz - xx - yy),
        C3[3] * z * (2.0 * zz - 3.0 * xx - 3.0 * yy),
        C3[4] * x * (4.0 * zz - xx - yy),
        C3[5] * z * (xx - yy),
        C3[6] * x * (xx - 3.0 * yy),
    ];
    std::array::from_fn(|c| (0.5 + (0..16).map(|k| basis[k] * sh[c][k]).sum::<f32>()).clamp(0.0, 1.0))
}

/// A pinhole camera as `transforms.json` stores it: camera-to-world (OpenGL axes), focal
/// lengths and principal point in pixels, image size.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub to_world: [[f32; 4]; 4],
    pub fx: f32,
    pub fy: f32,
    pub cx: f32,
    pub cy: f32,
    pub width: u32,
    pub height: u32,
}

/// The cameras of a Nerfstudio `transforms.json`, with the image paths.
pub fn read_cameras(path: &std::path::Path) -> Result<Vec<(String, Camera)>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let doc: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let num = |k: &str| doc[k].as_f64().map(|v| v as f32).ok_or(format!("no {k}"));
    let (fx, fy, cx, cy) = (num("fl_x")?, num("fl_y")?, num("cx")?, num("cy")?);
    let (w, h) = (num("w")? as u32, num("h")? as u32);
    doc["frames"]
        .as_array()
        .ok_or("no frames")?
        .iter()
        .map(|f| {
            let m = &f["transform_matrix"];
            let to_world: [[f32; 4]; 4] =
                std::array::from_fn(|r| std::array::from_fn(|c| m[r][c].as_f64().unwrap_or(0.0) as f32));
            let name = f["file_path"].as_str().unwrap_or_default().to_string();
            Ok((name, Camera { to_world, fx, fy, cx, cy, width: w, height: h }))
        })
        .collect()
}

/// Renders the splats through a camera, `scale` times its resolution, over `background`;
/// also returns the rendered coverage (alpha) per pixel. Standard splatting: every Gaussian
/// projected as an ellipse (its covariance through the camera's Jacobian, plus 0.3 px² so it
/// covers at least a pixel), coloured by its spherical harmonics towards the camera, blended
/// front to back until a pixel is opaque.
#[must_use]
pub fn render(splats: &[Splat], cam: &Camera, scale: f32, background: [u8; 3]) -> (image::RgbaImage, Vec<f32>) {
    let r = render_layers(splats, cam, scale, background);
    (r.image, r.alpha)
}

/// A rendering with its depth layers, per pixel. Depths are camera distances (metres along
/// the viewing direction); NaN where nothing is drawn.
pub struct Rendered {
    pub image: image::RgbaImage,
    /// Accumulated opacity, 0 to 1.
    pub alpha: Vec<f32>,
    /// Opacity-weighted mean depth (normalised by `alpha`): between layers when several are
    /// drawn over each other.
    pub expected: Vec<f32>,
    /// Depth at which the accumulated opacity passes one half: the first layer that covers
    /// the pixel.
    pub median: Vec<f32>,
    /// Opacity-weighted standard deviation of the depths: large where layers mix.
    pub spread: Vec<f32>,
}

/// A splat projected into the image: depth, centre (pixels), the inverse of its 2D
/// covariance (a, b, c of the conic), its reach (pixels), opacity and colour.
struct Projected {
    depth: f32,
    u: f32,
    v: f32,
    conic: [f32; 3],
    radius: f32,
    opacity: f32,
    colour: [f32; 3],
}

/// Edge of the square image tiles the splats are sorted into, pixels.
const TILE: usize = 16;

/// [`render`] with depth layers.
#[must_use]
pub fn render_layers(splats: &[Splat], cam: &Camera, scale: f32, background: [u8; 3]) -> Rendered {
    let (w, h) = (((cam.width as f32) * scale).round() as usize, ((cam.height as f32) * scale).round() as usize);
    let (fx, fy, cx, cy) = (cam.fx * scale, cam.fy * scale, cam.cx * scale, cam.cy * scale);
    let m = cam.to_world;
    let pos = [m[0][3], m[1][3], m[2][3]];
    // world to camera, OpenCV axes (x right, y down, z forward): rows are the camera's right,
    // -up and -back axes
    let rows = [
        [m[0][0], m[1][0], m[2][0]],
        [-m[0][1], -m[1][1], -m[2][1]],
        [-m[0][2], -m[1][2], -m[2][2]],
    ];
    let mut list: Vec<Projected> = splats
        .iter()
        .filter_map(|s| {
            let d = [s.centre[0] - pos[0], s.centre[1] - pos[1], s.centre[2] - pos[2]];
            let pc: [f32; 3] = std::array::from_fn(|r| (0..3).map(|k| rows[r][k] * d[k]).sum());
            if pc[2] <= 0.01 {
                return None;
            }
            let (u, v) = (cx + fx * pc[0] / pc[2], cy + fy * pc[1] / pc[2]);
            // 2D covariance: J W Σ W^T J^T
            let j = [[fx / pc[2], 0.0, -fx * pc[0] / (pc[2] * pc[2])], [0.0, fy / pc[2], -fy * pc[1] / (pc[2] * pc[2])]];
            let t: [[f32; 3]; 2] = std::array::from_fn(|a| std::array::from_fn(|b| (0..3).map(|k| j[a][k] * rows[k][b]).sum()));
            let c2: [[f32; 2]; 2] = std::array::from_fn(|a| {
                std::array::from_fn(|b| {
                    (0..3).map(|p| (0..3).map(|q| t[a][p] * s.cov[p][q] * t[b][q]).sum::<f32>()).sum::<f32>()
                })
            });
            let (a, b, c) = (c2[0][0] + 0.3, c2[0][1], c2[1][1] + 0.3);
            let det = a * c - b * b;
            if det <= 0.0 {
                return None;
            }
            let mid = f32::midpoint(a, c);
            let radius = 3.0 * (mid + (mid * mid - det).max(0.1).sqrt()).sqrt();
            if u + radius < 0.0 || v + radius < 0.0 || u - radius >= w as f32 || v - radius >= h as f32 {
                return None;
            }
            let l = (d.iter().map(|x| x * x).sum::<f32>()).sqrt().max(1e-12);
            Some(Projected {
                depth: pc[2],
                u,
                v,
                conic: [c / det, -b / det, a / det],
                radius,
                opacity: s.opacity,
                colour: sh_colour(&s.sh, d.map(|x| x / l)),
            })
        })
        .collect();
    list.sort_by(|p, q| p.depth.total_cmp(&q.depth));
    // per tile, the splats that reach it, nearest first
    let (tx, ty) = (w.div_ceil(TILE), h.div_ceil(TILE));
    let mut tiles: Vec<Vec<u32>> = vec![Vec::new(); tx * ty];
    for (i, p) in list.iter().enumerate() {
        let x0 = ((p.u - p.radius).max(0.0) as usize / TILE).min(tx - 1);
        let x1 = ((p.u + p.radius).max(0.0) as usize / TILE).min(tx - 1);
        let y0 = ((p.v - p.radius).max(0.0) as usize / TILE).min(ty - 1);
        let y1 = ((p.v + p.radius).max(0.0) as usize / TILE).min(ty - 1);
        for yy in y0..=y1 {
            for xx in x0..=x1 {
                tiles[yy * tx + xx].push(i as u32);
            }
        }
    }
    // per pixel: colour, opacity, then the depth sums (weight · depth, weight · depth²) and
    // the depth where the opacity passes one half
    let pixels: Vec<([f32; 3], f32, f32, f32, f32)> = (0..w * h)
        .into_par_iter()
        .map(|i| {
            let (px, py) = ((i % w) as f32 + 0.5, (i / w) as f32 + 0.5);
            let (mut colour, mut through) = ([0.0_f32; 3], 1.0_f32);
            let (mut sum, mut sum2, mut median) = (0.0_f32, 0.0_f32, f32::NAN);
            for &k in &tiles[(i / w / TILE) * tx + (i % w) / TILE] {
                let p = &list[k as usize];
                let (dx, dy) = (px - p.u, py - p.v);
                let power = -0.5 * (p.conic[0] * dx * dx + p.conic[2] * dy * dy) - p.conic[1] * dx * dy;
                if power > 0.0 {
                    continue;
                }
                let alpha = (p.opacity * power.exp()).min(0.99);
                if alpha < 1.0 / 255.0 {
                    continue;
                }
                let weight = alpha * through;
                for (acc, c) in colour.iter_mut().zip(p.colour) {
                    *acc += c * weight;
                }
                sum += weight * p.depth;
                sum2 += weight * p.depth * p.depth;
                through *= 1.0 - alpha;
                if median.is_nan() && through <= 0.5 {
                    median = p.depth;
                }
                if through < 1e-4 {
                    break;
                }
            }
            (colour, 1.0 - through, sum, sum2, median)
        })
        .collect();
    let mut img = image::RgbaImage::new(w as u32, h as u32);
    let (mut alpha, mut expected, mut median, mut spread) =
        (vec![0.0_f32; w * h], vec![f32::NAN; w * h], vec![f32::NAN; w * h], vec![f32::NAN; w * h]);
    for (i, (c, a, sum, sum2, med)) in pixels.iter().enumerate() {
        let px: [u8; 3] = std::array::from_fn(|k| ((c[k] + (1.0 - a) * f32::from(background[k]) / 255.0) * 255.0).round().clamp(0.0, 255.0) as u8);
        img.put_pixel((i % w) as u32, (i / w) as u32, image::Rgba([px[0], px[1], px[2], 255]));
        alpha[i] = *a;
        if *a > 1e-3 {
            let mean = sum / a;
            expected[i] = mean;
            spread[i] = (sum2 / a - mean * mean).max(0.0).sqrt();
            median[i] = *med;
        }
    }
    Rendered { image: img, alpha, expected, median, spread }
}

/// Stands splats free in a scene as `fframes-walk` stands a model on its floor: their box
/// (the centres of splats more than half opaque, the outer half percent left out so stray
/// splats do not count) scaled to `height` metres, its centre at `(x, height / 2, z)`. The
/// turn is kept, so view-dependent colour stays right.
pub fn stand(splats: &mut [Splat], height: f32, x: f32, z: f32) -> Result<(), String> {
    let range = |c: usize| {
        let q = percentiles(splats.iter().filter(|s| s.opacity > 0.5).map(|s| s.centre[c]).collect(), &[0.005, 0.995]);
        (q[0], q[1])
    };
    let (rx, ry, rz) = (range(0), range(1), range(2));
    if !(ry.1 - ry.0).is_finite() || ry.1 <= ry.0 {
        return Err("no opaque splats to measure".into());
    }
    let scale = height / (ry.1 - ry.0);
    let from = [f32::midpoint(rx.0, rx.1), f32::midpoint(ry.0, ry.1), f32::midpoint(rz.0, rz.1)];
    let to = [x, height / 2.0, z];
    for s in splats.iter_mut() {
        s.centre = std::array::from_fn(|c| (s.centre[c] - from[c]) * scale + to[c]);
        s.cov = s.cov.map(|row| row.map(|v| v * scale * scale));
    }
    Ok(())
}

/// A camera as `fframes-walk` logs it (position, target, vertical field of view in degrees,
/// image size; up is +y) as a splat camera.
#[must_use]
pub fn walk_camera(position: [f32; 3], target: [f32; 3], fov: f32, width: u32, height: u32) -> Camera {
    let norm = |a: [f32; 3]| {
        let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-12);
        a.map(|c| c / l)
    };
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let forward = norm([target[0] - position[0], target[1] - position[1], target[2] - position[2]]);
    let right = norm(cross(forward, [0.0, 1.0, 0.0]));
    let up = cross(right, forward);
    let back = forward.map(|c| -c);
    let focal = height as f32 / 2.0 / (fov.to_radians() / 2.0).tan();
    Camera {
        to_world: [
            [right[0], up[0], back[0], position[0]],
            [right[1], up[1], back[1], position[1]],
            [right[2], up[2], back[2], position[2]],
            [0.0, 0.0, 0.0, 1.0],
        ],
        fx: focal,
        fy: focal,
        cx: width as f32 / 2.0,
        cy: height as f32 / 2.0,
        width,
        height,
    }
}

/// Percentiles (0..1) of a list, NaN when empty.
fn percentiles(mut v: Vec<f32>, at: &[f32]) -> Vec<f32> {
    v.retain(|x| x.is_finite());
    v.sort_by(f32::total_cmp);
    at.iter()
        .map(|&p| if v.is_empty() { f32::NAN } else { v[((v.len() - 1) as f32 * p).round() as usize] })
        .collect()
}

/// A camera's world position and its world-to-camera rows (x right, y down, z forward).
fn camera_frame(cam: &Camera) -> ([f32; 3], [[f32; 3]; 3]) {
    let m = cam.to_world;
    (
        [m[0][3], m[1][3], m[2][3]],
        [[m[0][0], m[1][0], m[2][0]], [-m[0][1], -m[1][1], -m[2][1]], [-m[0][2], -m[1][2], -m[2][2]]],
    )
}

/// Depth diagnostics of trained splats against the dataset they were trained on, before any
/// surface is built from them. Every `every`-th camera is rendered with its depth layers;
/// only confident pixels count (opacity over 0.9, inside the image's figure). Depths are in
/// image pixels (the dataset's metres per pixel). Reports:
/// continuity of the median depth between neighbouring pixels; expected against median depth
/// (a gap means layers mixed into one pixel); the depth spread; cross-view agreement (a
/// confident point of one camera against the median depth `offset` cameras away, where both
/// see it); and the confident points themselves (world, metres) for cross-sections.
pub fn depth_report(
    splats: &[Splat],
    cams: &[(String, Camera)],
    dir: &std::path::Path,
    every: usize,
    offset: usize,
) -> Result<(String, Vec<[f32; 3]>), String> {
    let first = cams.first().ok_or("no cameras")?.1;
    let px_per_m = first.fx / DISTANCE;
    let chosen: Vec<usize> = (0..cams.len()).step_by(every.max(1)).collect();
    let layers: Vec<(usize, Rendered, Vec<bool>)> = chosen
        .iter()
        .map(|&i| {
            let (name, cam) = &cams[i];
            let image = image::open(dir.join(name)).map_err(|e| format!("{name}: {e}"))?.to_rgba8();
            let figure: Vec<bool> = image.pixels().map(|p| p.0[3] > 127).collect();
            Ok((i, render_layers(splats, cam, 1.0, [0, 0, 0]), figure))
        })
        .collect::<Result<_, String>>()?;
    let confident = |r: &Rendered, fig: &[bool], i: usize| fig[i] && r.alpha[i] > 0.9 && r.median[i].is_finite();
    let (mut steps, mut gaps, mut spreads, mut figure_px, mut sure_px) = (Vec::new(), Vec::new(), Vec::new(), 0usize, 0usize);
    let mut points: Vec<[f32; 3]> = Vec::new();
    let mut cross = Vec::new();
    for (i, r, fig) in &layers {
        let cam = cams[*i].1;
        let (w, h) = (cam.width as usize, cam.height as usize);
        let (pos, rows) = camera_frame(&cam);
        figure_px += fig.iter().filter(|&&f| f).count();
        // the camera `offset` further on, for the cross-view check
        let other = layers.iter().find(|(j, _, _)| *j == i + offset);
        let other_render;
        let (o_cam, o_r) = if let Some((j, ro, _)) = other {
            (cams[*j].1, ro)
        } else if i + offset < cams.len() {
            other_render = render_layers(splats, &cams[i + offset].1, 1.0, [0, 0, 0]);
            (cams[i + offset].1, &other_render)
        } else {
            (cam, r)
        };
        let (o_pos, o_rows) = camera_frame(&o_cam);
        for y in 0..h {
            for x in 0..w {
                let k = y * w + x;
                if !confident(r, fig, k) {
                    continue;
                }
                sure_px += 1;
                let z = r.median[k];
                gaps.push((r.expected[k] - z).abs() * px_per_m);
                spreads.push(r.spread[k] * px_per_m);
                if x + 1 < w && confident(r, fig, k + 1) {
                    steps.push((r.median[k + 1] - z).abs() * px_per_m);
                }
                // the point in the world
                let c = [(x as f32 + 0.5 - cam.cx) / cam.fx * z, (y as f32 + 0.5 - cam.cy) / cam.fy * z, z];
                let p: [f32; 3] = std::array::from_fn(|a| pos[a] + (0..3).map(|b| rows[b][a] * c[b]).sum::<f32>());
                if (x + y) % 3 == 0 {
                    points.push(p);
                }
                if i + offset < cams.len() {
                    let d = [p[0] - o_pos[0], p[1] - o_pos[1], p[2] - o_pos[2]];
                    let q: [f32; 3] = std::array::from_fn(|a| (0..3).map(|b| o_rows[a][b] * d[b]).sum());
                    let (u, v) = (o_cam.cx + o_cam.fx * q[0] / q[2], o_cam.cy + o_cam.fy * q[1] / q[2]);
                    let (ui, vi) = (u.floor(), v.floor());
                    if ui >= 0.0 && vi >= 0.0 && (ui as usize) < w && (vi as usize) < h {
                        let m = o_r.median[vi as usize * w + ui as usize];
                        // a point the other camera sees (not behind its first layer) agrees
                        if m.is_finite() && q[2] <= m + 4.5 / px_per_m {
                            cross.push((q[2] - m).abs() * px_per_m);
                        }
                    }
                }
            }
        }
    }
    let ps = [0.5, 0.9, 0.99];
    let fmt = |v: Vec<f32>| {
        let q = percentiles(v, &ps);
        format!("median {:.2}, p90 {:.2}, p99 {:.2} px", q[0], q[1], q[2])
    };
    let share = |v: &[f32], limit: f32| v.iter().filter(|&&x| x <= limit).count() as f32 / v.len().max(1) as f32 * 100.0;
    let report = format!(
        "{} cameras checked; confident depth for {:.1} % of figure pixels\n\
         continuity (neighbouring pixels): {}\n\
         expected against median depth: {}; within 2 px: {:.1} %\n\
         depth spread: {}\n\
         cross-view ({} cameras apart): {}; within 4.5 px: {:.1} %",
        layers.len(),
        sure_px as f32 / figure_px.max(1) as f32 * 100.0,
        fmt(steps),
        fmt(gaps.clone()),
        share(&gaps, 2.0),
        fmt(spreads),
        offset,
        fmt(cross.clone()),
        share(&cross, 4.5),
    );
    Ok((report, points))
}
