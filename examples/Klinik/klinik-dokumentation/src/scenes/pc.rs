//! Szene 11 · 06:00–06:31 · Einrichtung und Betrieb. Die Einrichtung ist
//! überschaubar (Ordner oder Server festlegen, die Verbindung einrichten,
//! Prompts und Vorlagen abstimmen), AMDG Engineers macht das mit der IT des
//! Hauses; der laufende Betrieb wird nicht unterbrochen, die vorhandenen
//! Programme bleiben; fällt der Dienst aus, arbeiten Klinik und Praxis weiter,
//! die Patientenakte hängt nicht von ihm ab.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::ui::*;

/// The setup steps: (x, label, sub, time).
const STEPS: [(f32, &str, &str, f32); 3] = [
    (700.0, "Ordner oder Server festlegen", "wie es im Haus passt", 3.2),
    (1140.0, "Verbindung einrichten", "verschlüsselt, nach außen", 5.2),
    (1580.0, "Prompts und Vorlagen abstimmen", "für die eigenen Dokumente", 7.1),
];

/// The programs that stay as they are: (x, label).
const PROGRAMS: [(f32, &str); 3] = [(170.0, "Praxissoftware / KIS"), (560.0, "Patientenakte"), (950.0, "Import")];

/// A heartbeat trace that scrolls left.
fn heartbeat(x: f32, y: f32, w: f32, t: f32, color: &str) -> Svgr<'static> {
    let period = 180.0;
    let shift = (t * 120.0) % period;
    let mut d = format!("M{x:.1} {y:.1}");
    let mut px = x - shift;
    while px < x + w {
        let beat = [(0.0, 0.0), (60.0, 0.0), (72.0, -6.0), (84.0, 0.0), (96.0, 14.0), (108.0, -26.0), (120.0, 8.0), (132.0, 0.0)];
        for (dx, dy) in beat {
            let bx = px + dx;
            if bx >= x && bx <= x + w {
                d.push_str(&format!(" L{bx:.1} {:.1}", y + dy));
            }
        }
        px += period;
    }
    d.push_str(&format!(" L{:.1} {y:.1}", x + w));
    let color = color.to_owned();
    fframes::svgr!(<path d={d} fill="none" stroke={color} stroke-width="3" stroke-linejoin="round" stroke-linecap="round" />)
}

pub fn draw<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    // a small setup, done by AMDG Engineers together with the IT of the house
    let mut steps = Vec::new();
    let line_x2 = lerp(STEPS[0].0, STEPS[2].0, ease_in_out(prog(t, 3.2, 7.6)));
    steps.push(show(
        enter(t, 3.0, 0.4),
        0.0,
        fframes::svgr!(<g>
            <rect x={STEPS[0].0} y="263" width={STEPS[2].0 - STEPS[0].0} height="4" rx="2" fill={LINE} />
            <rect x={STEPS[0].0} y="263" width={(line_x2 - STEPS[0].0).max(0.5)} height="4" rx="2" fill={GREEN} />
        </g>),
    ));
    for (i, (x, label, sub, at)) in STEPS.iter().enumerate() {
        let n = format!("{}", i + 1);
        steps.push(rise_in(
            t,
            *at,
            fframes::svgr!(<g>
                <circle cx={*x} cy="265" r="26" fill={PANEL} stroke={GREEN} stroke-width="3" />
                {mono(*x, 273.0, 22.0, GREEN, "middle", &n)}
                {check(*x + 22.0, 243.0, 12.0, prog(t, at + 0.5, at + 1.1), GREEN)}
                {text(*x, 334.0, 23.0, 500, INK, "middle", label)}
                {text(*x, 366.0, 19.0, 400, MUTED, "middle", sub)}
            </g>),
        ));
    }
    let small = rise_in(t, 1.0, text(130.0, 236.0, 22.0, 500, GREEN, "start", "Einrichtung: überschaubar"));

    // operation: nothing is interrupted, the existing programs stay
    let off = prog(t, 21.4, 22.0) * (1.0 - prog(t, 28.4, 29.0));
    let strip = rise_in(
        t,
        13.8,
        fframes::svgr!(<g>
            {card(130.0, 440.0, 1660.0, 230.0, Some(GREEN), PANEL, LINE)}
            {text(170.0, 482.0, 22.0, 500, GREEN, "start", "Laufender Betrieb · ohne Unterbrechung")}
            {heartbeat(170.0, 640.0, 1100.0, t, GREEN)}
        </g>),
    );
    let mut programs = Vec::new();
    for (i, (x, label)) in PROGRAMS.iter().enumerate() {
        let at = 17.0 + i as f32 * 0.4;
        let indep = if i == 1 { prog(t, 25.6, 26.3) } else { 0.0 };
        let stroke = mix(LINE, GREEN, indep);
        programs.push(rise_in(
            t,
            at,
            fframes::svgr!(<g>
                <rect x={*x} y="504" width="350" height="62" rx="12" fill={BG} stroke={stroke} stroke-width="2" />
                {text(*x + 20.0, 543.0, 21.0, 500, INK, "start", label)}
                {badge(*x + 220.0, 535.0, "BLEIBT", MUTED, PANEL, 13.0)}
            </g>),
        ));
    }
    let indep = rise_in(
        t,
        25.6,
        fframes::svgr!(<g>
            {check(574.0, 504.0, 14.0, prog(t, 25.8, 26.5), GREEN)}
            {text(560.0, 596.0, 18.0, 500, GREEN, "start", "unabhängig vom Dienst")}
        </g>),
    );
    let service = rise_in(
        t,
        14.4,
        fframes::svgr!(<g>
            {server(1420.0, 460.0, 340.0, 180.0, t, 1.0 - off, "")}
            <g opacity={1.0 - off}>{text(1590.0, 620.0, 22.0, 500, INK, "middle", "Dienst")}</g>
            <g opacity={off}>{text(1590.0, 620.0, 22.0, 500, RED, "middle", "Dienst aus")}</g>
            {cross(1736.0, 466.0, 18.0, off, RED)}
        </g>),
    );
    let goes_on = show(
        window(t, 23.0, 29.0, 0.4),
        0.0,
        text(1270.0, 482.0, 22.0, 500, INK, "end", "Klinik und Praxis arbeiten weiter"),
    );

    fframes::svgr!(<g>{small}{steps}{strip}{programs}{indep}{service}{goes_on}</g>)
}
