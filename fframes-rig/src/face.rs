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

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::{Joint, V3, cross, dot, sub};

/// The library that comes with fframes.
pub const LIBRARY: &str = include_str!("../expressions.json");

/// The face points an expression needs, as joint names.
pub const POINTS: [&str; 7] = ["mouth_l", "mouth_r", "eye_l", "eye_r", "cheek_l", "cheek_r", "nose"];

/// Action units (each a list of moving regions) and expressions (unit to strength).
#[derive(Clone, Debug, Deserialize)]
pub struct Library {
    pub units: BTreeMap<String, Vec<Region>>,
    pub expressions: BTreeMap<String, BTreeMap<String, f32>>,
}

/// A region that moves: around the face points named `at` (`mouth` stands for `mouth_l`
/// and `mouth_r`), `radius` mouth widths far, by `up`, `front` and `out` mouth widths.
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
}

impl Library {
    pub fn parse(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| format!("expression library: {e}"))
    }
}

/// A face: its points (with the derived `fold_l` and `fold_r`, the nasolabial folds between a
/// mouth corner and the nose wing beside it), its directions and its size (the mouth's width).
#[derive(Clone, Debug)]
pub struct Face {
    pub points: BTreeMap<String, V3>,
    pub up: V3,
    pub front: V3,
    /// Towards the `_r` points.
    pub side: V3,
    pub middle: V3,
    pub scale: f32,
}

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
        Ok(Face { points, up, front, side, middle, scale })
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
            for c in centres {
                let o = face.out(c);
                let shift: V3 = std::array::from_fn(|k| (face.up[k] * r.up + face.front[k] * r.front + o[k] * r.out) * face.scale * strength);
                for (offset, p) in offsets.iter_mut().zip(points) {
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
}
