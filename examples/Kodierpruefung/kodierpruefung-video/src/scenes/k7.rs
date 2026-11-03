//! Szene 7 · Betrieb und Datenschutz, nach dem Exposé: Betrieb im Netz des
//! Hauses, Pseudonymisierung vor der Auswertung, Rollen und Protokoll,
//! Verschlüsselung und Datenverwendung; die Verantwortung beim Krankenhaus.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::narration::at;
use crate::ui::*;

/// (x, y, title, detail, caption)
const CARDS: [(f32, f32, &str, &str, &str); 4] = [
    (130.0, 190.0, "Betrieb im Netz des Hauses", "Die Unterlagen werden auf einem Server im Netz des Krankenhauses geführt und verarbeitet.", "c7_1"),
    (975.0, 190.0, "Pseudonymisierung vor der Auswertung", "Personenbezogene Angaben können im Haus pseudonymisiert werden; die Zuordnung bleibt im Haus.", "c7_2"),
    (130.0, 440.0, "Rollen, Zugriffe, Protokoll", "Rollen regeln Lesen, Bearbeiten und Freigeben; Zugriffe und Änderungen werden protokolliert.", "c7_3"),
    (975.0, 440.0, "Verschlüsselung und Datenverwendung", "Übertragungen verschlüsselt; Falldaten nur für den Prüfauftrag, kein allgemeines Modelltraining.", "c7_4"),
];

pub fn draw<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let mut cards = Vec::new();
    for (i, (x, y, title, detail, cue)) in CARDS.iter().enumerate() {
        let icon = match i {
            0 => server(*x + 36.0, *y + 30.0, 120.0, 100.0, t, 1.0, ""),
            1 => fframes::svgr!(<g>{mono(*x + 40.0, *y + 76.0, 22.0, VIOLET, "start", "PAT-3K9X")}{lock(*x + 150.0, *y + 108.0, 30.0, 1.0, BLUE)}</g>),
            2 => fframes::svgr!(<g>{icon_person(*x + 70.0, *y + 34.0, 60.0, BLUE)}{icon_doc(*x + 110.0, *y + 60.0, 40.0, MUTED)}</g>),
            _ => fframes::svgr!(<g>{lock(*x + 90.0, *y + 80.0, 56.0, 1.0, GREEN)}</g>),
        };
        cards.push(rise_in(
            t,
            at(cue),
            fframes::svgr!(<g>
                {card(*x, *y, 815.0, 220.0, Some(BLUE), PANEL, LINE)}
                {icon}
                {text(*x + 200.0, *y + 62.0, 26.0, 500, INK, "start", title)}
                {para(f, ctx, (*x + 200.0) as i32, (*y + 104.0) as i32, 580, 20, 400, MUTED, detail)}
            </g>),
        ));
    }
    let a5 = at("c7_5");
    let note = rise_in(
        t,
        a5,
        fframes::svgr!(<g>
            {card(130.0, 700.0, 1660.0, 120.0, Some(AMBER), PANEL, AMBER)}
            {text(170.0, 750.0, 25.0, 500, INK, "start", "Die Assistenz liefert Hinweise und Entwürfe.")}
            {text(170.0, 790.0, 25.0, 500, AMBER, "start", "Die Verantwortung für Kodierung und Rechnung liegt beim Krankenhaus.")}
        </g>),
    );
    fframes::svgr!(<g>{cards}{note}</g>)
}
