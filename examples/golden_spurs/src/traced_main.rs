use fframes::cli::clap::Parser;
use fframes::{RenderOptions, cli};
use fframes_skia_renderer::{
    SkiaFFramesRenderer, SkiaPipelineConcurrencyPolicy, SkiaPipelineConfig, metal::SkiaMetalCtx,
};
use golden_spurs::traced::{HEIGHT, TracedWindow, WIDTH};
use std::process::ExitCode;

/// `golden-spurs-traced frame 0 -o frames_traced` writes the window as `frames_traced/0.png`.
fn main() -> ExitCode {
    let gpu = SkiaMetalCtx::new(WIDTH, HEIGHT).expect("GPU context");
    cli::new(
        &TracedWindow,
        RenderOptions {
            media: None,
            load_system_fonts: true,
            default_font: "Luminari",
            ..Default::default()
        },
    )
    .args(cli::Cli::<cli::NoArgs>::parse())
    .backend(
        SkiaFFramesRenderer::new_metal(
            &gpu,
            SkiaPipelineConfig {
                concurrency_policy: SkiaPipelineConcurrencyPolicy::MaxPerformance,
                ..Default::default()
            },
        )
        .expect("skia renderer"),
    )
    .preview(fframes_native_player::cli_preview)
    .run()
}
