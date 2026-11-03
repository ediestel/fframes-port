//! Palette, easing and the drawing pieces every scene uses.
//!
//! Colors follow the script: amber for prompts, blue for source data, violet for
//! the remote LLM, green for validated outcomes, red for conflicts. Color is always
//! paired with a word or an icon.

use fframes::{FFramesContext, FontQuery, Frame, Svgr};

pub const BG: &str = "#0A1028";
/// Cards: a lifted navy, slightly lighter than the canvas.
pub const PANEL: &str = "#151D45";
pub const INK: &str = "#EEF2FF";
pub const MUTED: &str = "#9BA6D0";
pub const FAINT: &str = "#5A6699";
pub const LINE: &str = "#2B3672";

pub const AMBER: &str = "#FFB23F";
pub const AMBER_BG: &str = "#3A2A10";
pub const AMBER_HL: &str = "#8A5A0C";
pub const BLUE: &str = "#5CA8FF";
pub const BLUE_BG: &str = "#13295C";
pub const VIOLET: &str = "#B48CFF";
pub const VIOLET_BG: &str = "#2B1D60";
pub const GREEN: &str = "#34D399";
pub const GREEN_BG: &str = "#0D3B33";
pub const RED: &str = "#FF6B7A";
pub const RED_BG: &str = "#47182B";

pub const SANS: &str = "DM Sans";
pub const MONO: &str = "JetBrains Mono";

/// Safe margin of the script: essential content stays 96 px inside the frame.
pub const M: f32 = 96.0;

// ---------------------------------------------------------------------------
// easing

pub fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

/// Where `t` is between `a` and `b`, clamped to 0..1.
pub fn prog(t: f32, a: f32, b: f32) -> f32 {
    if b <= a {
        return if t >= a { 1.0 } else { 0.0 };
    }
    clamp01((t - a) / (b - a))
}

pub fn lerp(a: f32, b: f32, k: f32) -> f32 {
    a + (b - a) * k
}

pub fn ease_out(k: f32) -> f32 {
    1.0 - (1.0 - clamp01(k)).powi(3)
}

pub fn ease_in(k: f32) -> f32 {
    clamp01(k).powi(3)
}

pub fn ease_in_out(k: f32) -> f32 {
    let k = clamp01(k);
    if k < 0.5 {
        4.0 * k * k * k
    } else {
        1.0 - (-2.0 * k + 2.0).powi(3) / 2.0
    }
}

/// A damped spring 0 → 1 with a small overshoot, `k` in seconds since start.
pub fn spring(secs: f32) -> f32 {
    if secs <= 0.0 {
        return 0.0;
    }
    1.0 - (-secs * 6.5).exp() * (secs * 10.0).cos()
}

/// Ease-out entrance over `dur` seconds from `a`.
pub fn enter(t: f32, a: f32, dur: f32) -> f32 {
    ease_out(prog(t, a, a + dur))
}

/// 1 inside `a..b`, fading in over `f` after `a` and out over `f` before `b`.
pub fn window(t: f32, a: f32, b: f32, f: f32) -> f32 {
    prog(t, a, a + f) * (1.0 - prog(t, b - f, b))
}

/// Mixes two `#rrggbb` colors.
pub fn mix(a: &str, b: &str, k: f32) -> String {
    let c = |s: &str, i: usize| u8::from_str_radix(&s[1 + i * 2..3 + i * 2], 16).unwrap_or(0) as f32;
    let k = clamp01(k);
    format!(
        "#{:02x}{:02x}{:02x}",
        lerp(c(a, 0), c(b, 0), k).round() as u8,
        lerp(c(a, 1), c(b, 1), k).round() as u8,
        lerp(c(a, 2), c(b, 2), k).round() as u8
    )
}

pub fn tr(x: f32, y: f32) -> String {
    format!("translate({x:.2} {y:.2})")
}

pub fn trs(x: f32, y: f32, s: f32) -> String {
    format!("translate({x:.2} {y:.2}) scale({s:.4})")
}

/// Scales around the point (cx, cy).
pub fn scale_at(cx: f32, cy: f32, s: f32) -> String {
    format!("translate({cx:.2} {cy:.2}) scale({s:.4}) translate({:.2} {:.2})", -cx, -cy)
}

// ---------------------------------------------------------------------------
// text

pub fn text(x: f32, y: f32, size: f32, weight: u16, fill: &str, anchor: &'static str, s: &str) -> Svgr<'static> {
    let s = s.to_owned();
    let fill = fill.to_owned();
    fframes::svgr!(
        <text x={x} y={y} font-family={SANS} font-size={size} font-weight={weight} fill={fill} text-anchor={anchor}>{s}</text>
    )
}

pub fn mono(x: f32, y: f32, size: f32, fill: &str, anchor: &'static str, s: &str) -> Svgr<'static> {
    let s = s.to_owned();
    let fill = fill.to_owned();
    fframes::svgr!(
        <text x={x} y={y} font-family={MONO} font-size={size} font-weight="400" fill={fill} text-anchor={anchor}>{s}</text>
    )
}

/// Advance width of JetBrains Mono: 0.6 em per character.
pub fn mono_w(s: &str, size: f32) -> f32 {
    s.chars().count() as f32 * size * 0.6
}

/// Measured width of DM Sans text.
pub fn sans_w<'a>(frame: &mut Frame, ctx: &FFramesContext<'a, '_>, s: &'a str, size: usize, weight: u16) -> f32 {
    frame
        .text_width(ctx, FontQuery { family: SANS, size, weight, ..Default::default() }, s)
        .map(|w| w as f32)
        .unwrap_or(s.chars().count() as f32 * size as f32 * 0.52)
}

/// Wrapped DM Sans paragraph, top-left at (x, y) with the first baseline at y.
#[allow(clippy::too_many_arguments)]
pub fn para<'a>(
    frame: &mut Frame,
    ctx: &FFramesContext<'a, '_>,
    x: i32,
    y: i32,
    width: usize,
    size: usize,
    weight: u16,
    fill: &'a str,
    s: &str,
) -> Svgr<'a> {
    frame
        .text_break_lines(
            ctx,
            s,
            fframes::BreakLinesOpts {
                font: FontQuery { family: SANS, size, weight, ..Default::default() },
                width,
                line_height: 1.3,
                x,
                y,
                align: fframes::TextAlign::Left,
                fill,
                dominant_baseline: "auto",
                text_anchor: "start",
                opacity: 1.0,
            },
        )
        .unwrap_or_else(Svgr::empty)
}

/// One line of DM Sans built from segments; segments with `Some(k)` get a
/// highlighter mark behind them that is drawn on with `k` (0..1).
#[allow(clippy::too_many_arguments)]
pub fn marked_line<'a>(
    frame: &mut Frame,
    ctx: &FFramesContext<'a, '_>,
    x: f32,
    y: f32,
    size: usize,
    fill: &'static str,
    mark: &'static str,
    segs: &[(&'a str, Option<f32>)],
) -> Svgr<'a> {
    let mut cx = x;
    let mut marks = Vec::new();
    let mut words = Vec::new();
    let s = size as f32;
    for (seg, k) in segs {
        // segments are written with their spaces; the gap is added here
        let lead = seg.starts_with(' ');
        let trail = seg.ends_with(' ');
        let word: &'a str = seg.trim();
        if lead {
            cx += s * 0.36;
        }
        let w = sans_w(frame, ctx, word, size, 400);
        if let Some(k) = k {
            let k = clamp01(*k);
            if k > 0.0 {
                marks.push(fframes::svgr!(
                    <rect x={cx - 5.0} y={y - s * 0.86} width={(w + 10.0) * ease_out(k)} height={s * 1.16} rx="5" fill={mark} />
                ));
            }
        }
        words.push(fframes::svgr!(
            <text x={cx} y={y} font-family={SANS} font-size={size} font-weight="400" fill={fill}>{word}</text>
        ));
        cx += w + if trail { s * 0.36 } else { 0.0 };
    }
    fframes::svgr!(<g>{marks}{words}</g>)
}

// ---------------------------------------------------------------------------
// shapes

/// A white card with a hairline and an optional colored left bar.
pub fn card(x: f32, y: f32, w: f32, h: f32, bar: Option<&str>, fill: &str, stroke: &str) -> Svgr<'static> {
    let fill = fill.to_owned();
    let stroke = stroke.to_owned();
    let bar_svg = match bar {
        Some(c) => {
            let c = c.to_owned();
            fframes::svgr!(<rect x={x} y={y} width="10" height={h} rx="5" fill={c} />)
        }
        None => Svgr::empty(),
    };
    fframes::svgr!(
        <g>
            <rect x={x + 4.0} y={y + 16.0} width={w - 8.0} height={h} rx="18" fill="#000000" opacity="0.22" />
            <rect x={x} y={y + 6.0} width={w} height={h} rx="16" fill="#000000" opacity="0.25" />
            <rect x={x} y={y} width={w} height={h} rx="16" fill={fill} stroke={stroke} stroke-width="1.5" />
            {bar_svg}
        </g>
    )
}

/// A pill badge in mono capitals, left edge at x, vertical center at y.
pub fn badge(x: f32, y: f32, s: &str, fg: &str, bg: &str, size: f32) -> Svgr<'static> {
    let w = mono_w(s, size) + size * 1.4;
    let h = size * 1.75;
    let s = s.to_owned();
    let fg = fg.to_owned();
    let bg = bg.to_owned();
    fframes::svgr!(
        <g>
            <rect x={x} y={y - h / 2.0} width={w} height={h} rx={h / 2.0} fill={bg} stroke={fg.clone()} stroke-width="1.5" />
            <text x={x + size * 0.7} y={y + size * 0.36} font-family={MONO} font-size={size} font-weight="400" fill={fg}>{s}</text>
        </g>
    )
}

pub fn badge_w(s: &str, size: f32) -> f32 {
    mono_w(s, size) + size * 1.4
}

/// A line from (x1, y1) to (x2, y2) drawn on with `p`, with an arrow head at the end.
pub fn arrow(x1: f32, y1: f32, x2: f32, y2: f32, p: f32, color: &str, width: f32) -> Svgr<'static> {
    let p = clamp01(p);
    if p <= 0.0 {
        return Svgr::empty();
    }
    let x = lerp(x1, x2, p);
    let y = lerp(y1, y2, p);
    let a = (y2 - y1).atan2(x2 - x1);
    let hl = 14.0;
    let (h1x, h1y) = (x - hl * (a - 0.45).cos(), y - hl * (a - 0.45).sin());
    let (h2x, h2y) = (x - hl * (a + 0.45).cos(), y - hl * (a + 0.45).sin());
    let line = format!("M{x1:.1} {y1:.1} L{x:.1} {y:.1}");
    let head = format!("M{h1x:.1} {h1y:.1} L{x:.1} {y:.1} L{h2x:.1} {h2y:.1}");
    let color = color.to_owned();
    fframes::svgr!(
        <g fill="none" stroke={color} stroke-width={width} stroke-linecap="round" stroke-linejoin="round">
            <path d={line} />
            <path d={head} opacity={prog(p, 0.7, 1.0)} />
        </g>
    )
}

/// A dot travelling along a straight line, looping every `period` seconds.
pub fn flow_dot(x1: f32, y1: f32, x2: f32, y2: f32, t: f32, period: f32, phase: f32, color: &str) -> Svgr<'static> {
    let k = ((t / period) + phase).fract();
    let o = (k * 6.0).min(1.0) * ((1.0 - k) * 6.0).min(1.0);
    let color = color.to_owned();
    fframes::svgr!(<circle cx={lerp(x1, x2, k)} cy={lerp(y1, y2, k)} r="6" fill={color} opacity={o} />)
}

/// A check mark in a circle, drawn on with `p`.
pub fn check(cx: f32, cy: f32, r: f32, p: f32, color: &str) -> Svgr<'static> {
    if p <= 0.0 {
        return Svgr::empty();
    }
    let s = spring(p * 0.9).max(0.0);
    let d = format!(
        "M{:.1} {:.1} L{:.1} {:.1} L{:.1} {:.1}",
        cx - r * 0.42,
        cy + r * 0.02,
        cx - r * 0.1,
        cy + r * 0.34,
        cx + r * 0.46,
        cy - r * 0.32
    );
    let len = r * 1.5;
    let dash = format!("{len:.1}");
    let off = format!("{:.1}", len * (1.0 - prog(p, 0.25, 0.8)));
    let color = color.to_owned();
    fframes::svgr!(
        <g transform={scale_at(cx, cy, s)}>
            <circle cx={cx} cy={cy} r={r} fill={color.clone()} />
            <path d={d} fill="none" stroke="#FFFFFF" stroke-width={r * 0.22} stroke-linecap="round" stroke-linejoin="round"
                  stroke-dasharray={dash} stroke-dashoffset={off} />
        </g>
    )
}

/// A cross in a circle (excluded, conflict).
pub fn cross(cx: f32, cy: f32, r: f32, p: f32, color: &str) -> Svgr<'static> {
    if p <= 0.0 {
        return Svgr::empty();
    }
    let s = spring(p * 0.9).max(0.0);
    let q = r * 0.36;
    let d = format!(
        "M{:.1} {:.1} L{:.1} {:.1} M{:.1} {:.1} L{:.1} {:.1}",
        cx - q, cy - q, cx + q, cy + q, cx + q, cy - q, cx - q, cy + q
    );
    let color = color.to_owned();
    fframes::svgr!(
        <g transform={scale_at(cx, cy, s)}>
            <circle cx={cx} cy={cy} r={r} fill={color} />
            <path d={d} stroke="#FFFFFF" stroke-width={r * 0.22} stroke-linecap="round" />
        </g>
    )
}

/// A padlock; `closed` 0..1 lowers the shackle.
pub fn lock(cx: f32, cy: f32, s: f32, closed: f32, color: &str) -> Svgr<'static> {
    let lift = (1.0 - ease_out(closed)) * s * 0.35;
    let shackle = format!(
        "M{:.1} {:.1} V{:.1} A{:.1} {:.1} 0 0 1 {:.1} {:.1} V{:.1}",
        cx - s * 0.32,
        cy - s * 0.05 - lift,
        cy - s * 0.32 - lift,
        s * 0.32,
        s * 0.32,
        cx + s * 0.32,
        cy - s * 0.32 - lift,
        cy - s * 0.05 - lift * 0.2
    );
    let color = color.to_owned();
    fframes::svgr!(
        <g>
            <path d={shackle} fill="none" stroke={color.clone()} stroke-width={s * 0.12} stroke-linecap="round" />
            <rect x={cx - s * 0.5} y={cy - s * 0.08} width={s} height={s * 0.72} rx={s * 0.12} fill={color} />
            <circle cx={cx} cy={cy + s * 0.25} r={s * 0.09} fill="#FFFFFF" />
        </g>
    )
}

/// The remote LLM: a violet disc with a slowly turning dashed orbit.
pub fn llm_node(cx: f32, cy: f32, r: f32, t: f32, pulse: f32, label: &str) -> Svgr<'static> {
    let rot = t * 14.0;
    let glow = r * (1.25 + 0.12 * pulse);
    let label = label.to_owned();
    let dash = format!("{:.1} {:.1}", r * 0.22, r * 0.16);
    fframes::svgr!(
        <g>
            <circle cx={cx} cy={cy} r={glow} fill={VIOLET_BG} opacity={0.7 + 0.3 * pulse} />
            <g transform={format!("rotate({rot:.2} {cx:.1} {cy:.1})")}>
                <circle cx={cx} cy={cy} r={r * 1.12} fill="none" stroke={VIOLET} stroke-width="3" stroke-dasharray={dash} opacity="0.55" />
            </g>
            <circle cx={cx} cy={cy + r * 0.12} r={r} fill="#4C1D95" opacity="0.18" />
            <circle cx={cx} cy={cy} r={r} fill="url(#llmFill)" />
            <text x={cx} y={cy + r * 0.16} font-family={SANS} font-size={r * 0.46} font-weight="500" fill="#FFFFFF" text-anchor="middle">"LLM"</text>
            <text x={cx} y={cy + r * 1.62} font-family={SANS} font-size="26" font-weight="500" fill={VIOLET} text-anchor="middle">{label}</text>
        </g>
    )
}

/// The two zones of the remote-exchange scenes: Korntal's controlled
/// environment on the left, the remote processing zone on the right, and a
/// labelled boundary between them.
pub fn zones(bx: f32, o: f32, left: &str, right: &str) -> Svgr<'static> {
    if o <= 0.0 {
        return Svgr::empty();
    }
    let left = left.to_owned();
    let right = right.to_owned();
    fframes::svgr!(
        <g opacity={o}>
            <rect x={M} y="160" width={bx - M - 16.0} height="712" rx="20" fill={BLUE_BG} opacity="0.55" />
            <rect x={bx + 16.0} y="160" width={1920.0 - M - bx - 16.0} height="712" rx="20" fill={VIOLET_BG} opacity="0.7" />
            <path d={format!("M{bx} 168 V864")} stroke={INK} stroke-width="3" stroke-dasharray="14 10" opacity="0.55" />
            <text x={M + 24.0} y="198" font-family={SANS} font-size="24" font-weight="500" fill={BLUE}>{left}</text>
            <text x={bx + 40.0} y="198" font-family={SANS} font-size="24" font-weight="500" fill={VIOLET}>{right}</text>
        </g>
    )
}

/// A small envelope icon.
pub fn icon_mail(x: f32, y: f32, s: f32, color: &str) -> Svgr<'static> {
    let d = format!("M{x} {y} L{} {} L{} {y}", x + s * 0.5, y + s * 0.38, x + s);
    let color = color.to_owned();
    fframes::svgr!(
        <g fill="none" stroke={color} stroke-width="3" stroke-linejoin="round">
            <rect x={x} y={y} width={s} height={s * 0.7} rx="4" />
            <path d={d} />
        </g>
    )
}

/// A database cylinder.
pub fn icon_db(cx: f32, top: f32, w: f32, h: f32, color: &str, fill: &str) -> Svgr<'static> {
    let rx = w / 2.0;
    let ry = w * 0.16;
    let body = format!(
        "M{:.1} {:.1} V{:.1} A{rx:.1} {ry:.1} 0 0 0 {:.1} {:.1} V{:.1}",
        cx - rx,
        top,
        top + h,
        cx + rx,
        top + h,
        top
    );
    let mid = format!("M{:.1} {:.1} A{rx:.1} {ry:.1} 0 0 0 {:.1} {:.1}", cx - rx, top + h * 0.5, cx + rx, top + h * 0.5);
    let color = color.to_owned();
    let fill = fill.to_owned();
    fframes::svgr!(
        <g stroke={color} stroke-width="3">
            <path d={body} fill={fill.clone()} />
            <ellipse cx={cx} cy={top} rx={rx} ry={ry} fill={fill} />
            <path d={mid} fill="none" />
        </g>
    )
}

/// A document with text lines.
pub fn icon_doc(x: f32, y: f32, s: f32, color: &str) -> Svgr<'static> {
    let lines = format!(
        "M{:.1} {:.1} H{:.1} M{:.1} {:.1} H{:.1} M{:.1} {:.1} H{:.1}",
        x + s * 0.2,
        y + s * 0.4,
        x + s * 0.6,
        x + s * 0.2,
        y + s * 0.58,
        x + s * 0.6,
        x + s * 0.2,
        y + s * 0.76,
        x + s * 0.48
    );
    let color = color.to_owned();
    fframes::svgr!(
        <g fill="none" stroke={color} stroke-width="3" stroke-linecap="round">
            <rect x={x} y={y} width={s * 0.8} height={s} rx="5" />
            <path d={lines} />
        </g>
    )
}

/// A grid of room tiles (room data).
pub fn icon_grid(x: f32, y: f32, s: f32, color: &str) -> Svgr<'static> {
    let c = s * 0.44;
    let color = color.to_owned();
    fframes::svgr!(
        <g fill="none" stroke={color} stroke-width="3">
            <rect x={x} y={y} width={c} height={c} rx="3" />
            <rect x={x + s * 0.56} y={y} width={c} height={c} rx="3" />
            <rect x={x} y={y + s * 0.56} width={c} height={c} rx="3" />
            <rect x={x + s * 0.56} y={y + s * 0.56} width={c} height={c} rx="3" />
        </g>
    )
}

/// A gear-like dial (configuration).
pub fn icon_dial(cx: f32, cy: f32, s: f32, color: &str) -> Svgr<'static> {
    let r = s * 0.32;
    let mut teeth = String::new();
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::PI / 4.0;
        teeth.push_str(&format!(
            "M{:.1} {:.1} L{:.1} {:.1} ",
            cx + a.cos() * r,
            cy + a.sin() * r,
            cx + a.cos() * s * 0.5,
            cy + a.sin() * s * 0.5
        ));
    }
    let color = color.to_owned();
    fframes::svgr!(
        <g fill="none" stroke={color} stroke-width="3" stroke-linecap="round">
            <circle cx={cx} cy={cy} r={r} />
            <circle cx={cx} cy={cy} r={r * 0.4} />
            <path d={teeth} />
        </g>
    )
}

/// A person (capacity, guest).
pub fn icon_person(cx: f32, top: f32, s: f32, color: &str) -> Svgr<'static> {
    let body = format!(
        "M{:.1} {:.1} A{:.1} {:.1} 0 0 1 {:.1} {:.1} Z",
        cx - s * 0.42,
        top + s,
        s * 0.42,
        s * 0.42,
        cx + s * 0.42,
        top + s
    );
    let color = color.to_owned();
    fframes::svgr!(
        <g fill={color}>
            <circle cx={cx} cy={top + s * 0.24} r={s * 0.22} />
            <path d={body} />
        </g>
    )
}

/// A monitor with a terminal prompt (desktop app / CLI host).
pub fn icon_desktop(cx: f32, cy: f32, s: f32, color: &str) -> Svgr<'static> {
    let w = s;
    let h = s * 0.62;
    let prompt = format!(
        "M{:.1} {:.1} L{:.1} {:.1} L{:.1} {:.1} M{:.1} {:.1} H{:.1}",
        cx - w * 0.34,
        cy - h * 0.62,
        cx - w * 0.24,
        cy - h * 0.5,
        cx - w * 0.34,
        cy - h * 0.38,
        cx - w * 0.18,
        cy - h * 0.36,
        cx - w * 0.02
    );
    let stand = format!("M{:.1} {:.1} V{:.1} M{:.1} {:.1} H{:.1}", cx, cy, cy + s * 0.14, cx - s * 0.16, cy + s * 0.14, cx + s * 0.16);
    let color = color.to_owned();
    fframes::svgr!(
        <g fill="none" stroke={color} stroke-width="3" stroke-linecap="round" stroke-linejoin="round">
            <rect x={cx - w / 2.0} y={cy - h} width={w} height={h} rx="8" />
            <path d={prompt} />
            <path d={stand} />
        </g>
    )
}

/// A chat window (a generic LLM interface).
pub fn icon_chat(cx: f32, cy: f32, s: f32, color: &str) -> Svgr<'static> {
    let w = s;
    let h = s * 0.7;
    let b1 = format!("M{:.1} {:.1} H{:.1}", cx - w * 0.36, cy - h * 0.62, cx + w * 0.06);
    let b2 = format!("M{:.1} {:.1} H{:.1}", cx - w * 0.06, cy - h * 0.4, cx + w * 0.36);
    let b3 = format!("M{:.1} {:.1} H{:.1}", cx - w * 0.36, cy - h * 0.18, cx + w * 0.14);
    let color = color.to_owned();
    fframes::svgr!(
        <g fill="none" stroke={color} stroke-width="3" stroke-linecap="round">
            <rect x={cx - w / 2.0} y={cy - h} width={w} height={h} rx="10" />
            <path d={b1} />
            <path d={b2} />
            <path d={b3} />
        </g>
    )
}

/// A folder with a lock (protected export folder).
pub fn icon_folder(x: f32, y: f32, w: f32, h: f32, color: &str, fill: &str) -> Svgr<'static> {
    let d = format!(
        "M{x:.1} {:.1} V{:.1} H{:.1} L{:.1} {:.1} H{:.1} V{:.1} Z",
        y + h,
        y,
        x + w * 0.32,
        x + w * 0.4,
        y + h * 0.1,
        x + w,
        y + h
    );
    let color = color.to_owned();
    let fill = fill.to_owned();
    fframes::svgr!(<path d={d} fill={fill} stroke={color} stroke-width="3" stroke-linejoin="round" />)
}

/// A button; `press` 0..1 sinks it and fills it.
pub fn button(x: f32, y: f32, w: f32, h: f32, label: &str, color: &str, press: f32) -> Svgr<'static> {
    let sink = (press * std::f32::consts::PI).sin() * 4.0;
    let fill = mix(PANEL, color, ease_out(press));
    let fg = mix(color, PANEL, ease_out(press));
    let label = label.to_owned();
    let color = color.to_owned();
    fframes::svgr!(
        <g transform={tr(0.0, sink)}>
            <rect x={x} y={y} width={w} height={h} rx={h / 2.0} fill={fill} stroke={color} stroke-width="2.5" />
            <text x={x + w / 2.0} y={y + h / 2.0 + 9.0} font-family={SANS} font-size="26" font-weight="500" fill={fg} text-anchor="middle">{label}</text>
        </g>
    )
}

/// A key/value row: muted label left, value right of it.
#[allow(clippy::too_many_arguments)]
pub fn kv(x: f32, y: f32, kw: f32, key: &str, val: &str, size: f32, val_fill: &str, val_mono: bool) -> Svgr<'static> {
    let k = text(x, y, size * 0.86, 400, MUTED, "start", key);
    let v = if val_mono {
        mono(x + kw, y, size * 0.9, val_fill, "start", val)
    } else {
        text(x + kw, y, size, 500, val_fill, "start", val)
    };
    fframes::svgr!(<g>{k}{v}</g>)
}
