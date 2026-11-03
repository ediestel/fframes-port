//! Narration of the revised script, one clip per sentence (`narration/lines.tsv`,
//! made by `narration/make.sh`), resolved by the `shared-narration` crate: each
//! sentence has a cue in its scene and starts at the cue or right after the
//! sentence before it, using the measured clip lengths in
//! `narration/durations.tsv`. Captions show the same sentences, at most two
//! lines at a time. The schedule is checked when first used: a missing line, a
//! missing clip length or a sentence running past its scene stops the video.

use std::sync::LazyLock;

use fframes::{FFramesContext, FontQuery, Frame, Svgr};
use shared_narration::{DurationSource, Input, Policy, Schedule};

use crate::ui::*;

/// Length of every scene in seconds (frame table of the revised script).
pub const SCENE_SECONDS: [f32; 8] = [15.0, 30.0, 45.0, 50.0, 35.0, 30.0, 25.0, 10.0];

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

/// The one timing of the video: scenes, lines, captions and voice tracks.
pub static SCHEDULE: LazyLock<Schedule> = LazyLock::new(|| {
    Schedule::build(&Input {
        scene_seconds: &SCENE_SECONDS,
        lines_tsv: include_str!("../narration/lines.tsv"),
        text_column: 2,
        durations: DurationSource::Measured(include_str!("../narration/durations.tsv")),
        cues: CUES,
        policy: Policy::default(),
    })
    .unwrap_or_else(|problems| {
        panic!(
            "narration does not resolve:\n{}",
            shared_narration::problems_text(&problems)
        )
    })
});

/// The caption band at the bottom, inside the safe margin.
pub fn captions<'a>(frame: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let Some(cue) = SCHEDULE.caption_at(t) else {
        return Svgr::empty();
    };
    let o = if SCHEDULE.captions_near(t, 0.5) > 1 { 1.0 } else { window(t, cue.from, cue.until, 0.18) };
    let words = frame
        .text_break_lines(
            ctx,
            &cue.text,
            fframes::BreakLinesOpts {
                font: FontQuery {
                    family: SANS,
                    size: 34,
                    weight: 400,
                    ..Default::default()
                },
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The data files and cues resolve without a problem, and the frame table is unchanged.
    #[test]
    fn schedule_resolves() {
        let s = &*SCHEDULE;
        assert_eq!(s.lines.len(), CUES.len());
        assert_eq!(s.total, 240.0);
        assert_eq!(s.scene_start(4), 140.0);
        assert_eq!(s.scene_start(7), 230.0);
    }
}
