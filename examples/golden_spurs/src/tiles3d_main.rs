use fframes::{RenderOptions, cli};
use fframes_skia_renderer::{
    SkiaFFramesRenderer, SkiaPipelineConcurrencyPolicy, SkiaPipelineConfig, metal::SkiaMetalCtx,
};
use golden_spurs::tiles3d::{HEIGHT, Tiles, WIDTH};
use std::process::ExitCode;

/// `golden-spurs-3d render -o film/tiles_3d.mp4`, run from the crate directory after
/// exporting `vector/window_tiles.glb` (see `src/tiles3d.rs`).
fn main() -> ExitCode {
    let gpu = SkiaMetalCtx::new(WIDTH, HEIGHT).expect("GPU context");
    cli::new(
        &Tiles,
        RenderOptions {
            load_system_fonts: true,
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
