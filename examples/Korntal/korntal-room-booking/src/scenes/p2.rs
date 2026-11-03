//! Scene 2 · 00:15–00:45 · Interface: the input folder (request email, read-only
//! database export) and the output folder (reply, record, audit log); a direct
//! database connection as the option.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::ui::*;

const ROOMS: [(&str, &str, &str); 4] = [
    ("DEMO-S1", "Single · 12 m²", "free"),
    ("DEMO-S2", "Single · 18 m²", "free"),
    ("DEMO-D1", "Double · 24 m²", "booked 17–18 Nov"),
    ("DEMO-S3", "Single · 14 m²", "maintenance 16–17 Nov"),
];

fn file_row(x: f32, y: f32, name: &str, color: &str) -> Svgr<'static> {
    fframes::svgr!(<g>{icon_doc(x, y - 26.0, 34.0, color)}{mono(x + 46.0, y, 22.0, INK, "start", name)}</g>)
}

pub fn draw<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    // input folder
    let mark = |a: f32| prog(t, a, a + 0.5);
    let email = rise_in(
        t,
        4.6,
        fframes::svgr!(<g>
            {file_row(150.0, 300.0, "DEMO-001_anfrage.eml", BLUE)}
            <rect x="150" y="320" width="620" height="200" rx="12" fill={BG} opacity="0.6" stroke={LINE} stroke-width="1.5" />
            {text(176.0, 356.0, 19.0, 400, MUTED, "start", "From: Lea Beispiel · lea.beispiel@example.com")}
            {marked_line(f, ctx, 176.0, 398.0, 21, INK, AMBER_HL, &[("ich möchte ", None), ("für eine Person", Some(mark(5.4))), (" ein ", None), ("Einzelzimmer", Some(mark(5.8))), (" vom", None)])}
            {marked_line(f, ctx, 176.0, 434.0, 21, INK, AMBER_HL, &[("16. bis zum 19. November 2026", Some(mark(6.2))), (" anfragen. Ich komme", None)])}
            {text(176.0, 470.0, 21.0, 400, INK, "start", "voraussichtlich gegen 18 Uhr an.")}
            {badge(176.0, 500.0, "Fiktive Beispieldaten", RED, RED_BG, 15.0)}
        </g>),
    );
    let mut rows = Vec::new();
    for (i, (r, ty, st)) in ROOMS.iter().enumerate() {
        let y = 616.0 + i as f32 * 34.0;
        let c = if st.starts_with("free") { GREEN } else { RED };
        rows.push(fframes::svgr!(<g>
            {mono(176.0, y, 19.0, BLUE, "start", r)}
            {text(310.0, y, 19.0, 400, INK, "start", ty)}
            {text(500.0, y, 19.0, 500, c, "start", st)}
        </g>));
    }
    let export = rise_in(
        t,
        8.4,
        fframes::svgr!(<g>
            {file_row(150.0, 574.0, "DEMO-SNAPSHOT-01_rooms.csv", BLUE)}
            {mono(560.0, 574.0, 16.0, MUTED, "start", "read-only")}
            {rows}
        </g>),
    );
    let input = rise_in(
        t,
        0.8,
        fframes::svgr!(<g>
            {card(120.0, 190.0, 680.0, 580.0, Some(BLUE), PANEL, LINE)}
            {icon_folder(150.0, 216.0, 46.0, 34.0, BLUE, BLUE_BG)}
            {mono(212.0, 246.0, 26.0, BLUE, "start", "input/")}
        </g>),
    );

    // output folder
    let outs = [("DEMO-001_antwort.txt", 12.0), ("room-request-OP-8N2K.json", 13.0), ("audit-OP-8N2K.log", 14.0)];
    let mut out_rows = Vec::new();
    for (i, (n, at)) in outs.iter().enumerate() {
        out_rows.push(rise_in(t, *at, file_row(1150.0, 320.0 + i as f32 * 64.0, n, GREEN)));
    }
    let output = rise_in(
        t,
        11.4,
        fframes::svgr!(<g>
            {card(1120.0, 190.0, 704.0, 330.0, Some(GREEN), PANEL, LINE)}
            {icon_folder(1150.0, 216.0, 46.0, 34.0, GREEN, GREEN_BG)}
            {mono(1212.0, 246.0, 26.0, GREEN, "start", "output/")}
            {text(1794.0, 246.0, 18.0, 400, MUTED, "end", "after processing")}
            {out_rows}
        </g>),
    );

    // processing between the folders
    let mid = rise_in(
        t,
        10.6,
        fframes::svgr!(<g>
            {arrow(812.0, 420.0, 1104.0, 360.0, prog(t, 10.8, 11.6), AMBER, 3.0)}
            {icon_doc(900.0, 300.0, 54.0, AMBER)}
            {llm_node(1010.0, 330.0, 34.0, t, 0.5, "")}
            {text(960.0, 470.0, 20.0, 500, MUTED, "middle", "prompts + LLM")}
        </g>),
    );

    // option: a direct database connection
    let k = prog(t, 20.6, 21.6);
    let direct = rise_in(
        t,
        20.4,
        fframes::svgr!(<g>
            {icon_db(1470.0, 640.0, 120.0, 110.0, BLUE, BLUE_BG)}
            {text(1470.0, 820.0, 22.0, 500, INK, "middle", "Korntal database")}
            <path d={format!("M1400 690 H{:.1}", lerp(1400.0, 810.0, k))} stroke={BLUE} stroke-width="3" stroke-dasharray="10 8" />
            <path d={format!("M1470 630 V{:.1}", lerp(630.0, 528.0, k))} stroke={BLUE} stroke-width="3" stroke-dasharray="10 8" />
            <g opacity={prog(t, 21.6, 22.2)}>{text(1100.0, 676.0, 20.0, 500, BLUE, "middle", "optional: direct database connection")}</g>
        </g>),
    );

    fframes::svgr!(<g>{input}{email}{export}{mid}{output}{direct}</g>)
}
