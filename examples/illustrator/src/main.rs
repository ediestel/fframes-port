//! `illustrator <figure folder> <stage>` runs a stage of the round trip (`trace`, `export`,
//! `import`, `all`); `illustrator <figure folder> <fframes command>` renders the stack
//! (`frame 0`, `render`, `inspect`, `preview`, ...).

use fframes::cli::clap::Parser;
use fframes::{MediaDirectory, RenderOptions, cli};
use fframes_skia_renderer::{
    SkiaFFramesRenderer, SkiaPipelineConcurrencyPolicy, SkiaPipelineConfig, metal::SkiaMetalCtx,
};
use illustrator::figure::{Figure, problems_text};
use illustrator::{FigureVideo, HEIGHT, WIDTH, stages};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: illustrator <figure folder> trace|export|import|all | <fframes command>");
        return ExitCode::FAILURE;
    }
    let root = args.remove(1);
    let figure = match Figure::load(&root) {
        Ok(figure) => figure,
        Err(problems) => {
            eprintln!("{root}/figure.toml:\n{}", problems_text(&problems));
            return ExitCode::FAILURE;
        }
    };

    let stage = match args[1].as_str() {
        "trace" => Some(stages::trace(&figure)),
        "export" => Some(stages::export(&figure)),
        "import" => Some(stages::import(&figure)),
        "all" => Some(stages::all(&figure)),
        _ => None,
    };
    if let Some(result) = stage {
        return match result {
            Ok(report) => {
                println!("{report}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        };
    }

    let video = match FigureVideo::open(figure) {
        Ok(video) => video,
        Err(problems) => {
            eprintln!("{root}:\n{}", problems_text(&problems));
            return ExitCode::FAILURE;
        }
    };
    // The figure folder is the media folder: the source picture and the artist's PNGs.
    let folder = match MediaDirectory::read_folder(&root) {
        Ok(folder) => folder,
        Err(e) => {
            eprintln!("{root}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut media = match folder.process_media_source() {
        Ok(media) => media,
        Err(e) => {
            eprintln!("{root}: {e}");
            return ExitCode::FAILURE;
        }
    };
    // The artist's SVG layers, drawn at the canvas size, join the images under their file name.
    let (w, h) = (video.figure.width, video.figure.height);
    for (file, rgba) in video.svg_images() {
        media.images.insert(file.clone(), fframes::media::rendered_image(w, h, rgba.clone()));
    }
    let gpu = SkiaMetalCtx::new(WIDTH, HEIGHT).expect("GPU context");
    cli::new(
        &video,
        RenderOptions {
            media: Some(&media),
            load_system_fonts: true,
            default_font: "Helvetica",
            ..Default::default()
        },
    )
    .args(cli::Cli::<cli::NoArgs>::parse_from(args))
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
