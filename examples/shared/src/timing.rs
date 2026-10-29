//! The clock of the video. Scenes and narration lines are placed in seconds of
//! the whole video; cue times inside a scene come from the pauses measured in
//! the voice files (`ffmpeg -af silencedetect`).

use fframes::Frame;

pub const FPS: usize = 30;
/// All times in this file and the scenes are design seconds; the film plays
/// them at this speed (the pace the user chose). Real seconds = design / SPEED.
pub const SPEED: f32 = 1.25;

/// Real seconds of the film for a design second, for the audio map.
pub fn real(design: f32) -> f32 {
    design / SPEED
}

pub const GARY: (f32, f32) = (0.0, 42.6);
/// The map plays at 1x inside the 1.25x film: 20.5 s of its own = 25.6 design.
pub const GEO: (f32, f32) = (42.6, 68.2);
pub const TIME: (f32, f32) = (68.2, 91.2);
pub const DOCTORS: (f32, f32) = (91.2, 171.2);
pub const SERVICES: (f32, f32) = (171.2, 197.8);
pub const FUTURE: (f32, f32) = (197.8, 209.3);
pub const BACK: (f32, f32) = (209.3, 223.3);
pub const CREDITS: (f32, f32) = (223.3, 233.3);

/// Narration lines (dynamic_media/vo_*.wav, text in narration.txt) and where they start.
pub const VOICE: &[(&str, f32)] = &[
    ("vo_01_gary.wav", 0.8),
    ("vo_02_tired.wav", 6.6),
    ("vo_03_north.wav", 56.1),
    ("vo_04_haussee.wav", 62.7),
    ("vo_05_since.wav", 81.2),
    ("vo_06_years.wav", 91.0),
    ("vo_07_doctors.wav", 115.0),
    ("vo_08_services.wav", 129.6),
    ("vo_09_back.wav", 142.7),
    ("vo_10_clinic.wav", 151.1),
];

/// Design seconds since the start of the video, whichever scene the frame belongs to.
pub fn gsec(frame: &Frame) -> f32 {
    frame.global_index as f32 / FPS as f32 * SPEED
}

pub fn span_frames(span: (f32, f32)) -> usize {
    (real(span.1) * FPS as f32).round() as usize - (real(span.0) * FPS as f32).round() as usize
}

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

pub fn expo_out(t: f32) -> f32 {
    let t = clamp01(t);
    if t >= 1.0 { 1.0 } else { 1.0 - 2f32.powf(-10.0 * t) }
}

pub fn expo_in(t: f32) -> f32 {
    let t = clamp01(t);
    if t <= 0.0 { 0.0 } else { 2f32.powf(10.0 * t - 10.0) }
}

pub fn cubic_in_out(t: f32) -> f32 {
    let t = clamp01(t);
    if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}

pub fn quart_out(t: f32) -> f32 {
    1.0 - (1.0 - clamp01(t)).powi(4)
}

/// Damped spring from 0 to 1 (mass 1), `t` in seconds since release.
pub fn spring(t: f32, stiffness: f32, damping: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    let w0 = stiffness.sqrt();
    let zeta = damping / (2.0 * w0);
    if zeta < 1.0 {
        let wd = w0 * (1.0 - zeta * zeta).sqrt();
        1.0 - (-zeta * w0 * t).exp() * ((wd * t).cos() + zeta * w0 / wd * (wd * t).sin())
    } else {
        1.0 - (1.0 + w0 * t) * (-w0 * t).exp()
    }
}

/// A soft spring with a small overshoot, `t` in seconds.
pub fn soft(t: f32) -> f32 {
    spring(t, 120.0, 14.0)
}

/// A value that moves to the next target at each key: `(time, target)`,
/// easing over `dur` seconds. Before the first key it is `start`.
pub fn steps(t: f32, start: f32, keys: &[(f32, f32)], dur: f32) -> f32 {
    let mut v = start;
    for &(at, to) in keys {
        if t <= at {
            break;
        }
        v = lerp(v, to, quart_out(prog(t, at, at + dur)));
    }
    v
}

/// Deterministic hash noise, 0..1.
pub fn hash(n: f32) -> f32 {
    let x = (n * 127.1 + 311.7).sin() * 43_758.547;
    x - x.floor()
}
