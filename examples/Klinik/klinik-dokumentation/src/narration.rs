//! Narration of the revised script, one clip per sentence (`narration/lines.tsv`,
//! made by `narration/make.sh`), resolved by the `shared-narration` crate: each
//! sentence has a cue in its scene and starts at the cue or right after the
//! sentence before it, using the measured clip lengths in
//! `narration/durations.tsv`. Captions show the same sentences, at most two
//! lines at a time; a gap shorter than a second keeps the previous caption.
//! The schedule is checked when first used: a missing line, a missing clip
//! length or a sentence running past its scene stops the video.

use std::sync::LazyLock;

use fframes::layout::{Block, FrameMeasurer, TextBlock};
use fframes::{FFramesContext, FontQuery, Frame, Svgr};
use shared_narration::{DurationSource, Input, Policy, Schedule};

use crate::ui::*;

/// Length of every scene in seconds (frame table of the revised script).
pub const SCENE_SECONDS: [f32; 15] = [
    18.0, 30.0, 45.0, 50.0, 51.0, 35.0, 30.0, 25.0, 35.0, 41.5, 30.5, 94.0, 22.0, 12.0, 10.0,
];

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
    ("sx_1", 5, 0.8),
    ("sx_2", 5, 5.6),
    ("sx_3", 5, 13.2),
    ("sx_4", 5, 39.6),
    ("s5_1", 6, 0.8),
    ("s5_2", 6, 9.5),
    ("s5_3", 6, 21.5),
    ("s6_1", 7, 0.8),
    ("s6_2", 7, 9.5),
    ("s6_3", 7, 19.5),
    ("s7_1", 8, 0.8),
    ("s7_2", 8, 10.5),
    ("sa_1", 9, 0.8),
    ("sa_2", 9, 5.8),
    ("sa_3", 9, 14.4),
    ("sa_4", 9, 26.2),
    ("sb_1", 10, 0.8),
    ("sb_2", 10, 7.6),
    ("sb_3", 10, 15.0),
    ("sb_4", 10, 24.9),
    ("sb_5", 10, 33.2),
    ("sc_1", 11, 0.8),
    ("sc_2", 11, 13.6),
    ("sc_3", 11, 20.7),
    ("sd_1", 12, 0.8),
    ("sd_2", 12, 12.5),
    ("sd_3", 12, 17.7),
    ("se_1", 12, 27.6),
    ("se_2", 12, 32.6),
    ("se_3", 12, 42.8),
    ("se_4", 12, 54.4),
    ("se_5", 12, 65.1),
    ("se_6", 12, 82.4),
];

/// The one timing of the video: scenes, lines and captions.
pub static SCHEDULE: LazyLock<Schedule> = LazyLock::new(|| {
    Schedule::build(&Input {
        scene_seconds: &SCENE_SECONDS,
        lines_tsv: include_str!("../narration/lines.tsv"),
        text_column: 2,
        durations: DurationSource::Measured(include_str!("../narration/durations.tsv")),
        cues: CUES,
        policy: Policy {
            bridge_gaps_under: Some(1.0),
            ..Policy::default()
        },
    })
    .unwrap_or_else(|problems| {
        panic!(
            "narration does not resolve:\n{}",
            shared_narration::problems_text(&problems)
        )
    })
});

/// Bottom edge of the caption band, inside the safe margin, and its padding around the text.
const BAND_BOTTOM: f32 = 988.0;
const BAND_PAD: f32 = 30.0;

/// The caption band at the bottom: as high as its measured text plus the padding, the bottom
/// edge fixed, the text centred in it. More than two lines is reported by `inspect`.
pub fn captions<'a>(frame: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let Some(cue) = SCHEDULE.caption_at(t) else {
        return Svgr::empty();
    };
    let o = if SCHEDULE.captions_near(t, 0.5) > 1 { 1.0 } else { window(t, cue.from, cue.until, 0.18) };
    let text = TextBlock::new(cue.text.clone(), FontQuery { family: SANS, size: 34, weight: 400, ..Default::default() })
        .wrap(1560)
        .line_height(1.3)
        .align(fframes::TextAlign::Center)
        .fill("#F3F6FF");
    let Some(text) = Block::Text(text).measure(&mut FrameMeasurer { frame, ctx }) else {
        return Svgr::empty();
    };
    if text.line_count() > 2 {
        fframes::diagnostics::warning("caption_lines", format!("{} lines: \"{}\"", text.line_count(), cue.text));
    }
    let height = text.size().height + 2.0 * BAND_PAD;
    let top = BAND_BOTTOM - height;
    fframes::svgr!(
        <g opacity={o}>
            <rect x="96" y={top} width="1728" height={height} rx="22" fill="#050916" opacity="0.82" stroke="#2A3566" stroke-width="1.5" />
            {text.render(180.0, top + BAND_PAD)}
        </g>
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The data files and cues resolve without a problem, and the frame table is unchanged.
    #[test]
    fn schedule_resolves() {
        let s = &*SCHEDULE;
        assert_eq!(s.lines.len(), CUES.len());
        assert_eq!(s.total, 529.0);
        assert_eq!(s.scene_start(10), 360.5);
        assert_eq!(s.scene_start(12), 485.0);
    }
}
