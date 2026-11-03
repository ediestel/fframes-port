//! Szene 5 · 02:20–02:55 · Prüfung in der Prompt-Reihe: ein Prüf-Prompt,
//! jede Prüfung mit Ergebnis und Begründung, ein Brief mit abweichender Dosis
//! zurückgewiesen, ärztliche Freigabe.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::ui::*;

const CHECKS: [(&str, &str); 6] = [
    ("Aussagen belegt", "jede Aussage einem Quelldokument zugeordnet"),
    ("Medikation und Dosis", "entspricht der Medikationsliste"),
    ("Laborwerte", "entsprechen FALL-0417_Labor.csv"),
    ("Diagnosen und Kodierung", "J18.9 · I10.90 · E11.90 belegt"),
    ("Keine identifizierenden Daten", "im ausgehenden Inhalt nur Tokens"),
    ("Pflichtabschnitte", "vollständig"),
];

pub fn draw<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let prompt = rise_in(
        t,
        0.6,
        fframes::svgr!(<g>
            {card(120.0, 190.0, 600.0, 230.0, Some(AMBER), PANEL, LINE)}
            {mono(150.0, 232.0, 20.0, AMBER, "start", "prompts/pruefung.md")}
            {text(150.0, 278.0, 21.0, 400, INK, "start", "Prüfe das Dokument gegen die Quelldaten.")}
            {text(150.0, 308.0, 21.0, 400, INK, "start", "Gib für jede Prüfung Ergebnis und")}
            {text(150.0, 338.0, 21.0, 400, INK, "start", "Begründung zurück. Weise Dokumente mit")}
            {text(150.0, 368.0, 21.0, 400, INK, "start", "abweichenden Angaben zurück.")}
        </g>),
    );

    let mut rows = Vec::new();
    for (i, (c, r)) in CHECKS.iter().enumerate() {
        let y = 268.0 + i as f32 * 58.0;
        let at = 2.0 + i as f32 * 1.0;
        rows.push(rise_in(
            t,
            at - 0.3,
            fframes::svgr!(<g>
                {check(800.0, y - 7.0, 16.0, prog(t, at, at + 0.5), GREEN)}
                {text(830.0, y, 21.0, 500, INK, "start", c)}
                <g opacity={prog(t, at, at + 0.5)}>{text(1250.0, y, 19.0, 400, GREEN, "start", r)}</g>
                <rect x="790" y={y + 22.0} width="1010" height="1" fill={LINE} />
            </g>),
        ));
    }
    let panel = rise_in(
        t,
        1.2,
        fframes::svgr!(<g>
            {card(760.0, 190.0, 1064.0, 400.0, Some(GREEN), PANEL, LINE)}
            {text(790.0, 226.0, 21.0, 500, GREEN, "start", "Prüfungen · vom LLM als Daten zurückgegeben")}
            {rows}
        </g>),
    );

    let fail = rise_in(
        t,
        12.0,
        fframes::svgr!(<g>
            {card(120.0, 620.0, 1060.0, 220.0, Some(RED), PANEL, RED)}
            {text(150.0, 662.0, 20.0, 500, RED, "start", "Beispiel: zurückgewiesener Entlassbrief")}
            {mono(150.0, 706.0, 20.0, INK, "start", "Entlassmedikation: Metoprolol 95 mg 1-0-0")}
            {cross(170.0, 754.0, 16.0, prog(t, 13.4, 13.9), RED)}
            {text(200.0, 762.0, 20.0, 500, INK, "start", "Medikation und Dosis: abweichend — Medikationsliste: Metoprolol 47,5 mg")}
            {arrow(150.0, 806.0, 420.0, 806.0, prog(t, 15.0, 15.8), RED, 3.0)}
            <g opacity={prog(t, 15.6, 16.2)}>{text(440.0, 813.0, 20.0, 500, RED, "start", "mit Begründung zur Korrektur zurückgegeben")}</g>
        </g>),
    );

    let press = prog(t, 23.2, 23.8);
    let approve = rise_in(
        t,
        21.4,
        fframes::svgr!(<g>
            {card(1220.0, 620.0, 604.0, 220.0, Some(GREEN), PANEL, LINE)}
            {text(1250.0, 662.0, 20.0, 500, MUTED, "start", "Stationsärztin · im Haus")}
            {mono(1250.0, 702.0, 20.0, VIOLET, "start", "PAT-3K9X")}
            {text(1400.0, 702.0, 20.0, 500, INK, "start", "→  Erika Beispiel")}
            {button(1250.0, 736.0, 420.0, 58.0, "Geprüftes Dokument freigeben", GREEN, press)}
            <g opacity={prog(t, 24.0, 24.6)}>{check(1720.0, 765.0, 22.0, prog(t, 24.0, 24.6), GREEN)}</g>
        </g>),
    );

    fframes::svgr!(<g>{prompt}{panel}{fail}{approve}</g>)
}
