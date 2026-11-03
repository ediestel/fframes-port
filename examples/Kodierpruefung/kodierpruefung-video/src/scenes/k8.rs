//! Szene 8 · Der Nutzen für das Krankenhaus: sechs der zehn Punkte des
//! Exposés als Karten, jede, wenn der Untertitel sie nennt.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::narration::at;
use crate::ui::*;

/// (x, y, figure, title, detail, caption, extra delay, colour)
const TILES: [(f32, f32, &str, &str, &str, &str, f32, &str); 6] = [
    (130.0, 190.0, "3.600–14.800 €", "Erlöse, die nicht verloren gehen", "im Modellfall, für einen einzigen Fall", "c8_1", 0.0, AMBER),
    (700.0, 190.0, "Kein Aufschlag", "für eine zutreffende Rechnung", "§ 275c SGB V", "c8_2", 0.0, GREEN),
    (1270.0, 190.0, "Prüfquote sinkt", "mit zutreffender Abrechnung", "§ 275c SGB V", "c8_2", 1.6, GREEN),
    (130.0, 480.0, "N18.32", "Dokumentiert, aber nicht kodiert", "Leistungen, auf die das Haus Anspruch hat", "c8_3", 0.0, BLUE),
    (700.0, 480.0, "Minuten statt Stunden", "je Fall", "die Rechnung ohne Verzug nach der Entlassung", "c8_4", 0.0, BLUE),
    (1270.0, 480.0, "Entlassbrief und Akte", "Widersprüche vor dem Versand", "mehr Zeit für die Kodierentscheidung", "c8_5", 0.0, VIOLET),
];

pub fn draw<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let mut tiles = Vec::new();
    for (x, y, fig, title, detail, cue, delay, color) in TILES {
        let w = 540.0;
        tiles.push(rise_in(
            t,
            at(cue) + delay,
            fframes::svgr!(<g>
                {card(x, y, w, 260.0, Some(color), PANEL, LINE)}
                {text(x + 36.0, y + 78.0, 46.0, 500, color, "start", fig)}
                {text(x + 36.0, y + 128.0, 24.0, 500, INK, "start", title)}
                {para(f, ctx, (x + 36.0) as i32, (y + 170.0) as i32, (w - 70.0) as usize, 19, 400, MUTED, detail)}
            </g>),
        ));
    }
    fframes::svgr!(<g>{tiles}</g>)
}
