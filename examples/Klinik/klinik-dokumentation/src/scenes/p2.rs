//! Szene 2 · 00:15–00:45 · Schnittstelle: Eingangsordner mit der
//! Falldokumentation, Ausgangsordner mit den erstellten Dokumenten und dem
//! Protokoll; optional die direkte Anbindung an das KIS.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::ui::*;

fn file_row(x: f32, y: f32, name: &str, color: &str) -> Svgr<'static> {
    fframes::svgr!(<g>{icon_doc(x, y - 24.0, 30.0, color)}{mono(x + 42.0, y, 19.0, INK, "start", name)}</g>)
}

pub fn draw<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let mark = |a: f32| prog(t, a, a + 0.5);
    // the case documentation
    let ins = [
        ("FALL-0417_Aufnahmebefund.txt", 4.6),
        ("FALL-0417_Labor.csv", 6.8),
        ("FALL-0417_Medikation.txt", 7.6),
        ("FALL-0417_Pflegebericht.txt", 8.4),
        ("FALL-0417_Vorbefunde.pdf", 9.0),
    ];
    let mut in_rows = Vec::new();
    for (i, (n, at)) in ins.iter().enumerate() {
        let y = 300.0 + i as f32 * 44.0 + if i > 0 { 214.0 } else { 0.0 };
        in_rows.push(rise_in(t, *at, file_row(150.0, y, n, BLUE)));
    }
    let preview = rise_in(
        t,
        5.0,
        fframes::svgr!(<g>
            <rect x="150" y="318" width="620" height="186" rx="12" fill={BG} opacity="0.6" stroke={LINE} stroke-width="1.5" />
            {text(176.0, 352.0, 18.0, 400, MUTED, "start", "Erika Beispiel · geb. 14.03.1956 · Station 4B")}
            {marked_line(f, ctx, 176.0, 390.0, 20, INK, AMBER_HL, &[("Aufnahme am 28.10.2026 mit ", None), ("Fieber, Husten, Dyspnoe", Some(mark(5.6)))])}
            {marked_line(f, ctx, 176.0, 424.0, 20, INK, AMBER_HL, &[("Rö-Thorax: ", None), ("Infiltrat rechts basal", Some(mark(6.0))), (" · CRP 148 mg/l", None)])}
            {marked_line(f, ctx, 176.0, 458.0, 20, INK, AMBER_HL, &[("Diagnose: ", None), ("ambulant erworbene Pneumonie", Some(mark(6.4)))])}
            {badge(176.0, 486.0, "Fiktive Beispieldaten", RED, RED_BG, 14.0)}
        </g>),
    );
    let input = rise_in(
        t,
        0.8,
        fframes::svgr!(<g>
            {card(120.0, 190.0, 680.0, 580.0, Some(BLUE), PANEL, LINE)}
            {icon_folder(150.0, 216.0, 46.0, 34.0, BLUE, BLUE_BG)}
            {mono(212.0, 246.0, 26.0, BLUE, "start", "eingang/")}
        </g>),
    );

    // the documents produced from it
    let outs = [
        "FALL-0417_Verlaufsnotiz.txt",
        "FALL-0417_Entlassbrief.txt",
        "FALL-0417_Ueberweisung.txt",
        "FALL-0417_Kodierpruefung.json",
        "FALL-0417_Protokoll.log",
    ];
    let mut out_rows = Vec::new();
    for (i, n) in outs.iter().enumerate() {
        out_rows.push(rise_in(t, 11.8 + i as f32 * 0.5, file_row(1150.0, 300.0 + i as f32 * 44.0, n, GREEN)));
    }
    let output = rise_in(
        t,
        11.2,
        fframes::svgr!(<g>
            {card(1120.0, 190.0, 704.0, 340.0, Some(GREEN), PANEL, LINE)}
            {icon_folder(1150.0, 216.0, 46.0, 34.0, GREEN, GREEN_BG)}
            {mono(1212.0, 246.0, 26.0, GREEN, "start", "ausgang/")}
            {text(1794.0, 246.0, 18.0, 400, MUTED, "end", "nach der Verarbeitung")}
            {out_rows}
        </g>),
    );

    let mid = rise_in(
        t,
        10.6,
        fframes::svgr!(<g>
            {arrow(812.0, 420.0, 1104.0, 360.0, prog(t, 10.8, 11.6), AMBER, 3.0)}
            {icon_doc(900.0, 300.0, 54.0, AMBER)}
            {llm_node(1010.0, 330.0, 34.0, t, 0.5, "")}
            {text(960.0, 470.0, 20.0, 500, MUTED, "middle", "Prompts + LLM-API")}
        </g>),
    );

    let k = prog(t, 20.6, 21.6);
    let direct = rise_in(
        t,
        20.4,
        fframes::svgr!(<g>
            {icon_db(1470.0, 640.0, 120.0, 110.0, BLUE, BLUE_BG)}
            {text(1470.0, 820.0, 22.0, 500, INK, "middle", "Krankenhausinformationssystem")}
            <path d={format!("M1400 690 H{:.1}", lerp(1400.0, 810.0, k))} stroke={BLUE} stroke-width="3" stroke-dasharray="10 8" />
            <path d={format!("M1470 630 V{:.1}", lerp(630.0, 538.0, k))} stroke={BLUE} stroke-width="3" stroke-dasharray="10 8" />
            <g opacity={prog(t, 21.6, 22.2)}>{text(1100.0, 676.0, 20.0, 500, BLUE, "middle", "optional: direkte Anbindung an das KIS")}</g>
        </g>),
    );

    fframes::svgr!(<g>{input}{preview}{in_rows}{mid}{output}{direct}</g>)
}
