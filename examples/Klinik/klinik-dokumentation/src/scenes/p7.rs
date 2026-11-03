//! Szene 7 · 03:25–03:50 · Protokoll und Herkunft: das Protokoll des Falls,
//! dann der Entlassbrief zurückverfolgt auf Quelldaten, Prompt, Prüfung und
//! Freigabe.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::ui::*;

const LOG: [(&str, &str, &str); 10] = [
    ("01", "Eingang", "FALL-2026-0417 · 5 Quelldokumente · sha256 7c2e…41f9"),
    ("02", "Tokens vergeben", "PAT-3K9X · CASE-7T2M · SRC-5D1Q · Schlüssel im Haus"),
    ("03", "API-Aufrufe", "7 Aufgaben-Prompts · nur pseudonymisierter Inhalt"),
    ("04", "Prompt geändert", "entlassbrief.md Rev. 2 · durch Stationsärztin"),
    ("05", "Entlassbrief erstellt", "entlassbrief.md Rev. 2 · Antwort gespeichert"),
    ("06", "Prüfung", "Entlassbrief Fassung 1 zurückgewiesen · Dosis abweichend"),
    ("07", "Prüfung", "Entlassbrief Fassung 2 · 6 Prüfungen bestanden"),
    ("08", "Freigabe", "Stationsärztin"),
    ("09", "Name eingesetzt", "im Haus · Schlüssel nicht übertragen"),
    ("10", "Import", "Patientenakte FALL-2026-0417"),
];

/// Rückverfolgung vom abgelegten Brief: (Zeile, Zeitpunkt)
const TRACE: [(usize, f32); 5] = [(9, 11.0), (0, 12.8), (4, 14.6), (6, 16.4), (7, 18.4)];

pub fn draw<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let mut rows = Vec::new();
    for (i, (n, step, detail)) in LOG.iter().enumerate() {
        let y = 300.0 + i as f32 * 52.0;
        let lit = TRACE.iter().find(|(r, _)| *r == i).map_or(0.0, |(_, at)| prog(t, *at, at + 0.5));
        rows.push(rise_in(
            t,
            0.9 + i as f32 * 0.45,
            fframes::svgr!(<g>
                <rect x="190" y={y - 32.0} width="1600" height="44" rx="8" fill={GREEN_BG} opacity={lit * 0.9} />
                {mono(214.0, y, 19.0, MUTED, "start", n)}
                {text(280.0, y, 22.0, 500, INK, "start", step)}
                {mono(620.0, y, 19.0, if lit > 0.0 { GREEN } else { MUTED }, "start", detail)}
            </g>),
        ));
    }
    let trace_k = prog(t, 11.0, 18.8);
    let trace = if trace_k > 0.0 {
        let top = 300.0 - 10.0;
        let bottom = 300.0 + 9.0 * 52.0 - 10.0;
        let y = lerp(bottom, top, ease_in_out(trace_k));
        fframes::svgr!(<g>
            <rect x="160" y={y} width="4" height={(bottom - y).max(0.5)} rx="2" fill={GREEN} />
            <circle cx="162" cy={y} r="8" fill={GREEN} />
        </g>)
    } else {
        Svgr::empty()
    };
    let log = rise_in(
        t,
        0.5,
        fframes::svgr!(<g>
            {card(120.0, 190.0, 1704.0, 660.0, Some(GREEN), PANEL, LINE)}
            {mono(150.0, 236.0, 20.0, GREEN, "start", "ausgang/FALL-0417_Protokoll.log")}
            {text(1794.0, 236.0, 18.0, 400, MUTED, "end", "Eingangsdaten · Prompt-Version · Antwort · Prüfung · Freigabe")}
        </g>),
    );
    fframes::svgr!(<g>{log}{rows}{trace}</g>)
}
