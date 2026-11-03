use fframes::cli::clap::Parser;
use fframes::{EncoderOptions, RenderOptions, Video, cli};
use fframes_skia_renderer::{
    SkiaFFramesRenderer, SkiaPipelineConcurrencyPolicy, SkiaPipelineConfig, metal::SkiaMetalCtx,
};
use gp_figures::{HEIGHT, Plate, Puppet, Short, WIDTH};
use std::process::ExitCode;

/// `gp-figures <command>` works on the short, `gp-figures --plate <command>` on the book
/// plate, `gp-figures --puppet <command>` on the test sheet of `thieme_puppet_rig.txt`.
fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);
    let (plate, puppet) = (has("--plate"), has("--puppet"));
    args.retain(|a| a != "--plate" && a != "--puppet");
    let gpu = SkiaMetalCtx::new(WIDTH, HEIGHT).expect("GPU context");
    if puppet {
        run(&Puppet, &gpu, args)
    } else if plate {
        run(&Plate, &gpu, args)
    } else {
        run(&Short, &gpu, args)
    }
}

fn run<V: Video + Send + Sync>(video: &V, gpu: &SkiaMetalCtx, args: Vec<String>) -> ExitCode {
    cli::new(
        video,
        RenderOptions {
            media: None,
            load_system_fonts: true,
            default_font: "Helvetica",
            video_encoder_options: EncoderOptions {
                preferred_encoder: Some("h264_videotoolbox"),
                bitrate: Some(8_000_000),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .args(cli::Cli::<cli::NoArgs>::parse_from(args))
    .backend(
        SkiaFFramesRenderer::new_metal(
            gpu,
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
