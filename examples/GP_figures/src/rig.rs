//! The figure rig. A faceless adult in side view, posed in metres (y up) and
//! drawn as flat SVG in the `style.txt` palette: every segment is a closed
//! outline with round ends, one flat fill and the charcoal contour.
//!
//! A pose names only the pelvis, the torso and neck angles, the head turn and
//! where the wrists and ankles go; elbows and knees come from two-bone IK.
//! Poses interpolate field by field, so a movement is two poses and a curve.

use fframes::Svgr;
use std::ops::{Add, Mul, Sub};

pub const CREAM: &str = "#FAF8F4";
pub const INK: &str = "#2C3539";
pub const BLUE: &str = "#8DA9C4";
pub const TERRA: &str = "#C9735D";

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}

pub const fn v2(x: f32, y: f32) -> V2 {
    V2 { x, y }
}

impl V2 {
    pub fn polar(angle: f32, r: f32) -> V2 {
        v2(angle.cos() * r, angle.sin() * r)
    }
    pub fn len(self) -> f32 {
        self.x.hypot(self.y)
    }
    pub fn norm(self) -> V2 {
        let l = self.len();
        if l < 1e-6 { v2(1.0, 0.0) } else { self * (1.0 / l) }
    }
    pub fn angle(self) -> f32 {
        self.y.atan2(self.x)
    }
    pub fn lerp(self, o: V2, t: f32) -> V2 {
        self + (o - self) * t
    }
}

impl Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        v2(self.x + o.x, self.y + o.y)
    }
}

impl Sub for V2 {
    type Output = V2;
    fn sub(self, o: V2) -> V2 {
        v2(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f32> for V2 {
    type Output = V2;
    fn mul(self, k: f32) -> V2 {
        v2(self.x * k, self.y * k)
    }
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// World (metres, y up) to canvas pixels; `stroke` is the contour width in pixels.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub ox: f32,
    pub oy: f32,
    pub s: f32,
    pub stroke: f32,
}

impl View {
    pub fn p(&self, w: V2) -> V2 {
        v2(self.ox + w.x * self.s, self.oy - w.y * self.s)
    }
    pub fn m(&self, metres: f32) -> f32 {
        metres * self.s
    }
}

/// A flat-filled shape with the charcoal contour.
pub fn shape(view: &View, d: String, fill: &'static str) -> Svgr<'static> {
    fframes::svgr!(
        <path d={d} fill={fill} stroke={INK} stroke-width={view.stroke} stroke-linejoin="round" stroke-linecap="round" />
    )
}

/// One limb segment: round end of radius `ra` at `a`, of `rb` at `b`.
pub fn capsule(view: &View, a: V2, b: V2, ra: f32, rb: f32, fill: &'static str) -> Svgr<'static> {
    let (pa, pb) = (view.p(a), view.p(b));
    let (ra, rb) = (view.m(ra), view.m(rb));
    let d = (pb - pa).norm();
    let n = v2(-d.y, d.x);
    let (a1, b1, b2, a2) = (pa + n * ra, pb + n * rb, pb - n * rb, pa - n * ra);
    let path = format!(
        "M{:.1} {:.1}L{:.1} {:.1}A{rb:.1} {rb:.1} 0 0 0 {:.1} {:.1}L{:.1} {:.1}A{ra:.1} {ra:.1} 0 0 0 {:.1} {:.1}Z",
        a1.x, a1.y, b1.x, b1.y, b2.x, b2.y, a2.x, a2.y, a1.x, a1.y
    );
    shape(view, path, fill)
}

/// Two-bone IK: the middle joint for a chain `root`-(l1)-mid-(l2)-end reaching
/// for `target`. `bend` +1 bends the middle joint counter-clockwise of the
/// root-target line, -1 clockwise. Out of reach, the chain points straight at it.
pub fn ik2(root: V2, target: V2, l1: f32, l2: f32, bend: f32) -> (V2, V2) {
    let d = target - root;
    let dist = d.len().clamp((l1 - l2).abs() + 1e-4, l1 + l2 - 1e-4);
    let dir = d.norm();
    let at_root = ((l1 * l1 + dist * dist - l2 * l2) / (2.0 * l1 * dist)).clamp(-1.0, 1.0).acos();
    let mid = root + V2::polar(dir.angle() + bend * at_root, l1);
    (mid, root + dir * dist)
}

/// Segment lengths and half-widths in metres.
#[derive(Clone, Copy, Debug)]
pub struct Body {
    pub torso: f32,
    pub neck: f32,
    /// Half depth (front to back) and half height of the head.
    pub head: (f32, f32),
    pub upper_arm: f32,
    pub forearm: f32,
    pub hand: f32,
    pub thigh: f32,
    pub shin: f32,
    pub foot: f32,
    pub hip_r: f32,
    pub chest_r: f32,
}

pub const ADULT: Body = Body {
    torso: 0.52,
    neck: 0.08,
    head: (0.10, 0.12),
    upper_arm: 0.30,
    forearm: 0.27,
    hand: 0.09,
    thigh: 0.45,
    shin: 0.44,
    foot: 0.21,
    hip_r: 0.12,
    chest_r: 0.115,
};

pub const TALL: Body = Body {
    torso: 0.55,
    neck: 0.09,
    head: (0.10, 0.125),
    upper_arm: 0.31,
    forearm: 0.28,
    hand: 0.09,
    thigh: 0.47,
    shin: 0.46,
    foot: 0.24,
    hip_r: 0.115,
    chest_r: 0.12,
};

/// Fills: top (shirt, coat), bottom (trousers), skin and shoes.
#[derive(Clone, Copy, Debug)]
pub struct Look {
    pub top: &'static str,
    pub bottom: &'static str,
    pub skin: &'static str,
    pub shoe: &'static str,
}

/// Index 0 of the pairs is the near side (towards the viewer), 1 the far side.
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub pelvis: V2,
    /// Direction pelvis to neck base, radians.
    pub torso: f32,
    /// Direction neck base to head, radians; also the head's up axis.
    pub neck: f32,
    /// Head turn about its up axis: 0 profile, PI/2 facing the viewer.
    pub yaw: f32,
    pub wrist: [V2; 2],
    pub ankle: [V2; 2],
    /// Direction ankle to toes, radians.
    pub foot: [f32; 2],
}

impl Pose {
    pub fn lerp(&self, o: &Pose, t: f32) -> Pose {
        let pair = |a: [V2; 2], b: [V2; 2]| [a[0].lerp(b[0], t), a[1].lerp(b[1], t)];
        Pose {
            pelvis: self.pelvis.lerp(o.pelvis, t),
            torso: lerp(self.torso, o.torso, t),
            neck: lerp(self.neck, o.neck, t),
            yaw: lerp(self.yaw, o.yaw, t),
            wrist: pair(self.wrist, o.wrist),
            ankle: pair(self.ankle, o.ankle),
            foot: [lerp(self.foot[0], o.foot[0], t), lerp(self.foot[1], o.foot[1], t)],
        }
    }
}

/// Where every joint of a posed body is.
#[derive(Clone, Copy, Debug)]
pub struct Joints {
    pub pelvis: V2,
    pub neck_base: V2,
    pub shoulder: V2,
    pub head: V2,
    /// Head up axis and facing axis (unit vectors).
    pub up: V2,
    pub face: V2,
    pub yaw: f32,
    pub elbow: [V2; 2],
    pub wrist: [V2; 2],
    pub knee: [V2; 2],
    pub ankle: [V2; 2],
    pub foot: [f32; 2],
}

impl Body {
    pub fn solve(&self, p: &Pose) -> Joints {
        let neck_base = p.pelvis + V2::polar(p.torso, self.torso);
        let shoulder = p.pelvis + V2::polar(p.torso, self.torso - 0.05);
        let up = V2::polar(p.neck, 1.0);
        let head = neck_base + up * (self.neck + self.head.1 * 0.75);
        let arm = |w: V2| ik2(shoulder, w, self.upper_arm, self.forearm, -1.0);
        let leg = |a: V2| ik2(p.pelvis, a, self.thigh, self.shin, 1.0);
        let (arms, legs) = ([arm(p.wrist[0]), arm(p.wrist[1])], [leg(p.ankle[0]), leg(p.ankle[1])]);
        Joints {
            pelvis: p.pelvis,
            neck_base,
            shoulder,
            head,
            up,
            face: v2(up.y, -up.x),
            yaw: p.yaw,
            elbow: [arms[0].0, arms[1].0],
            wrist: [arms[0].1, arms[1].1],
            knee: [legs[0].0, legs[1].0],
            ankle: [legs[0].1, legs[1].1],
            foot: p.foot,
        }
    }
}

/// A drawn figure in three layers, so other figures can go between them:
/// far limbs, the body with near leg and head, the near arm.
pub struct Drawn {
    pub far: Svgr<'static>,
    pub body: Svgr<'static>,
    pub near_arm: Svgr<'static>,
}

fn leg(view: &View, b: &Body, l: &Look, j: &Joints, i: usize) -> Svgr<'static> {
    let toe = j.ankle[i] + V2::polar(j.foot[i], b.foot);
    let heel = j.ankle[i] - V2::polar(j.foot[i], 0.03);
    fframes::svgr!(
        <g>
            {capsule(view, heel, toe, 0.04, 0.03, l.shoe)}
            {capsule(view, j.knee[i], j.ankle[i], 0.055, 0.04, l.bottom)}
            {capsule(view, j.pelvis, j.knee[i], 0.085, 0.06, l.bottom)}
        </g>
    )
}

fn arm(view: &View, b: &Body, l: &Look, j: &Joints, i: usize) -> Svgr<'static> {
    let dir = (j.wrist[i] - j.elbow[i]).norm();
    let tip = j.wrist[i] + dir * b.hand;
    fframes::svgr!(
        <g>
            {capsule(view, j.wrist[i], tip, 0.03, 0.026, l.skin)}
            {capsule(view, j.elbow[i], j.wrist[i], 0.042, 0.033, l.top)}
            {capsule(view, j.shoulder, j.elbow[i], 0.052, 0.045, l.top)}
        </g>
    )
}

/// Faceless head: the outline, and a nose that sits on the outline in profile
/// and moves into the face as the head turns towards the viewer.
fn head(view: &View, b: &Body, l: &Look, j: &Joints) -> Svgr<'static> {
    let (rx, ry) = b.head;
    let c = view.p(j.head);
    let deg = -(j.up.angle() - std::f32::consts::FRAC_PI_2).to_degrees();
    let tr = format!("rotate({deg:.2} {:.1} {:.1})", c.x, c.y);
    let k = j.yaw.cos();
    let at = |f: f32, u: f32| view.p(j.head + j.face * f + j.up * u);
    let (bridge, tip, base) = (at(rx * 0.97 * k, 0.012), at((rx + 0.026) * k, -0.018), at(rx * 0.95 * k, -0.034));
    let nose = format!(
        "M{:.1} {:.1}L{:.1} {:.1}L{:.1} {:.1}",
        bridge.x, bridge.y, tip.x, tip.y, base.x, base.y
    );
    fframes::svgr!(
        <g>
            <ellipse cx={c.x} cy={c.y} rx={view.m(rx)} ry={view.m(ry)} transform={tr}
                     fill={l.skin} stroke={INK} stroke-width={view.stroke} />
            {shape(view, nose, l.skin)}
        </g>
    )
}

pub fn draw(view: &View, b: &Body, l: &Look, j: &Joints) -> Drawn {
    let chest = j.pelvis + (j.neck_base - j.pelvis).norm() * (b.torso - b.chest_r * 0.55);
    let waist = j.pelvis + (j.neck_base - j.pelvis).norm() * 0.1;
    Drawn {
        far: fframes::svgr!(<g>{leg(view, b, l, j, 1)}{arm(view, b, l, j, 1)}</g>),
        body: fframes::svgr!(
            <g>
                {capsule(view, j.neck_base, j.head, 0.045, 0.045, l.skin)}
                {capsule(view, j.pelvis, chest, b.hip_r, b.chest_r, l.top)}
                {capsule(view, j.pelvis, waist, b.hip_r, b.hip_r * 0.97, l.bottom)}
                {leg(view, b, l, j, 0)}
                {head(view, b, l, j)}
            </g>
        ),
        near_arm: arm(view, b, l, j, 0),
    }
}

/// An open stroke along `pts` (world), drawn to `progress` (0..1) of its points,
/// with an arrowhead on the drawn end.
pub fn arrow(view: &View, pts: &[V2], progress: f32, color: &'static str, width: f32, opacity: f32) -> Svgr<'static> {
    let n = ((pts.len() as f32 - 1.0) * progress.clamp(0.0, 1.0)).round() as usize + 1;
    if n < 3 || opacity <= 0.0 {
        return Svgr::empty();
    }
    let p: Vec<V2> = pts[..n].iter().map(|w| view.p(*w)).collect();
    let mut d = format!("M{:.1} {:.1}", p[0].x, p[0].y);
    for q in &p[1..] {
        d.push_str(&format!("L{:.1} {:.1}", q.x, q.y));
    }
    let end = p[n - 1];
    let dir = (end - p[n - 3]).norm();
    let side = v2(-dir.y, dir.x);
    let h = width * 4.2;
    let (l, r) = (end - dir * h + side * (h * 0.55), end - dir * h - side * (h * 0.55));
    let tip = end + dir * (width * 0.8);
    let head = format!("M{:.1} {:.1}L{:.1} {:.1}L{:.1} {:.1}Z", tip.x, tip.y, l.x, l.y, r.x, r.y);
    fframes::svgr!(
        <g opacity={opacity}>
            <path d={d} fill="none" stroke={color} stroke-width={width} stroke-linecap="round" stroke-linejoin="round" />
            <path d={head} fill={color} />
        </g>
    )
}

/// Points on an arc of radius `r` about `c` from angle `a0` to `a1`.
pub fn arc(c: V2, r: f32, a0: f32, a1: f32, n: usize) -> Vec<V2> {
    (0..=n).map(|i| c + V2::polar(lerp(a0, a1, i as f32 / n as f32), r)).collect()
}
