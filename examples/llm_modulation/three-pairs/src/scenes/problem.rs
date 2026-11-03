//! 1 · The Problem: hallucination. What it is, one example from the study
//! with its underline (and the tone), and the question the research asks.
//! Complete from its first frame apart from the underline.

use fframes::{FFramesContext, Frame, Svgr};

use crate::page::*;
use crate::qr::{URL, qr};
use crate::ui::*;

/// Pixels per QR module: 41 modules with the quiet zone, 246 px.
const QR_M: f32 = 6.0;

script_scene!(ProblemScene, 5.0, 17.0);

const WHAT: &str = "A model adds statements its source does not support, | and presents them as established.";
const EXAMPLE: &[Entry] = &[pf(
    "\u{201C}The document establishes a five-year residential lease \u{2026}\u{201D}",
    "residential",
)];
const EXAMPLE_NOTE: &str = "grok-4.7, on a lease summary that nowhere says \u{201C}residential\u{201D}";
const READER: &str = "Without checking every statement, the reader cannot tell which ones they are.";
const QUESTION: &str = "Can hallucination be controlled without changing the model?";

/// Underlines on this page, for the tones.
pub const FLAGS: usize = 1;

impl ProblemScene {
    fn draw(&self, frame: &mut Frame, ctx: &FFramesContext, t: f32) -> Svgr<'static> {
        let what = block(frame, ctx, W / 2.0, 270.0, WHAT, Type::new(SERIF, 46, 700, INK).width(1600.0).line_h(62.0).center());
        let example = entries(frame, ctx, 260.0, 500.0, 1400.0, EXAMPLE, 34, t);
        let note = EXAMPLE_NOTE.to_owned();
        // the text on the left, the code to the paper on the right
        let reader = block(frame, ctx, MX, 720.0, READER, Type::new(SERIF, 32, 400, GREY).width(1260.0).line_h(42.0).italic());
        let question = block(frame, ctx, MX, 880.0, QUESTION, Type::new(SERIF, 44, 700, ACCENT).width(1260.0).line_h(60.0));
        let side = 41.0 * QR_M;
        let qx = W - MX - side;
        let qy = 640.0;
        let url = URL.trim_start_matches("https://").to_owned();
        fframes::svgr!(
            <g>
                {stage()}
                {header(1, "The Problem: Hallucination", 1.0)}
                {what}
                <rect x="200" y="420" width="1520" height="170" rx="6" fill={SHEET} stroke={RULE} stroke-width="1.5" />
                <rect x="200" y="420" width="5" height="170" fill={FLAG} />
                {example}
                <text x="260" y="560" font-family={SERIF} font-size="24" font-style="italic" fill={GREY}>{note}</text>
                {reader}
                {question}
                {qr(qx, qy, QR_M)}
                <text x={W - MX} y={qy + side + 34.0} text-anchor="end" font-family={SERIF} font-size="22" font-style="italic" fill={GREY}>"Paper, queries and responses"</text>
                <text x={W - MX} y={qy + side + 62.0} text-anchor="end" font-family={MONO} font-size="16" fill={ACCENT}>{url}</text>
            </g>
        )
    }
}
