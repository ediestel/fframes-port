//! "A Test for Hallucination in Robot Code Generation": the film from the
//! video script in `../Robot Code Hallucination Test — Video Script.pdf`,
//! presenting the poster `../gtc_poster_ur5e_full.pdf` in its own order: a
//! title slide, then seven sections, paced to be read. Every claim is set
//! once on the page, with no subtitles, so the film works without a voice.

pub mod code;
pub mod poster;
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
pub const TOTAL: f32 = IndustryScene::END;

/// Seconds since the start of the film, whichever scene the frame belongs to.
pub fn gsec(frame: &Frame) -> f32 {
    frame.global_index as f32 / FPS as f32
}

#[derive(Debug)]
pub struct Film;

impl Video for Film {
    const FPS: usize = FPS;
    const WIDTH: usize = WIDTH;
    const HEIGHT: usize = HEIGHT;
    const BACKGROUND_COLOR: Color = Color::WHITE;

    fn duration(&self) -> Duration<'_> {
        Duration::Auto
    }

    /// Quiet room tone under everything, no music; one soft tone each time a
    /// red underline lands (script, "Sound").
    fn audio(&self) -> AudioMap<'_> {
        let mut tracks = vec![
            AudioTrack::new("room.wav", Second(0.0)..Second(TOTAL))
                .gain_db(-6.0)
                .fade_in(2.0)
                .fade_out(3.0),
        ];
        for at in scenes::tones() {
            tracks.push(AudioTrack::new("tone.wav", Second(at)..Eof).gain_db(-14.0));
        }
        AudioMap::from(tracks)
    }

    fn define_scenes(&self) -> Scenes<'_> {
        Scenes::from(vec![
            &IntroScene as &dyn Scene,
            &QueryScene,
            &TestScene,
            &ResultScene,
            &IndustryScene,
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
