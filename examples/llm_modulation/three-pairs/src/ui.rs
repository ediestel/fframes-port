//! Palette, type, the page layout and the pieces every scene shares.
//!
//! Dark, like an editor: a charcoal ground, dark panels, light type in
//! Computer Modern, electric blue for titles and the method. Amber marks a
//! hallucinated literal, mint a justified one; nothing else is colour-coded.
//! The two also differ by mark (underline against tick), so they read
//! without colour vision.
//!
//! Every section page has the same grid: a header with the section number,
//! title and progress (y < 150) and the content beneath it. There are no
//! subtitles: everything the film claims is on the page once.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use fframes::{FFramesContext, FontQuery, FontStyle, Frame, Svgr};

/// Text.
pub const INK: &str = "#E6E8EB";
/// Titles, the method, section numbers.
pub const ACCENT: &str = "#4C9AFF";
/// Justified literals.
pub const OK: &str = "#3DDC97";
/// Hallucinated literals.
pub const FLAG: &str = "#FF8A3D";
/// Line numbers and secondary text.
pub const GREY: &str = "#8A93A0";
/// The ground.
pub const PAPER: &str = "#0F1115";
/// Panels: code, tables, cards.
pub const SHEET: &str = "#181B22";
/// The desk the poster lies on.
pub const DESK: &str = "#0A0B0E";
/// Hairlines.
pub const RULE: &str = "#2A2F38";
/// Washes behind highlighted code lines and chips.
pub const FLAG_WASH: &str = "#3A2618";
pub const OK_WASH: &str = "#15342A";

pub const SERIF: &str = "CMU Serif";
pub const MONO: &str = "CMU Typewriter Text";

pub const W: f32 = 1920.0;
pub const H: f32 = 1080.0;
/// Left margin of the stage: section titles land here.
pub const MX: f32 = 120.0;

// ---------------------------------------------------------------------------
// easing, all on 0..1

pub fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

/// Where `x` is between `a` and `b`, clamped to 0..1.
pub fn prog(x: f32, a: f32, b: f32) -> f32 {
    clamp01((x - a) / (b - a))
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - clamp01(t)).powi(3)
}

pub fn ease_in(t: f32) -> f32 {
    clamp01(t).powi(3)
}

pub fn ease_in_out(t: f32) -> f32 {
    let t = clamp01(t);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// A damped spring from 0 to 1 with a small overshoot.
pub fn spring(t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    1.0 - (-t * 7.0).exp() * (t * 11.0).cos()
}

/// In over `fade` seconds from `a`, out over `fade` seconds before `b`.
pub fn window(t: f32, a: f32, b: f32, fade: f32) -> f32 {
    prog(t, a, a + fade) * (1.0 - prog(t, b - fade, b))
}

// ---------------------------------------------------------------------------
// text measuring

/// (family, size, weight, italic, text)
type WidthKey = (String, usize, u16, bool, String);

static WIDTHS: LazyLock<Mutex<HashMap<WidthKey, f32>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Width of a string in pixels, measured once per (font, text) and cached.
pub fn measure(
    frame: &mut Frame,
    ctx: &FFramesContext,
    family: &str,
    size: usize,
    weight: u16,
    italic: bool,
    text: &str,
) -> f32 {
    let key = (family.to_owned(), size, weight, italic, text.to_owned());
    if let Some(w) = WIDTHS.lock().unwrap().get(&key) {
        return *w;
    }
    let query = FontQuery {
        family,
        size,
        weight,
        style: if italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        },
        ..Default::default()
    };
    // `text_width` wants the text to live as long as the context; results are
    // cached, so every distinct string is leaked at most once.
    let text: &'static str = Box::leak(text.to_owned().into_boxed_str());
    match frame.text_width(ctx, query, text) {
        Some(w) => {
            let w = w as f32;
            WIDTHS.lock().unwrap().insert(key, w);
            w
        }
        None => text.chars().count() as f32 * size as f32 * 0.52,
    }
}

/// A word placed by `layout`: the word, its x and its line.
#[derive(Clone)]
pub struct Placed {
    pub word: String,
    pub x: f32,
    pub line: usize,
    pub width: f32,
}

type LayoutKey = (String, String, usize, u16, bool, usize);
static LAYOUTS: LazyLock<Mutex<HashMap<LayoutKey, Vec<Placed>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Breaks `text` into lines no wider than `max_w` and returns every word with
/// its x offset and line. `|` forces a line break.
#[allow(clippy::too_many_arguments)]
pub fn layout(
    frame: &mut Frame,
    ctx: &FFramesContext,
    text: &str,
    family: &str,
    size: usize,
    weight: u16,
    italic: bool,
    max_w: f32,
) -> Vec<Placed> {
    let key = (
        text.to_owned(),
        family.to_owned(),
        size,
        weight,
        italic,
        max_w as usize,
    );
    if let Some(p) = LAYOUTS.lock().unwrap().get(&key) {
        return p.clone();
    }
    let space = measure(frame, ctx, family, size, weight, italic, "a b")
        - measure(frame, ctx, family, size, weight, italic, "ab");
    let mut out = Vec::new();
    let mut line = 0;
    for (k, forced) in text.split('|').enumerate() {
        if k > 0 {
            line += 1;
        }
        let mut x = 0.0;
        for word in forced.split_whitespace() {
            let w = measure(frame, ctx, family, size, weight, italic, word);
            if x > 0.0 && x + w > max_w {
                line += 1;
                x = 0.0;
            }
            out.push(Placed {
                word: word.to_owned(),
                x,
                line,
                width: w,
            });
            x += w + space;
        }
    }
    LAYOUTS.lock().unwrap().insert(key, out.clone());
    out
}

/// Width of each line of a layout.
pub fn line_widths(placed: &[Placed]) -> Vec<f32> {
    let lines = placed.iter().map(|p| p.line).max().map_or(0, |l| l + 1);
    let mut widths = vec![0.0_f32; lines];
    for p in placed {
        widths[p.line] = widths[p.line].max(p.x + p.width);
    }
    widths
}

/// How a block of words is set.
#[derive(Clone, Copy)]
pub struct Type {
    pub family: &'static str,
    pub size: usize,
    pub weight: u16,
    pub italic: bool,
    pub fill: &'static str,
    pub line_h: f32,
    pub max_w: f32,
    pub center: bool,
}

impl Type {
    pub fn new(family: &'static str, size: usize, weight: u16, fill: &'static str) -> Self {
        Type {
            family,
            size,
            weight,
            italic: false,
            fill,
            line_h: size as f32 * 1.3,
            max_w: 1600.0,
            center: false,
        }
    }
    pub fn width(mut self, w: f32) -> Self {
        self.max_w = w;
        self
    }
    pub fn center(mut self) -> Self {
        self.center = true;
        self
    }
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }
    pub fn line_h(mut self, h: f32) -> Self {
        self.line_h = h;
        self
    }
}

/// Words that rise into place one after another, starting at `at`.
/// `x` is the left edge, or the center for centered type.
#[allow(clippy::too_many_arguments)]
pub fn rise(
    frame: &mut Frame,
    ctx: &FFramesContext,
    x: f32,
    y: f32,
    text: &str,
    ty: Type,
    at: f32,
    t: f32,
) -> Svgr<'static> {
    if t < at {
        return Svgr::empty();
    }
    let placed = layout(
        frame, ctx, text, ty.family, ty.size, ty.weight, ty.italic, ty.max_w,
    );
    let widths = line_widths(&placed);
    let style = if ty.italic { "italic" } else { "normal" };
    let words: Vec<Svgr> = placed
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let start = at + i as f32 * 0.05;
            let k = ease_out(prog(t, start, start + 0.45));
            if k <= 0.0 {
                return None;
            }
            let lx = if ty.center {
                x - widths[p.line] / 2.0
            } else {
                x
            };
            let wx = lx + p.x;
            let wy = y + p.line as f32 * ty.line_h + (1.0 - k) * 14.0;
            let word = p.word.clone();
            Some(fframes::svgr!(
                <text x={wx} y={wy} font-family={ty.family} font-size={ty.size} font-weight={ty.weight}
                      font-style={style} fill={ty.fill} opacity={k}>{word}</text>
            ))
        })
        .collect();
    fframes::svgr!(<g>{words}</g>)
}

/// A paragraph set one `<text>` per line, so the font spaces the words, not
/// our measurements. Lines fade up one after another from `at`.
#[allow(clippy::too_many_arguments)]
pub fn text_lines(
    frame: &mut Frame,
    ctx: &FFramesContext,
    x: f32,
    y: f32,
    text: &str,
    ty: Type,
    at: f32,
    t: f32,
) -> Svgr<'static> {
    if t < at {
        return Svgr::empty();
    }
    let placed = layout(
        frame, ctx, text, ty.family, ty.size, ty.weight, ty.italic, ty.max_w,
    );
    let n = placed.iter().map(|p| p.line).max().map_or(0, |l| l + 1);
    let style = if ty.italic { "italic" } else { "normal" };
    let anchor = if ty.center { "middle" } else { "start" };
    let lines: Vec<Svgr> = (0..n)
        .filter_map(|line| {
            let words: Vec<&str> = placed.iter().filter(|p| p.line == line).map(|p| p.word.as_str()).collect();
            if words.is_empty() {
                return None;
            }
            let start = at + line as f32 * 0.15;
            let k = ease_out(prog(t, start, start + 0.5));
            let text = words.join(" ");
            Some(fframes::svgr!(
                <text x={x} y={y + line as f32 * ty.line_h + (1.0 - k) * 10.0} text-anchor={anchor} font-family={ty.family}
                      font-size={ty.size} font-weight={ty.weight} font-style={style} fill={ty.fill} opacity={k}>{text}</text>
            ))
        })
        .collect();
    fframes::svgr!(<g>{lines}</g>)
}

/// Lines set at once (no motion), for blocks that move as a whole; one
/// `<text>` per line, like `text_lines`.
pub fn block(frame: &mut Frame, ctx: &FFramesContext, x: f32, y: f32, text: &str, ty: Type) -> Svgr<'static> {
    // started long ago: every line fully in
    text_lines(frame, ctx, x, y, text, ty, -1000.0, 0.0)
}

// ---------------------------------------------------------------------------
// text that types itself

/// Text that types itself at `cps` characters per second from `at`.
#[derive(Debug, Clone, Copy)]
pub struct Typing {
    pub at: f32,
    pub text: &'static str,
    pub cps: f32,
}

impl Typing {
    pub const fn new(at: f32, text: &'static str, cps: f32) -> Self {
        Typing { at, text, cps }
    }
    pub fn len(&self) -> usize {
        self.text.chars().count()
    }
    pub fn end(&self) -> f32 {
        self.at + self.len() as f32 / self.cps
    }
    pub fn count(&self, t: f32) -> usize {
        if t < self.at {
            return 0;
        }
        (((t - self.at) * self.cps) as usize).min(self.len())
    }
}

/// A typed paragraph wrapped like `layout`, with a caret that blinks for
/// `blink` seconds after the last character.
#[allow(clippy::too_many_arguments)]
pub fn typed_block(
    frame: &mut Frame,
    ctx: &FFramesContext,
    x: f32,
    y: f32,
    typing: &Typing,
    ty: Type,
    blink: f32,
    t: f32,
) -> Svgr<'static> {
    if t < typing.at {
        return Svgr::empty();
    }
    let placed = layout(
        frame, ctx, typing.text, ty.family, ty.size, ty.weight, ty.italic, ty.max_w,
    );
    let style = if ty.italic { "italic" } else { "normal" };
    let mut left = typing.count(t);
    let mut caret_at = (x, y);
    let mut words = Vec::new();
    for p in &placed {
        if left == 0 {
            break;
        }
        let n = p.word.chars().count();
        let take = n.min(left);
        left -= take;
        left = left.saturating_sub(1);
        let shown: String = p.word.chars().take(take).collect();
        let wx = x + p.x;
        let wy = y + p.line as f32 * ty.line_h;
        let w = if take == n {
            p.width
        } else {
            measure(frame, ctx, ty.family, ty.size, ty.weight, ty.italic, &shown)
        };
        caret_at = (wx + w, wy);
        words.push(fframes::svgr!(
            <text x={wx} y={wy} font-family={ty.family} font-size={ty.size} font-weight={ty.weight}
                  font-style={style} fill={ty.fill}>{shown}</text>
        ));
    }
    let since = t - typing.end();
    let caret_on = since < 0.0 || (since < blink && (since * 2.0).fract() < 0.55);
    let s = ty.size as f32;
    let caret_svg = if caret_on {
        fframes::svgr!(<rect x={caret_at.0 + s * 0.08} y={caret_at.1 - s * 0.74} width={(s * 0.06).max(2.5)} height={s * 0.92} fill={ty.fill} />)
    } else {
        Svgr::empty()
    };
    fframes::svgr!(<g>{words}{caret_svg}</g>)
}

// ---------------------------------------------------------------------------
// the page header

/// The sections, in order, for the header's progress marker.
pub const SECTIONS: [&str; 7] = ["The Problem", "The Setup", "Documents", "Mathematics", "Robot Code", "Results", "Conclusion"];

/// The header of section `n` (1-based): its number and `title` on the left,
/// one mark per section on the right, the current one lit.
pub fn header(n: usize, title: &str, o: f32) -> Svgr<'static> {
    if o <= 0.0 {
        return Svgr::empty();
    }
    let num = format!("{n:02}");
    let title = title.to_owned();
    let seg_w = 34.0;
    let gap = 8.0;
    let x0 = W - MX - SECTIONS.len() as f32 * (seg_w + gap) + gap;
    let marks: Vec<Svgr> = (1..=SECTIONS.len())
        .map(|i| {
            let fill = if i == n { ACCENT } else if i < n { GREY } else { RULE };
            let x = x0 + (i - 1) as f32 * (seg_w + gap);
            fframes::svgr!(<rect x={x} y="94" width={seg_w} height="4" rx="2" fill={fill} />)
        })
        .collect();
    fframes::svgr!(
        <g opacity={o}>
            <text x={MX} y="108" font-family={MONO} font-size="28" fill={ACCENT}>{num}</text>
            <text x={MX + 64.0} y="108" font-family={SERIF} font-size="46" font-weight="700" fill={INK}>{title}</text>
            {marks}
            <rect x={MX} y="138" width={W - 2.0 * MX} height="1.5" fill={RULE} />
        </g>
    )
}

// ---------------------------------------------------------------------------
// marks

/// A tick for a justified literal, drawn on with `p`, centred on (x, y), `s` high.
pub fn tick(x: f32, y: f32, s: f32, p: f32) -> Svgr<'static> {
    if p <= 0.0 {
        return Svgr::empty();
    }
    let d = format!(
        "M{} {} L{} {} L{} {}",
        x - s * 0.42,
        y + s * 0.02,
        x - s * 0.12,
        y + s * 0.32,
        x + s * 0.46,
        y - s * 0.34
    );
    let len = s * 1.25;
    fframes::svgr!(
        <path d={d} fill="none" stroke={OK} stroke-width={s * 0.13} stroke-linecap="round" stroke-linejoin="round"
              stroke-dasharray={len} stroke-dashoffset={len * (1.0 - clamp01(p))} />
    )
}

/// The underline of a hallucinated literal, drawn left to right over `w` pixels.
pub fn underline(x: f32, y: f32, w: f32, p: f32, thick: f32) -> Svgr<'static> {
    if p <= 0.0 {
        return Svgr::empty();
    }
    fframes::svgr!(<rect x={x} y={y} width={(w * ease_out(p)).max(0.5)} height={thick} fill={FLAG} />)
}

/// Mixes two `#rrggbb` colours.
pub fn mix(a: &str, b: &str, t: f32) -> String {
    let p = |s: &str, i: usize| u8::from_str_radix(&s[1 + i * 2..3 + i * 2], 16).unwrap_or(0) as f32;
    let t = clamp01(t);
    format!(
        "#{:02x}{:02x}{:02x}",
        lerp(p(a, 0), p(b, 0), t).round() as u8,
        lerp(p(a, 1), p(b, 1), t).round() as u8,
        lerp(p(a, 2), p(b, 2), t).round() as u8
    )
}

/// A mono label, small capitals spaced out.
pub fn label(x: f32, y: f32, text: &str, color: &'static str, size: f32, anchor: &'static str) -> Svgr<'static> {
    let text = text.to_owned();
    fframes::svgr!(
        <text x={x} y={y} font-family={MONO} font-size={size} font-weight="400" letter-spacing="1.5"
              fill={color} text-anchor={anchor}>{text}</text>
    )
}
