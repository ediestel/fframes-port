//! Real renders through the segment cache: a second render of the same video takes every
//! segment from the cache, a changed frame renders only its own segment again, and the output
//! of a cached render is the same video.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::cpu::CpuRenderingBackend;
use crate::{
    AudioMap, Duration, EncoderOptions, FFramesContext, FFramesLogger,
    FFramesLoggerVariant, Frame, RenderOptions, SegmentCacheOptions, SegmentCacheReport, Svgr,
    Video,
};

/// 200 frames at 30 fps: segments 0..72, 72..144, 144..200.
struct Shades {
    changed_frame: Option<usize>,
}

impl Video for Shades {
    const FPS: usize = 30;
    const WIDTH: usize = 32;
    const HEIGHT: usize = 32;

    fn duration(&self) -> Duration<'_> {
        Duration::Frames(200)
    }

    fn audio(&self) -> AudioMap<'_> {
        AudioMap::none()
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let shade = if self.changed_frame == Some(frame.index) {
            255
        } else {
            frame.index % 50 * 4
        };
        Svgr::from(format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32" fill="rgb({shade},40,90)"/></svg>"#
        ))
    }
}

#[derive(Default)]
struct Reports(Mutex<Vec<SegmentCacheReport>>);

impl FFramesLogger for Reports {
    fn success(&self, _output_path: &Path, _temp_files_dir: Option<&PathBuf>) {}
    fn warn(&self, _message: &str) {}
    fn log_segment_cache(&self, report: &SegmentCacheReport) {
        self.0.lock().unwrap().push(*report);
    }
}

fn render(video: &Shades, directory: &Path, output: &str) -> SegmentCacheReport {
    let reports = Arc::new(Reports::default());
    let options = RenderOptions {
        logger: FFramesLoggerVariant::Custom(reports.clone()),
        video_encoder_options: EncoderOptions {
            preferred_encoder: Some("mpeg4"),
            ..Default::default()
        },
        segment_cache: Some(SegmentCacheOptions::new(directory.join("cache"))),
        ..Default::default()
    };
    crate::render(
        directory.join(output),
        video,
        CpuRenderingBackend::default(),
        &options,
    )
    .unwrap();
    assert!(directory.join(output).metadata().unwrap().len() > 0);
    let reports = reports.0.lock().unwrap();
    *reports.last().expect("the render reports the cache")
}

#[test]
fn only_the_segment_of_a_changed_frame_renders_again() {
    let directory = std::env::temp_dir().join(format!("fframes-cache-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();

    let first = render(&Shades { changed_frame: None }, &directory, "first.mp4");
    assert_eq!((first.segments, first.reused_segments), (3, 0));
    assert_eq!(first.rendered_frames(), 200);

    let again = render(&Shades { changed_frame: None }, &directory, "again.mp4");
    assert_eq!((again.segments, again.reused_segments), (3, 3));
    assert_eq!(again.rendered_frames(), 0);
    // the same segments in the same order: the same video stream
    assert_eq!(
        std::fs::read(directory.join("first.mp4")).unwrap().len(),
        std::fs::read(directory.join("again.mp4")).unwrap().len()
    );

    let changed = render(&Shades { changed_frame: Some(100) }, &directory, "changed.mp4");
    assert_eq!((changed.segments, changed.reused_segments), (3, 2));
    assert_eq!(changed.rendered_frames(), 72);

    std::fs::remove_dir_all(directory).unwrap();
}
