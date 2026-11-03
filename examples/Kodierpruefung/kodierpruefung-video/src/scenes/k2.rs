//! Szene 2 · Der Arbeitsablauf: wann die Assistenz hilft (vor der
//! Rechnungsstellung und im Prüfverfahren), die fünf Schritte, die
//! Fundstellen, die Entscheidung der Kodierfachkraft, das Prüfverfahren des
//! Medizinischen Dienstes.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::narration::at;
use crate::ui::*;

const STEPS: [(&str, &str); 5] = [
    ("Fall übernehmen", "Abrechnungsdatensatz, Kodierung und Unterlagen zusammenführen"),
    ("Akte erschließen", "Verlauf, Befunde, Beatmung, Verlegungen mit Datum und Uhrzeit"),
    ("Kodierung abgleichen", "Jeder Kode gegen Kodierrichtlinien, ICD-10-GM, OPS und Akte"),
    ("Abweichungen ausweisen", "kodiert, aber nicht belegt; dokumentiert, aber nicht kodiert"),
    ("Prüfen und freigeben", "Die Kodierfachkraft entscheidet über jede Änderung"),
];

const SOURCES: [&str; 4] = ["Arztbriefe", "Verlauf", "Pflege", "Befunde"];

pub fn draw<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    // when: before the bill, and in the audit procedure
    let when = rise_in(
        t,
        at("c2_1"),
        fframes::svgr!(<g>
            <rect x="130" y="196" width="1660" height="4" rx="2" fill={LINE} />
            {badge(130.0, 230.0, "ENTLASSUNG", MUTED, PANEL, 15.0)}
            {badge(560.0, 230.0, "VOR DER RECHNUNGSSTELLUNG", GREEN, GREEN_BG, 15.0)}
            {badge(1180.0, 230.0, "IM PRÜFVERFAHREN DES MD", BLUE, BLUE_BG, 15.0)}
        </g>),
    );
    let (x0, w, gap, y) = (130.0, 316.0, 20.0, 290.0);
    let mut cards = Vec::new();
    let s2 = at("c2_2");
    for (i, (title, sub)) in STEPS.iter().enumerate() {
        let x = x0 + i as f32 * (w + gap);
        let lit = match i {
            1 => window(t, at("c2_3"), at("c2_3") + 5.5, 0.4),
            4 => window(t, at("c2_4"), at("c2_4") + 7.5, 0.4),
            _ => 0.0,
        };
        let stroke = mix(LINE, AMBER, lit);
        let n = format!("{}", i + 1);
        cards.push(rise_in(
            t,
            s2 + i as f32 * 1.2,
            fframes::svgr!(<g>
                <rect x={x} y={y} width={w} height="250" rx="16" fill={PANEL} stroke={stroke} stroke-width="2" />
                <circle cx={x + 40.0} cy={y + 44.0} r="22" fill={AMBER_BG} stroke={AMBER} stroke-width="2" />
                {mono(x + 40.0, y + 51.0, 20.0, AMBER, "middle", &n)}
                {text(x + 24.0, y + 112.0, 25.0, 500, INK, "start", title)}
                {para(f, ctx, (x + 24.0) as i32, (y + 150.0) as i32, (w - 44.0) as usize, 18, 400, MUTED, sub)}
            </g>),
        ));
        if i + 1 < STEPS.len() {
            cards.push(arrow(x + w + 2.0, y + 125.0, x + w + gap - 2.0, y + 125.0, prog(t, s2 + i as f32 * 1.2 + 0.6, s2 + i as f32 * 1.2 + 1.0), FAINT, 2.0));
        }
    }
    // every answer leads to its place in the record
    let a3 = at("c2_3");
    let mut chips = Vec::new();
    for (i, s) in SOURCES.iter().enumerate() {
        let x = 470.0 + i as f32 * 170.0;
        chips.push(rise_in(t, a3 + 0.4 + i as f32 * 0.3, fframes::svgr!(<g>
            {arrow(x + 70.0, 600.0, 550.0, 548.0, 1.0, FAINT, 1.5)}
            {badge(x, 618.0, s, BLUE, BLUE_BG, 15.0)}
        </g>)));
    }
    let fund = rise_in(t, a3 + 1.8, text(130.0, 624.0, 20.0, 500, BLUE, "start", "Fundstelle:"));
    // the coding specialist decides
    let decides = rise_in(
        t,
        at("c2_4"),
        fframes::svgr!(<g>
            {check(1500.0, 612.0, 16.0, prog(t, at("c2_4") + 0.3, at("c2_4") + 1.0), GREEN)}
            {text(1526.0, 620.0, 20.0, 500, GREEN, "start", "entscheidet: Kodierfachkraft")}
        </g>),
    );
    // in the audit procedure of the Medizinischer Dienst
    let md = rise_in(
        t,
        at("c2_5"),
        fframes::svgr!(<g>
            {card(130.0, 680.0, 1660.0, 170.0, Some(BLUE), PANEL, LINE)}
            {text(170.0, 726.0, 24.0, 500, BLUE, "start", "Im Prüfverfahren des Medizinischen Dienstes")}
            {text(170.0, 772.0, 22.0, 400, INK, "start", "Unterlagen zusammenstellen · belegte Stellungnahme vorbereiten · Gutachten mit der eigenen Beleglage abgleichen")}
            {text(170.0, 814.0, 20.0, 400, MUTED, "start", "Wiederkehrende Ursachen von Kürzungen werden sichtbar.")}
        </g>),
    );
    fframes::svgr!(<g>{when}{cards}{fund}{chips}{decides}{md}</g>)
}
