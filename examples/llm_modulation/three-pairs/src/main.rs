use fframes::{
    AudioMixOptions, CombinedMediaProvider, EncoderOptions, LimiterOptions, MediaDirectory,
    MediaProvider, RenderOptions, StaticMediaProvider, cli,
};
use fframes_skia_renderer::{
    SkiaFFramesRenderer, SkiaPipelineConcurrencyPolicy, SkiaPipelineConfig, metal::SkiaMetalCtx,
};
use three_pairs::{Film, FilmMedia, HEIGHT, WIDTH};
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let media = FilmMedia::prepare().expect("media");
    // room tone and the soft tone: loaded at runtime, stereo
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("dynamic_media");
    let dir = MediaDirectory::read_folder(&folder).expect("dynamic media folder");
    let dynamic = dir.process_media_source().expect("dynamic media");
    let all = CombinedMediaProvider::from([&media as &dyn MediaProvider, &dynamic]);
    let gpu = SkiaMetalCtx::new(WIDTH, HEIGHT).expect("GPU context");

    cli::new(
        &Film,
        RenderOptions {
            media: Some(&all),
            default_font: "CMU Serif",
            video_encoder_options: EncoderOptions {
                // the M1's hardware encoder: frames stay on the GPU, the CPU is free
                preferred_encoder: Some("h264_videotoolbox"),
                bitrate: Some(10_000_000),
                ..Default::default()
            },
            audio_mix: AudioMixOptions {
                master_gain_db: 0.0,
                limiter: Some(LimiterOptions {
                    ceiling_db: -1.5,
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        },
    )
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
