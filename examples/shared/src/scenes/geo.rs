//! From Berlin north to Mecklenburg-Vorpommern, the lakes, Feldberg, the
//! Haussee and the clinic on its shore: one camera flight over real
//! OpenStreetMap geometry (`geo_data.rs`, in km), then the view of the clinic from the lake.

use std::sync::LazyLock;

use fframes::{Duration, FFramesContext, Frame, Scene, Svgr};

use crate::geo_data::*;
use crate::timing::*;
use crate::ui::*;

timed_scene!(GeoScene, GEO);

/// Camera keys: (time, centre x km, centre y km, pixels per km).
const CAMERA: &[(f32, f32, f32, f32)] = &[
    // first the whole of Mecklenburg-Vorpommern with Berlin below it
    (0.0, -60.0, -80.0, 2.6),
    (1.6, -62.0, -52.0, 3.3),
    (3.6, -52.0, -46.0, 3.7),
    (5.4, 1.5, 7.0, 7.4),
    (7.0, 1.5, 6.0, 7.8),
    (9.3, 2.4, -38.6, 52.0),
    (10.7, HAUSSEE_C.0, HAUSSEE_C.1, 230.0),
    (12.3, CLINIC_C.0, CLINIC_C.1, 700.0),
    (14.0, CLINIC_C.0, CLINIC_C.1, 820.0),
];

/// Camera at `t`: centre and scale move together, the scale in log space so
/// a zoom feels like a steady descent.
pub fn camera(t: f32) -> (f32, f32, f32) {
    let k = CAMERA.iter().rposition(|c| c.0 <= t).unwrap_or(0);
    let a = CAMERA[k];
    let Some(&b) = CAMERA.get(k + 1) else {
        return (a.1, a.2, a.3);
    };
    let e = cubic_in_out(prog(t, a.0, b.0));
    let s = (a.3.ln() + (b.3.ln() - a.3.ln()) * e).exp();
    // the centre follows the zoom (in screen space) so targets do not swing out of frame
    let ez = if (b.3 - a.3).abs() > 1.0 { ((s - a.3) / (b.3 - a.3)).clamp(0.0, 1.0) } else { e };
    let ec = lerp(e, ez, 0.6);
    (lerp(a.1, b.1, ec), lerp(a.2, b.2, ec), s)
}

pub fn to_screen(t: f32, p: (f32, f32)) -> (f32, f32) {
    let (cx, cy, s) = camera(t);
    (960.0 + (p.0 - cx) * s, 540.0 + (p.1 - cy) * s)
}

/// Where Berlin is on the first frame: the Gary panel shrinks into it.
pub fn berlin_on_screen() -> (f32, f32) {
    to_screen(0.0, BERLIN_C)
}

static ROUTE_LEN: LazyLock<Vec<f32>> = LazyLock::new(|| {
    let mut acc = vec![0.0];
    for w in ROUTE.windows(2) {
        let d = ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt();
        acc.push(acc.last().unwrap() + d);
    }
    acc
});

/// The route drawn up to `f` (0..1) and the point where it ends.
fn route_part(f: f32) -> (String, (f32, f32)) {
    let total = *ROUTE_LEN.last().unwrap();
    let want = total * clamp01(f);
    let mut d = format!("M{} {}", ROUTE[0].0, ROUTE[0].1);
    let mut head = ROUTE[0];
    for i in 1..ROUTE.len() {
        let (l0, l1) = (ROUTE_LEN[i - 1], ROUTE_LEN[i]);
        if l1 <= want {
            d.push_str(&format!(" L{} {}", ROUTE[i].0, ROUTE[i].1));
            head = ROUTE[i];
        } else {
            let u = (want - l0) / (l1 - l0).max(1e-6);
            head = (lerp(ROUTE[i - 1].0, ROUTE[i].0, u), lerp(ROUTE[i - 1].1, ROUTE[i].1, u));
            d.push_str(&format!(" L{} {}", head.0, head.1));
            break;
        }
    }
    (d, head)
}

const WORDS: &[Kw] = &[
    kw("two hours north", Face::Mono, 34.0),
    kw("Mecklenburg-Vorpommern", Face::Serif, 62.0),
    kw("forests", Face::Serif, 84.0),
    kw("2,000+ lakes", Face::Display, 64.0),
    kw("Feldberger Seenlandschaft", Face::Mono, 28.0),
    kw("Haussee", Face::Display, 92.0),
    kw("Klinik am Haussee", Face::Serif, 66.0),
];
const KEYS: &[(f32, f32)] = &[(0.9, 0.0), (2.0, 1.0), (3.9, 2.0), (4.9, 3.0), (7.6, 4.0), (9.9, 5.0), (11.3, 6.0)];

/// A label pinned to a map point, with a short leader line.
fn pin(p: (f32, f32), text: &str, color: &'static str, size: f32, alpha: f32) -> Svgr<'static> {
    if alpha <= 0.0 {
        return Svgr::empty();
    }
    let (x, y) = p;
    let text = text.to_owned();
    // the labels 50 % larger than first drawn; the leader line grows with them
    let size = size * 1.5;
    let k = size / 18.0;
    fframes::svgr!(
        <g opacity={alpha}>
            <path d={format!("M{x} {y} L{} {} H{}", x + 18.0 * k, y - 18.0 * k, x + 40.0 * k)} fill="none" stroke={color} stroke-width="2" />
            <text x={x + 48.0 * k} y={y - 12.0 * k} font-family={MONO} font-weight="600" font-size={size} letter-spacing={3.0 * k} fill={color}>{text}</text>
        </g>
    )
}

/// The view from the lake holds on the clinic until here, then about 1.5 s dark before
/// the history.
const PHOTO_OUT: f32 = 18.4;
/// The aerial comes in only after the last zoom step onto the clinic (14.0).
const PHOTO_IN: f32 = 14.2;

fn window(t: f32, a: f32, b: f32) -> f32 {
    prog(t, a, a + 0.4) * (1.0 - prog(t, b - 0.4, b))
}

impl Scene for GeoScene {
    fn duration(&self) -> Duration<'_> {
        Self::frames()
    }

    fn render_frame<'a>(&'a self, frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        // the map keeps its original pace: it plays at 1x inside the 1.25x film
        let t = Self::t(&frame) / SPEED;
        let (cx, cy, s) = camera(t);
        let px = 1.0 / s; // one screen pixel in km
        let cam = format!("translate(960 540) scale({s}) translate({} {})", -cx, -cy);

        // land: MV lights up when it is named, turns to forest green at "forests"
        let mv_hi = prog(t, 1.9, 2.6);
        let forest = prog(t, 3.9, 4.8);
        let mv_fill = if forest > 0.0 { "#1c3427" } else { "#1b2a2a" };
        let mv_alpha = 0.55 + 0.45 * mv_hi.max(forest);

        // lakes appear south to north while "more than two thousand lakes" is spoken
        let n = LAKES.len() as f32;
        let lakes: Vec<Svgr> = LAKES
            .iter()
            .enumerate()
            .filter_map(|(i, (d, lx, ly, _))| {
                let at = 4.9 + i as f32 / n * 1.7;
                let k = expo_out(prog(t, at, at + 0.35));
                if k <= 0.0 {
                    return None;
                }
                let tr = format!("translate({lx} {ly}) scale({}) translate({} {})", 0.3 + 0.7 * k, -lx, -ly);
                Some(fframes::svgr!(<path d={*d} transform={tr} fill={LAKE} opacity={k * 0.85} />))
            })
            .collect();

        // the drive: drawn while "two hours north of Berlin" is spoken
        let (route_d, head) = route_part(cubic_in_out(prog(t, 0.8, 3.4)));
        let route_alpha = 1.0 - prog(t, 9.0, 10.0) * 0.6;

        // Feldberg in detail once the camera is close
        let detail = prog(s, 16.0, 45.0);
        let haussee_hi = prog(t, 9.8, 10.6);
        let trail_r = lerp(0.0, 7.0, quart_out(prog(t, 9.9, 12.6)));
        let trails: Vec<Svgr> = TRAILS
            .iter()
            .filter(|(_, dist)| *dist < trail_r)
            .map(|(d, dist)| {
                let o = clamp01((trail_r - dist) / 0.6) * 0.85;
                fframes::svgr!(<path d={*d} fill="none" stroke="#e3d3ad" stroke-width={1.6 * px} opacity={o} stroke-linecap="round" />)
            })
            .collect();
        let clinic_on = prog(t, 11.2, 11.7);

        let map = fframes::svgr!(
            <g transform={cam}>
                <path d={GERMANY} fill="#141b1c" stroke="#2b3536" stroke-width={1.2 * px} />
                <path d={MV} fill={mv_fill} opacity={mv_alpha} stroke={if mv_hi > 0.0 && t < 4.0 { SUN } else { "#3d4a48" }} stroke-width={2.0 * px} />
                <path d={BERLIN} fill="#283032" stroke="#64706f" stroke-width={1.5 * px} />
                <g opacity={detail}>
                    <path d={FELD_FOREST} fill="#1f3b2b" />
                    <path d={FELD_WATER} fill={LAKE} opacity="0.85" />
                    <path d={FELD_ROADS} fill="none" stroke="#45514f" stroke-width={1.2 * px} />
                    <path d={FELD_BUILDINGS} fill="#56625f" />
                </g>
                {lakes}
                <path d={HAUSSEE} fill={if haussee_hi > 0.0 { "#6cc4cf" } else { LAKE }} opacity={0.85 + 0.15 * haussee_hi}
                      stroke={BONE} stroke-width={2.0 * px} stroke-opacity={haussee_hi} />
                <g>{trails}</g>
                <path d={route_d} fill="none" stroke={SUN} stroke-width={3.0 * px} stroke-linecap="round" stroke-linejoin="round" opacity={route_alpha} />
                <path d={CLINIC_GROUNDS} fill={SUN} opacity={clinic_on * 0.25} />
                <path d={CLINIC_BUILDING} fill={SUN} opacity={clinic_on} />
            </g>
        );

        // screen space: car, pins, rings
        let car = to_screen(t, head);
        let car_alpha = prog(t, 0.8, 1.0) * (1.0 - prog(t, 4.6, 5.2));
        let berlin = to_screen(t, BERLIN_C);
        let clinic = to_screen(t, CLINIC_C);
        let ring = (t - 11.4).rem_euclid(1.2) / 1.2;
        let rings = if clinic_on > 0.0 {
            fframes::svgr!(
                <g>
                    <circle cx={clinic.0} cy={clinic.1} r={20.0 + ring * 70.0} fill="none" stroke={SUN} stroke-width="2" opacity={(1.0 - ring) * clinic_on} />
                </g>
            )
        } else {
            Svgr::empty()
        };
        let mid = to_screen(t, ROUTE[ROUTE.len() / 2]);

        // the view from the lake (vom_see.jpg) takes over at the end and stays on the clinic,
        // then a dark pause
        // the aerial stays until the end; the dark comes over it, so the map never shows again
        let photo_in = cubic_in_out(prog(t, PHOTO_IN, PHOTO_IN + 0.8));
        let photo = if photo_in > 0.0 {
            // the clinic itself (klinik-am-haussee.de, start01: on the quality page)
            ctx.get_image("clinic_start.jpg").map_or(Svgr::empty(), |img| {
                let k = 1.0 + 0.06 * (1.0 - prog(t, PHOTO_IN, PHOTO_OUT + 0.8));
                let tr = format!("translate(960 540) scale({k}) translate(-960 -540)");
                fframes::svgr!(
                    <g opacity={photo_in} transform={tr}>
                        <image href={img.href()} x="0" y="0" width="1920" height="1080" preserveAspectRatio="xMidYMid slice" />
                    </g>
                )
            })
        } else {
            Svgr::empty()
        };
        let pos = steps(t, -1.0, KEYS, 0.55);
        let words_alpha = 1.0 - prog(t, 12.4, 12.9);

        fframes::svgr!(
            <g>
                {map}
                <circle cx={berlin.0} cy={berlin.1} r="6" fill={SUN} opacity={1.0 - prog(t, 5.0, 6.0)} />
                <circle cx={car.0} cy={car.1} r="7" fill={BONE} opacity={car_alpha} />
                {rings}
                {pin(berlin, "BERLIN", BONE, 18.0, window(t, 0.3, 5.0))}
                {pin(to_screen(t, (-95.0, -120.0)), "MECKLENBURG-VORPOMMERN", SUN, 18.0, window(t, 1.9, 5.2))}
                {pin(mid, "136 KM · 1 H 52 MIN", BONE, 16.0, window(t, 2.6, 5.2))}
                {pin(to_screen(t, (2.0, -37.0)), "FELDBERGER SEENLANDSCHAFT", BONE, 18.0, window(t, 8.3, 10.0))}
                {pin(to_screen(t, HAUSSEE_C), "HAUSSEE", BONE, 18.0, window(t, 10.0, 12.2))}
                {pin(clinic, "KLINIK AM HAUSSEE", SUN, 20.0, window(t, 11.4, 12.9))}
                {photo}
                <rect width="980" height="1080" fill="url(#geo-shade)" opacity={words_alpha} />
                <defs>
                    <linearGradient id="geo-shade" x1="0" y1="0" x2="1" y2="0">
                        <stop offset="0" stop-color={BG} stop-opacity="0.92" />
                        <stop offset="1" stop-color={BG} stop-opacity="0" />
                    </linearGradient>
                </defs>
                {keyword_column(WORDS, pos, 190.0, 540.0, SUN, words_alpha)}
                <rect width="1920" height="1080" fill={BG} opacity={cubic_in_out(prog(t, PHOTO_OUT, PHOTO_OUT + 0.8))} />
            </g>
        )
    }
}
