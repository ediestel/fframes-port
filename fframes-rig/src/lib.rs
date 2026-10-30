//! Rigging the 3D figures (the glTF that `fframes-vector depth` exports, and its Gaussian
//! splat). A turntable reconstruction joins a limb to the body where they touch or nearly
//! touch; the user cuts the bridge away and places the joints in the editor
//! (`fframes-rig-editor`), on planes through the figure.
//!
//! Everything lives in a rig file next to the model (`figure.rig.json`), in model
//! coordinates (metres, y up, as the model is stored), so the original model is never changed
//! and every edit can be rebuilt from it.
//!
//! - A [`Plane`] is a direction through the figure: transverse, coronal and sagittal are
//!   presets of the figure's own axes (its front as the user confirmed it), any other
//!   direction can be stored too.
//! - A [`Cut`] removes a volume. It is drawn in one or more views (each one plane direction);
//!   in a view, contours at several positions along the plane's normal are interpolated
//!   between them, with explicit ends beyond the first and last. The views of one cut
//!   intersect; cuts combine by union; protected cuts always win over removal.
//! - A [`Joint`] is placed by clicks on planes; it is complete when seen in two planes that
//!   are not parallel. [`RigFile::open_tasks`] lists what still needs work.

use serde::{Deserialize, Serialize};

pub type V3 = [f32; 3];

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// The figure's own axes for a front direction `front_yaw` (degrees about y; 0 faces +z):
/// side (to the right seen from the front), up, front.
#[must_use]
pub fn axes(front_yaw: f32) -> (V3, V3, V3) {
    let (s, c) = front_yaw.to_radians().sin_cos();
    ([c, 0.0, -s], [0.0, 1.0, 0.0], [s, 0.0, c])
}

/// A plane direction: its unit normal and two unit axes in the plane (`u`, `v`, with
/// `u × v = normal`). A plane at `offset` holds the points `p` with `p · normal = offset`;
/// its 2D coordinates are `(p · u, p · v)`.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Plane {
    pub normal: V3,
    pub u: V3,
    pub v: V3,
}

/// The preset plane directions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Preset {
    /// Horizontal: slices at one height.
    Transverse,
    /// Separates front from back.
    Coronal,
    /// Separates left from right.
    Sagittal,
}

impl Preset {
    pub const ALL: [Preset; 3] = [Preset::Transverse, Preset::Coronal, Preset::Sagittal];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Preset::Transverse => "transverse",
            Preset::Coronal => "coronal",
            Preset::Sagittal => "sagittal",
        }
    }

    /// The plane of this preset for the figure's front.
    #[must_use]
    pub fn plane(self, front_yaw: f32) -> Plane {
        let (side, up, front) = axes(front_yaw);
        match self {
            // u × v = normal for each, so a plane is a turn of the axes, never a mirror
            // seen from above: side to the right, back up the picture
            Preset::Transverse => Plane { normal: up, u: side, v: front.map(|c| -c) },
            Preset::Coronal => Plane { normal: front, u: side, v: up },
            // seen from the side: back to the right, up
            Preset::Sagittal => Plane { normal: side, u: front.map(|c| -c), v: up },
        }
    }
}

impl Plane {
    #[must_use]
    pub fn offset_of(&self, p: V3) -> f32 {
        dot(p, self.normal)
    }

    #[must_use]
    pub fn to_2d(&self, p: V3) -> [f32; 2] {
        [dot(p, self.u), dot(p, self.v)]
    }

    #[must_use]
    pub fn to_3d(&self, q: [f32; 2], offset: f32) -> V3 {
        std::array::from_fn(|k| self.normal[k] * offset + self.u[k] * q[0] + self.v[k] * q[1])
    }

    /// Where a ray meets the plane at `offset`: distance along the ray, `None` when it runs
    /// parallel or the plane is behind.
    #[must_use]
    pub fn hit(&self, offset: f32, origin: V3, direction: V3) -> Option<(V3, f32)> {
        let d = dot(direction, self.normal);
        if d.abs() < 1e-6 {
            return None;
        }
        let t = (offset - dot(origin, self.normal)) / d;
        (t > 0.0).then(|| (std::array::from_fn(|k| origin[k] + direction[k] * t), t))
    }

    /// Whether two planes are close to parallel (normals within 20°).
    #[must_use]
    pub fn parallel_to(&self, other: &Plane) -> bool {
        dot(self.normal, other.normal).abs() > 20_f32.to_radians().cos()
    }
}

/// One contour of a view, at a position along the plane's normal.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Section {
    pub offset: f32,
    /// Closed polygon in the plane's 2D coordinates (metres).
    pub contour: Vec<[f32; 2]>,
    /// Per corner, how far it lies off the section's plane along the normal (metres): a box's
    /// corners move freely. Empty: all on the plane.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lift: Vec<f32>,
}

impl Section {
    #[must_use]
    pub fn new(offset: f32, contour: Vec<[f32; 2]>) -> Self {
        Section { offset, contour, lift: Vec::new() }
    }

    /// Corner `k`'s offset along the plane's normal.
    #[must_use]
    pub fn corner_offset(&self, k: usize) -> f32 {
        self.offset + self.lift.get(k).copied().unwrap_or(0.0)
    }

    /// Corner `k` in model space.
    #[must_use]
    pub fn corner(&self, plane: &Plane, k: usize) -> V3 {
        plane.to_3d(self.contour[k], self.corner_offset(k))
    }

    /// Moves corner `k` to the model point `p`, wherever it lies.
    pub fn set_corner(&mut self, plane: &Plane, k: usize, p: V3) {
        self.lift.resize(self.contour.len(), 0.0);
        self.contour[k] = plane.to_2d(p);
        self.lift[k] = plane.offset_of(p) - self.offset;
    }
}

/// A cut drawn in one plane direction: its sections, sorted by offset, and how far the
/// volume reaches beyond the first and the last section.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct View {
    pub plane: Plane,
    pub sections: Vec<Section>,
    pub before: f32,
    pub after: f32,
}

/// Default reach of a view beyond its first and last section, metres.
pub const DEFAULT_END: f32 = 0.01;

impl View {
    #[must_use]
    pub fn new(plane: Plane) -> Self {
        View { plane, sections: Vec::new(), before: DEFAULT_END, after: DEFAULT_END }
    }

    /// The offsets the volume spans.
    #[must_use]
    pub fn range(&self) -> Option<(f32, f32)> {
        let offsets = self.sections.iter().flat_map(|s| (0..s.contour.len().max(1)).map(|k| s.corner_offset(k)));
        let (lo, hi) = offsets.fold(None, |r: Option<(f32, f32)>, o| Some(r.map_or((o, o), |(a, b)| (a.min(o), b.max(o)))))?;
        Some((lo - self.before, hi + self.after))
    }

    /// Keeps the sections in offset order (after adding one).
    pub fn sort(&mut self) {
        self.sections.sort_by(|a, b| a.offset.total_cmp(&b.offset));
    }

    /// The contour at `offset`: a section's own, the nearest end section beyond the first or
    /// last, else interpolated between the two sections around it. `None` outside the range.
    #[must_use]
    pub fn contour_at(&self, offset: f32) -> Option<Vec<[f32; 2]>> {
        let (lo, hi) = self.range()?;
        if offset < lo || offset > hi {
            return None;
        }
        let s = &self.sections;
        if offset <= s[0].offset {
            return Some(s[0].contour.clone());
        }
        if offset >= s[s.len() - 1].offset {
            return Some(s[s.len() - 1].contour.clone());
        }
        let k = s.windows(2).position(|w| offset >= w[0].offset && offset <= w[1].offset)?;
        let (a, b) = (&s[k], &s[k + 1]);
        let t = if b.offset > a.offset { (offset - a.offset) / (b.offset - a.offset) } else { 0.0 };
        Some(interpolate(&a.contour, &b.contour, t))
    }

    /// Whether all sections have the same number of corners, so corresponding corners are
    /// joined by straight edges (a box when there are two sections of four).
    #[must_use]
    pub fn corresponding(&self) -> bool {
        self.sections.first().is_some_and(|f| f.contour.len() >= 3 && self.sections.iter().all(|s| s.contour.len() == f.contour.len()))
    }

    /// The surface of a view whose sections correspond: the sections as its levels, quads
    /// between corresponding edges of neighbouring levels, the first and last closed (fans).
    #[must_use]
    pub fn surface(&self) -> (Vec<V3>, Vec<u32>) {
        if !self.corresponding() {
            return (Vec::new(), Vec::new());
        }
        let n = self.sections[0].contour.len() as u32;
        let pos: Vec<V3> = self.sections.iter().flat_map(|s| (0..s.contour.len()).map(|k| s.corner(&self.plane, k))).collect();
        let mut ind = Vec::new();
        for l in 0..self.sections.len() as u32 - 1 {
            for k in 0..n {
                let (a, b) = (l * n + k, l * n + (k + 1) % n);
                ind.extend([a, b, b + n, a, b + n, a + n]);
            }
        }
        let last = (self.sections.len() as u32 - 1) * n;
        for k in 1..n - 1 {
            ind.extend([0, k + 1, k, last, last + k, last + k + 1]);
        }
        (pos, ind)
    }

    /// Adds a corner on edge `edge` (between corners `edge` and `edge + 1`) of section
    /// `section`, at `point`; the other sections get one at the same place along their edge.
    pub fn insert_corner(&mut self, section: usize, edge: usize, point: [f32; 2]) {
        let Some(s) = self.sections.get(section) else { return };
        let n = s.contour.len();
        let (a, b) = (s.contour[edge % n], s.contour[(edge + 1) % n]);
        let ab = [b[0] - a[0], b[1] - a[1]];
        let t = (((point[0] - a[0]) * ab[0] + (point[1] - a[1]) * ab[1]) / (ab[0] * ab[0] + ab[1] * ab[1]).max(1e-12)).clamp(0.0, 1.0);
        for (i, s) in self.sections.iter_mut().enumerate() {
            let m = s.contour.len();
            let (p, q) = (s.contour[edge % m], s.contour[(edge + 1) % m]);
            let at = if i == section { point } else { [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t] };
            if !s.lift.is_empty() {
                let (lp, lq) = (s.lift[edge % m], s.lift[(edge + 1) % m]);
                s.lift.insert(edge % m + 1, lp + (lq - lp) * t);
            }
            s.contour.insert(edge % m + 1, at);
        }
    }

    /// Adds a level at `offset` (between two sections): the shape there, with corner
    /// `corner` moved to `point`.
    pub fn insert_level(&mut self, offset: f32, corner: usize, point: [f32; 2]) {
        let Some(mut contour) = self.contour_at(offset) else { return };
        if let Some(c) = contour.get_mut(corner) {
            *c = point;
        }
        self.sections.push(Section::new(offset, contour));
        self.sort();
    }

    /// Removes a corner from every section (a section keeps at least three).
    pub fn remove_corner(&mut self, corner: usize) -> bool {
        if !self.sections.iter().all(|s| s.contour.len() > 3 && corner < s.contour.len()) {
            return false;
        }
        for s in &mut self.sections {
            s.contour.remove(corner);
            if corner < s.lift.len() {
                s.lift.remove(corner);
            }
        }
        true
    }

    /// Whether a box's corner lies off its level (moved on its own).
    #[must_use]
    pub fn lifted(&self) -> bool {
        self.corresponding() && self.sections.iter().any(|s| s.lift.iter().any(|l| *l != 0.0))
    }

    /// Whether a point lies in this view's volume (a box with corners off their levels:
    /// inside its surface).
    #[must_use]
    pub fn contains(&self, p: V3) -> bool {
        if self.lifted() {
            let (pos, ind) = self.surface();
            return enclosed(&pos, &ind, p);
        }
        self.contour_at(self.plane.offset_of(p)).is_some_and(|c| inside(&c, self.plane.to_2d(p)))
    }
}

/// A removal (or, protected, a region no cut may remove): the intersection of its views.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Cut {
    pub name: String,
    #[serde(default)]
    pub protect: bool,
    pub views: Vec<View>,
    /// Proposed by [`propose_cuts`] and not yet accepted: it removes nothing.
    #[serde(default)]
    pub proposed: bool,
}

impl Cut {
    #[must_use]
    pub fn contains(&self, p: V3) -> bool {
        self.views.iter().any(|v| !v.sections.is_empty()) && self.views.iter().filter(|v| !v.sections.is_empty()).all(|v| v.contains(p))
    }
}

/// A joint seen on one plane: the plane, its offset and the clicked point in it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Observation {
    pub plane: Plane,
    pub offset: f32,
    pub point: [f32; 2],
}

/// A named joint; bones run from a joint to its parent.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Joint {
    pub name: String,
    #[serde(default)]
    pub parent: Option<String>,
    pub observations: Vec<Observation>,
    /// Proposed by [`propose_skeleton`] and not yet checked by the user.
    #[serde(default)]
    pub proposed: bool,
}

/// A joint's placements that disagree by more than this are reported, metres.
pub const JOINT_TOLERANCE: f32 = 0.02;

impl Joint {
    /// The joint's position: the mean of its placements, and how far the farthest lies from
    /// it (metres).
    #[must_use]
    pub fn position(&self) -> Option<(V3, f32)> {
        if self.observations.is_empty() {
            return None;
        }
        let pts: Vec<V3> = self.observations.iter().map(|o| o.plane.to_3d(o.point, o.offset)).collect();
        let n = pts.len() as f32;
        let mean: V3 = std::array::from_fn(|k| pts.iter().map(|p| p[k]).sum::<f32>() / n);
        let spread = pts.iter().map(|p| dot(sub(*p, mean), sub(*p, mean)).sqrt()).fold(0.0, f32::max);
        Some((mean, spread))
    }

    /// Whether two of its planes are not parallel.
    #[must_use]
    pub fn crossed(&self) -> bool {
        self.observations
            .iter()
            .enumerate()
            .any(|(i, a)| self.observations[i + 1..].iter().any(|b| !a.plane.parallel_to(&b.plane)))
    }
}

/// Where the model came from, so the editor can rebuild it with the cuts.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Source {
    /// The turntable video.
    pub video: String,
    /// The `fframes-vector depth` options it was built with (`--height 1.7 --hull`).
    pub options: Vec<String>,
}

/// The rig file: everything the editor adds to a model.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct RigFile {
    /// The model it belongs to, as given on the command line.
    pub model: String,
    #[serde(default)]
    pub source: Option<Source>,
    /// The figure's front as the user confirmed it (degrees about y; 0 faces +z), and
    /// whether it was confirmed.
    #[serde(default)]
    pub front_yaw: f32,
    #[serde(default)]
    pub front_confirmed: bool,
    #[serde(default)]
    pub cuts: Vec<Cut>,
    #[serde(default)]
    pub joints: Vec<Joint>,
}

impl RigFile {
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, text + "\n").map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Whether a model point is removed: inside a cut and inside no protected one.
    #[must_use]
    pub fn removes(&self, p: V3) -> bool {
        let real = || self.cuts.iter().filter(|c| !c.proposed);
        real().any(|c| !c.protect && c.contains(p)) && !real().any(|c| c.protect && c.contains(p))
    }

    /// What still needs work, one line each.
    #[must_use]
    pub fn open_tasks(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.front_confirmed {
            out.push(format!("front found from the shape at {:.0}°, not confirmed: F confirms, B turns it round", self.front_yaw));
        }
        for c in self.cuts.iter().filter(|c| c.proposed) {
            out.push(format!("{}: proposed, not checked: drag its dots to fit, A accepts, D dismisses", c.name));
        }
        for c in self.cuts.iter().filter(|c| !c.proposed) {
            let views = c.views.iter().filter(|v| !v.sections.is_empty()).count();
            if views == 0 {
                out.push(format!("{}: nothing drawn", c.name));
            } else if views == 1 && !c.views.iter().any(|v| v.corresponding() && v.sections.len() >= 2) {
                // (a box is whole in one view: its levels and corners give all three directions)
                out.push(format!("{}: drawn in one plane only; its extent across that plane is set by the contour alone, check it in a second plane", c.name));
            }
        }
        let proposed = self.joints.iter().filter(|j| j.proposed).count();
        if proposed > 0 {
            out.push(format!("{proposed} proposed joints not checked: drag them where they belong, A accepts the skeleton"));
        }
        for j in self.joints.iter().filter(|j| !j.proposed) {
            match j.position() {
                None => out.push(format!("joint {}: not placed", j.name)),
                Some(_) if !j.crossed() => out.push(format!("joint {}: placed in one plane direction only, place it in a second plane", j.name)),
                Some((_, spread)) if spread > JOINT_TOLERANCE => {
                    out.push(format!("joint {}: its placements disagree by {:.0} mm", j.name, spread * 1000.0));
                }
                Some(_) => {}
            }
            if let Some(p) = &j.parent {
                if !self.joints.iter().any(|q| &q.name == p) {
                    out.push(format!("joint {}: parent {p} does not exist", j.name));
                }
            }
        }
        out
    }
}

/// A view ready for many point tests: its range, and its sections' contours resampled with
/// each one's start turned to match the one before, so interpolating is point by point.
struct PreparedView {
    plane: Plane,
    lo: f32,
    hi: f32,
    sections: Vec<(f32, Vec<[f32; 2]>)>,
    /// A box: its surface, through its corners wherever they lie.
    surface: Option<(Vec<V3>, Vec<u32>)>,
}

impl PreparedView {
    fn new(v: &View) -> Option<Self> {
        let (lo, hi) = v.range()?;
        let mut sections: Vec<(f32, Vec<[f32; 2]>)> = Vec::new();
        if v.lifted() {
            return Some(PreparedView { plane: v.plane, lo, hi, sections, surface: Some(v.surface()) });
        }
        // sections with the same corners (a box): corner to corner, as drawn
        if v.corresponding() {
            return Some(PreparedView { plane: v.plane, lo, hi, sections: v.sections.iter().map(|s| (s.offset, s.contour.clone())).collect(), surface: None });
        }
        for s in &v.sections {
            let ring = resample(&s.contour, SAMPLES);
            let ring = match sections.last() {
                Some((_, prev)) => {
                    let cost = |shift: usize| -> f32 {
                        (0..SAMPLES).map(|i| (prev[i][0] - ring[(i + shift) % SAMPLES][0]).powi(2) + (prev[i][1] - ring[(i + shift) % SAMPLES][1]).powi(2)).sum()
                    };
                    let shift = (0..SAMPLES).min_by(|&a, &b| cost(a).total_cmp(&cost(b))).unwrap_or(0);
                    (0..SAMPLES).map(|i| ring[(i + shift) % SAMPLES]).collect()
                }
                None => ring,
            };
            sections.push((s.offset, ring));
        }
        Some(PreparedView { plane: v.plane, lo, hi, sections, surface: None })
    }

    fn contains(&self, p: V3) -> bool {
        let t = self.plane.offset_of(p);
        if t < self.lo || t > self.hi {
            return false;
        }
        if let Some((pos, ind)) = &self.surface {
            return enclosed(pos, ind, p);
        }
        let q = self.plane.to_2d(p);
        let s = &self.sections;
        let k = s.iter().rposition(|(o, _)| *o <= t);
        match k {
            None => inside(&s[0].1, q),
            Some(k) if k + 1 >= s.len() => inside(&s[k].1, q),
            Some(k) => {
                let ((a, ra), (b, rb)) = (&s[k], &s[k + 1]);
                let f = if b > a { (t - a) / (b - a) } else { 0.0 };
                let ring: Vec<[f32; 2]> = ra.iter().zip(rb).map(|(x, y)| [x[0] + (y[0] - x[0]) * f, x[1] + (y[1] - x[1]) * f]).collect();
                inside(&ring, q)
            }
        }
    }
}

/// The cuts of a rig file ready for testing many points (every voxel of a figure).
pub struct Removal {
    cuts: Vec<(bool, Vec<PreparedView>)>,
}

impl Removal {
    #[must_use]
    pub fn new(rig: &RigFile) -> Self {
        Removal {
            cuts: rig
                .cuts
                .iter()
                .filter(|c| !c.proposed)
                .map(|c| (c.protect, c.views.iter().filter_map(PreparedView::new).collect::<Vec<_>>()))
                .filter(|(_, v)| !v.is_empty())
                .collect(),
        }
    }

    /// Whether a model point is removed: inside a cut, inside no protected one.
    #[must_use]
    pub fn removes(&self, p: V3) -> bool {
        let within = |protect: bool| self.cuts.iter().any(|(pr, views)| *pr == protect && views.iter().all(|v| v.contains(p)));
        within(false) && !within(true)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cuts.iter().all(|(protect, _)| *protect)
    }
}

/// How much each vertex follows a limb (0 to 1), for a limb from joint `root` (the shoulder)
/// towards `tip` (the elbow), and the distance from the root along the limb where it is free
/// of the body.
///
/// The limb is the surface reached from the vertex nearest `tip` without passing below a
/// level `t` along the limb's axis (seen from the root). Searching up from the root, the
/// first level at which that region stays as narrow as the limb near its tip is where the
/// limb leaves the body: above it the region would run into the body. That needs the
/// limb to be free below there (cut away from the body). Vertices from that level up to
/// `blend` metres closer to the root follow partly (smoothstep), so the shoulder bends
/// instead of tearing.
#[must_use]
pub fn limb_weights(positions: &[V3], indices: &[u32], root: V3, tip: V3, blend: f32) -> Option<(Vec<f32>, f32)> {
    let length = dot(sub(tip, root), sub(tip, root)).sqrt();
    if length < 1e-4 || positions.is_empty() {
        return None;
    }
    let axis: V3 = sub(tip, root).map(|c| c / length);
    let along = |p: V3| dot(sub(p, root), axis);
    let radial = |p: V3| {
        let d = sub(p, root);
        let t = dot(d, axis);
        let r = [d[0] - axis[0] * t, d[1] - axis[1] * t, d[2] - axis[2] * t];
        dot(r, r).sqrt()
    };
    // corners shared between triangles (the same position) are one vertex for walking
    let mut welded: std::collections::HashMap<[i64; 3], u32> = std::collections::HashMap::new();
    let key = |p: V3| p.map(|c| (f64::from(c) * 1e5).round() as i64);
    let id: Vec<u32> = positions
        .iter()
        .map(|p| {
            let n = welded.len() as u32;
            *welded.entry(key(*p)).or_insert(n)
        })
        .collect();
    let count = welded.len();
    let mut at = vec![[0.0_f32; 3]; count];
    for (i, p) in positions.iter().enumerate() {
        at[id[i] as usize] = *p;
    }
    let mut next: Vec<Vec<u32>> = vec![Vec::new(); count];
    for t in indices.chunks_exact(3) {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (a, b) = (id[a as usize], id[b as usize]);
            if a != b {
                next[a as usize].push(b);
                next[b as usize].push(a);
            }
        }
    }
    let start = (0..count).min_by(|&a, &b| {
        let d = |v: usize| dot(sub(at[v], tip), sub(at[v], tip));
        d(a).total_cmp(&d(b))
    })?;
    let region = |level: f32| -> Vec<bool> {
        let mut inside = vec![false; count];
        let mut stack = vec![start as u32];
        inside[start] = along(at[start]) > level;
        if !inside[start] {
            return inside;
        }
        while let Some(v) = stack.pop() {
            for &q in &next[v as usize] {
                if !inside[q as usize] && along(at[q as usize]) > level {
                    inside[q as usize] = true;
                    stack.push(q);
                }
            }
        }
        inside
    };
    // the limb's own width: the region near its tip
    let tip_region = region(length * 0.8);
    let mut widths: Vec<f32> = (0..count).filter(|&v| tip_region[v]).map(|v| radial(at[v])).collect();
    widths.sort_by(f32::total_cmp);
    let limit = 2.5 * widths.get(widths.len() * 9 / 10).copied().unwrap_or(0.0).max(0.01);
    let step = 0.005_f32;
    let mut level = 0.0;
    let mut free = None;
    while level < length * 0.8 {
        let r = region(level);
        if (0..count).filter(|&v| r[v]).all(|v| radial(at[v]) <= limit) {
            free = Some((level, r));
            break;
        }
        level += step;
    }
    let (level, limb) = free?;
    // the blend: vertices above the level, near the limb's axis, follow partly
    let weights: Vec<f32> = positions
        .iter()
        .enumerate()
        .map(|(i, p)| {
            if limb[id[i] as usize] {
                return 1.0;
            }
            let s = along(*p);
            if s <= level - blend || s > level || radial(*p) > limit {
                return 0.0;
            }
            let x = ((s - (level - blend)) / blend).clamp(0.0, 1.0);
            x * x * (3.0 - 2.0 * x)
        })
        .collect();
    Some((weights, level))
}

/// The figure's front from its shape (degrees about y, as [`axes`] takes it): seen from
/// above, a standing figure is widest across the shoulders, so its side axis is the long axis
/// of its surface (area-weighted), and its front is square to that. Of the two ways round,
/// the one nearer +z (the camera of the turntable's first frame) is taken.
#[must_use]
pub fn shape_front(positions: &[V3], indices: &[u32]) -> f32 {
    let (mut sum, mut sx, mut sz, mut sxx, mut sxz, mut szz) = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    for t in indices.chunks_exact(3) {
        let [a, b, c] = [t[0], t[1], t[2]].map(|i| positions[i as usize]);
        let n = cross(sub(b, a), sub(c, a));
        let area = f64::from(dot(n, n).sqrt() * 0.5);
        let (x, z) = (f64::from(a[0] + b[0] + c[0]) / 3.0, f64::from(a[2] + b[2] + c[2]) / 3.0);
        sum += area;
        sx += area * x;
        sz += area * z;
        sxx += area * x * x;
        sxz += area * x * z;
        szz += area * z * z;
    }
    if sum <= 0.0 {
        return 0.0;
    }
    let (mx, mz) = (sx / sum, sz / sum);
    let (cxx, cxz, czz) = (sxx / sum - mx * mx, sxz / sum - mx * mz, szz / sum - mz * mz);
    // the long axis of the 2x2 covariance
    let angle = 0.5 * (2.0 * cxz).atan2(cxx - czz);
    let (ax, az) = (angle.cos() as f32, angle.sin() as f32);
    // side = (cos y, -sin y) along the long axis; the front (sin y, cos y) towards +z
    let mut yaw = (-az).atan2(ax).to_degrees();
    if yaw.to_radians().cos() < 0.0 {
        yaw += 180.0;
    }
    yaw.rem_euclid(360.0)
}

impl Joint {
    /// Places the joint at a model point, seen on a transverse and a coronal plane through
    /// it (so it counts as placed in two plane directions).
    pub fn set_at(&mut self, p: V3, front_yaw: f32) {
        self.observations = [Preset::Transverse, Preset::Coronal]
            .iter()
            .map(|pr| {
                let plane = pr.plane(front_yaw);
                Observation { plane, offset: plane.offset_of(p), point: plane.to_2d(p) }
            })
            .collect();
    }
}

/// The skeleton of a standing figure, proposed for the user to check: each joint at its
/// height in the human proportions (fractions of the stature, as in the standard tables:
/// ankle 0.04, knee 0.285, hip 0.53, chest 0.72, shoulder 0.818, neck 0.87, head 0.93,
/// elbow 0.63, wrist 0.485), then moved into the figure's cross-section at that height:
/// the middle of the body for the trunk and head, of the leg on its side for hip, knee and
/// ankle, one limb radius in from the outermost surface on its side for shoulder, elbow and
/// wrist. `_l` is the figure's own left (its side axis points there).
#[must_use]
pub fn propose_skeleton(positions: &[V3], indices: &[u32], front_yaw: f32) -> Vec<Joint> {
    let (lo, hi) = positions.iter().fold((f32::MAX, f32::MIN), |(l, h), p| (l.min(p[1]), h.max(p[1])));
    let height = hi - lo;
    if height <= 0.0 {
        return Vec::new();
    }
    let (side, _, front) = axes(front_yaw);
    let transverse = Preset::Transverse.plane(front_yaw);
    // the cross-section at a height: (side, front) of its segments' middles
    let section = |f: f32| -> Vec<(f32, f32)> {
        section_segments(positions, indices, &transverse, lo + f * height)
            .iter()
            .map(|(a, b)| {
                let m: V3 = std::array::from_fn(|k| f32::midpoint(a[k], b[k]));
                (dot(m, side), dot(m, front))
            })
            .collect()
    };
    let mean = |pts: &[(f32, f32)]| -> Option<(f32, f32)> {
        (!pts.is_empty()).then(|| {
            let n = pts.len() as f32;
            (pts.iter().map(|p| p.0).sum::<f32>() / n, pts.iter().map(|p| p.1).sum::<f32>() / n)
        })
    };
    let all = section(0.5);
    let middle = mean(&all).map_or(0.0, |m| m.0);
    // trunk and head: the middle of the section near the figure's middle
    let centre = |f: f32| {
        let pts: Vec<(f32, f32)> = section(f).into_iter().filter(|p| (p.0 - middle).abs() < 0.15 * height).collect();
        mean(&pts).unwrap_or((middle, 0.0))
    };
    // a leg: the middle of the section on its side
    let leg = |f: f32, sign: f32| {
        let pts: Vec<(f32, f32)> = section(f).into_iter().filter(|p| (p.0 - middle) * sign > 0.0).collect();
        mean(&pts).unwrap_or((middle + sign * 0.05 * height, 0.0))
    };
    // an arm: a limb radius in from the outermost surface on its side
    let arm = |f: f32, sign: f32| {
        let pts: Vec<(f32, f32)> = section(f).into_iter().filter(|p| (p.0 - middle) * sign > 0.0).collect();
        let reach = pts.iter().map(|p| (p.0 - middle) * sign).fold(0.0, f32::max);
        let outer: Vec<(f32, f32)> = pts.into_iter().filter(|p| (p.0 - middle) * sign > reach - 0.06 * height).collect();
        let front_of = mean(&outer).map_or(0.0, |m| m.1);
        (middle + sign * (reach - 0.03 * height).max(0.0), front_of)
    };
    let point = |f: f32, (s, fr): (f32, f32)| -> V3 { std::array::from_fn(|k| side[k] * s + front[k] * fr + if k == 1 { lo + f * height } else { 0.0 }) };
    let mut out = Vec::new();
    let mut add = |name: &str, parent: Option<&str>, p: V3| {
        let mut j = Joint { name: name.into(), parent: parent.map(String::from), observations: Vec::new(), proposed: true };
        j.set_at(p, front_yaw);
        out.push(j);
    };
    add("pelvis", None, point(0.53, centre(0.53)));
    add("chest", Some("pelvis"), point(0.72, centre(0.72)));
    add("neck", Some("chest"), point(0.87, centre(0.87)));
    add("head", Some("neck"), point(0.93, centre(0.93)));
    for (sign, s) in [(1.0_f32, "l"), (-1.0, "r")] {
        add(&format!("shoulder_{s}"), Some("chest"), point(0.818, arm(0.818, sign)));
        add(&format!("elbow_{s}"), Some(&format!("shoulder_{s}")), point(0.63, arm(0.63, sign)));
        add(&format!("wrist_{s}"), Some(&format!("elbow_{s}")), point(0.485, arm(0.485, sign)));
        add(&format!("hip_{s}"), Some("pelvis"), point(0.53, leg(0.40, sign)));
        add(&format!("knee_{s}"), Some(&format!("hip_{s}")), point(0.285, leg(0.285, sign)));
        add(&format!("ankle_{s}"), Some(&format!("knee_{s}")), point(0.04, leg(0.06, sign)));
    }
    out
}

/// The connected pieces of a cross-section: segments joined where they share an end
/// (to a tenth of a millimetre), as lists of segment indices.
#[must_use]
pub fn section_pieces(segments: &[(V3, V3)]) -> Vec<Vec<usize>> {
    let key = |p: V3| p.map(|c| (c * 1e4).round() as i64);
    let mut ends: std::collections::HashMap<[i64; 3], usize> = std::collections::HashMap::new();
    let mut parent: Vec<usize> = (0..segments.len()).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for (i, (a, b)) in segments.iter().enumerate() {
        for p in [a, b] {
            match ends.get(&key(*p)) {
                Some(&j) => {
                    let (ri, rj) = (root(&mut parent, i), root(&mut parent, j));
                    parent[ri] = rj;
                }
                None => {
                    ends.insert(key(*p), i);
                }
            }
        }
    }
    let mut groups: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    for i in 0..segments.len() {
        let r = root(&mut parent, i);
        groups.entry(r).or_default().push(i);
    }
    groups.into_values().collect()
}

/// Cuts proposed for the user to check: for each arm, where it stops being free of the
/// body. Going up from the wrist (0.485 of the height), the outermost piece of each
/// transverse section on that side is the arm while it is not the piece in the figure's
/// middle; the height where they first become one is where the reconstruction joined them.
/// The gap seen just below (between the body's outer edge and the arm's inner edge, over the
/// arm's depth) is proposed as removed from there up to 0.74 of the height (under the
/// armpit), as one box: a rectangle at the bottom and one at the top, corner over corner.
#[must_use]
pub fn propose_cuts(positions: &[V3], indices: &[u32], front_yaw: f32) -> Vec<Cut> {
    let (lo, hi) = positions.iter().fold((f32::MAX, f32::MIN), |(l, h), p| (l.min(p[1]), h.max(p[1])));
    let height = hi - lo;
    if height <= 0.0 {
        return Vec::new();
    }
    let (side, _, front) = axes(front_yaw);
    let planes = (Preset::Transverse.plane(front_yaw),);
    let mid_section = section_segments(positions, indices, &planes.0, lo + 0.5 * height);
    let middle = if mid_section.is_empty() { 0.0 } else { mid_section.iter().map(|(a, _)| dot(*a, side)).sum::<f32>() / mid_section.len() as f32 };
    // per piece: lowest and highest side coordinate, lowest and highest front coordinate
    let pieces_at = |f: f32| -> Vec<[f32; 4]> {
        let segs = section_segments(positions, indices, &planes.0, lo + f * height);
        section_pieces(&segs)
            .iter()
            .map(|piece| {
                piece.iter().fold([f32::MAX, f32::MIN, f32::MAX, f32::MIN], |b, &i| {
                    let (s0, s1) = (dot(segs[i].0, side), dot(segs[i].1, side));
                    let (f0, f1) = (dot(segs[i].0, front), dot(segs[i].1, front));
                    [b[0].min(s0.min(s1)), b[1].max(s0.max(s1)), b[2].min(f0.min(f1)), b[3].max(f0.max(f1))]
                })
            })
            .collect()
    };
    let mut out = Vec::new();
    for (sign, name) in [(1.0_f32, "left arm"), (-1.0, "right arm")] {
        // in the side's own direction: inner and outer edge
        let span = |b: &[f32; 4]| if sign > 0.0 { (b[0] - middle, b[1] - middle) } else { (middle - b[1], middle - b[0]) };
        let mut free: Option<(f32, f32, f32, f32)> = None; // body's outer edge, arm's inner edge, arm's depth range
        let mut joined_at = None;
        let mut f = 0.485;
        while f < 0.74 {
            let pieces = pieces_at(f);
            let body = pieces.iter().min_by(|a, b| {
                let d = |x: &[f32; 4]| if x[0] <= middle && x[1] >= middle { 0.0 } else { (x[0] - middle).abs().min((x[1] - middle).abs()) };
                d(a).total_cmp(&d(b))
            });
            let arm = pieces.iter().max_by(|a, b| span(a).1.total_cmp(&span(b).1));
            if let (Some(body), Some(arm)) = (body, arm) {
                if std::ptr::eq(body, arm) {
                    if free.is_some() {
                        joined_at = Some(f);
                        break;
                    }
                } else if span(arm).0 > span(body).1 {
                    free = Some((span(body).1, span(arm).0, arm[2], arm[3]));
                }
            }
            f += 0.005;
        }
        let (Some(at), Some((body_edge, arm_edge, f0, f1))) = (joined_at, free) else { continue };
        let (y0, y1) = (lo + (at - 0.01) * height, lo + 0.74 * height);
        let (s0, s1) = (middle + sign * body_edge, middle + sign * arm_edge);
        let (s0, s1) = (s0.min(s1), s0.max(s1));
        let (f0, f1) = (f0 - 0.01, f1 + 0.01);
        // one box: its bottom and top rectangles, corner over corner
        let t = planes.0;
        let corner = |s: f32, fr: f32, y: f32| -> [f32; 2] { t.to_2d(std::array::from_fn(|k| side[k] * s + front[k] * fr + if k == 1 { y } else { 0.0 })) };
        let across = |y: f32| Section::new(y, vec![corner(s0, f0, y), corner(s1, f0, y), corner(s1, f1, y), corner(s0, f1, y)]);
        let views = vec![View { plane: t, sections: vec![across(y0), across(y1)], before: 0.0, after: 0.0 }];
        out.push(Cut { name: format!("{name} bridge"), protect: false, views, proposed: true });
    }
    out
}

/// A rotation of `angle` radians about a unit `axis`, as a matrix.
#[must_use]
pub fn rotation(axis: V3, angle: f32) -> [[f32; 3]; 3] {
    let (s, c) = angle.sin_cos();
    let [x, y, z] = axis;
    let t = 1.0 - c;
    [
        [t * x * x + c, t * x * y - s * z, t * x * z + s * y],
        [t * x * y + s * z, t * y * y + c, t * y * z - s * x],
        [t * x * z - s * y, t * y * z + s * x, t * z * z + c],
    ]
}

/// Points turned by `rot` about `pivot`, each by its weight (linear blend between staying and
/// turning, as skinning does for one bone).
#[must_use]
pub fn pose(points: &[V3], weights: &[f32], pivot: V3, rot: &[[f32; 3]; 3]) -> Vec<V3> {
    points
        .iter()
        .zip(weights)
        .map(|(p, &w)| {
            if w <= 0.0 {
                return *p;
            }
            let d = sub(*p, pivot);
            let turned: V3 = std::array::from_fn(|r| pivot[r] + rot[r][0] * d[0] + rot[r][1] * d[1] + rot[r][2] * d[2]);
            std::array::from_fn(|k| p[k] + (turned[k] - p[k]) * w)
        })
        .collect()
}

/// Work not yet accepted, saved after every change so nothing drawn is lost (beside the rig
/// file: `figure.rig.draft.json`): the cut being drawn or edited (and which accepted cut it
/// was opened from), the joint being placed, and open contours kept per plane position.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Draft {
    #[serde(default)]
    pub cut: Option<Cut>,
    #[serde(default)]
    pub editing: Option<usize>,
    #[serde(default)]
    pub joint: Option<Joint>,
    /// Open contours: plane direction, position, dots.
    #[serde(default)]
    pub contours: Vec<(Preset, f32, Vec<[f32; 2]>)>,
}

impl Draft {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cut.is_none() && self.joint.is_none() && self.contours.iter().all(|c| c.2.is_empty())
    }

    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Writes the draft, or removes the file when nothing is left unaccepted.
    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        if self.is_empty() {
            return match std::fs::remove_file(path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("{}: {e}", path.display())),
                _ => Ok(()),
            };
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, text + "\n").map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// The draft file beside a rig file.
#[must_use]
pub fn draft_path(rig: &std::path::Path) -> std::path::PathBuf {
    rig.with_extension("draft.json")
}

/// The rig file of a model: `figure.glb` gives `figure.rig.json` beside it.
#[must_use]
pub fn rig_path(model: &std::path::Path) -> std::path::PathBuf {
    model.with_extension("rig.json")
}

/// Whether a 2D point lies inside a closed polygon (even-odd rule).
#[must_use]
pub fn inside(poly: &[[f32; 2]], p: [f32; 2]) -> bool {
    let mut odd = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
            odd = !odd;
        }
    }
    odd
}

/// Whether a point lies inside a closed triangle surface: a ray from it crosses the surface
/// an odd number of times (the ray slanted so it misses edges and corners of a box).
#[must_use]
pub fn enclosed(positions: &[V3], indices: &[u32], p: V3) -> bool {
    let d = [0.573_576_4, 0.577_350_3, 0.581_238_2];
    let mut odd = false;
    for t in indices.chunks_exact(3) {
        let [a, b, c] = [t[0], t[1], t[2]].map(|i| positions[i as usize]);
        let (e1, e2) = (sub(b, a), sub(c, a));
        let h = cross(d, e2);
        let det = dot(e1, h);
        if det.abs() < 1e-12 {
            continue;
        }
        let s = sub(p, a);
        let u = dot(s, h) / det;
        if !(0.0..1.0).contains(&u) {
            continue;
        }
        let q = cross(s, e1);
        let v = dot(d, q) / det;
        if v < 0.0 || u + v >= 1.0 {
            continue;
        }
        if dot(e2, q) / det > 0.0 {
            odd = !odd;
        }
    }
    odd
}

/// Points spaced evenly along a closed polygon's outline, counter-clockwise.
fn resample(poly: &[[f32; 2]], count: usize) -> Vec<[f32; 2]> {
    let n = poly.len();
    if n == 0 {
        return Vec::new();
    }
    let area: f32 = (0..n).map(|i| poly[i][0] * poly[(i + 1) % n][1] - poly[(i + 1) % n][0] * poly[i][1]).sum();
    let ring: Vec<[f32; 2]> = if area < 0.0 { poly.iter().rev().copied().collect() } else { poly.to_vec() };
    let len = |a: [f32; 2], b: [f32; 2]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
    let total: f32 = (0..n).map(|i| len(ring[i], ring[(i + 1) % n])).sum();
    if total <= 0.0 {
        return vec![ring[0]; count];
    }
    let mut out = Vec::with_capacity(count);
    let (mut edge, mut walked) = (0, 0.0_f32);
    for s in 0..count {
        let want = total * s as f32 / count as f32;
        while edge < n - 1 && walked + len(ring[edge], ring[(edge + 1) % n]) < want {
            walked += len(ring[edge], ring[(edge + 1) % n]);
            edge += 1;
        }
        let (a, b) = (ring[edge], ring[(edge + 1) % n]);
        let l = len(a, b).max(1e-12);
        let t = ((want - walked) / l).clamp(0.0, 1.0);
        out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
    }
    out
}

/// Samples per contour when interpolating.
const SAMPLES: usize = 64;

/// The contour a fraction `t` of the way from `a` to `b`: both resampled evenly, `b`'s start
/// turned to match `a` best, each point moved in a straight line.
#[must_use]
pub fn interpolate(a: &[[f32; 2]], b: &[[f32; 2]], t: f32) -> Vec<[f32; 2]> {
    // the same number of corners: each corner moves to its own (a box's edges stay straight)
    if a.len() == b.len() {
        return a.iter().zip(b).map(|(p, q)| [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]).collect();
    }
    let (ra, rb) = (resample(a, SAMPLES), resample(b, SAMPLES));
    if ra.is_empty() || rb.is_empty() {
        return if t < 0.5 { a.to_vec() } else { b.to_vec() };
    }
    let cost = |shift: usize| -> f32 {
        (0..SAMPLES).map(|i| {
            let (p, q) = (ra[i], rb[(i + shift) % SAMPLES]);
            (p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)
        }).sum()
    };
    let shift = (0..SAMPLES).min_by(|&x, &y| cost(x).total_cmp(&cost(y))).unwrap_or(0);
    (0..SAMPLES)
        .map(|i| {
            let (p, q) = (ra[i], rb[(i + shift) % SAMPLES]);
            [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]
        })
        .collect()
}

/// The segments where a plane at `offset` cuts the triangles: the model's cross-section, as
/// pairs of model points.
#[must_use]
pub fn section_segments(positions: &[V3], indices: &[u32], plane: &Plane, offset: f32) -> Vec<(V3, V3)> {
    let mut out = Vec::new();
    for t in indices.chunks_exact(3) {
        let pts = [t[0], t[1], t[2]].map(|i| positions[i as usize]);
        let d = pts.map(|p| plane.offset_of(p) - offset);
        let mut crossing = Vec::with_capacity(2);
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            if (d[a] < 0.0) != (d[b] < 0.0) {
                let s = d[a] / (d[a] - d[b]);
                crossing.push(std::array::from_fn(|k| pts[a][k] + (pts[b][k] - pts[a][k]) * s));
            }
        }
        if let [p, q] = crossing[..] {
            out.push((p, q));
        }
    }
    out
}

/// The nearest point where a ray (`origin`, unit `direction`) meets the triangles, with its
/// distance along the ray (Möller–Trumbore, both faces).
#[must_use]
pub fn pick(positions: &[V3], indices: &[u32], origin: V3, direction: V3) -> Option<(V3, f32)> {
    let mut best: Option<f32> = None;
    for t in indices.chunks_exact(3) {
        let [a, b, c] = [t[0], t[1], t[2]].map(|i| positions[i as usize]);
        let (e1, e2) = (sub(b, a), sub(c, a));
        let p = cross(direction, e2);
        let det = dot(e1, p);
        if det.abs() < 1e-12 {
            continue;
        }
        let s = sub(origin, a);
        let u = dot(s, p) / det;
        if !(0.0..=1.0).contains(&u) {
            continue;
        }
        let q = cross(s, e1);
        let v = dot(direction, q) / det;
        if v < 0.0 || u + v > 1.0 {
            continue;
        }
        let d = dot(e2, q) / det;
        if d > 1e-6 && best.is_none_or(|b| d < b) {
            best = Some(d);
        }
    }
    best.map(|d| (std::array::from_fn(|k| origin[k] + direction[k] * d), d))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(cx: f32, cy: f32, half: f32) -> Vec<[f32; 2]> {
        vec![[cx - half, cy - half], [cx + half, cy - half], [cx + half, cy + half], [cx - half, cy + half]]
    }

    /// A ray through two parallel squares meets the nearer one.
    #[test]
    fn pick_finds_the_nearest_surface() {
        let positions = vec![
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
            [-1.0, -1.0, -2.0],
            [1.0, -1.0, -2.0],
            [1.0, 1.0, -2.0],
            [-1.0, 1.0, -2.0],
        ];
        let indices = vec![4, 5, 6, 4, 6, 7, 0, 1, 2, 0, 2, 3];
        let (hit, d) = pick(&positions, &indices, [0.2, 0.3, 5.0], [0.0, 0.0, -1.0]).expect("hit");
        assert!((d - 5.0).abs() < 1e-5 && hit[2].abs() < 1e-5, "{hit:?} at {d}");
        assert!(pick(&positions, &indices, [3.0, 0.0, 5.0], [0.0, 0.0, -1.0]).is_none());
    }

    /// Two transverse sections, a small square low and a large one high: halfway up the
    /// volume is the size in between; beyond the ends it stops.
    #[test]
    fn sections_interpolate_and_end() {
        let mut view = View::new(Preset::Transverse.plane(0.0));
        view.sections.push(Section::new(1.0, square(0.0, 0.0, 0.01)));
        view.sections.push(Section::new(1.2, square(0.0, 0.0, 0.03)));
        view.sort();
        // at 1.1 m the square is 2 cm across each way from the middle
        assert!(view.contains([0.019, 1.1, 0.0]) && !view.contains([0.021, 1.1, 0.0]));
        // the ends reach DEFAULT_END beyond the first and last section
        assert!(view.contains([0.0, 1.0 - DEFAULT_END * 0.9, 0.0]) && !view.contains([0.0, 1.0 - DEFAULT_END * 1.1, 0.0]));
        assert!(view.contains([0.025, 1.205, 0.0]) && !view.contains([0.0, 1.22, 0.0]));
    }

    /// The views of a cut intersect; cuts combine by union; protection wins.
    #[test]
    fn views_intersect_and_protection_wins() {
        let mut across = View::new(Preset::Transverse.plane(0.0));
        across.sections.push(Section::new(1.0, square(0.3, 0.0, 0.02)));
        across.before = 0.1;
        across.after = 0.1;
        // seen from the front: only the lower half of that height range
        let mut front = View::new(Preset::Coronal.plane(0.0));
        front.sections.push(Section::new(0.0, square(0.3, 0.95, 0.05)));
        front.before = 0.5;
        front.after = 0.5;
        let cut = Cut { name: "bridge".into(), views: vec![across, front], ..Cut::default() };
        assert!(cut.contains([0.3, 0.95, 0.0]));
        assert!(!cut.contains([0.3, 1.05, 0.0]), "outside the coronal view");
        let mut rig = RigFile { cuts: vec![cut], ..RigFile::default() };
        assert!(rig.removes([0.3, 0.92, 0.0]));
        let mut shoulder = View::new(Preset::Transverse.plane(0.0));
        shoulder.sections.push(Section::new(0.92, square(0.3, 0.0, 0.05)));
        rig.cuts.push(Cut { name: "shoulder".into(), protect: true, views: vec![shoulder], ..Cut::default() });
        assert!(!rig.removes([0.3, 0.92, 0.0]), "protected");
        assert!(rig.removes([0.3, 0.97, 0.0]), "outside the protection");
        // the prepared form gives the same answers on a grid of points
        let prepared = Removal::new(&rig);
        for i in 0..20 {
            for j in 0..40 {
                for k in 0..5 {
                    let p = [0.25 + i as f32 * 0.005, 0.85 + j as f32 * 0.005, -0.02 + k as f32 * 0.01];
                    assert_eq!(prepared.removes(p), rig.removes(p), "at {p:?}");
                }
            }
        }
    }

    /// A joint seen in one plane direction is an open task; seen across, it is placed, and
    /// placements far apart are reported.
    #[test]
    fn joints_need_two_plane_directions() {
        let t = Preset::Transverse.plane(0.0);
        let c = Preset::Coronal.plane(0.0);
        let mut j = Joint { name: "shoulder".into(), observations: vec![Observation { plane: t, offset: 1.3, point: [0.2, 0.0] }], ..Joint::default() };
        let mut rig = RigFile { front_confirmed: true, joints: vec![j.clone()], ..RigFile::default() };
        assert_eq!(rig.open_tasks().len(), 1, "{:?}", rig.open_tasks());
        j.observations.push(Observation { plane: c, offset: 0.0, point: [0.2, 1.31] });
        rig.joints = vec![j.clone()];
        assert!(rig.open_tasks().is_empty(), "{:?}", rig.open_tasks());
        let (p, spread) = j.position().expect("placed");
        assert!((p[1] - 1.305).abs() < 1e-4 && spread < 0.01, "{p:?} {spread}");
        j.observations.push(Observation { plane: c, offset: 0.0, point: [0.2, 1.4] });
        rig.joints = vec![j];
        assert!(rig.open_tasks()[0].contains("disagree"), "{:?}", rig.open_tasks());
    }

    /// The cross-section of a unit cube at half height is its square outline.
    #[test]
    fn cube_section() {
        let mut pos = Vec::new();
        for k in 0..8 {
            pos.push([f32::from(u8::from(k & 1 != 0)), f32::from(u8::from(k & 2 != 0)), f32::from(u8::from(k & 4 != 0))]);
        }
        let mut ind = Vec::new();
        for f in [[0u32, 1, 3, 2], [4, 6, 7, 5], [0, 4, 5, 1], [2, 3, 7, 6], [0, 2, 6, 4], [1, 5, 7, 3]] {
            ind.extend([f[0], f[1], f[2], f[0], f[2], f[3]]);
        }
        let segs = section_segments(&pos, &ind, &Preset::Transverse.plane(0.0), 0.5);
        let length: f32 = segs.iter().map(|(a, b)| dot(sub(*a, *b), sub(*a, *b)).sqrt()).sum();
        assert!((length - 4.0).abs() < 1e-4, "outline length {length}");
    }

    /// The closed surface of a voxel set (cube corners shared), `size` metres per voxel.
    fn voxel_surface(solid: &dyn Fn(i32, i32, i32) -> bool, n: i32, size: f32) -> (Vec<V3>, Vec<u32>) {
        let mut corner: std::collections::HashMap<(i32, i32, i32), u32> = std::collections::HashMap::new();
        let (mut pos, mut ind) = (Vec::new(), Vec::new());
        let mut vertex = |c: (i32, i32, i32), pos: &mut Vec<V3>| -> u32 {
            *corner.entry(c).or_insert_with(|| {
                pos.push([c.0 as f32 * size, c.1 as f32 * size, c.2 as f32 * size]);
                pos.len() as u32 - 1
            })
        };
        for x in -1..=n {
            for y in -1..=n {
                for z in -1..=n {
                    if !solid(x, y, z) {
                        continue;
                    }
                    // each face towards an empty neighbour: its four corners
                    let faces: [((i32, i32, i32), [(i32, i32, i32); 4]); 6] = [
                        ((1, 0, 0), [(1, 0, 0), (1, 1, 0), (1, 1, 1), (1, 0, 1)]),
                        ((-1, 0, 0), [(0, 0, 0), (0, 0, 1), (0, 1, 1), (0, 1, 0)]),
                        ((0, 1, 0), [(0, 1, 0), (0, 1, 1), (1, 1, 1), (1, 1, 0)]),
                        ((0, -1, 0), [(0, 0, 0), (1, 0, 0), (1, 0, 1), (0, 0, 1)]),
                        ((0, 0, 1), [(0, 0, 1), (1, 0, 1), (1, 1, 1), (0, 1, 1)]),
                        ((0, 0, -1), [(0, 0, 0), (0, 1, 0), (1, 1, 0), (1, 0, 0)]),
                    ];
                    for ((dx, dy, dz), quad) in faces {
                        if solid(x + dx, y + dy, z + dz) {
                            continue;
                        }
                        let q = quad.map(|(a, b, c)| vertex((x + a, y + b, z + c), &mut pos));
                        ind.extend([q[0], q[1], q[2], q[0], q[2], q[3]]);
                    }
                }
            }
        }
        (pos, ind)
    }

    /// A body with an arm hanging free beside it, joined at the shoulder: the arm below the
    /// shoulder follows fully, the body not at all; raising the arm moves only the arm.
    #[test]
    fn the_free_arm_follows_the_shoulder() {
        let size = 0.02;
        // body x 0..8, arm x 10..12 (a 4 cm gap), joined by the shoulder at the top
        let solid = |x: i32, y: i32, z: i32| {
            (0..4).contains(&z)
                && (0..30).contains(&y)
                && ((0..8).contains(&x) || ((10..12).contains(&x) && y >= 8) || ((8..12).contains(&x) && y >= 26))
        };
        let (pos, ind) = voxel_surface(&solid, 31, size);
        let shoulder = [11.0 * size, 28.0 * size, 2.0 * size];
        let elbow = [11.0 * size, 15.0 * size, 2.0 * size];
        let (w, level) = limb_weights(&pos, &ind, shoulder, elbow, 0.03).expect("weights");
        eprintln!("free {level:.3} m below the shoulder");
        for (p, wt) in pos.iter().zip(&w) {
            if p[0] >= 10.0 * size && p[1] <= 24.0 * size {
                assert!((wt - 1.0).abs() < 1e-6, "arm vertex {p:?} weight {wt}");
            }
            if p[0] <= 8.0 * size && p[1] <= 24.0 * size {
                assert!(*wt == 0.0, "body vertex {p:?} weight {wt}");
            }
        }
        // the arm is free from the armpit (2 voxels under the shoulder joint) down
        assert!(level > 0.0 && level < 0.08, "free from {level}");
        let raised = pose(&pos, &w, shoulder, &rotation([0.0, 0.0, 1.0], 80_f32.to_radians()));
        for ((p, q), wt) in pos.iter().zip(&raised).zip(&w) {
            if *wt == 0.0 {
                assert_eq!(p, q);
            }
        }
        // the hand end of the arm swings out sideways, above its old height
        let hand = pos.iter().zip(&raised).filter(|(p, _)| p[1] < 9.0 * size && p[0] >= 10.0 * size).map(|(_, q)| q[1]).fold(f32::MAX, f32::min);
        assert!(hand > 0.4, "the raised hand end is at {hand:.2} m");
    }

    /// Every preset, at any front, has unit axes square to each other with u × v = normal: a
    /// turn of the figure's axes (what the editor draws the pane with), never a mirror.
    #[test]
    fn presets_are_turns() {
        for yaw in [0.0_f32, 26.0, 135.0, 300.0] {
            for p in Preset::ALL {
                let pl = p.plane(yaw);
                let n = cross(pl.u, pl.v);
                for k in 0..3 {
                    assert!((n[k] - pl.normal[k]).abs() < 1e-5, "{} at {yaw}°: u × v {n:?}, normal {:?}", p.name(), pl.normal);
                }
                assert!((dot(pl.u, pl.u) - 1.0).abs() < 1e-5 && dot(pl.u, pl.v).abs() < 1e-5);
            }
        }
    }

    /// A figure wide across the shoulders and shallow front to back, turned about y: its
    /// front is found square to its width, the way nearer +z.
    #[test]
    fn front_from_the_shape() {
        for (turn, expected) in [(30.0_f32, 30.0_f32), (200.0, 20.0), (-60.0, 300.0)] {
            let (side, _, front) = axes(turn);
            let mut pos = Vec::new();
            for k in 0..8 {
                let (x, y, z) = (if k & 1 == 0 { -0.3 } else { 0.3 }, if k & 2 == 0 { 0.0 } else { 1.7 }, if k & 4 == 0 { -0.1 } else { 0.1 });
                pos.push([x * side[0] + z * front[0], y, x * side[2] + z * front[2]]);
            }
            let mut ind = Vec::new();
            for f in [[0u32, 1, 3, 2], [4, 6, 7, 5], [0, 4, 5, 1], [2, 3, 7, 6], [0, 2, 6, 4], [1, 5, 7, 3]] {
                ind.extend([f[0], f[1], f[2], f[0], f[2], f[3]]);
            }
            let got = shape_front(&pos, &ind);
            let diff = (got - expected).rem_euclid(360.0);
            // a box's two triangles per face, split along one diagonal, lean the estimate a little
            assert!(diff.min(360.0 - diff) < 1.5, "turned {turn}°: front {got:.1}°, expected {expected}°");
        }
    }

    /// The proposed skeleton of a body with two legs and two arms: sixteen joints, the
    /// limbs on their own sides, heights in order, every joint placed in two directions.
    #[test]
    fn proposed_skeleton_is_ordered() {
        let size = 0.02;
        // 1.7 m: legs x 6..10 and 14..18 up to 0.8 m, body x 6..18, arms x 2..5 and 19..22
        let solid = |x: i32, y: i32, z: i32| {
            (0..6).contains(&z)
                && (0..85).contains(&y)
                && (((6..10).contains(&x) || (14..18).contains(&x)) && y < 40
                    || (6..18).contains(&x) && (40..72).contains(&y)
                    || (9..15).contains(&x) && y >= 72
                    || ((2..5).contains(&x) || (19..22).contains(&x)) && (36..70).contains(&y))
        };
        let (pos, ind) = voxel_surface(&solid, 90, size);
        let joints = propose_skeleton(&pos, &ind, 0.0);
        assert_eq!(joints.len(), 16);
        let at = |n: &str| joints.iter().find(|j| j.name == n).and_then(Joint::position).expect(n).0;
        for (s, sign) in [("l", 1.0_f32), ("r", -1.0)] {
            for limb in ["shoulder", "elbow", "wrist", "hip", "knee", "ankle"] {
                let p = at(&format!("{limb}_{s}"));
                assert!((p[0] - 12.0 * size) * sign > 0.0, "{limb}_{s} on the wrong side: {p:?}");
            }
            assert!(at(&format!("ankle_{s}"))[1] < at(&format!("knee_{s}"))[1] && at(&format!("knee_{s}"))[1] < at(&format!("hip_{s}"))[1]);
            assert!(at(&format!("wrist_{s}"))[1] < at(&format!("elbow_{s}"))[1] && at(&format!("elbow_{s}"))[1] < at(&format!("shoulder_{s}"))[1]);
        }
        let rig = RigFile { front_confirmed: true, joints: joints.clone(), ..RigFile::default() };
        assert_eq!(rig.open_tasks().len(), 1, "only the check of the proposal: {:?}", rig.open_tasks());
        assert!(joints.iter().all(Joint::crossed));
    }

    /// A body with one arm free and one joined to it by a bridge at mid height: a cut is
    /// proposed for the joined arm only, covering the bridge, not the arm or the body, and it
    /// removes nothing until accepted.
    #[test]
    fn a_bridge_is_proposed_as_a_cut() {
        let size = 0.02;
        let solid = |x: i32, y: i32, z: i32| {
            (0..6).contains(&z)
                && (0..85).contains(&y)
                && (((6..10).contains(&x) || (14..18).contains(&x)) && y < 40
                    || (6..18).contains(&x) && (40..72).contains(&y)
                    || (9..15).contains(&x) && y >= 72
                    || ((2..5).contains(&x) || (19..22).contains(&x)) && (36..70).contains(&y)
                    // the bridge: the arm on +x joined to the body from 0.92 m to 1.2 m
                    || x == 18 && (46..60).contains(&y))
        };
        let (pos, ind) = voxel_surface(&solid, 90, size);
        let cuts = propose_cuts(&pos, &ind, 0.0);
        assert_eq!(cuts.len(), 1, "{:?}", cuts.iter().map(|c| &c.name).collect::<Vec<_>>());
        let cut = &cuts[0];
        assert!(cut.proposed && cut.name.starts_with("left"), "{}", cut.name);
        assert!(cut.contains([18.5 * size, 52.0 * size, 3.0 * size]), "the bridge");
        assert!(!cut.contains([20.5 * size, 52.0 * size, 3.0 * size]), "the arm");
        assert!(!cut.contains([16.0 * size, 52.0 * size, 3.0 * size]), "the body");
        let rig = RigFile { cuts: cuts.clone(), ..RigFile::default() };
        assert!(!rig.removes([18.5 * size, 52.0 * size, 3.0 * size]) && Removal::new(&rig).is_empty());
    }

    /// A box (two rectangles, corner over corner): its surface is closed; a corner added on an
    /// edge goes to both rectangles and the box still holds what it held; a level added part
    /// way up bends it there; removing the added corner gives the box back.
    #[test]
    fn box_corners_and_levels() {
        let mut v = View { plane: Preset::Transverse.plane(0.0), sections: vec![
            Section::new(1.0, square(0.0, 0.0, 0.05)),
            Section::new(1.2, square(0.0, 0.0, 0.05)),
        ], before: 0.0, after: 0.0 };
        let closed = |v: &View| {
            let (_, ind) = v.surface();
            let mut edges: std::collections::HashMap<(u32, u32), i32> = std::collections::HashMap::new();
            for t in ind.chunks_exact(3) {
                for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    *edges.entry((a.min(b), a.max(b))).or_default() += 1;
                }
            }
            !ind.is_empty() && edges.values().all(|&c| c == 2)
        };
        assert!(v.corresponding() && closed(&v));
        let inside_before = v.contains(v.plane.to_3d([0.04, 0.04], 1.1));
        v.insert_corner(0, 1, [0.05, 0.0]);
        assert!(v.sections.iter().all(|s| s.contour.len() == 5) && closed(&v));
        assert_eq!(v.contains(v.plane.to_3d([0.04, 0.04], 1.1)), inside_before);
        // a level at 1.1 m with corner 2 pulled out: the box bulges there, not at the ends
        let out = [0.09, 0.0];
        v.insert_level(1.1, 2, out);
        assert_eq!(v.sections.len(), 3);
        assert!(v.contains(v.plane.to_3d([0.08, 0.0], 1.1)) && !v.contains(v.plane.to_3d([0.08, 0.0], 1.01)));
        assert!(v.remove_corner(2) && v.sections.iter().all(|s| s.contour.len() == 4) && closed(&v));
        // a corner moved on its own, like a joint (10 cm up): the box reaches up to it there
        let plane = v.plane;
        let last = v.sections.len() - 1;
        let (c0, c2) = (v.sections[last].corner(&plane, 0), v.sections[last].corner(&plane, 2));
        let raised: V3 = std::array::from_fn(|k| c0[k] + plane.normal[k] * 0.1);
        let probe: V3 = std::array::from_fn(|k| c2[k] + (c0[k] - c2[k]) * 0.75 + plane.normal[k] * 0.05);
        assert!(!v.contains(probe));
        v.sections[last].set_corner(&plane, 0, raised);
        assert!(v.contains(probe) && closed(&v) && v.range().is_some_and(|(_, hi)| hi > 1.29));
        assert!(Removal::new(&RigFile { cuts: vec![Cut { name: "box".into(), views: vec![v.clone()], ..Cut::default() }], ..RigFile::default() }).removes(probe));
    }

    /// A draft keeps unaccepted work through saving and loading; an empty one leaves no file.
    #[test]
    fn draft_round_trip() {
        let rig = std::env::temp_dir().join(format!("fframes-draft-test-{}.rig.json", std::process::id()));
        let path = draft_path(&rig);
        assert!(path.to_string_lossy().ends_with(".rig.draft.json"));
        let draft = Draft { contours: vec![(Preset::Coronal, 0.02, vec![[0.1, 1.0], [0.2, 1.0]])], ..Draft::default() };
        draft.save(&path).expect("save");
        assert_eq!(Draft::load(&path).expect("load"), draft);
        Draft::default().save(&path).expect("empty");
        assert!(!path.exists());
    }

    /// The rig file keeps everything through saving and loading.
    #[test]
    fn rig_file_round_trip() {
        let mut view = View::new(Preset::Sagittal.plane(10.0));
        view.sections.push(Section::new(0.2, square(0.0, 1.0, 0.02)));
        let rig = RigFile {
            model: "farmer.glb".into(),
            source: Some(Source { video: "turntable_farmer.mp4".into(), options: vec!["--hull".into()] }),
            front_yaw: 10.0,
            front_confirmed: true,
            cuts: vec![Cut { name: "right arm".into(), views: vec![view], ..Cut::default() }],
            joints: Vec::new(),
        };
        let path = std::env::temp_dir().join(format!("fframes-rig-test-{}.rig.json", std::process::id()));
        rig.save(&path).expect("save");
        let back = RigFile::load(&path).expect("load");
        let _ = std::fs::remove_file(&path);
        assert_eq!(rig, back);
        assert_eq!(rig_path(std::path::Path::new("a/farmer_n_low.glb")), std::path::Path::new("a/farmer_n_low.rig.json"));
    }
}
