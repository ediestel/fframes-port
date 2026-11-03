//! Szene 8 · 03:50–04:00 · Abschluss.

use fframes::{FFramesContext, Frame, Svgr};

use crate::ui::*;

pub fn draw<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let c1 = enter(t, 0.3, 0.8);
    let rule = enter(t, 0.7, 0.9);
    let pre = enter(t, 0.1, 0.7);
    fframes::svgr!(<g>
        <g opacity={pre} transform={tr(0.0, (1.0 - pre) * 16.0)}>
            <text x="960" y="290" font-family={SANS} font-size="40" font-weight="400" fill={MUTED} text-anchor="middle">"Präsentiert von"</text>
        </g>
        <g opacity={c1} transform={tr(0.0, (1.0 - c1) * 26.0)}>
            <text x="960" y="420" font-family={SANS} font-size="128" font-weight="500" fill={INK} text-anchor="middle" letter-spacing="-3">"AMDG Engineers"</text>
        </g>
        <rect x={960.0 - 110.0 * rule} y="470" width={(220.0 * rule).max(0.5)} height="4" rx="2" fill="url(#amberBar)" opacity={rule} />
    </g>)
}
