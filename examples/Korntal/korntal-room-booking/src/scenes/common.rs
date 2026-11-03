use fframes::Svgr;

use crate::ui::*;

/// Wraps `body` in a group with opacity `o`, shifted down by `dy`.
pub fn show<'a>(o: f32, dy: f32, body: Svgr<'a>) -> Svgr<'a> {
    if o <= 0.001 {
        return Svgr::empty();
    }
    fframes::svgr!(<g opacity={clamp01(o)} transform={tr(0.0, dy)}>{body}</g>)
}

/// Entrance at `at`: fades in and rises 24 px.
pub fn rise_in<'a>(t: f32, at: f32, body: Svgr<'a>) -> Svgr<'a> {
    let k = enter(t, at, 0.55);
    show(k, (1.0 - k) * 24.0, body)
}

/// Visible from `a` (rising in) until `b` (fading out faster).
pub fn during<'a>(t: f32, a: f32, b: f32, body: Svgr<'a>) -> Svgr<'a> {
    let k = enter(t, a, 0.55);
    let out = 1.0 - ease_in(prog(t, b - 0.3, b));
    show(k * out, (1.0 - k) * 24.0, body)
}

/// Wraps `body` in an arbitrary transform with opacity.
pub fn moved<'a>(o: f32, transform: String, body: Svgr<'a>) -> Svgr<'a> {
    if o <= 0.001 {
        return Svgr::empty();
    }
    fframes::svgr!(<g opacity={clamp01(o)} transform={transform}>{body}</g>)
}
