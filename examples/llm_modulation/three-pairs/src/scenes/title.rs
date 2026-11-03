//! Title page: the manuscript's title, author and date, complete from the
//! first frame and held; then a cut to the setup.

use fframes::{FFramesContext, Frame, Svgr};

use crate::page::*;
use crate::ui::*;

script_scene!(TitleScene, 0.0, 5.0);

/// Set on two lines on purpose, at the colon.
const TITLE: &str = "Controlling Hallucination Without Changing the Model: | Paired Tests in Documents, Mathematics, and Robot Code";
const AUTHOR: &str = "Eckhart Diestel, Dr. univ. Gent, MD";
const DATE: &str = "AMDG Engineering · Manuscript dated November 3, 2026";

impl TitleScene {
    fn draw(&self, frame: &mut Frame, ctx: &FFramesContext, _t: f32) -> Svgr<'static> {
        let ty = Type::new(SERIF, 60, 700, ACCENT).width(1700.0).line_h(80.0).center();
        let title = block(frame, ctx, W / 2.0, 400.0, TITLE, ty);
        fframes::svgr!(
            <g>
                {stage()}
                {title}
                <text x={W / 2.0} y="610" text-anchor="middle" font-family={SERIF} font-size="38" fill={INK}>{AUTHOR}</text>
                <text x={W / 2.0} y="662" text-anchor="middle" font-family={SERIF} font-size="30" font-style="italic" fill={GREY}>{DATE}</text>
            </g>
        )
    }
}
