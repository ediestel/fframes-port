//! One file per page, in the order of the paper: title, the setup, the three
//! pairs (each: what was asked and answered, then the check), the
//! conclusion and the challenge. Every page has its start and end in film
//! seconds; inside a page `t` is seconds since the page appeared. Pages cut
//! from one to the next, each paced to be read.

/// A page struct with its place in the film. The page implements
/// `fn draw(&self, frame, ctx, t) -> Svgr`.
macro_rules! script_scene {
    ($name:ident, $start:expr, $end:expr) => {
        #[derive(Debug)]
        pub struct $name;
        impl $name {
            pub const START: f32 = $start;
            pub const END: f32 = $end;
            pub const LEN: f32 = $end - $start;
        }
        impl fframes::Scene for $name {
            fn duration(&self) -> fframes::Duration<'_> {
                fframes::Duration::Frames(((Self::END - Self::START) * crate::FPS as f32).round() as usize)
            }
            fn name(&self) -> &'static str {
                stringify!($name)
            }
            fn render_frame<'a>(
                &'a self,
                mut frame: fframes::Frame,
                ctx: &fframes::FFramesContext<'a, '_>,
            ) -> fframes::Svgr<'a> {
                let t = crate::gsec(&frame) - Self::START;
                self.draw(&mut frame, ctx, t)
            }
        }
    };
}

pub mod conclusion;
pub mod pairs;
pub mod results;
pub mod setup;
pub mod title;

pub use conclusion::{ChallengeScene, ConclusionScene};
pub use pairs::{DocumentsScene, MathScene, RobotScene};
pub use results::ResultsScene;
pub use setup::SetupScene;
pub use title::TitleScene;

/// Film seconds at which an amber underline lands: each gets the soft tone.
pub fn tones() -> Vec<f32> {
    use crate::page::{FLAG_AT, FLAG_DUR, FLAG_STEP};
    let mut out = Vec::new();
    for (start, n) in [
        (DocumentsScene::START, pairs::flags(pairs::DOC_ROWS)),
        (MathScene::START, pairs::flags(pairs::MATH_ROWS)),
        (RobotScene::START, pairs::flags(pairs::ROBOT_ROWS)),
    ] {
        for k in 0..n {
            out.push(start + FLAG_AT + FLAG_DUR + k as f32 * FLAG_STEP);
        }
    }
    out
}
