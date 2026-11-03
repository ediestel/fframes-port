//! Szene 5 · Die Folge für die Vergütung: A13F (abgerechnet) gegen die nach
//! der Korrektur in Frage kommenden DRG, mit den Werten des Exposés
//! (aG-DRG-Katalog 2025, Landesbasisfallwert Niedersachsen 2025, 14 Tage).

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::narration::at;
use crate::ui::*;

/// (DRG, note, sum in €, shown sum, difference)
const DRGS: [(&str, &str, f32, &str, &str); 6] = [
    ("A13F", "abgerechnet", 25665.64, "25.665,64 €", ""),
    ("I08B", "", 22021.50, "22.021,50 €", "−3.644 € (−14 %)"),
    ("I08C", "", 16783.81, "16.783,81 €", "−8.882 € (−35 %)"),
    ("I08D", "", 14764.38, "14.764,38 €", "−10.901 € (−42 %)"),
    ("I47A", "", 12499.54, "12.499,54 €", "−13.166 € (−51 %)"),
    ("I47B", "", 10825.45, "10.825,45 €", "−14.840 € (−58 %)"),
];

const BAR_X: f32 = 420.0;
const BAR_MAX: f32 = 880.0;
const TOP: f32 = 210.0;
const STEP: f32 = 86.0;

pub fn draw<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let mut rows = Vec::new();
    for (i, (drg, note, sum, shown, diff)) in DRGS.iter().enumerate() {
        let y = TOP + i as f32 * STEP;
        let at_i = if i == 0 { at("c5_1") } else { at("c5_2") + 0.6 + (i - 1) as f32 * 0.9 };
        let k = ease_out(prog(t, at_i, at_i + 0.9));
        let w = BAR_MAX * sum / 25665.64 * k;
        let color = if i == 0 { AMBER } else if i <= 3 { BLUE } else { FAINT };
        let label = if note.is_empty() { (*drg).to_owned() } else { format!("{drg}  ({note})") };
        rows.push(show(
            enter(t, at_i, 0.4),
            0.0,
            fframes::svgr!(<g>
                {text(150.0, y + 40.0, 26.0, 500, INK, "start", &label)}
                <rect x={BAR_X} y={y + 14.0} width={w.max(0.5)} height="36" rx="8" fill={color} opacity="0.9" />
                <g opacity={prog(k, 0.8, 1.0)}>
                    {text(BAR_X + w + 18.0, y + 41.0, 24.0, 500, INK, "start", shown)}
                    {text(1600.0, y + 41.0, 22.0, 500, RED, "end", diff)}
                </g>
            </g>),
        ));
    }
    // I08B–I08D most likely; I47 the lower bound
    let a2 = at("c5_2");
    let likely = rise_in(
        t,
        a2 + 4.0,
        fframes::svgr!(<g>
            <path d={format!("M1640 {:.1} h20 V{:.1} h-20", TOP + STEP + 14.0, TOP + 3.0 * STEP + 50.0)} fill="none" stroke={GREEN} stroke-width="3" />
            {text(1676.0, TOP + 2.0 * STEP + 40.0, 20.0, 500, GREEN, "start", "plausibel")}
            {text(1676.0, TOP + 4.5 * STEP + 40.0, 20.0, 500, MUTED, "start", "Untergrenze")}
        </g>),
    );
    // the nursing revenue alone
    let pflege = rise_in(
        t,
        at("c5_3"),
        fframes::svgr!(<g>
            {card(130.0, 744.0, 1660.0, 104.0, Some(AMBER), PANEL, LINE)}
            {text(170.0, 788.0, 23.0, 500, INK, "start", "Pflegeerlös je Tag: A13F 2,96 · Hüft-DRG rund 0,9 – über 14 Tage allein rund 7.000 €")}
            <g opacity={enter(t, at("c5_4"), 0.6)}>
                {mono(170.0, 826.0, 14.0, FAINT, "start", "aG-DRG-Katalog 2025 · Landesbasisfallwert Niedersachsen 4.421,37 € · Pflegeentgeltwert 250 € · 14 Belegungstage · DRG nach Korrektur ohne Grouper nicht bestimmt")}
            </g>
        </g>),
    );
    fframes::svgr!(<g>{rows}{likely}{pflege}</g>)
}
