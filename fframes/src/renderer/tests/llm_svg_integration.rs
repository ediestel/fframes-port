//! Runtime SVG documents of the kind an LLM emits (gradients, paths, layered fills, no help from
//! `compile-time-svgtree`) go from `Svgr::from_svg_string` to pixels without being reduced to a
//! single colour, and malformed ones fail with a `ValidationError`.

use crate::{
    AudioMap, CpuFrameRenderer, FFramesContext, Frame, Previewer, RenderOptions, Svgr,
    ValidationIssueKind, Video, error::FFramesError,
};

const LLM_STYLE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 64 64">
    <defs>
        <radialGradient id="g" cx="50%" cy="50%" r="50%">
            <stop offset="0%" stop-color="#ffee88"/>
            <stop offset="100%" stop-color="#884422"/>
        </radialGradient>
    </defs>
    <rect width="64" height="64" fill="#1a1230"/>
    <circle cx="32" cy="32" r="20" fill="url(#g)"/>
    <path d="M 12 32 L 32 8 L 52 32 L 32 56 Z" fill="#cc2244" stroke="#2a1a3a" stroke-width="1.5"/>
    <rect x="28" y="28" width="8" height="8" fill="#33aa66"/>
</svg>"##;

struct RuntimeSvgVideo(&'static str);

impl Video for RuntimeSvgVideo {
    const FPS: usize = 24;
    const WIDTH: usize = 64;
    const HEIGHT: usize = 64;

    fn duration(&self) -> crate::Duration<'_> {
        crate::Duration::Frames(3)
    }

    fn audio(&self) -> AudioMap<'_> {
        AudioMap::none()
    }

    fn render_frame<'a>(&'a self, _frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        Svgr::from(self.0)
    }
}

#[test]
fn complex_llm_style_svg_produces_non_trivial_pixels() {
    assert!(Svgr::from_svg_string(LLM_STYLE_SVG.to_owned()).is_ok());

    let video = RuntimeSvgVideo(LLM_STYLE_SVG);
    let mut previewer = Previewer::new(&video, &RenderOptions::default()).unwrap();
    let mut renderer = CpuFrameRenderer::default();

    for index in 0..3 {
        let frame = previewer.render(index, &mut renderer).unwrap();
        assert_eq!(
            frame.pixels.len(),
            64 * 64 * 4,
            "frame {index} has unexpected byte count"
        );

        let non_black_pixels = frame
            .pixels
            .chunks_exact(4)
            .filter(|px| px[0] > 5 || px[1] > 5 || px[2] > 5)
            .count();
        // Quantised to keep gradient steps from inflating the count.
        let distinct_hues = frame
            .pixels
            .chunks_exact(4)
            .map(|px| (px[0] / 32, px[1] / 32, px[2] / 32))
            .collect::<std::collections::HashSet<_>>()
            .len();

        // Background, gradient, red diamond and green square: anything below four hues means
        // usvgr dropped a gradient, a path or a fill.
        assert!(
            non_black_pixels > 64 * 64 / 2,
            "frame {index}: only {non_black_pixels} of 4096 pixels are not black"
        );
        assert!(
            distinct_hues >= 4,
            "frame {index}: only {distinct_hues} distinct quantised hues"
        );
    }
}

#[test]
fn malformed_svg_fails_at_ingest() {
    match Svgr::from_svg_string("<html><body>not svg</body></html>".to_owned()) {
        Err(FFramesError::ValidationError(issue)) => {
            assert_eq!(
                issue.kind,
                ValidationIssueKind::WrongRootElement,
                "got {issue:?}"
            );
        }
        other => panic!("expected ValidationError, got {other:?}"),
    }
}

#[test]
fn empty_svg_fails_validation() {
    match Svgr::from_svg_string(String::new()) {
        Err(FFramesError::ValidationError(issue)) => {
            assert_eq!(issue.kind, ValidationIssueKind::Empty)
        }
        other => panic!("expected ValidationError, got {other:?}"),
    }
}

#[test]
fn malformed_svg_fails_at_render_with_validation_error() {
    let video = RuntimeSvgVideo("<svg><rect");
    let mut previewer = Previewer::new(&video, &RenderOptions::default()).unwrap();
    let err = previewer
        .render(0, &mut CpuFrameRenderer::default())
        .unwrap_err();
    let debug = format!("{err:?}");
    assert!(
        debug.contains("ValidationError") && debug.contains("MalformedXml"),
        "got {debug}"
    );
}
