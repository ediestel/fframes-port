//! Scene 8 · 03:50–04:00 · Closing card.

use fframes::{FFramesContext, Frame, Svgr};

use crate::ui::*;

pub fn draw<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let c1 = enter(t, 0.3, 0.8);
    let c2 = enter(t, 0.9, 0.8);
    let c3 = enter(t, 1.5, 0.8);
    let rule = enter(t, 0.7, 0.9);
    fframes::svgr!(<g>
        <g opacity={c1} transform={tr(0.0, (1.0 - c1) * 26.0)}>
            <text x="960" y="420" font-family={SANS} font-size="128" font-weight="500" fill={INK} text-anchor="middle" letter-spacing="-3">"AMDG Engineers"</text>
        </g>
        <rect x={960.0 - 110.0 * rule} y="470" width={(220.0 * rule).max(0.5)} height="4" rx="2" fill="url(#amberBar)" opacity={rule} />
        <g opacity={c2} transform={tr(0.0, (1.0 - c2) * 20.0)}>
            <text x="960" y="560" font-family={SANS} font-size="50" font-weight="400" fill={INK} text-anchor="middle">"Prompt-based room management with an LLM"</text>
        </g>
        <g opacity={c3} transform={tr(0.0, (1.0 - c3) * 16.0)}>
            <text x="960" y="640" font-family={SANS} font-size="34" font-weight="400" fill={MUTED} text-anchor="middle">"Video and service offered by AMDG Engineers"</text>
        </g>
    </g>)
}
