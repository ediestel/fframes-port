//! Szene 6 · Vom Befund zur Rechnung: die nächsten Arbeitsschritte vor der
//! Rechnungsstellung, abgehakt; daneben dasselbe im Prüfverfahren.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::narration::at;
use crate::ui::*;

const STEPS: [&str; 6] = [
    "Beatmungsstunden auf 87 korrigieren",
    "A41.9, R65.1, E43 und 8-800.c0 streichen",
    "N18.3 durch N18.32 ersetzen; Entlassgrund 09",
    "Aufwandspunkte für 8-980.10 anfordern",
    "Entlassbrief an Verlauf und Befunde angleichen",
    "Neu gruppieren und die Rechnung freigeben",
];

pub fn draw<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let mut items = Vec::new();
    for (i, s) in STEPS.iter().enumerate() {
        let at_i = if i < 3 { at("c6_1") + 0.8 + i as f32 * 1.6 } else { at("c6_2") + 0.8 + (i - 3) as f32 * 1.8 };
        let y = 300.0 + i as f32 * 74.0;
        items.push(rise_in(
            t,
            at_i,
            fframes::svgr!(<g>
                {check(176.0, y - 8.0, 16.0, prog(t, at_i + 0.3, at_i + 1.0), GREEN)}
                {text(210.0, y, 24.0, 400, INK, "start", s)}
            </g>),
        ));
    }
    let before = rise_in(
        t,
        0.4,
        fframes::svgr!(<g>
            {card(130.0, 180.0, 1000.0, 600.0, Some(GREEN), PANEL, LINE)}
            {text(166.0, 228.0, 24.0, 500, GREEN, "start", "Vor der Rechnungsstellung")}
        </g>),
    );
    let a3 = at("c6_3");
    let md = rise_in(
        t,
        a3,
        fframes::svgr!(<g>
            {card(1170.0, 180.0, 620.0, 600.0, Some(BLUE), PANEL, LINE)}
            {text(1206.0, 228.0, 24.0, 500, BLUE, "start", "Im Prüfverfahren")}
            {text(1206.0, 272.0, 19.0, 400, MUTED, "start", "dieselben Feststellungen tragen")}
            {text(1206.0, 298.0, 19.0, 400, MUTED, "start", "die Stellungnahme")}
            <g opacity={enter(t, a3 + 2.5, 0.5)}>
                {check(1222.0, 372.0, 15.0, prog(t, a3 + 2.7, a3 + 3.3), GREEN)}
                {text(1250.0, 380.0, 21.0, 500, INK, "start", "anerkennen, was die")}
                {text(1250.0, 408.0, 21.0, 500, INK, "start", "Dokumentation nicht stützt")}
            </g>
            <g opacity={enter(t, a3 + 5.5, 0.5)}>
                {icon_doc(1210.0, 470.0, 34.0, BLUE)}
                {text(1250.0, 492.0, 21.0, 500, INK, "start", "mit Unterlage und Seite")}
                {text(1250.0, 520.0, 21.0, 500, INK, "start", "belegen, was strittig ist")}
            </g>
        </g>),
    );
    fframes::svgr!(<g>{before}{items}{md}</g>)
}
