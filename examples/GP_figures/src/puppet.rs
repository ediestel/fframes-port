//! The front figure of `thieme_atlas_style_reference_sheet.png`, the original
//! the rig libraries were made from, as a posable rig. `trace_reference.py`
//! cuts the figure's pixels at its joints and traces each piece into
//! `reference_rig.svg` (nested `<g id data-pivot>`, image pixel coordinates).
//! Here each piece turns about its pivot (forward kinematics), a disc as wide
//! as the joint in the drawing closes the joint, and all of it is merged by a
//! Boolean union into one silhouette, drawn in the original's fill and outline.
//!
//! The figure is seen from the front; its right side is on the image's left
//! (`_r`). Angles are degrees in the SVG sense (positive is clockwise on
//! screen): moving the right arm or leg outwards is positive, the left one
//! negative.

use fframes::Svgr;
use fframes::animation::Easing;
use fframes::params::{Key, KeyEasing, Track};
use geo::{BooleanOps, Coord, LineString, MultiPolygon, Polygon};
use std::collections::BTreeMap;
use std::sync::{LazyLock, OnceLock};

const RIG: &str = include_str!("../reference_rig.svg");
const POSES_TOML: &str = include_str!("../poses.toml");

/// Rig point drawn at the (x, y) given to `draw`.
pub const ORIGIN: (f32, f32) = (-15.6, 70.0);

/// Fill and outline of the figure, sampled from the original (torso at
/// (193, 380); outline at (82, 380), about 3 px wide).
pub const FILL: &str = "#92AAC2";
pub const INK: &str = "#242B31";
pub const LINE: f32 = 3.0;

type Pt = (f64, f64);

#[derive(Debug)]
struct Seg {
    id: String,
    pivot: Pt,
    rings: Vec<Vec<Pt>>,
    children: Vec<Seg>,
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(r#" {name}=""#);
    let at = tag.find(&key)? + key.len();
    Some(&tag[at..at + tag[at..].find('"')?])
}

/// The rings of an absolute M/L/Z path, as `trace_reference.py` writes them.
fn rings(d: &str) -> Vec<Vec<Pt>> {
    let toks: Vec<&str> = d.split([' ', ',']).filter(|t| !t.is_empty()).collect();
    let num = |i: usize| toks.get(i).and_then(|t| t.parse::<f64>().ok()).unwrap_or(0.0);
    let (mut out, mut ring, mut i) = (vec![], vec![], 0);
    while i < toks.len() {
        match toks[i] {
            "M" | "L" => {
                if toks[i] == "M" && ring.len() > 2 {
                    out.push(std::mem::take(&mut ring));
                }
                ring.push((num(i + 1), num(i + 2)));
                i += 3;
            }
            "Z" => {
                if ring.len() > 2 {
                    out.push(std::mem::take(&mut ring));
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    out
}

fn disc((x, y): Pt, r: f64) -> Vec<Pt> {
    (0..32).map(|k| (k as f64 / 32.0 * std::f64::consts::TAU).sin_cos()).map(|(s, c)| (x + r * c, y + r * s)).collect()
}

/// Disc closing the joint at a segment's own pivot: centre and radius, kept
/// inside the drawn outline in the rest pose so it never shows as a bump.
/// Widths in the drawing: neck 47 px, upper arm 36, elbow 40, wrist 27, top
/// of the thigh 60, knee 43, ankle 31. The shoulder disc sits 7 px in from
/// the pivot towards the arm's middle; at the pivot it would stand 7 px out
/// of the shoulder's outer edge (x 102 at row 232).
fn joint_disc(id: &str, (x, y): Pt) -> Option<(Pt, f64)> {
    let side = if id.ends_with("_r") { 1.0 } else { -1.0 };
    match id.trim_end_matches("_r").trim_end_matches("_l") {
        "head" => Some(((x, y), 23.0)),
        "upper_arm" => Some(((x + 7.0 * side, y + 13.0), 17.0)),
        "lower_arm" => Some(((x, y), 18.0)),
        "hand" => Some(((x, y), 13.0)),
        "thigh" => Some(((x, y), 30.0)),
        "shin" => Some(((x, y), 20.0)),
        "foot" => Some(((x, y), 14.0)),
        _ => None,
    }
}

/// `ring` resampled at 1 px spacing and run twice through a 5-point moving
/// average: takes out bumps, ledges and pixel steps of a few pixels where the
/// pieces meet, keeps the shape and the fingers.
fn smooth(ring: &[Pt]) -> Vec<Pt> {
    let mut pts: Vec<Pt> = vec![];
    for (k, &a) in ring.iter().enumerate() {
        let b = ring[(k + 1) % ring.len()];
        let n = (((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt().ceil() as usize).max(1);
        pts.extend((0..n).map(|i| {
            let t = i as f64 / n as f64;
            (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
        }));
    }
    let len = pts.len();
    if len < 8 {
        return ring.to_vec();
    }
    for _ in 0..2 {
        pts = (0..len)
            .map(|i| {
                let s = (0..5).map(|k| pts[(i + len + k - 2) % len]).fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
                (s.0 / 5.0, s.1 / 5.0)
            })
            .collect();
    }
    pts
}

/// A traced rig: nested `<g id data-pivot>` groups of absolute M/L/Z rings.
#[derive(Debug)]
pub struct Rig(Vec<Seg>);

impl Rig {
    /// `front` adds the joint and torso discs of `reference_rig.svg`; other
    /// rigs (`trace_side_rigs.py`) carry their discs as rings of their own.
    pub fn parse(src: &str, front: bool) -> Rig {
        Rig(parse(src, front))
    }

    /// The ids of every segment, parents before children.
    pub fn ids(&self) -> Vec<&str> {
        fn walk<'s>(seg: &'s Seg, out: &mut Vec<&'s str>) {
            out.push(&seg.id);
            for child in &seg.children {
                walk(child, out);
            }
        }
        let mut out = Vec::new();
        for seg in &self.0 {
            walk(seg, &mut out);
        }
        out
    }

    /// Outline of the rig posed by `angle(id)` (degrees, clockwise on screen):
    /// one smoothed path of the union of all pieces, in rig coordinates.
    pub fn outline(&self, angle: &dyn Fn(&str) -> f64) -> String {
        let mut all = vec![];
        for seg in &self.0 {
            collect(seg, &|p| p, angle, &mut all);
        }
        to_d(&union(all))
    }
}

fn parse(src: &str, front: bool) -> Vec<Seg> {
    // open groups: (id, pivot, rings, children); the bottom entry collects the root
    let mut stack: Vec<(String, Pt, Vec<Vec<Pt>>, Vec<Seg>)> = vec![(String::new(), (0.0, 0.0), vec![], vec![])];
    let mut rest = src;
    while let Some(i) = rest.find('<') {
        rest = &rest[i..];
        let end = rest.find('>').unwrap_or(rest.len() - 1);
        let tag = &rest[..end];
        rest = &rest[end + 1..];
        if tag.starts_with("<g ") {
            let pivot = attr(tag, "data-pivot")
                .and_then(|p| {
                    let (x, y) = p.split_once(',')?;
                    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
                })
                .unwrap_or((0.0, 0.0));
            stack.push((attr(tag, "id").unwrap_or("").to_string(), pivot, vec![], vec![]));
        } else if tag.starts_with("</g") && stack.len() > 1 {
            let (id, pivot, mut rs, children) = stack.pop().unwrap_or_default();
            if let Some((centre, r)) = joint_disc(&id, pivot).filter(|_| front) {
                rs.push(disc(centre, r));
            }
            if front && id == "torso" {
                // the torso's own corners where the arms and legs were cut off
                // (shoulder tops near (128, 212) and (259, 212), the crotch at
                // (193.5, 480)) stand out as points once a limb swings away;
                // these discs round them and lie inside the drawing at rest
                rs.extend([disc((124.0, 232.0), 20.0), disc((263.0, 232.0), 20.0), disc((193.5, 468.0), 12.0)]);
            }
            if let Some(top) = stack.last_mut() {
                top.3.push(Seg { id, pivot, rings: rs, children });
            }
        } else if tag.starts_with("<path ") {
            let d = attr(tag, "d").unwrap_or("");
            if let Some(top) = stack.last_mut() {
                top.2.extend(rings(d));
            }
        }
    }
    stack.pop().map(|s| s.3).unwrap_or_default()
}

fn rig() -> &'static Rig {
    static RIG_TREE: OnceLock<Rig> = OnceLock::new();
    RIG_TREE.get_or_init(|| Rig::parse(RIG, true))
}

/// Angles in degrees per segment id of `reference_rig.svg`, from `poses.toml`.
#[derive(Clone, Debug, PartialEq)]
pub struct Pose {
    pub name: String,
    pub angles: Vec<(String, f32)>,
}

impl Pose {
    /// The angle of segment `id`, 0 when the pose does not name it.
    pub fn angle(&self, id: &str) -> f32 {
        self.angles.iter().find(|(n, _)| n == id).map_or(0.0, |(_, a)| *a)
    }
}

/// One pose turning into another between two seconds, from `poses.toml`.
#[derive(Clone, Debug)]
pub struct Move {
    pub from: usize,
    pub to: usize,
    pub start: f32,
    pub end: f32,
    pub easing: KeyEasing,
}

#[derive(serde::Deserialize)]
struct PosesFile {
    #[serde(default)]
    poses: Vec<PoseFile>,
    #[serde(default)]
    moves: Vec<MoveFile>,
}

#[derive(serde::Deserialize)]
struct PoseFile {
    name: String,
    #[serde(default)]
    angles: BTreeMap<String, f32>,
}

#[derive(serde::Deserialize)]
struct MoveFile {
    from: String,
    to: String,
    start: f32,
    end: f32,
    easing: Option<String>,
}

/// `poses.toml` read and checked against the rig: every angle names a segment of
/// `reference_rig.svg`, every move names two poses in time order, every easing is known.
fn load_poses() -> (Vec<Pose>, Vec<Move>) {
    let file: PosesFile = toml::from_str(POSES_TOML).unwrap_or_else(|e| panic!("poses.toml: {}", e.message()));
    let ids = rig().ids();
    let mut problems: Vec<String> = Vec::new();
    let poses: Vec<Pose> = file
        .poses
        .into_iter()
        .map(|p| {
            for id in p.angles.keys().filter(|id| !ids.contains(&id.as_str())) {
                problems.push(format!("pose \"{}\": \"{id}\" is not a segment of the rig ({})", p.name, ids.join(", ")));
            }
            Pose { name: p.name, angles: p.angles.into_iter().collect() }
        })
        .collect();
    let moves: Vec<Move> = file
        .moves
        .iter()
        .filter_map(|m| {
            let index = |name: &str| poses.iter().position(|p| p.name == name);
            let (Some(from), Some(to)) = (index(&m.from), index(&m.to)) else {
                problems.push(format!("move {} -> {}: a pose with that name does not exist", m.from, m.to));
                return None;
            };
            if m.end <= m.start {
                problems.push(format!("move {} -> {}: ends at {} s, before it starts at {} s", m.from, m.to, m.end, m.start));
                return None;
            }
            let easing = match m.easing.as_deref() {
                None => KeyEasing::Ease(Easing::Linear),
                Some(name) => match KeyEasing::named(name) {
                    Some(easing) => easing,
                    None => {
                        problems.push(format!("move {} -> {}: unknown easing \"{name}\"", m.from, m.to));
                        return None;
                    }
                },
            };
            Some(Move { from, to, start: m.start, end: m.end, easing })
        })
        .collect();
    assert!(problems.is_empty(), "poses.toml:\n{}", problems.iter().map(|p| format!("- {p}\n")).collect::<String>());
    (poses, moves)
}

/// The poses and the moves of `poses.toml`.
type PosesAndMoves = (Vec<Pose>, Vec<Move>);

static POSES_AND_MOVES: LazyLock<PosesAndMoves> = LazyLock::new(load_poses);

/// The poses of `poses.toml`, in its order.
pub static POSES: LazyLock<&'static [Pose]> = LazyLock::new(|| &POSES_AND_MOVES.0);

/// The pose at `t` seconds of the moves in `poses.toml`: before the first move its `from`
/// pose, during a move the angles eased between its two poses, after it the `to` pose.
pub fn pose_at(t: f32) -> Pose {
    let (poses, moves) = &*POSES_AND_MOVES;
    let Some(first) = moves.first() else {
        return poses.first().cloned().unwrap_or(Pose { name: "neutral".into(), angles: vec![] });
    };
    let current = moves.iter().rev().find(|m| t >= m.start).unwrap_or(first);
    let (from, to) = (&poses[current.from], &poses[current.to]);
    if t <= current.start {
        return from.clone();
    }
    if t >= current.end {
        return to.clone();
    }
    let mut ids: Vec<&str> = from.angles.iter().chain(&to.angles).map(|(id, _)| id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    let angles = ids
        .into_iter()
        .map(|id| {
            let track = Track {
                keys: vec![
                    Key { t: current.start, value: from.angle(id), easing: KeyEasing::Ease(Easing::Linear) },
                    Key { t: current.end, value: to.angle(id), easing: current.easing },
                ],
            };
            (id.to_owned(), track.at(t))
        })
        .collect();
    Pose { name: format!("{} -> {}", from.name, to.name), angles }
}

/// Rings of `seg` and its children, posed: `rotate(angle(id))` about each
/// pivot, composed down the hierarchy.
fn collect(seg: &Seg, parent: &dyn Fn(Pt) -> Pt, angle: &dyn Fn(&str) -> f64, out: &mut Vec<Vec<Pt>>) {
    let (s, c) = angle(&seg.id).to_radians().sin_cos();
    let (px, py) = seg.pivot;
    let world = |(x, y): Pt| parent((px + c * (x - px) - s * (y - py), py + s * (x - px) + c * (y - py)));
    out.extend(seg.rings.iter().map(|r| r.iter().map(|p| world(*p)).collect()));
    for child in &seg.children {
        collect(child, &world, angle, out);
    }
}

fn union(rings: Vec<Vec<Pt>>) -> MultiPolygon<f64> {
    rings.into_iter().fold(MultiPolygon::new(vec![]), |acc, r| {
        let line: LineString<f64> = r.into_iter().map(|(x, y)| Coord { x, y }).collect();
        acc.union(&MultiPolygon::new(vec![Polygon::new(line, vec![])]))
    })
}

fn to_d(mp: &MultiPolygon<f64>) -> String {
    let mut d = String::new();
    for poly in mp {
        for ring in std::iter::once(poly.exterior()).chain(poly.interiors()) {
            let pts: Vec<Pt> = ring.coords().map(|c| (c.x, c.y)).collect();
            for (k, (x, y)) in smooth(&pts[..pts.len().saturating_sub(1)]).into_iter().enumerate() {
                d += &format!("{}{:.2} {:.2} ", if k == 0 { "M" } else { "L" }, x, y);
            }
            d += "Z ";
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `poses.toml` names only segments of the rig and its moves resolve.
    #[test]
    fn poses_load_and_moves_interpolate() {
        let poses = *POSES;
        assert_eq!(poses.len(), 4);
        assert_eq!(poses[1].name, "arms out");
        assert_eq!(poses[1].angle("upper_arm_r"), 45.0);
        assert!(rig().ids().contains(&"upper_arm_r"));

        // before the first move: neutral; in its middle (ease-in-out is symmetric): half way
        assert_eq!(pose_at(-1.0).angle("upper_arm_r"), 0.0);
        let middle = pose_at(0.5).angle("upper_arm_r");
        assert!((middle - 22.5).abs() < 2.0, "{middle}");
        assert_eq!(pose_at(1.2).angle("upper_arm_r"), 45.0);
        // the second move ends in "hands on hips"
        assert_eq!(pose_at(3.0), poses[3]);
    }
}

/// `pose` with rig point `ORIGIN` at (x, y), at `scale`.
pub fn draw(pose: &Pose, x: f32, y: f32, scale: f32) -> Svgr<'static> {
    let tr = format!("translate({x} {y}) scale({scale}) translate({} {})", -ORIGIN.0, -ORIGIN.1);
    let d = rig().outline(&|id| pose.angles.iter().find(|(n, _)| *n == id).map(|(_, a)| *a as f64).unwrap_or(0.0));
    fframes::svgr!(
        <g transform={tr}>
            <path d={d} fill={FILL} fill-rule="evenodd" stroke={INK} stroke-width={LINE} stroke-linejoin="round" />
        </g>
    )
}
