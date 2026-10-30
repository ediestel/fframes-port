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
/// The vertex positions of a binary glTF's first mesh (its first accessor, as fframes-vector
/// writes them), metres.
fn glb_positions(path: &str) -> Result<Vec<[f32; 3]>, String> {
    let data = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let word = |o: usize| u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]) as usize;
    if data.len() < 20 || &data[..4] != b"glTF" {
        return Err(format!("{path}: not a binary glTF"));
    }
    let json_len = word(12);
    let doc: serde_json::Value = serde_json::from_slice(&data[20..20 + json_len]).map_err(|e| format!("{path}: {e}"))?;
    let bin = 20 + json_len + 8;
    let acc = &doc["accessors"][0];
    let view = &doc["bufferViews"][acc["bufferView"].as_u64().unwrap_or(0) as usize];
    let (offset, count) = (view["byteOffset"].as_u64().unwrap_or(0) as usize, acc["count"].as_u64().unwrap_or(0) as usize);
    let f = |o: usize| f32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]);
    Ok((0..count).map(|i| std::array::from_fn(|c| f(bin + offset + (i * 3 + c) * 4))).collect())
}

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
            // the frames ffmpeg wrote: f_001.png onwards
            std::fs::read_dir(&dir)
                .map_err(|e| format!("{}: {e}", dir.display()))?
                .filter_map(Result::ok)
                .filter(|e| e.file_name().to_string_lossy().starts_with("f_"))
                .count()
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

/// Every expression of the library (the one that comes with fframes, or `library`) as morph
/// target offsets for the model's vertices.
fn expression_morphs(positions: &[[f32; 3]], face: &fframes_rig::face::Face, library: Option<String>) -> Result<Vec<(String, Vec<[f32; 3]>)>, String> {
    let text = match &library {
        Some(path) => std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?,
        None => fframes_rig::face::LIBRARY.to_string(),
    };
    let library = fframes_rig::face::Library::parse(&text)?;
    library
        .expressions
        .keys()
        .map(|name| fframes_rig::face::expression(positions, face, &library, name).map(|o| (name.clone(), o)))
        .collect()
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
        Some("points") if args.len() >= 3 => {
            // points video.mp4 out.ply [--height 1.7] [--frames 72] [--step 2] [--max-disagreement 30] [--clockwise]
            use fframes_vector::points::{PointsParams, to_ply, turntable_points};
            let (input, output) = (&args[1], &args[2]);
            let count: usize = opt(&args, "--frames").unwrap_or(72);
            let height: f32 = opt(&args, "--height").unwrap_or(1.7);
            let mut p = PointsParams::default();
            if let Some(s) = opt(&args, "--step") {
                p.step = s;
            }
            if let Some(m) = opt(&args, "--max-disagreement") {
                p.max_disagreement = m;
            }            p.clockwise = args.iter().any(|a| a == "--clockwise");
            let result = (|| -> Result<_, String> {
                let frames = turntable_frames(input, count)?;
                let (points, support) = turntable_points(&frames, &p)?;
                let top = points.iter().map(|q| q.y).fold(0.0_f32, f32::max).max(1.0);
                std::fs::write(output, to_ply(&points, height / top)).map_err(|e| format!("{output}: {e}"))?;
                // the same points as dots for a 3D viewer, next to the PLY
                let glb = std::path::Path::new(output).with_extension("glb");
                std::fs::write(&glb, fframes_vector::points::to_glb(&points, height / top, 0.008))
                    .map_err(|e| format!("{}: {e}", glb.display()))?;
                Ok((points, support, height / top))
            })();
            match result {
                Ok((points, support, scale)) => {
                    let ext = |f: fn(&fframes_vector::points::Point) -> f32| {
                        points.iter().map(f).fold((f32::MAX, f32::MIN), |m, v| (m.0.min(v), m.1.max(v)))
                    };
                    let (x, y, z) = (ext(|q| q.x), ext(|q| q.y), ext(|q| q.z));
                    eprintln!(
                        "{output}: {} points; x {:.2}..{:.2} m, y {:.2}..{:.2} m, z {:.2}..{:.2} m; median support {:.0} % ({:.1} s)",
                        points.len(),
                        x.0 * scale, x.1 * scale, y.0 * scale, y.1 * scale, z.0 * scale, z.1 * scale,
                        support * 100.0,
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
            if let Some(v) = opt(&args, "--smooth") {
                p.smooth = v;
            }
            if let Some(v) = opt(&args, "--holdout") {
                p.holdout = v;
            }
            // --low 5000: also out_low.glb with at most 5000 vertices and one baked texture
            // --low N: the reduced model's vertex budget (default 5000; 0: none)
            if let Some(v) = opt::<usize>(&args, "--low") {
                p.low = (v > 0).then_some(v);
            }
            if let Some(v) = opt(&args, "--search") {
                p.search = v;
            }
            // --half-turn 88: frames per half turn, when the clip is too short for it to be found
            if let Some(v) = opt(&args, "--half-turn") {
                p.half = Some(v);
            }
            // --splat dir: a Gaussian splatting dataset (Nerfstudio format) from the measured turn
            p.splat = opt::<String>(&args, "--splat").map(std::path::PathBuf::from);
            // --cuts figure.rig.json: the rig editor's cuts taken out of the solid
            if let Some(path) = opt::<String>(&args, "--cuts") {
                match fframes_rig::RigFile::load(std::path::Path::new(&path)) {
                    Ok(rig) => p.cuts = Some(rig),
                    Err(e) => {
                        eprintln!("{e}");
                        return ExitCode::FAILURE;
                    }
                }
            }
            // --solid out.solid: the solid with its transform, for editing
            p.solid = opt::<String>(&args, "--solid").map(std::path::PathBuf::from);
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
                        "{path}: {:.1} MB, {} vertices, {} triangles, texture {} px; fit to the frames that shaped it: missed {:.1} %, excess {:.1} %, overlap {:.3}, outline distance {:.2} px; held-out frames: overlap {:.3}, outline distance {:.2} px; normal map detail: median {:.1}°, p90 {:.1}°",
                        low.len() as f64 / 1_048_576.0,
                        info.vertices,
                        info.triangles,
                        info.texture,
                        info.fit_used.missed * 100.0,
                        info.fit_used.excess * 100.0,
                        info.fit_used.iou,
                        info.fit_used.boundary,
                        info.fit_held.iou,
                        info.fit_held.boundary,
                        info.normal_detail.0,
                        info.normal_detail.1
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
                    eprintln!("{} loose fragments removed; voxel {:.1} mm", t.fragments, t.voxel_metres * 1000.0);
                    if p.cuts.is_some() {
                        eprintln!("cuts took out {} voxels", t.cut_voxels);
                    }
                    for (name, fit) in [
                        ("frames that shaped it", &t.fit_used),
                        ("held-out frames", &t.fit_held),
                        ("frames that shaped it, exported surface", &t.fit_mesh_used),
                        ("held-out frames, exported surface", &t.fit_mesh_held),
                    ] {
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
        Some("splat-depth") if args.len() >= 3 => {
            // splat-depth splats.ply dataset_dir [--every 8] [--offset 3] [--mesh model.glb] [--points out.ply]:
            // depth diagnostics of trained splats, and their cross-sections against a mesh
            use fframes_vector::splat::{depth_report, read_cameras, read_splats};
            let (ply, dir) = (&args[1], std::path::Path::new(&args[2]));
            let every: usize = opt(&args, "--every").unwrap_or(8);
            let offset: usize = opt(&args, "--offset").unwrap_or(3);
            let run = || -> Result<(), String> {
                let splats = read_splats(std::path::Path::new(ply))?;
                let cams = read_cameras(&dir.join("transforms.json"))?;
                let (report, points) = depth_report(&splats, &cams, dir, every, offset)?;
                eprintln!("{report}");
                // --points out.ply: the confident surface points (world, metres)
                if let Some(out) = opt::<String>(&args, "--points") {
                    let mut ply = format!(
                        "ply\nformat binary_little_endian 1.0\nelement vertex {}\nproperty float x\nproperty float y\nproperty float z\nend_header\n",
                        points.len()
                    )
                    .into_bytes();
                    ply.extend(points.iter().flatten().flat_map(|v| v.to_le_bytes()));
                    std::fs::write(&out, ply).map_err(|e| format!("{out}: {e}"))?;
                    eprintln!("{out}: {} surface points", points.len());
                }
                // cross-sections: per tenth of the height, the width in x and z (2-98 %)
                let bands = |pts: &[[f32; 3]]| -> Vec<(f32, f32)> {
                    let (lo, hi) = pts.iter().fold((f32::MAX, f32::MIN), |m, p| (m.0.min(p[1]), m.1.max(p[1])));
                    (0..10)
                        .map(|b| {
                            let (y0, y1) = (hi - (hi - lo) * (b + 1) as f32 / 10.0, hi - (hi - lo) * b as f32 / 10.0);
                            let slice: Vec<&[f32; 3]> = pts.iter().filter(|p| p[1] >= y0 && p[1] <= y1).collect();
                            let range = |c: usize| {
                                let mut v: Vec<f32> = slice.iter().map(|p| p[c]).collect();
                                v.sort_by(f32::total_cmp);
                                if v.len() < 10 { f32::NAN } else { v[v.len() * 98 / 100] - v[v.len() * 2 / 100] }
                            };
                            (range(0), range(2))
                        })
                        .collect()
                };
                let splat_bands = bands(&points);
                let mesh_bands = match opt::<String>(&args, "--mesh") {
                    Some(m) => Some(bands(&glb_positions(&m)?)),
                    None => None,
                };
                eprintln!("cross-sections, top to bottom: width x / z in cm, splat surface{}", if mesh_bands.is_some() { " against the mesh" } else { "" });
                for (b, (sx, sz)) in splat_bands.iter().enumerate() {
                    match &mesh_bands {
                        Some(mb) => eprintln!(
                            "  {:>3}-{:>3} %: {:5.1} / {:5.1}   mesh {:5.1} / {:5.1}",
                            b * 10, b * 10 + 10, sx * 100.0, sz * 100.0, mb[b].0 * 100.0, mb[b].1 * 100.0
                        ),
                        None => eprintln!("  {:>3}-{:>3} %: {:5.1} / {:5.1}", b * 10, b * 10 + 10, sx * 100.0, sz * 100.0),
                    }
                }
                Ok(())
            };
            match run() {
                Ok(()) => {
                    eprintln!("({:.1} s)", start.elapsed().as_secs_f32());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("parts") if args.len() >= 2 => {
            // parts model.glb [--cell 0.015]: the parts of a standing figure that move on their
            // own (legs, an arm free of the body) and where they join, from height slices
            let run = || -> Result<(), String> {
                let data = std::fs::read(&args[1]).map_err(|e| format!("{}: {e}", args[1]))?;
                let (positions, indices) = fframes_vector::rig::read_glb_mesh(&data).map_err(|e| format!("{}: {e}", args[1]))?;
                let p = fframes_vector::rig::parts(&positions, &indices, opt(&args, "--cell").unwrap_or(0.015));
                eprintln!("{}: {:.2} m high, {} parts ({} cm layers, {} small sections left out)", args[1], p.height, p.parts.len(), p.cell * 100.0, p.dropped);
                for (i, q) in p.parts.iter().enumerate() {
                    eprintln!(
                        "{i:3}: {:4.2}-{:4.2} m ({:3.0}-{:3.0} %), centre x {:5.2} z {:5.2}, section {:5.0} cm², below {:?}, above {:?}",
                        q.bottom,
                        q.top,
                        100.0 * q.bottom / p.height,
                        100.0 * q.top / p.height,
                        q.centre[0],
                        q.centre[1],
                        q.area * 1e4,
                        q.below,
                        q.above
                    );
                }
                Ok(())
            };
            match run() {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("expressions") if args.len() >= 4 => {
            // expressions model.glb rig.json out.glb [--library expressions.json]: the model with
            // every expression of the library as a morph target (Blender shape keys, Unreal
            // morph targets), from the face points placed as joints; no skeleton
            let run = || -> Result<(), String> {
                let data = std::fs::read(&args[1]).map_err(|e| format!("{}: {e}", args[1]))?;
                let (positions, _) = fframes_vector::rig::read_glb_mesh(&data).map_err(|e| format!("{}: {e}", args[1]))?;
                let rig = fframes_rig::RigFile::load(std::path::Path::new(&args[2]))?;
                let face = fframes_rig::face::Face::from_joints(&rig.joints)?;
                let morphs = expression_morphs(&positions, &face, opt::<String>(&args, "--library"))?;
                let out = fframes_vector::rig::export_rigged(&data, None, &morphs).map_err(|e| format!("{}: {e}", args[1]))?;
                std::fs::write(&args[3], out).map_err(|e| format!("{}: {e}", args[3]))?;
                eprintln!("{}: mouth {:.1} cm wide", args[3], face.scale * 100.0);
                for (name, offsets) in &morphs {
                    let moved = offsets.iter().filter(|o| o.iter().any(|v| v.abs() > 1e-6)).count();
                    let most = offsets.iter().map(|o| (o[0] * o[0] + o[1] * o[1] + o[2] * o[2]).sqrt()).fold(0.0, f32::max);
                    eprintln!("  {name:12} {moved:5} vertices move, at most {:.1} mm", most * 1000.0);
                }
                Ok(())
            };
            match run() {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("rig-export") if args.len() >= 4 => {
            // rig-export model.glb rig.json out.glb [--falloff 0.04] [--library expressions.json]:
            // with the face points placed, also every expression as a morph target; the model with the rig's
            // skeleton and soft skin weights (up to four joints per vertex), for Blender and
            // Unreal; mesh, UVs and textures unchanged
            let run = || -> Result<(), String> {
                let data = std::fs::read(&args[1]).map_err(|e| format!("{}: {e}", args[1]))?;
                let (positions, _) = fframes_vector::rig::read_glb_mesh(&data).map_err(|e| format!("{}: {e}", args[1]))?;
                let rig = fframes_rig::RigFile::load(std::path::Path::new(&args[2]))?;
                let falloff = opt(&args, "--falloff").unwrap_or(0.04);
                let skinning = fframes_rig::skin_weights(&positions, &rig.joints, falloff).ok_or("the rig has no bones: place and accept joints first")?;
                // the expressions too, once the face points are placed
                let morphs = match fframes_rig::face::Face::from_joints(&rig.joints) {
                    Ok(face) => expression_morphs(&positions, &face, opt::<String>(&args, "--library"))?,
                    Err(e) => {
                        eprintln!("no expressions: {e}");
                        Vec::new()
                    }
                };
                let out = fframes_vector::rig::export_rigged(&data, Some(&skinning), &morphs).map_err(|e| format!("{}: {e}", args[1]))?;
                std::fs::write(&args[3], out).map_err(|e| format!("{}: {e}", args[3]))?;
                let shared = skinning.influences.iter().filter(|inf| inf[1].1 > 0.0).count();
                eprintln!(
                    "{}: {} joints, {} vertices skinned ({shared} shared between joints, falloff {:.0} mm)",
                    args[3],
                    skinning.joints.len(),
                    positions.len(),
                    falloff * 1000.0
                );
                for (j, (name, parent, _)) in skinning.joints.iter().enumerate() {
                    let main = skinning.influences.iter().filter(|inf| inf[0].0 as usize == j && inf[0].1 > 0.0).count();
                    eprintln!("  {name:12} parent {:12} {main:5} vertices mainly", parent.map_or("-", |p| skinning.joints[p].0.as_str()));
                }
                Ok(())
            };
            match run() {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("splat-scene") if args.len() >= 4 => {
            // splat-scene walk.mp4 cameras.json out.mp4 --splat file.ply:height:x:z [--splat ...]:
            // Gaussian splats drawn into a fframes-walk video, through the cameras it logged
            // (--camera-log); all splats are rendered together, so they hide each other, and
            // laid over the frames (the walk stays inside its room: nothing of it is in front)
            use fframes_vector::splat::{read_splats, render_layers, stand, walk_camera};
            let (walk, log, output) = (&args[1], &args[2], &args[3]);
            let run = || -> Result<String, String> {
                let mut splats = Vec::new();
                for (i, a) in args.iter().enumerate() {
                    if a != "--splat" {
                        continue;
                    }
                    let spec = args.get(i + 1).ok_or("--splat needs file.ply:height:x:z")?;
                    let parts: Vec<&str> = spec.rsplitn(4, ':').collect();
                    let [z, x, height, path] = parts[..] else {
                        return Err(format!("{spec}: --splat is file.ply:height:x:z"));
                    };
                    let num = |v: &str| v.parse::<f32>().map_err(|_| format!("{spec}: {v} is no number"));
                    let mut s = read_splats(std::path::Path::new(path))?;
                    stand(&mut s, num(height)?, num(x)?, num(z)?)?;
                    eprintln!("{path}: {} splats, {height} m high at x {x}, z {z}", s.len());
                    splats.extend(s);
                }
                if splats.is_empty() {
                    return Err("no --splat given".into());
                }
                let text = std::fs::read_to_string(log).map_err(|e| format!("{log}: {e}"))?;
                let doc: Vec<serde_json::Value> = serde_json::from_str(&text).map_err(|e| format!("{log}: {e}"))?;
                let v3 = |v: &serde_json::Value| -> [f32; 3] { std::array::from_fn(|c| v[c].as_f64().unwrap_or(0.0) as f32) };
                let cams: Vec<_> = doc
                    .iter()
                    .map(|c| {
                        let size = |k: &str| c[k].as_u64().unwrap_or(0) as u32;
                        walk_camera(v3(&c["position"]), v3(&c["target"]), c["fov"].as_f64().unwrap_or(60.0) as f32, size("width"), size("height"))
                    })
                    .collect();
                let first = cams.first().ok_or_else(|| format!("{log}: no cameras"))?;
                let (w, h) = (first.width, first.height);
                let mut decode = std::process::Command::new("ffmpeg")
                    .args(["-loglevel", "error", "-i", walk, "-f", "rawvideo", "-pix_fmt", "rgba", "-"])
                    .stdout(std::process::Stdio::piped())
                    .spawn()
                    .map_err(|e| format!("ffmpeg: {e}"))?;
                let mut encode = std::process::Command::new("ffmpeg")
                    .args(["-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba", "-s"])
                    .arg(format!("{w}x{h}"))
                    .args(["-r", "30", "-i", "-", "-c:v", "libx264", "-preset", "slow", "-crf", "17", "-pix_fmt", "yuv420p", "-movflags", "+faststart", output])
                    .stdin(std::process::Stdio::piped())
                    .spawn()
                    .map_err(|e| format!("ffmpeg: {e}"))?;
                let (mut frames_in, mut frames_out) = (
                    decode.stdout.take().ok_or("ffmpeg: no output")?,
                    encode.stdin.take().ok_or("ffmpeg: no input")?,
                );
                let mut frame = vec![0u8; (w * h * 4) as usize];
                let mut done = 0;
                for cam in &cams {
                    if std::io::Read::read_exact(&mut frames_in, &mut frame).is_err() {
                        break;
                    }
                    let r = render_layers(&splats, cam, 1.0, [0, 0, 0]);
                    // the splats' colour is over black: add the walk where they let it through
                    for ((out, px), alpha) in frame.as_chunks_mut::<4>().0.iter_mut().zip(r.image.pixels()).zip(&r.alpha) {
                        let keep = 1.0 - alpha;
                        for (o, splat) in out.iter_mut().zip(px.0).take(3) {
                            *o = (f32::from(splat) + keep * f32::from(*o)).round().clamp(0.0, 255.0) as u8;
                        }
                    }
                    std::io::Write::write_all(&mut frames_out, &frame).map_err(|e| format!("ffmpeg: {e}"))?;
                    done += 1;
                    if done % 30 == 0 {
                        eprint!("\r{done}/{} frames", cams.len());
                    }
                }
                drop(frames_out);
                let _ = decode.wait();
                if !encode.wait().map_err(|e| format!("ffmpeg: {e}"))?.success() {
                    return Err(format!("ffmpeg failed on {output}"));
                }
                if done != cams.len() {
                    return Err(format!("{walk} has {done} frames, the log {} cameras", cams.len()));
                }
                Ok(format!("\r{output}: {done} frames, {} splats", splats.len()))
            };
            match run() {
                Ok(report) => {
                    eprintln!("{report} ({:.1} s)", start.elapsed().as_secs_f32());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("splat-video") if args.len() >= 4 => {
            // splat-video splats.ply dataset_dir out.mp4 [--scale 2] [--fps 24]: the splats seen
            // by every camera of the dataset's transforms.json, encoded with ffmpeg; every 8th
            // camera also checked against its image's outline
            use fframes_vector::splat::{read_cameras, read_splats, render};
            let (ply, dir, output) = (&args[1], std::path::Path::new(&args[2]), &args[3]);
            let scale: f32 = opt(&args, "--scale").unwrap_or(2.0);
            let fps: u32 = opt(&args, "--fps").unwrap_or(24);
            let run = || -> Result<String, String> {
                let splats = read_splats(std::path::Path::new(ply))?;
                let cams = read_cameras(&dir.join("transforms.json"))?;
                let first = cams.first().ok_or("no cameras")?.1;
                let (vw, vh) = (
                    ((first.width as f32 * scale).round() as u32) & !1,
                    ((first.height as f32 * scale).round() as u32) & !1,
                );
                // frames straight into ffmpeg as raw RGBA, nothing on disk
                let mut ffmpeg = std::process::Command::new("ffmpeg")
                    .args(["-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba", "-s"])
                    .arg(format!("{vw}x{vh}"))
                    .args(["-framerate", &fps.to_string(), "-i", "-", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "16", output])
                    .stdin(std::process::Stdio::piped())
                    .spawn()
                    .map_err(|e| format!("ffmpeg: {e}"))?;
                let mut pipe = ffmpeg.stdin.take().ok_or("ffmpeg: no input")?;
                let mut fits = Vec::new();
                for (i, (name, cam)) in cams.iter().enumerate() {
                    let (img, _) = render(&splats, cam, scale, [40, 40, 40]);
                    let img = image::imageops::crop_imm(&img, 0, 0, vw, vh).to_image();
                    std::io::Write::write_all(&mut pipe, img.as_raw()).map_err(|e| format!("ffmpeg: {e}"))?;
                    if i % 8 == 0 {
                        let (_, cover) = render(&splats, cam, 1.0, [0, 0, 0]);
                        let image = image::open(dir.join(name)).map_err(|e| format!("{name}: {e}"))?.to_rgba8();
                        let mask: Vec<bool> = image.pixels().map(|p| p.0[3] > 127).collect();
                        let shadow: Vec<bool> = cover.iter().map(|&a| a > 0.5).collect();
                        fits.push(fframes_vector::depth::fit(&shadow, &mask, image.width() as usize, image.height() as usize));
                    }
                }
                drop(pipe);
                if !ffmpeg.wait().map_err(|e| format!("ffmpeg: {e}"))?.success() {
                    return Err(format!("ffmpeg failed on {output}"));
                }
                let n = fits.len().max(1) as f32;
                Ok(format!(
                    "{output}: {} splats, {} frames at {scale}x, {fps} fps; outline against the images ({} frames): missed {:.1} %, excess {:.1} %, overlap {:.3}, outline distance {:.2} px",
                    splats.len(),
                    cams.len(),
                    fits.len(),
                    fits.iter().map(|f| f.missed).sum::<f32>() / n * 100.0,
                    fits.iter().map(|f| f.excess).sum::<f32>() / n * 100.0,
                    fits.iter().map(|f| f.iou).sum::<f32>() / n,
                    fits.iter().map(|f| f.boundary).sum::<f32>() / n
                ))
            };
            match run() {
                Ok(report) => {
                    eprintln!("{report} ({:.1} s)", start.elapsed().as_secs_f32());
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
