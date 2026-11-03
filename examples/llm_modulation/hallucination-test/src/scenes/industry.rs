//! 4 · The conclusion, and the film's last page: the closing sentence in its
//! panel, the one implication beneath it, both complete from the first frame.

use fframes::{FFramesContext, Frame, Svgr};

use crate::poster::*;
use crate::ui::*;

script_scene!(IndustryScene, 43.0, 49.0);

/// A time after every build-up of the page: it is drawn as it stands then.
const BUILT: f32 = 99.0;

/// Set on two lines on purpose: one model per line.
const CLOSING: &str = "grok-4.7: hallucinations | grok-4.7+modulation: no hallucinations";
const CLOSING_AT: f32 = 0.0;
const CLAIM: &str = "We claim that hallucination is a controllable behaviour, not a fixed limit of the model.";
const CLAIM_AT: f32 = 0.0;

/// The two panels: top y and height.
const PANEL_H: f32 = 230.0;
const CLOSING_Y: f32 = 300.0;
const CLAIM_Y: f32 = 600.0;

impl IndustryScene {
    fn draw(&self, frame: &mut Frame, ctx: &FFramesContext, _t: f32) -> Svgr<'static> {
        let t = BUILT;
        let c = 1.0;
        let ty = Type::new(SERIF, 46, 700, INK).width(1480.0).line_h(62.0).center();
        let mut out = Vec::new();
        for (text, at, top, stroke) in [(CLOSING, CLOSING_AT, CLOSING_Y, ACCENT), (CLAIM, CLAIM_AT, CLAIM_Y, FLAG)] {
            let p = ease_out(prog(t, at, at + 0.8));
            let lift = (1.0 - p) * 20.0;
            out.push(fframes::svgr!(
                <g opacity={p} transform={format!("translate(0 {lift})")}>
                    <rect x="160" y={top} width="1600" height={PANEL_H} rx="10" fill={SHEET} stroke={stroke} stroke-width="2.5" />
                    <rect x="160" y={top} width="8" height={PANEL_H} rx="4" fill={stroke} />
                </g>
            ));
            out.push(fframes::svgr!(
                <g transform={format!("translate(0 {lift})")}>
                    {text_lines(frame, ctx, W / 2.0, top + 98.0, text, ty, at + 0.3, t)}
                </g>
            ));
        }
        fframes::svgr!(
            <g>
                {stage(1.0)}
                {header(4, "The Conclusion", c)}
                <g opacity={c}>{out}</g>
            </g>
        )
    }
}
