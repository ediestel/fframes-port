//! Rigging, step 1: the parts of a standing figure that can move on their own, read from its
//! surface (a low-poly model as `depth` exports it: metres, y up, feet at the bottom).
//!
//! 1. Shell: the triangles are sampled into voxels of `cell` metres.
//! 2. Slices: every layer of voxels, seen from above, is a set of outlines; what they enclose
//!    is filled, and every connected area is one cross-section (a leg, the body, an arm that
//!    hangs free of the body).
//! 3. Parts: cross-sections that overlap from one layer to the next belong together; a run of
//!    single cross-sections that neither split nor merge is one part. Where two legs meet the
//!    body, or an arm leaves it, parts join.
//!
//! An arm that touches the body over its length shows as part of the body: that is what the
//! report measures before bones are placed.

use std::collections::HashMap;

/// One part: a run of cross-sections between two joins.
#[derive(Clone, Debug)]
pub struct Part {
    /// Lowest and highest layer, metres above the model's bottom.
    pub bottom: f32,
    pub top: f32,
    /// Mean centre of the cross-sections (x, z), metres.
    pub centre: [f32; 2],
    /// Mean cross-section, square metres.
    pub area: f32,
    /// Parts it joins below and above.
    pub below: Vec<usize>,
    pub above: Vec<usize>,
}

/// The parts of a figure.
#[derive(Clone, Debug)]
pub struct Parts {
    pub height: f32,
    pub cell: f32,
    pub parts: Vec<Part>,
    /// Cross-sections left out as too small (fewer than [`MIN_CELLS`]) or in parts too short.
    pub dropped: usize,
}

/// Smallest cross-section that counts, cells.
pub const MIN_CELLS: usize = 3;
/// Shortest part that counts at a tip (a part that joins on one side only), layers.
const MIN_LAYERS: usize = 3;

/// Positions and triangle indices of a binary glTF's first mesh primitive.
pub fn read_glb_mesh(data: &[u8]) -> Result<(Vec<[f32; 3]>, Vec<u32>), String> {
    let word = |o: usize| u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]) as usize;
    if data.len() < 28 || &data[..4] != b"glTF" {
        return Err("not a binary glTF".into());
    }
    let json_len = word(12);
    let doc: serde_json::Value = serde_json::from_slice(&data[20..20 + json_len]).map_err(|e| e.to_string())?;
    let bin = 20 + json_len + 8;
    let prim = &doc["meshes"][0]["primitives"][0];
    // byte offset, count, stride and component type of an accessor
    let access = |a: &serde_json::Value| -> Result<(usize, usize, usize, u64), String> {
        let acc = &doc["accessors"][a.as_u64().ok_or("no accessor")? as usize];
        let view = &doc["bufferViews"][acc["bufferView"].as_u64().ok_or("no buffer view")? as usize];
        let offset = bin + view["byteOffset"].as_u64().unwrap_or(0) as usize + acc["byteOffset"].as_u64().unwrap_or(0) as usize;
        Ok((offset, acc["count"].as_u64().unwrap_or(0) as usize, view["byteStride"].as_u64().unwrap_or(0) as usize, acc["componentType"].as_u64().unwrap_or(0)))
    };
    let (offset, count, stride, _) = access(&prim["attributes"]["POSITION"])?;
    let stride = if stride == 0 { 12 } else { stride };
    if offset + count * stride > data.len() {
        return Err("positions beyond the buffer".into());
    }
    let f = |o: usize| f32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]);
    let positions = (0..count).map(|i| std::array::from_fn(|c| f(offset + i * stride + c * 4))).collect();
    let (offset, count, _, kind) = access(&prim["indices"])?;
    let size = match kind {
        5125 => 4,
        5123 => 2,
        5121 => 1,
        k => return Err(format!("index type {k}")),
    };
    if offset + count * size > data.len() {
        return Err("indices beyond the buffer".into());
    }
    let indices = (0..count)
        .map(|i| {
            let o = offset + i * size;
            match size {
                4 => word(o) as u32,
                2 => u32::from(u16::from_le_bytes([data[o], data[o + 1]])),
                _ => u32::from(data[o]),
            }
        })
        .collect();
    Ok((positions, indices))
}

/// The parts of a standing figure, from its surface sampled in voxels of `cell` metres.
#[must_use]
pub fn parts(positions: &[[f32; 3]], indices: &[u32], cell: f32) -> Parts {
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in positions {
        for c in 0..3 {
            lo[c] = lo[c].min(p[c]);
            hi[c] = hi[c].max(p[c]);
        }
    }
    let dims: [usize; 3] = std::array::from_fn(|c| ((hi[c] - lo[c]).max(0.0) / cell) as usize + 3);
    let (nx, ny, nz) = (dims[0], dims[1], dims[2]);
    // 1. shell: every triangle sampled at half a cell
    let mut shell = vec![false; nx * ny * nz];
    let voxel = |p: [f32; 3]| -> usize {
        let v: [usize; 3] = std::array::from_fn(|c| (((p[c] - lo[c]) / cell) as usize + 1).min(dims[c] - 1));
        (v[1] * nz + v[2]) * nx + v[0]
    };
    for t in indices.chunks_exact(3) {
        let [a, b, c] = [t[0], t[1], t[2]].map(|i| positions[i as usize]);
        let edge = |p: [f32; 3], q: [f32; 3]| ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt();
        let n = ((edge(a, b).max(edge(b, c)).max(edge(c, a)) / (cell * 0.5)).ceil() as usize).max(1);
        for i in 0..=n {
            for j in 0..=n - i {
                let (u, v) = (i as f32 / n as f32, j as f32 / n as f32);
                shell[voxel(std::array::from_fn(|k| a[k] + (b[k] - a[k]) * u + (c[k] - a[k]) * v))] = true;
            }
        }
    }
    // 2. slices: per layer, the outlines' insides filled, connected areas labelled
    let layers: Vec<Vec<Vec<usize>>> = (0..ny).map(|y| sections(&shell[y * nx * nz..(y + 1) * nx * nz], nx, nz)).collect();
    let mut dropped = 0;
    let layers: Vec<Vec<Vec<usize>>> = layers
        .into_iter()
        .map(|l| {
            let (keep, small): (Vec<_>, Vec<_>) = l.into_iter().partition(|s| s.len() >= MIN_CELLS);
            dropped += small.len();
            keep
        })
        .collect();
    // 3. links between overlapping sections of neighbouring layers
    let node = |y: usize, s: usize| (y, s);
    let mut up: HashMap<(usize, usize), Vec<(usize, usize)>> = HashMap::new();
    let mut down: HashMap<(usize, usize), Vec<(usize, usize)>> = HashMap::new();
    for y in 0..ny.saturating_sub(1) {
        let mut owner = vec![usize::MAX; nx * nz];
        for (s, cells) in layers[y + 1].iter().enumerate() {
            for &c in cells {
                owner[c] = s;
            }
        }
        for (s, cells) in layers[y].iter().enumerate() {
            let mut met: Vec<usize> = cells.iter().map(|&c| owner[c]).filter(|&o| o != usize::MAX).collect();
            met.sort_unstable();
            met.dedup();
            for o in met {
                up.entry(node(y, s)).or_default().push(node(y + 1, o));
                down.entry(node(y + 1, o)).or_default().push(node(y, s));
            }
        }
    }
    // runs: a section continues the part below when it is the only one above that section
    // and that section the only one below it
    let mut part_of: HashMap<(usize, usize), usize> = HashMap::new();
    let mut runs: Vec<Vec<(usize, usize)>> = Vec::new();
    for (y, layer) in layers.iter().enumerate() {
        for s in 0..layer.len() {
            let here = node(y, s);
            let below = down.get(&here).map_or(&[][..], Vec::as_slice);
            let continues = match below {
                [b] => up.get(b).is_some_and(|u| u.len() == 1),
                _ => false,
            };
            if continues {
                let p = part_of[&below[0]];
                runs[p].push(here);
                part_of.insert(here, p);
            } else {
                part_of.insert(here, runs.len());
                runs.push(vec![here]);
            }
        }
    }
    let joins = |p: usize, map: &HashMap<(usize, usize), Vec<(usize, usize)>>, end: (usize, usize)| -> Vec<usize> {
        let _ = p;
        let mut v: Vec<usize> = map.get(&end).map_or_else(Vec::new, |n| n.iter().map(|x| part_of[x]).collect());
        v.sort_unstable();
        v.dedup();
        v
    };
    let mut below: Vec<Vec<usize>> = (0..runs.len()).map(|p| joins(p, &down, runs[p][0])).collect();
    let mut above: Vec<Vec<usize>> = (0..runs.len()).map(|p| joins(p, &up, runs[p][runs[p].len() - 1])).collect();
    // short tips are noise: dropped with their links
    let keep: Vec<bool> = (0..runs.len())
        .map(|p| runs[p].len() >= MIN_LAYERS || (!below[p].is_empty() && !above[p].is_empty()))
        .collect();
    let new_id: Vec<usize> = keep
        .iter()
        .scan(0, |n, &k| {
            let id = *n;
            *n += usize::from(k);
            Some(if k { id } else { usize::MAX })
        })
        .collect();
    for list in below.iter_mut().chain(above.iter_mut()) {
        *list = list.iter().filter(|&&q| keep[q]).map(|&q| new_id[q]).collect();
    }
    let mut out = Vec::new();
    for (p, run) in runs.iter().enumerate() {
        if !keep[p] {
            dropped += run.len();
            continue;
        }
        let (mut sx, mut sz, mut cells) = (0.0_f64, 0.0_f64, 0usize);
        for &(y, s) in run {
            for &c in &layers[y][s] {
                sx += (c % nx) as f64;
                sz += (c / nx) as f64;
            }
            cells += layers[y][s].len();
        }
        let at = |y: usize| (y as f32 - 0.5) * cell;
        let n = cells.max(1) as f64;
        out.push(Part {
            bottom: at(run[0].0).max(0.0),
            top: at(run[run.len() - 1].0 + 1).min(hi[1] - lo[1]),
            centre: [lo[0] + ((sx / n) as f32 - 0.5) * cell, lo[2] + ((sz / n) as f32 - 0.5) * cell],
            area: cells as f32 / run.len() as f32 * cell * cell,
            below: std::mem::take(&mut below[p]),
            above: std::mem::take(&mut above[p]),
        });
    }
    Parts { height: hi[1] - lo[1], cell, parts: out, dropped }
}

/// The filled, connected areas of one layer: the cells outside every outline are found from
/// the border (stepping sideways only, so a diagonal outline closes); the rest is grouped by
/// touching cells, diagonals included.
fn sections(shell: &[bool], nx: usize, nz: usize) -> Vec<Vec<usize>> {
    let mut outside = vec![false; nx * nz];
    let mut stack: Vec<usize> = (0..nx * nz)
        .filter(|&c| {
            let (x, z) = (c % nx, c / nx);
            (x == 0 || z == 0 || x == nx - 1 || z == nz - 1) && !shell[c]
        })
        .collect();
    for &c in &stack {
        outside[c] = true;
    }
    while let Some(c) = stack.pop() {
        let (x, z) = (c % nx, c / nx);
        for (dx, dz) in [(-1_i64, 0_i64), (1, 0), (0, -1), (0, 1)] {
            let (x2, z2) = (x as i64 + dx, z as i64 + dz);
            if x2 < 0 || z2 < 0 || x2 >= nx as i64 || z2 >= nz as i64 {
                continue;
            }
            let q = z2 as usize * nx + x2 as usize;
            if !outside[q] && !shell[q] {
                outside[q] = true;
                stack.push(q);
            }
        }
    }
    let mut seen = outside;
    let mut out = Vec::new();
    for start in 0..nx * nz {
        if seen[start] {
            continue;
        }
        seen[start] = true;
        let (mut cells, mut stack) = (Vec::new(), vec![start]);
        while let Some(c) = stack.pop() {
            cells.push(c);
            let (x, z) = (c % nx, c / nx);
            for dz in -1_i64..=1 {
                for dx in -1_i64..=1 {
                    let (x2, z2) = (x as i64 + dx, z as i64 + dz);
                    if x2 < 0 || z2 < 0 || x2 >= nx as i64 || z2 >= nz as i64 {
                        continue;
                    }
                    let q = z2 as usize * nx + x2 as usize;
                    if !seen[q] {
                        seen[q] = true;
                        stack.push(q);
                    }
                }
            }
        }
        out.push(cells);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A closed box's corners and triangles, appended to a mesh.
    fn add_box(pos: &mut Vec<[f32; 3]>, ind: &mut Vec<u32>, lo: [f32; 3], hi: [f32; 3]) {
        let base = pos.len() as u32;
        for k in 0..8 {
            pos.push([if k & 1 == 0 { lo[0] } else { hi[0] }, if k & 2 == 0 { lo[1] } else { hi[1] }, if k & 4 == 0 { lo[2] } else { hi[2] }]);
        }
        for f in [[0, 1, 3, 2], [4, 6, 7, 5], [0, 4, 5, 1], [2, 3, 7, 6], [0, 2, 6, 4], [1, 5, 7, 3]] {
            ind.extend([f[0], f[1], f[2], f[0], f[2], f[3]].map(|i| base + i));
        }
    }

    /// Two legs under a body with an arm hanging free beside it: the legs are two parts that
    /// join the body; the arm is its own part, joining the body at the shoulder.
    #[test]
    fn legs_body_and_a_free_arm() {
        let (mut pos, mut ind) = (Vec::new(), Vec::new());
        add_box(&mut pos, &mut ind, [-0.2, 0.0, -0.1], [-0.05, 0.8, 0.1]);
        add_box(&mut pos, &mut ind, [0.05, 0.0, -0.1], [0.2, 0.8, 0.1]);
        add_box(&mut pos, &mut ind, [-0.2, 0.8, -0.1], [0.2, 1.5, 0.1]);
        // the arm from the hip to the shoulder, 6 cm beside the body, joined at the top
        add_box(&mut pos, &mut ind, [0.26, 0.9, -0.05], [0.34, 1.4, 0.05]);
        add_box(&mut pos, &mut ind, [0.2, 1.4, -0.05], [0.34, 1.5, 0.05]);
        let p = parts(&pos, &ind, 0.02);
        for (i, part) in p.parts.iter().enumerate() {
            eprintln!("{i}: {:.2}-{:.2} m centre {:?} area {:.4} below {:?} above {:?}", part.bottom, part.top, part.centre, part.area, part.below, part.above);
        }
        let feet: Vec<&Part> = p.parts.iter().filter(|q| q.below.is_empty() && q.bottom < 0.05).collect();
        assert_eq!(feet.len(), 2, "two legs reach the floor");
        for leg in &feet {
            assert!((leg.top - 0.8).abs() < 0.05, "a leg ends at the hip: {:.2}", leg.top);
        }
        let arm = p.parts.iter().find(|q| q.centre[0] > 0.25).expect("the arm is a part of its own");
        assert!((arm.bottom - 0.9).abs() < 0.05 && arm.below.is_empty(), "the arm hangs free: {arm:?}");
    }
}
