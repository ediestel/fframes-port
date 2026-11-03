//! 2–4 · The three pairs, one poster each: a row per query with what was
//! planted or asked, grok-4.7's answer and grok-4.7+modulation's in a few
//! words, every statement the source does not support underlined in amber
//! (with the tone), every point the modulated answer names as open ticked;
//! the count beneath. The full texts are in the manuscript.

use fframes::{FFramesContext, Frame, Svgr};

use crate::page::*;
use crate::ui::*;

script_scene!(DocumentsScene, 19.0, 41.0);
script_scene!(MathScene, 41.0, 59.0);
script_scene!(RobotScene, 59.0, 81.0);

const DOCUMENTS: &[Row] = &[
    Row {
        what: "Site report",
        note: "planted: two inspection dates, 3 and 5 March",
        direct: &[p("Finds the conflicting dates."), pf("Adds: \u{201C}not the inspection report itself\u{201D}", "\u{201C}not the inspection report itself\u{201D}")],
        modulated: &[p("Finds the conflicting dates. Does not establish:"), po("one inspection date · that the text is Moser\u{2019}s report · a release date · the roof beyond Moser\u{2019}s finding")],
    },
    Row {
        what: "Lease summary",
        note: "planted: none",
        direct: &[pf("Adds: \u{201C}residential\u{201D}", "\u{201C}residential\u{201D}")],
        modulated: &[p("Supported deductions only. Does not establish:"), po("the premises · the parties · city · end date · signature · governing law · repair scope · enforceability")],
    },
];

const MATH: &[Row] = &[
    Row {
        what: "Eigenvector",
        note: "asked: a real 3 × 3 matrix with exactly one eigenvector",
        direct: &[
            p("Answers that such a matrix cannot exist, because every multiple of an eigenvector is again an eigenvector."),
            p("Then answers a different question: a matrix whose eigenvectors all lie on one line (a Jordan block)."),
        ],
        modulated: &[p("Answers that such a matrix cannot exist: every real 3 × 3 matrix has an eigenvector, and each of its infinitely many multiples is one too.")],
    },
    Row {
        what: "Integral",
        note: "asked: the integral from 0 to ∞ of sin(x²)/x dx",
        direct: &[p("π/4."), pf("Adds: sin(x²)/x = ∫ from 0 to x of cos(t²) dt", "sin(x²)/x = ∫ from 0 to x of cos(t²) dt")],
        modulated: &[p("π/4, in 12 steps from the least-upper-bound property.")],
    },
];

const ROBOT: &[Row] = &[Row {
    what: "Spot robot",
    note: "bosdyn-client 5.2.0: stand, 1.0 m forward, dwell, sit, power off; no library given",
    direct: &[
        pf("start_time_sync() returns nothing: the script aborts at startup.", "start_time_sync()"),
        pf("SE2TrajectoryCommand is read from the wrong module.", "SE2TrajectoryCommand"),
        pf("STATUS_ACHIEVED does not exist in 5.2.0.", "STATUS_ACHIEVED"),
        p("None of the three is mentioned."),
    ],
    modulated: &[
        p("Every reference exists in 5.2.0. Does not establish:"),
        po("the 5.2.0 API itself · a numeric E-stop test · how partial credentials combine · tolerance between 0.1 s samples · safety after a second interrupt"),
    ],
}];

/// Underlines per page, for the tones.
pub fn flags(rows: &[Row]) -> usize {
    rows.iter().map(|r| flag_count(r.direct)).sum()
}
pub const DOC_ROWS: &[Row] = DOCUMENTS;
pub const MATH_ROWS: &[Row] = MATH;
pub const ROBOT_ROWS: &[Row] = ROBOT;

impl DocumentsScene {
    fn draw(&self, frame: &mut Frame, ctx: &FFramesContext, t: f32) -> Svgr<'static> {
        let g = grid(frame, ctx, t, DOCUMENTS, ("2", "0"), "statements the documents do not support");
        fframes::svgr!(<g>{stage()}{header(2, "Documents", 1.0)}{g}</g>)
    }
}

impl MathScene {
    fn draw(&self, frame: &mut Frame, ctx: &FFramesContext, t: f32) -> Svgr<'static> {
        let g = grid(frame, ctx, t, MATH, ("1", "0"), "statements the mathematics does not support");
        fframes::svgr!(<g>{stage()}{header(3, "Mathematics", 1.0)}{g}</g>)
    }
}

impl RobotScene {
    fn draw(&self, frame: &mut Frame, ctx: &FFramesContext, t: f32) -> Svgr<'static> {
        let g = grid(frame, ctx, t, ROBOT, ("3", "0"), "API errors, against the 5.2.0 package");
        fframes::svgr!(<g>{stage()}{header(4, "Robot Code", 1.0)}{g}</g>)
    }
}
