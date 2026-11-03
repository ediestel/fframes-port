//! One file per scene, in poster order after the title slide. Every scene
//! has its start and end in film seconds; inside a scene `t` is seconds since
//! the scene start. Between the push into the poster and the pull back out,
//! every scene fades its content in and out on the same stage, so the scenes
//! join on an empty page.

/// A scene struct with its place in the script. The scene implements
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

pub mod industry;
pub mod intro;
pub mod query;
pub mod result;
pub mod test;

pub use industry::IndustryScene;
pub use intro::IntroScene;
pub use query::QueryScene;
pub use result::ResultScene;
pub use test::TestScene;

/// Film seconds at which a hallucinated literal is underlined: each gets the soft tone.
pub fn tones() -> Vec<f32> {
    let mut out = Vec::new();
    for (start, times) in [
        (TestScene::START, test::tones()),
        (ResultScene::START, result::tones()),
    ] {
        out.extend(times.into_iter().map(|t| start + t));
    }
    out
}
