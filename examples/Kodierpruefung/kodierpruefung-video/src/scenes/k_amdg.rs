//! Szene 10 · Warum AMDG Engineers: die Seite "Warum AMDG Engineers" aus dem
//! Klinik-Video im Wortlaut des Nutzers (ohne den Begriff Halluzination): je
//! eine Aussage in Amber, darunter die zugehörigen Aussagen in Grün,
//! nacheinander eingeblendet.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::ui::*;

const AMBER_1: &str = "\u{201E}AMDG Engineers verbindet automatisierte Dokumentation mit der Prüfung jeder Aussage. Fehler werden erkannt und korrigiert.\u{201C}";
const GREEN_1A: &str = "\u{201E}Ihr Krankenhaus erhält überprüfbare Ergebnisse: mit nachvollziehbaren Quellen, dokumentierten Prüfergebnissen und ausdrücklich ausgewiesenen Prüflücken.\u{201C}";
const GREEN_1B: &str = "\u{201E}Jede Antwort durchläuft einen iterativen Prüf- und Korrekturprozess, bis alle erkannten Falschaussagen entfernt sind.\u{201C}";
const AMBER_2: &str = "\u{201E}Das System prüft jede Angabe vor der KI-Verarbeitung und kontrolliert die Ergebnisse anschließend so lange, bis alle erkannten Fehler entfernt sind.\u{201C}";
const GREEN_2: &str = "\u{201E}Das Prüfprotokoll dokumentiert Quellen, Prüfergebnisse und die Aspekte, die nicht geprüft wurden.\u{201C}";

pub fn draw<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let w = 1560;
    let a1 = para(f, ctx, 180, 240, w, 32, 500, AMBER, AMBER_1);
    let g1a = para(f, ctx, 180, 348, w, 28, 400, GREEN, GREEN_1A);
    let g1b = para(f, ctx, 180, 446, w, 28, 400, GREEN, GREEN_1B);
    let a2 = para(f, ctx, 180, 650, w, 32, 500, AMBER, AMBER_2);
    let g2 = para(f, ctx, 180, 758, w, 28, 400, GREEN, GREEN_2);
    fframes::svgr!(<g>
        {rise_in(t, 0.5, fframes::svgr!(<g>
            {card(140.0, 180.0, 1640.0, 380.0, Some(AMBER), PANEL, LINE)}
            {a1}
        </g>))}
        {rise_in(t, 4.0, g1a)}
        {rise_in(t, 8.0, g1b)}
        {rise_in(t, 12.0, fframes::svgr!(<g>
            {card(140.0, 590.0, 1640.0, 270.0, Some(AMBER), PANEL, LINE)}
            {a2}
        </g>))}
        {rise_in(t, 16.0, g2)}
    </g>)
}
