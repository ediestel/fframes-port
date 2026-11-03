//! Szene 10 · Abschluss: der Schlusssatz des Exposés, dann "Präsentiert von
//! AMDG Engineers".

use fframes::{FFramesContext, Frame, Svgr};

use crate::ui::*;

pub fn draw<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let lines = [
        ("Quellengestützte Prüfung.", INK, 0.3),
        ("Nachvollziehbare Hinweise.", INK, 1.0),
        ("Mehr Zeit für die Kodierentscheidung.", AMBER, 1.7),
    ];
    let mut out = Vec::new();
    for (i, (s, color, at)) in lines.iter().enumerate() {
        let k = enter(t, *at, 0.8);
        let y = 380.0 + i as f32 * 96.0;
        out.push(fframes::svgr!(
            <g opacity={k} transform={tr(0.0, (1.0 - k) * 24.0)}>{text(960.0, y, 68.0, 500, color, "middle", s)}</g>
        ));
    }
    let pre = enter(t, 3.2, 0.8);
    let rule = enter(t, 3.0, 0.9);
    fframes::svgr!(<g>
        {out}
        <rect x={960.0 - 110.0 * rule} y="660" width={(220.0 * rule).max(0.5)} height="4" rx="2" fill="url(#amberBar)" opacity={rule} />
        <g opacity={pre} transform={tr(0.0, (1.0 - pre) * 16.0)}>
            <text x="960" y="740" font-family={SANS} font-size="32" font-weight="400" fill={MUTED} text-anchor="middle">"Präsentiert von "<tspan font-weight="500" fill={INK}>"AMDG Engineers"</tspan></text>
        </g>
    </g>)
}
