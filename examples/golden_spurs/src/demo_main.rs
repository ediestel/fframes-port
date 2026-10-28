use fframes::{RenderOptions, cli};
use fframes_skia_renderer::{
    SkiaFFramesRenderer, SkiaPipelineConcurrencyPolicy, SkiaPipelineConfig, metal::SkiaMetalCtx,
};
use golden_spurs::demo::{Demo, HEIGHT, WIDTH};
use std::process::ExitCode;

/// `golden-spurs-demo render -o film/objects_demo.mp4`: the object features of
/// `fframes::vector` on the window, no sound.
fn main() -> ExitCode {
    let gpu = SkiaMetalCtx::new(WIDTH, HEIGHT).expect("GPU context");
    cli::new(
        &Demo,
        RenderOptions {
            load_system_fonts: true,
            default_font: "Helvetica Neue",
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
