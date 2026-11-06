//! The captions of the AMDG film: one sentence per cue (`narration/captions.tsv`), no voice.
//! Each sentence stays up for its reading time (about 20 characters a
//! second, at least 3 s) and starts at its cue or just after the sentence
//! before it, whichever is later. Scenes ask `at(id)` when a sentence starts,
//! so their motion follows the captions. Captions show at most two lines.

use std::sync::LazyLock;

use fframes::{FFramesContext, FontQuery, Frame, Svgr};

use crate::ui::*;

/// Start of every scene in seconds.
pub const SCENE_START: [f32; 15] = [0.0, 13.0, 31.0, 48.0, 67.0, 84.0, 131.0, 158.0, 178.0, 194.0, 210.0, 235.0, 267.0, 287.0, 302.0];
pub const TOTAL: f32 = 314.0;

const CAPTIONS_TSV: &str = include_str!("../narration/captions.tsv");

/// Characters read per second.
const READ_CPS: f32 = 20.0;

/// (caption id, scene, cue in seconds into the scene)
const CUES: &[(&str, usize, f32)] = &[
    ("g01_1", 1, 0.8),
    ("g02_1", 2, 0.8),
    ("g02_2", 2, 1.0),
    ("g03_1", 3, 0.8),
    ("g03_2", 3, 1.0),
    ("g04_1", 4, 0.8),
    ("g04_2", 4, 1.0),
    ("g05_1", 5, 0.8),
    ("g06_0", 6, 0.8),
    ("g06_1", 6, 0.8),
    ("g06_2", 6, 1.0),
    ("g06_3", 6, 1.0),
    ("g06_4", 6, 1.0),
    ("g07_1", 7, 0.8),
    ("g07_2", 7, 1.0),
    ("g07_3", 7, 1.0),
    ("g07_4", 7, 1.0),
    ("g10_1", 8, 0.8),
    ("g10_2", 8, 1.0),
    ("g09_1", 10, 0.8),
    ("g09_2", 10, 1.0),
    ("g09_3", 10, 1.0),
    ("g03b_1", 11, 0.8),
    ("g03b_2", 11, 1.0),
    ("g03c_1", 12, 0.8),
    ("g03c_2", 12, 1.0),
    ("g11_1", 14, 0.8),
];

pub struct Line {
    pub id: &'static str,
    pub scene: usize,
    pub start: f32,
    pub end: f32,
    pub text: String,
}

pub static LINES: LazyLock<Vec<Line>> = LazyLock::new(|| {
    let text_of = |id: &str| {
        CAPTIONS_TSV
            .lines()
            .find(|l| l.split('\t').next() == Some(id))
            .and_then(|l| l.split('\t').nth(1))
            .unwrap_or("")
            .to_owned()
    };
    let mut out: Vec<Line> = Vec::new();
    let mut prev: Option<(usize, f32)> = None;
    for (id, scene, cue) in CUES {
        let text = text_of(id);
        let mut start = SCENE_START[scene - 1] + cue;
        if let Some((s, end)) = prev
            && s == *scene
        {
            start = start.max(end + 0.35);
        }
        let end = start + (text.chars().count() as f32 / READ_CPS).max(3.0);
        out.push(Line { id, scene: *scene, start, end, text });
        prev = Some((*scene, end));
    }
    out
});

/// When caption `id` starts, in seconds into its scene.
pub fn at(id: &str) -> f32 {
    LINES
        .iter()
        .find(|l| l.id == id)
        .map_or(0.0, |l| l.start - SCENE_START[l.scene - 1])
}

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
    // a gap shorter than a second would blank the band for a few frames:
    // keep the caption until the next one starts
    for i in 1..cues.len() {
        if cues[i].from - cues[i - 1].until < 1.0 {
            cues[i - 1].until = cues[i - 1].until.max(cues[i].from);
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
