//! "Wie kann ein LLM die klinische Dokumentation unterstützen?": Video für
//! Kliniken und Praxen (1920 × 1080, 30 fps, 7:26 min, 13.380 Frames), deutsch.
//! Eine Szene je Kernpunkt; ein fiktiver Behandlungsfall im fiktiven Klinikum
//! Musterstadt als Beispiel. Alle Patientendaten, Tokens und Referenzen sind
//! erfunden. Die Szenen 9–12 beantworten, was eine Klinik oder Praxis braucht
//! und davon hat: Datenübertragung und Sicherheit, Anbindung im Haus,
//! Einrichtung und Betrieb, Zeitgewinn und Kosten.

pub mod narration;
pub mod scenes;
pub mod ui;

use fframes::{
    AudioMap, Color, Duration, FFramesContext, Frame, Scene, Scenes, Svgr, Video,
    include_media_dir,
};

use narration::{SCHEDULE, captions};
use ui::*;

include_media_dir!(pub struct KlinikMedia, "examples/Klinik/klinik-dokumentation/media");

pub const FPS: usize = 30;
pub const WIDTH: usize = 1920;
pub const HEIGHT: usize = 1080;
const N: usize = 15;

pub const NAMES: [&str; N] = [
    "Einleitung",
    "Schnittstelle: Eingangs- und Ausgangsordner",
    "Pseudonymisierung · Daten bleiben im Haus",
    "Prompts erstellen die Dokumente",
    "Jede Dokumentart",
    "Prüfung in der Prompt-Reihe",
    "Ausgabe: Entlassbrief und Kodierprüfung",
    "Protokoll und Herkunft",
    "Datenübertragung und Sicherheit",
    "Anbindung im Haus: frei wählbar",
    "Einrichtung und Betrieb",
    "Zeitgewinn und Nutzen",
    "Warum jetzt",
    "Warum AMDG Engineers",
    "Abschluss",
];

const SCENE_IDS: [&str; N] = [
    "S1_Einleitung",
    "S2_Schnittstelle",
    "S3_Pseudonymisierung",
    "S4_Prompts",
    "S4b_Dokumentarten",
    "S5_Pruefung",
    "S6_Ausgabe",
    "S7_Protokoll",
    "S7a_Server",
    "S7b_Zugang",
    "S7c_Betrieb",
    "S7d_Zeitgewinn",
    "S7e_WarumJetzt",
    "S7f_AMDG",
    "S8_Abschluss",
];

/// Der gerade protokollierte Schritt, unter der Kopfzeile.
const AUDIT: [&str; N] = [
    "",
    "Eingang · Fall FALL-2026-0417 · 5 Quelldokumente",
    "Tokens vergeben · PAT-3K9X · CASE-7T2M · SRC-5D1Q · Schlüssel lokal",
    "7 Aufgaben-Prompts · 7 API-Aufrufe · Prompt Entlassbrief Rev. 2",
    "12 Vorlagen · dieselben Eingangsdaten · je ein API-Aufruf",
    "Prüfungen mit Begründung · 1 Dokument zurückgewiesen · ärztlich freigegeben",
    "Entlassbrief und Kodierprüfung geschrieben · in die Akte importiert",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
];

pub fn gsec(frame: &Frame) -> f32 {
    frame.global_index as f32 / FPS as f32
}

pub fn scene_end(i: usize) -> f32 {
    SCHEDULE.scene_end(i)
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
        let len = scene_end(self.index) - SCHEDULE.scene_start(self.index);
        Duration::Frames((len * FPS as f32).round() as usize)
    }

    fn name(&self) -> &'static str {
        SCENE_IDS[self.index]
    }

    fn render_frame<'a>(&'a self, mut frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = frame.index as f32 / FPS as f32;
        let len = scene_end(self.index) - SCHEDULE.scene_start(self.index);
        let fade_in = if self.index == 0 { 1.0 } else { ease_out(prog(t, 0.0, 0.45)) };
        let fade_out = if self.index == N - 1 { 1.0 } else { 1.0 - ease_in(prog(t, len - 0.4, len)) };
        let body = (self.draw)(&mut frame, ctx, t);
        fframes::svgr!(<g opacity={fade_in * fade_out}>{body}</g>)
    }
}

pub struct KlinikVideo {
    parts: Vec<Part>,
}

impl Default for KlinikVideo {
    fn default() -> Self {
        let draws: [Draw; N] = [
            scenes::p1::draw,
            scenes::p2::draw,
            scenes::p3::draw,
            scenes::p4::draw,
            scenes::px::draw,
            scenes::p5::draw,
            scenes::p6::draw,
            scenes::p7::draw,
            scenes::pa::draw,
            scenes::pb::draw,
            scenes::pc::draw,
            scenes::pd::draw,
            scenes::pe::draw,
            scenes::pf::draw,
            scenes::p8::draw,
        ];
        Self { parts: draws.into_iter().enumerate().map(|(index, draw)| Part { index, draw }).collect() }
    }
}

impl std::fmt::Debug for KlinikVideo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("KlinikVideo")
    }
}

impl Video for KlinikVideo {
    const FPS: usize = FPS;
    const WIDTH: usize = WIDTH;
    const HEIGHT: usize = HEIGHT;
    const BACKGROUND_COLOR: Color = Color::BLACK;

    fn duration(&self) -> Duration<'_> {
        Duration::Auto
    }

    /// No sound: the voice-over was removed at the user's request; the
    /// captions carry every sentence and keep the timing of the spoken clips.
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

/// Stärke der vier Leuchtflächen (blau Daten, violett LLM, amber Prompts,
/// grün geprüft) je Szene.
const GLOWS: [[f32; 4]; N] = [
    [0.8, 0.9, 0.6, 0.0],
    [1.0, 0.2, 0.3, 0.5],
    [1.0, 1.0, 0.1, 0.0],
    [0.3, 1.0, 1.0, 0.0],
    [0.6, 0.9, 0.9, 0.3],
    [0.3, 0.4, 0.4, 1.0],
    [0.7, 0.3, 0.5, 0.9],
    [0.6, 0.3, 0.6, 0.9],
    [1.0, 0.6, 0.1, 0.6],
    [0.9, 0.3, 0.4, 0.5],
    [0.5, 0.2, 0.8, 0.8],
    [0.6, 0.4, 0.9, 1.0],
    [0.8, 0.7, 0.9, 0.3],
    [0.5, 0.8, 1.0, 0.8],
    [0.8, 0.9, 0.7, 0.4],
];

fn backdrop(t: f32) -> Svgr<'static> {
    let i = SCHEDULE.scene_at(t);
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

/// Szenennummer und -name, die laufende Protokollzeile und der dezente Hinweis.
fn header(t: f32) -> Svgr<'static> {
    let i = SCHEDULE.scene_at(t);
    let local = t - SCHEDULE.scene_start(i);
    let end = scene_end(i) - SCHEDULE.scene_start(i);
    let shown = i > 0 && i < N - 1;
    let o_head = if shown { window(local, 0.2, end - 0.1, 0.4) } else { 0.0 };
    let credit_o = if i == 0 { enter(local, 13.0, 1.0) } else if i == N - 1 { 0.0 } else { 1.0 };
    let head = format!("{}", i);
    let name = NAMES[i];
    let audit = AUDIT[i];
    let line = format!("PROTOKOLL · {audit}");
    let audit_o = if audit.is_empty() { 0.0 } else { window(local, 2.5, end - 0.2, 0.5) };
    fframes::svgr!(
        <g>
            <g opacity={o_head}>
                <rect x="96" y="98" width="40" height="36" rx="18" fill="url(#amberBar)" />
                <text x="116" y="123" font-family={MONO} font-size="20" font-weight="400" fill="#1A1203" text-anchor="middle">{head}</text>
                <text x="152" y="125" font-family={SANS} font-size="25" font-weight="500" fill={INK}>{name}</text>
            </g>
            <g opacity={credit_o}>
                <text x="1824" y="124" font-family={SANS} font-size="22" font-weight="400" fill={MUTED} text-anchor="end">"Präsentiert von "<tspan font-weight="500" fill={INK}>"AMDG Engineers"</tspan></text>
            </g>
            <g opacity={audit_o}>
                <circle cx={1824.0 - mono_w(&line, 16.0) - 14.0} cy="154" r="5" fill={GREEN} />
                <text x="1824" y="160" font-family={MONO} font-size="16" font-weight="400" fill={GREEN} text-anchor="end">{line.clone()}</text>
            </g>
        </g>
    )
}
