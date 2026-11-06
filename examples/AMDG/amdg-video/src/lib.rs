//! "KI und Sprachmodelle in der professionellen Arbeit": Video nach der
//! Präsentation `examples/AMDG/AMDG_Engineers.pptx` (November 2026), auf
//! Deutsch, in der Reihe der AMDG-Videos (Klinik, Kodierprüfung, MD): dunkle
//! Seiten, ohne Untertitel, ohne Ton, "Präsentiert von AMDG Engineers", nach
//! "Warum AMDG Engineers" die Quellen als vorletzte Seite. Die Folien 9, 10 und 12 der Präsentation
//! sind nicht enthalten.

pub mod narration;
pub mod qr;
pub mod scenes;
pub mod ui;

use fframes::{AudioMap, Color, Duration, FFramesContext, Frame, Scene, Scenes, Svgr, Video, include_media_dir};

use narration::{SCENE_START, TOTAL};
use ui::*;

include_media_dir!(pub struct AmdgMedia, "examples/AMDG/amdg-video/media");

pub const FPS: usize = 30;
pub const WIDTH: usize = 1920;
pub const HEIGHT: usize = 1080;
pub const N: usize = 15;

pub const NAMES: [&str; N] = [
    "Einleitung",
    "Heutige Anwendungen",
    "Verbreitung und Nutzen",
    "Grenzen",
    "Zuverlässigkeit heute",
    "AMDG-Modulation",
    "Beispiel: eine Anfrage",
    "Beispiel eines Prüfprotokolls",
    "Was der Leser erhält",
    "High-Risk-Domänen und AMDG",
    "Wirtschaftlicher Nutzen der AMDG-Modulation",
    "Beispiele aus High-Risk-Domänen",
    "AMDG Engineers",
    "Quellen",
    "Abschluss",
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
        write!(f, "M{:02}", self.index + 1)
    }
}

impl Scene for Part {
    fn duration(&self) -> Duration<'_> {
        let len = scene_end(self.index) - SCENE_START[self.index];
        Duration::Frames((len * FPS as f32).round() as usize)
    }

    fn name(&self) -> &'static str {
        scenes::slides::IDS[self.index]
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

pub struct AmdgVideo {
    parts: Vec<Part>,
}

impl Default for AmdgVideo {
    fn default() -> Self {
        Self { parts: scenes::slides::DRAWS.iter().enumerate().map(|(index, draw)| Part { index, draw: *draw }).collect() }
    }
}

impl std::fmt::Debug for AmdgVideo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AmdgVideo")
    }
}

impl Video for AmdgVideo {
    const FPS: usize = FPS;
    const WIDTH: usize = WIDTH;
    const HEIGHT: usize = HEIGHT;
    const BACKGROUND_COLOR: Color = Color::BLACK;

    fn duration(&self) -> Duration<'_> {
        Duration::Auto
    }

    /// No sound.
    fn audio(&self) -> AudioMap<'_> {
        AudioMap::none()
    }

    fn define_scenes(&self) -> Scenes<'_> {
        Scenes::from(self.parts.iter().map(|p| p as &dyn Scene).collect::<Vec<_>>())
    }

    fn render_frame<'a>(&'a self, frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = gsec(&frame);
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
                    <linearGradient id="amberBar" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#FFD166" /><stop offset="1" stop-color="#F59E0B" /></linearGradient>
                </defs>
                <rect width="1920" height="1080" fill="url(#bg)" />
                {backdrop(t)}
                {ctx.render_scenes(&frame)}
                {header(t)}
            </svg>
        )
    }
}

/// Strength of the four glows per page: a gentle cycle through the colours.
fn glow(i: usize) -> [f32; 4] {
    const SET: [[f32; 4]; 4] = [[0.9, 0.5, 0.6, 0.4], [0.6, 0.4, 0.9, 0.5], [0.8, 0.6, 0.4, 0.8], [0.5, 0.8, 0.7, 0.6]];
    SET[i % 4]
}

fn backdrop(t: f32) -> Svgr<'static> {
    let i = SCENE_START.iter().rposition(|s| t >= *s).unwrap_or(0);
    let end = scene_end(i);
    let next = (i + 1).min(N - 1);
    let k = ease_in_out(prog(t, end - 1.5, end));
    let (a, b) = (glow(i), glow(next));
    let w: Vec<f32> = (0..4).map(|c| lerp(a[c], b[c], k)).collect();
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

/// Page number and name, and the presenter.
fn header(t: f32) -> Svgr<'static> {
    let i = SCENE_START.iter().rposition(|s| t >= *s).unwrap_or(0);
    let local = t - SCENE_START[i];
    let end = scene_end(i) - SCENE_START[i];
    let shown = i > 0 && i < N - 1;
    let o_head = if shown { window(local, 0.2, end - 0.1, 0.4) } else { 0.0 };
    // the closing page carries its own credit; the header's fades out before it
    let credit_o = if i == 0 {
        enter(local, 11.0, 1.0)
    } else if i == N - 1 {
        0.0
    } else if i == N - 2 {
        1.0 - prog(local, end - 0.4, end)
    } else {
        1.0
    };
    let head = format!("{}", i);
    let name = NAMES[i];
    fframes::svgr!(
        <g>
            <g opacity={o_head}>
                <rect x="96" y="98" width="44" height="36" rx="18" fill="url(#amberBar)" />
                <text x="118" y="123" font-family={MONO} font-size="20" font-weight="400" fill="#1A1203" text-anchor="middle">{head}</text>
                <text x="156" y="125" font-family={SANS} font-size="25" font-weight="500" fill={INK}>{name}</text>
            </g>
            <g opacity={credit_o}>
                <text x="1824" y="124" font-family={SANS} font-size="22" font-weight="400" fill={MUTED} text-anchor="end">"Präsentiert von "<tspan font-weight="500" fill={INK}>"AMDG Engineers"</tspan></text>
            </g>
        </g>
    )
}
