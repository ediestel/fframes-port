//! Facial expressions as shapes. The face's own points are joints placed in the rig editor:
//! `mouth_l` and `mouth_r` at the corners of the mouth, `eye_l` and `eye_r` at the lower lids,
//! `cheek_l` and `cheek_r` on the cheekbones, `nose` at its tip. A library (the file
//! `expressions.json`, or one of the user's) describes FACS action units as regions around
//! those points that move, and expressions that mix the units. An expression's shape is every
//! vertex's offset at full strength, as a glTF morph target takes it (a Blender shape key, an
//! Unreal morph target).
//!
//! Sizes and moves are in mouth widths, so one library fits every face. Directions are the
//! face's own: `up` from the mouth to the eyes, `front` out of the face towards the nose tip,
//! `out` away from the face's middle (on each side its own way). Each region moves fully at
//! its point and fades to nothing at its radius, `(1 - (r / radius)²)²`.

use std::collections::{BTreeMap, HashMap};

use serde::Deserialize;

use crate::{Joint, V3, cross, dot, sub};

/// The library that comes with fframes.
pub const LIBRARY: &str = include_str!("../expressions.json");

/// The face points an expression needs, as joint names.
pub const POINTS: [&str; 7] = ["mouth_l", "mouth_r", "eye_l", "eye_r", "cheek_l", "cheek_r", "nose"];

/// Action units (each a list of moving regions), expressions (unit to strength) and the
/// teeth shown when the lips part.
#[derive(Clone, Debug, Deserialize)]
pub struct Library {
    pub units: BTreeMap<String, Vec<Region>>,
    pub expressions: BTreeMap<String, BTreeMap<String, f32>>,
    #[serde(default)]
    pub teeth: Option<Teeth>,
}

/// A region that moves: around the face points named `at` (`mouth` stands for `mouth_l`
/// and `mouth_r`), `radius` mouth widths far, by `up`, `front` and `out` mouth widths; with
/// `side` (`upper`, `lower`) only the points on that side of the mouth slit.
#[derive(Clone, Debug, Deserialize)]
pub struct Region {
    pub at: String,
    pub radius: f32,
    #[serde(default)]
    pub up: f32,
    #[serde(default)]
    pub front: f32,
    #[serde(default)]
    pub out: f32,
    #[serde(default)]
    pub side: Option<String>,
}

/// The arch of teeth behind the lips, in mouth widths: its `width` across, how far its ends
/// curve back (`depth`), how far its front lies `behind` the lips, the `height` of a tooth,
/// the teeth across (`count`) and the colours (linear RGB) of the teeth and of the mouth
/// cavity behind them.
#[derive(Clone, Debug, Deserialize)]
pub struct Teeth {
    pub width: f32,
    pub depth: f32,
    pub behind: f32,
    pub height: f32,
    pub count: usize,
    pub colour: [f32; 3],
    pub cavity: [f32; 3],
}

impl Library {
    pub fn parse(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| format!("expression library: {e}"))
    }
}

/// A face: its points (with the derived `fold_l` and `fold_r`, the nasolabial folds between a
/// mouth corner and the nose wing beside it, and `lips`, the middle of the mouth in front of
/// the line between its corners), its directions and its size (the mouth's width).
#[derive(Clone, Debug)]
pub struct Face {
    pub points: BTreeMap<String, V3>,
    pub up: V3,
    pub front: V3,
    /// Towards the `_r` points.
    pub side: V3,
    pub middle: V3,
    /// Halfway between the mouth corners: the mouth slit runs through it, square to `up`.
    pub mouth: V3,
    pub scale: f32,
}

/// How far the middle of the lips lies in front of the line between the mouth corners,
/// mouth widths.
const LIPS_FRONT: f32 = 0.12;

fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt().max(1e-12);
    a.map(|c| c / l)
}

fn mid(a: V3, b: V3) -> V3 {
    std::array::from_fn(|k| f32::midpoint(a[k], b[k]))
}

impl Face {
    /// The face from the rig's joints; an error names the face points still to be placed.
    pub fn from_joints(joints: &[Joint]) -> Result<Self, String> {
        let at = |n: &str| joints.iter().find(|j| j.name == n).and_then(Joint::position).map(|p| p.0);
        let missing: Vec<&str> = POINTS.iter().copied().filter(|n| at(n).is_none()).collect();
        if !missing.is_empty() {
            return Err(format!("face points not placed: {} (place them as joints in the rig editor)", missing.join(", ")));
        }
        let mut points: BTreeMap<String, V3> = POINTS.iter().map(|n| ((*n).to_string(), at(n).unwrap_or([0.0; 3]))).collect();
        let p = |n: &str| points[n];
        let (ml, mr, el, er, nose) = (p("mouth_l"), p("mouth_r"), p("eye_l"), p("eye_r"), p("nose"));
        let scale = dot(sub(mr, ml), sub(mr, ml)).sqrt();
        if scale < 1e-4 {
            return Err("the mouth corners lie on one another".into());
        }
        let (mouth, eyes) = (mid(ml, mr), mid(el, er));
        let side = norm(std::array::from_fn(|k| (mr[k] - ml[k]) + (er[k] - el[k])));
        let rise = sub(eyes, mouth);
        let up = norm(std::array::from_fn(|k| rise[k] - side[k] * dot(rise, side)));
        let middle = mid(mouth, eyes);
        let mut front = cross(side, up);
        if dot(sub(nose, middle), front) < 0.0 {
            front = front.map(|c| -c);
        }
        for (corner, s, name) in [(ml, -1.0, "fold_l"), (mr, 1.0, "fold_r")] {
            let wing: V3 = std::array::from_fn(|k| nose[k] + side[k] * s * 0.35 * scale - front[k] * 0.15 * scale);
            points.insert(name.into(), mid(corner, wing));
        }
        points.insert("lips".into(), std::array::from_fn(|k| mouth[k] + front[k] * LIPS_FRONT * scale));
        Ok(Face { points, up, front, side, middle, mouth, scale })
    }

    /// Away from the face's middle, at a point.
    fn out(&self, p: V3) -> V3 {
        if dot(sub(p, self.middle), self.side) >= 0.0 { self.side } else { self.side.map(|c| -c) }
    }
}

/// Every point's offset for the expression `name` of the library, at full strength.
pub fn expression(points: &[V3], face: &Face, library: &Library, name: &str) -> Result<Vec<V3>, String> {
    let mix = library.expressions.get(name).ok_or_else(|| format!("no expression {name} in the library"))?;
    let mut offsets = vec![[0.0_f32; 3]; points.len()];
    for (unit, &strength) in mix {
        let regions = library.units.get(unit).ok_or_else(|| format!("{name}: no action unit {unit} in the library"))?;
        for r in regions {
            let centres: Vec<V3> = face
                .points
                .iter()
                .filter(|(n, _)| **n == r.at || n.strip_suffix("_l").or_else(|| n.strip_suffix("_r")) == Some(r.at.as_str()))
                .map(|(_, p)| *p)
                .collect();
            if centres.is_empty() {
                return Err(format!("{unit}: no face point {}", r.at));
            }
            let reach = (r.radius * face.scale).max(1e-6);
            // above the mouth slit (the slit's own upper edge counts as above), or below it
            let above = |p: &V3| dot(sub(*p, face.mouth), face.up) >= 0.0;
            let keep: fn(bool) -> bool = match r.side.as_deref() {
                None => |_: bool| true,
                Some("upper") => |a: bool| a,
                Some("lower") => |a: bool| !a,
                Some(other) => return Err(format!("{unit}: side {other}; upper or lower")),
            };
            for c in centres {
                let o = face.out(c);
                let shift: V3 = std::array::from_fn(|k| (face.up[k] * r.up + face.front[k] * r.front + o[k] * r.out) * face.scale * strength);
                for (offset, p) in offsets.iter_mut().zip(points) {
                    if !keep(above(p)) {
                        continue;
                    }
                    let d = sub(*p, c);
                    let x = dot(d, d) / (reach * reach);
                    if x < 1.0 {
                        let f = (1.0 - x) * (1.0 - x);
                        for k in 0..3 {
                            offset[k] += shift[k] * f;
                        }
                    }
                }
            }
        }
    }
    Ok(offsets)
}

/// A mesh with the mouth slit cut: positions, triangles, and per vertex where it comes from
/// (`(a, b, t)`: `t` of the way from vertex `a` to `b`; an old vertex is `(i, i, 0)`), so any
/// attribute (UVs, normals) can follow; how many edges were cut.
#[derive(Clone, Debug)]
pub struct Slit {
    pub positions: Vec<V3>,
    pub sources: Vec<(u32, u32, f32)>,
    pub indices: Vec<u32>,
    pub cut: usize,
}

/// How far apart the two edges of the slit start, metres: enough for the side test, far too
/// little to see.
const SLIT_GAP: f32 = 1e-5;

/// The crossing of edge `a`-`b` with the slit, as two new vertices (above, below), made once
/// per edge.
fn crossing(slit: &mut Slit, edges: &mut HashMap<(u32, u32), (u32, u32)>, face: &Face, (a, sa): (u32, f32), (b, sb): (u32, f32)) -> (u32, u32) {
    let key = (a.min(b), a.max(b));
    if let Some(&c) = edges.get(&key) {
        return c;
    }
    let t = sa / (sa - sb);
    let (pa, pb) = (slit.positions[a as usize], slit.positions[b as usize]);
    let mut make = |shift: f32| {
        slit.positions.push(std::array::from_fn(|k| pa[k] + (pb[k] - pa[k]) * t + face.up[k] * shift));
        slit.sources.push((a, b, t));
        (slit.positions.len() - 1) as u32
    };
    let c = (make(SLIT_GAP), make(-SLIT_GAP));
    edges.insert(key, c);
    slit.cut += 1;
    c
}

/// Cuts the mesh along the mouth slit: the plane through the mouth corners square to `up`,
/// between the corners and on the front of the face. Every triangle there that the plane
/// crosses is split along it, and each point where it crosses an edge becomes two vertices,
/// one for the triangles above, one for those below, so the lips can part.
#[must_use]
pub fn mouth_slit(positions: &[V3], indices: &[u32], face: &Face) -> Slit {
    let level = |p: V3| dot(sub(p, face.mouth), face.up);
    let near = |p: V3| {
        let d = sub(p, face.mouth);
        dot(d, face.side).abs() <= 0.5 * face.scale && dot(d, face.front) > -0.3 * face.scale && level(p).abs() < 0.3 * face.scale
    };
    let mut slit = Slit {
        positions: positions.to_vec(),
        sources: (0..positions.len() as u32).map(|i| (i, i, 0.0)).collect(),
        indices: Vec::with_capacity(indices.len()),
        cut: 0,
    };
    let mut edges = HashMap::new();
    for t in indices.chunks_exact(3) {
        let v = [t[0], t[1], t[2]];
        let p = v.map(|i| positions[i as usize]);
        let s = p.map(level);
        let above = s.map(|x| x >= 0.0);
        if above.iter().all(|&a| a) || above.iter().all(|&a| !a) || !p.iter().all(|&q| near(q)) {
            slit.indices.extend_from_slice(&v);
            continue;
        }
        // the vertex alone on its side first, the order (and so the facing) kept
        let i = (0..3).find(|&i| above[i] != above[(i + 1) % 3] && above[i] != above[(i + 2) % 3]).unwrap_or(0);
        let (a, b, c) = (i, (i + 1) % 3, (i + 2) % 3);
        let ab = crossing(&mut slit, &mut edges, face, (v[a], s[a]), (v[b], s[b]));
        let ac = crossing(&mut slit, &mut edges, face, (v[a], s[a]), (v[c], s[c]));
        // its own copies for the lone vertex's side, the others' for the rest
        let (mine, theirs) = if above[a] { ((ab.0, ac.0), (ab.1, ac.1)) } else { ((ab.1, ac.1), (ab.0, ac.0)) };
        slit.indices.extend_from_slice(&[v[a], mine.0, mine.1, theirs.0, v[b], v[c], theirs.0, v[c], theirs.1]);
    }
    slit
}

/// A triangle mesh with normals.
#[derive(Clone, Debug, Default)]
pub struct Part {
    pub positions: Vec<V3>,
    pub normals: Vec<V3>,
    pub indices: Vec<u32>,
}

/// The teeth and the mouth cavity behind the lips. The teeth: an arch across the slit, the
/// upper row above it and the lower below, curving back towards its ends; each tooth rounded
/// at its front with a groove to the next. The cavity: a dark shell behind the arch, so the
/// open mouth shows no hole into the head.
#[must_use]
pub fn teeth(face: &Face, t: &Teeth) -> (Part, Part) {
    let w = face.scale;
    // across the arch (u from -1 to 1), up the teeth (metres): position and normal
    let at = |u: f32, height: f32, back: f32, bulge: f32| -> (V3, V3) {
        let lateral = u * t.width * w / 2.0;
        let front = (LIPS_FRONT - t.behind - t.depth * u * u - back) * w + bulge;
        let slope = -2.0 * t.depth * u / (t.width / 2.0);
        let n = norm(std::array::from_fn(|k| face.front[k] - face.side[k] * slope));
        (std::array::from_fn(|k| face.mouth[k] + face.side[k] * lateral + face.front[k] * front + face.up[k] * height), n)
    };
    let grid = |part: &mut Part, columns: usize, rows: &[f32], point: &dyn Fn(usize, f32) -> (V3, V3)| {
        let base = part.positions.len() as u32;
        for c in 0..=columns {
            for &r in rows {
                let (p, n) = point(c, r);
                part.positions.push(p);
                part.normals.push(n);
            }
        }
        let h = rows.len() as u32;
        for c in 0..columns as u32 {
            for r in 0..h - 1 {
                let (a, b) = (base + c * h + r, base + (c + 1) * h + r);
                part.indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
            }
        }
    };
    let per = 6;
    let columns = t.count.max(1) * per;
    let tooth = |c: usize, height: f32| {
        let u = c as f32 / columns as f32 * 2.0 - 1.0;
        let x = (c % per) as f32 / per as f32;
        at(u, height, 0.0, 0.012 * w * (std::f32::consts::PI * x).sin())
    };
    let mut teeth = Part::default();
    let upper: Vec<f32> = (0..5).map(|k| (-0.02 + (t.height + 0.02) * k as f32 / 4.0) * w).collect();
    let lower: Vec<f32> = (0..5).map(|k| (-0.85 * t.height + (0.85 * t.height - 0.03) * k as f32 / 4.0) * w).collect();
    grid(&mut teeth, columns, &upper, &tooth);
    grid(&mut teeth, columns, &lower, &tooth);
    let mut cavity = Part::default();
    let shell: Vec<f32> = (0..7).map(|k| (-0.35 + 0.65 * k as f32 / 6.0) * w).collect();
    grid(&mut cavity, 24, &shell, &|c, height| at(c as f32 / 12.0 - 1.0, height, 0.25, 0.0));
    (teeth, cavity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Observation, Preset};

    /// A face 1 m from the origin, looking along +z: mouth 5 cm wide at y 0, eyes 6 cm higher,
    /// cheekbones between and outside, the nose tip 2 cm in front.
    fn joints() -> Vec<Joint> {
        let plane = Preset::Transverse.plane(0.0);
        let at = |name: &str, p: V3| Joint {
            name: name.into(),
            parent: None,
            observations: vec![Observation { plane, offset: p[1], point: plane.to_2d(p) }],
            proposed: false,
        };
        vec![
            at("mouth_l", [-0.025, 0.0, 1.0]),
            at("mouth_r", [0.025, 0.0, 1.0]),
            at("eye_l", [-0.032, 0.06, 0.99]),
            at("eye_r", [0.032, 0.06, 0.99]),
            at("cheek_l", [-0.045, 0.035, 0.985]),
            at("cheek_r", [0.045, 0.035, 0.985]),
            at("nose", [0.0, 0.03, 1.02]),
        ]
    }

    /// The library parses; the smile lifts each mouth corner by about 0.15 mouth widths (0.16
    /// along the face's up, which leans back a little here) and pulls it outwards on its own
    /// side and back; nothing far from the face moves.
    #[test]
    fn smile_moves_the_mouth_corners() {
        let library = Library::parse(LIBRARY).expect("library");
        let face = Face::from_joints(&joints()).expect("face");
        assert!((face.scale - 0.05).abs() < 1e-5 && face.front[2] > 0.9 && face.up[1] > 0.9);
        let points = [[0.025, 0.0, 1.0], [-0.025, 0.0, 1.0], [0.0, 0.0, 0.9], [0.0, 0.3, 1.0]];
        let s = expression(&points, &face, &library, "smile").expect("smile");
        assert!(s[0][1] > 0.12 * 0.05 && s[0][1] < 0.25 * 0.05, "{:?}", s[0]);
        assert!(s[0][0] > 0.0 && s[1][0] < 0.0, "out on each side: {:?} {:?}", s[0], s[1]);
        assert!(s[0][2] < 0.0, "back: {:?}", s[0]);
        assert_eq!(s[2], [0.0; 3]);
        assert_eq!(s[3], [0.0; 3]);
        // a missing face point is named
        let err = Face::from_joints(&joints()[..6]).expect_err("no nose");
        assert!(err.contains("nose"), "{err}");
    }

    /// A flat face (a grid of triangles at z 1.0) cut along the slit: edges are cut, no
    /// triangle near the mouth reaches across the slit any more, and the lips parted by
    /// `smile_teeth` move apart there; the teeth lie behind the lips, within the mouth's width.
    #[test]
    fn slit_opens_and_teeth_sit_behind_the_lips() {
        let library = Library::parse(LIBRARY).expect("library");
        let face = Face::from_joints(&joints()).expect("face");
        let n = 21;
        let positions: Vec<V3> = (0..n * n).map(|i| [-0.05 + 0.1 * (i % n) as f32 / (n - 1) as f32, -0.043 + 0.1 * (i / n) as f32 / (n - 1) as f32, 1.0]).collect();
        let mut indices = Vec::new();
        for y in 0..n as u32 - 1 {
            for x in 0..n as u32 - 1 {
                let a = y * n as u32 + x;
                indices.extend_from_slice(&[a, a + 1, a + n as u32, a + 1, a + n as u32 + 1, a + n as u32]);
            }
        }
        let slit = mouth_slit(&positions, &indices, &face);
        assert!(slit.cut > 0 && slit.indices.len() > indices.len(), "{} cut", slit.cut);
        let level = |p: V3| dot(sub(p, face.mouth), face.up);
        for t in slit.indices.chunks_exact(3) {
            let p = [t[0], t[1], t[2]].map(|i| slit.positions[i as usize]);
            let side_ok = p.iter().all(|q| dot(sub(*q, face.mouth), face.side).abs() <= 0.5 * face.scale);
            if side_ok {
                let above = p.map(|q| level(q) >= 0.0);
                assert!(above.iter().all(|&a| a) || above.iter().all(|&a| !a), "a triangle across the slit");
            }
        }
        // the parted lips: the copies on both edges of the slit, in the middle of the mouth,
        // move apart
        let open = expression(&slit.positions, &face, &library, "smile_teeth").expect("smile_teeth");
        let upper = (positions.len()..slit.positions.len())
            .step_by(2)
            .min_by(|&a, &b| dot(sub(slit.positions[a], face.mouth), face.side).abs().total_cmp(&dot(sub(slit.positions[b], face.mouth), face.side).abs()))
            .expect("cut");
        let lower = upper + 1;
        assert!(open[upper][1] - open[lower][1] > 0.1 * face.scale, "{:?} {:?}", open[upper], open[lower]);
        let (teeth, cavity) = teeth(&face, library.teeth.as_ref().expect("teeth"));
        assert!(!teeth.indices.is_empty() && !cavity.indices.is_empty());
        let lips = dot(face.points["lips"], face.front);
        assert!(teeth.positions.iter().all(|p| dot(*p, face.front) < lips && dot(sub(*p, face.mouth), face.side).abs() <= 0.5 * face.scale + 1e-6));
    }
}
