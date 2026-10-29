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
