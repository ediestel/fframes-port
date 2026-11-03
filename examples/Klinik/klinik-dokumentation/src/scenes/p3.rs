//! Szene 3 · 00:45–01:30 · Pseudonymisierung; Daten bleiben im Haus.
//! Identifizierende Angaben markiert, im ausgehenden Inhalt durch Tokens
//! ersetzt oder entfernt; Schlüssel und Originaldaten bleiben im Krankenhaus;
//! nur der pseudonymisierte Inhalt geht über die API an zugelassene Server;
//! die Antwort kommt mit denselben Tokens zurück und wird im Haus zugeordnet.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::ui::*;

const BX: f32 = 1170.0;

/// (lokale Angabe, identifizierend?, ausgehende Darstellung, Art: 0 Token, 1 entfernt, 2 Fakt)
const ROWS: [(&str, bool, &str, u8); 6] = [
    ("Erika Beispiel", true, "PAT-3K9X", 0),
    ("geb. 14.03.1956", true, "Alter: 70 Jahre", 2),
    ("FALL-2026-0417", false, "CASE-7T2M", 0),
    ("Hausarzt Dr. Muster, Musterstr. 5", true, "Adresse entfernt", 1),
    ("Befunde, Labor, Medikation", false, "Klinische Fakten zur Aufgabe", 2),
    ("Tochter, Tel. 0711 …", true, "Kontaktdaten entfernt", 1),
];

pub fn draw<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let z = zones(BX, enter(t, 0.0, 0.8), "Klinik — kontrollierter Bereich", "LLM-API · zugelassene Server");

    let (lx, ox, y0, step) = (120.0, 650.0, 300.0, 54.0);
    let out_fade = 1.0 - prog(t, 22.0, 22.8);
    let mut local = Vec::new();
    let mut outbound = Vec::new();
    for (i, (src, ident, out, kind)) in ROWS.iter().enumerate() {
        let y = y0 + i as f32 * step;
        let mark_k = if *ident { prog(t, 1.5 + i as f32 * 0.4, 2.1 + i as f32 * 0.4) } else { 0.0 };
        let fill = if *ident { mix(INK, RED, mark_k) } else { INK.to_owned() };
        let under = if mark_k > 0.0 {
            fframes::svgr!(<rect x={lx + 30.0} y={y + 8.0} width={440.0 * ease_out(mark_k)} height="3" fill={RED} />)
        } else {
            Svgr::empty()
        };
        let is_mono = i == 2;
        let label = if is_mono { mono(lx + 30.0, y, 22.0, &fill, "start", src) } else { text(lx + 30.0, y, 22.0, 500, &fill, "start", src) };
        local.push(fframes::svgr!(<g>{under}{label}</g>));

        let at = 8.6 + i as f32 * 1.4;
        let k = prog(t, at, at + 0.7);
        local.push(show(out_fade, 0.0, arrow(lx + 476.0, y - 8.0, ox + 16.0, y - 8.0, k, FAINT, 2.0)));
        let flip = ease_out(prog(t, at + 0.3, at + 0.9));
        if flip > 0.0 {
            let v = match kind {
                0 => mono(ox + 30.0, y, 24.0, VIOLET, "start", out),
                1 => fframes::svgr!(<g>{text(ox + 30.0, y, 21.0, 400, FAINT, "start", out)}<rect x={ox + 30.0} y={y - 8.0} width="200" height="2" fill={FAINT} /></g>),
                _ => text(ox + 30.0, y, 21.0, 500, INK, "start", out),
            };
            outbound.push(moved(flip, format!("translate(0 {:.1}) scale(1 {:.3}) translate(0 {:.1})", y, flip.max(0.05), -y), v));
        }
    }
    let local_card = rise_in(
        t,
        0.4,
        fframes::svgr!(
            <g>
                {card(lx, 220.0, 500.0, 400.0, Some(BLUE), PANEL, LINE)}
                {text(lx + 30.0, 258.0, 22.0, 500, MUTED, "start", "Originaldaten im Haus")}
                {local}
            </g>
        ),
    );
    let out_card = show(
        enter(t, 8.0, 0.5) * out_fade,
        0.0,
        fframes::svgr!(
            <g>
                {card(ox, 220.0, 500.0, 400.0, Some(VIOLET), PANEL, LINE)}
                {text(ox + 30.0, 258.0, 22.0, 500, MUTED, "start", "Ausgehender Inhalt")}
                {outbound}
            </g>
        ),
    );

    let lk = prog(t, 13.6, 14.4);
    let mapping = rise_in(
        t,
        12.5,
        fframes::svgr!(
            <g>
                {card(lx, 650.0, 1030.0, 200.0, Some(BLUE), PANEL, BLUE)}
                {lock(lx + 58.0, 700.0, 40.0, lk, BLUE)}
                {text(lx + 100.0, 712.0, 26.0, 500, BLUE, "start", "Schlüssel und alle Originaldaten · bleiben im Krankenhaus")}
                {mono(lx + 100.0, 760.0, 22.0, INK, "start", "PAT-3K9X   ↔  Erika Beispiel")}
                {mono(lx + 100.0, 800.0, 22.0, INK, "start", "CASE-7T2M  ↔  FALL-2026-0417")}
                {mono(lx + 560.0, 760.0, 22.0, INK, "start", "SRC-5D1Q  ↔  Quelldokumente")}
            </g>
        ),
    );

    let node = (1560.0, 690.0);
    let cross = ease_in_out(prog(t, 22.4, 24.8));
    let packet = if t >= 22.0 {
        let x = lerp(ox, 1250.0, cross);
        let body = fframes::svgr!(
            <g>
                {card(x, 240.0, 470.0, 270.0, Some(VIOLET), PANEL, VIOLET)}
                {text(x + 30.0, 280.0, 22.0, 500, VIOLET, "start", "Pseudonymisierter Inhalt")}
                {mono(x + 30.0, 330.0, 24.0, INK, "start", "PAT-3K9X")}
                {mono(x + 30.0, 370.0, 24.0, INK, "start", "CASE-7T2M · SRC-5D1Q")}
                {text(x + 30.0, 414.0, 21.0, 500, INK, "start", "Alter 70 · Pneumonie · CRP 148 mg/l")}
                {text(x + 30.0, 452.0, 21.0, 500, INK, "start", "Medikation · Verlauf · Aufgabe")}
            </g>
        );
        show(enter(t, 22.0, 0.5), 0.0, body)
    } else {
        Svgr::empty()
    };
    let llm = rise_in(t, 21.4, llm_node(node.0, node.1, 58.0, t, 0.5 + 0.5 * (t * 1.7).sin(), "API"));
    let feed = arrow(1485.0, 516.0, 1530.0, 626.0, prog(t, 25.0, 25.6), VIOLET, 3.0);

    let back = ease_in_out(prog(t, 31.4, 33.4));
    let reply = if t >= 31.2 {
        let x = lerp(1380.0, 700.0, back);
        let y = lerp(760.0, 420.0, back);
        let body = fframes::svgr!(
            <g>
                <rect x={x} y={y} width="400" height="96" rx="16" fill={PANEL} stroke={VIOLET} stroke-width="2.5" />
                {text(x + 24.0, y + 36.0, 20.0, 500, VIOLET, "start", "Antwort")}
                {mono(x + 24.0, y + 74.0, 22.0, INK, "start", "PAT-3K9X · CASE-7T2M")}
            </g>
        );
        show(enter(t, 31.2, 0.4), 0.0, body)
    } else {
        Svgr::empty()
    };
    let link_k = prog(t, 34.0, 35.2);
    let link = if link_k > 0.0 {
        let len = 160.0 * link_k;
        fframes::svgr!(
            <g>
                <path d={format!("M900 516 V{:.1}", 516.0 + len)} stroke={BLUE} stroke-width="3" stroke-dasharray="8 8" />
                <g opacity={prog(link_k, 0.8, 1.0)}>{text(920.0, 600.0, 22.0, 500, BLUE, "start", "im Haus der Patientin zugeordnet")}</g>
            </g>
        )
    } else {
        Svgr::empty()
    };

    fframes::svgr!(<g>{z}{local_card}{out_card}{mapping}{llm}{feed}{packet}{reply}{link}</g>)
}
