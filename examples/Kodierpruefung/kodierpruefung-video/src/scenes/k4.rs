//! Szene 4 · Die Beatmungsstunden: kodiert 97, belegt 87. Eine Zeitachse
//! von der Intubation (06.11.2025, 04:24) bis zur Extubation (09.11.2025,
//! 18:30), 86 Stunden 6 Minuten, aufgerundet 87; die Schwelle der DRG A13F
//! (mehr als 95 Stunden) und die kodierten 97 Stunden dahinter.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::narration::at;
use crate::ui::*;

/// Hours to screen x on the axis (0 h at 200, 100 h at 1700).
fn hx(h: f32) -> f32 {
    200.0 + h * 15.0
}

const AXIS_Y: f32 = 560.0;
/// 86 h 6 min.
const VENT_H: f32 = 86.1;

pub fn draw<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let a1 = at("c4_1");
    let a2 = at("c4_2");
    let a3 = at("c4_3");
    let a4 = at("c4_4");
    // the two numbers
    let coded = rise_in(
        t,
        a1,
        fframes::svgr!(<g>
            {text(200.0, 300.0, 24.0, 500, MUTED, "start", "kodiert")}
            {text(200.0, 390.0, 96.0, 500, RED, "start", "97")}
            {text(330.0, 390.0, 34.0, 400, MUTED, "start", "Stunden")}
        </g>),
    );
    let strike = prog(t, a2 + 7.0, a2 + 7.6);
    let strike_bar = show(strike, 0.0, fframes::svgr!(<rect x="196" y="350" width={(130.0 * strike).max(0.5)} height="6" rx="3" fill={RED} />));
    let belegt = rise_in(
        t,
        a2 + 7.4,
        fframes::svgr!(<g>
            {text(640.0, 300.0, 24.0, 500, GREEN, "start", "belegt")}
            {text(640.0, 390.0, 96.0, 500, GREEN, "start", "87")}
            {text(770.0, 390.0, 34.0, 400, MUTED, "start", "Stunden")}
            {text(940.0, 382.0, 22.0, 400, INK, "start", "86 Stunden 6 Minuten, aufgerundet")}
        </g>),
    );
    // the axis and the ventilated time
    let grow = ease_in_out(prog(t, a2 + 0.8, a2 + 5.6));
    let hours_now = VENT_H * grow;
    let count = format!("{:.0} h", hours_now.floor());
    let mut ticks = Vec::new();
    for h in (0..=100).step_by(10) {
        let x = hx(h as f32);
        let label = format!("{h}");
        ticks.push(fframes::svgr!(<g>
            <rect x={x - 1.0} y={AXIS_Y + 30.0} width="2" height="10" fill={FAINT} />
            {mono(x, AXIS_Y + 64.0, 14.0, FAINT, "middle", &label)}
        </g>));
    }
    let axis = rise_in(
        t,
        a2,
        fframes::svgr!(<g>
            <rect x={hx(0.0)} y={AXIS_Y} width={hx(100.0) - hx(0.0)} height="24" rx="12" fill={PANEL} stroke={LINE} stroke-width="1.5" />
            {ticks}
            {mono(hx(100.0), AXIS_Y + 64.0, 14.0, FAINT, "start", "  h")}
            {text(hx(0.0), AXIS_Y - 24.0, 20.0, 500, INK, "start", "Intubation 06.11.2025, 04:24")}
        </g>),
    );
    let bar = show(
        prog(t, a2 + 0.8, a2 + 1.0),
        0.0,
        fframes::svgr!(<g>
            <rect x={hx(0.0)} y={AXIS_Y} width={(hx(hours_now) - hx(0.0)).max(0.5)} height="24" rx="12" fill={GREEN} opacity="0.85" />
            {mono(hx(hours_now) - 8.0, AXIS_Y + 17.0, 15.0, BG, "end", &count)}
        </g>),
    );
    let extub = rise_in(
        t,
        a2 + 5.6,
        fframes::svgr!(<g>
            <rect x={hx(VENT_H) - 1.5} y={AXIS_Y - 60.0} width="3" height="84" fill={GREEN} />
            {text(hx(VENT_H) - 12.0, AXIS_Y - 70.0, 20.0, 500, GREEN, "end", "Extubation 09.11.2025, 18:30")}
        </g>),
    );
    // A13F needs more than 95 hours; 97 were coded
    let thresh = rise_in(
        t,
        a3,
        fframes::svgr!(<g>
            <path d={format!("M{:.1} {:.1} V{:.1}", hx(95.0), AXIS_Y - 110.0, AXIS_Y + 40.0)} stroke={AMBER} stroke-width="3" stroke-dasharray="8 6" />
            {text(hx(95.0) - 14.0, AXIS_Y - 120.0, 21.0, 500, AMBER, "end", "A13F: mehr als 95 Stunden")}
        </g>),
    );
    let coded_mark = rise_in(
        t,
        a3 + 1.2,
        fframes::svgr!(<g>
            <circle cx={hx(97.0)} cy={AXIS_Y + 12.0} r="10" fill={RED} />
            {text(hx(97.0), AXIS_Y + 110.0, 20.0, 500, RED, "middle", "kodiert: 97")}
        </g>),
    );
    // after the extubation does not count
    let after = rise_in(
        t,
        a4,
        fframes::svgr!(<g>
            <rect x={hx(VENT_H) + 4.0} y={AXIS_Y + 6.0} width={hx(97.0) - hx(VENT_H) - 14.0} height="12" rx="6" fill="none" stroke={RED} stroke-width="2" stroke-dasharray="6 5" />
            {text(hx(91.5), AXIS_Y + 150.0, 19.0, 400, MUTED, "middle", "nach der Extubation: zählt nicht")}
            {card(200.0, 760.0, 900.0, 80.0, Some(RED), PANEL, LINE)}
            {icon_doc(230.0, 776.0, 44.0, RED)}
            {text(290.0, 808.0, 21.0, 500, INK, "start", "Entlassbrief: Beatmung „bis 10.11.2025“ – ungenau")}
        </g>),
    );
    fframes::svgr!(<g>{coded}{strike_bar}{belegt}{axis}{bar}{extub}{thresh}{coded_mark}{after}</g>)
}
