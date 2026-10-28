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

/// `group doc.json [zones.svg] [--zones z.svg] [--mask m.png [--legend l.json]] [--clusters]
/// [--k N] [--gap N] [--share 0.5] [--raster 4096] [--scale N] [--check c.svg] [-o out.json]`
/// `clusters doc.json [--k N] [--gap N] [--min-area N] [--raster 4096] [--check c.svg] [-o out.json]`
fn group_command(cmd: &str, args: &[String]) -> Result<(), String> {
    use fframes_vector::{AssignParams, ClusterParams, GroupMap, LabelLayer, PieceRaster, group};
    let doc_path = &args[1];
    let mut doc = PieceDocument::load(doc_path).map_err(|e| format!("{doc_path}: {e}"))?;
    let raster = PieceRaster::fit(&doc, opt(args, "--raster").unwrap_or(4096));
    let mut cp = ClusterParams::default();
    if let Some(k) = opt(args, "--k") {
        cp.k = k;
    }
    if let Some(g) = opt(args, "--gap") {
        cp.max_gap = g;
    }
    if let Some(a) = opt(args, "--min-area") {
        cp.min_area = a;
    }
    if cmd == "clusters" {
        let n = group::name_clusters(&mut doc, &raster, &cp);
        eprintln!("{n} clusters");
    } else {
        let zones_path = opt::<String>(args, "--zones").or_else(|| {
            args.get(2)
                .filter(|a| !a.starts_with('-') && a.to_ascii_lowercase().ends_with(".svg"))
                .cloned()
        });
        let mut layer: Option<LabelLayer> = None;
        if let Some(z) = &zones_path {
            let svg = std::fs::read_to_string(z).map_err(|e| format!("{z}: {e}"))?;
            let map = GroupMap::from_svg(&svg);
            let scale: f32 = opt(args, "--scale").unwrap_or(doc.scale as f32);
            eprintln!("{z}: {} zone shapes", map.zones.len());
            layer = Some(map.layer(&raster, scale));
        }
        if let Some(m) = opt::<String>(args, "--mask") {
            let img = image::open(&m).map_err(|e| format!("{m}: {e}"))?.to_rgba8();
            let legend = match opt::<String>(args, "--legend") {
                Some(l) => Some(group::legend_from_json(
                    &std::fs::read_to_string(&l).map_err(|e| format!("{l}: {e}"))?,
                )?),
                None => None,
            };
            let mask = LabelLayer::from_mask(&img, legend.as_deref(), &raster);
            layer = Some(match layer {
                Some(z) => z.under(&mask),
                None => mask,
            });
        }
        let layer = layer.ok_or("group needs zones (an .svg) or --mask")?;
        let p = AssignParams {
            share: opt(args, "--share").unwrap_or(0.5),
            clusters: args.iter().any(|a| a == "--clusters").then_some(cp),
        };
        let counts = group::assign(&mut doc, &raster, &layer, &p);
        let mut names: Vec<_> = counts.iter().collect();
        names.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        for (name, n) in &names {
            eprintln!("{n:6}  {name}");
        }
        eprintln!(
            "{} of {} pieces grouped into {} objects",
            counts.values().sum::<usize>(),
            doc.pieces.len(),
            names.len()
        );
    }
    let out = opt::<String>(args, "-o").unwrap_or_else(|| doc_path.clone());
    doc.save(&out).map_err(|e| format!("{out}: {e}"))?;
    if let Some(check) = opt::<String>(args, "--check") {
        std::fs::write(&check, doc.to_group_check_svg()).map_err(|e| format!("{check}: {e}"))?;
        eprintln!("check picture: {check}");
    }
    Ok(())
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
        Some(cmd @ ("group" | "clusters")) if args.len() >= 2 => match group_command(cmd, &args) {
            Ok(()) => {
                eprintln!("({:.1} s)", start.elapsed().as_secs_f32());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        },
        Some("rename") if args.len() >= 3 => {
            // rename doc.json names.json [-o out.json] [--check c.svg]
            let run = || -> Result<usize, String> {
                let mut doc =
                    PieceDocument::load(&args[1]).map_err(|e| format!("{}: {e}", args[1]))?;
                let text =
                    std::fs::read_to_string(&args[2]).map_err(|e| format!("{}: {e}", args[2]))?;
                let names: std::collections::BTreeMap<String, Vec<String>> =
                    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", args[2]))?;
                let n = fframes_vector::group::rename(&mut doc, &names);
                let out = opt::<String>(&args, "-o").unwrap_or_else(|| args[1].clone());
                doc.save(&out).map_err(|e| format!("{out}: {e}"))?;
                if let Some(check) = opt::<String>(&args, "--check") {
                    std::fs::write(&check, doc.to_group_check_svg())
                        .map_err(|e| format!("{check}: {e}"))?;
                }
                Ok(n)
            };
            match run() {
                Ok(n) => {
                    eprintln!("{n} pieces renamed");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
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
                "fframes-vector vectorize in.png out.json [--scale N] [--colors N] [--svg out.svg] [--flat flat.png]\nfframes-vector group doc.json zones.svg|--mask m.png [--legend l.json] [--clusters] [--share 0.5] [--check c.svg] [-o out.json]\nfframes-vector clusters doc.json [--k N] [--gap N] [--min-area N] [--check c.svg] [-o out.json]\nfframes-vector svg doc.json out.svg [--grouped]\nfframes-vector sprites doc.json dir [--density 1,2] [--group name] [--no-outline]"
            );
            ExitCode::FAILURE
        }
    }
}
