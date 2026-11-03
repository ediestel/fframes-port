//! 6 · Conclusion: what the difference between the two programs means,
//! built as four statements that appear one after another and stay. The
//! last one, the finding, is set in the accent. The film ends on it.

use fframes::{FFramesContext, Frame, Svgr};

use crate::poster::*;
use crate::ui::*;

script_scene!(ConclusionScene, 165.5, 230.0);

/// The six statements give way to the significance, alone and large.
const LIST_OUT: (f32, f32) = (50.0, 50.8);
const FINAL_AT: f32 = 51.2;
const SIGNIFICANCE: &str = "So the difference is epistemic. In robot code that isn\u{2019}t a style difference, it\u{2019}s a safety property.";

/// (statement, when it appears), each held long enough to be read before
/// the next one starts. The models are always named, never A or B.
const STATEMENTS: [(&str, f32); 6] = [
    ("grok-4.7 and grok-4.7+modulation received the same query.", 1.0),
    (
        "grok-4.7 filled every gap in it with a confident guess \u{2014} the controller address, a home pose it declares safe, a tolerance check nobody requested \u{2014} and turned those guesses into motion commands.",
        5.0,
    ),
    (
        "grok-4.7+modulation filled none: it took from the query and the library only what they actually provide, made the operator supply what only the operator can know, validated it, refused to move without it, and said why.",
        15.0,
    ),
    (
        "So the difference is epistemic. grok-4.7 can\u{2019}t tell what it knows from what it\u{2019}s guessing; grok-4.7+modulation can, and its code shows it line by line.",
        25.0,
    ),
    (
        "In robot code that isn\u{2019}t a style difference, it\u{2019}s a safety property: grok-4.7\u{2019}s guesses run on a physical arm, they pass a compiler, a linter and a mocked test, and only a test that asks of every literal \u{201C}does the query justify this?\u{201D} catches them.",
        33.0,
    ),
    ("The modulation changes exactly that behaviour without changing the model.", 44.0),
];

const SIZE: usize = 32;
const LINE: f32 = 44.0;
const GAP: f32 = 24.0;
const WIDTH: f32 = 1680.0;

impl ConclusionScene {
    fn draw(&self, frame: &mut Frame, ctx: &FFramesContext, t: f32) -> Svgr<'static> {
        let c = prog(t, 0.0, 0.7) * (1.0 - prog(t, LIST_OUT.0, LIST_OUT.1));
        // the significance, alone and large, to the end
        let f = ease_out(prog(t, FINAL_AT, FINAL_AT + 1.0)) * (1.0 - prog(t, Self::LEN - 1.2, Self::LEN - 0.2));
        let big = Type::new(SERIF, 64, 700, ACCENT).width(1600.0).line_h(86.0).center();
        let significance = text_lines(frame, ctx, W / 2.0, 380.0, SIGNIFICANCE, big, FINAL_AT, t);
        let mut out = Vec::new();
        let mut y = 230.0;
        for (i, (text, at)) in STATEMENTS.iter().enumerate() {
            let last = i + 1 == STATEMENTS.len();
            let ty = if last {
                Type::new(SERIF, SIZE, 700, ACCENT)
            } else {
                Type::new(SERIF, SIZE, 400, INK)
            }
            .width(WIDTH)
            .line_h(LINE);
            out.push(text_lines(frame, ctx, MX, y, text, ty, *at, t));
            let lines = line_widths(&layout(frame, ctx, text, SERIF, SIZE, ty.weight, false, WIDTH)).len();
            y += lines as f32 * LINE + GAP;
        }
        fframes::svgr!(
            <g>
                {stage(1.0)}
                {header(6, "The Conclusion", c)}
                <g opacity={c}>{out}</g>
                <g opacity={f}>{significance}</g>
            </g>
        )
    }
}
