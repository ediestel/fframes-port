//! Szene 9 · Das Pilotprojekt: die vier Schritte des Exposés und die
//! Bewertungskriterien.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::narration::at;
use crate::ui::*;

const STEPS: [(&str, &str); 4] = [
    ("Aufgabe und Maßstab festlegen", "Fallgruppe, Fallzahl, Laufzeit, Quellen, Erfolgskriterien"),
    ("Referenz aufbauen", "bereits vom MD geprüfte Fälle mit ihrem Gutachten"),
    ("Im Arbeitsablauf erproben", "mit und ohne Assistenz, vergleichbar bearbeitet"),
    ("Gemeinsam entscheiden", "Qualität, Gesamtzeit, Korrekturaufwand, Betriebskosten"),
];

const CRITERIA: [(&str, &str); 4] = [
    ("Fakten und Quellen", "erfundene Angaben, falsche Zuordnungen, Auslassungen"),
    ("Kodierqualität", "Richtlinien, Referenz, Begründung, Gegenbelege"),
    ("Arbeitsaufwand", "gesamte Bearbeitungszeit einschließlich Kontrolle"),
    ("Nutzbarkeit", "Auffindbarkeit der Belege, Nachvollziehbarkeit"),
];

pub fn draw<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let a2 = at("c9_2");
    let mut steps = Vec::new();
    for (i, (title, sub)) in STEPS.iter().enumerate() {
        let x = 130.0 + i as f32 * 420.0;
        let n = format!("{}", i + 1);
        steps.push(rise_in(
            t,
            a2 + i as f32 * 2.2,
            fframes::svgr!(<g>
                {card(x, 200.0, 400.0, 230.0, Some(AMBER), PANEL, LINE)}
                <circle cx={x + 44.0} cy="246" r="22" fill={AMBER_BG} stroke={AMBER} stroke-width="2" />
                {mono(x + 44.0, 253.0, 20.0, AMBER, "middle", &n)}
                {para(f, ctx, (x + 30.0) as i32, 312, 350, 24, 500, INK, title)}
                {para(f, ctx, (x + 30.0) as i32, 380, 350, 18, 400, MUTED, sub)}
            </g>),
        ));
    }
    let intro = rise_in(t, at("c9_1"), text(130.0, 176.0, 22.0, 500, MUTED, "start", "Fachliche Qualität und Wirtschaftlichkeit, an Fällen des Hauses"));
    let a3 = at("c9_3");
    let mut crit = Vec::new();
    for (i, (title, sub)) in CRITERIA.iter().enumerate() {
        let x = 170.0 + (i % 2) as f32 * 820.0;
        let y = 560.0 + (i / 2) as f32 * 110.0;
        crit.push(rise_in(
            t,
            a3 + i as f32 * 0.8,
            fframes::svgr!(<g>
                {check(x + 14.0, y - 8.0, 15.0, prog(t, a3 + i as f32 * 0.8 + 0.3, a3 + i as f32 * 0.8 + 1.0), GREEN)}
                {text(x + 44.0, y, 24.0, 500, INK, "start", title)}
                {text(x + 44.0, y + 34.0, 19.0, 400, MUTED, "start", sub)}
            </g>),
        ));
    }
    let crit_card = rise_in(
        t,
        a3,
        fframes::svgr!(<g>
            {card(130.0, 470.0, 1660.0, 330.0, Some(GREEN), PANEL, LINE)}
            {text(170.0, 514.0, 22.0, 500, GREEN, "start", "Bewertungskriterien")}
        </g>),
    );
    fframes::svgr!(<g>{intro}{steps}{crit_card}{crit}</g>)
}
