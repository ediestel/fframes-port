//! glTF 2.0 export (`.glb`): the pieces as meshes for Blender, three.js, game engines and
//! `fframes::three_d`.
//!
//! Every piece outline is flattened to polygons (curves within `tolerance`), its rings are
//! sorted into outer contours and holes by nesting, and each contour with its holes is
//! triangulated (ear cutting). With `depth > 0` the pieces become tiles: a front face at
//! z = 0, a back face at z = -depth and side walls. One material per palette colour.
//!
//! Nodes follow the object names: `battle/french/knights` is the node `knights` under
//! `french` under `battle`; pieces of no object are in `_rest`. Each object node carries
//! one mesh with a primitive per colour, or with `per_piece` one child node per piece
//! (`p<id>`, with `extras.id` and `extras.color`) so pieces can move on their own.
//!
//! Coordinates: x to the right, y up, centred on the picture, `unit` metres per document
//! pixel (default: the picture is 2 m high).

use crate::document::PieceDocument;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::str::FromStr;
use usvgr::svgtree::svgrtypes::{Color, SimplePathSegment, SimplifyingPathParser};

#[derive(Clone, Debug)]
pub struct GltfParams {
    /// Largest distance of the polygons from the curves, in document pixels.
    pub tolerance: f32,
    /// Tile thickness in document pixels; 0 = flat pieces (double-sided).
    pub depth: f32,
    /// Metres per document pixel; `None` = the picture is 2 m high.
    pub unit: Option<f32>,
    /// One node per piece instead of one mesh per object.
    pub per_piece: bool,
    /// Leave out the outline colour (palette 0).
    pub skip_outline: bool,
    /// Only this object (and its sub-objects).
    pub group: Option<String>,
}

impl Default for GltfParams {
    fn default() -> Self {
        GltfParams {
            tolerance: 0.5,
            depth: 0.0,
            unit: None,
            per_piece: false,
            skip_outline: false,
            group: None,
        }
    }
}

type Ring = Vec<[f32; 2]>;

/// The rings (closed polylines) of SVG path data, curves flattened within `tol`.
fn rings(d: &str, tol: f32) -> Vec<Ring> {
    let mut out: Vec<Ring> = Vec::new();
    let mut cur: Ring = Vec::new();
    let mut last = [0f32; 2];
    let close = |cur: &mut Ring, out: &mut Vec<Ring>| {
        if cur.len() > 1 && cur.first() == cur.last() {
            cur.pop();
        }
        if cur.len() >= 3 {
            out.push(std::mem::take(cur));
        } else {
            cur.clear();
        }
    };
    let steps = |pts: &[[f32; 2]]| {
        // flatness: the largest distance of the control points from the chord
        let (a, b) = (pts[0], pts[pts.len() - 1]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        let dev = pts[1..pts.len() - 1]
            .iter()
            .map(|p| ((p[0] - a[0]) * dy - (p[1] - a[1]) * dx).abs() / len)
            .fold(0f32, f32::max);
        ((dev / tol.max(1e-3)).sqrt().ceil() as usize).clamp(1, 64)
    };
    for seg in SimplifyingPathParser::from(d) {
        let Ok(seg) = seg else { break };
        match seg {
            SimplePathSegment::MoveTo { x, y } => {
                close(&mut cur, &mut out);
                last = [x as f32, y as f32];
                cur.push(last);
            }
            SimplePathSegment::LineTo { x, y } => {
                last = [x as f32, y as f32];
                cur.push(last);
            }
            SimplePathSegment::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                let p = [
                    last,
                    [x1 as f32, y1 as f32],
                    [x2 as f32, y2 as f32],
                    [x as f32, y as f32],
                ];
                let n = steps(&p);
                for i in 1..=n {
                    let t = i as f32 / n as f32;
                    let u = 1.0 - t;
                    let (a, b, c, e) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    cur.push([
                        a * p[0][0] + b * p[1][0] + c * p[2][0] + e * p[3][0],
                        a * p[0][1] + b * p[1][1] + c * p[2][1] + e * p[3][1],
                    ]);
                }
                last = p[3];
            }
            SimplePathSegment::Quadratic { x1, y1, x, y } => {
                let p = [last, [x1 as f32, y1 as f32], [x as f32, y as f32]];
                let n = steps(&p);
                for i in 1..=n {
                    let t = i as f32 / n as f32;
                    let u = 1.0 - t;
                    cur.push([
                        u * u * p[0][0] + 2.0 * u * t * p[1][0] + t * t * p[2][0],
                        u * u * p[0][1] + 2.0 * u * t * p[1][1] + t * t * p[2][1],
                    ]);
                }
                last = p[2];
            }
            SimplePathSegment::ClosePath => close(&mut cur, &mut out),
        }
    }
    close(&mut cur, &mut out);
    out
}

fn signed_area(r: &Ring) -> f32 {
    let mut a = 0.0;
    for i in 0..r.len() {
        let (p, q) = (r[i], r[(i + 1) % r.len()]);
        a += p[0] * q[1] - q[0] * p[1];
    }
    a / 2.0
}

fn contains(r: &Ring, p: [f32; 2]) -> bool {
    let mut inside = false;
    let mut j = r.len() - 1;
    for i in 0..r.len() {
        let (a, b) = (r[i], r[j]);
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Contours with their holes: a ring nested in an even number of other rings is an outer
/// contour, in an odd number a hole of the smallest outer contour around it.
fn shapes(rings: Vec<Ring>) -> Vec<(Ring, Vec<Ring>)> {
    let areas: Vec<f32> = rings.iter().map(|r| signed_area(r).abs()).collect();
    let depth: Vec<usize> = (0..rings.len())
        .map(|i| {
            (0..rings.len())
                .filter(|&j| j != i && areas[j] > areas[i] && contains(&rings[j], rings[i][0]))
                .count()
        })
        .collect();
    let mut outers: Vec<usize> = (0..rings.len())
        .filter(|&i| depth[i].is_multiple_of(2))
        .collect();
    outers.sort_by(|&a, &b| areas[a].total_cmp(&areas[b]));
    let mut holes: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in (0..rings.len()).filter(|&i| depth[i] % 2 == 1) {
        if let Some(&o) = outers.iter().find(|&&o| {
            areas[o] > areas[i] && depth[o] + 1 == depth[i] && contains(&rings[o], rings[i][0])
        }) {
            holes.entry(o).or_default().push(i);
        }
    }
    let mut rings: Vec<Option<Ring>> = rings.into_iter().map(Some).collect();
    let mut out = Vec::new();
    for o in outers {
        let Some(outer) = rings[o].take() else {
            continue;
        };
        let hs = holes
            .get(&o)
            .map(|v| v.iter().filter_map(|&h| rings[h].take()).collect())
            .unwrap_or_default();
        out.push((outer, hs));
    }
    out
}

/// One primitive's data: positions, normals, indices.
#[derive(Default)]
struct Mesh {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    idx: Vec<u32>,
}

impl Mesh {
    /// Adds a contour with holes (document pixels) at `z`, with `to3` mapping to metres.
    fn add_shape(
        &mut self,
        outer: &Ring,
        holes: &[Ring],
        to3: &dyn Fn([f32; 2], f32) -> [f32; 3],
        depth: f32,
    ) {
        // y up: orient outer contours counter-clockwise, holes clockwise (in y-up space,
        // i.e. the opposite sign in the y-down document)
        let orient = |r: &Ring, ccw: bool| -> Ring {
            let a = -signed_area(r); // y-up area
            if (a > 0.0) == ccw {
                r.clone()
            } else {
                r.iter().rev().copied().collect()
            }
        };
        let outer = orient(outer, true);
        let holes: Vec<Ring> = holes.iter().map(|h| orient(h, false)).collect();
        let mut flat: Vec<f32> = Vec::new();
        let mut hole_at = Vec::new();
        let mut all: Vec<[f32; 2]> = Vec::new();
        for (k, r) in std::iter::once(&outer).chain(&holes).enumerate() {
            if k > 0 {
                hole_at.push(all.len());
            }
            for p in r {
                flat.extend_from_slice(&[p[0], -p[1]]);
                all.push(*p);
            }
        }
        let Ok(tris) = earcutr::earcut(&flat, &hole_at, 2) else {
            return;
        };
        let face = |m: &mut Mesh, z: f32, n: [f32; 3], flip: bool| {
            let base = m.pos.len() as u32;
            for p in &all {
                m.pos.push(to3(*p, z));
                m.nrm.push(n);
            }
            for t in tris.as_chunks::<3>().0 {
                let (a, b, c) = (t[0] as u32, t[1] as u32, t[2] as u32);
                // earcut works in (x, -y): its triangles come out in that space's winding
                let ccw = {
                    let (p, q, r) = (all[t[0]], all[t[1]], all[t[2]]);
                    (q[0] - p[0]) * (-(r[1]) + p[1]) - (-(q[1]) + p[1]) * (r[0] - p[0]) > 0.0
                };
                let (b, c) = if ccw == flip { (c, b) } else { (b, c) };
                m.idx.extend_from_slice(&[base + a, base + b, base + c]);
            }
        };
        face(self, 0.0, [0.0, 0.0, 1.0], false);
        if depth > 0.0 {
            face(self, -depth, [0.0, 0.0, -1.0], true);
            for r in std::iter::once(&outer).chain(&holes) {
                for i in 0..r.len() {
                    let (p, q) = (r[i], r[(i + 1) % r.len()]);
                    // y-up edge direction; outward normal of a ccw outer / cw hole edge
                    let (ex, ey) = (q[0] - p[0], -(q[1] - p[1]));
                    let len = (ex * ex + ey * ey).sqrt();
                    if len < 1e-6 {
                        continue;
                    }
                    let n = [ey / len, -ex / len, 0.0];
                    let base = self.pos.len() as u32;
                    for v in [to3(p, 0.0), to3(q, 0.0), to3(q, -depth), to3(p, -depth)] {
                        self.pos.push(v);
                        self.nrm.push(n);
                    }
                    self.idx.extend_from_slice(&[
                        base,
                        base + 3,
                        base + 2,
                        base,
                        base + 2,
                        base + 1,
                    ]);
                }
            }
        }
    }
}

fn srgb_to_linear(c: u8) -> f32 {
    let c = f32::from(c) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// A growing binary buffer with buffer views and accessors.
#[derive(Default)]
struct Bin {
    data: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl Bin {
    fn align(&mut self) {
        while !self.data.len().is_multiple_of(4) {
            self.data.push(0);
        }
    }
    fn vec3(&mut self, v: &[[f32; 3]], with_bounds: bool) -> usize {
        self.align();
        let off = self.data.len();
        for p in v {
            for c in p {
                self.data.extend_from_slice(&c.to_le_bytes());
            }
        }
        self.views.push(
            json!({"buffer": 0, "byteOffset": off, "byteLength": v.len() * 12, "target": 34962}),
        );
        let mut acc = json!({"bufferView": self.views.len() - 1, "componentType": 5126, "count": v.len(), "type": "VEC3"});
        if with_bounds {
            let mut lo = [f32::MAX; 3];
            let mut hi = [f32::MIN; 3];
            for p in v {
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
            acc["min"] = json!(lo);
            acc["max"] = json!(hi);
        }
        self.accessors.push(acc);
        self.accessors.len() - 1
    }
    fn indices(&mut self, v: &[u32]) -> usize {
        self.align();
        let off = self.data.len();
        for i in v {
            self.data.extend_from_slice(&i.to_le_bytes());
        }
        self.views.push(
            json!({"buffer": 0, "byteOffset": off, "byteLength": v.len() * 4, "target": 34963}),
        );
        self.accessors.push(json!({"bufferView": self.views.len() - 1, "componentType": 5125, "count": v.len(), "type": "SCALAR"}));
        self.accessors.len() - 1
    }
}

/// The document as a binary glTF (`.glb`).
#[must_use]
pub fn to_glb(doc: &PieceDocument, p: &GltfParams) -> Vec<u8> {
    let unit = p.unit.unwrap_or(2.0 / doc.height.max(1) as f32);
    let (cx, cy) = (doc.width as f32 / 2.0, doc.height as f32 / 2.0);
    let to3 = move |q: [f32; 2], z: f32| [(q[0] - cx) * unit, (cy - q[1]) * unit, z * unit];
    let depth = p.depth.max(0.0);

    // materials: one per palette colour, created on first use
    let mut material_of: BTreeMap<u16, usize> = BTreeMap::new();
    let mut materials: Vec<Value> = Vec::new();
    let mut material = |c: u16| -> usize {
        *material_of.entry(c).or_insert_with(|| {
            let hex = doc.palette.get(c as usize).map_or("#000000", String::as_str);
            let col = Color::from_str(hex).unwrap_or_else(|_| Color::black());
            materials.push(json!({
                "name": hex,
                "pbrMetallicRoughness": {
                    "baseColorFactor": [srgb_to_linear(col.red), srgb_to_linear(col.green), srgb_to_linear(col.blue), 1.0],
                    "metallicFactor": 0.0,
                    "roughnessFactor": 0.6
                },
                "doubleSided": depth == 0.0
            }));
            materials.len() - 1
        })
    };

    // object name -> (colour -> mesh), or per piece: (piece, colour, mesh)
    let mut by_group: BTreeMap<String, BTreeMap<u16, Mesh>> = BTreeMap::new();
    let mut pieces: BTreeMap<String, Vec<(u32, u16, Mesh)>> = BTreeMap::new();
    for piece in &doc.pieces {
        if p.skip_outline && piece.color == 0 {
            continue;
        }
        if let Some(g) = &p.group
            && !crate::group::in_group(&piece.group, g)
        {
            continue;
        }
        let name = if piece.group.is_empty() {
            "_rest".to_string()
        } else {
            piece.group.clone()
        };
        let mut mesh = Mesh::default();
        for (outer, holes) in shapes(rings(&piece.d, p.tolerance)) {
            mesh.add_shape(&outer, &holes, &to3, depth);
        }
        if mesh.idx.is_empty() {
            continue;
        }
        if p.per_piece {
            pieces
                .entry(name)
                .or_default()
                .push((piece.id, piece.color, mesh));
        } else {
            let m = by_group
                .entry(name)
                .or_default()
                .entry(piece.color)
                .or_default();
            let base = m.pos.len() as u32;
            m.pos.extend(mesh.pos);
            m.nrm.extend(mesh.nrm);
            m.idx.extend(mesh.idx.iter().map(|i| i + base));
        }
    }

    let mut bin = Bin::default();
    let mut meshes: Vec<Value> = Vec::new();
    let mut nodes: Vec<Value> = vec![json!({"name": "picture", "children": []})];
    // node index per object path ("battle", "battle/french", ...)
    let mut path_node: BTreeMap<String, usize> = BTreeMap::new();
    let mut node_for = |nodes: &mut Vec<Value>, name: &str| -> usize {
        let mut parent = 0usize;
        let mut path = String::new();
        for part in name.split('/') {
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(part);
            parent = *path_node.entry(path.clone()).or_insert_with(|| {
                nodes.push(json!({"name": part, "extras": {"group": path}}));
                let id = nodes.len() - 1;
                let kids = nodes[parent]
                    .as_object_mut()
                    .map(|o| o.entry("children").or_insert(json!([])));
                if let Some(Value::Array(k)) = kids {
                    k.push(json!(id));
                }
                id
            });
        }
        parent
    };
    let primitive = |bin: &mut Bin, m: &Mesh, mat: usize| {
        let pos = bin.vec3(&m.pos, true);
        let nrm = bin.vec3(&m.nrm, false);
        let idx = bin.indices(&m.idx);
        json!({"attributes": {"POSITION": pos, "NORMAL": nrm}, "indices": idx, "material": mat})
    };
    for (name, by_color) in &by_group {
        let prims: Vec<Value> = by_color
            .iter()
            .map(|(c, m)| primitive(&mut bin, m, material(*c)))
            .collect();
        meshes.push(json!({"name": name, "primitives": prims}));
        let n = node_for(&mut nodes, name);
        nodes[n]["mesh"] = json!(meshes.len() - 1);
    }
    for (name, list) in &pieces {
        let parent = node_for(&mut nodes, name);
        for (id, c, m) in list {
            let prim = primitive(&mut bin, m, material(*c));
            meshes.push(json!({"name": format!("p{id}"), "primitives": [prim]}));
            nodes.push(json!({"name": format!("p{id}"), "mesh": meshes.len() - 1, "extras": {"id": id, "color": c}}));
            let id_node = nodes.len() - 1;
            let kids = nodes[parent]
                .as_object_mut()
                .map(|o| o.entry("children").or_insert(json!([])));
            if let Some(Value::Array(k)) = kids {
                k.push(json!(id_node));
            }
        }
    }
    bin.align();
    let gltf = json!({
        "asset": {"version": "2.0", "generator": "fframes-vector"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": nodes,
        "meshes": meshes,
        "materials": materials,
        "accessors": bin.accessors,
        "bufferViews": bin.views,
        "buffers": [{"byteLength": bin.data.len()}],
        "extras": {"width": doc.width, "height": doc.height, "scale": doc.scale, "unit": unit}
    });
    let mut js = serde_json::to_vec(&gltf).unwrap_or_default();
    while !js.len().is_multiple_of(4) {
        js.push(b' ');
    }
    let total = 12 + 8 + js.len() + 8 + bin.data.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(js.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&js);
    out.extend_from_slice(&(bin.data.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&bin.data);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Piece;

    fn doc() -> PieceDocument {
        let sq = |id, color, x: f32, s: f32, group: &str| Piece {
            id,
            color,
            d: format!("M{x} 0L{} 0L{} {s}L{x} {s}Z", x + s, x + s),
            bbox: [x, 0.0, s, s],
            area: s * s,
            group: group.into(),
        };
        let mut ring = sq(3, 1, 60.0, 30.0, "rider/spurs");
        // a square with a square hole (the hole drawn the other way round, as potrace does)
        ring.d = "M60 0L90 0L90 30L60 30ZM70 10L70 20L80 20L80 10Z".into();
        PieceDocument {
            width: 100,
            height: 40,
            scale: 1,
            palette: vec!["#000000".into(), "#ff0000".into(), "#0000ff".into()],
            pieces: vec![sq(1, 1, 0.0, 20.0, "rider"), sq(2, 2, 30.0, 20.0, ""), ring],
        }
    }

    fn area_of(m: &::gltf::Document, buffers: &[::gltf::buffer::Data], mesh_name: &str) -> f32 {
        let mesh = m
            .meshes()
            .find(|x| x.name() == Some(mesh_name))
            .expect("mesh");
        let mut a = 0.0;
        for prim in mesh.primitives() {
            let r = prim.reader(|b| Some(&buffers[b.index()]));
            let pos: Vec<[f32; 3]> = r.read_positions().expect("positions").collect();
            let idx: Vec<u32> = r.read_indices().expect("indices").into_u32().collect();
            for t in idx.chunks(3) {
                let (p, q, s) = (pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]);
                // front faces only (z = 0), counted with their winding: ccw seen from +z
                if p[2] == 0.0 && q[2] == 0.0 && s[2] == 0.0 {
                    a += ((q[0] - p[0]) * (s[1] - p[1]) - (q[1] - p[1]) * (s[0] - p[0])) / 2.0;
                }
            }
        }
        a
    }

    #[test]
    fn flat_export_loads_with_nodes_holes_and_winding() {
        let glb = to_glb(
            &doc(),
            &GltfParams {
                unit: Some(1.0),
                ..GltfParams::default()
            },
        );
        let (m, buffers, _) = ::gltf::import_slice(&glb).expect("valid glb");
        let names: Vec<_> = m
            .nodes()
            .filter_map(|n| n.name().map(str::to_string))
            .collect();
        assert_eq!(names, ["picture", "_rest", "rider", "spurs"]);
        assert_eq!(m.materials().count(), 2);
        assert!((area_of(&m, &buffers, "rider") - 400.0).abs() < 1e-3);
        // 30 x 30 minus the 10 x 10 hole, all front faces facing +z
        assert!((area_of(&m, &buffers, "rider/spurs") - 800.0).abs() < 1e-3);
    }

    #[test]
    fn extruded_tiles_and_per_piece_nodes() {
        let p = GltfParams {
            unit: Some(1.0),
            depth: 4.0,
            per_piece: true,
            ..GltfParams::default()
        };
        let (m, buffers, _) = ::gltf::import_slice(to_glb(&doc(), &p)).expect("valid glb");
        assert!(m.nodes().any(|n| n.name() == Some("p3")));
        let mesh = m.meshes().find(|x| x.name() == Some("p1")).expect("p1");
        let prim = mesh.primitives().next().expect("primitive");
        let b = prim.bounding_box();
        assert_eq!((b.min[2], b.max[2]), (-4.0, 0.0));
        let r = prim.reader(|bf| Some(&buffers[bf.index()]));
        // front + back (2 triangles each) + 4 walls (2 each)
        assert_eq!(r.read_indices().expect("indices").into_u32().count(), 36);
    }
}
