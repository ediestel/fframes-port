//! Szene 1 · Titel: "Belegte Kodierung. Geprüfte Rechnung." Die
//! Prüfassistenz für das Medizincontrolling; Akte, Kodierung und Rechnung
//! auf einer Linie, die sich von links nach rechts zieht.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::ui::*;

/// (x, title, sub, appears)
const STAGES: [(f32, &str, &str, f32); 3] = [
    (260.0, "AKTE", "Dokumentierte Fakten und Fundstellen", 3.4),
    (860.0, "KODIERUNG", "Diagnosen, Prozeduren mit Belegstelle", 5.0),
    (1460.0, "RECHNUNG", "Geprüfter Datensatz zur Freigabe", 6.6),
];

pub fn draw<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let l1 = enter(t, 0.3, 0.8);
    let l2 = enter(t, 0.9, 0.8);
    let sub = enter(t, 1.7, 0.8);
    let line_k = ease_in_out(prog(t, 3.0, 7.0));
    let line_x = lerp(260.0, 1660.0, line_k);
    let mut stages = Vec::new();
    for (x, title, s, at) in STAGES {
        stages.push(rise_in(
            t,
            at,
            fframes::svgr!(<g>
                <circle cx={x} cy="660" r="12" fill={AMBER} />
                {mono(x, 716.0, 22.0, AMBER, "start", title)}
                {text(x, 752.0, 22.0, 400, INK, "start", s)}
            </g>),
        ));
    }
    fframes::svgr!(<g>
        <g opacity={l1} transform={tr(0.0, (1.0 - l1) * 30.0)}>
            <text x="260" y="360" font-family={SANS} font-size="104" font-weight="500" fill={INK} letter-spacing="-2">"Belegte Kodierung."</text>
        </g>
        <g opacity={l2} transform={tr(0.0, (1.0 - l2) * 30.0)}>
            <text x="260" y="480" font-family={SANS} font-size="104" font-weight="500" fill={AMBER} letter-spacing="-2">"Geprüfte Rechnung."</text>
        </g>
        <g opacity={sub}>{text(264.0, 560.0, 38.0, 400, MUTED, "start", "Die Prüfassistenz für das Medizincontrolling")}</g>
        <rect x="260" y="658" width={(line_x - 260.0).max(0.5)} height="4" rx="2" fill={INK} opacity={prog(t, 3.0, 3.2) * 0.8} />
        {stages}
    </g>)
}
