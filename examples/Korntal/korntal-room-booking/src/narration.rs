//! Narration of the revised script, one clip per sentence (`narration/lines.tsv`,
//! made by `narration/make.sh`). Each sentence has a cue in its scene; it
//! starts at the cue or right after the sentence before it, whichever is later,
//! using the measured clip lengths in `narration/durations.tsv`. Captions show
//! the same sentences, at most two lines at a time.

use std::sync::LazyLock;

use fframes::{FFramesContext, FontQuery, Frame, Svgr};

use crate::ui::*;

/// Start of every scene in seconds (frame table of the revised script).
pub const SCENE_START: [f32; 8] = [0.0, 15.0, 45.0, 90.0, 140.0, 175.0, 205.0, 230.0];
pub const TOTAL: f32 = 240.0;

const LINES_TSV: &str = include_str!("../narration/lines.tsv");
const DURATIONS_TSV: &str = include_str!("../narration/durations.tsv");

/// (clip id, scene, cue in seconds into the scene)
const CUES: &[(&str, usize, f32)] = &[
    ("s1_1", 1, 0.5),
    ("s1_2", 1, 4.0),
    ("s2_1", 2, 0.8),
    ("s2_2", 2, 4.5),
    ("s2_3", 2, 11.5),
    ("s2_4", 2, 19.5),
    ("s3_1", 3, 0.8),
    ("s3_2", 3, 12.5),
    ("s3_3", 3, 22.0),
    ("s3_4", 3, 31.0),
    ("s4_1", 4, 0.8),
    ("s4_2", 4, 8.0),
    ("s4_3", 4, 17.5),
    ("s4_4", 4, 29.5),
    ("s4_5", 4, 39.5),
    ("s5_1", 5, 0.8),
    ("s5_2", 5, 9.5),
    ("s5_3", 5, 21.5),
    ("s6_1", 6, 0.8),
    ("s6_2", 6, 9.5),
    ("s6_3", 6, 19.5),
    ("s7_1", 7, 0.8),
    ("s7_2", 7, 10.5),
    ("s8_1", 8, 1.5),
];

pub struct Line {
    pub file: String,
    pub start: f32,
    pub end: f32,
    pub text: String,
}

pub static LINES: LazyLock<Vec<Line>> = LazyLock::new(|| {
    let text_of = |id: &str| {
        LINES_TSV
            .lines()
            .find(|l| l.split('\t').next() == Some(id))
            .and_then(|l| l.split('\t').nth(2))
            .unwrap_or("")
            .to_owned()
    };
    let dur_of = |id: &str| {
        DURATIONS_TSV
            .lines()
            .find(|l| l.split('\t').next() == Some(id))
            .and_then(|l| l.split('\t').nth(1))
            .and_then(|d| d.trim().parse::<f32>().ok())
            .unwrap_or(3.0)
    };
    let mut out: Vec<Line> = Vec::new();
    let mut prev: Option<(usize, f32)> = None;
    for (id, scene, cue) in CUES {
        let mut start = SCENE_START[scene - 1] + cue;
        if let Some((s, end)) = prev
            && s == *scene
        {
            start = start.max(end + 0.35);
        }
        let end = start + dur_of(id);
        out.push(Line { file: format!("{id}.wav"), start, end, text: text_of(id) });
        prev = Some((*scene, end));
    }
    out
});

/// One caption: a piece of a sentence short enough for two lines.
pub struct Cue {
    pub from: f32,
    pub until: f32,
    pub text: String,
}

const MAX_CHARS: usize = 140;

/// Splits a sentence near its middle, preferring a comma, colon or semicolon.
fn split(text: &str) -> Vec<String> {
    if text.chars().count() <= MAX_CHARS {
        return vec![text.to_owned()];
    }
    let mid = text.len() / 2;
    let mut best: Option<(usize, usize)> = None;
    for (i, c) in text.char_indices() {
        if c != ' ' {
            continue;
        }
        let prev = text[..i].chars().last().unwrap_or(' ');
        let punct = matches!(prev, ',' | ':' | ';');
        let dist = i.abs_diff(mid);
        let score = if punct { dist } else { dist + 40 };
        if best.is_none_or(|(s, _)| score < s) {
            best = Some((score, i));
        }
    }
    match best {
        Some((_, i)) => {
            let mut out = split(text[..i].trim());
            out.extend(split(text[i..].trim()));
            out
        }
        None => vec![text.to_owned()],
    }
}

pub static CUES_CAPTIONS: LazyLock<Vec<Cue>> = LazyLock::new(|| {
    let mut cues = Vec::new();
    for line in LINES.iter() {
        let parts = split(&line.text);
        let total: usize = parts.iter().map(|p| p.chars().count()).sum();
        let mut from = line.start;
        let dur = line.end - line.start;
        for (i, p) in parts.iter().enumerate() {
            let share = p.chars().count() as f32 / total.max(1) as f32;
            let until = if i + 1 == parts.len() { line.end + 0.35 } else { from + dur * share };
            cues.push(Cue { from, until, text: p.clone() });
            from = until;
        }
    }
    cues
});

/// The caption band at the bottom, inside the safe margin.
pub fn captions<'a>(frame: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let Some(cue) = CUES_CAPTIONS.iter().find(|c| t >= c.from && t < c.until) else {
        return Svgr::empty();
    };
    let near = CUES_CAPTIONS.iter().filter(|c| c.until > t - 0.5 && c.from < t + 0.5).count();
    let o = if near > 1 { 1.0 } else { window(t, cue.from, cue.until, 0.18) };
    let words = frame
        .text_break_lines(
            ctx,
            &cue.text,
            fframes::BreakLinesOpts {
                font: FontQuery { family: SANS, size: 34, weight: 400, ..Default::default() },
                width: 1560,
                line_height: 1.3,
                x: 180,
                y: 926,
                align: fframes::TextAlign::Center,
                fill: "#F3F6FF",
                dominant_baseline: "auto",
                text_anchor: "start",
                opacity: 1.0,
            },
        )
        .unwrap_or_else(Svgr::empty);
    fframes::svgr!(
        <g opacity={o}>
            <rect x="96" y="884" width="1728" height="104" rx="22" fill="#050916" opacity="0.82" stroke="#2A3566" stroke-width="1.5" />
            {words}
        </g>
    )
}
