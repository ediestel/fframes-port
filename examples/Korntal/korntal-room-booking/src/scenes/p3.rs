//! Scene 3 · 00:45–01:30 · Pseudonymisation; data stay local.
//! Identifying items marked, replaced by tokens in the outbound view, the key
//! and the original data stay at Korntal, only the tokenised content crosses,
//! the answer returns with its tokens and is matched locally.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::ui::*;

const BX: f32 = 1170.0;

/// (local information, identifying?, outbound representation, kind: 0 token, 1 omitted, 2 kept fact)
const ROWS: [(&str, bool, &str, u8); 6] = [
    ("Lea Beispiel", true, "GUEST-7Q4M", 0),
    ("lea.beispiel@example.com", true, "contact details omitted", 1),
    ("DEMO-001", false, "OP-8N2K", 0),
    ("DEMO-SNAPSHOT-01", false, "SRC-4B2R", 0),
    ("16.–19.11.2026 · 1 Person · Einzelzimmer", false, "Task-relevant stay and room facts", 2),
    ("Other guests' reservation details", true, "Occupied intervals · pseudonymous refs", 2),
];

pub fn draw<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let z = zones(BX, enter(t, 0.0, 0.8), "Korntal — kontrollierter Bereich", "Entfernte LLM-Verarbeitung");

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
        let is_mono = i == 2 || i == 3;
        let label = if is_mono { mono(lx + 30.0, y, 22.0, &fill, "start", src) } else { text(lx + 30.0, y, 22.0, 500, &fill, "start", src) };
        local.push(fframes::svgr!(<g>{under}{label}</g>));

        let at = 8.6 + i as f32 * 1.4;
        let k = prog(t, at, at + 0.7);
        local.push(show(out_fade, 0.0, arrow(lx + 476.0, y - 8.0, ox + 16.0, y - 8.0, k, FAINT, 2.0)));
        let flip = ease_out(prog(t, at + 0.3, at + 0.9));
        if flip > 0.0 {
            let v = match kind {
                0 => mono(ox + 30.0, y, 24.0, VIOLET, "start", out),
                1 => fframes::svgr!(<g>{text(ox + 30.0, y, 21.0, 400, FAINT, "start", out)}<rect x={ox + 30.0} y={y - 8.0} width="240" height="2" fill={FAINT} /></g>),
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
                {text(lx + 30.0, 258.0, 22.0, 500, MUTED, "start", "Local source")}
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
                {text(ox + 30.0, 258.0, 22.0, 500, MUTED, "start", "Outbound view")}
                {outbound}
            </g>
        ),
    );

    // the key and the original data stay at Korntal
    let lk = prog(t, 13.6, 14.4);
    let mapping = rise_in(
        t,
        12.5,
        fframes::svgr!(
            <g>
                {card(lx, 650.0, 1030.0, 200.0, Some(BLUE), PANEL, BLUE)}
                {lock(lx + 58.0, 700.0, 40.0, lk, BLUE)}
                {text(lx + 100.0, 712.0, 26.0, 500, BLUE, "start", "Key and all original data · remain at Korntal")}
                {mono(lx + 100.0, 760.0, 22.0, INK, "start", "GUEST-7Q4M  ↔  Lea Beispiel")}
                {mono(lx + 100.0, 800.0, 22.0, INK, "start", "OP-8N2K     ↔  DEMO-001")}
                {mono(lx + 560.0, 760.0, 22.0, INK, "start", "SRC-4B2R  ↔  DEMO-SNAPSHOT-01")}
            </g>
        ),
    );

    // only the tokenised content crosses
    let node = (1560.0, 690.0);
    let cross = ease_in_out(prog(t, 22.4, 24.8));
    let packet = if t >= 22.0 {
        let x = lerp(ox, 1250.0, cross);
        let body = fframes::svgr!(
            <g>
                {card(x, 240.0, 470.0, 270.0, Some(VIOLET), PANEL, VIOLET)}
                {text(x + 30.0, 280.0, 22.0, 500, VIOLET, "start", "Pseudonymised content")}
                {mono(x + 30.0, 330.0, 24.0, INK, "start", "GUEST-7Q4M")}
                {mono(x + 30.0, 370.0, 24.0, INK, "start", "OP-8N2K · SRC-4B2R")}
                {text(x + 30.0, 414.0, 21.0, 500, INK, "start", "Stay: 16–19 Nov 2026 · 1 person · single")}
                {text(x + 30.0, 452.0, 21.0, 500, INK, "start", "Rooms: capacity, size, availability")}
            </g>
        );
        show(enter(t, 22.0, 0.5), 0.0, body)
    } else {
        Svgr::empty()
    };
    let llm = rise_in(t, 21.4, llm_node(node.0, node.1, 58.0, t, 0.5 + 0.5 * (t * 1.7).sin(), "LLM"));
    let feed = arrow(1485.0, 516.0, 1530.0, 626.0, prog(t, 25.0, 25.6), VIOLET, 3.0);

    // the answer returns with the same tokens and is matched locally
    let back = ease_in_out(prog(t, 31.4, 33.4));
    let reply = if t >= 31.2 {
        let x = lerp(1380.0, 700.0, back);
        let y = lerp(760.0, 420.0, back);
        let body = fframes::svgr!(
            <g>
                <rect x={x} y={y} width="400" height="96" rx="16" fill={PANEL} stroke={VIOLET} stroke-width="2.5" />
                {text(x + 24.0, y + 36.0, 20.0, 500, VIOLET, "start", "Answer")}
                {mono(x + 24.0, y + 74.0, 22.0, INK, "start", "GUEST-7Q4M · OP-8N2K")}
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
                <g opacity={prog(link_k, 0.8, 1.0)}>{text(920.0, 600.0, 22.0, 500, BLUE, "start", "matched to the guest locally")}</g>
            </g>
        )
    } else {
        Svgr::empty()
    };

    fframes::svgr!(<g>{z}{local_card}{out_card}{mapping}{llm}{feed}{packet}{reply}{link}</g>)
}
