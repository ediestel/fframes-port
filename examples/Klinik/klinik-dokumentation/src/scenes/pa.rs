//! Szene 9 · 04:44–05:19 · Datenübertragung und Sicherheit: was eine Klinik
//! oder Praxis braucht. Technisch nur Senden und Empfangen über eine
//! verschlüsselte Verbindung wie bei jeder sicheren Webseite; die
//! Originaldokumente bleiben im Haus; zum gesicherten Server des Dienstes geht
//! nur der pseudonymisierte Inhalt, wird verarbeitet und zurückgeschickt; auf
//! dem Server bleibt nichts gespeichert; auch das Protokoll kommt zurück und
//! liegt im Haus.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::ui::*;

const BX: f32 = 1170.0;

/// The house's end (left) and the service's server (right).
const HOME: (f32, f32, f32, f32) = (330.0, 400.0, 400.0, 200.0);
const SRV: (f32, f32, f32, f32) = (1390.0, 400.0, 400.0, 200.0);

/// Send and receive lines between the two.
const SEND_Y: f32 = 455.0;
const RECV_Y: f32 = 545.0;

fn packet(cx: f32, cy: f32, label: &str, color: &str) -> Svgr<'static> {
    let w = 300.0;
    let c = color.to_owned();
    fframes::svgr!(<g>
        <rect x={cx - w / 2.0} y={cy - 24.0} width={w} height="48" rx="12" fill={PANEL} stroke={c} stroke-width="2.5" />
        {mono(cx, cy + 7.0, 17.0, color, "middle", label)}
    </g>)
}

pub fn draw<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let z = zones(BX, enter(t, 0.0, 0.8), "Klinik oder Praxis", "Gesicherter Server des Dienstes");

    // the question the viewer has
    let question = during(
        t,
        1.0,
        5.6,
        fframes::svgr!(<g>
            {text(625.0, 470.0, 54.0, 500, INK, "middle", "Was brauche ich dafür?")}
            {text(625.0, 548.0, 54.0, 500, BLUE, "middle", "Und wie sicher ist das?")}
        </g>),
    );

    // sending and receiving, nothing more
    let (hx, hy, hw, hh) = HOME;
    let (sx, sy, sw, sh) = SRV;
    let busy = window(t, 20.8, 23.0, 0.4);
    let home = rise_in(t, 6.0, server(hx, hy, hw, hh, t, 1.0, "Ihr Rechner oder Server"));
    let srv = rise_in(t, 6.3, server(sx, sy, sw, sh, t * (1.0 + 2.0 * busy), 1.0, "Server des Dienstes"));
    let send = arrow(hx + hw + 20.0, SEND_Y, sx - 20.0, SEND_Y, prog(t, 6.8, 7.8), BLUE, 3.5);
    let recv = arrow(sx - 20.0, RECV_Y, hx + hw + 20.0, RECV_Y, prog(t, 8.2, 9.2), GREEN, 3.5);
    let words = fframes::svgr!(<g>
        {show(enter(t, 7.2, 0.4), 0.0, text(950.0, SEND_Y - 18.0, 21.0, 500, BLUE, "middle", "Senden"))}
        {show(enter(t, 8.6, 0.4), 0.0, text(950.0, RECV_Y + 38.0, 21.0, 500, GREEN, "middle", "Empfangen"))}
    </g>);
    let lk = prog(t, 10.2, 10.8);
    let https = rise_in(
        t,
        9.9,
        fframes::svgr!(<g>
            <rect x={BX - 150.0} y="642" width="300" height="46" rx="23" fill={BG} stroke={LINE} stroke-width="1.5" />
            {lock(BX - 118.0, 660.0, 22.0, lk, GREEN)}
            {mono(BX - 96.0, 672.0, 19.0, INK, "start", "https:// · TLS")}
            {text(BX, 722.0, 20.0, 500, MUTED, "middle", "verschlüsselt, wie jede sichere Webseite")}
        </g>),
    );

    // the original documents stay in the house
    let mut originals = Vec::new();
    for i in 0..4 {
        originals.push(icon_doc(150.0 + i as f32 * 14.0, 690.0 - i as f32 * 10.0, 56.0, BLUE));
    }
    let originals = rise_in(
        t,
        14.8,
        fframes::svgr!(<g>
            {originals}
            {lock(268.0, 744.0, 32.0, prog(t, 15.3, 15.9), BLUE)}
            {text(310.0, 740.0, 22.0, 500, INK, "start", "Originaldokumente")}
            {text(310.0, 772.0, 20.0, 500, BLUE, "start", "bleiben im Haus")}
        </g>),
    );

    // only the pseudonymised content goes out, and comes back processed
    let mut moving = Vec::new();
    if t >= 17.4 && t < 20.4 {
        let k = ease_in_out(prog(t, 17.6, 19.8));
        let x = lerp(hx + hw - 40.0, sx + 40.0, k);
        moving.push(show(window(t, 17.4, 20.4, 0.3), 0.0, packet(x, SEND_Y - 62.0, "PAT-3K9X · pseudonymisiert", VIOLET)));
    }
    if t >= 22.4 && t < 25.4 {
        let k = ease_in_out(prog(t, 22.6, 24.8));
        let x = lerp(sx + 40.0, hx + hw - 40.0, k);
        moving.push(show(window(t, 22.4, 25.4, 0.3), 0.0, packet(x, RECV_Y + 60.0, "Dokumente · Protokoll", GREEN)));
    }
    let working = show(busy, 0.0, text(sx + sw / 2.0, sy - 22.0, 21.0, 500, VIOLET, "middle", "verarbeitet …"));

    // nothing stays on the server
    let kept = rise_in(
        t,
        26.6,
        fframes::svgr!(<g>
            <rect x={sx} y="660" width={sw} height="96" rx="14" fill={BG} stroke={GREEN} stroke-width="2" />
            {text(sx + 24.0, 700.0, 20.0, 400, MUTED, "start", "gespeicherte Falldaten")}
            {mono(sx + 24.0, 736.0, 22.0, GREEN, "start", "0")}
            {check(sx + sw - 36.0, 708.0, 18.0, prog(t, 27.4, 28.1), GREEN)}
        </g>),
    );
    let log = rise_in(
        t,
        29.6,
        fframes::svgr!(<g>
            {icon_doc(640.0, 690.0, 50.0, GREEN)}
            {mono(690.0, 722.0, 19.0, GREEN, "start", "Protokoll.log")}
            {text(690.0, 752.0, 19.0, 500, INK, "start", "liegt im Haus")}
            {check(906.0, 714.0, 15.0, prog(t, 30.2, 30.9), GREEN)}
        </g>),
    );

    fframes::svgr!(<g>{z}{question}{home}{srv}{send}{recv}{words}{https}{originals}{moving}{working}{kept}{log}</g>)
}
