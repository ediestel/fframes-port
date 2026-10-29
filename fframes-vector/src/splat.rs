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
    struct Projected {
        depth: f32,
        u: f32,
        v: f32,
        conic: [f32; 3],
        radius: f32,
        opacity: f32,
        colour: [f32; 3],
    }
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
            let mid = 0.5 * (a + c);
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
    // per tile of 16 x 16 pixels, the splats that reach it, nearest first
    const TILE: usize = 16;
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
    let pixels: Vec<([f32; 3], f32)> = (0..w * h)
        .into_par_iter()
        .map(|i| {
            let (px, py) = ((i % w) as f32 + 0.5, (i / w) as f32 + 0.5);
            let (mut colour, mut through) = ([0.0_f32; 3], 1.0_f32);
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
                for c in 0..3 {
                    colour[c] += p.colour[c] * alpha * through;
                }
                through *= 1.0 - alpha;
                if through < 1e-4 {
                    break;
                }
            }
            (colour, 1.0 - through)
        })
        .collect();
    let mut img = image::RgbaImage::new(w as u32, h as u32);
    let mut cover = vec![0.0_f32; w * h];
    for (i, (c, a)) in pixels.iter().enumerate() {
        let px: [u8; 3] = std::array::from_fn(|k| ((c[k] + (1.0 - a) * f32::from(background[k]) / 255.0) * 255.0).round().clamp(0.0, 255.0) as u8);
        img.put_pixel((i % w) as u32, (i / w) as u32, image::Rgba([px[0], px[1], px[2], 255]));
        cover[i] = *a;
    }
    (img, cover)
}
