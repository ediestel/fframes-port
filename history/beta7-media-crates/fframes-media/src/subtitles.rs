use crate::error::FFramesMediaError;
use std::marker::PhantomData;

pub trait FFramesSubtitlesCue {
    fn text(&self) -> &str;
}

pub trait FFramesSubtitles<'a> {
    type Cue: FFramesSubtitlesCue;
    fn get_cue_by_time(&'a self, ms: u64) -> Option<(usize, &'a Self::Cue)>;
    fn get_cue_stack(&'a self, ms: u64, overlap: u64) -> Vec<&'a Self::Cue>;
}

#[derive(Debug, Clone)]
pub struct VttCue {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

impl FFramesSubtitlesCue for VttCue {
    fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Debug, Clone, Default)]
pub struct Subtitles<'a> {
    cues: Vec<VttCue>,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> Subtitles<'a> {
    pub fn parse(input: &str) -> Result<Self, FFramesMediaError> {
        let mut cues = Vec::new();
        let mut lines = input.lines().peekable();

        // Skip WEBVTT header
        if let Some(first) = lines.next() {
            if !first.starts_with("WEBVTT") {
                return Err(FFramesMediaError::SubtitleParseError(
                    "Not a valid WebVTT file".to_string(),
                ));
            }
        }

        while let Some(line) = lines.next() {
            let line = line.trim();

            if line.is_empty() {
                continue;
            }

            // Fix 9: consume the entire NOTE/STYLE/REGION block (header + body lines until blank)
            if line.starts_with("NOTE") || line.starts_with("STYLE") || line.starts_with("REGION") {
                for body in lines.by_ref() {
                    if body.trim().is_empty() {
                        break;
                    }
                }
                continue;
            }

            // Optionally skip cue identifier (non-timing line)
            let timing_line = if line.contains("-->") {
                line
            } else {
                // This might be a cue ID; next non-empty line should be timing
                match lines.next() {
                    Some(l) if l.contains("-->") => l.trim(),
                    _ => continue,
                }
            };

            let (start_ms, end_ms) = match parse_timing(timing_line) {
                Some(v) => v,
                None => continue,
            };

            let mut text_lines = Vec::new();
            while let Some(text_line) = lines.next() {
                let tl = text_line.trim();
                if tl.is_empty() {
                    break;
                }
                text_lines.push(tl.to_string());
            }

            if !text_lines.is_empty() {
                cues.push(VttCue {
                    start_ms,
                    end_ms,
                    text: text_lines.join("\n"),
                });
            }
        }

        Ok(Self {
            cues,
            _phantom: PhantomData,
        })
    }
}

fn parse_timing(line: &str) -> Option<(u64, u64)> {
    let parts: Vec<&str> = line.splitn(2, "-->").collect();
    if parts.len() != 2 {
        return None;
    }
    let start = parse_timestamp(parts[0].trim())?;
    let end = parse_timestamp(parts[1].trim().split_whitespace().next()?)?;
    Some((start, end))
}

fn parse_timestamp(ts: &str) -> Option<u64> {
    let parts: Vec<&str> = ts.split(':').collect();
    let (h, m, s_ms) = match parts.len() {
        3 => (
            parts[0].parse::<u64>().ok()?,
            parts[1].parse::<u64>().ok()?,
            parts[2],
        ),
        2 => (0, parts[0].parse::<u64>().ok()?, parts[1]),
        _ => return None,
    };

    let s_ms_parts: Vec<&str> = s_ms.splitn(2, '.').collect();
    let secs = s_ms_parts[0].parse::<u64>().ok()?;
    let ms = if s_ms_parts.len() > 1 {
        let ms_str = s_ms_parts[1];
        let ms_val = ms_str.parse::<u64>().ok()?;
        const MS_PER_TENTH_SECOND: u64 = 100;
        const MS_PER_HUNDREDTH_SECOND: u64 = 10;
        match ms_str.len() {
            1 => ms_val * MS_PER_TENTH_SECOND,
            2 => ms_val * MS_PER_HUNDREDTH_SECOND,
            _ => ms_val,
        }
    } else {
        0
    };

    Some(h * 3_600_000 + m * 60_000 + secs * 1_000 + ms)
}

impl<'a> FFramesSubtitles<'a> for Subtitles<'a> {
    type Cue = VttCue;

    fn get_cue_by_time(&'a self, ms: u64) -> Option<(usize, &'a VttCue)> {
        self.cues
            .iter()
            .enumerate()
            .find(|(_, cue)| cue.start_ms <= ms && ms < cue.end_ms)
            .map(|(i, cue)| (i, cue))
    }

    fn get_cue_stack(&'a self, ms: u64, overlap: u64) -> Vec<&'a VttCue> {
        self.cues
            .iter()
            .filter(|cue| cue.start_ms <= ms + overlap && ms < cue.end_ms)
            .collect()
    }
}
