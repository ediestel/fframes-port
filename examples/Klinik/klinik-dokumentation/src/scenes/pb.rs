//! Szene 10 · 05:19–06:00 · Anbindung im Haus, frei wählbar: wie die Daten im
//! Haus gesammelt und verteilt werden, entscheidet jede Klinik oder Praxis
//! selbst. Im einfachsten Fall zwei Ordner auf einem Rechner; größere Häuser
//! betreiben einen eigenen Server, der im Haus verteilt und als einziger die
//! Verbindung nach außen hält; auch Browser oder API sind möglich. Am KIS oder
//! an der Praxissoftware wird nichts geändert.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::ui::*;

const BX: f32 = 1300.0;
const SRV: (f32, f32, f32, f32) = (1370.0, 420.0, 400.0, 200.0);

/// The three ways: (top, title, appears).
const WAYS: [(f32, &str, f32); 3] = [
    (210.0, "Zwei Ordner auf einem Rechner", 8.0),
    (400.0, "Eigener Server im Haus", 15.4),
    (590.0, "Browser oder API", 25.2),
];
const CW: f32 = 870.0;
const CH: f32 = 160.0;

pub fn draw<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let z = zones(BX, enter(t, 0.0, 0.8), "Ihr Haus — Anbindung frei wählbar", "Gesicherter Server");
    let (sx, sy, sw, sh) = SRV;
    let srv = rise_in(t, 0.6, server(sx, sy, sw, sh, t, 1.0, "Server des Dienstes"));

    // each house decides: three open slots, filled one after another
    let mut slots = Vec::new();
    for (i, (top, title, at)) in WAYS.iter().enumerate() {
        let n = format!("{}", i + 1);
        let empty = enter(t, 2.0 + i as f32 * 0.4, 0.5) * (1.0 - prog(t, *at, at + 0.4));
        slots.push(show(
            empty,
            0.0,
            fframes::svgr!(<g>
                <rect x="130" y={*top} width={CW} height={CH} rx="16" fill="none" stroke={FAINT} stroke-width="2" stroke-dasharray="10 8" />
                {mono(170.0, top + 92.0, 40.0, FAINT, "start", &n)}
            </g>),
        ));
        slots.push(rise_in(
            t,
            *at,
            fframes::svgr!(<g>
                {card(130.0, *top, CW, CH, Some(BLUE), PANEL, LINE)}
                {badge(160.0, top + 34.0, &n, BLUE, BLUE_BG, 16.0)}
                {text(206.0, top + 41.0, 25.0, 500, INK, "start", title)}
            </g>),
        ));
    }

    // 1: two folders on one computer
    let (a_top, _, _) = WAYS[0];
    let folders = fframes::svgr!(<g>
        {rise_in(t, 10.6, fframes::svgr!(<g>
            {icon_folder(170.0, a_top + 74.0, 64.0, 48.0, BLUE, BLUE_BG)}
            {mono(250.0, a_top + 108.0, 22.0, INK, "start", "eingang/")}
        </g>))}
        {rise_in(t, 12.0, fframes::svgr!(<g>
            {icon_folder(480.0, a_top + 74.0, 64.0, 48.0, GREEN, GREEN_BG)}
            {mono(560.0, a_top + 108.0, 22.0, INK, "start", "ausgang/")}
        </g>))}
        {rise_in(t, 9.0, icon_desktop(900.0, a_top + 128.0, 84.0, MUTED))}
    </g>);

    // 2: an own server that distributes in the house and alone talks outside
    let (b_top, _, _) = WAYS[1];
    let mut desks = Vec::new();
    for i in 0..4 {
        let dx = 560.0 + i as f32 * 110.0;
        let at = 18.4 + i as f32 * 0.25;
        desks.push(arrow(410.0, b_top + 104.0, dx - 4.0, b_top + 110.0, prog(t, at, at + 0.4), FAINT, 2.0));
        desks.push(rise_in(t, at + 0.2, icon_desktop(dx + 26.0, b_top + 136.0, 52.0, MUTED)));
    }
    let house_srv = fframes::svgr!(<g>
        {rise_in(t, 16.6, server(170.0, b_top + 62.0, 230.0, 90.0, t, 1.0, ""))}
        {desks}
        {rise_in(t, 18.8, text(560.0, b_top + 76.0, 19.0, 500, MUTED, "start", "verteilt die Ergebnisse im Haus"))}
    </g>);

    // 3: a browser, or an API between two programs
    let (c_top, _, _) = WAYS[2];
    let web = fframes::svgr!(<g>
        {rise_in(t, 25.8, fframes::svgr!(<g>
            <rect x="170" y={c_top + 66.0} width="440" height="40" rx="20" fill={BG} stroke={LINE} stroke-width="1.5" />
            {lock(194.0, c_top + 84.0, 18.0, 1.0, GREEN)}
            {mono(214.0, c_top + 92.0, 16.0, INK, "start", "https://dokumentation.haus.intern")}
        </g>))}
        {rise_in(t, 27.4, badge(650.0, c_top + 86.0, "API", AMBER, AMBER_BG, 18.0))}
        {rise_in(t, 28.6, text(170.0, c_top + 140.0, 19.0, 400, MUTED, "start", "API: feste Schnittstelle zwischen zwei Programmen"))}
    </g>);

    // every way ends at the same server; the own server is the only way out
    let only = window(t, 20.6, 24.6, 0.4);
    let links = fframes::svgr!(<g>
        {arrow(1004.0, a_top + 80.0, sx - 12.0, sy + 50.0, prog(t, 13.2, 13.9), BLUE, 3.0)}
        {arrow(1004.0, b_top + 80.0, sx - 12.0, sy + 100.0, prog(t, 20.4, 21.1), mix(BLUE, GREEN, only).as_str(), 3.0 + 2.0 * only)}
        {show(only, 0.0, text(1150.0, b_top + 52.0, 19.0, 500, GREEN, "middle", "einzige Verbindung nach außen"))}
        {arrow(1004.0, c_top + 80.0, sx - 12.0, sy + 150.0, prog(t, 29.6, 30.3), BLUE, 3.0)}
    </g>);

    // nothing changes in the KIS or the practice software
    let kis = rise_in(
        t,
        33.6,
        fframes::svgr!(<g>
            {icon_db(1150.0, 700.0, 110.0, 96.0, AMBER, AMBER_BG)}
            {check(1214.0, 696.0, 16.0, prog(t, 36.4, 37.1), GREEN)}
            {text(1150.0, 832.0, 20.0, 500, INK, "middle", "KIS / Praxissoftware")}
            <g opacity={prog(t, 36.6, 37.2)}>{text(1150.0, 860.0, 20.0, 500, GREEN, "middle", "keine Änderung")}</g>
        </g>),
    );

    fframes::svgr!(<g>{z}{slots}{folders}{house_srv}{web}{links}{kis}{srv}</g>)
}
