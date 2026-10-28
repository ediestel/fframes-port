use fframes::cli::clap::Parser;
use fframes::{RenderOptions, cli};
use fframes_skia_renderer::{
    SkiaFFramesRenderer, SkiaPipelineConcurrencyPolicy, SkiaPipelineConfig, metal::SkiaMetalCtx,
};
use golden_spurs::{HEIGHT, WIDTH, Window};
use std::process::ExitCode;

/// `golden-spurs frame 0` writes the window as `frames/*.png`.
fn main() -> ExitCode {
    let gpu = SkiaMetalCtx::new(WIDTH, HEIGHT).expect("GPU context");
    cli::new(
        &Window,
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
