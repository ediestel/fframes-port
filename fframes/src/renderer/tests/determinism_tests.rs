//! The determinism vocabulary (`InputFingerprint`, `OutputHash`, `assert_deterministic`) applied
//! to real renders: the same input renders to the same bytes, and a changed input is detected.

use crate::{
    AudioMap, CpuFrameRenderer, DeterminismError, Duration, FFramesContext, Frame,
    InputFingerprint, OutputHash, Previewer, RenderOptions, RenderResult, Svgr, Video,
    assert_deterministic,
};

const GRADIENT_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48">
    <defs>
        <linearGradient id="g" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0%" stop-color="#ffee88"/>
            <stop offset="100%" stop-color="#224488"/>
        </linearGradient>
    </defs>
    <rect width="48" height="48" fill="url(#g)"/>
    <circle cx="24" cy="24" r="14" fill="#cc2244" opacity="0.7"/>
    <path d="M 6 40 Q 24 4 42 40" fill="none" stroke="#33aa66" stroke-width="3"/>
</svg>"##;

struct StillVideo(&'static str);

impl Video for StillVideo {
    const FPS: usize = 24;
    const WIDTH: usize = 48;
    const HEIGHT: usize = 48;

    fn duration(&self) -> Duration<'_> {
        Duration::Frames(2)
    }

    fn audio(&self) -> AudioMap<'_> {
        AudioMap::none()
    }

    fn render_frame<'a>(&'a self, _frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        Svgr::from(self.0)
    }
}

/// Renders every frame with a fresh previewer and renderer, as a separate render run would.
fn render_run(svg: &'static str) -> RenderResult {
    let video = StillVideo(svg);
    let mut previewer = Previewer::new(&video, &RenderOptions::default()).unwrap();
    let mut renderer = CpuFrameRenderer::default();
    let mut bytes = Vec::new();
    for index in 0..2 {
        bytes.extend(previewer.render(index, &mut renderer).unwrap().pixels);
    }

    RenderResult {
        output_hash: OutputHash::of_bytes(&bytes),
        input_fingerprint: InputFingerprint::new()
            .add_str("svg", svg)
            .add_u32("width", StillVideo::WIDTH as u32)
            .add_u32("height", StillVideo::HEIGHT as u32)
            .add_u32("fps", StillVideo::FPS as u32)
            .finalize(),
        frame_count: 2,
    }
}

#[test]
fn identical_input_renders_identical_bytes() {
    assert_deterministic(&render_run(GRADIENT_SVG), &render_run(GRADIENT_SVG))
        .expect("two renders of the same input must match");
}

#[test]
fn changed_input_is_reported_as_input_mismatch() {
    let changed = r##"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48"><rect width="48" height="48" fill="#123456"/></svg>"##;
    let err = assert_deterministic(&render_run(GRADIENT_SVG), &render_run(changed))
        .expect_err("different inputs must not compare equal");
    assert!(
        matches!(err, DeterminismError::InputMismatch { .. }),
        "got {err}"
    );
}

#[test]
fn output_drift_under_the_same_fingerprint_is_reported() {
    let left = render_run(GRADIENT_SVG);
    let mut right = left.clone();
    right.output_hash = OutputHash::of_bytes(b"drifted output");
    let err = assert_deterministic(&left, &right).expect_err("byte-different output must fail");
    assert!(
        matches!(err, DeterminismError::OutputMismatch { .. }),
        "got {err}"
    );
}
