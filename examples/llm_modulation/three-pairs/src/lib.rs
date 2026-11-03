//! "Controlling Hallucination Without Changing the Model: Paired Tests in
//! Documents, Mathematics, and Robot Code": the film of the manuscript of
//! November 3, 2026 (`modulation_three_pairs_2026-11-03.tex`), in its own
//! order. Made the way the hallucination-test film is: a dark page per
//! point, every claim set once, no voice and no subtitles; room tone under
//! everything and one soft tone each time an amber underline lands under a
//! statement the source does not support.

pub mod page;
pub mod qr;
pub mod scenes;
pub mod ui;

use fframes::{
    AudioMap, AudioTimestamp::*, AudioTrack, Color, Duration, FFramesContext, Frame, Scene,
    Scenes, Svgr, Video, include_media_dir,
};

use scenes::*;

include_media_dir!(pub struct FilmMedia, "media");

pub const FPS: usize = 30;
pub const WIDTH: usize = 1920;
pub const HEIGHT: usize = 1080;
pub const TOTAL: f32 = ChallengeScene::END;

/// Seconds since the start of the film, whichever page the frame belongs to.
pub fn gsec(frame: &Frame) -> f32 {
    frame.global_index as f32 / FPS as f32
}

#[derive(Debug)]
pub struct Film;

impl Video for Film {
    const FPS: usize = FPS;
    const WIDTH: usize = WIDTH;
    const HEIGHT: usize = HEIGHT;
    const BACKGROUND_COLOR: Color = Color::BLACK;

    fn duration(&self) -> Duration<'_> {
        Duration::Auto
    }

    /// No background sound; one soft tone each time an amber underline lands.
    fn audio(&self) -> AudioMap<'_> {
        let mut tracks = Vec::new();
        for at in scenes::tones() {
            tracks.push(AudioTrack::new("tone.wav", Second(at)..Eof).gain_db(-14.0));
        }
        AudioMap::from(tracks)
    }

    fn define_scenes(&self) -> Scenes<'_> {
        Scenes::from(vec![
            &TitleScene as &dyn Scene,
            &ProblemScene,
            &SetupScene,
            &DocumentsScene,
            &MathScene,
            &RobotScene,
            &ResultsScene,
            &ConclusionScene,
            &ChallengeScene,
        ])
    }

    fn render_frame<'a>(&'a self, frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        fframes::svgr!(
            <svg xmlns="http://www.w3.org/2000/svg" width="1920" height="1080" viewBox="0 0 1920 1080">
                {ctx.render_scenes(&frame)}
            </svg>
        )
    }
}
