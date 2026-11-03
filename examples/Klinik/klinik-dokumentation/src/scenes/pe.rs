//! Szene 13 · 08:05–08:27 · Warum jetzt: drei Aussagen nacheinander, jede mit
//! ihrer Quelle, im Wortlaut des Nutzers: Verwaltung bindet Kapazität
//! (MB-Monitor 2022), der Prüfdruck steigt (BMG,
//! GKV-Beitragssatzstabilisierungsgesetz), Versicherer setzen KI bereits ein
//! (Geschäftsbericht Versicherungskammer).

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::ui::*;

/// (appears, statement, detail, source, colour)
const ITEMS: [(f32, &str, &str, &str, &str); 3] = [
    (
        1.0,
        "Verwaltung bindet Kapazität.",
        "32 % der befragten Ärzte berichten von mindestens vier Stunden Verwaltungsarbeit täglich.",
        "MB-Monitor 2022 · Marburger Bund Bundesverband",
        AMBER,
    ),
    (
        7.5,
        "Der Prüfdruck steigt.",
        "Höhere gesetzliche Prüfquoten ab 2027.",
        "BMG · GKV-Beitragssatzstabilisierungsgesetz",
        BLUE,
    ),
    (
        14.0,
        "Versicherer setzen KI bereits ein.",
        "KI prüft Krankenhausrechnungen bei den privaten Krankenversicherern der Versicherungskammer.",
        "Geschäftsbericht · konzern-versicherungskammer.de",
        VIOLET,
    ),
];

pub fn draw<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let mut rows = Vec::new();
    for (i, (at, statement, detail, source, color)) in ITEMS.iter().enumerate() {
        let y = 210.0 + i as f32 * 220.0;
        rows.push(rise_in(
            t,
            *at,
            fframes::svgr!(<g>
                {card(130.0, y, 1660.0, 190.0, Some(color), PANEL, LINE)}
                {text(176.0, y + 62.0, 42.0, 500, INK, "start", statement)}
                {para(f, ctx, 176, (y + 108.0) as i32, 1560, 27, 400, MUTED, detail)}
                {mono(176.0, y + 168.0, 16.0, FAINT, "start", source)}
            </g>),
        ));
    }
    fframes::svgr!(<g>{rows}</g>)
}
