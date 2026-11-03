//! Szene 3 · Der Modellfall: die Prüffrage und die acht geprüften Angaben
//! des Exposés mit Ergebnis, Beleglage und Fundstelle. Die Ergebnisse
//! leuchten auf, wenn die Untertitel sie nennen.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::narration::at;
use crate::ui::*;

/// Result kinds: 0 belegt, 1 nicht belegt, 2 korrigiert, 3 nicht prüfbar.
struct Row {
    item: &'static str,
    result: &'static str,
    kind: u8,
    evidence: &'static str,
    source: &'static str,
    /// Caption at which the result lights.
    cue: &'static str,
}

const ROWS: [Row; 8] = [
    Row { item: "Beatmungsstunden 97", result: "87 STUNDEN", kind: 2, evidence: "Intubation 06.11. 04:24 · Extubation 09.11. 18:30", source: "Unterlage 5, S. 20 · 6, S. 13 · 7, S. 10", cue: "c3_1" },
    Row { item: "Hauptdiagnose S72.01 L", result: "BELEGT", kind: 0, evidence: "Sturzbedingte Schenkelhalsfraktur Garden IV als Anlass der Aufnahme", source: "Unterlage 8, S. 6–8 · 5, S. 18", cue: "c3_2" },
    Row { item: "Nebendiagnosen A41.9, R65.1", result: "NICHT BELEGT", kind: 1, evidence: "Kreislaufstabil, Laktat 1,4 mmol/l, Blutkulturen steril", source: "Unterlage 7, S. 10 · 4, S. 21–22", cue: "c3_3" },
    Row { item: "Nebendiagnose E43", result: "NICHT BELEGT", kind: 1, evidence: "BMI 23,4, NRS-2002 0: Ernährungsrisiko, keine Mangelernährung", source: "Unterlage 6, S. 12, 15", cue: "c3_3" },
    Row { item: "Nebendiagnose N18.3", result: "N18.32", kind: 2, evidence: "Dokumentiert ist das Stadium 3b", source: "Unterlage 9, S. 3–4", cue: "c3_4" },
    Row { item: "OPS 8-800.c0", result: "NICHT ERBRACHT", kind: 1, evidence: "Zwei Erythrozytenkonzentrate ungeöffnet zurückgegeben", source: "Unterlage 4, S. 22 · 6, S. 14", cue: "c3_3" },
    Row { item: "OPS 8-980.10", result: "NICHT PRÜFBAR", kind: 3, evidence: "Aufwandspunkte (SAPS II, TISS) fehlen in den Unterlagen", source: "", cue: "c3_4" },
    Row { item: "Entlassgrund 09 1", result: "09", kind: 2, evidence: "Entlassung in eine Rehabilitationseinrichtung", source: "Unterlage 9, S. 3", cue: "c3_1" },
];

/// How the case data were obtained: shown first, before the table.
const ORIGIN: &str = "Die folgenden Daten wurden mit der proprietären LLM-Modulation von AMDG Engineers extrahiert und analysiert, ohne weitere Unterstützung.";

pub fn draw<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let start = at("c3_1");
    let origin = during(
        t,
        at("c3_0"),
        start - 0.1,
        fframes::svgr!(<g>
            {card(230.0, 360.0, 1460.0, 250.0, Some(BLUE), PANEL, BLUE)}
            {badge(270.0, 410.0, "VIRTUELLER MODELLFALL · ALLE DATEN FIKTIV", AMBER, AMBER_BG, 15.0)}
            {para(f, ctx, 270, 490, 1380, 36, 500, INK, ORIGIN)}
        </g>),
    );
    let question = rise_in(
        t,
        start,
        fframes::svgr!(<g>
            {card(130.0, 172.0, 1660.0, 80.0, Some(AMBER), PANEL, LINE)}
            {mono(166.0, 220.0, 16.0, AMBER, "start", "PRÜFFRAGE")}
            {text(300.0, 222.0, 24.0, 500, INK, "start", "Ist die Kodierung durch die Dokumentation belegt: Beatmungsdauer, Diagnosen, Prozeduren?")}
        </g>),
    );
    let sources_lit = prog(t, at("c3_5"), at("c3_5") + 0.6);
    let src_fill = mix(FAINT, BLUE, sources_lit);
    let mut rows = Vec::new();
    for (i, r) in ROWS.iter().enumerate() {
        let y = 290.0 + i as f32 * 70.0;
        let lit = prog(t, at(r.cue) + 0.6, at(r.cue) + 1.2);
        let (fg, bg) = match r.kind {
            0 => (GREEN, GREEN_BG),
            1 => (RED, RED_BG),
            2 => (AMBER, AMBER_BG),
            _ => (MUTED, PANEL),
        };
        let zebra = if i % 2 == 1 { 0.6 } else { 0.0 };
        let tag = show(lit, 0.0, badge(560.0, y + 30.0, r.result, fg, bg, 16.0));
        rows.push(rise_in(
            t,
            start + 0.6 + i as f32 * 0.25,
            fframes::svgr!(<g>
                <rect x="130" y={y} width="1660" height="64" rx="8" fill={PANEL} opacity={zebra} />
                {text(150.0, y + 38.0, 22.0, 500, INK, "start", r.item)}
                {tag}
                {text(830.0, y + 28.0, 19.0, 400, INK, "start", r.evidence)}
                {mono(830.0, y + 52.0, 14.0, &src_fill, "start", r.source)}
            </g>),
        ));
    }
    fframes::svgr!(<g>{origin}{question}{rows}</g>)
}
