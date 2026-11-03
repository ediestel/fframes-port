//! Szene · Die Kosten der MD-Prüfungen: die Zahlen aus dem Klinik-Video,
//! jede mit ihrer Quelle auf der Karte: Personalkosten je Haus (DKI 2026),
//! Rückzahlungen 2024 (GKV-Statistik), Beanstandungsquote (MD Bund),
//! steigende Prüfquoten ab 2027 (BMG, GKV-Beitragssatzstabilisierungsgesetz).

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::narration::at;
use crate::ui::*;

/// (x, y, figure, label, detail, source, caption, delay, colour)
const TILES: [(f32, f32, &str, &str, &str, &str, &str, f32, &str); 4] = [
    (130.0, 190.0, "236.000 €", "je Haus und Jahr für MD-Prüfungen", "rund vier Vollkräfte; bundesweit rund 245 Mio. € im Jahr", "DKI-Umfrage 2026", "cm_1", 0.0, BLUE),
    (975.0, 190.0, "1,19 Mrd. €", "Rückzahlungen an die Kassen", "nach Prüfungen, bundesweit, Abrechnungsjahr 2024", "GKV-Statistik", "cm_2", 0.0, BLUE),
    (130.0, 500.0, "rund 50 %", "der geprüften Rechnungen beanstandet", "seit dem MDK-Reformgesetz im Mittel kaum verändert", "MD Bund, Stellungnahme", "cm_3", 0.0, AMBER),
    (975.0, 500.0, "ab 2027", "höhere gesetzliche Prüfquoten", "Prüfquote enger an die Abrechnungsqualität gebunden", "BMG · GKV-Beitragssatzstabilisierungsgesetz", "cm_3", 2.5, AMBER),
];

pub fn draw<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let mut tiles = Vec::new();
    for (x, y, fig, label, detail, src, cue, delay, color) in TILES {
        let (w, h) = (815.0, 270.0);
        tiles.push(rise_in(
            t,
            at(cue) + delay,
            fframes::svgr!(<g>
                {card(x, y, w, h, Some(color), PANEL, LINE)}
                {text(x + 36.0, y + 82.0, 58.0, 500, color, "start", fig)}
                {text(x + 36.0, y + 132.0, 25.0, 500, INK, "start", label)}
                {para(f, ctx, (x + 36.0) as i32, (y + 174.0) as i32, (w - 70.0) as usize, 19, 400, MUTED, detail)}
                {mono(x + 36.0, y + h - 20.0, 15.0, FAINT, "start", src)}
            </g>),
        ));
    }
    fframes::svgr!(<g>{tiles}</g>)
}
