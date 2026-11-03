//! Dix-Hallpike manoeuvre, brief `GP/image_prompts/briefs/ch01_P2_B_dix-hallpike.txt`,
//! in the style of `thieme_atlas_style_reference_sheet.png`.
//!
//! The patient sits lengthwise on the couch, legs out; the examiner stands
//! behind the head end and turns the head 45 degrees to the patient's right
//! (towards the viewer). Then the patient is brought rapidly back to lying,
//! the head still turned and hanging 20 degrees below the horizontal beyond
//! the couch end, held in the examiner's hands. The inset shows the right eye
//! with torsional upbeating nystagmus: slow drift, fast phase with the upper
//! pole towards the lower (right) ear, after a short latency, crescendo then
//! decrescendo.
//!
//! Patient and examiner are traced rigs (`trace_side_rigs.py`): the patient
//! from the JVP figure of the couch-positions template, cut at hip and neck;
//! the examiner from the profile figure of the geriatric template, the arm
//! cut at the shoulder. The couch is drawn in the reference's furniture style.
//! The short adds the steps as captions; the plate is the brief's 2:1 strip
//! without text.

use fframes::Svgr;
use fframes::params::Params;
use std::f32::consts::PI;
use std::sync::{LazyLock, OnceLock};

use crate::puppet::{FILL, INK, LINE, Rig};
use crate::rig::{V2, View, arc, arrow, lerp, v2};

/// The scene's angles and title, from `dix_hallpike.toml`; every name the scene reads is
/// checked once here, so a missing or misspelt one stops the video at start.
static PARAMS: LazyLock<Params> = LazyLock::new(|| {
    let params = Params::parse(include_str!("../dix_hallpike.toml"))
        .unwrap_or_else(|problems| panic!("dix_hallpike.toml:\n{}", fframes::params::problems_text(&problems)));
    let missing = params.missing(&["trunk_sit", "trunk_lie", "head_sit", "head_hang", "title"], &[]);
    assert!(missing.is_empty(), "dix_hallpike.toml lacks: {}", missing.join(", "));
    params
});

/// A number of `dix_hallpike.toml`, checked present at start.
fn num(name: &str) -> f32 {
    PARAMS.number(name).unwrap_or_else(|| panic!("dix_hallpike.toml: {name} is not a number"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `dix_hallpike.toml` has every name the scene reads.
    #[test]
    fn params_resolve() {
        assert_eq!(num("trunk_sit"), 46.0);
        assert_eq!(num("head_hang"), -48.0);
        assert_eq!(PARAMS.text("title"), Some("DIX-HALLPIKE TEST · RIGHT EAR"));
    }
}

/// Reference sheet tokens: background, furniture cream, soft off-white, terracotta.
pub const PAPER: &str = "#F5F6F0";
const CREAM: &str = "#F3F1E4";
const OFF_WHITE: &str = "#E2EAF5";
const TERRA: &str = "#C1735C";

/// Rig pixels per metre: both figures 1.75 m tall (patient head top to heel
/// 900 px along the body, examiner head top to sole 1314 px).
const KP: f32 = 1.75 / 900.0;
const KE: f32 = 1.75 / 1314.0;

/// Patient rig points: hip and neck pivots, head centre.
const P_HIP: V2 = v2(569.0, 389.0);
const P_NECK: V2 = v2(342.5, 206.5);
const P_HEAD: V2 = v2(308.0, 128.0);
/// Examiner rig points: shoulder pivot, palm, sole line.
const E_SHOULDER: V2 = v2(178.0, 360.0);
const E_PALM: V2 = v2(232.0, 860.0);
const E_SOLE: f32 = 1400.0;

/// Couch top and its ends (metres); the patient's hip sits at x = 0.
const COUCH_TOP: f32 = 0.72;
const COUCH_HEAD: f32 = -0.42;
const COUCH_FOOT: f32 = 1.55;
/// The rig's hip pivot is 81 px above the line the body lies on.
const HIP: V2 = v2(0.0, COUCH_TOP + 81.0 * KP);

// Trunk and head angles sitting and with the head hanging: `trunk_sit`, `trunk_lie`,
// `head_sit`, `head_hang` in `dix_hallpike.toml`.

/// Short timing, seconds.
pub const TURN: (f32, f32) = (2.2, 3.4);
pub const LIE: (f32, f32) = (6.4, 7.4);
pub const INSET: (f32, f32) = (10.4, 11.0);
pub const NYSTAGMUS: (f32, f32) = (11.6, 17.4);
pub const LEN: f32 = 19.0;

/// Captions of the short: start, end, step, text.
const CAPTIONS: [(f32, f32, &str, &str); 3] = [
    (0.6, 6.2, "1", "Patient sits on the couch, legs out. Turn the head 45° to the right."),
    (6.2, 10.2, "2", "Lay the patient back quickly: head 20° below the couch, still turned."),
    (10.2, LEN, "3", "Watch the eyes: torsional upbeat nystagmus = right posterior-canal BPPV."),
];

fn patient_rig() -> &'static Rig {
    static R: OnceLock<Rig> = OnceLock::new();
    R.get_or_init(|| Rig::parse(include_str!("../patient_rig.svg"), false))
}

fn examiner_rig() -> &'static Rig {
    static R: OnceLock<Rig> = OnceLock::new();
    R.get_or_init(|| Rig::parse(include_str!("../examiner_rig.svg"), false))
}

pub fn prog(x: f32, a: f32, b: f32) -> f32 {
    ((x - a) / (b - a)).clamp(0.0, 1.0)
}

pub fn ease_in_out(t: f32) -> f32 {
    if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}

pub fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Everything that changes over the short.
#[derive(Clone, Copy, Debug)]
pub struct State {
    pub turn: f32,
    pub lie: f32,
    pub turn_arrow: (f32, f32),
    pub lie_arrow: (f32, f32),
    pub inset: f32,
    /// Torsion of the eye (radians, counter-clockwise as seen) and its rise (metres).
    pub torsion: f32,
    pub rise: f32,
    /// 1 at the moment of a fast phase, decaying after it.
    pub beat: f32,
    /// Seconds into the short, for the captions; `None` on the plate.
    pub time: Option<f32>,
}

impl State {
    pub fn step_1() -> State {
        State { turn: 1.0, lie: 0.0, turn_arrow: (1.0, 1.0), lie_arrow: (0.0, 0.0), inset: 0.0, torsion: 0.0, rise: 0.0, beat: 0.0, time: None }
    }

    pub fn step_2() -> State {
        State { lie: 1.0, turn_arrow: (0.0, 0.0), lie_arrow: (1.0, 1.0), inset: 1.0, ..State::step_1() }
    }

    pub fn at(t: f32) -> State {
        let lie = ease_in_out(prog(t, LIE.0, LIE.1));
        // sawtooth: slow drift clockwise and down, fast phase back up and counter-clockwise
        let env = (PI * prog(t, NYSTAGMUS.0, NYSTAGMUS.1)).sin().max(0.0).powf(0.8);
        let period = 0.42;
        let ph = ((t - NYSTAGMUS.0).max(0.0) / period).fract();
        let slow = (ph / 0.85).min(1.0);
        let fast = ((ph - 0.85) / 0.15).clamp(0.0, 1.0);
        let saw = 0.5 - slow + fast;
        State {
            turn: ease_in_out(prog(t, TURN.0, TURN.1)),
            lie,
            turn_arrow: (ease_out(prog(t, TURN.0, TURN.1 - 0.1)), 1.0 - prog(t, LIE.0 - 0.4, LIE.0)),
            lie_arrow: (prog(t, LIE.0 + 0.05, LIE.1 + 0.05), 1.0 - prog(t, LIE.1 + 2.0, LIE.1 + 2.6)),
            inset: ease_out(prog(t, INSET.0, INSET.1)),
            torsion: env * 14f32.to_radians() * saw,
            rise: env * 0.014 * saw,
            beat: if t > NYSTAGMUS.0 { env * (1.0 - (ph - 0.85).rem_euclid(1.0) * 2.5).max(0.0) } else { 0.0 },
            time: Some(t),
        }
    }
}

/// `p` turned by `deg` (clockwise on screen) about `c`, in rig pixels (y down).
fn rot(p: V2, c: V2, deg: f32) -> V2 {
    let (s, co) = deg.to_radians().sin_cos();
    let d = p - c;
    c + v2(co * d.x - s * d.y, s * d.x + co * d.y)
}

/// Rig point to world metres, the rig's `anchor` placed at `at`, `k` metres per rig pixel.
fn to_world(p: V2, anchor: V2, at: V2, k: f32) -> V2 {
    v2(at.x + (p.x - anchor.x) * k, at.y - (p.y - anchor.y) * k)
}

/// Transform that draws a rig with its `anchor` at world point `at`.
fn placed(view: &View, anchor: V2, at: V2, k: f32) -> String {
    let p = view.p(at);
    format!("translate({:.2} {:.2}) scale({:.5}) translate({:.2} {:.2})", p.x, p.y, view.s * k, -anchor.x, -anchor.y)
}

/// The patient's angles and head (world) for a state.
struct Patient {
    trunk: f32,
    head: f32,
    centre: V2,
    up: V2,
}

fn patient(s: &State) -> Patient {
    let trunk = lerp(num("trunk_sit"), num("trunk_lie"), s.lie);
    let head = lerp(num("head_sit"), num("head_hang"), s.lie);
    let posed = |p: V2| to_world(rot(rot(p, P_NECK, head), P_HIP, trunk), P_HIP, HIP, KP);
    let (centre, neck) = (posed(P_HEAD), posed(P_NECK));
    Patient { trunk, head, centre, up: (centre - neck).norm() }
}

/// Sitting, the shoulder is where the straight arm just reaches the head;
/// for the lie-back the examiner steps back, clear of the hanging head,
/// and the hands go under it.
fn examiner_shoulder(lie: f32) -> V2 {
    let head = patient(&State::step_1()).centre;
    let reach = (E_PALM - E_SHOULDER).len() * KE;
    let y = (E_SOLE - E_SHOULDER.y) * KE;
    let sit = head.x - (reach * reach - (head.y - y).powi(2)).max(0.0).sqrt();
    let back = patient(&State::step_2()).centre.x - 0.36;
    v2(lerp(sit, back, lie), y)
}

/// Arm angle that points the palm from `shoulder` at `target` (world).
fn arm_angle(shoulder: V2, target: V2) -> f32 {
    let rest = E_PALM - E_SHOULDER;
    let to = v2(target.x - shoulder.x, shoulder.y - target.y);
    (to.angle() - rest.angle()).to_degrees()
}

fn couch(view: &View) -> Svgr<'static> {
    let r = |x0: f32, y0: f32, x1: f32, y1: f32, rr: f32| {
        let (a, b) = (view.p(v2(x0, y1)), view.p(v2(x1, y0)));
        fframes::svgr!(
            <rect x={a.x} y={a.y} width={b.x - a.x} height={b.y - a.y} rx={rr} fill={CREAM} stroke={INK} stroke-width={LINE} stroke-linejoin="round" />
        )
    };
    let (mat, frame, leg) = (0.085, 0.065, 0.05);
    fframes::svgr!(
        <g>
            {r(COUCH_HEAD + 0.10, 0.0, COUCH_HEAD + 0.10 + leg, COUCH_TOP - mat, 2.0)}
            {r(COUCH_FOOT - 0.10 - leg, 0.0, COUCH_FOOT - 0.10, COUCH_TOP - mat, 2.0)}
            {r(COUCH_HEAD + 0.04, COUCH_TOP - mat - frame, COUCH_FOOT - 0.04, COUCH_TOP - mat, 3.0)}
            {r(COUCH_HEAD, COUCH_TOP - mat, COUCH_FOOT, COUCH_TOP, view.m(0.025))}
        </g>
    )
}

/// Patient's right eye, face-on, in a ringed inset tied to the head by a leader.
fn inset(view: &View, head: V2, s: &State) -> Svgr<'static> {
    if s.inset <= 0.0 {
        return Svgr::empty();
    }
    let c = v2(1.25, 1.40);
    let r = 0.27 * s.inset;
    let from = head + (c - head).norm() * 0.14;
    let to = c - (c - head).norm() * r;
    let (a, b, pc) = (view.p(from), view.p(to), view.p(c));
    let (ew, eh) = (0.18 * s.inset, 0.075 * s.inset);
    let (l, rr) = (view.p(c + v2(-ew, 0.0)), view.p(c + v2(ew, 0.0)));
    let (top, bot) = (view.p(c + v2(0.0, eh * 2.0)), view.p(c + v2(0.0, -eh * 2.0)));
    let almond = format!(
        "M{:.1} {:.1}Q{:.1} {:.1} {:.1} {:.1}Q{:.1} {:.1} {:.1} {:.1}Z",
        l.x, l.y, top.x, top.y, rr.x, rr.y, bot.x, bot.y, l.x, l.y
    );
    let iris_c = view.p(c + v2(0.0, s.rise * s.inset));
    let (ir, pr) = (view.m(0.062 * s.inset), view.m(0.024 * s.inset));
    let turn = format!("rotate({:.2} {:.1} {:.1})", -s.torsion.to_degrees(), iris_c.x, iris_c.y);
    let marks: Vec<Svgr> = (0..8)
        .map(|i| {
            let a = i as f32 * PI / 4.0 + 0.2;
            let (p0, p1) = (iris_c + V2::polar(a, pr * 1.35), iris_c + V2::polar(a, ir * 0.82));
            fframes::svgr!(<path d={format!("M{:.1} {:.1}L{:.1} {:.1}", p0.x, p0.y, p1.x, p1.y)} stroke={INK} stroke-width={LINE * 0.6} stroke-linecap="round" />)
        })
        .collect();
    // upper pole towards the patient's right ear, which is the viewer's left
    let sweep = arc(c + v2(0.0, s.rise * s.inset), 0.105 * s.inset, 35f32.to_radians(), 150f32.to_radians(), 24);
    fframes::svgr!(
        <g>
            <path d={format!("M{:.1} {:.1}L{:.1} {:.1}", a.x, a.y, b.x, b.y)} stroke={INK} stroke-width={LINE * 0.8} />
            <circle cx={pc.x} cy={pc.y} r={view.m(r)} fill={OFF_WHITE} stroke={INK} stroke-width={LINE} />
            <defs><clipPath id="eye-clip"><path d={almond.clone()} /></clipPath></defs>
            <path d={almond.clone()} fill={PAPER} />
            <g clip-path="url(#eye-clip)">
                <g transform={turn}>
                    <circle cx={iris_c.x} cy={iris_c.y} r={ir} fill={FILL} stroke={INK} stroke-width={LINE} />
                    {marks}
                    <circle cx={iris_c.x} cy={iris_c.y} r={pr} fill={INK} />
                </g>
            </g>
            <path d={almond} fill="none" stroke={INK} stroke-width={LINE} stroke-linejoin="round" />
            {arrow(view, &sweep, 1.0, TERRA, LINE * (1.3 + 0.5 * s.beat), s.inset)}
        </g>
    )
}

/// Title and the step caption of the moment, fading in and out.
fn captions(t: f32) -> Svgr<'static> {
    let lines: Vec<Svgr> = CAPTIONS
        .iter()
        .filter(|(a, b, _, _)| t >= *a && t < *b)
        .map(|(a, b, step, text)| {
            let out = if *b >= LEN { 0.0 } else { prog(t, b - 0.4, *b) };
            let o = prog(t, *a, a + 0.4) * (1.0 - out);
            fframes::svgr!(
                <g opacity={o} font-family="Helvetica" fill={INK}>
                    <circle cx="96" cy="128" r="25" fill={TERRA} />
                    <text x="96" y="139" font-size="30" font-weight="700" fill={PAPER} text-anchor="middle">{*step}</text>
                    <text x="140" y="139" font-size="33" font-weight="600">{*text}</text>
                </g>
            )
        })
        .collect();
    fframes::svgr!(
        <g>
            <text x="71" y="66" font-family="Helvetica" font-size="24" font-weight="700" fill={INK} letter-spacing="1.5">{PARAMS.text("title").unwrap_or("")}</text>
            {lines}
        </g>
    )
}

/// The whole scene for one state, in one view.
pub fn scene(view: &View, s: &State) -> Svgr<'static> {
    let p = patient(s);
    let shoulder = examiner_shoulder(s.lie);
    // hands at the temples while sitting, under the occiput when lying
    let arm = arm_angle(shoulder, p.centre + p.up * (0.10 * (1.0 - s.lie)) + v2(-0.03, -0.06) * s.lie) as f64;
    let (trunk, head) = (p.trunk as f64, p.head as f64);
    // the forearm stays level (traced 10 degrees down): on the thighs sitting, on the couch lying
    let patient_d = patient_rig().outline(&|id| match id {
        "trunk" => trunk,
        "head" => head,
        "forearm" => -trunk - 10.0,
        _ => 0.0,
    });
    let examiner_d = examiner_rig().outline(&|id| if id == "arm" { arm } else { 0.0 });
    let line = |k: f32| LINE / (view.s * k);

    // 45 degrees about the head's long axis, drawn as a ring seen slightly from above
    let face = v2(p.up.y, -p.up.x);
    let ring = |a: f32| p.centre + face * (0.20 * a.cos()) - p.up * (0.02 + 0.065 * a.sin());
    let turn: Vec<V2> = (0..=24).map(|i| ring(lerp(-0.5, 1.25, i as f32 / 24.0))).collect();
    // path of the head about the hips, from sitting to hanging
    let (from, to) = (patient(&State::step_1()).centre - HIP, patient(&State::step_2()).centre - HIP);
    // counter-clockwise from above the hips round to the left; angles past pi wrap negative
    let end = if to.angle() < from.angle() { to.angle() + 2.0 * PI } else { to.angle() };
    let lie = arc(HIP, from.len() + 0.10, from.angle() + 0.10, end - 0.06, 40);
    fframes::svgr!(
        <g>
            {couch(view)}
            <path d={examiner_d} transform={placed(view, E_SHOULDER, shoulder, KE)} fill={OFF_WHITE} stroke={INK} stroke-width={line(KE)} stroke-linejoin="round" />
            <path d={patient_d} transform={placed(view, P_HIP, HIP, KP)} fill={FILL} stroke={INK} stroke-width={line(KP)} stroke-linejoin="round" />
            {arrow(view, &turn, s.turn_arrow.0, TERRA, LINE * 1.4, s.turn_arrow.1)}
            {arrow(view, &lie, s.lie_arrow.0, TERRA, LINE * 1.4, s.lie_arrow.1)}
            {inset(view, p.centre, s)}
            {match s.time { Some(t) => captions(t), None => Svgr::empty() }}
        </g>
    )
}
