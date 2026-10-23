use crate::{
    AudioMap, Color, CpuFrameRenderer, Duration, FFramesContext, Frame, ImageSequence, Previewer,
    RenderOptions, Svgr, Video, cpu::CpuRenderingBackend, render_image_sequence,
};
use std::path::PathBuf;

struct TintedFrames;

impl Video for TintedFrames {
    const FPS: usize = 24;
    const WIDTH: usize = 16;
    const HEIGHT: usize = 16;

    fn duration(&self) -> Duration<'_> {
        Duration::Frames(5)
    }

    fn audio(&self) -> AudioMap<'_> {
        AudioMap::none()
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let r = 20 + frame.index * 40;
        Svgr::from(format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16'>\
             <rect width='16' height='16' fill='rgb({r},80,120)'/></svg>"
        ))
    }
}

/// Half transparent red over a transparent background.
struct TranslucentFrame;

impl Video for TranslucentFrame {
    const FPS: usize = 24;
    const WIDTH: usize = 8;
    const HEIGHT: usize = 8;
    const BACKGROUND_COLOR: Color = Color::TRANSPARENT;

    fn duration(&self) -> Duration<'_> {
        Duration::Frames(1)
    }

    fn audio(&self) -> AudioMap<'_> {
        AudioMap::none()
    }

    fn render_frame<'a>(&'a self, _frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        Svgr::from(
            "<svg xmlns='http://www.w3.org/2000/svg' width='8' height='8'>\
             <rect width='8' height='8' fill='#ff0000' fill-opacity='0.5'/></svg>",
        )
    }
}

fn scratch_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("fframes-{name}-{}", uuid::Uuid::new_v4()))
}

#[test]
fn png_sequence_matches_preview_pixels_in_frame_order() {
    let dir = scratch_dir("png-sequence");
    let sequence = ImageSequence::png(&dir);
    let written = render_image_sequence(
        &sequence,
        &TintedFrames,
        CpuRenderingBackend::default(),
        &RenderOptions::default(),
    )
    .unwrap();

    assert_eq!(
        written,
        (0..5).map(|f| sequence.frame_path(f)).collect::<Vec<_>>()
    );
    assert_eq!(written[3].file_name().unwrap(), "frame_000003.png");

    let mut previewer = Previewer::new(&TintedFrames, &RenderOptions::default()).unwrap();
    let mut renderer = CpuFrameRenderer::default();
    for (frame, path) in written.iter().enumerate() {
        let decoded = image::open(path).unwrap().into_rgba8().into_raw();
        let expected = previewer.render(frame, &mut renderer).unwrap().pixels;
        assert_eq!(decoded, expected, "frame {frame}");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn frame_range_writes_only_those_frames_named_by_video_frame() {
    let dir = scratch_dir("range-sequence");
    let options = RenderOptions {
        frame_range: Some(2..4),
        ..Default::default()
    };
    let sequence = ImageSequence::tiff(&dir);
    let written = render_image_sequence(
        &sequence,
        &TintedFrames,
        CpuRenderingBackend::default(),
        &options,
    )
    .unwrap();
    assert_eq!(
        written,
        vec![sequence.frame_path(2), sequence.frame_path(3)]
    );
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
    assert!(image::open(&written[0]).is_ok());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn png_keeps_straight_alpha_of_translucent_pixels() {
    let dir = scratch_dir("alpha-png");
    let written = render_image_sequence(
        &ImageSequence::png(&dir),
        &TranslucentFrame,
        CpuRenderingBackend::default(),
        &RenderOptions::default(),
    )
    .unwrap();
    let pixel = *image::open(&written[0])
        .unwrap()
        .into_rgba8()
        .get_pixel(4, 4);
    // Straight alpha: full red at half coverage, not red darkened by the coverage.
    assert_eq!(pixel.0[0], 255, "got {pixel:?}");
    assert!((127..=128).contains(&pixel.0[3]), "got {pixel:?}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn exr_is_linear_with_premultiplied_alpha() {
    let dir = scratch_dir("alpha-exr");
    let written = render_image_sequence(
        &ImageSequence::exr(&dir),
        &TranslucentFrame,
        CpuRenderingBackend::default(),
        &RenderOptions::default(),
    )
    .unwrap();
    let pixel = *image::open(&written[0])
        .unwrap()
        .into_rgba32f()
        .get_pixel(4, 4);
    let [r, g, b, a] = pixel.0;
    assert!((a - 0.5).abs() < 0.01, "alpha {a}");
    // Red 1.0 in linear light, premultiplied by the coverage.
    assert!((r - a).abs() < 0.01, "red {r} for alpha {a}");
    assert!(g.abs() < 1e-3 && b.abs() < 1e-3, "green {g} blue {b}");
    std::fs::remove_dir_all(dir).unwrap();
}
