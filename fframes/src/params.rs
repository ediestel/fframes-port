//! The numbers and words of a scene in a data file instead of Rust constants, read once and
//! checked, so an angle, a timing or a label changes without touching the code. The file is
//! TOML:
//!
//! ```toml
//! # values: numbers and words, read with `number` and `text`
//! [values]
//! trunk_sit = 46.0
//! title = "DIX-HALLPIKE TEST"
//!
//! # tracks: a number over time, read with `at`; every key is [seconds, value] or
//! # [seconds, value, easing], the easing being how the value reaches this key from the key
//! # before it: "linear" (the default), "ease-in", "ease-out", "ease-in-out", or "hold" (the
//! # previous value until this key)
//! [tracks]
//! trunk = [[6.4, 46.0], [7.4, -40.0, "ease-in-out"]]
//! ```
//!
//! Before its first key a track holds the first value, after its last key the last one. A
//! track without keys, keys out of time order, an unknown easing, a key that is not
//! `[seconds, value, easing?]` and a value that is neither a number nor a word are errors,
//! all listed by [`Params::parse`]; nothing is guessed.

use std::collections::BTreeMap;
use std::fmt;

use crate::animation::{AnimationRuntime, Easing};

/// A static value of the `[values]` table.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f32),
    Text(String),
}

/// How a key is reached from the key before it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeyEasing {
    /// The previous value until this key's time, then this value.
    Hold,
    Ease(Easing),
}

impl KeyEasing {
    /// The easing a key names; `None` for an unknown name.
    pub fn named(name: &str) -> Option<Self> {
        Some(match name {
            "linear" => KeyEasing::Ease(Easing::Linear),
            "ease-in" => KeyEasing::Ease(Easing::EaseIn),
            "ease-out" => KeyEasing::Ease(Easing::EaseOut),
            "ease-in-out" => KeyEasing::Ease(Easing::EaseInOut),
            "hold" => KeyEasing::Hold,
            _ => return None,
        })
    }
}

/// One key of a track.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Key {
    pub t: f32,
    pub value: f32,
    pub easing: KeyEasing,
}

/// A number over time.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    /// At least one key, in time order.
    pub keys: Vec<Key>,
}

impl Track {
    /// The value at `t` seconds.
    pub fn at(&self, t: f32) -> f32 {
        let keys = &self.keys;
        let Some(first) = keys.first() else {
            return 0.0;
        };
        if t <= first.t {
            return first.value;
        }
        let Some(next) = keys.iter().position(|key| key.t > t) else {
            return keys[keys.len() - 1].value;
        };
        let (from, to) = (&keys[next - 1], &keys[next]);
        match to.easing {
            KeyEasing::Hold => from.value,
            KeyEasing::Ease(easing) => {
                let duration = to.t - from.t;
                let progress = AnimationRuntime::new(duration, &easing)
                    .solve(&(t - from.t))
                    .clamp(0.0, 1.0);
                from.value + (to.value - from.value) * progress
            }
        }
    }
}

/// What stops the file from being used.
#[derive(Debug, Clone, PartialEq)]
pub enum Problem {
    /// The text is not TOML.
    Syntax(String),
    /// A value is neither a number nor a word.
    NotANumberOrText { name: String },
    /// A track has no keys.
    EmptyTrack { track: String },
    /// A key is not `[seconds, value]` or `[seconds, value, easing]`.
    BadKey { track: String, index: usize, reason: String },
    /// A key is earlier than the key before it.
    OutOfOrder { track: String, index: usize },
    UnknownEasing { track: String, index: usize, easing: String },
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Problem::Syntax(message) => write!(f, "not TOML: {message}"),
            Problem::NotANumberOrText { name } => {
                write!(f, "values.{name}: neither a number nor a word")
            }
            Problem::EmptyTrack { track } => write!(f, "tracks.{track}: no keys"),
            Problem::BadKey { track, index, reason } => {
                write!(f, "tracks.{track}, key {index}: {reason}")
            }
            Problem::OutOfOrder { track, index } => {
                write!(f, "tracks.{track}, key {index}: earlier than the key before it")
            }
            Problem::UnknownEasing { track, index, easing } => write!(
                f,
                "tracks.{track}, key {index}: unknown easing \"{easing}\" (linear, ease-in, \
                 ease-out, ease-in-out, hold)"
            ),
        }
    }
}

/// The problems, one per line, for a panic message or a test.
pub fn problems_text(problems: &[Problem]) -> String {
    use std::fmt::Write;
    problems.iter().fold(String::new(), |mut text, p| {
        let _ = writeln!(text, "- {p}");
        text
    })
}

#[derive(Debug, Default, serde::Deserialize)]
struct File {
    #[serde(default)]
    values: BTreeMap<String, toml::Value>,
    #[serde(default)]
    tracks: BTreeMap<String, Vec<toml::Value>>,
}

/// The values and tracks of one file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Params {
    values: BTreeMap<String, Value>,
    tracks: BTreeMap<String, Track>,
}

impl Params {
    /// Reads the TOML text, or every problem found in it.
    pub fn parse(text: &str) -> Result<Params, Vec<Problem>> {
        let file: File = match toml::from_str(text) {
            Ok(file) => file,
            Err(error) => return Err(vec![Problem::Syntax(error.message().to_owned())]),
        };
        let mut problems = Vec::new();

        let mut values = BTreeMap::new();
        for (name, value) in file.values {
            match value {
                toml::Value::Float(number) => {
                    values.insert(name, Value::Number(number as f32));
                }
                toml::Value::Integer(number) => {
                    values.insert(name, Value::Number(number as f32));
                }
                toml::Value::String(text) => {
                    values.insert(name, Value::Text(text));
                }
                _ => problems.push(Problem::NotANumberOrText { name }),
            }
        }

        let mut tracks = BTreeMap::new();
        for (name, raw_keys) in file.tracks {
            if raw_keys.is_empty() {
                problems.push(Problem::EmptyTrack { track: name });
                continue;
            }
            let mut keys: Vec<Key> = Vec::with_capacity(raw_keys.len());
            for (index, raw) in raw_keys.iter().enumerate() {
                match key(raw) {
                    Ok((t, value, easing_name)) => {
                        let easing = match easing_name {
                            None => KeyEasing::Ease(Easing::Linear),
                            Some(easing_name) => {
                                let Some(easing) = KeyEasing::named(&easing_name) else {
                                    problems.push(Problem::UnknownEasing {
                                        track: name.clone(),
                                        index,
                                        easing: easing_name,
                                    });
                                    continue;
                                };
                                easing
                            }
                        };
                        if keys.last().is_some_and(|previous| t < previous.t) {
                            problems.push(Problem::OutOfOrder { track: name.clone(), index });
                            continue;
                        }
                        keys.push(Key { t, value, easing });
                    }
                    Err(reason) => {
                        problems.push(Problem::BadKey { track: name.clone(), index, reason });
                    }
                }
            }
            tracks.insert(name, Track { keys });
        }

        if problems.is_empty() {
            Ok(Params { values, tracks })
        } else {
            Err(problems)
        }
    }

    /// A number of `[values]`.
    pub fn number(&self, name: &str) -> Option<f32> {
        match self.values.get(name)? {
            Value::Number(number) => Some(*number),
            Value::Text(_) => None,
        }
    }

    /// A word of `[values]`.
    pub fn text(&self, name: &str) -> Option<&str> {
        match self.values.get(name)? {
            Value::Text(text) => Some(text),
            Value::Number(_) => None,
        }
    }

    pub fn track(&self, name: &str) -> Option<&Track> {
        self.tracks.get(name)
    }

    /// The value of track `name` at `t` seconds; `None` for an unknown track.
    pub fn at(&self, name: &str, t: f32) -> Option<f32> {
        Some(self.track(name)?.at(t))
    }

    /// The names of the values and tracks that are missing, to check a file against what a
    /// scene reads from it.
    pub fn missing<'n>(&self, values: &[&'n str], tracks: &[&'n str]) -> Vec<&'n str> {
        values
            .iter()
            .filter(|name| !self.values.contains_key(**name))
            .chain(tracks.iter().filter(|name| !self.tracks.contains_key(**name)))
            .copied()
            .collect()
    }
}

/// `[seconds, value]` or `[seconds, value, easing]`.
fn key(raw: &toml::Value) -> Result<(f32, f32, Option<String>), String> {
    let Some(items) = raw.as_array() else {
        return Err("not an array".to_owned());
    };
    if items.len() < 2 || items.len() > 3 {
        return Err(format!("{} items, expected [seconds, value, easing?]", items.len()));
    }
    let number = |item: &toml::Value, what: &str| -> Result<f32, String> {
        item.as_float()
            .or_else(|| item.as_integer().map(|n| n as f64))
            .map(|n| n as f32)
            .ok_or_else(|| format!("{what} is not a number"))
    };
    let t = number(&items[0], "seconds")?;
    let value = number(&items[1], "value")?;
    let easing = match items.get(2) {
        None => None,
        Some(item) => Some(
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| "easing is not a word".to_owned())?,
        ),
    };
    Ok((t, value, easing))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = r#"
[values]
trunk_sit = 46.0
steps = 3
title = "DIX-HALLPIKE TEST"

[tracks]
trunk = [[6.4, 46.0], [7.4, -40.0, "ease-in-out"]]
flag = [[0, 0], [2, 1, "hold"], [3, 2]]
"#;

    #[test]
    fn values_are_typed() {
        let p = Params::parse(FILE).unwrap();
        assert_eq!(p.number("trunk_sit"), Some(46.0));
        assert_eq!(p.number("steps"), Some(3.0));
        assert_eq!(p.text("title"), Some("DIX-HALLPIKE TEST"));
        assert_eq!(p.number("title"), None);
        assert_eq!(p.text("trunk_sit"), None);
        assert_eq!(p.number("nothing"), None);
        assert_eq!(p.missing(&["trunk_sit", "head_sit"], &["trunk", "head"]), ["head_sit", "head"]);
    }

    #[test]
    fn tracks_hold_outside_their_keys_and_ease_between() {
        let p = Params::parse(FILE).unwrap();
        assert_eq!(p.at("trunk", 0.0), Some(46.0));
        assert_eq!(p.at("trunk", 6.4), Some(46.0));
        assert_eq!(p.at("trunk", 10.0), Some(-40.0));
        // ease-in-out is symmetric: the middle of the key is the middle of the values
        let middle = p.at("trunk", 6.9).unwrap();
        assert!((middle - 3.0).abs() < 0.5, "{middle}");
        // slow start: a tenth of the way in time is well under a tenth of the way in value
        let early = p.at("trunk", 6.5).unwrap();
        assert!(early > 44.0, "{early}");
        assert_eq!(p.at("nothing", 1.0), None);

        // a held key keeps the previous value until its time, then a linear key follows
        assert_eq!(p.at("flag", 1.9), Some(0.0));
        assert_eq!(p.at("flag", 2.0), Some(1.0));
        assert_eq!(p.at("flag", 2.5), Some(1.5));
    }

    #[test]
    fn every_problem_is_listed() {
        let problems = Params::parse(
            r#"
[values]
ok = 1
bad = [1, 2]

[tracks]
empty = []
late = [[2, 0], [1, 0]]
odd = [[0, 0, "bounce"], [1], [2, "x"], 5]
"#,
        )
        .unwrap_err();
        assert_eq!(
            problems,
            vec![
                Problem::NotANumberOrText { name: "bad".into() },
                Problem::EmptyTrack { track: "empty".into() },
                Problem::OutOfOrder { track: "late".into(), index: 1 },
                Problem::UnknownEasing { track: "odd".into(), index: 0, easing: "bounce".into() },
                Problem::BadKey {
                    track: "odd".into(),
                    index: 1,
                    reason: "1 items, expected [seconds, value, easing?]".into()
                },
                Problem::BadKey { track: "odd".into(), index: 2, reason: "value is not a number".into() },
                Problem::BadKey { track: "odd".into(), index: 3, reason: "not an array".into() },
            ]
        );
        assert!(problems_text(&problems).contains("tracks.late, key 1"));
        assert!(matches!(Params::parse("values = [").unwrap_err()[0], Problem::Syntax(_)));
    }
}
