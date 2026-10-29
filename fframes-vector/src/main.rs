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

/// `group doc.json [zones.svg] [--zones z.svg] [--cut [--cut-min 0.05]] [--mask m.png [--legend l.json]] [--clusters]
/// [--k N] [--gap N] [--share 0.5] [--raster 4096] [--scale N] [--split 0.3] [--report r.json]
/// [--check c.svg] [-o out.json]`
/// `clusters doc.json [--k N] [--gap N] [--min-area N] [--raster 4096] [--check c.svg] [-o out.json]`
fn group_command(cmd: &str, args: &[String]) -> Result<(), String> {
    use fframes_vector::{
        AssignParams, ClusterParams, GroupMap, LabelLayer, PieceRaster, cut, group, report,
    };
    let doc_path = &args[1];
    let mut doubts: Option<report::Report> = None;
    let mut doc = PieceDocument::load(doc_path).map_err(|e| format!("{doc_path}: {e}"))?;
    let raster_side = opt(args, "--raster").unwrap_or(4096);
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
        let raster = PieceRaster::fit(&doc, raster_side);
        let n = group::name_clusters(&mut doc, &raster, &cp);
        eprintln!("{n} clusters");
    } else {
        let zones_path = opt::<String>(args, "--zones").or_else(|| {
            args.get(2)
                .filter(|a| !a.starts_with('-') && a.to_ascii_lowercase().ends_with(".svg"))
                .cloned()
        });
        let scale: f32 = opt(args, "--scale").unwrap_or(doc.scale as f32);
        let mut map: Option<GroupMap> = None;
        if let Some(z) = &zones_path {
            let svg = std::fs::read_to_string(z).map_err(|e| format!("{z}: {e}"))?;
            let m = GroupMap::from_svg(&svg);
            eprintln!("{z}: {} zone shapes", m.zones.len());
            map = Some(m);
        }
        // --cut: split pieces along the zone borders before the vote
        let mut cut_names = Vec::new();
        if args.iter().any(|a| a == "--cut") {
            let m = map.as_ref().ok_or("--cut needs zones (an .svg)")?;
            let p = cut::CutParams {
                min_share: opt(args, "--cut-min").unwrap_or(0.05),
                ..cut::CutParams::default()
            };
            let s = cut::cut_at_zones(&mut doc, m, scale, &p);
            eprintln!("{} pieces cut into {} parts", s.cut, s.parts);
            cut_names = s.named;
        }
        let raster = PieceRaster::fit(&doc, raster_side);
        let mut layer: Option<LabelLayer> = map.as_ref().map(|m| m.layer(&raster, scale));
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
        let mut counts = group::assign(&mut doc, &raster, &layer, &p);
        if !cut_names.is_empty() {
            cut::keep_names(&mut doc, &cut_names);
            counts.clear();
            for piece in doc.pieces.iter().filter(|q| !q.group.is_empty()) {
                *counts.entry(piece.group.clone()).or_insert(0) += 1;
            }
        }
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
        let r = report::report(
            &doc,
            &raster,
            &layer,
            &report::ReportParams {
                split: opt(args, "--split").unwrap_or(0.3),
            },
        );
        eprintln!("doubtful: {} unnamed, {} split", r.unnamed, r.split);
        for e in r.entries.iter().take(5) {
            let [x, y, w, h] = e.bbox;
            let shares: Vec<String> = e
                .shares
                .iter()
                .take(3)
                .map(|(n, s)| format!("{} {:.0} %", if n.is_empty() { "-" } else { n }, s * 100.0))
                .collect();
            eprintln!(
                "  piece {} ({:?}, {:.0} px² visible, box {x:.0},{y:.0} {w:.0}x{h:.0}): {}",
                e.id,
                e.doubt,
                e.visible,
                shares.join(", ")
            );
        }
        if let Some(path) = opt::<String>(args, "--report") {
            let json = serde_json::to_string_pretty(&r).map_err(|e| e.to_string())?;
            std::fs::write(&path, json).map_err(|e| format!("{path}: {e}"))?;
            eprintln!("report: {path}");
        }
        doubts = Some(r);
    }
    let out = opt::<String>(args, "-o").unwrap_or_else(|| doc_path.clone());
    doc.save(&out).map_err(|e| format!("{out}: {e}"))?;
    if let Some(check) = opt::<String>(args, "--check") {
        let svg = match &doubts {
            Some(r) => doc.to_report_svg(r),
            None => doc.to_group_check_svg(),
        };
        std::fs::write(&check, svg).map_err(|e| format!("{check}: {e}"))?;
        eprintln!("check picture: {check}");
    }
    Ok(())
}

/// `count` frames spread evenly over one turn of a video (0: every frame), read with the
/// `ffmpeg` command into a temporary folder that is removed again.
fn turntable_frames(video: &str, count: usize) -> Result<Vec<image::RgbaImage>, String> {
    let probe = std::process::Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", video])
        .output()
        .map_err(|e| format!("ffprobe: {e}"))?;
    let seconds: f64 = String::from_utf8_lossy(&probe.stdout)
        .trim()
        .parse()
        .map_err(|_| format!("{video}: no duration"))?;
    let dir = std::env::temp_dir().join(format!("fframes-turntable-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut ffmpeg = std::process::Command::new("ffmpeg");
    ffmpeg.args(["-loglevel", "error", "-y", "-i", video]);
    if count > 0 {
        ffmpeg
            .arg("-vf")
            .arg(format!("fps={count}/{seconds}"))
            .args(["-frames:v", &count.to_string()]);
    }
    let status = ffmpeg.arg(dir.join("f_%03d.png")).status().map_err(|e| format!("ffmpeg: {e}"));
    let frames = status.and_then(|s| {
        if !s.success() {
            return Err(format!("ffmpeg failed on {video}"));
        }
        let count = if count > 0 {
            count
        } else {
            (1..).take_while(|i| dir.join(format!("f_{i:03}.png")).exists()).count()
        };
        (1..=count)
            .map(|i| {
                let f = dir.join(format!("f_{i:03}.png"));
                image::open(&f)
                    .map(|im| im.to_rgba8())
                    .map_err(|e| format!("{}: {e}", f.display()))
            })
            .collect::<Result<Vec<_>, _>>()
    });
    let _ = std::fs::remove_dir_all(&dir);
    frames
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let start = std::time::Instant::now();
    match args.first().map(String::as_str) {
        Some("vectorize") if args.len() >= 3 => {
            let (input, output) = (&args[1], &args[2]);
            let scale: u32 = opt(&args, "--scale").unwrap_or(2);
            let mut p = match opt::<String>(&args, "--preset").as_deref() {
                Some("drawing") => VectorizeParams::drawing(scale),
                _ => VectorizeParams::outlined(scale),
            };
            if let Some(c) = opt(&args, "--colors") {
                p.colors = c;
            }
            // --gap N: close outline gaps up to N source pixels (0 = off)
            if let Some(g) = opt::<usize>(&args, "--gap") {
                p.flatten.gap = g * scale.max(1) as usize;
            }
            if args.iter().any(|a| a == "--clean") {
                p.scan = Some(fframes_vector::drawing::ScanParams::new(1));
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
        Some("turntable") if args.len() >= 3 => {
            // turntable video.mp4 out.glb [--height 1.7] [--frames 36] [--keep N] [--resolution 192] [--direction cw|ccw]
            use fframes_vector::figure::{TurntableParams, turntable_glb};
            let (input, output) = (&args[1], &args[2]);
            let count: usize = opt(&args, "--frames").unwrap_or(36);
            let mut p = TurntableParams::default();
            if let Some(h) = opt(&args, "--height") {
                p.height = h;
            }
            if let Some(r) = opt(&args, "--resolution") {
                p.resolution = r;
            }
            p.keep = opt(&args, "--keep");
            p.clockwise = match opt::<String>(&args, "--direction").as_deref() {
                Some("cw") => Some(true),
                Some("ccw") => Some(false),
                _ => None,
            };
            let result = (|| -> Result<_, String> {
                let frames = turntable_frames(input, count)?;
                let (glb, t) = turntable_glb(&frames, &p)?;
                std::fs::write(output, &glb).map_err(|e| format!("{output}: {e}"))?;
                Ok((glb.len(), t))
            })();
            match result {
                Ok((bytes, t)) => {
                    eprintln!(
                        "{output}: {:.1} MB, {} m high, {} frames, turning {}{}, a point kept inside {} of them, {} voxels ({:.1} s)",
                        bytes as f64 / 1_048_576.0,
                        p.height,
                        t.frames,
                        if t.clockwise { "clockwise" } else { "anticlockwise" },
                        t.disagreement.map_or_else(String::new, |(a, b)| format!(
                            " (colour disagreement {a:.1} against {b:.1} the other way)"
                        )),
                        t.keep,
                        t.volume,
                        start.elapsed().as_secs_f32()
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("depth") if args.len() >= 3 => {
            // depth video.mp4 out.glb [--height 1.7] [--frames 0 (all)] [--views 5] [--window 2] [--voxel 1.5]
            //   [--sharpness 0.6] [--outside N] [--direction cw|ccw] [--maps dir]
            use fframes_vector::depth::{DepthParams, depth_glb, depth_picture};
            let (input, output) = (&args[1], &args[2]);
            let count: usize = opt(&args, "--frames").unwrap_or(0);
            let mut p = DepthParams::default();
            if let Some(h) = opt(&args, "--height") {
                p.height = h;
            }
            if let Some(v) = opt(&args, "--views") {
                p.views = v;
            }
            if let Some(v) = opt(&args, "--window") {
                p.window = v;
            }
            if let Some(v) = opt(&args, "--voxel") {
                p.voxel = v;
            }
            if let Some(v) = opt(&args, "--sharpness") {
                p.sharpness = v;
            }
            if let Some(v) = opt(&args, "--kept") {
                p.kept = v;
            }
            if let Some(v) = opt(&args, "--surface") {
                p.surface = v;
            }
            p.hull = args.iter().any(|a| a == "--hull");
            if let Some(v) = opt(&args, "--refine") {
                p.refine = v;
            }
            if let Some(v) = opt(&args, "--holdout") {
                p.holdout = v;
            }
            // --low 5000: also out_low.glb with at most 5000 vertices and one baked texture
            p.low = opt(&args, "--low");
            // --splat dir: a Gaussian splatting dataset (Nerfstudio format) from the measured turn
            p.splat = opt::<String>(&args, "--splat").map(std::path::PathBuf::from);
            // --check 1,30,60: loss pictures of these frames (1-based) into the --maps folder
            if let Some(list) = opt::<String>(&args, "--check") {
                p.check = list.split(',').filter_map(|f| f.trim().parse::<usize>().ok()).map(|f| f.saturating_sub(1)).collect();
            }
            p.outside = opt(&args, "--outside");
            p.clockwise = match opt::<String>(&args, "--direction").as_deref() {
                Some("cw") => Some(true),
                Some("ccw") => Some(false),
                _ => None,
            };
            let result = (|| -> Result<_, String> {
                let frames = turntable_frames(input, count)?;
                let (glb, t, maps) = depth_glb(&frames, &p)?;
                std::fs::write(output, &glb).map_err(|e| format!("{output}: {e}"))?;
                if let Some(dir) = opt::<String>(&args, "--maps") {
                    std::fs::create_dir_all(&dir).map_err(|e| format!("{dir}: {e}"))?;
                    for f in 0..maps.maps.len() {
                        let path = format!("{dir}/depth_{:03}.png", f + 1);
                        depth_picture(&maps, f).save(&path).map_err(|e| format!("{path}: {e}"))?;
                    }
                }
                // the reduced model next to it: out_low.glb
                if let Some((low, info)) = &t.low {
                    let stem = std::path::Path::new(output).with_extension("");
                    let path = format!("{}_low.glb", stem.display());
                    std::fs::write(&path, low).map_err(|e| format!("{path}: {e}"))?;
                    eprintln!(
                        "{path}: {:.1} MB, {} vertices, {} triangles, texture {} px; fit to the frames that shaped it: missed {:.1} %, excess {:.1} %, overlap {:.3}, outline distance {:.2} px; held-out frames: overlap {:.3}, outline distance {:.2} px",
                        low.len() as f64 / 1_048_576.0,
                        info.vertices,
                        info.triangles,
                        info.texture,
                        info.fit_used.missed * 100.0,
                        info.fit_used.excess * 100.0,
                        info.fit_used.iou,
                        info.fit_used.boundary,
                        info.fit_held.iou,
                        info.fit_held.boundary
                    );
                }
                // loss pictures next to the model: out_loss_030.png
                for c in &t.checks {
                    let path = std::path::Path::new(output).with_extension("");
                    let path = format!("{}_loss_{:03}.png", path.display(), c.frame + 1);
                    c.picture.save(&path).map_err(|e| format!("{path}: {e}"))?;
                    eprintln!(
                        "{path}: frame {}, {} figure pixels, lost {} by the outline check (red), {} by the depth check (blue), excess {} (magenta)",
                        c.frame + 1,
                        c.figure,
                        c.by_outline,
                        c.by_depth,
                        c.excess
                    );
                }
                Ok((glb.len(), t))
            })();
            match result {
                Ok((bytes, t)) => {
                    eprintln!(
                        "{output}: {:.1} MB, {} frames, half turn {:.2} frames ({:.3}° per frame, {:.0}° in all), axis x {:.1} (mirror error {:.2} px), turning {}{}, depth known for {:.0} % of figure pixels, {} voxels ({:.1} s)",
                        bytes as f64 / 1_048_576.0,
                        t.frames,
                        t.half_turn,
                        t.step_degrees,
                        t.step_degrees * (t.frames - 1) as f32,
                        t.axis,
                        t.mirror_error,
                        if t.clockwise { "clockwise" } else { "anticlockwise" },
                        t.violations.map_or_else(String::new, |(a, b)| format!(
                            " ({:.1} % of points in front of another frame's surface, {:.1} % the other way)",
                            a * 100.0,
                            b * 100.0
                        )),
                        t.known * 100.0,
                        t.voxels,
                        start.elapsed().as_secs_f32()
                    );
                    for (r, (mean, max)) in t.corrections.iter().enumerate() {
                        eprintln!("angle round {}: frames corrected by {mean:.2}° on average, at most {max:.2}°", r + 1);
                    }
                    eprintln!("{} loose fragments removed", t.fragments);
                    for (name, fit) in [("frames that shaped it", &t.fit_used), ("held-out frames", &t.fit_held)] {
                        eprintln!(
                            "fit to the {name} ({}): missed {:.1} %, excess {:.1} %, overlap {:.3}, outline distance {:.2} px",
                            fit.frames,
                            fit.missed * 100.0,
                            fit.excess * 100.0,
                            fit.iou,
                            fit.boundary
                        );
                    }
                    eprintln!("height band (top to bottom): outline disagreement half a turn later / kept / removed by outline / removed by depth");
                    for (b, [kept, outline, depth]) in t.bands.iter().enumerate() {
                        eprintln!(
                            "  {:>3}-{:>3} %: {:>5.1} px / {kept:>9} / {outline:>9} / {depth:>9}",
                            b * 10,
                            b * 10 + 10,
                            t.disagreement.get(b).copied().unwrap_or(0.0)
                        );
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("figure") if args.len() >= 3 => {
            // figure sheet.png out.glb [--height 1.7] [--resolution 256] [--order front,side,back] [--side-faces right]
            use fframes_vector::figure::{FigureParams, View, figure_glb};
            let (input, output) = (&args[1], &args[2]);
            let mut p = FigureParams::default();
            if let Some(h) = opt(&args, "--height") {
                p.height = h;
            }
            if let Some(r) = opt(&args, "--resolution") {
                p.resolution = r;
            }
            if opt::<String>(&args, "--side-faces").as_deref() == Some("right") {
                p.side_faces_left = false;
            }
            if let Some(o) = opt::<String>(&args, "--order") {
                let v: Vec<View> = o
                    .split(',')
                    .filter_map(|x| match x.trim() {
                        "front" => Some(View::Front),
                        "side" => Some(View::Side),
                        "back" => Some(View::Back),
                        _ => None,
                    })
                    .collect();
                if let [a, b, c] = v[..] {
                    p.order = Some([a, b, c]);
                }
            }
            let result = std::fs::read(input)
                .map_err(|e| format!("{input}: {e}"))
                .and_then(|png| figure_glb(&png, &p))
                .and_then(|(glb, views)| {
                    std::fs::write(output, &glb).map_err(|e| format!("{output}: {e}"))?;
                    Ok((glb.len(), views))
                });
            match result {
                Ok((bytes, views)) => {
                    for v in &views {
                        eprintln!("{:?}: x {} y {}, {} x {} px", v.view, v.x, v.y, v.w, v.h);
                    }
                    eprintln!(
                        "{output}: {:.1} MB, {} m high ({:.1} s)",
                        bytes as f64 / 1_048_576.0,
                        p.height,
                        start.elapsed().as_secs_f32()
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("gltf") if args.len() >= 3 => {
            // gltf doc.json out.glb [--depth N] [--unit M] [--tolerance T] [--per-piece] [--no-outline] [--group name]
            let doc = match PieceDocument::load(&args[1]) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("{}: {e}", args[1]);
                    return ExitCode::FAILURE;
                }
            };
            let p = fframes_vector::gltf::GltfParams {
                tolerance: opt(&args, "--tolerance").unwrap_or(0.5),
                depth: opt(&args, "--depth").unwrap_or(0.0),
                unit: opt(&args, "--unit"),
                per_piece: args.iter().any(|a| a == "--per-piece"),
                skip_outline: args.iter().any(|a| a == "--no-outline"),
                group: opt::<String>(&args, "--group"),
            };
            let glb = fframes_vector::gltf::to_glb(&doc, &p);
            match std::fs::write(&args[2], &glb) {
                Ok(()) => {
                    eprintln!(
                        "{} ({:.1} MB, {:.1} s)",
                        args[2],
                        glb.len() as f64 / 1e6,
                        start.elapsed().as_secs_f32()
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{}: {e}", args[2]);
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
                "fframes-vector vectorize in.png out.json [--scale N] [--colors N] [--svg out.svg] [--flat flat.png]\nfframes-vector group doc.json zones.svg|--mask m.png [--cut [--cut-min 0.05]] [--legend l.json] [--clusters] [--share 0.5] [--report r.json] [--check c.svg] [-o out.json]\nfframes-vector clusters doc.json [--k N] [--gap N] [--min-area N] [--check c.svg] [-o out.json]\nfframes-vector turntable video.mp4 out.glb [--height 1.7] [--frames 36] [--keep N] [--direction cw|ccw]\nfframes-vector figure sheet.png out.glb [--height 1.7] [--resolution 256] [--order front,side,back] [--side-faces right]\nfframes-vector svg doc.json out.svg [--grouped]\nfframes-vector sprites doc.json dir [--density 1,2] [--group name] [--no-outline]"
            );
            ExitCode::FAILURE
        }
    }
}
