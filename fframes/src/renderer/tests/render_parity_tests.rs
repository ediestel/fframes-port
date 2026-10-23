//! Rendering frames one by one with `render_frame`, which sets everything up per call, produces
//! the same RGBA bytes as rendering them in sequence through one `Previewer` and one
//! `CpuFrameRenderer`, which reuse their setup and layer cache. Reused state must not change
//! what a frame looks like, in any order.

use crate::{
    AudioMap, CpuFrameRenderer, Duration, FFramesContext, Frame, Previewer, RenderOptions, Svgr,
    Video, cpu::CpuRenderingBackend, render_frame,
};

struct TintedFrames;

impl Video for TintedFrames {
    const FPS: usize = 24;
    const WIDTH: usize = 32;
    const HEIGHT: usize = 32;

    fn duration(&self) -> Duration<'_> {
        Duration::Frames(3)
    }

    fn audio(&self) -> AudioMap<'_> {
        AudioMap::none()
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        // A different tint per frame catches a frame index reused from earlier state.
        let r = 20 + frame.index * 40;
        Svgr::from(format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='32' height='32' viewBox='0 0 32 32'>\
             <rect width='32' height='32' fill='rgb({r},80,120)'/></svg>"
        ))
    }
}

fn independent_frame(index: usize) -> Vec<u8> {
    render_frame(
        index,
        &TintedFrames,
        CpuRenderingBackend::default(),
        &RenderOptions::default(),
    )
    .unwrap_or_else(|e| panic!("render_frame({index}) failed: {e:?}"))
}

fn sequential_frames(indices: &[usize]) -> Vec<Vec<u8>> {
    let video = TintedFrames;
    let mut previewer = Previewer::new(&video, &RenderOptions::default()).unwrap();
    let mut renderer = CpuFrameRenderer::default();
    indices
        .iter()
        .map(|&index| previewer.render(index, &mut renderer).unwrap().pixels)
        .collect()
}

#[test]
fn sequential_rendering_matches_independent_frames_byte_for_byte() {
    let sequential = sequential_frames(&[0, 1, 2]);
    for (index, pixels) in sequential.iter().enumerate() {
        assert_eq!(pixels, &independent_frame(index), "frame {index} differs");
    }
    assert_ne!(
        sequential[0], sequential[1],
        "frames must differ, or the check proves nothing"
    );
}

#[test]
fn subrange_matches_independent_frames() {
    let sequential = sequential_frames(&[1, 2]);
    assert_eq!(sequential[0], independent_frame(1));
    assert_eq!(sequential[1], independent_frame(2));
}

#[test]
fn render_order_does_not_change_frames() {
    let backwards = sequential_frames(&[2, 1, 0]);
    assert_eq!(backwards[0], independent_frame(2));
    assert_eq!(backwards[1], independent_frame(1));
    assert_eq!(backwards[2], independent_frame(0));
}
