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

/// The server of the service: a card with three rack slots whose lights blink
/// while it runs; `on` 0..1 fades the lights from grey to green.
pub fn server(x: f32, y: f32, w: f32, h: f32, t: f32, on: f32, label: &str) -> Svgr<'static> {
    let sh = (h - 86.0) / 3.0;
    let mut slots = Vec::new();
    for i in 0..3 {
        let sy = y + 20.0 + i as f32 * (sh + 8.0);
        let blink = 0.45 + 0.55 * (0.5 + 0.5 * (t * 2.3 + i as f32 * 1.7).sin());
        let led = mix(FAINT, GREEN, on);
        let led_o = lerp(1.0, blink, clamp01(on));
        slots.push(fframes::svgr!(
            <g>
                <rect x={x + 20.0} y={sy} width={w - 40.0} height={sh} rx="8" fill={BG} stroke={LINE} stroke-width="1.5" />
                <circle cx={x + 44.0} cy={sy + sh / 2.0} r="6" fill={led} opacity={led_o} />
                <rect x={x + 64.0} y={sy + sh / 2.0 - 2.0} width={w * 0.34} height="4" rx="2" fill={LINE} />
                <rect x={x + w - 92.0} y={sy + sh / 2.0 - 2.0} width="52" height="4" rx="2" fill={LINE} />
            </g>
        ));
    }
    let bar = mix(FAINT, BLUE, on);
    fframes::svgr!(
        <g>
            {card(x, y, w, h, Some(&bar), PANEL, LINE)}
            {slots}
            {text(x + w / 2.0, y + h - 20.0, 22.0, 500, &mix(MUTED, INK, on), "middle", label)}
        </g>
    )
}

/// Wraps `body` in an arbitrary transform with opacity.
pub fn moved<'a>(o: f32, transform: String, body: Svgr<'a>) -> Svgr<'a> {
    if o <= 0.001 {
        return Svgr::empty();
    }
    fframes::svgr!(<g opacity={clamp01(o)} transform={transform}>{body}</g>)
}
