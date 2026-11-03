//! 5 · Results: the three checks in one table. Complete from its first frame.

use fframes::{FFramesContext, Frame, Svgr};

use crate::page::*;
use crate::ui::*;

script_scene!(ResultsScene, 81.0, 93.0);

impl ResultsScene {
    fn draw(&self, frame: &mut Frame, ctx: &FFramesContext, _t: f32) -> Svgr<'static> {
        let (tab, _) = table(
            frame,
            ctx,
            220.0,
            "Each response checked afterwards against its source",
            &[
                ("Documents: statements the documents do not support", "2", "0"),
                ("Mathematics: statements the mathematics does not support", "1", "0"),
                ("Robot code: API errors against bosdyn-client 5.2.0", "3", "0"),
                ("Of these, named in the response", "0", "\u{2014}"),
                ("Robot code passes the package checks", "no", "yes"),
                ("Requested components present", "yes", "yes"),
            ],
        );
        fframes::svgr!(<g>{stage()}{header(5, "Results", 1.0)}{tab}</g>)
    }
}
