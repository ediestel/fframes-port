use fframes::cli::clap::Parser;
use fframes::{RenderOptions, cli};
use fframes_skia_renderer::{
    SkiaFFramesRenderer, SkiaPipelineConcurrencyPolicy, SkiaPipelineConfig, metal::SkiaMetalCtx,
};
use golden_spurs::film::{Film, HEIGHT, WIDTH};
use std::process::ExitCode;

/// `golden-spurs-film render -o film/golden_spurs.mp4`; run from the crate directory
/// (the voice files are read from `film/vo/`).
fn main() -> ExitCode {
    let dir = match fframes::MediaDirectory::read_folder("film/vo") {
        Ok(d) => d,
        Err(e) => {
            eprintln!("film/vo: {e}");
            return ExitCode::FAILURE;
        }
    };
    let media = match dir.process_media_source() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("film/vo: {e}");
            return ExitCode::FAILURE;
        }
    };
    let gpu = SkiaMetalCtx::new(WIDTH, HEIGHT).expect("GPU context");
    cli::new(
        &Film,
        RenderOptions {
            media: Some(&media),
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
