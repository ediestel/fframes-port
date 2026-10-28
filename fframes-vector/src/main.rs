//! `fframes-vector vectorize in.png out.json [--scale 3] [--colors 96] [--svg out.svg] [--flat flat.png]`
//! `fframes-vector svg doc.json out.svg`

use fframes_vector::{PieceDocument, VectorizeParams, flat_image, vectorize};
use std::process::ExitCode;

fn opt<T: std::str::FromStr>(args: &[String], name: &str) -> Option<T> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).and_then(|v| v.parse().ok())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let start = std::time::Instant::now();
    match args.first().map(String::as_str) {
        Some("vectorize") if args.len() >= 3 => {
            let (input, output) = (&args[1], &args[2]);
            let scale: u32 = opt(&args, "--scale").unwrap_or(2);
            let mut p = VectorizeParams::outlined(scale);
            if let Some(c) = opt(&args, "--colors") {
                p.colors = c;
            }
            let img = match image::open(input) {
                Ok(i) => i.to_rgb8(),
                Err(e) => {
                    eprintln!("{input}: {e}");
                    return ExitCode::FAILURE;
                }
            };
            if let Some(flat) = opt::<String>(&args, "--flat") {
                if let Err(e) = flat_image(&img, &p).save(&flat) {
                    eprintln!("{flat}: {e}");
                    return ExitCode::FAILURE;
                }
                eprintln!("flat picture: {flat}");
            }
            let doc = match vectorize(&img, &p) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::FAILURE;
                }
            };
            if let Err(e) = doc.save(output) {
                eprintln!("{output}: {e}");
                return ExitCode::FAILURE;
            }
            if let Some(svg) = opt::<String>(&args, "--svg") {
                if let Err(e) = std::fs::write(&svg, doc.to_svg()) {
                    eprintln!("{svg}: {e}");
                    return ExitCode::FAILURE;
                }
                eprintln!("svg: {svg}");
            }
            eprintln!(
                "{}x{} at {}x, {} colours, {} pieces -> {output} ({:.1} s)",
                doc.width,
                doc.height,
                doc.scale,
                doc.palette.len(),
                doc.pieces.len(),
                start.elapsed().as_secs_f32()
            );
            ExitCode::SUCCESS
        }
        Some("svg") if args.len() >= 3 => match PieceDocument::load(&args[1]) {
            Ok(doc) => match std::fs::write(&args[2], doc.to_svg()) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("{}: {e}", args[2]);
                    ExitCode::FAILURE
                }
            },
            Err(e) => {
                eprintln!("{}: {e}", args[1]);
                ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!("fframes-vector vectorize in.png out.json [--scale N] [--colors N] [--svg out.svg] [--flat flat.png]\nfframes-vector svg doc.json out.svg");
            ExitCode::FAILURE
        }
    }
}
