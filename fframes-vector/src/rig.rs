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

/// Adds a skeleton with skin weights and morph targets to a binary glTF with one mesh
/// primitive (as `depth` exports it), its mesh, UVs and textures left as they are. The
/// skeleton: a node per joint (placed relative to its parent, not turned), a skin with the
/// inverse bind matrices, `JOINTS_0` and `WEIGHTS_0` on the primitive, the skin on every node
/// of the mesh, the skeleton's roots in the scene. Each morph target (an expression's offsets
/// per vertex) is named in the mesh's `extras.targetNames` and rests at weight 0. Blender
/// imports it as an armature with shape keys, Unreal as a skeletal mesh with morph targets.
pub fn export_rigged(data: &[u8], skinning: Option<&fframes_rig::Skinning>, morphs: &[(String, Vec<[f32; 3]>)]) -> Result<Vec<u8>, String> {
    use serde_json::{Value, json};
    let word = |o: usize| u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]) as usize;
    if data.len() < 28 || &data[..4] != b"glTF" {
        return Err("not a binary glTF".into());
    }
    let json_len = word(12);
    let mut doc: Value = serde_json::from_slice(&data[20..20 + json_len]).map_err(|e| e.to_string())?;
    let at = 20 + json_len;
    if data.len() < at + 8 || &data[at + 4..at + 8] != b"BIN\0" {
        return Err("no binary chunk".into());
    }
    let mut bin = data[at + 8..at + 8 + word(at)].to_vec();
    let primitives: usize = doc["meshes"].as_array().map_or(0, |m| m.iter().map(|m| m["primitives"].as_array().map_or(0, Vec::len)).sum());
    if primitives != 1 {
        return Err(format!("{primitives} mesh primitives; one expected"));
    }
    let position = doc["meshes"][0]["primitives"][0]["attributes"]["POSITION"].as_u64().ok_or("no positions")? as usize;
    let count = doc["accessors"][position]["count"].as_u64().unwrap_or(0) as usize;
    if let Some(s) = skinning {
        if count != s.influences.len() {
            return Err(format!("{count} vertices, {} weights", s.influences.len()));
        }
    }
    if let Some((name, offsets)) = morphs.iter().find(|(_, o)| o.len() != count) {
        return Err(format!("{count} vertices, {} offsets in {name}", offsets.len()));
    }
    // a buffer view (4-byte aligned) and an accessor over it
    let mut add = |doc: &mut Value, bytes: Vec<u8>, target: Option<u32>, accessor: Value| -> Result<usize, String> {
        while bin.len() % 4 != 0 {
            bin.push(0);
        }
        let mut view = json!({"buffer": 0, "byteOffset": bin.len(), "byteLength": bytes.len()});
        if let Some(t) = target {
            view["target"] = json!(t);
        }
        bin.extend(bytes);
        let views = doc["bufferViews"].as_array_mut().ok_or("no buffer views")?;
        views.push(view);
        let mut accessor = accessor;
        accessor["bufferView"] = json!(views.len() - 1);
        let accessors = doc["accessors"].as_array_mut().ok_or("no accessors")?;
        accessors.push(accessor);
        Ok(accessors.len() - 1)
    };
    if let Some(skinning) = skinning {
        let joints_bytes: Vec<u8> = skinning.influences.iter().flat_map(|inf| inf.iter().flat_map(|(j, _)| j.to_le_bytes())).collect();
        let weight_bytes: Vec<u8> = skinning.influences.iter().flat_map(|inf| inf.iter().flat_map(|(_, w)| w.to_le_bytes())).collect();
        // inverse bind matrices (column-major): the joints are not turned, so each takes its
        // position away
        let inverse_bind: Vec<u8> = skinning
            .joints
            .iter()
            .flat_map(|(_, _, p)| [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -p[0], -p[1], -p[2], 1.0_f32])
            .flat_map(f32::to_le_bytes)
            .collect();
        let n = skinning.joints.len();
        let joints_at = add(&mut doc, joints_bytes, Some(34962), json!({"componentType": 5123, "count": count, "type": "VEC4"}))?;
        let weights_at = add(&mut doc, weight_bytes, Some(34962), json!({"componentType": 5126, "count": count, "type": "VEC4"}))?;
        let bind_at = add(&mut doc, inverse_bind, None, json!({"componentType": 5126, "count": n, "type": "MAT4"}))?;
        // the mesh's nodes skinned, the joints as nodes, the skin, the skeleton in the scene
        let skin_index = doc["skins"].as_array().map_or(0, Vec::len);
        let nodes = doc["nodes"].as_array_mut().ok_or("no nodes")?;
        let base = nodes.len();
        for node in nodes.iter_mut() {
            if node.get("mesh").is_some() {
                node["skin"] = json!(skin_index);
            }
        }
        for (i, (name, parent, p)) in skinning.joints.iter().enumerate() {
            let from = parent.map_or([0.0; 3], |q| skinning.joints[q].2);
            let children: Vec<usize> = (0..n).filter(|&c| skinning.joints[c].1 == Some(i)).map(|c| base + c).collect();
            let mut node = json!({"name": name, "translation": [p[0] - from[0], p[1] - from[1], p[2] - from[2]]});
            if !children.is_empty() {
                node["children"] = json!(children);
            }
            nodes.push(node);
        }
        let roots: Vec<usize> = (0..n).filter(|&i| skinning.joints[i].1.is_none()).map(|i| base + i).collect();
        let skin = json!({
            "name": "rig",
            "joints": (base..base + n).collect::<Vec<_>>(),
            "inverseBindMatrices": bind_at,
            "skeleton": roots.first().copied().unwrap_or(base),
        });
        match doc["skins"].as_array_mut() {
            Some(skins) => skins.push(skin),
            None => doc["skins"] = json!([skin]),
        }
        let attributes = &mut doc["meshes"][0]["primitives"][0]["attributes"];
        attributes["JOINTS_0"] = json!(joints_at);
        attributes["WEIGHTS_0"] = json!(weights_at);
        let scene = doc["scene"].as_u64().unwrap_or(0) as usize;
        doc["scenes"][scene]["nodes"].as_array_mut().ok_or("no scene")?.extend(roots.iter().map(|&r| json!(r)));
    }
    // the morph targets: position offsets (with their bounds, as glTF requires), at rest
    if !morphs.is_empty() {
        let mut targets = Vec::new();
        for (_, offsets) in morphs {
            let bytes: Vec<u8> = offsets.iter().flat_map(|o| o.iter().flat_map(|v| v.to_le_bytes())).collect();
            let min: Vec<f32> = (0..3).map(|k| offsets.iter().map(|o| o[k]).fold(f32::MAX, f32::min)).collect();
            let max: Vec<f32> = (0..3).map(|k| offsets.iter().map(|o| o[k]).fold(f32::MIN, f32::max)).collect();
            let at = add(&mut doc, bytes, Some(34962), json!({"componentType": 5126, "count": count, "type": "VEC3", "min": min, "max": max}))?;
            targets.push(json!({"POSITION": at}));
        }
        let mesh = &mut doc["meshes"][0];
        mesh["primitives"][0]["targets"] = json!(targets);
        mesh["weights"] = json!(vec![0.0; morphs.len()]);
        mesh["extras"]["targetNames"] = json!(morphs.iter().map(|(name, _)| name).collect::<Vec<_>>());
    }
    // the binary glTF: header, JSON (padded with spaces), binary (padded with zeros)
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    doc["buffers"][0]["byteLength"] = json!(bin.len());
    let mut text = serde_json::to_vec(&doc).map_err(|e| e.to_string())?;
    while text.len() % 4 != 0 {
        text.push(b' ');
    }
    let total = 12 + 8 + text.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend(b"glTF");
    out.extend(2_u32.to_le_bytes());
    out.extend((total as u32).to_le_bytes());
    out.extend((text.len() as u32).to_le_bytes());
    out.extend(b"JSON");
    out.extend(text);
    out.extend((bin.len() as u32).to_le_bytes());
    out.extend(b"BIN\0");
    out.extend(bin);
    Ok(out)
}

/// A binary glTF's JSON and binary chunk.
fn glb_open(data: &[u8]) -> Result<(serde_json::Value, Vec<u8>), String> {
    let word = |o: usize| u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]) as usize;
    if data.len() < 28 || &data[..4] != b"glTF" {
        return Err("not a binary glTF".into());
    }
    let json_len = word(12);
    let doc = serde_json::from_slice(&data[20..20 + json_len]).map_err(|e| e.to_string())?;
    let at = 20 + json_len;
    if data.len() < at + 8 || &data[at + 4..at + 8] != b"BIN\0" {
        return Err("no binary chunk".into());
    }
    Ok((doc, data[at + 8..at + 8 + word(at)].to_vec()))
}

/// A binary glTF from its JSON and binary chunk.
fn glb_close(mut doc: serde_json::Value, mut bin: Vec<u8>) -> Result<Vec<u8>, String> {
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    doc["buffers"][0]["byteLength"] = serde_json::json!(bin.len());
    let mut text = serde_json::to_vec(&doc).map_err(|e| e.to_string())?;
    while text.len() % 4 != 0 {
        text.push(b' ');
    }
    let total = 12 + 8 + text.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend(b"glTF");
    out.extend(2_u32.to_le_bytes());
    out.extend((total as u32).to_le_bytes());
    out.extend((text.len() as u32).to_le_bytes());
    out.extend(b"JSON");
    out.extend(text);
    out.extend((bin.len() as u32).to_le_bytes());
    out.extend(b"BIN\0");
    out.extend(bin);
    Ok(out)
}

/// Appends `bytes` as a buffer view (4-byte aligned) with `accessor` over it; the accessor's
/// index.
fn glb_add(doc: &mut serde_json::Value, bin: &mut Vec<u8>, bytes: &[u8], target: Option<u32>, mut accessor: serde_json::Value) -> Result<usize, String> {
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let mut view = serde_json::json!({"buffer": 0, "byteOffset": bin.len(), "byteLength": bytes.len()});
    if let Some(t) = target {
        view["target"] = serde_json::json!(t);
    }
    bin.extend_from_slice(bytes);
    let views = doc["bufferViews"].as_array_mut().ok_or("no buffer views")?;
    views.push(view);
    accessor["bufferView"] = serde_json::json!(views.len() - 1);
    let accessors = doc["accessors"].as_array_mut().ok_or("no accessors")?;
    accessors.push(accessor);
    Ok(accessors.len() - 1)
}

/// A float accessor's values, `n` per element.
fn glb_floats(doc: &serde_json::Value, bin: &[u8], index: usize) -> Result<(Vec<f32>, usize), String> {
    let a = &doc["accessors"][index];
    if a["componentType"].as_u64() != Some(5126) {
        return Err(format!("accessor {index} is not float"));
    }
    let n = match a["type"].as_str() {
        Some("SCALAR") => 1,
        Some("VEC2") => 2,
        Some("VEC3") => 3,
        Some("VEC4") => 4,
        t => return Err(format!("accessor {index}: type {t:?}")),
    };
    let count = a["count"].as_u64().unwrap_or(0) as usize;
    let view = &doc["bufferViews"][a["bufferView"].as_u64().ok_or("no buffer view")? as usize];
    let start = view["byteOffset"].as_u64().unwrap_or(0) as usize + a["byteOffset"].as_u64().unwrap_or(0) as usize;
    let stride = view["byteStride"].as_u64().map_or(4 * n, |s| s as usize);
    if start + count.saturating_sub(1) * stride + 4 * n > bin.len() {
        return Err(format!("accessor {index} beyond the buffer"));
    }
    let f = |o: usize| f32::from_le_bytes([bin[o], bin[o + 1], bin[o + 2], bin[o + 3]]);
    Ok(((0..count).flat_map(|i| (0..n).map(move |c| (i, c))).map(|(i, c)| f(start + i * stride + 4 * c)).collect(), n))
}

/// The model with its mesh cut along the mouth slit: every float attribute (positions,
/// normals, tangents, texture coordinates) follows each vertex's sources, normals and
/// tangents kept unit length, so the texture stays whole across the cut; new triangles.
pub fn apply_slit(data: &[u8], slit: &fframes_rig::face::Slit) -> Result<Vec<u8>, String> {
    use serde_json::json;
    let (mut doc, mut bin) = glb_open(data)?;
    let attributes: Vec<(String, usize)> = doc["meshes"][0]["primitives"][0]["attributes"]
        .as_object()
        .ok_or("no attributes")?
        .iter()
        .filter_map(|(k, v)| v.as_u64().map(|i| (k.clone(), i as usize)))
        .collect();
    for (name, index) in attributes {
        let (values, n) = glb_floats(&doc, &bin, index)?;
        let mut out = Vec::with_capacity(slit.sources.len() * n);
        for &(a, b, t) in &slit.sources {
            let at = |i: u32, c: usize| values.get(i as usize * n + c).copied().unwrap_or(0.0);
            let mut v: Vec<f32> = (0..n).map(|c| at(a, c) + (at(b, c) - at(a, c)) * t).collect();
            if name == "NORMAL" || name == "TANGENT" {
                let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-12);
                v[..3].iter_mut().for_each(|c| *c /= l);
                if name == "TANGENT" {
                    v[3] = at(a, 3);
                }
            }
            out.extend(v);
        }
        let kind = ["SCALAR", "VEC2", "VEC3", "VEC4"][n - 1];
        let mut accessor = json!({"componentType": 5126, "count": slit.sources.len(), "type": kind});
        if name == "POSITION" {
            let bound = |pick: fn(f32, f32) -> f32, start: f32| -> Vec<f32> { (0..3).map(|c| out.iter().skip(c).step_by(3).copied().fold(start, pick)).collect() };
            accessor["min"] = json!(bound(f32::min, f32::MAX));
            accessor["max"] = json!(bound(f32::max, f32::MIN));
        }
        let bytes: Vec<u8> = out.iter().flat_map(|v| v.to_le_bytes()).collect();
        let at = glb_add(&mut doc, &mut bin, &bytes, Some(34962), accessor)?;
        doc["meshes"][0]["primitives"][0]["attributes"][&name] = json!(at);
    }
    let bytes: Vec<u8> = slit.indices.iter().flat_map(|i| i.to_le_bytes()).collect();
    let at = glb_add(&mut doc, &mut bin, &bytes, Some(34963), json!({"componentType": 5125, "count": slit.indices.len(), "type": "SCALAR"}))?;
    doc["meshes"][0]["primitives"][0]["indices"] = json!(at);
    glb_close(doc, bin)
}

/// Adds parts (the teeth, the mouth cavity) as one mesh of their own, a primitive each with a
/// plain colour (linear RGB) and roughness, on a node in the scene; with `joint` (of the skin
/// already in the model), wholly fixed to that joint.
pub fn add_parts(data: &[u8], parts: &[(&str, &fframes_rig::face::Part, [f32; 3], f32)], joint: Option<u16>) -> Result<Vec<u8>, String> {
    use serde_json::json;
    let (mut doc, mut bin) = glb_open(data)?;
    let mut primitives = Vec::new();
    for (name, part, colour, roughness) in parts {
        let count = part.positions.len();
        let floats = |v: &[[f32; 3]]| -> Vec<u8> { v.iter().flat_map(|p| p.iter().flat_map(|c| c.to_le_bytes())).collect() };
        let min: Vec<f32> = (0..3).map(|c| part.positions.iter().map(|p| p[c]).fold(f32::MAX, f32::min)).collect();
        let max: Vec<f32> = (0..3).map(|c| part.positions.iter().map(|p| p[c]).fold(f32::MIN, f32::max)).collect();
        let position = glb_add(&mut doc, &mut bin, &floats(&part.positions), Some(34962), json!({"componentType": 5126, "count": count, "type": "VEC3", "min": min, "max": max}))?;
        let normal = glb_add(&mut doc, &mut bin, &floats(&part.normals), Some(34962), json!({"componentType": 5126, "count": count, "type": "VEC3"}))?;
        let bytes: Vec<u8> = part.indices.iter().flat_map(|i| i.to_le_bytes()).collect();
        let indices = glb_add(&mut doc, &mut bin, &bytes, Some(34963), json!({"componentType": 5125, "count": part.indices.len(), "type": "SCALAR"}))?;
        let mut attributes = json!({"POSITION": position, "NORMAL": normal});
        if let Some(j) = joint {
            let joints: Vec<u8> = (0..count).flat_map(|_| [j, 0, 0, 0]).flat_map(u16::to_le_bytes).collect();
            let weights: Vec<u8> = (0..count).flat_map(|_| [1.0_f32, 0.0, 0.0, 0.0]).flat_map(f32::to_le_bytes).collect();
            attributes["JOINTS_0"] = json!(glb_add(&mut doc, &mut bin, &joints, Some(34962), json!({"componentType": 5123, "count": count, "type": "VEC4"}))?);
            attributes["WEIGHTS_0"] = json!(glb_add(&mut doc, &mut bin, &weights, Some(34962), json!({"componentType": 5126, "count": count, "type": "VEC4"}))?);
        }
        let materials = doc["materials"].as_array_mut().ok_or("no materials")?;
        materials.push(json!({
            "name": name,
            "doubleSided": true,
            "pbrMetallicRoughness": {"baseColorFactor": [colour[0], colour[1], colour[2], 1.0], "metallicFactor": 0.0, "roughnessFactor": roughness}
        }));
        primitives.push(json!({"attributes": attributes, "indices": indices, "material": materials.len() - 1}));
    }
    let meshes = doc["meshes"].as_array_mut().ok_or("no meshes")?;
    meshes.push(json!({"name": "mouth", "primitives": primitives}));
    let mesh = meshes.len() - 1;
    let mut node = json!({"name": "mouth", "mesh": mesh});
    if joint.is_some() {
        node["skin"] = json!(0);
    }
    let nodes = doc["nodes"].as_array_mut().ok_or("no nodes")?;
    nodes.push(node);
    let at = nodes.len() - 1;
    let scene = doc["scene"].as_u64().unwrap_or(0) as usize;
    doc["scenes"][scene]["nodes"].as_array_mut().ok_or("no scene")?.push(json!(at));
    glb_close(doc, bin)
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
