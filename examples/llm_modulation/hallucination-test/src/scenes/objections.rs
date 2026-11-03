//! 5 · Potential objections and responses, in one place: a table that
//! builds row by row, the objection on the left with the section it belongs
//! to, the response on the right. Each row lights as it appears and stays
//! lit, so at the end all six stand together.

use fframes::{FFramesContext, Frame, Svgr};

use crate::poster::*;
use crate::ui::*;

script_scene!(ObjectionsScene, 120.5, 172.5);

/// (section, objection, response)
const ROWS: [(&str, &str, &str); 6] = [
    (
        "THE QUERY",
        "Same query?",
        "Yes. In both cases the same query string was entered. Modulation is the only independent variable.",
    ),
    (
        "DEFINITION AND TEST",
        "Subjective?",
        "The criteria are fixed and applied to every literal. Each classification cites a line number and is independently verifiable.",
    ),
    (
        "RESULT",
        "Defaults?",
        "Program B (grok-4.7) asserts each value as fact and uses it to command robot motion.",
    ),
    (
        "ROBOTIC CODE EXAMPLES",
        "Refusal?",
        "No. Program A (grok-4.7+modulation) issues the same moveJ command; it takes the address and pose as validated inputs rather than fabricating them.",
    ),
    (
        "RESULT",
        "Sampling variance?",
        "Sampling variance can omit a literal. It does not produce the input validation that Program A (grok-4.7+modulation) implements and documents.",
    ),
    (
        "CONCLUSION",
        "Undisclosed modulation?",
        "No. Both programs are shown in full, and both calls are preserved with their SHA-256 hashes.",
    ),
];

const FIRST: f32 = 1.5;
/// Seconds per row: the objection alone, then the response long enough to read.
const STEP: f32 = 7.8;
const ANSWER_AFTER: f32 = 1.8;

const TOP: f32 = 190.0;
const ROW_H: f32 = 120.0;
const Q_W: f32 = 600.0;
const A_X: f32 = 780.0;
const A_W: f32 = 1020.0;

impl ObjectionsScene {
    fn draw(&self, frame: &mut Frame, ctx: &FFramesContext, t: f32) -> Svgr<'static> {
        let s = window(t, 0.0, Self::LEN, 0.7);
        let content = if s > 0.0 { self.content(frame, ctx, t) } else { Svgr::empty() };
        fframes::svgr!(
            <g>
                {stage(1.0)}
                {header(5, "The Potential Objections And Responses", s)}
                <g opacity={s}>{content}</g>
            </g>
        )
    }

    fn content(&self, frame: &mut Frame, ctx: &FFramesContext, t: f32) -> Svgr<'static> {
        let heads = ease_out(prog(t, 0.4, 1.0));
        let mut out = vec![fframes::svgr!(
            <g opacity={heads}>
                {label(MX + 24.0, TOP - 14.0, "POTENTIAL OBJECTION", GREY, 18.0, "start")}
                {label(A_X, TOP - 14.0, "RESPONSE", GREY, 18.0, "start")}
            </g>
        )];
        let q_ty = Type::new(SERIF, 30, 400, INK).italic().width(Q_W).line_h(36.0);
        let a_ty = Type::new(SERIF, 28, 400, INK).width(A_W).line_h(36.0);
        for (i, (section, question, answer)) in ROWS.iter().enumerate() {
            let at = FIRST + i as f32 * STEP;
            if t < at {
                break;
            }
            let y0 = TOP + i as f32 * ROW_H;
            let enter = ease_out(prog(t, at, at + 0.6));
            // lit as it appears, and it stays lit
            let panel = ease_out(prog(t, at, at + 0.5));
            let tag = format!("{:02}  {section}", i + 1);
            out.push(fframes::svgr!(
                <g opacity={enter}>
                    <rect x="100" y={y0} width="1720" height={ROW_H - 8.0} rx="6" fill={SHEET} opacity={panel} />
                    <rect x="100" y={y0} width="4" height={ROW_H - 8.0} fill={ACCENT} opacity={panel} />
                    {label(MX + 24.0, y0 + 28.0, &tag, ACCENT, 16.0, "start")}
                    {text_lines(frame, ctx, MX + 24.0, y0 + 64.0, question, q_ty, at, t)}
                    {text_lines(frame, ctx, A_X, y0 + 44.0, answer, a_ty, at + ANSWER_AFTER, t)}
                    <rect x="100" y={y0 + ROW_H - 4.0} width="1720" height="1" fill={RULE} />
                </g>
            ));
        }
        fframes::svgr!(<g>{out}</g>)
    }
}
