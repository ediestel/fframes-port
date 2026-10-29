//! A figure's surface reduced to a vertex budget, with one texture baked from the frames it
//! was built from, for game engines (Unreal, Godot) and the web.
//!
//! 1. Weld: the surface's corners shared between faces.
//! 2. Simplify (meshoptimizer, quadric error) until the unwrapped model fits the budget.
//! 3. Unwrap (xatlas) into one square texture.
//! 4. Bake: every texel's surface point is painted from the frames that face it most directly
//!    and see it (a depth buffer of the reduced model per frame), inside the figure there; the
//!    best three blended by how directly they face it, so seams between them soften.
//! 5. Pad: empty texels take the nearest painted colour, so texture filtering never reaches
//!    the empty space between islands.
//!
//! The reduced model's shadow is measured against every frame like the full model's.

use crate::depth::{FrameFit, frame_fit};
use crate::figure::{Mesh, write_glb};
use rayon::prelude::*;

/// What the reduction gave.
#[derive(Clone, Debug)]
pub struct LowPoly {
    pub vertices: usize,
    pub triangles: usize,
    /// Texture edge, pixels.
    pub texture: u32,
    /// Shadow fit to the frames that shaped the figure, and to the held-out ones.
    pub fit_used: FrameFit,
    pub fit_held: FrameFit,
}

/// Where a world point (metres) shows in frame `f`: image column, depth towards the camera
/// (pixels, larger is nearer) and image row.
pub type Project<'a> = dyn Fn(usize, [f32; 3]) -> (f32, f32, f32) + Sync + 'a;

/// The frames, cleaned of screen spill, their figure masks and image size.
pub struct Views<'a> {
    pub frames: &'a [image::RgbaImage],
    pub masks: &'a [Vec<bool>],
    pub width: usize,
    pub height: usize,
    /// Direction from the figure to frame f's camera, (x, z) in world axes.
    pub to_camera: &'a [[f32; 2]],
    pub held: &'a (dyn Fn(usize) -> bool + Sync),
}

const TEXTURE: u32 = 2048;
const PADDING: u32 = 4;

/// The reduced, textured model as a binary glTF.
pub(crate) fn low_poly_glb(mesh: &Mesh, budget: usize, views: &Views<'_>, project: &Project<'_>) -> Result<(Vec<u8>, LowPoly), String> {
    // 1. weld: equal positions become one vertex
    let mut index_of: std::collections::HashMap<[i64; 3], u32> = std::collections::HashMap::new();
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let key = |p: [f32; 3]| p.map(|c| (f64::from(c) * 1e5).round() as i64);
    let welded: Vec<u32> = mesh
        .indices
        .iter()
        .map(|&i| {
            let p = mesh.positions[i as usize];
            *index_of.entry(key(p)).or_insert_with(|| {
                positions.push(p);
                positions.len() as u32 - 1
            })
        })
        .collect();
    let welded: Vec<u32> = welded
        .chunks(3)
        .filter(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2])
        .flatten()
        .copied()
        .collect();

    // 2 and 3: simplify, unwrap; fewer triangles until the unwrapped vertices fit the budget
    let bytes = meshopt::typed_to_bytes(&positions);
    let adapter = meshopt::VertexDataAdapter::new(bytes, 12, 0).map_err(|e| e.to_string())?;
    // a closed surface has about twice as many triangles as vertices; seams add some vertices
    let mut triangles = budget * 2 * 85 / 100;
    for _ in 0..8 {
        let mut indices = meshopt::simplify(&welded, &adapter, triangles * 3, 1.0, meshopt::SimplifyOptions::None, None);
        let compact = meshopt::optimize_vertex_fetch(&mut indices, &positions);
        let unwrapped = unwrap(&compact, &indices)?;
        if unwrapped.positions.len() <= budget || triangles < 100 {
            return bake(&unwrapped, views, project);
        }
        triangles = triangles * budget / unwrapped.positions.len() * 95 / 100;
    }
    Err(format!("could not reduce to {budget} vertices"))
}

/// A reduced mesh with texture coordinates in texels.
struct Unwrapped {
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
    size: (u32, u32),
}

fn unwrap(positions: &[[f32; 3]], indices: &[u32]) -> Result<Unwrapped, String> {
    let flat: Vec<f32> = positions.iter().flatten().copied().collect();
    let decl = xatlas_rs_v2::MeshDecl {
        vertex_position_data: xatlas_rs_v2::MeshData::Contiguous(&flat),
        index_data: Some(xatlas_rs_v2::IndexData::U32(indices)),
        face_count: (indices.len() / 3) as u32,
        ..Default::default()
    };
    let mut atlas = xatlas_rs_v2::Xatlas::new();
    atlas.add_mesh(&decl).map_err(|e| format!("unwrap: {e:?}"))?;
    let pack = xatlas_rs_v2::PackOptions { resolution: TEXTURE, padding: PADDING, ..Default::default() };
    atlas.generate(&xatlas_rs_v2::ChartOptions::default(), &pack);
    let size = (atlas.width(), atlas.height());
    let meshes = atlas.meshes();
    let m = meshes.first().ok_or("unwrap: no mesh")?;
    Ok(Unwrapped {
        positions: m.vertex_array.iter().map(|v| positions[v.xref as usize]).collect(),
        uvs: m.vertex_array.iter().map(|v| v.uv).collect(),
        indices: m.index_array.to_vec(),
        size,
    })
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn unit(a: [f32; 3]) -> [f32; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-12);
    a.map(|c| c / l)
}

/// Visits the pixel centres (integer coordinates) inside a 2D triangle with their
/// barycentric weights.
fn raster(p: [[f32; 2]; 3], w: usize, h: usize, mut visit: impl FnMut(usize, usize, [f32; 3])) {
    let area = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]);
    if area.abs() < 1e-9 {
        return;
    }
    let x0 = p.iter().map(|q| q[0]).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
    let x1 = (p.iter().map(|q| q[0]).fold(f32::MIN, f32::max).ceil().max(0.0) as usize).min(w.saturating_sub(1));
    let y0 = p.iter().map(|q| q[1]).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
    let y1 = (p.iter().map(|q| q[1]).fold(f32::MIN, f32::max).ceil().max(0.0) as usize).min(h.saturating_sub(1));
    for y in y0..=y1 {
        for x in x0..=x1 {
            let (px, py) = (x as f32, y as f32);
            let b1 = ((px - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (py - p[0][1])) / area;
            let b2 = ((p[1][0] - p[0][0]) * (py - p[0][1]) - (px - p[0][0]) * (p[1][1] - p[0][1])) / area;
            let b0 = 1.0 - b1 - b2;
            if b0 >= -1e-4 && b1 >= -1e-4 && b2 >= -1e-4 {
                visit(x, y, [b0, b1, b2]);
            }
        }
    }
}

fn bake(m: &Unwrapped, views: &Views<'_>, project: &Project<'_>) -> Result<(Vec<u8>, LowPoly), String> {
    let (w, h) = (views.width, views.height);
    let n = views.frames.len();
    let tris: Vec<[usize; 3]> = m.indices.chunks(3).map(|t| [t[0] as usize, t[1] as usize, t[2] as usize]).collect();

    // a depth buffer of the reduced model in every frame: nearest surface per pixel
    let zbuffers: Vec<Vec<f32>> = (0..n)
        .into_par_iter()
        .map(|f| {
            let mut z = vec![f32::NEG_INFINITY; w * h];
            let at: Vec<(f32, f32, f32)> = m.positions.iter().map(|&p| project(f, p)).collect();
            for t in &tris {
                let q = t.map(|i| at[i]);
                raster([[q[0].0, q[0].2], [q[1].0, q[1].2], [q[2].0, q[2].2]], w, h, |x, y, b| {
                    let d = b[0] * q[0].1 + b[1] * q[1].1 + b[2] * q[2].1;
                    let cell = &mut z[y * w + x];
                    if d > *cell {
                        *cell = d;
                    }
                });
            }
            z
        })
        .collect();

    // the reduced model's fit, from the same depth buffers (its shadow)
    let fits: Vec<(bool, FrameFit)> = (0..n)
        .into_par_iter()
        .map(|f| {
            let shadow: Vec<bool> = zbuffers[f].iter().map(|d| d.is_finite()).collect();
            ((views.held)(f), frame_fit(&shadow, &views.masks[f], w, h))
        })
        .collect();
    let average = |which: bool| {
        let chosen: Vec<&FrameFit> = fits.iter().filter(|(hf, _)| *hf == which).map(|(_, x)| x).collect();
        FrameFit::mean(&chosen)
    };

    // 4. bake, triangle by triangle in texture space
    let (tw, th) = (m.size.0.max(1) as usize, m.size.1.max(1) as usize);
    let painted: Vec<Vec<(usize, [u8; 3])>> = tris
        .par_iter()
        .map(|t| {
            let p = t.map(|i| m.positions[i]);
            let normal = unit(cross(sub(p[1], p[0]), sub(p[2], p[0])));
            // frames by how directly they face this triangle
            let mut order: Vec<(f32, usize)> = (0..n)
                .map(|f| (normal[0] * views.to_camera[f][0] + normal[2] * views.to_camera[f][1], f))
                .filter(|(facing, _)| *facing > 0.15)
                .collect();
            order.sort_by(|a, b| b.0.total_cmp(&a.0));
            let uv = t.map(|i| m.uvs[i]);
            let mut out = Vec::new();
            raster(uv, tw, th, |x, y, b| {
                let point = [0, 1, 2].map(|c| b[0] * p[0][c] + b[1] * p[1][c] + b[2] * p[2][c]);
                let (mut sum, mut weight) = ([0.0_f32; 3], 0.0_f32);
                let mut used = 0;
                for &(facing, f) in &order {
                    let (col, depth, row) = project(f, point);
                    let (ci, ri) = (col.round(), row.round());
                    if ci < 1.0 || ri < 0.0 || ci as usize + 1 >= w || ri as usize >= h {
                        continue;
                    }
                    let (ci, ri) = (ci as usize, ri as usize);
                    let mask = &views.masks[f][ri * w..(ri + 1) * w];
                    // seen (nothing of the model nearer) and inside the figure, a pixel from its edge
                    if depth < zbuffers[f][ri * w + ci] - 2.0 || !(mask[ci - 1] && mask[ci] && mask[ci + 1]) {
                        continue;
                    }
                    let q = views.frames[f].get_pixel(ci as u32, ri as u32).0;
                    let k = facing.powi(4);
                    for c in 0..3 {
                        sum[c] += k * f32::from(q[c]);
                    }
                    weight += k;
                    used += 1;
                    if used == 3 {
                        break;
                    }
                }
                if weight > 0.0 {
                    out.push((y * tw + x, sum.map(|s| (s / weight).round().clamp(0.0, 255.0) as u8)));
                }
            });
            out
        })
        .collect();
    let mut texture = image::RgbaImage::new(tw as u32, th as u32);
    let mut filled = vec![false; tw * th];
    for (i, c) in painted.into_iter().flatten() {
        texture.put_pixel((i % tw) as u32, (i / tw) as u32, image::Rgba([c[0], c[1], c[2], 255]));
        filled[i] = true;
    }
    // 5. pad: empty texels take their painted neighbours' colour, ring by ring
    for _ in 0..(PADDING * 4) {
        let before = filled.clone();
        let mut any = false;
        for y in 0..th {
            for x in 0..tw {
                if before[y * tw + x] {
                    continue;
                }
                let (mut sum, mut k) = ([0u32; 3], 0u32);
                for (dx, dy) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
                    let (qx, qy) = (x as i64 + dx, y as i64 + dy);
                    if qx >= 0 && qy >= 0 && (qx as usize) < tw && (qy as usize) < th && before[qy as usize * tw + qx as usize] {
                        let q = texture.get_pixel(qx as u32, qy as u32).0;
                        for c in 0..3 {
                            sum[c] += u32::from(q[c]);
                        }
                        k += 1;
                    }
                }
                if k > 0 {
                    texture.put_pixel(x as u32, y as u32, image::Rgba([(sum[0] / k) as u8, (sum[1] / k) as u8, (sum[2] / k) as u8, 255]));
                    filled[y * tw + x] = true;
                    any = true;
                }
            }
        }
        if !any {
            break;
        }
    }
    let mut png = Vec::new();
    texture
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;

    // smooth normals from the triangles, weighted by area
    let mut normals = vec![[0.0_f32; 3]; m.positions.len()];
    for t in &tris {
        let p = t.map(|i| m.positions[i]);
        let a = cross(sub(p[1], p[0]), sub(p[2], p[0]));
        for &i in t {
            for c in 0..3 {
                normals[i][c] += a[c];
            }
        }
    }
    let out = Mesh {
        positions: m.positions.clone(),
        normals: normals.into_iter().map(unit).collect(),
        uvs: m.uvs.iter().map(|uv| [uv[0] / tw as f32, uv[1] / th as f32]).collect(),
        indices: m.indices.clone(),
    };
    Ok((
        write_glb(&out, &png),
        LowPoly {
            vertices: out.positions.len(),
            triangles: tris.len(),
            texture: tw.max(th) as u32,
            fit_used: average(false),
            fit_held: average(true),
        },
    ))
}
