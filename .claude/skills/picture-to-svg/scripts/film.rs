//! "The Battle of the Golden Spurs": a narrated camera film over the traced window
//! (`KI_guldensporenslag_edit_traced.svg`). The narration (`film/narration.txt`, one
//! sentence per line, recorded to `film/vo/vo_NN.wav`) sets the timing: every
//! sentence has a shot, the camera eases to it just before the sentence starts and
//! drifts slowly while it is spoken. Durations come from `film/vo/durations.txt`,
//! so a new recording only needs a new durations file.
//!
//! Camera positions are in the coordinates of the flat source picture
//! (`KI_guldensporenslag_edit.png`, 1664 x 2496); the traced SVG is the same
//! picture at 3x.

use crate::window_tree;
use fframes::{
    AudioMap, AudioTimestamp, AudioTrack, Color, Duration, FFramesContext, Frame, Scene, Scenes,
    Svgr, Transform, Video,
};
use std::sync::OnceLock;

pub const WIDTH: usize = 1920;
pub const HEIGHT: usize = 1080;
const FPS: usize = 30;

pub(crate) const TRACED: &str = include_str!("../KI_guldensporenslag_edit_traced.svg");
const DURATIONS: &str = include_str!("../film/vo/durations.txt");
/// The traced SVG is the source picture at this scale.
const SCALE: f32 = 3.0;
const SRC_W: f32 = 1664.0;
const SRC_H: f32 = 2496.0;

/// Picture before the first word, pauses inside and between beats, picture after the last word.
const LEAD_IN: f32 = 3.0;
const GAP_SENTENCE: f32 = 0.7;
const GAP_BEAT: f32 = 1.8;
const TAIL: f32 = 4.5;

/// A camera position: centre and visible height in source-picture pixels.
#[derive(Clone, Copy, Debug)]
struct Cam {
    cx: f32,
    cy: f32,
    h: f32,
}

const fn cam(cx: f32, cy: f32, h: f32) -> Cam {
    Cam { cx, cy, h }
}

/// One shot per sentence: where the camera arrives, how long after the sentence starts
/// it arrives, and where it has drifted to when the next move begins.
struct Shot {
    arrive: Cam,
    settle: f32,
    drift: Cam,
}

const FULL: Cam = cam(832.0, 1248.0, 2700.0);

const SHOTS: [Shot; 16] = [
    // 1 open: the whole window, lights come up, slow push in
    Shot { arrive: FULL, settle: 0.0, drift: cam(832.0, 1180.0, 2300.0) },
    // 2 the king imprisons the count: the lion of Flanders in the rose
    Shot { arrive: cam(838.0, 345.0, 820.0), settle: 1.8, drift: cam(838.0, 350.0, 720.0) },
    // 3 rebels in Bruges: the Flemish banner
    Shot { arrive: cam(1140.0, 860.0, 760.0), settle: 1.5, drift: cam(1150.0, 880.0, 700.0) },
    // 4 Robert of Artois: the French banner and lances
    Shot { arrive: cam(520.0, 960.0, 600.0), settle: 1.8, drift: cam(530.0, 1010.0, 560.0) },
    // 5 armoured knights: the leading knight with the shield of France
    Shot { arrive: cam(540.0, 1205.0, 560.0), settle: 1.5, drift: cam(555.0, 1215.0, 500.0) },
    // 6 weavers and craftsmen: the faces of the militia
    Shot { arrive: cam(1250.0, 1380.0, 560.0), settle: 1.8, drift: cam(1260.0, 1395.0, 510.0) },
    // 7 Guy of Namur and William of Juelich: the man with the goedendag in front
    Shot { arrive: cam(1150.0, 1720.0, 800.0), settle: 1.5, drift: cam(1160.0, 1700.0, 740.0) },
    // 8 standing close together: the whole militia
    Shot { arrive: cam(1230.0, 1550.0, 1400.0), settle: 2.0, drift: cam(1230.0, 1560.0, 1300.0) },
    // 9 brooks and wet ground: along the water
    Shot { arrive: cam(560.0, 2080.0, 760.0), settle: 1.8, drift: cam(1150.0, 2150.0, 760.0) },
    // 10 the knights attack: both lancets
    Shot { arrive: cam(832.0, 1450.0, 1650.0), settle: 1.8, drift: cam(832.0, 1450.0, 1550.0) },
    // 11 Robert of Artois is killed: the fallen knight
    Shot { arrive: cam(640.0, 1560.0, 620.0), settle: 1.5, drift: cam(645.0, 1560.0, 560.0) },
    // 12 the victors collect the spurs: spurs in the grass
    Shot { arrive: cam(1200.0, 1900.0, 800.0), settle: 1.8, drift: cam(1230.0, 1930.0, 740.0) },
    // 13 hung in the Church of Our Lady: the largest spur
    Shot { arrive: cam(1400.0, 2170.0, 560.0), settle: 1.6, drift: cam(1400.0, 2170.0, 500.0) },
    // 14 Guldensporenslag: the inscription
    Shot { arrive: cam(400.0, 2335.0, 420.0), settle: 1.6, drift: cam(470.0, 2335.0, 400.0) },
    // 15 organised infantry: back to the whole window
    Shot { arrive: FULL, settle: 3.0, drift: cam(832.0, 1248.0, 2600.0) },
    // 16 the eleventh of July: hold the window
    Shot { arrive: cam(832.0, 1248.0, 2600.0), settle: 0.5, drift: cam(832.0, 1248.0, 2750.0) },
];

/// The schedule derived from the recording.
struct Plan {
    /// (file, start second) per sentence
    voice: Vec<(String, f32)>,
    /// camera keys (second, cam), increasing in time
    keys: Vec<(f32, Cam)>,
    total: f32,
}

fn plan() -> &'static Plan {
    static PLAN: OnceLock<Plan> = OnceLock::new();
    PLAN.get_or_init(|| {
        // durations.txt: "NN beat seconds"
        let rows: Vec<(String, String, f32)> = DURATIONS
            .lines()
            .filter_map(|l| {
                let mut it = l.split_whitespace();
                Some((it.next()?.to_string(), it.next()?.to_string(), it.next()?.parse().ok()?))
            })
            .collect();
        let mut voice = Vec::new();
        let mut starts = Vec::new();
        let mut ends = Vec::new();
        let mut t = LEAD_IN;
        for (i, (n, beat, d)) in rows.iter().enumerate() {
            if i > 0 {
                t += if rows[i - 1].1 == *beat { GAP_SENTENCE } else { GAP_BEAT };
            }
            voice.push((format!("vo_{n}.wav"), t));
            starts.push(t);
            t += d;
            ends.push(t);
        }
        let total = t + TAIL;

        let mut keys: Vec<(f32, Cam)> = vec![(0.0, SHOTS[0].arrive)];
        for (i, shot) in SHOTS.iter().enumerate().take(starts.len()) {
            if i > 0 {
                // leave the previous drift a moment before the sentence starts
                let depart = starts[i] - 0.35;
                keys.push((depart, SHOTS[i - 1].drift));
                keys.push((starts[i] + shot.settle, shot.arrive));
            }
        }
        let last = SHOTS.len().min(starts.len()) - 1;
        keys.push((total, SHOTS[last].drift));
        keys.dedup_by(|b, a| (b.0 - a.0).abs() < 1e-3);

        Plan { voice, keys, total }
    })
}

fn ease(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    // smootherstep: gentle start and stop, like a camera on a slider
    u * u * u * (u * (u * 6.0 - 15.0) + 10.0)
}

fn camera_at(t: f32) -> Cam {
    let keys = &plan().keys;
    if t <= keys[0].0 {
        return keys[0].1;
    }
    for w in keys.windows(2) {
        let ((t0, a), (t1, b)) = (w[0], w[1]);
        if t <= t1 {
            let u = ease((t - t0) / (t1 - t0).max(1e-3));
            // zoom in log space so the speed of the zoom looks even
            let h = (a.h.ln() + (b.h.ln() - a.h.ln()) * u).exp();
            return cam(a.cx + (b.cx - a.cx) * u, a.cy + (b.cy - a.cy) * u, h);
        }
    }
    keys[keys.len() - 1].1
}

/// Darkness over the picture: lights come up at the start, go down at the end.
fn darkness(t: f32) -> f32 {
    let total = plan().total;
    let up = 1.0 - ease(t / 3.0);
    let down = ease((t - (total - 3.0)) / 3.0);
    up.max(down)
}

/// The camera as a transform from source-picture space at 3x (the traced SVG) to the frame.
fn camera_transform(t: f32) -> Transform {
    let c = camera_at(t);
    let vh = f64::from(c.h * SCALE);
    let k = HEIGHT as f64 / vh;
    let vw = vh * WIDTH as f64 / HEIGHT as f64;
    let vx = f64::from(c.cx * SCALE) - vw / 2.0;
    let vy = f64::from(c.cy * SCALE) - vh / 2.0;
    Transform { translate_x: -k * vx, translate_y: -k * vy, ..Transform::scale(k) }
}

#[derive(Debug)]
pub struct FilmScene;

impl Scene for FilmScene {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(plan().total)
    }
    fn name(&self) -> &'static str {
        "Film"
    }
    fn render_frame<'a>(&'a self, _frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        Svgr::empty()
    }
}

#[derive(Debug)]
pub struct Film;

impl Video for Film {
    const FPS: usize = FPS;
    const WIDTH: usize = WIDTH;
    const HEIGHT: usize = HEIGHT;
    const BACKGROUND_COLOR: Color = Color::BLACK;

    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(plan().total)
    }
    fn audio(&self) -> AudioMap<'_> {
        let p = plan();
        let mut tracks: Vec<AudioTrack> = p
            .voice
            .iter()
            .map(|(f, s)| AudioTrack::new(f.as_str(), AudioTimestamp::Second(*s)..AudioTimestamp::Eof).voice())
            .collect();
        // placeholder church bell under the last line
        let bell_at = p.voice.last().map(|v| v.1 + 1.0).unwrap_or(0.0);
        tracks.push(
            AudioTrack::new("bell.wav", AudioTimestamp::Second(bell_at)..AudioTimestamp::Eof).gain_db(-14.),
        );
        AudioMap::from(tracks)
    }
    fn define_scenes(&self) -> Scenes<'_> {
        Scenes::from(vec![&FilmScene as &dyn Scene])
    }
    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        // the window is a static tree (window_tree): parsed once, recorded once by the
        // renderer, replayed under the camera transform; only the camera and the
        // darkness change per frame
        let t = frame.seconds();
        let camera = camera_transform(t);
        let dark = darkness(t);
        fframes::svgr!(
            <svg xmlns="http://www.w3.org/2000/svg" width={WIDTH} height={HEIGHT}>
                <rect x="0" y="0" width="1920" height="1080" fill="#141320" />
                <g transform={camera}>{window_tree::window()}</g>
                <rect x="0" y="0" width="1920" height="1080" fill="#000000" opacity={dark} />
            </svg>
        )
    }
}
