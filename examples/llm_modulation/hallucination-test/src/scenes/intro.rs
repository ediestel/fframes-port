//! Title slide. The poster's title, author and date on the stage, complete
//! from the first frame and held; then a cut to the poster.

use fframes::{FFramesContext, Frame, Svgr};

use crate::poster::*;
use crate::ui::*;

script_scene!(IntroScene, 0.0, 2.5);

/// A time after every build-up of the slide: it is drawn as it stands then.
const BUILT: f32 = 99.0;

/// Set on two lines on purpose, so the break does not fall on its own.
const TITLE: &str = "A Test for Hallucination | in Robot Code Generation";
const AUTHOR: &str = "Eckhart Diestel, MD";
const DATE: &str = "Manuscript of October 11, 2026";

impl IntroScene {
    fn draw(&self, frame: &mut Frame, ctx: &FFramesContext, _t: f32) -> Svgr<'static> {
        let t = BUILT;
        let out = 1.0;
        let ty = Type::new(SERIF, 66, 700, ACCENT).width(1600.0).line_h(84.0).center();
        let author = 1.0;
        let date = 1.0;
        fframes::svgr!(
            <g>
                {stage(1.0)}
                <g opacity={out}>
                    {text_lines(frame, ctx, W / 2.0, 410.0, TITLE, ty, 0.1, t)}
                    <text x={W / 2.0} y={610.0 + (1.0 - author) * 12.0} text-anchor="middle" font-family={SERIF} font-size="38"
                          fill={INK} opacity={author}>{AUTHOR}</text>
                    <text x={W / 2.0} y={662.0 + (1.0 - date) * 12.0} text-anchor="middle" font-family={SERIF} font-size="30"
                          font-style="italic" fill={GREY} opacity={date}>{DATE}</text>
                    <text x={W / 2.0} y={712.0 + (1.0 - date) * 12.0} text-anchor="middle" font-family={SERIF} font-size="26"
                          fill={GREY} opacity={date}>"Built with fframes-port, based on dmtrKovalenko/fframes"</text>
                </g>
            </g>
        )
    }
}
