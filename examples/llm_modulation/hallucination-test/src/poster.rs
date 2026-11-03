//! The poster frames the film. It lies on a desk; the camera pushes into its
//! first section, the sections are then presented one after another on an
//! off-white stage under their own titles, and at the end the camera pulls
//! back out to the whole poster.
//!
//! Poster coordinates are PDF points of the A0 page (2383.94 × 3370.39);
//! `poster.jpg` is the page rendered at 144 dpi, two pixels per point.

use fframes::{FFramesContext, Svgr};

use crate::ui::*;

pub const PW: f32 = 2383.94;
pub const PH: f32 = 3370.39;

/// A rectangle on the poster, in points.
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

pub const fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
    Rect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
}

// The section titles, from the poster's word boxes (with a little room).
pub const H_QUERY: Rect = rect(62.0, 378.0, 232.0, 416.0);
pub const H_PROGRAM_B: Rect = rect(62.0, 1607.0, 939.0, 1645.0);
pub const H_CONCLUSION: Rect = rect(62.0, 2755.0, 240.0, 2793.0);

/// Where the camera looks: the poster point at the centre of the frame and
/// the zoom in screen pixels per point.
#[derive(Debug, Clone, Copy)]
pub struct Cam {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// The whole poster, a sheet on the desk.
pub const FULL: Cam = Cam { x: PW / 2.0, y: PH / 2.0 + 6.0, z: 0.296 };

/// The zoom at which a section title reads at 48 px, as the stage's titles.
pub const STAGE_Z: f32 = 1.55;

/// The camera that puts a section title's top-left corner at the stage's
/// title position.
pub fn on_title(r: Rect, z: f32) -> Cam {
    on_point(r.x, r.y, MX, 62.0, z)
}

/// The camera that shows poster point (px, py) at screen point (sx, sy).
pub fn on_point(px: f32, py: f32, sx: f32, sy: f32, z: f32) -> Cam {
    Cam { x: px + (W / 2.0 - sx) / z, y: py + (H / 2.0 - sy) / z, z }
}

impl Cam {
    /// Screen position of a poster point.
    pub fn screen(&self, px: f32, py: f32) -> (f32, f32) {
        (W / 2.0 + (px - self.x) * self.z, H / 2.0 + (py - self.y) * self.z)
    }

    pub fn transform(&self) -> String {
        format!(
            "translate({} {}) scale({})",
            W / 2.0 - self.x * self.z,
            H / 2.0 - self.y * self.z,
            self.z
        )
    }

    /// The camera `u` of the way from `self` to `to`. Pushes and pulls keep
    /// the more zoomed-in view's centre gliding on a straight screen path;
    /// glides between two close-ups lean back a little on the way.
    pub fn to(self, to: Cam, u: f32) -> Cam {
        let u = ease_in_out(u);
        let z = (self.z.ln() + (to.z.ln() - self.z.ln()) * u).exp();
        if self.z.min(to.z) > 0.5 {
            let dist = ((to.x - self.x).powi(2) + (to.y - self.y).powi(2)).sqrt() * self.z.min(to.z) / W;
            let dip = (dist * 0.45).clamp(0.0, 0.62);
            let z = z * (1.0 - dip * (u * std::f32::consts::PI).sin());
            return Cam { x: lerp(self.x, to.x, u), y: lerp(self.y, to.y, u), z };
        }
        let (near, far, k) = if to.z >= self.z { (to, self, u) } else { (self, to, 1.0 - u) };
        // the near view's centre: where it sits in the far view, gliding to the middle
        let (sx0, sy0) = far.screen(near.x, near.y);
        let sx = lerp(sx0, W / 2.0, k);
        let sy = lerp(sy0, H / 2.0, k);
        Cam { x: near.x - (sx - W / 2.0) / z, y: near.y - (sy - H / 2.0) / z, z }
    }
}

/// A camera path: a start and moves (from, to, target), applied in order.
pub fn cam_at(start: Cam, moves: &[(f32, f32, Cam)], t: f32) -> Cam {
    let mut cam = start;
    for &(a, b, target) in moves {
        if t <= a {
            break;
        }
        cam = cam.to(target, prog(t, a, b));
    }
    cam
}

/// The poster on its desk, seen through `cam`.
pub fn poster(ctx: &FFramesContext, cam: Cam) -> Svgr<'static> {
    let Some(image) = ctx.get_image("poster.jpg") else {
        return fframes::svgr!(<rect width="1920" height="1080" fill={DESK} />);
    };
    let href = image.href();
    fframes::svgr!(
        <g>
            <rect width="1920" height="1080" fill={DESK} />
            <g transform={cam.transform()}>
                <rect x="6" y="10" width={PW} height={PH} fill="#000000" opacity="0.05" />
                <rect x="2" y="3" width={PW} height={PH} fill="#000000" opacity="0.06" />
                <image href={href} x="0" y="0" width={PW} height={PH} preserveAspectRatio="none" />
            </g>
        </g>
    )
}

/// The stage over the poster, `o` opaque: the page the sections are
/// presented on between the push in and the pull back.
pub fn stage(o: f32) -> Svgr<'static> {
    if o <= 0.0 {
        return Svgr::empty();
    }
    fframes::svgr!(<rect width="1920" height="1080" fill={PAPER} opacity={o} />)
}
