//! Palette, type and the pieces every scene shares: the keyword column, the
//! photo frame, the color grade and the ECG line.

use fframes::Svgr;

use crate::timing::*;

pub const BG: &str = "#0d1315";
pub const BONE: &str = "#ece8e1";
pub const PAPER: &str = "#efe9dd";
pub const INK: &str = "#14201f";
pub const GREY: &str = "#7c8582";
pub const DIM: &str = "#26302f";
/// Lake water, the accent of the clinic.
pub const LAKE: &str = "#5fb3bf";
/// Morning sun: highlights and the route.
pub const SUN: &str = "#f0a243";
/// The tired heart.
pub const EMBER: &str = "#e2553f";

pub const DISPLAY: &str = "Archivo Black";
pub const MONO: &str = "IBM Plex Mono";
pub const SERIF: &str = "Instrument Serif";

#[derive(Debug, Clone, Copy)]
pub enum Face {
    Display,
    Serif,
    Mono,
}

/// One word of a keyword column.
#[derive(Debug, Clone, Copy)]
pub struct Kw {
    pub text: &'static str,
    pub face: Face,
    pub size: f32,
}

pub const fn kw(text: &'static str, face: Face, size: f32) -> Kw {
    Kw { text, face, size }
}

/// A brainstorm column: words of different faces and sizes scroll upwards.
/// `pos` is the (fractional) index of the word on the centre line `cy`; words
/// above it fade with distance, words below it are not there yet and rise in
/// as `pos` reaches them. The word on the centre line takes the accent.
pub fn keyword_column(words: &[Kw], pos: f32, x: f32, cy: f32, accent: &'static str, alpha: f32) -> Svgr<'static> {
    if words.is_empty() || alpha <= 0.0 || pos < -0.95 {
        return Svgr::empty();
    }
    let mut offs = Vec::with_capacity(words.len());
    let mut y = 0.0;
    for (i, w) in words.iter().enumerate() {
        if i > 0 {
            y += (words[i - 1].size + w.size) * 0.56;
        }
        offs.push(y);
    }
    let last = (words.len() - 1) as f32;
    let p = pos.clamp(0.0, last);
    let i0 = p.floor() as usize;
    let i1 = (i0 + 1).min(words.len() - 1);
    let at = lerp(offs[i0], offs[i1], p - i0 as f32) + (pos.min(0.0)) * -words[0].size * 1.1;
    let current = pos.round() as i32;
    let items: Vec<Svgr> = words
        .iter()
        .enumerate()
        .filter_map(|(i, w)| {
            let fi = i as f32;
            let dy = offs[i] - at;
            let near = (1.0 - (dy / 440.0).abs()).clamp(0.0, 1.0);
            let mut o = near.powf(1.6) * alpha;
            if fi > pos {
                // not spoken yet: rises in over the last half step
                o *= clamp01(1.0 - (fi - pos) * 2.0);
            }
            if o < 0.01 {
                return None;
            }
            let hot = i as i32 == current;
            let fill = if hot { accent } else { BONE };
            let o = if hot { o } else { o * 0.5 };
            let baseline = cy + dy + w.size * 0.34;
            let shift = (1.0 - near) * 40.0;
            let (family, weight, style, spacing, text) = match w.face {
                Face::Display => (DISPLAY, 400, "normal", -w.size * 0.03, w.text.to_uppercase()),
                Face::Serif => (SERIF, 400, "italic", 0.0, w.text.to_owned()),
                Face::Mono => (MONO, 500, "normal", w.size * 0.12, w.text.to_uppercase()),
            };
            Some(fframes::svgr!(
                <text x={x + shift} y={baseline} font-family={family} font-weight={weight} font-style={style}
                      font-size={w.size} letter-spacing={spacing} fill={fill} opacity={o}>{text}</text>
            ))
        })
        .collect();
    fframes::svgr!(
        <g>
            <rect x={x - 40.0} y={cy - 1.5} width="22" height="3" fill={accent} opacity={alpha} />
            {items}
        </g>
    )
}

/// About how wide a keyword is set, for laying words out in a row.
fn kw_width(w: &Kw) -> f32 {
    let per_char = match w.face {
        Face::Display => 0.74,
        Face::Serif => 0.4,
        Face::Mono => 0.72,
    };
    w.text.chars().count() as f32 * w.size * per_char
}

/// The keyword column turned on its side: words run in one line and slide to
/// the left, the word at `pos` centred on `cx`. Spoken words fade with distance
/// to the left, words not spoken yet slide in from the right.
pub fn keyword_row(words: &[Kw], pos: f32, cx: f32, baseline: f32, accent: &'static str, alpha: f32) -> Svgr<'static> {
    if words.is_empty() || alpha <= 0.0 || pos < -0.95 {
        return Svgr::empty();
    }
    const GAP: f32 = 70.0;
    let mut mids = Vec::with_capacity(words.len());
    let mut x = 0.0;
    for w in words {
        let wd = kw_width(w);
        mids.push(x + wd / 2.0);
        x += wd + GAP;
    }
    let last = (words.len() - 1) as f32;
    let p = pos.clamp(0.0, last);
    let i0 = p.floor() as usize;
    let i1 = (i0 + 1).min(words.len() - 1);
    let at = lerp(mids[i0], mids[i1], p - i0 as f32) + pos.min(0.0) * -400.0;
    let current = pos.round() as i32;
    let items: Vec<Svgr> = words
        .iter()
        .enumerate()
        .filter_map(|(i, w)| {
            let fi = i as f32;
            let dx = mids[i] - at;
            let near = (1.0 - (dx / 900.0).abs()).clamp(0.0, 1.0);
            let mut o = near.powf(1.4) * alpha;
            if fi > pos {
                o *= clamp01(1.0 - (fi - pos) * 2.0);
            }
            if o < 0.01 {
                return None;
            }
            let hot = i as i32 == current;
            let fill = if hot { accent } else { BONE };
            let o = if hot { o } else { o * 0.5 };
            let (family, weight, style, spacing, text) = match w.face {
                Face::Display => (DISPLAY, 400, "normal", -w.size * 0.03, w.text.to_uppercase()),
                Face::Serif => (SERIF, 400, "italic", 0.0, w.text.to_owned()),
                Face::Mono => (MONO, 500, "normal", w.size * 0.12, w.text.to_uppercase()),
            };
            Some(fframes::svgr!(
                <text x={cx + dx} y={baseline} text-anchor="middle" font-family={family} font-weight={weight} font-style={style}
                      font-size={w.size} letter-spacing={spacing} fill={fill} opacity={o}>{text}</text>
            ))
        })
        .collect();
    fframes::svgr!(
        <g>
            <rect x={cx - 11.0} y={baseline + 34.0} width="22" height="3" fill={accent} opacity={alpha} />
            {items}
        </g>
    )
}

/// A small mono label with wide tracking.
pub fn label(x: f32, y: f32, text: String, color: &'static str, size: f32, anchor: &'static str) -> Svgr<'static> {
    fframes::svgr!(
        <text x={x} y={y} font-family={MONO} font-weight="500" font-size={size} letter-spacing="2.5"
              fill={color} text-anchor={anchor}>{text}</text>
    )
}

/// Desaturate and tint the rectangle below (blend modes over what is drawn there).
pub fn grade(x: f32, y: f32, w: f32, h: f32, desat: f32, tint: &'static str, tint_amount: f32) -> Svgr<'static> {
    fframes::svgr!(
        <g>
            <rect x={x} y={y} width={w} height={h} fill="#808080" style="mix-blend-mode:saturation" opacity={desat} />
            <rect x={x} y={y} width={w} height={h} fill={tint} style="mix-blend-mode:multiply" opacity={tint_amount} />
        </g>
    )
}

/// Crop-mark corners around a rectangle.
pub fn corners(x: f32, y: f32, w: f32, h: f32, arm: f32, color: &'static str, width: f32) -> Svgr<'static> {
    let d = format!(
        "M{x} {y1} V{y} H{x1} M{x2} {y} H{x3} V{y1} M{x3} {y2} V{y3} H{x2} M{x1} {y3} H{x} V{y2}",
        x = x,
        y = y,
        x1 = x + arm,
        x2 = x + w - arm,
        x3 = x + w,
        y1 = y + arm,
        y2 = y + h - arm,
        y3 = y + h,
    );
    fframes::svgr!(<path d={d} fill="none" stroke={color} stroke-width={width} />)
}

// ---------------------------------------------------------------------------
// ECG

/// One heartbeat, `ph` in 0..1: P wave, QRS, T wave. Up is positive.
fn beat_shape(ph: f32) -> f32 {
    let g = |c: f32, w: f32, a: f32| a * (-((ph - c) / w).powi(2)).exp();
    g(0.12, 0.025, 0.12) + g(0.235, 0.008, -0.15) + g(0.25, 0.010, 1.0) + g(0.27, 0.010, -0.3) + g(0.45, 0.045, 0.25)
}

/// The signal at time `tau`: `irregular` 0 is a calm sinus rhythm, 1 shifts and
/// scales beats and adds noise.
pub fn ecg_signal(tau: f32, bpm: f32, irregular: f32) -> f32 {
    let beats = tau * bpm / 60.0;
    let k = beats.floor();
    let shift = (hash(k * 3.1) - 0.5) * irregular * 0.45;
    let ph = (beats - k - shift).rem_euclid(1.0);
    let amp = 1.0 + (hash(k * 7.7) - 0.5) * irregular * 0.9;
    beat_shape(ph) * amp + (hash((tau * 90.0).floor()) - 0.5) * 0.06 * irregular
}

/// The ECG line along the bottom: the newest sample on the right, older ones
/// fade out to the left.
pub fn ecg_line(now: f32, bpm: f32, irregular: f32, color: &'static str, alpha: f32) -> Svgr<'static> {
    if alpha <= 0.0 {
        return Svgr::empty();
    }
    let (x0, x1, base, amp, window) = (150.0, 1770.0, 1008.0, 46.0, 4.5);
    let n = 280;
    let mut d = String::with_capacity(n * 16);
    let mut last = (x1, base);
    for i in 0..=n {
        let x = x0 + (x1 - x0) * i as f32 / n as f32;
        let tau = now - (x1 - x) / (x1 - x0) * window;
        let y = base - ecg_signal(tau, bpm, irregular) * amp;
        d.push_str(if i == 0 { "M" } else { " L" });
        d.push_str(&format!("{x:.1} {y:.1}"));
        last = (x, y);
    }
    fframes::svgr!(
        <g opacity={alpha}>
            <defs>
                <linearGradient id="ecg-fade" gradientUnits="userSpaceOnUse" x1="150" y1="0" x2="1770" y2="0">
                    <stop offset="0" stop-color={color} stop-opacity="0" />
                    <stop offset="0.6" stop-color={color} stop-opacity="0.55" />
                    <stop offset="1" stop-color={color} stop-opacity="1" />
                </linearGradient>
            </defs>
            <path d={d} fill="none" stroke="url(#ecg-fade)" stroke-width="2.4" stroke-linejoin="round" />
            <circle cx={last.0} cy={last.1} r="5" fill={color} />
            <circle cx={last.0} cy={last.1} r="13" fill={color} opacity="0.18" />
        </g>
    )
}
