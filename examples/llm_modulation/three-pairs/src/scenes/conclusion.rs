//! 5 · Conclusion: the manuscript's two closing paragraphs, the claim in its
//! panel beneath; then the last page, the addendum: proof by challenge.
//! Both complete from their first frame.

use fframes::{FFramesContext, Frame, Svgr};

use crate::page::*;
use crate::ui::*;

script_scene!(ConclusionScene, 105.0, 115.0);
script_scene!(ChallengeScene, 115.0, 122.0);

/// Set on two lines on purpose: one model per line.
const CLOSING: &str = "grok-4.7: unsupported statements in every domain | grok-4.7+modulation: none, and names what it cannot establish";
const CLAIM: &str = "We claim that hallucination is a controllable behaviour, not a fixed limit of the model.";

const CHALLENGE: &str = "We challenge readers to refute the claim using the queries and verification criterion stated above.";

impl ConclusionScene {
    fn draw(&self, frame: &mut Frame, ctx: &FFramesContext, _t: f32) -> Svgr<'static> {
        let ty = Type::new(SERIF, 44, 700, INK).width(1480.0).line_h(62.0).center();
        let closing = block(frame, ctx, W / 2.0, 398.0, CLOSING, ty);
        let claim = block(frame, ctx, W / 2.0, 728.0, CLAIM, ty);
        fframes::svgr!(
            <g>
                {stage()}
                {header(7, "Conclusion", 1.0)}
                <rect x="160" y="300" width="1600" height="230" rx="10" fill={SHEET} stroke={ACCENT} stroke-width="2.5" />
                <rect x="160" y="300" width="8" height="230" rx="4" fill={ACCENT} />
                {closing}
                <rect x="160" y="600" width="1600" height="230" rx="10" fill={SHEET} stroke={FLAG} stroke-width="2.5" />
                <rect x="160" y="600" width="8" height="230" rx="4" fill={FLAG} />
                {claim}
            </g>
        )
    }
}

impl ChallengeScene {
    fn draw(&self, frame: &mut Frame, ctx: &FFramesContext, _t: f32) -> Svgr<'static> {
        let body = block(frame, ctx, W / 2.0, 560.0, CHALLENGE, Type::new(SERIF, 46, 700, INK).width(1480.0).line_h(62.0).center());
        fframes::svgr!(
            <g>
                {stage()}
                {header(7, "Addendum \u{2014} Proof by Challenge", 1.0)}
                <rect x="160" y="430" width="1600" height="260" rx="10" fill={SHEET} stroke={ACCENT} stroke-width="2.5" />
                <rect x="160" y="430" width="8" height="260" rx="4" fill={ACCENT} />
                {body}
            </g>
        )
    }
}
