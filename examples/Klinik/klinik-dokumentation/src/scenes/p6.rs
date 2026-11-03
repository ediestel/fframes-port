//! Szene 6 · 02:55–03:25 · Ausgabe: der Entlassbrief (Name im Haus
//! eingesetzt) und die Kodierprüfung als strukturierte Daten mit Herkunft im
//! Ausgangsordner; der bestehende Import legt sie in der Patientenakte ab.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::ui::*;

const LETTER: [&str; 8] = [
    "Sehr geehrter Herr Kollege,",
    "",
    "die vom 28.10. bis 04.11.2026 bei uns stationär behandelt wurde.",
    "Diagnosen: ambulant erworbene Pneumonie (J18.9), arterielle",
    "Hypertonie (I10.90), Diabetes mellitus Typ 2 (E11.90).",
    "Unter Amoxicillin/Clavulansäure besserten sich Fieber, Husten",
    "und Entzündungswerte (CRP zuletzt 18 mg/l).",
    "In einfacher Sprache: Sie hatten eine Lungenentzündung.",
];

const RECORD: [(&str, &str); 10] = [
    ("fall", "FALL-2026-0417"),
    ("hauptdiagnose", "J18.9"),
    ("nebendiagnosen", "I10.90, E11.90"),
    ("beleg_J18.9", "Rö-Thorax 28.10. · CRP 148 mg/l"),
    ("beleg_E11.90", "Medikation Metformin · HbA1c"),
    ("hinweis", "keine Abweichung zur Dokumentation"),
    ("quelle", "SRC-5D1Q · 5 Dokumente"),
    ("prompt", "kodierpruefung.md Rev. 1"),
    ("pruefung", "bestanden"),
    ("freigabe", "Stationsärztin"),
];

pub fn draw<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let named = prog(t, 10.6, 11.4);
    let mut lines = Vec::new();
    for (i, l) in LETTER.iter().enumerate() {
        if l.is_empty() {
            continue;
        }
        let y = 290.0 + i as f32 * 40.0;
        let fill = if i == 7 { AMBER } else { INK };
        lines.push(rise_in(t, 1.6 + i as f32 * 0.6, text(150.0, y, 21.0, 400, fill, "start", l)));
    }
    let patient = fframes::svgr!(<g>
        <g opacity={1.0 - named}>{text(150.0, 330.0, 21.0, 500, VIOLET, "start", "wir berichten über PAT-3K9X,")}</g>
        <g opacity={named}>{text(150.0, 330.0, 21.0, 500, INK, "start", "wir berichten über Frau Erika Beispiel, geb. 14.03.1956,")}</g>
    </g>);
    let filed = rise_in(t, 13.4, badge(150.0, 676.0, "Name im Haus eingesetzt · ärztlich freigegeben", GREEN, GREEN_BG, 16.0));
    let letter = rise_in(
        t,
        0.8,
        fframes::svgr!(<g>
            {card(120.0, 190.0, 800.0, 520.0, Some(GREEN), PANEL, LINE)}
            {mono(150.0, 236.0, 18.0, GREEN, "start", "ausgang/FALL-0417_Entlassbrief.txt")}
            {text(890.0, 236.0, 16.0, 400, MUTED, "end", "vom LLM erstellt")}
            <g opacity={enter(t, 2.2, 0.5)}>{patient}</g>
            {lines}
            {filed}
        </g>),
    );

    let mut rows = Vec::new();
    for (i, (k, v)) in RECORD.iter().enumerate() {
        let y = 290.0 + i as f32 * 38.0;
        let prov = i >= 6;
        rows.push(rise_in(
            t,
            9.6 + i as f32 * 0.35,
            fframes::svgr!(<g>
                {mono(1000.0, y, 17.0, if prov { AMBER } else { MUTED }, "start", &format!("\"{k}\":"))}
                {mono(1190.0, y, 17.0, INK, "start", &format!("\"{v}\""))}
            </g>),
        ));
    }
    let record = rise_in(
        t,
        8.8,
        fframes::svgr!(<g>
            {card(970.0, 190.0, 854.0, 520.0, Some(GREEN), PANEL, LINE)}
            {mono(1000.0, 236.0, 18.0, GREEN, "start", "ausgang/FALL-0417_Kodierpruefung.json")}
            {rows}
            <g opacity={enter(t, 13.0, 0.5)}>{text(1794.0, 690.0, 16.0, 400, AMBER, "end", "Herkunft: Quelle · Prompt · Prüfung · Freigabe")}</g>
        </g>),
    );

    let k = prog(t, 14.6, 15.4);
    let commit = prog(t, 16.4, 17.0);
    let import = rise_in(
        t,
        14.2,
        fframes::svgr!(<g>
            {arrow(1396.0, 716.0, 1396.0, 760.0, k, GREEN, 3.0)}
            {icon_db(1160.0, 760.0, 90.0, 70.0, if commit > 0.0 { GREEN } else { BLUE }, PANEL)}
            {text(1230.0, 790.0, 22.0, 500, INK, "start", "bestehender Import · Patientenakte")}
            <g opacity={commit}>{mono(1230.0, 826.0, 20.0, GREEN, "start", "FALL-2026-0417 · 2 Dokumente abgelegt")}</g>
        </g>),
    );

    fframes::svgr!(<g>{letter}{record}{import}</g>)
}
