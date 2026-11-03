//! Figures for the GP books, drawn with the rig in `rig.rs` in the Thieme
//! Taschenatlas style of `GP/image_prompts/style.txt`. One scene serves two
//! outputs: the 5-7 s clinical short (`Short`) and the 2:1 book plate
//! (`Plate`, one frame; `gp-figures --plate frame 0`).

pub mod dix_hallpike;
pub mod puppet;
pub mod rig;

use fframes::{AudioMap, Color, Duration, FFramesContext, Frame, Scene, Scenes, Svgr, Video};

use dix_hallpike::{PAPER, State};
use rig::{INK, View};

pub const FPS: usize = 30;
pub const WIDTH: usize = 1920;
pub const HEIGHT: usize = 960;

const SHORT_VIEW: View = View { ox: 800.0, oy: 925.0, s: 430.0, stroke: 3.0 };

#[derive(Debug)]
pub struct DixHallpike;

impl Scene for DixHallpike {
    fn duration(&self) -> Duration<'_> {
        Duration::Frames((dix_hallpike::LEN * FPS as f32) as usize)
    }
    fn name(&self) -> &'static str {
        "DixHallpike"
    }
    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        dix_hallpike::scene(&SHORT_VIEW, &State::at(frame.seconds()))
    }
}

/// Step 1 and step 2 side by side, split by a hairline.
#[derive(Debug)]
pub struct DixHallpikePlate;

impl Scene for DixHallpikePlate {
    fn duration(&self) -> Duration<'_> {
        Duration::Frames(1)
    }
    fn name(&self) -> &'static str {
        "DixHallpikePlate"
    }
    fn render_frame<'a>(&'a self, _frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let left = View { ox: 425.0, oy: 860.0, s: 330.0, stroke: 3.0 };
        let right = View { ox: 960.0 + 345.0, ..left };
        fframes::svgr!(
            <g>
                {dix_hallpike::scene(&left, &State::step_1())}
                {dix_hallpike::scene(&right, &State::step_2())}
                <path d="M960 120V840" stroke={INK} stroke-width="1.2" />
            </g>
        )
    }
}

macro_rules! figure_video {
    ($name:ident, $scene:expr) => {
        #[derive(Debug)]
        pub struct $name;

        impl Video for $name {
            const FPS: usize = FPS;
            const WIDTH: usize = WIDTH;
            const HEIGHT: usize = HEIGHT;
            const BACKGROUND_COLOR: Color = Color::WHITE;

            fn duration(&self) -> Duration<'_> {
                Duration::Auto
            }
            fn audio(&self) -> AudioMap<'_> {
                AudioMap::none()
            }
            fn define_scenes(&self) -> Scenes<'_> {
                Scenes::from(vec![&$scene as &dyn Scene])
            }
            fn render_frame<'a>(&'a self, frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
                fframes::svgr!(
                    <svg xmlns="http://www.w3.org/2000/svg" width="1920" height="960" viewBox="0 0 1920 960">
                        <rect width="1920" height="960" fill={PAPER} />
                        {ctx.render_scenes(&frame)}
                    </svg>
                )
            }
        }
    };
}

/// `thieme_puppet_rig.txt` in the test poses of `puppet::POSES`, side by side.
#[derive(Debug)]
pub struct PuppetSheet;

pub const PUPPET_SCALE: f32 = 1.1;

/// Top left of pose `i` on the sheet, in canvas pixels.
pub fn puppet_origin(i: usize) -> (f32, f32) {
    (60.0 + i as f32 * 460.0, 25.0)
}

impl Scene for PuppetSheet {
    fn duration(&self) -> Duration<'_> {
        Duration::Frames(1)
    }
    fn name(&self) -> &'static str {
        "PuppetSheet"
    }
    fn render_frame<'a>(&'a self, _frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let figures: Vec<Svgr> = puppet::POSES
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let (x, y) = puppet_origin(i);
                puppet::draw(p, x, y, PUPPET_SCALE)
            })
            .collect();
        fframes::svgr!(<g>{figures}</g>)
    }
}

figure_video!(Short, DixHallpike);
figure_video!(Plate, DixHallpikePlate);
figure_video!(Puppet, PuppetSheet);
