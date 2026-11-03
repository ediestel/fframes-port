//! The stages of the round trip, each one a command that writes files into the figure folder
//! and can be run alone or all in a row:
//!
//! 1. [`trace`]: the source picture to vector pieces (fframes-vector, locally): `trace/pieces.json`,
//!    `trace/trace.svg` with the groups `outlines` and `pieces`, `trace/flat.png`.
//! 2. [`export`]: the layers as PNGs at the canvas size for the artist: `export/01_source.png`,
//!    `export/02_pieces.png`, `export/03_outlines.png`, `export/composite.png`, and a
//!    `README.txt` with the steps.
//! 3. The artist draws, outside: layers on top, exported as PNG or SVG into the folder.
//! 4. [`import`]: every artist PNG a layer asks to trace becomes vector pieces,
//!    `trace/<name>.json`; everything is checked.
//!
//! Rendering the stack is the video in `lib.rs`.

use std::fmt::Write as _;
use std::path::Path;

use fframes_vector::{PieceDocument, flat_image, upscale, vectorize};
use image::{RgbImage, RgbaImage};

use crate::figure::{Figure, Kind, Trace, is_png};

/// Group names of the source trace.
pub const OUTLINES: &str = "outlines";
pub const PIECES: &str = "pieces";

/// Stage 1: the source picture to pieces.
pub fn trace(fig: &Figure) -> Result<String, String> {
    let source = load_rgb(&fig.root.join(&fig.source))?;
    let params = fig.trace.params(fig.scale);
    let mut doc = vectorize(&source, &params)?;
    for piece in &mut doc.pieces {
        piece.group = if piece.color == 0 { OUTLINES } else { PIECES }.to_owned();
    }
    let dir = fig.trace_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    doc.save(fig.pieces_path()).map_err(|e| format!("pieces.json: {e}"))?;
    std::fs::write(dir.join("trace.svg"), doc.to_svg_grouped()).map_err(|e| format!("trace.svg: {e}"))?;
    let flat = flat_image(&source, &params);
    flat.save(dir.join("flat.png")).map_err(|e| format!("flat.png: {e}"))?;

    // the centrelines of the outline mask, as strokes of the chosen width
    let lines = crate::lines::centrelines(&flat, fig.trace.line_min.unwrap_or(6), 1.0);
    let json = serde_json::to_string(&lines).map_err(|e| format!("lines.json: {e}"))?;
    std::fs::write(fig.lines_path(), json).map_err(|e| format!("lines.json: {e}"))?;
    std::fs::write(
        dir.join("lines.svg"),
        crate::lines::to_svg(&lines, doc.width, doc.height, fig.line_width(), fig.line_color()),
    )
    .map_err(|e| format!("lines.svg: {e}"))?;

    let outlines = doc.pieces.iter().filter(|p| p.group == OUTLINES).count();
    Ok(format!(
        "trace: {} x {} px, {} pieces ({outlines} outline, {} colour), {} centrelines in {}",
        doc.width,
        doc.height,
        doc.pieces.len(),
        doc.pieces.len() - outlines,
        lines.len(),
        dir.display()
    ))
}

/// The centrelines of the source trace, or the advice to run the stage.
pub fn load_lines(fig: &Figure) -> Result<Vec<crate::lines::Line>, String> {
    let path = fig.lines_path();
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("{}: {e}; run the trace stage first", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Stage 2: the layers as PNGs for the artist.
pub fn export(fig: &Figure) -> Result<String, String> {
    let doc = load_trace(fig)?;
    let dir = fig.export_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let (w, h) = (fig.width, fig.height);

    let source = upscale(&load_rgb(&fig.root.join(&fig.source))?, fig.scale);
    let source_rgba = RgbaImage::from_fn(w, h, |x, y| {
        let p = source.get_pixel(x, y);
        image::Rgba([p[0], p[1], p[2], 255])
    });
    save(&source_rgba, &dir.join("01_source.png"))?;

    // Only the layers the manifest uses are exported, so the artist gets nothing to ignore.
    let mut written = vec!["01_source"];
    let pieces = rasterize(&group_svg(&doc, PIECES), w, h, None)?;
    let uses = |kind: &Kind| fig.layers.iter().any(|layer| &layer.kind == kind);
    if uses(&Kind::TracePieces) {
        save(&pieces, &dir.join("02_pieces.png"))?;
        written.push("02_pieces");
    }
    let outlines = rasterize(&group_svg(&doc, OUTLINES), w, h, None)?;
    if uses(&Kind::TraceOutlines) {
        save(&outlines, &dir.join("03_outlines.png"))?;
        written.push("03_outlines");
    }
    let lines = if uses(&Kind::TraceLines) {
        let centre = load_lines(fig)?;
        let lines = rasterize(
            &crate::lines::to_svg(&centre, w, h, fig.line_width(), fig.line_color()),
            w,
            h,
            None,
        )?;
        save(&lines, &dir.join("04_lines.png"))?;
        written.push("04_lines");
        Some(lines)
    } else {
        None
    };

    let contours = if uses(&Kind::TraceContours) {
        let contours = rasterize(&contours_svg(&doc, fig), w, h, None)?;
        save(&contours, &dir.join("05_contours.png"))?;
        written.push("05_contours");
        Some(contours)
    } else {
        None
    };

    let mut composite = source_rgba;
    for (layer, opacity) in fig.layers.iter().filter_map(|layer| match layer.kind {
        Kind::TracePieces => Some((&pieces, layer.opacity)),
        Kind::TraceOutlines => Some((&outlines, layer.opacity)),
        Kind::TraceLines => lines.as_ref().map(|lines| (lines, layer.opacity)),
        Kind::TraceContours => contours.as_ref().map(|contours| (contours, layer.opacity)),
        _ => None,
    }) {
        blend(&mut composite, layer, opacity);
    }
    save(&composite, &dir.join("composite.png"))?;
    written.push("composite");

    // One layered file for the artist's app (Procreate, Photoshop, Affinity, Krita, GIMP open
    // a PSD with its layers): the composite as the merged picture, then every layer the
    // manifest uses, bottom to top, named, with its opacity baked into its alpha. Written by
    // ImageMagick when it is installed.
    let psd_name = format!("{}.psd", fig.root.file_name().map_or("figure".into(), |n| n.to_string_lossy()));
    let mut psd = std::process::Command::new("magick");
    psd.current_dir(&dir).arg("composite.png");
    for layer in &fig.layers {
        let file = match layer.kind {
            Kind::Source => "01_source.png",
            Kind::TracePieces => "02_pieces.png",
            Kind::TraceOutlines => "03_outlines.png",
            Kind::TraceLines => "04_lines.png",
            Kind::TraceContours => "05_contours.png",
            Kind::File { .. } => continue,
        };
        psd.args(["(", file, "-alpha", "set", "-channel", "A", "-evaluate", "multiply"])
            .arg(format!("{}", layer.opacity))
            .args(["+channel", "-set", "label", &layer.name, ")"]);
    }
    psd.arg(&psd_name);
    match psd.output() {
        Ok(out) if out.status.success() => written.push("the layered PSD"),
        Ok(out) => {
            return Err(format!("{psd_name}: magick: {}", String::from_utf8_lossy(&out.stderr).trim()));
        }
        Err(_) => written.push("no PSD (ImageMagick's `magick` is not installed)"),
    }

    let mut readme = String::new();
    let _ = writeln!(readme, "Figure {} for drawing on top.", fig.root.display());
    let _ = writeln!(readme);
    let _ = writeln!(readme, "Canvas: {w} x {h} px. Every layer you export must have exactly this size.");
    let _ = writeln!(readme);
    let _ = writeln!(readme, "In Procreate, Photoshop, Affinity, Krita or GIMP:");
    let _ = writeln!(readme, "1. Open the .psd in this folder: the layers are there, named and in order, with their");
    let _ = writeln!(readme, "   opacity. (Without it: new canvas {w} x {h} px, insert the numbered PNGs as layers.)");
    let _ = writeln!(readme, "2. Draw on new layers above them. Name each layer; do not draw on the ones that came with the file.");
    let _ = writeln!(readme, "3. Export the layers you drew as PNG files (Procreate: Actions > Share > PNG files),");
    let _ = writeln!(readme, "   or as SVG from a vector app, and copy them into the folder next to source.png,");
    let _ = writeln!(readme, "   named in the order they stack, e.g. 10_draft.png, 20_lines.png.");
    let _ = writeln!(readme, "4. The import stage picks them up; a layer entry in figure.toml can set opacity and tracing.");
    std::fs::write(dir.join("README.txt"), readme).map_err(|e| format!("README.txt: {e}"))?;

    Ok(format!("export: {} ({w} x {h} px) in {}", written.join(", "), dir.display()))
}

/// Stage 4: the artist's PNGs that layers ask to trace, to pieces.
pub fn import(fig: &Figure) -> Result<String, String> {
    let dir = fig.trace_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut report = String::new();
    for layer in &fig.layers {
        let Kind::File { file, trace } = &layer.kind else {
            continue;
        };
        if *trace == Trace::None || !is_png(file) {
            let _ = writeln!(report, "import: {} from {file}, as it is", layer.name);
            continue;
        }
        let rgba = image::open(fig.root.join(file))
            .map_err(|e| format!("{file}: {e}"))?
            .to_rgba8();
        // Tracing works on an opaque picture: the transparent background becomes white, and
        // the white pieces are dropped again afterwards.
        let on_white = RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
            let p = rgba.get_pixel(x, y);
            let a = u32::from(p[3]);
            let channel = |c: u8| ((u32::from(c) * a + 255 * (255 - a)) / 255) as u8;
            image::Rgb([channel(p[0]), channel(p[1]), channel(p[2])])
        });
        // The artist's file is already at the canvas size: scale 1, the figure's settings.
        let mut doc = vectorize(&on_white, &fig.trace.params(1))?;
        let white: Vec<bool> = doc.palette.iter().map(|hex| is_near_white(hex)).collect();
        let before = doc.pieces.len();
        doc.pieces.retain(|piece| {
            let background = white.get(piece.color as usize).copied().unwrap_or(false);
            match trace {
                Trace::Outlines => piece.color == 0,
                Trace::Pieces => !background,
                Trace::None => true,
            }
        });
        for piece in &mut doc.pieces {
            piece.group = layer.name.clone();
        }
        doc.save(fig.artist_trace_path(file)).map_err(|e| format!("{file}: {e}"))?;
        let _ = writeln!(
            report,
            "import: {} from {file}, traced to {} pieces ({} dropped as background)",
            layer.name,
            doc.pieces.len(),
            before - doc.pieces.len()
        );
    }
    if report.is_empty() {
        report.push_str("import: no file layers yet; the artist's PNG or SVG files go next to source.png\n");
    }
    Ok(report.trim_end().to_owned())
}

/// Stages 1, 2 and 4 in a row.
pub fn all(fig: &Figure) -> Result<String, String> {
    Ok([trace(fig)?, export(fig)?, import(fig)?].join("\n"))
}

/// The source trace, or the advice to run the stage.
pub fn load_trace(fig: &Figure) -> Result<PieceDocument, String> {
    let path = fig.pieces_path();
    PieceDocument::load(&path).map_err(|e| format!("{}: {e}; run the trace stage first", path.display()))
}

fn load_rgb(path: &Path) -> Result<RgbImage, String> {
    Ok(image::open(path).map_err(|e| format!("{}: {e}", path.display()))?.to_rgb8())
}

fn save(img: &RgbaImage, path: &Path) -> Result<(), String> {
    img.save(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn is_near_white(hex: &str) -> bool {
    let channel = |i: usize| u8::from_str_radix(hex.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0);
    channel(1) >= 235 && channel(3) >= 235 && channel(5) >= 235
}

/// The path data of the colour pieces large enough to contour, in draw order.
pub fn contour_paths(doc: &PieceDocument, min_area: f32) -> Vec<String> {
    doc.pieces
        .iter()
        .filter(|p| p.group == PIECES && p.area >= min_area)
        .map(|p| p.d.clone())
        .collect()
}

/// The contour drawing as a standalone SVG at the canvas size: the borders of the colour
/// pieces as strokes, the outline pieces filled in the same colour.
pub fn contours_svg(doc: &PieceDocument, fig: &Figure) -> String {
    let color = fig.contour_color();
    let mut s = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><g fill="none" stroke="{color}" stroke-width="{}" stroke-linejoin="round">"#,
        fig.contour_width(),
        w = doc.width,
        h = doc.height
    );
    for d in contour_paths(doc, fig.contour_min_area()) {
        let _ = write!(s, r#"<path d="{d}"/>"#);
    }
    let _ = write!(s, r#"</g><g fill="{color}">"#);
    for piece in doc.pieces.iter().filter(|p| p.group == OUTLINES) {
        let _ = write!(s, r#"<path d="{}"/>"#, piece.d);
    }
    s.push_str("</g></svg>");
    s
}

/// One group of the trace as a standalone SVG at the canvas size.
fn group_svg(doc: &PieceDocument, group: &str) -> String {
    let mut s = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#,
        w = doc.width,
        h = doc.height
    );
    for piece in doc.pieces.iter().filter(|p| p.group == group) {
        let _ = write!(s, r#"<path fill="{}" d="{}"/>"#, doc.color_hex(piece), piece.d);
    }
    s.push_str("</svg>");
    s
}

/// An SVG drawn on a transparent canvas by the CPU renderer fframes uses.
fn render_pixmap(svg: &str, w: u32, h: u32, resources_dir: Option<&Path>) -> Result<svgr::tiny_skia::Pixmap, String> {
    let fontdb = usvgr::fontdb::Database::new();
    let options = usvgr::Options {
        resources_dir: resources_dir.map(Path::to_path_buf),
        ..Default::default()
    };
    let tree = usvgr::Tree::from_str(svg, &options, &fontdb).map_err(|e| format!("svg: {e}"))?;
    let mut pixmap = svgr::tiny_skia::Pixmap::new(w, h).ok_or("empty canvas")?;
    let ctx = svgr::Context::new_from_pixmap_unsafe(&pixmap);
    let mut cache = svgr::SvgrCache::new(0);
    let pool = svgr::PixmapPool::new();
    svgr::render(
        &tree,
        svgr::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
        &mut cache,
        &pool,
        &ctx,
    );
    Ok(pixmap)
}

/// An SVG rendered to straight (not premultiplied) RGBA pixels, for PNG files.
pub fn rasterize(svg: &str, w: u32, h: u32, resources_dir: Option<&Path>) -> Result<RgbaImage, String> {
    let pixmap = render_pixmap(svg, w, h, resources_dir)?;
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for p in pixmap.pixels() {
        let c = p.demultiply();
        rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    RgbaImage::from_raw(w, h, rgba).ok_or_else(|| "raster size".to_owned())
}

/// An SVG rendered to premultiplied RGBA pixels, the form `fframes::media::rendered_image`
/// takes for an image the video draws.
pub fn rasterize_premultiplied(svg: &str, w: u32, h: u32) -> Result<Vec<u8>, String> {
    Ok(render_pixmap(svg, w, h, None)?.data().to_vec())
}

/// `layer` over `base`, both straight alpha, at `opacity`.
fn blend(base: &mut RgbaImage, layer: &RgbaImage, opacity: f32) {
    for (dst, src) in base.pixels_mut().zip(layer.pixels()) {
        let a = f32::from(src[3]) / 255.0 * opacity;
        for i in 0..3 {
            dst[i] = (f32::from(src[i]) * a + f32::from(dst[i]) * (1.0 - a)).round() as u8;
        }
    }
}
