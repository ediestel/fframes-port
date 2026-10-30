//! A textured mesh straight from trained Gaussians, without measuring depth from pictures
//! again.
//!
//! 1. Points: the centres of opaque splats that some camera of the dataset sees (no deeper
//!    than the rendered surface there by more than `tolerance`); splats inside the figure
//!    drop out.
//! 2. Normals: each point's neighbourhood normal ([`crate::splatshape::neighbourhoods`]),
//!    turned towards the cameras that see it.
//! 3. Field: around every point the signed distance to its tangent plane, Gaussian-weighted
//!    over the points near a voxel; negative inside. Where no point is near, the figure's
//!    outlines decide: inside when the voxel falls inside the images' figure in all but two
//!    cameras.
//! 4. Surface: [`crate::figure::surface_mesh_field`] through the field's zero crossings.
//! 5. Texture: every quad gets a square of the atlas; each texel is painted from the
//!    Gaussian renders of the cameras that see its point, weighted by how directly they
//!    face it.

use crate::figure::{Mesh, surface_mesh_field, write_glb};
use crate::splat::{Camera, Splat, render_layers};
use crate::splatshape::{Local, NeighbourhoodParams, neighbourhoods, spacing};
use rayon::prelude::*;

#[derive(Clone, Debug)]
pub struct SurfaceParams {
    /// Voxel edge, metres.
    pub voxel: f32,
    /// Splats less opaque than this are not surface.
    pub opacity: f32,
    /// How far behind the rendered surface a splat centre still counts as seen, metres.
    pub tolerance: f32,
    /// Every n-th camera is rendered and used.
    pub every: usize,
    /// Rounds of smoothing of the surface.
    pub smooth: usize,
    /// Texels along a quad's square in the atlas.
    pub texels: usize,
}

impl Default for SurfaceParams {
    fn default() -> Self {
        SurfaceParams { voxel: 0.0015, opacity: 0.3, tolerance: 0.003, every: 2, smooth: 1, texels: 4 }
    }
}

/// What [`splat_surface`] built.
pub struct SurfaceInfo {
    pub opaque: usize,
    pub seen: usize,
    pub dims: [usize; 3],
    pub quads: usize,
    pub atlas: u32,
}

/// A camera's position and world-to-camera rows (x right, y down, z forward), as
/// [`render_layers`] uses them.
fn frame(cam: &Camera) -> ([f32; 3], [[f32; 3]; 3]) {
    let m = cam.to_world;
    ([m[0][3], m[1][3], m[2][3]], [[m[0][0], m[1][0], m[2][0]], [-m[0][1], -m[1][1], -m[2][1]], [-m[0][2], -m[1][2], -m[2][2]]])
}

/// Pixel position (pixel centres at .5) and depth of a world point.
fn project(cam: &Camera, (pos, rows): &([f32; 3], [[f32; 3]; 3]), p: [f32; 3]) -> Option<(f32, f32, f32)> {
    let d = [p[0] - pos[0], p[1] - pos[1], p[2] - pos[2]];
    let pc: [f32; 3] = std::array::from_fn(|r| (0..3).map(|k| rows[r][k] * d[k]).sum());
    (pc[2] > 0.01).then(|| (cam.cx + cam.fx * pc[0] / pc[2], cam.cy + cam.fy * pc[1] / pc[2], pc[2]))
}

/// One used camera: its frame, the render's colours and median depths, the image's figure.
struct View {
    cam: Camera,
    frame: ([f32; 3], [[f32; 3]; 3]),
    image: image::RgbaImage,
    median: Vec<f32>,
    mask: Vec<bool>,
}

impl View {
    fn pixel(&self, u: f32, v: f32) -> Option<usize> {
        let (w, h) = (self.cam.width as i64, self.cam.height as i64);
        let (x, y) = (u.floor() as i64, v.floor() as i64);
        (x >= 0 && y >= 0 && x < w && y < h).then(|| (y * w + x) as usize)
    }

    /// Whether the point is no deeper than the rendered surface plus `tol`.
    fn sees(&self, p: [f32; 3], tol: f32) -> Option<(f32, f32)> {
        let (u, v, z) = project(&self.cam, &self.frame, p)?;
        let m = self.median[self.pixel(u, v)?];
        (m.is_finite() && z <= m + tol).then_some((u, v))
    }

    /// Bilinear colour at pixel position (u, v).
    fn colour(&self, u: f32, v: f32) -> [f32; 3] {
        let (w, h) = (self.image.width() as f32, self.image.height() as f32);
        let (x, y) = ((u - 0.5).clamp(0.0, w - 1.001), (v - 0.5).clamp(0.0, h - 1.001));
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let at = |a: f32, b: f32| self.image.get_pixel(a as u32, b as u32).0;
        let (c00, c10, c01, c11) = (at(x0, y0), at(x0 + 1.0, y0), at(x0, y0 + 1.0), at(x0 + 1.0, y0 + 1.0));
        std::array::from_fn(|k| {
            let top = f32::from(c00[k]) * (1.0 - fx) + f32::from(c10[k]) * fx;
            let bottom = f32::from(c01[k]) * (1.0 - fx) + f32::from(c11[k]) * fx;
            top * (1.0 - fy) + bottom * fy
        })
    }
}

/// Builds the textured mesh of the splats seen by `cameras` (with their images' figure from
/// the dataset folder `dir`) and returns it as a binary glTF: feet at y = 0, centred on x and
/// z, metres.
pub fn splat_surface(
    splats: &[Splat],
    cameras: &[(String, Camera)],
    dir: &std::path::Path,
    p: &SurfaceParams,
) -> Result<(Vec<u8>, SurfaceInfo), String> {
    // renders and figures of the used cameras
    let views: Vec<View> = cameras
        .iter()
        .step_by(p.every.max(1))
        .collect::<Vec<_>>()
        .par_iter()
        .map(|(name, cam)| {
            let r = render_layers(splats, cam, 1.0, [0, 0, 0]);
            let img = image::open(dir.join(name)).map_err(|e| format!("{name}: {e}"))?.to_rgba8();
            if img.dimensions() != (cam.width, cam.height) {
                return Err(format!("{name}: not the camera's size"));
            }
            Ok(View { cam: *cam, frame: frame(cam), image: r.image, median: r.median, mask: img.pixels().map(|px| px.0[3] > 127).collect() })
        })
        .collect::<Result<_, String>>()?;

    // 1. opaque splats some camera sees, with the cameras that see them
    let opaque: Vec<[f32; 3]> = splats.iter().filter(|s| s.opacity >= p.opacity).map(|s| s.centre).collect();
    let seen_by: Vec<Vec<usize>> = opaque
        .par_iter()
        .map(|&c| views.iter().enumerate().filter(|(_, v)| v.sees(c, p.tolerance).is_some()).map(|(i, _)| i).collect())
        .collect();
    let seen: Vec<usize> = (0..opaque.len()).filter(|&i| !seen_by[i].is_empty()).collect();
    let points: Vec<[f32; 3]> = seen.iter().map(|&i| opaque[i]).collect();
    if points.len() < 100 {
        return Err(format!("only {} splats seen", points.len()));
    }

    // 2. normals towards the cameras that see them
    let hood = neighbourhoods(&points, 2.0 * spacing(&points), &NeighbourhoodParams::default());
    let oriented: Vec<([f32; 3], [f32; 3])> = seen
        .iter()
        .zip(&points)
        .zip(&hood)
        .filter(|(_, h)| h.local != Local::Floater)
        .map(|((&i, &c), h)| {
            let towards: f32 = seen_by[i]
                .iter()
                .map(|&v| {
                    let pos = views[v].frame.0;
                    let d = [pos[0] - c[0], pos[1] - c[1], pos[2] - c[2]];
                    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-9);
                    (0..3).map(|a| h.normal[a] * d[a]).sum::<f32>() / l
                })
                .sum();
            (c, if towards < 0.0 { h.normal.map(|v| -v) } else { h.normal })
        })
        .collect();

    // 3. the field over the points' box (with a margin), y running down
    let vox = p.voxel;
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for (c, _) in &oriented {
        for a in 0..3 {
            lo[a] = lo[a].min(c[a]);
            hi[a] = hi[a].max(c[a]);
        }
    }
    let margin = 4.0 * vox;
    let dims: [usize; 3] = std::array::from_fn(|a| ((hi[a] - lo[a] + 2.0 * margin) / vox).ceil() as usize + 1);
    let [nx, ny, nz] = dims;
    if nx * ny * nz > 120_000_000 {
        return Err(format!("{nx}x{ny}x{nz} voxels: use a larger voxel"));
    }
    let (x0, ytop, z0) = (lo[0] - margin, hi[1] + margin, lo[2] - margin);
    let centre = |i: usize, j: usize, k: usize| [x0 + i as f32 * vox, ytop - j as f32 * vox, z0 + k as f32 * vox];
    let idx = |i: usize, j: usize, k: usize| (k * ny + j) * nx + i;
    let (mut wsum, mut fsum) = (vec![0.0_f32; nx * ny * nz], vec![0.0_f32; nx * ny * nz]);
    let sigma2 = 2.0 * vox * vox;
    let reach = 3_i64;
    for (c, n) in &oriented {
        let g = [((c[0] - x0) / vox).round() as i64, ((ytop - c[1]) / vox).round() as i64, ((c[2] - z0) / vox).round() as i64];
        for dk in -reach..=reach {
            for dj in -reach..=reach {
                for di in -reach..=reach {
                    let (i, j, k) = (g[0] + di, g[1] + dj, g[2] + dk);
                    if i < 0 || j < 0 || k < 0 || i >= nx as i64 || j >= ny as i64 || k >= nz as i64 {
                        continue;
                    }
                    let x = centre(i as usize, j as usize, k as usize);
                    let d = [x[0] - c[0], x[1] - c[1], x[2] - c[2]];
                    let w = (-(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) / sigma2).exp();
                    let at = idx(i as usize, j as usize, k as usize);
                    wsum[at] += w;
                    fsum[at] += w * (n[0] * d[0] + n[1] * d[1] + n[2] * d[2]);
                }
            }
        }
    }
    // known near the points; elsewhere the outlines decide
    let field: Vec<f32> = wsum.iter().zip(&fsum).map(|(&w, &f)| if w > 0.05 { f / w } else { f32::NAN }).collect();
    let solid: Vec<bool> = (0..nx * ny * nz)
        .into_par_iter()
        .map(|at| {
            let (i, j, k) = (at % nx, (at / nx) % ny, at / (nx * ny));
            if i == 0 || j == 0 || k == 0 || i == nx - 1 || j == ny - 1 || k == nz - 1 {
                return false;
            }
            if field[at].is_finite() {
                return field[at] < 0.0;
            }
            let x = centre(i, j, k);
            let outside = views
                .iter()
                .filter(|v| project(&v.cam, &v.frame, x).and_then(|(u, vv, _)| v.pixel(u, vv)).is_none_or(|px| !v.mask[px]))
                .take(3)
                .count();
            outside <= 2
        })
        .collect();

    // 4. surface; every quad its atlas square (uv by the corner's side of the quad's centre)
    let t = p.texels.max(2) as f32;
    let cells = std::sync::Mutex::new(std::collections::HashMap::<[i32; 3], usize>::new());
    let uv = |_n: [f32; 3], axis: usize, centre: [f32; 3], corner: [f32; 3]| -> [f32; 2] {
        let key = centre.map(|v| (v * 4.0).round() as i32);
        let mut map = cells.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let len = map.len();
        let cell = *map.entry(key).or_insert(len);
        let (a, b) = match axis {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        let su = if corner[a] > centre[a] { 1.0 } else { 0.0 };
        let sv = if corner[b] > centre[b] { 1.0 } else { 0.0 };
        // cell index and corner, to be placed in the atlas once its size is known
        [cell as f32 + su * 0.25, sv]
    };
    let mut mesh: Mesh = surface_mesh_field(&solid, Some(&field), dims, vox, &uv, p.smooth);
    let quads = mesh.indices.len() / 6;
    let count = cells.lock().map_or(0, |m| m.len()).max(1);
    let row = (count as f32).sqrt().ceil() as usize;
    let atlas = (row as f32 * t).ceil() as u32;
    // corners at the centres of the square's outer texels
    for uvc in &mut mesh.uvs {
        let cell = uvc[0].floor() as usize;
        let (su, sv) = ((uvc[0] - cell as f32) * 4.0, uvc[1]);
        let (cx, cy) = ((cell % row) as f32 * t, (cell / row) as f32 * t);
        *uvc = [(cx + 0.5 + su * (t - 1.0)) / atlas as f32, (cy + 0.5 + sv * (t - 1.0)) / atlas as f32];
    }

    // 5. paint: mesh to dataset coordinates, then each texel from the cameras that see it
    let offset = [x0 + nx as f32 / 2.0 * vox, ytop - (ny as f32 - 1.0) * vox, z0 + nz as f32 / 2.0 * vox];
    let mut texture = vec![[0u8; 3]; (atlas * atlas) as usize];
    let painted: Vec<(usize, [u8; 3])> = (0..quads)
        .into_par_iter()
        .flat_map_iter(|q| {
            let base = q * 4;
            // the quad's corners by their place in the square
            let mut corner = [[0.0_f32; 3]; 4];
            let mut origin = [f32::MAX; 2];
            for v in base..base + 4 {
                origin = [origin[0].min(mesh.uvs[v][0]), origin[1].min(mesh.uvs[v][1])];
            }
            let mut found = [false; 4];
            for v in base..base + 4 {
                let su = usize::from(mesh.uvs[v][0] > origin[0] + 0.5 / atlas as f32);
                let sv = usize::from(mesh.uvs[v][1] > origin[1] + 0.5 / atlas as f32);
                let slot = sv * 2 + su;
                corner[slot] = std::array::from_fn(|a| mesh.positions[v][a] + offset[a]);
                found[slot] = true;
            }
            let n = mesh.normals[base..base + 4].iter().fold([0.0_f32; 3], |s, n| [s[0] + n[0], s[1] + n[1], s[2] + n[2]]);
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
            let n = n.map(|v| v / l);
            let (px0, py0) = ((origin[0] * atlas as f32 - 0.5).round() as usize, (origin[1] * atlas as f32 - 0.5).round() as usize);
            let size = t as usize;
            let views = &views;
            (0..size * size).filter_map(move |s| {
                let (a, b) = ((s % size) as f32 / (t - 1.0), (s / size) as f32 / (t - 1.0));
                let point: [f32; 3] = if found.iter().all(|&f| f) {
                    std::array::from_fn(|k| {
                        let top = corner[0][k] * (1.0 - a) + corner[1][k] * a;
                        let bottom = corner[2][k] * (1.0 - a) + corner[3][k] * a;
                        top * (1.0 - b) + bottom * b
                    })
                } else {
                    std::array::from_fn(|k| corner.iter().map(|c| c[k]).sum::<f32>() / 4.0)
                };
                let (mut sum, mut wsum) = ([0.0_f32; 3], 0.0_f32);
                for v in views {
                    let pos = v.frame.0;
                    let d = [pos[0] - point[0], pos[1] - point[1], pos[2] - point[2]];
                    let dl = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-9);
                    let facing = (n[0] * d[0] + n[1] * d[1] + n[2] * d[2]) / dl;
                    if facing <= 0.05 {
                        continue;
                    }
                    if let Some((u, vv)) = v.sees(point, p.tolerance) {
                        let w = facing.powi(8);
                        let c = v.colour(u, vv);
                        for k in 0..3 {
                            sum[k] += w * c[k];
                        }
                        wsum += w;
                    }
                }
                (wsum > 0.0).then(|| {
                    let at = (py0 + s / size) * atlas as usize + px0 + s % size;
                    (at, sum.map(|c| (c / wsum).round().clamp(0.0, 255.0) as u8))
                })
            })
        })
        .collect();
    for (at, c) in painted {
        if at < texture.len() {
            texture[at] = c;
        }
    }
    let img = image::RgbImage::from_raw(atlas, atlas, texture.into_iter().flatten().collect()).ok_or("atlas")?;
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    let info = SurfaceInfo { opaque: opaque.len(), seen: oriented.len(), dims, quads, atlas };
    Ok((write_glb(&mesh, &png), info))
}
