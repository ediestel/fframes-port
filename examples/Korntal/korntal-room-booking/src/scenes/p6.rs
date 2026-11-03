//! Scene 6 · 02:55–03:25 · Output: the reply drafted by the LLM (token replaced
//! by the name locally) and the record in the agreed JSON format with its
//! provenance, both in the output folder; Korntal's existing import.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::ui::*;

const LETTER: [&str; 7] = [
    "vielen Dank für Ihre Anfrage. Für den 16. bis 19. November",
    "2026 (3 Nächte) haben wir ein Einzelzimmer für Sie vorgemerkt.",
    "Ihre Ankunft gegen 18 Uhr haben wir notiert. Bitte teilen Sie",
    "uns mit, ob Sie Verpflegung wünschen. Preis und verbindliche",
    "Buchung bestätigen wir Ihnen gesondert.",
    "Freundliche Grüße",
    "Ihre Rezeption",
];

const RECORD: [(&str, &str); 11] = [
    ("request", "DEMO-001"),
    ("operation", "OP-8N2K"),
    ("stay", "2026-11-16 / 2026-11-19 · 3 nights"),
    ("room", "DEMO-S2"),
    ("alternative", "DEMO-S1"),
    ("reason", "largest eligible single room"),
    ("open", "price · meals · arrival"),
    ("source", "DEMO-SNAPSHOT-01"),
    ("prompt", "rev. 2"),
    ("checks", "passed"),
    ("approved_by", "reception"),
];

pub fn draw<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    // the reply
    let named = prog(t, 10.6, 11.4);
    let mut lines = Vec::new();
    for (i, l) in LETTER.iter().enumerate() {
        let y = 340.0 + i as f32 * 38.0 + if i >= 5 { 24.0 } else { 0.0 };
        lines.push(rise_in(t, 2.0 + i as f32 * 0.7, text(150.0, y, 22.0, 400, INK, "start", l)));
    }
    let salutation = fframes::svgr!(<g>
        <g opacity={1.0 - named}>{text(150.0, 294.0, 22.0, 500, VIOLET, "start", "Guten Tag GUEST-7Q4M,")}</g>
        <g opacity={named}>{text(150.0, 294.0, 22.0, 500, INK, "start", "Guten Tag Lea Beispiel,")}</g>
    </g>);
    let sent = rise_in(t, 13.4, badge(150.0, 676.0, "name inserted locally · sent by reception", GREEN, GREEN_BG, 16.0));
    let letter = rise_in(
        t,
        0.8,
        fframes::svgr!(<g>
            {card(120.0, 190.0, 800.0, 520.0, Some(GREEN), PANEL, LINE)}
            {mono(150.0, 236.0, 18.0, GREEN, "start", "output/DEMO-001_antwort.txt")}
            {text(890.0, 236.0, 16.0, 400, MUTED, "end", "drafted by the LLM")}
            <g opacity={enter(t, 1.4, 0.5)}>{salutation}</g>
            {lines}
            {sent}
        </g>),
    );

    // the record
    let mut rows = Vec::new();
    for (i, (k, v)) in RECORD.iter().enumerate() {
        let y = 290.0 + i as f32 * 36.0;
        let prov = i >= 7;
        rows.push(rise_in(
            t,
            9.6 + i as f32 * 0.35,
            fframes::svgr!(<g>
                {mono(1000.0, y, 18.0, if prov { AMBER } else { MUTED }, "start", &format!("\"{k}\":"))}
                {mono(1200.0, y, 18.0, INK, "start", &format!("\"{v}\""))}
            </g>),
        ));
    }
    let record = rise_in(
        t,
        8.8,
        fframes::svgr!(<g>
            {card(970.0, 190.0, 854.0, 520.0, Some(GREEN), PANEL, LINE)}
            {mono(1000.0, 236.0, 18.0, GREEN, "start", "output/room-request-OP-8N2K.json")}
            {text(1794.0, 236.0, 16.0, 400, MUTED, "end", "agreed JSON format")}
            {rows}
            <g opacity={enter(t, 13.0, 0.5)}>{text(1794.0, 690.0, 16.0, 400, AMBER, "end", "provenance: source · prompt · checks · approval")}</g>
        </g>),
    );

    // Korntal's existing import
    let k = prog(t, 14.6, 15.4);
    let commit = prog(t, 16.4, 17.0);
    let pending = window(t, 19.5, 30.5, 0.4);
    let import = rise_in(
        t,
        14.2,
        fframes::svgr!(<g>
            {arrow(1396.0, 716.0, 1396.0, 760.0, k, GREEN, 3.0)}
            {icon_db(1160.0, 760.0, 90.0, 70.0, if commit > 0.0 { GREEN } else { BLUE }, PANEL)}
            {text(1230.0, 790.0, 22.0, 500, INK, "start", "Korntal's existing import")}
            <g opacity={commit}>
                {mono(1230.0, 826.0, 20.0, GREEN, "start", "DEMO-DB-1042 · committed")}
            </g>
            <g opacity={commit}>{badge(1560.0, 820.0, "reservation: approval pending", AMBER, mix(AMBER_BG, "#5A3D0C", pending).as_str(), 15.0)}</g>
        </g>),
    );

    fframes::svgr!(<g>{letter}{record}{import}</g>)
}
