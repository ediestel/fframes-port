//! "Belegte Kodierung. Geprüfte Rechnung.": Video zur Prüfassistenz für die
//! Kodierprüfung im Krankenhaus, nach dem Exposé
//! `ntgt_desktop/docs/Kodierpruefung/kodierpruefung_expose.pdf` (18.10.2026).
//! Im Stil des Klinik-Videos: dunkle Seiten, Untertitel, ohne Ton
//! (1920 × 1080, 30 fps, 5:14 min), deutsch. Der Modellfall und alle
//! Falldaten sind fiktiv, wie im Exposé.

pub mod narration;
pub mod scenes;
pub mod ui;

use fframes::{AudioMap, Color, Duration, FFramesContext, Frame, Scene, Scenes, Svgr, Video, include_media_dir};

use narration::{SCENE_START, TOTAL, captions};
use ui::*;

include_media_dir!(pub struct KodierMedia, "examples/Kodierpruefung/kodierpruefung-video/media");

pub const FPS: usize = 30;
pub const WIDTH: usize = 1920;
pub const HEIGHT: usize = 1080;
const N: usize = 12;

pub const NAMES: [&str; N] = [
    "Einleitung",
    "Der Arbeitsablauf",
    "Der Modellfall",
    "Die Beatmungsstunden",
    "Die Folge für die Vergütung",
    "Vom Befund zur Rechnung",
    "Betrieb und Datenschutz",
    "Die Kosten der MD-Prüfungen",
    "Der Nutzen für das Krankenhaus",
    "Das Pilotprojekt",
    "Warum AMDG Engineers",
    "Abschluss",
];

const SCENE_IDS: [&str; N] = [
    "K1_Titel",
    "K2_Arbeitsablauf",
    "K3_Modellfall",
    "K4_Beatmung",
    "K5_Verguetung",
    "K6_Rechnung",
    "K7_Betrieb",
    "K7b_MDKosten",
    "K8_Nutzen",
    "K9_Pilot",
    "K10_AMDG",
    "K11_Abschluss",
];

pub fn gsec(frame: &Frame) -> f32 {
    frame.global_index as f32 / FPS as f32
}

pub fn scene_end(i: usize) -> f32 {
    if i + 1 < N { SCENE_START[i + 1] } else { TOTAL }
}

type Draw = for<'a> fn(&mut Frame, &FFramesContext<'a, '_>, f32) -> Svgr<'a>;

pub struct Part {
    pub index: usize,
    pub draw: Draw,
}

impl std::fmt::Debug for Part {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(SCENE_IDS[self.index])
    }
}

impl Scene for Part {
    fn duration(&self) -> Duration<'_> {
        let len = scene_end(self.index) - SCENE_START[self.index];
        Duration::Frames((len * FPS as f32).round() as usize)
    }

    fn name(&self) -> &'static str {
        SCENE_IDS[self.index]
    }

    fn render_frame<'a>(&'a self, mut frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = frame.index as f32 / FPS as f32;
        let len = scene_end(self.index) - SCENE_START[self.index];
        let fade_in = if self.index == 0 { 1.0 } else { ease_out(prog(t, 0.0, 0.45)) };
        let fade_out = if self.index == N - 1 { 1.0 } else { 1.0 - ease_in(prog(t, len - 0.4, len)) };
        let body = (self.draw)(&mut frame, ctx, t);
        fframes::svgr!(<g opacity={fade_in * fade_out}>{body}</g>)
    }
}

pub struct KodierVideo {
    parts: Vec<Part>,
}

impl Default for KodierVideo {
    fn default() -> Self {
        let draws: [Draw; N] = [
            scenes::k1::draw,
            scenes::k2::draw,
            scenes::k3::draw,
            scenes::k4::draw,
            scenes::k5::draw,
            scenes::k6::draw,
            scenes::k7::draw,
            scenes::k_md::draw,
            scenes::k8::draw,
            scenes::k9::draw,
            scenes::k_amdg::draw,
            scenes::k10::draw,
        ];
        Self { parts: draws.into_iter().enumerate().map(|(index, draw)| Part { index, draw }).collect() }
    }
}

impl std::fmt::Debug for KodierVideo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("KodierVideo")
    }
}

impl Video for KodierVideo {
    const FPS: usize = FPS;
    const WIDTH: usize = WIDTH;
    const HEIGHT: usize = HEIGHT;
    const BACKGROUND_COLOR: Color = Color::BLACK;

    fn duration(&self) -> Duration<'_> {
        Duration::Auto
    }

    /// No sound, like the clinic video: the captions carry every sentence.
    fn audio(&self) -> AudioMap<'_> {
        AudioMap::none()
    }

    fn define_scenes(&self) -> Scenes<'_> {
        Scenes::from(self.parts.iter().map(|p| p as &dyn Scene).collect::<Vec<_>>())
    }

    fn render_frame<'a>(&'a self, mut frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = gsec(&frame);
        let caps = captions(&mut frame, ctx, t);
        fframes::svgr!(
            <svg xmlns="http://www.w3.org/2000/svg" width={WIDTH} height={HEIGHT} viewBox="0 0 1920 1080">
                <defs>
                    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
                        <stop offset="0" stop-color="#0B1233" />
                        <stop offset="0.55" stop-color="#0A0F28" />
                        <stop offset="1" stop-color="#120B2E" />
                    </linearGradient>
                    <radialGradient id="glowBlue"><stop offset="0" stop-color="#2F6BFF" stop-opacity="0.42" /><stop offset="1" stop-color="#2F6BFF" stop-opacity="0" /></radialGradient>
                    <radialGradient id="glowViolet"><stop offset="0" stop-color="#8B4DFF" stop-opacity="0.42" /><stop offset="1" stop-color="#8B4DFF" stop-opacity="0" /></radialGradient>
                    <radialGradient id="glowAmber"><stop offset="0" stop-color="#FF9F1C" stop-opacity="0.3" /><stop offset="1" stop-color="#FF9F1C" stop-opacity="0" /></radialGradient>
                    <radialGradient id="glowGreen"><stop offset="0" stop-color="#10D49A" stop-opacity="0.3" /><stop offset="1" stop-color="#10D49A" stop-opacity="0" /></radialGradient>
                    <radialGradient id="llmFill" cx="0.36" cy="0.3" r="0.8">
                        <stop offset="0" stop-color="#D2B8FF" />
                        <stop offset="0.5" stop-color="#8B4DFF" />
                        <stop offset="1" stop-color="#3B1590" />
                    </radialGradient>
                    <linearGradient id="amberBar" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#FFD166" /><stop offset="1" stop-color="#F59E0B" /></linearGradient>
                </defs>
                <rect width="1920" height="1080" fill="url(#bg)" />
                {backdrop(t)}
                {ctx.render_scenes(&frame)}
                {header(t)}
                {caps}
            </svg>
        )
    }
}

/// Strength of the four glows (blue record, violet assistant, amber
/// coding/billing, green confirmed) per scene.
const GLOWS: [[f32; 4]; N] = [
    [0.8, 0.6, 0.8, 0.2],
    [1.0, 0.6, 0.5, 0.4],
    [0.9, 0.3, 0.8, 0.5],
    [0.8, 0.2, 1.0, 0.3],
    [0.4, 0.2, 1.0, 0.3],
    [0.6, 0.3, 0.6, 0.9],
    [1.0, 0.4, 0.2, 0.6],
    [0.9, 0.3, 0.9, 0.3],
    [0.5, 0.3, 0.8, 1.0],
    [0.7, 0.5, 0.6, 0.7],
    [0.5, 0.8, 1.0, 0.8],
    [0.8, 0.9, 0.7, 0.4],
];

fn backdrop(t: f32) -> Svgr<'static> {
    let i = SCENE_START.iter().rposition(|s| t >= *s).unwrap_or(0);
    let end = scene_end(i);
    let next = (i + 1).min(N - 1);
    let k = ease_in_out(prog(t, end - 1.5, end));
    let w: Vec<f32> = (0..4).map(|c| lerp(GLOWS[i][c], GLOWS[next][c], k)).collect();
    let d = |a: f32, b: f32| (t * a + b).sin();
    let blobs = [
        ("url(#glowBlue)", 240.0 + 90.0 * d(0.05, 0.0), 180.0 + 70.0 * d(0.04, 1.0), 860.0, w[0]),
        ("url(#glowViolet)", 1720.0 + 80.0 * d(0.045, 2.0), 240.0 + 90.0 * d(0.05, 3.0), 820.0, w[1]),
        ("url(#glowAmber)", 1500.0 + 110.0 * d(0.035, 4.0), 1000.0 + 60.0 * d(0.05, 5.0), 820.0, w[2]),
        ("url(#glowGreen)", 340.0 + 100.0 * d(0.04, 6.0), 1020.0 + 50.0 * d(0.045, 7.0), 780.0, w[3]),
    ];
    let circles: Vec<Svgr<'static>> = blobs
        .iter()
        .filter(|b| b.4 > 0.01)
        .map(|(fill, x, y, r, o)| fframes::svgr!(<circle cx={*x} cy={*y} r={*r} fill={*fill} opacity={*o} />))
        .collect();
    fframes::svgr!(<g>{circles}</g>)
}

/// Scene number and name, and the presenter.
fn header(t: f32) -> Svgr<'static> {
    let i = SCENE_START.iter().rposition(|s| t >= *s).unwrap_or(0);
    let local = t - SCENE_START[i];
    let end = scene_end(i) - SCENE_START[i];
    let shown = i > 0 && i < N - 1;
    let o_head = if shown { window(local, 0.2, end - 0.1, 0.4) } else { 0.0 };
    // the closing page carries its own credit; the header's fades out before it
    let credit_o = if i == 0 {
        enter(local, 16.0, 1.0)
    } else if i == N - 1 {
        0.0
    } else if i == N - 2 {
        1.0 - prog(local, end - 0.4, end)
    } else {
        1.0
    };
    let head = format!("{}", i);
    let name = NAMES[i];
    // the four scenes of the model case carry a badge: it is a virtual case
    let fictive_o = if (2..=5).contains(&i) { o_head } else { 0.0 };
    let fictive = "VIRTUELLER MODELLFALL · ALLE DATEN FIKTIV";
    let fictive_x = 1824.0 - badge_w(fictive, 15.0);
    fframes::svgr!(
        <g>
            <g opacity={fictive_o}>{badge(fictive_x, 154.0, fictive, AMBER, AMBER_BG, 15.0)}</g>
            <g opacity={o_head}>
                <rect x="96" y="98" width="40" height="36" rx="18" fill="url(#amberBar)" />
                <text x="116" y="123" font-family={MONO} font-size="20" font-weight="400" fill="#1A1203" text-anchor="middle">{head}</text>
                <text x="152" y="125" font-family={SANS} font-size="25" font-weight="500" fill={INK}>{name}</text>
            </g>
            <g opacity={credit_o}>
                <text x="1824" y="124" font-family={SANS} font-size="22" font-weight="400" fill={MUTED} text-anchor="end">"Präsentiert von "<tspan font-weight="500" fill={INK}>"AMDG Engineers"</tspan></text>
            </g>
        </g>
    )
}
