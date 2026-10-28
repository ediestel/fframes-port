//! A demo of the `fframes::vector` object features on the Golden Spurs window, without
//! narration: every step shows one capability and names the call that produces it.
//!
//! 1. The traced window: one cached piece tree (`PieceTree::all`).
//! 2. Automatic clusters (`fframes-vector clusters`), as a false-colour tree
//!    (`PieceDocument::colored_by_group`).
//! 3. Objects from hand-drawn zones snapped to the clusters (`group --clusters`) and nested
//!    by `rename` (`battle/french/knights`).
//! 4. A spotlight tour: `tree.group(name)` lit, `tree.others(name)` dimmed, the camera framed
//!    on `doc.group_bbox(name)`. Parents light their sub-objects (`banners`, `rose`, `battle`).
//!
//! Data: `vector/demo_objects.json`, `vector/demo_clusters.json` (see `vector/demo_names.json`).

use fframes::vector::{PieceDocument, PieceTree};
use fframes::{Color, Duration, FFramesContext, Frame, Scene, Scenes, Svgr, Transform, Video};
use std::sync::OnceLock;

pub const WIDTH: usize = 1920;
pub const HEIGHT: usize = 1080;
const FPS: usize = 30;

const OBJECTS: &str = include_str!("../vector/demo_objects.json");
const CLUSTERS: &str = include_str!("../vector/demo_clusters.json");

const SANS: &str = "Helvetica Neue";
const MONO: &str = "Menlo";

struct Data {
    doc: PieceDocument,
    window: PieceTree,
    clusters: PieceTree,
    objects: PieceTree,
    /// (name, x, y) in document pixels: each object's name on its largest piece
    labels: Vec<(String, f32, f32)>,
}

fn load(json: &str, what: &str) -> PieceDocument {
    PieceDocument::from_json(json).unwrap_or_else(|e| {
        eprintln!("{what}: {e}");
        PieceDocument { width: 1, height: 1, scale: 1, palette: vec![], pieces: vec![] }
    })
}

fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        let doc = load(OBJECTS, "vector/demo_objects.json");
        let clusters = load(CLUSTERS, "vector/demo_clusters.json");
        let mut largest: Vec<(String, f32, [f32; 4])> = Vec::new();
        for p in &doc.pieces {
            if p.group.is_empty() {
                continue;
            }
            match largest.iter_mut().find(|l| l.0 == p.group) {
                Some(l) if l.1 < p.area => *l = (p.group.clone(), p.area, p.bbox),
                Some(_) => {}
                None => largest.push((p.group.clone(), p.area, p.bbox)),
            }
        }
        let labels = largest
            .into_iter()
            .map(|(n, _, b)| (n, b[0] + b[2] / 2.0, b[1] + b[3] / 2.0))
            .collect();
        Data {
            window: PieceTree::new(&doc, "demo-window"),
            clusters: PieceTree::new(&clusters.colored_by_group(), "demo-clusters"),
            objects: PieceTree::new(&doc.colored_by_group(), "demo-objects"),
            labels,
            doc,
        }
    })
}

/// What a beat shows.
#[derive(Clone, Copy)]
enum Show {
    Window,
    Clusters,
    Objects,
    Spot(&'static str),
    End,
}

struct Beat {
    show: Show,
    secs: f32,
    title: &'static str,
    code: &'static str,
}

const BEATS: [Beat; 12] = [
    Beat { show: Show::Window, secs: 6.0, title: "A traced window: 6,765 pieces, one cached tree", code: "PieceTree::new(&doc, salt).all()" },
    Beat { show: Show::Clusters, secs: 7.0, title: "Automatic clusters: like-coloured glass across the lead (870)", code: "fframes-vector clusters window.json" },
    Beat { show: Show::Objects, secs: 8.0, title: "Objects: hand-drawn zones snapped to the clusters, names nested", code: "fframes-vector group window.json zones.svg --clusters; rename names.json" },
    Beat { show: Show::Spot("banners/flanders"), secs: 5.5, title: "One object lit, everything else dimmed", code: "tree.group(\"banners/flanders\")  tree.others(\"banners/flanders\")" },
    Beat { show: Show::Spot("banners"), secs: 5.5, title: "A parent lights its sub-objects: both banners", code: "tree.group(\"banners\")" },
    Beat { show: Show::Spot("rose/lion"), secs: 5.0, title: "The lion of Flanders", code: "tree.group(\"rose/lion\")" },
    Beat { show: Show::Spot("rose"), secs: 5.0, title: "The rose window with its lion", code: "tree.group(\"rose\")" },
    Beat { show: Show::Spot("battle/flemish/militia"), secs: 5.5, title: "The Flemish militia, camera framed on the object", code: "doc.group_bbox(\"battle/flemish/militia\")" },
    Beat { show: Show::Spot("battle/french"), secs: 5.5, title: "The French: knights, horse, lances, the fallen knight", code: "tree.group(\"battle/french\")" },
    Beat { show: Show::Spot("spurs"), secs: 5.0, title: "The golden spurs", code: "tree.group(\"spurs\")" },
    Beat { show: Show::Spot("battle"), secs: 5.5, title: "The whole battle", code: "tree.group(\"battle\")" },
    Beat { show: Show::End, secs: 5.0, title: "", code: "" },
];

fn total() -> f32 {
    BEATS.iter().map(|b| b.secs).sum()
}

/// The beat at `t`, its start and its index.
fn beat_at(t: f32) -> (usize, f32) {
    let mut start = 0.0;
    for (i, b) in BEATS.iter().enumerate() {
        if t < start + b.secs || i + 1 == BEATS.len() {
            return (i, start);
        }
        start += b.secs;
    }
    (BEATS.len() - 1, start)
}

fn ease(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    u * u * u * (u * (u * 6.0 - 15.0) + 10.0)
}

/// A camera: centre and visible height in document pixels.
#[derive(Clone, Copy)]
struct Cam {
    cx: f32,
    cy: f32,
    h: f32,
}

fn full() -> Cam {
    let d = &data().doc;
    Cam { cx: d.width as f32 / 2.0, cy: d.height as f32 / 2.0, h: d.height as f32 * 1.08 }
}

/// The frame around an object: its box with a margin, at the frame's aspect, never
/// closer than a sixth of the window's height.
fn frame_on(name: &str) -> Cam {
    let d = &data().doc;
    let Some([x, y, w, h]) = d.group_bbox(name) else { return full() };
    let aspect = WIDTH as f32 / HEIGHT as f32;
    let vh = (h.max(w / aspect) * 1.3).clamp(d.height as f32 / 6.0, d.height as f32 * 1.08);
    Cam { cx: x + w / 2.0, cy: y + h / 2.0, h: vh }
}

/// Where the camera rests during beat `i` (with a slow push in).
fn rest(i: usize, u: f32) -> Cam {
    let c = match BEATS[i].show {
        Show::Spot(name) => frame_on(name),
        _ => full(),
    };
    Cam { h: c.h * (1.0 - 0.06 * u), ..c }
}

fn lerp_cam(a: Cam, b: Cam, u: f32) -> Cam {
    let h = (a.h.ln() + (b.h.ln() - a.h.ln()) * u).exp();
    Cam { cx: a.cx + (b.cx - a.cx) * u, cy: a.cy + (b.cy - a.cy) * u, h }
}

/// The camera moves from the previous beat's rest during the first 1.4 s of a beat.
fn camera_at(t: f32) -> Cam {
    let (i, start) = beat_at(t);
    let u = (t - start) / BEATS[i].secs;
    let here = rest(i, u);
    if i == 0 {
        return here;
    }
    let from = rest(i - 1, 1.0);
    lerp_cam(from, here, ease((t - start) / 1.4))
}

fn transform(c: Cam) -> Transform {
    let k = f64::from(HEIGHT as f32 / c.h);
    let vx = f64::from(c.cx) - WIDTH as f64 / k / 2.0;
    let vy = f64::from(c.cy) - HEIGHT as f64 / k / 2.0;
    Transform { translate_x: -k * vx, translate_y: -k * vy, ..Transform::scale(k) }
}

/// Weights of the window, cluster and object views at `t` (cross-fades of 1.2 s).
fn views(t: f32) -> (f32, f32, f32) {
    let (i, start) = beat_at(t);
    let k = ease((t - start) / 1.2);
    let one = |s: Show| match s {
        Show::Clusters => (0.0, 1.0, 0.0),
        Show::Objects => (0.0, 0.0, 1.0),
        _ => (1.0, 0.0, 0.0),
    };
    let now = one(BEATS[i].show);
    let before = if i == 0 { now } else { one(BEATS[i - 1].show) };
    (
        before.0 + (now.0 - before.0) * k,
        before.1 + (now.1 - before.1) * k,
        before.2 + (now.2 - before.2) * k,
    )
}

/// How dark the rest of the picture is during a spotlight: it falls after the camera
/// arrives and rises before the beat ends.
fn spot_dim(t: f32) -> f32 {
    let (i, start) = beat_at(t);
    let local = t - start;
    let secs = BEATS[i].secs;
    0.82 * ease((local - 1.0) / 0.8) * ease((secs - local) / 0.6)
}

/// Black over everything: lights up at the start, down at the end.
fn darkness(t: f32) -> f32 {
    let up = 1.0 - ease(t / 2.0);
    let down = ease((t - (total() - 2.5)) / 2.5);
    up.max(down)
}

fn caption(t: f32) -> Svgr<'static> {
    let (i, start) = beat_at(t);
    let b = &BEATS[i];
    if b.title.is_empty() {
        return Svgr::empty();
    }
    let local = t - start;
    let k = ease((local - 0.4) / 0.6) * ease((b.secs - local) / 0.4);
    let title = b.title.to_owned();
    let code = b.code.to_owned();
    fframes::svgr!(
        <g opacity={k}>
            <rect x="0" y="930" width="1920" height="150" fill="#000000" opacity="0.62" />
            <text x="80" y="990" font-family={SANS} font-size="38" fill="#f4efe4">{title}</text>
            <text x="80" y="1040" font-family={MONO} font-size="26" fill="#e3b84a">{code}</text>
        </g>
    )
}

fn labels(cam: Cam, opacity: f32) -> Svgr<'static> {
    if opacity <= 0.001 {
        return Svgr::empty();
    }
    let k = HEIGHT as f32 / cam.h;
    let (vx, vy) = (cam.cx - WIDTH as f32 / k / 2.0, cam.cy - HEIGHT as f32 / k / 2.0);
    let out: Vec<Svgr<'static>> = data()
        .labels
        .iter()
        .map(|(name, x, y)| {
            let fx = (x - vx) * k;
            let fy = (y - vy) * k;
            let name = name.clone();
            fframes::svgr!(
                <text x={fx as f32} y={fy as f32} text-anchor="middle" font-family={SANS} font-size="17"
                      fill="#ffffff" stroke="#000000" stroke-width="3.5" paint-order="stroke">{name}</text>
            )
        })
        .collect();
    fframes::svgr!(<g opacity={opacity}>{out}</g>)
}

#[derive(Debug)]
pub struct DemoScene;

impl Scene for DemoScene {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(total())
    }
    fn name(&self) -> &'static str {
        "Demo"
    }
    fn render_frame<'a>(&'a self, _frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        Svgr::empty()
    }
}

#[derive(Debug)]
pub struct Demo;

impl Video for Demo {
    const FPS: usize = FPS;
    const WIDTH: usize = WIDTH;
    const HEIGHT: usize = HEIGHT;
    const BACKGROUND_COLOR: Color = Color::BLACK;

    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(total())
    }
    fn audio(&self) -> fframes::AudioMap<'_> {
        fframes::AudioMap::none()
    }
    fn define_scenes(&self) -> Scenes<'_> {
        Scenes::from(vec![&DemoScene as &dyn Scene])
    }
    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = frame.seconds();
        let d = data();
        let cam_at = camera_at(t);
        let cam = transform(cam_at);
        let (w, c, o) = views(t);
        let (i, _) = beat_at(t);
        let layer = |tree: &PieceTree, k: f32| {
            if k <= 0.001 {
                Svgr::empty()
            } else {
                let all = Svgr::from(tree.all());
                fframes::svgr!(<g opacity={k}><g transform={cam}>{all}</g></g>)
            }
        };
        let picture = match BEATS[i].show {
            Show::Spot(name) => {
                let lit = Svgr::from(d.window.group(name));
                let others = Svgr::from(d.window.others(name));
                let dim = spot_dim(t);
                // the object colours fade out over the first spotlight
                let fading = layer(&d.objects, o);
                fframes::svgr!(
                    <g>
                        <g transform={cam}>{others}</g>
                        <rect x="0" y="0" width="1920" height="1080" fill="#000000" opacity={dim} />
                        <g transform={cam}>{lit}</g>
                        {fading}
                    </g>
                )
            }
            _ => {
                let (wl, cl, ol) = (layer(&d.window, w), layer(&d.clusters, c), layer(&d.objects, o));
                fframes::svgr!(<g>{wl}{cl}{ol}</g>)
            }
        };
        let names = labels(cam_at, o);
        let words = caption(t);
        let dark = darkness(t);
        fframes::svgr!(
            <svg xmlns="http://www.w3.org/2000/svg" width={WIDTH} height={HEIGHT}>
                <rect x="0" y="0" width="1920" height="1080" fill="#141320" />
                {picture}
                {names}
                {words}
                <rect x="0" y="0" width="1920" height="1080" fill="#000000" opacity={dark} />
            </svg>
        )
    }
}
