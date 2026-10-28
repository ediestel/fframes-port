//! The window traced from `KI_guldensporenslag.png`: the picture was
//! quantised to a palette, every colour traced to vector outlines with
//! potrace, the lead last; `KI_guldensporenslag_traced.svg` holds those
//! layers, one `<g>` per colour, masonry first, lead last. This module embeds
//! that file over a dark wall gradient that shows through any untraced gap.
//! One frame: `golden-spurs-traced frame 0`.

use fframes::{AudioMap, Color, Duration, FFramesContext, Frame, Scene, Scenes, Svgr, Video};
use std::sync::OnceLock;

pub const WIDTH: usize = 1200;
pub const HEIGHT: usize = 1800;

/// `KI_guldensporenslag_edit_traced.svg`: the flat-vector edit of the picture
/// (`KI_guldensporenslag_edit.png`), traced with `FLAT=1 trace.sh`.
/// `KI_guldensporenslag_traced.svg` is the trace of the photograph itself.
const TRACED: &str = include_str!("../KI_guldensporenslag_edit_traced.svg");

fn window_svg() -> String {
    let head_end = TRACED.find('>').map(|i| i + 1).unwrap_or(0);
    let body_end = TRACED.rfind("</svg>").unwrap_or(TRACED.len());
    let head = &TRACED[..head_end];
    let body = &TRACED[head_end..body_end];
    let (w, h) = (attr(head, "width").unwrap_or(1664.0), attr(head, "height").unwrap_or(2496.0));
    // the wall layers are left out by trace.sh; the gradient shows through
    let body = body.to_string();
    let (cx, cy) = (w * 0.5, h * 0.6);
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}" viewBox="0 0 {w} {h}"><defs><linearGradient id="wallg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#15142a"/><stop offset="0.45" stop-color="#201f36"/><stop offset="1" stop-color="#191830"/></linearGradient><radialGradient id="halo" cx="{cx}" cy="{cy}" r="{r1}" gradientUnits="userSpaceOnUse"><stop offset="0.6" stop-color="#8ea0d8" stop-opacity="0.07"/><stop offset="1" stop-color="#8ea0d8" stop-opacity="0"/></radialGradient><radialGradient id="vignette" cx="{cx}" cy="{cy2}" r="{r2}" gradientUnits="userSpaceOnUse"><stop offset="0.5" stop-color="#000" stop-opacity="0"/><stop offset="1" stop-color="#000" stop-opacity="0.7"/></radialGradient></defs><rect width="{w}" height="{h}" fill="url(#wallg)"/>{body}</svg>"##,
        r1 = w * 0.7,
        cy2 = h * 0.56,
        r2 = h * 0.66,
    )
}

fn attr(tag: &str, name: &str) -> Option<f32> {
    let key = format!("{name}=\"");
    let i = tag.find(&key)? + key.len();
    let j = tag[i..].find('"')? + i;
    tag[i..j].parse().ok()
}

static SVG: OnceLock<String> = OnceLock::new();

/// The finished window as SVG text.
pub fn svg() -> String {
    SVG.get_or_init(window_svg).clone()
}

#[derive(Debug)]
pub struct TracedPanel;

impl Scene for TracedPanel {
    fn duration(&self) -> Duration<'_> {
        Duration::Frames(1)
    }
    fn name(&self) -> &'static str {
        "Window"
    }
    fn render_frame<'a>(&'a self, _frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        Svgr::empty()
    }
}

#[derive(Debug)]
pub struct TracedWindow;

impl Video for TracedWindow {
    const FPS: usize = 30;
    const WIDTH: usize = WIDTH;
    const HEIGHT: usize = HEIGHT;
    const BACKGROUND_COLOR: Color = Color::BLACK;

    fn duration(&self) -> Duration<'_> {
        Duration::Auto
    }
    fn audio(&self) -> AudioMap<'_> {
        AudioMap::none()
    }
    fn define_scenes(&self) -> Scenes<'_> {
        Scenes::from(vec![&TracedPanel as &dyn Scene])
    }
    fn render_frame<'a>(&'a self, _frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        Svgr::from_svg_string(svg()).unwrap_or_else(|_| Svgr::empty())
    }
}
