//! One schedule for a narrated video: the scenes' lengths, the spoken lines with their start
//! and end, the captions cut from them and the voice tracks, resolved once from the data files
//! of the project and checked. Every time is in seconds from the start of the video.
//!
//! Inputs, as the examples keep them:
//!
//! - the length of every scene in seconds, in order;
//! - `lines.tsv`: one row per sentence, tab separated, the id in the first column and the
//!   caption text in `Input::text_column`;
//! - `durations.tsv`: id and measured length of the spoken clip in seconds (written by the
//!   project's `make.sh`), or a reading speed when there is no voice;
//! - cues: `(id, scene number counted from 1, seconds into the scene)`, in the order spoken.
//!
//! Policies ([`Policy`]), as the examples apply them: a line starts at its cue or
//! `gap_after_previous` after the line before it in the same scene, whichever is later; the
//! last caption of a line stays `caption_tail` after the voice ends; a sentence longer than
//! `max_caption_chars` is split near its middle, at a comma, colon or semicolon when there is
//! one; with `bridge_gaps_under` a caption is kept until the next one starts when the gap
//! between them would be shorter than that.
//!
//! Missing or unparsable data, an unknown scene, a duplicate id and a line running past the
//! end of its scene are errors: [`Schedule::build`] returns all of them at once, and the project
//! does not make a video from guessed values.

use std::fmt;

use fframes::{AudioTimestamp::*, AudioTrack};

/// Where the length of a spoken line comes from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DurationSource<'a> {
    /// The text of `durations.tsv`: id, tab, seconds, one clip per row.
    Measured(&'a str),
    /// No voice: the reading time of the caption text, at least `min_seconds`.
    Reading {
        chars_per_second: f32,
        min_seconds: f32,
    },
}

/// The timing rules, see the crate documentation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Policy {
    /// A line starts no earlier than this many seconds after the previous line of its scene.
    pub gap_after_previous: f32,
    /// The last caption of a line stays this many seconds after the line ends.
    pub caption_tail: f32,
    /// A sentence with more characters is split into several captions.
    pub max_caption_chars: usize,
    /// A caption stays until the next one starts when the gap would be shorter than this.
    pub bridge_gaps_under: Option<f32>,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            gap_after_previous: 0.35,
            caption_tail: 0.35,
            max_caption_chars: 140,
            bridge_gaps_under: None,
        }
    }
}

/// The data files and settings of one video.
#[derive(Debug, Clone, Copy)]
pub struct Input<'a> {
    /// Length of every scene in seconds, in order.
    pub scene_seconds: &'a [f32],
    /// The text of `lines.tsv`.
    pub lines_tsv: &'a str,
    /// The tab separated column of `lines_tsv` with the caption text (the id is column 0).
    pub text_column: usize,
    pub durations: DurationSource<'a>,
    /// `(id, scene number counted from 1, seconds into the scene)`, in the order spoken.
    pub cues: &'a [(&'a str, usize, f32)],
    pub policy: Policy,
}

/// A spoken line, resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub id: String,
    /// Index of the scene, the cue's number minus one.
    pub scene: usize,
    /// The cue, seconds into the scene.
    pub cue: f32,
    /// Seconds from the start of the video.
    pub start: f32,
    pub end: f32,
    /// The clip, `<id>.wav`.
    pub file: String,
    pub text: String,
}

/// One caption: a sentence or a piece of one, short enough for the band.
#[derive(Debug, Clone, PartialEq)]
pub struct Caption {
    /// Index into [`Schedule::lines`].
    pub line: usize,
    pub from: f32,
    pub until: f32,
    pub text: String,
}

/// What stops a schedule from being built.
#[derive(Debug, Clone, PartialEq)]
pub enum Problem {
    NoScenes,
    SceneLength {
        scene: usize,
        seconds: f32,
    },
    UnknownScene {
        id: String,
        scene: usize,
        scenes: usize,
    },
    NegativeCue {
        id: String,
        cue: f32,
    },
    DuplicateCue {
        id: String,
    },
    MissingText {
        id: String,
    },
    MissingDuration {
        id: String,
    },
    BadDuration {
        id: String,
        value: String,
    },
    /// The line ends after its scene; `end` and `scene_end` in seconds of the video.
    Overflow {
        id: String,
        scene: usize,
        end: f32,
        scene_end: f32,
    },
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Problem::NoScenes => write!(f, "no scenes"),
            Problem::SceneLength { scene, seconds } => {
                write!(f, "scene {} has a length of {seconds} s", scene + 1)
            }
            Problem::UnknownScene { id, scene, scenes } => {
                write!(f, "{id}: scene {scene} does not exist, there are {scenes}")
            }
            Problem::NegativeCue { id, cue } => write!(f, "{id}: cue {cue} s is before its scene"),
            Problem::DuplicateCue { id } => write!(f, "{id}: cued twice"),
            Problem::MissingText { id } => write!(f, "{id}: no text in lines.tsv"),
            Problem::MissingDuration { id } => write!(f, "{id}: no duration in durations.tsv"),
            Problem::BadDuration { id, value } => {
                write!(f, "{id}: duration \"{value}\" is not a number of seconds")
            }
            Problem::Overflow {
                id,
                scene,
                end,
                scene_end,
            } => write!(
                f,
                "{id}: ends at {end:.2} s, after scene {} ends at {scene_end:.2} s",
                scene + 1
            ),
        }
    }
}

/// The problems, one per line, for a panic message or a test.
pub fn problems_text(problems: &[Problem]) -> String {
    problems.iter().map(|p| format!("- {p}\n")).collect()
}

/// The resolved timing of a video.
#[derive(Debug, Clone, PartialEq)]
pub struct Schedule {
    /// Start of every scene in seconds.
    pub scene_starts: Vec<f32>,
    /// Length of the video in seconds.
    pub total: f32,
    /// In cue order.
    pub lines: Vec<Line>,
    /// In time order.
    pub captions: Vec<Caption>,
}

impl Schedule {
    /// Resolves the schedule, or every problem found in the input.
    pub fn build(input: &Input<'_>) -> Result<Schedule, Vec<Problem>> {
        let mut problems = Vec::new();

        if input.scene_seconds.is_empty() {
            problems.push(Problem::NoScenes);
        }
        for (scene, &seconds) in input.scene_seconds.iter().enumerate() {
            if !(seconds > 0.0) {
                problems.push(Problem::SceneLength { scene, seconds });
            }
        }
        let mut scene_starts = Vec::with_capacity(input.scene_seconds.len());
        let mut total = 0.0;
        for &seconds in input.scene_seconds {
            scene_starts.push(total);
            total += seconds;
        }

        let text_of = |id: &str| -> Option<String> {
            input
                .lines_tsv
                .lines()
                .find(|row| row.split('\t').next() == Some(id))
                .and_then(|row| row.split('\t').nth(input.text_column))
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_owned)
        };
        let duration_of = |id: &str, text: &str| -> Result<f32, Problem> {
            match input.durations {
                DurationSource::Measured(tsv) => {
                    let value = tsv
                        .lines()
                        .find(|row| row.split('\t').next() == Some(id))
                        .and_then(|row| row.split('\t').nth(1))
                        .ok_or_else(|| Problem::MissingDuration { id: id.to_owned() })?;
                    value
                        .trim()
                        .parse::<f32>()
                        .ok()
                        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
                        .ok_or_else(|| Problem::BadDuration {
                            id: id.to_owned(),
                            value: value.trim().to_owned(),
                        })
                }
                DurationSource::Reading {
                    chars_per_second,
                    min_seconds,
                } => Ok((text.chars().count() as f32 / chars_per_second).max(min_seconds)),
            }
        };

        let mut lines: Vec<Line> = Vec::with_capacity(input.cues.len());
        let mut previous: Option<(usize, f32)> = None;
        for &(id, scene_number, cue) in input.cues {
            if lines.iter().any(|line| line.id == id) {
                problems.push(Problem::DuplicateCue { id: id.to_owned() });
                continue;
            }
            let Some(scene) = scene_number
                .checked_sub(1)
                .filter(|scene| *scene < input.scene_seconds.len())
            else {
                problems.push(Problem::UnknownScene {
                    id: id.to_owned(),
                    scene: scene_number,
                    scenes: input.scene_seconds.len(),
                });
                continue;
            };
            if !(cue >= 0.0) {
                problems.push(Problem::NegativeCue {
                    id: id.to_owned(),
                    cue,
                });
                continue;
            }
            let Some(text) = text_of(id) else {
                problems.push(Problem::MissingText { id: id.to_owned() });
                continue;
            };
            let duration = match duration_of(id, &text) {
                Ok(duration) => duration,
                Err(problem) => {
                    problems.push(problem);
                    continue;
                }
            };

            let mut start = scene_starts[scene] + cue;
            if let Some((previous_scene, previous_end)) = previous
                && previous_scene == scene
            {
                start = start.max(previous_end + input.policy.gap_after_previous);
            }
            let end = start + duration;
            let scene_end = scene_starts[scene] + input.scene_seconds[scene];
            if end > scene_end {
                problems.push(Problem::Overflow {
                    id: id.to_owned(),
                    scene,
                    end,
                    scene_end,
                });
            }
            lines.push(Line {
                id: id.to_owned(),
                scene,
                cue,
                start,
                end,
                file: format!("{id}.wav"),
                text,
            });
            previous = Some((scene, end));
        }

        if !problems.is_empty() {
            return Err(problems);
        }

        let captions = captions(&lines, &input.policy);
        Ok(Schedule {
            scene_starts,
            total,
            lines,
            captions,
        })
    }

    /// Start of scene `index` in seconds.
    pub fn scene_start(&self, index: usize) -> f32 {
        self.scene_starts[index]
    }

    /// End of scene `index` in seconds (the start of the next, or the total).
    pub fn scene_end(&self, index: usize) -> f32 {
        self.scene_starts
            .get(index + 1)
            .copied()
            .unwrap_or(self.total)
    }

    /// Index of the scene playing at `t` seconds (the first scene before the video starts).
    pub fn scene_at(&self, t: f32) -> usize {
        self.scene_starts
            .iter()
            .rposition(|start| t >= *start)
            .unwrap_or(0)
    }

    pub fn line(&self, id: &str) -> Option<&Line> {
        self.lines.iter().find(|line| line.id == id)
    }

    /// When line `id` starts, in seconds into its scene; `None` for an unknown id.
    pub fn line_start_in_scene(&self, id: &str) -> Option<f32> {
        self.line(id)
            .map(|line| line.start - self.scene_starts[line.scene])
    }

    /// The caption shown at `t` seconds.
    pub fn caption_at(&self, t: f32) -> Option<&Caption> {
        self.captions.iter().find(|c| t >= c.from && t < c.until)
    }

    /// How many captions are shown within `within` seconds around `t`: more than one means
    /// a change is near and the band should not fade between them.
    pub fn captions_near(&self, t: f32, within: f32) -> usize {
        self.captions
            .iter()
            .filter(|c| c.until > t - within && c.from < t + within)
            .count()
    }

    /// One voice track per line, from its start to the end of its clip.
    pub fn tracks(&self) -> Vec<AudioTrack<'_>> {
        self.lines
            .iter()
            .map(|line| AudioTrack::new(line.file.as_str(), Second(line.start)..Eof).voice())
            .collect()
    }
}

/// Splits a sentence longer than `max_chars` near its middle, preferring a comma, colon or
/// semicolon, until every piece fits.
fn split(text: &str, max_chars: usize) -> Vec<String> {
    if text.chars().count() <= max_chars {
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
            let mut out = split(text[..i].trim(), max_chars);
            out.extend(split(text[i..].trim(), max_chars));
            out
        }
        None => vec![text.to_owned()],
    }
}

/// The captions of every line: the pieces share the line's time by their length, the last one
/// keeps the tail; then the gaps under the bridging limit are closed.
fn captions(lines: &[Line], policy: &Policy) -> Vec<Caption> {
    let mut captions = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let parts = split(&line.text, policy.max_caption_chars);
        let total: usize = parts.iter().map(|p| p.chars().count()).sum();
        let mut from = line.start;
        let duration = line.end - line.start;
        for (i, part) in parts.iter().enumerate() {
            let share = part.chars().count() as f32 / total.max(1) as f32;
            let until = if i + 1 == parts.len() {
                line.end + policy.caption_tail
            } else {
                from + duration * share
            };
            captions.push(Caption {
                line: index,
                from,
                until,
                text: part.clone(),
            });
            from = until;
        }
    }
    if let Some(limit) = policy.bridge_gaps_under {
        for i in 1..captions.len() {
            if captions[i].from - captions[i - 1].until < limit {
                captions[i - 1].until = captions[i - 1].until.max(captions[i].from);
            }
        }
    }
    captions
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINES: &str = "a1\tVoice\tFirst sentence.\tFirst sentence.\n\
        a2\tVoice\tSecond sentence, a bit longer.\tSecond sentence, a bit longer.\n\
        b1\tVoice\tThird.\tThird.\n";
    const DURATIONS: &str = "a1\t2.0\na2\t3.0\nb1\t1.5\n";

    fn input<'a>(cues: &'a [(&'a str, usize, f32)], policy: Policy) -> Input<'a> {
        Input {
            scene_seconds: &[10.0, 5.0],
            lines_tsv: LINES,
            text_column: 2,
            durations: DurationSource::Measured(DURATIONS),
            cues,
            policy,
        }
    }

    #[test]
    fn lines_follow_their_cues_and_each_other() {
        let cues = [("a1", 1, 0.5), ("a2", 1, 1.0), ("b1", 2, 0.8)];
        let s = Schedule::build(&input(&cues, Policy::default())).unwrap();
        assert_eq!(s.scene_starts, vec![0.0, 10.0]);
        assert_eq!(s.total, 15.0);
        // a1: 0.5..2.5; a2 is cued at 1.0 but waits 0.35 s after a1: 2.85..5.85
        assert_eq!((s.lines[0].start, s.lines[0].end), (0.5, 2.5));
        assert_eq!((s.lines[1].start, s.lines[1].end), (2.85, 5.85));
        // a new scene starts at its cue
        assert_eq!((s.lines[2].start, s.lines[2].end), (10.8, 12.3));
        assert_eq!(s.lines[2].file, "b1.wav");
        let into_scene = s.line_start_in_scene("b1").unwrap();
        assert!((into_scene - 0.8).abs() < 1e-5, "{into_scene}");
        assert_eq!(s.scene_at(10.0), 1);
        assert_eq!(s.scene_end(0), 10.0);
        assert_eq!(s.scene_end(1), 15.0);
        assert_eq!(s.tracks().len(), 3);
    }

    #[test]
    fn captions_keep_the_tail_and_bridge_short_gaps_only_when_asked() {
        let cues = [("a1", 1, 0.5), ("a2", 1, 3.0)];
        let s = Schedule::build(&input(&cues, Policy::default())).unwrap();
        // a1 ends at 2.5, its caption at 2.85; a2 starts at 3.0: a gap of 0.15 s
        assert_eq!((s.captions[0].from, s.captions[0].until), (0.5, 2.85));
        assert_eq!(s.captions[1].from, 3.0);
        assert!(s.caption_at(2.9).is_none());
        assert_eq!(s.captions_near(2.9, 0.5), 2);

        let bridged = Schedule::build(&input(
            &cues,
            Policy {
                bridge_gaps_under: Some(1.0),
                ..Policy::default()
            },
        ))
        .unwrap();
        assert_eq!(bridged.captions[0].until, 3.0);
        assert_eq!(bridged.caption_at(2.9).map(|c| c.line), Some(0));
    }

    #[test]
    fn long_sentences_are_split_at_punctuation() {
        let cues = [("a2", 1, 0.0)];
        let policy = Policy {
            max_caption_chars: 20,
            ..Policy::default()
        };
        let s = Schedule::build(&input(&cues, policy)).unwrap();
        let texts: Vec<&str> = s.captions.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(texts, ["Second sentence,", "a bit longer."]);
        assert_eq!(s.captions[0].from, 0.0);
        assert_eq!(s.captions[1].from, s.captions[0].until);
        assert_eq!(s.captions[1].until, 3.35);
    }

    #[test]
    fn reading_time_when_there_is_no_voice() {
        let cues = [("a1", 1, 0.0), ("a2", 1, 4.0)];
        let s = Schedule::build(&Input {
            durations: DurationSource::Reading {
                chars_per_second: 10.0,
                min_seconds: 3.0,
            },
            ..input(&cues, Policy::default())
        })
        .unwrap();
        // 15 characters at 10 per second is under the 3 s minimum; 30 characters take 3.0 s
        assert_eq!(s.lines[0].end, 3.0);
        assert_eq!(s.lines[1].end - s.lines[1].start, 3.0);
    }

    #[test]
    fn every_problem_is_reported_at_once() {
        let cues = [
            ("a1", 1, 0.5),
            ("a1", 1, 3.0),
            ("zz", 1, 0.0),
            ("a2", 3, 0.0),
            ("b1", 2, 4.0),
            ("a2", 1, -1.0),
        ];
        let problems = Schedule::build(&input(&cues, Policy::default())).unwrap_err();
        assert_eq!(
            problems,
            vec![
                Problem::DuplicateCue { id: "a1".into() },
                Problem::MissingText { id: "zz".into() },
                Problem::UnknownScene {
                    id: "a2".into(),
                    scene: 3,
                    scenes: 2
                },
                Problem::Overflow {
                    id: "b1".into(),
                    scene: 1,
                    end: 15.5,
                    scene_end: 15.0
                },
                Problem::NegativeCue {
                    id: "a2".into(),
                    cue: -1.0
                },
            ]
        );
        assert!(problems_text(&problems).contains("b1: ends at 15.50 s"));

        let no_duration = Schedule::build(&Input {
            durations: DurationSource::Measured("a1\tlong\n"),
            ..input(&[("a1", 1, 0.0), ("a2", 1, 1.0)], Policy::default())
        })
        .unwrap_err();
        assert_eq!(
            no_duration,
            vec![
                Problem::BadDuration {
                    id: "a1".into(),
                    value: "long".into()
                },
                Problem::MissingDuration { id: "a2".into() },
            ]
        );
    }
}
