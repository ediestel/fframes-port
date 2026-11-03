//! Scene 5 · 02:20–02:55 · Validation in the prompt series: a checking prompt,
//! every check with result and reason, an answer naming a room outside the
//! records rejected and returned, reception approval.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::ui::*;

const CHECKS: [(&str, &str); 6] = [
    ("Room exists in the snapshot", "DEMO-S2 found in DEMO-SNAPSHOT-01"),
    ("Dates and nights", "16–19 Nov 2026 · 3 nights"),
    ("Room type and capacity", "single · 1 person"),
    ("Bookings, holds, maintenance", "no conflict for DEMO-S2"),
    ("Selection rule (rev. 2)", "largest eligible single room"),
    ("Answer structure", "all required fields present"),
];

pub fn draw<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let prompt = rise_in(
        t,
        0.6,
        fframes::svgr!(<g>
            {card(120.0, 190.0, 600.0, 230.0, Some(AMBER), PANEL, LINE)}
            {mono(150.0, 232.0, 20.0, AMBER, "start", "prompts/check_answer.md")}
            {text(150.0, 278.0, 21.0, 400, INK, "start", "Verify the answer against the supplied")}
            {text(150.0, 308.0, 21.0, 400, INK, "start", "records. For every check, return the")}
            {text(150.0, 338.0, 21.0, 400, INK, "start", "result and the reason. Reject an answer")}
            {text(150.0, 368.0, 21.0, 400, INK, "start", "that names a room outside the snapshot.")}
        </g>),
    );

    let mut rows = Vec::new();
    for (i, (c, r)) in CHECKS.iter().enumerate() {
        let y = 268.0 + i as f32 * 58.0;
        let at = 2.0 + i as f32 * 1.0;
        rows.push(rise_in(
            t,
            at - 0.3,
            fframes::svgr!(<g>
                {check(800.0, y - 7.0, 16.0, prog(t, at, at + 0.5), GREEN)}
                {text(830.0, y, 21.0, 500, INK, "start", c)}
                <g opacity={prog(t, at, at + 0.5)}>{text(1240.0, y, 20.0, 400, GREEN, "start", r)}</g>
                <rect x="790" y={y + 22.0} width="1010" height="1" fill={LINE} />
            </g>),
        ));
    }
    let panel = rise_in(
        t,
        1.2,
        fframes::svgr!(<g>
            {card(760.0, 190.0, 1064.0, 400.0, Some(GREEN), PANEL, LINE)}
            {text(790.0, 226.0, 21.0, 500, GREEN, "start", "Checks · returned by the LLM as data")}
            {rows}
        </g>),
    );

    // a failed check: the answer is returned
    let fail = rise_in(
        t,
        12.0,
        fframes::svgr!(<g>
            {card(120.0, 620.0, 1060.0, 220.0, Some(RED), PANEL, RED)}
            {text(150.0, 662.0, 20.0, 500, RED, "start", "Example of a rejected answer")}
            {mono(150.0, 706.0, 20.0, INK, "start", "\"recommended\": \"DEMO-S9\"")}
            {cross(170.0, 754.0, 16.0, prog(t, 13.4, 13.9), RED)}
            {text(200.0, 762.0, 20.0, 500, INK, "start", "Room exists in the snapshot: failed — DEMO-S9 is not in DEMO-SNAPSHOT-01")}
            {arrow(150.0, 806.0, 420.0, 806.0, prog(t, 15.0, 15.8), RED, 3.0)}
            <g opacity={prog(t, 15.6, 16.2)}>{text(440.0, 813.0, 20.0, 500, RED, "start", "returned to the LLM with the reason, for correction")}</g>
        </g>),
    );

    // approval
    let press = prog(t, 23.2, 23.8);
    let approve = rise_in(
        t,
        21.4,
        fframes::svgr!(<g>
            {card(1220.0, 620.0, 604.0, 220.0, Some(GREEN), PANEL, LINE)}
            {text(1250.0, 662.0, 20.0, 500, MUTED, "start", "Reception · on Korntal's side")}
            {mono(1250.0, 702.0, 20.0, VIOLET, "start", "GUEST-7Q4M")}
            {text(1420.0, 702.0, 20.0, 500, INK, "start", "→  Lea Beispiel")}
            {button(1250.0, 736.0, 420.0, 58.0, "Geprüften Vorschlag freigeben", GREEN, press)}
            <g opacity={prog(t, 24.0, 24.6)}>{check(1720.0, 765.0, 22.0, prog(t, 24.0, 24.6), GREEN)}</g>
        </g>),
    );

    fframes::svgr!(<g>{prompt}{panel}{fail}{approve}</g>)
}
