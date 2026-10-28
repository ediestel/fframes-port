//! `fframes-vector vectorize in.png out.json [--scale 3] [--colors 96] [--svg out.svg] [--flat flat.png]`
//! `fframes-vector svg doc.json out.svg`

use fframes_vector::{PieceDocument, VectorizeParams, flat_image, vectorize};
use std::process::ExitCode;

fn opt<T: std::str::FromStr>(args: &[String], name: &str) -> Option<T> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
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
        Some("group") if args.len() >= 3 => {
            // group doc.json zones.svg [--scale N] [--check check.svg] [-o out.json]
            let (doc_path, zones_path) = (&args[1], &args[2]);
            let mut doc = match PieceDocument::load(doc_path) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("{doc_path}: {e}");
                    return ExitCode::FAILURE;
                }
            };
            let zones = match std::fs::read_to_string(zones_path) {
                Ok(s) => fframes_vector::GroupMap::from_svg(&s),
                Err(e) => {
                    eprintln!("{zones_path}: {e}");
                    return ExitCode::FAILURE;
                }
            };
            let scale: f32 = opt(&args, "--scale").unwrap_or(doc.scale as f32);
            let counts = zones.assign(&mut doc, scale);
            let mut names: Vec<_> = counts.iter().collect();
            names.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
            let grouped: usize = counts.values().sum();
            for (name, n) in names {
                eprintln!("{n:6}  {name}");
            }
            eprintln!(
                "{grouped} of {} pieces grouped, {} zones",
                doc.pieces.len(),
                zones.zones.len()
            );
            let out = opt::<String>(&args, "-o").unwrap_or_else(|| doc_path.clone());
            if let Err(e) = doc.save(&out) {
                eprintln!("{out}: {e}");
                return ExitCode::FAILURE;
            }
            if let Some(check) = opt::<String>(&args, "--check") {
                if let Err(e) = std::fs::write(&check, doc.to_group_check_svg()) {
                    eprintln!("{check}: {e}");
                    return ExitCode::FAILURE;
                }
                eprintln!("check picture: {check}");
            }
            ExitCode::SUCCESS
        }
        Some("sprites") if args.len() >= 3 => {
            // sprites doc.json out_dir [--density 1,2,3] [--group name] [--max 4096] [--pad 2] [--no-outline]
            let (doc_path, dir) = (&args[1], &args[2]);
            let doc = match PieceDocument::load(doc_path) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("{doc_path}: {e}");
                    return ExitCode::FAILURE;
                }
            };
            let densities: Vec<f32> = opt::<String>(&args, "--density")
                .unwrap_or_else(|| "1".to_string())
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            for density in densities {
                let p = fframes_vector::atlas::AtlasParams {
                    density,
                    max_size: opt(&args, "--max").unwrap_or(4096),
                    padding: opt(&args, "--pad").unwrap_or(2),
                    group: opt::<String>(&args, "--group"),
                    skip_outline: args.iter().any(|a| a == "--no-outline"),
                };
                let pages = fframes_vector::atlas::build(&doc, &p);
                let stem = match &p.group {
                    Some(g) => format!("{g}_{density}x"),
                    None => format!("atlas_{density}x"),
                };
                match fframes_vector::atlas::write(&pages, dir, &stem, density) {
                    Ok(files) => {
                        let n: usize = pages.iter().map(|pg| pg.sprites.len()).sum();
                        eprintln!(
                            "{density}x: {n} sprites on {} page(s): {}",
                            pages.len(),
                            files.join(", ")
                        );
                    }
                    Err(e) => {
                        eprintln!("{dir}: {e}");
                        return ExitCode::FAILURE;
                    }
                }
            }
            eprintln!("({:.1} s)", start.elapsed().as_secs_f32());
            ExitCode::SUCCESS
        }
        Some("svg") if args.len() >= 3 => match PieceDocument::load(&args[1]) {
            Ok(doc) => match std::fs::write(
                &args[2],
                if args.iter().any(|a| a == "--grouped") {
                    doc.to_svg_grouped()
                } else {
                    doc.to_svg()
                },
            ) {
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
            eprintln!(
                "fframes-vector vectorize in.png out.json [--scale N] [--colors N] [--svg out.svg] [--flat flat.png]\nfframes-vector svg doc.json out.svg"
            );
            ExitCode::FAILURE
        }
    }
}
