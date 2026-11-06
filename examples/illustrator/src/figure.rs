//! One figure is one folder: `figure.toml`, the source picture, the files the artist draws,
//! and the folders the stages write, `trace/` (ours, the vector pieces) and `export/` (ours,
//! the layers as PNGs for the artist). The folder is the exchange: the artist drops PNG or SVG
//! layers next to `source.png`, nothing else moves.
//!
//! ```toml
//! source = "source.png"
//! scale = 3                       # the canvas is the picture at 3x; the artist draws at that size
//!
//! [[layers]]                      # bottom to top
//! name = "photo"
//! kind = "source"                 # source | trace-pieces | trace-outlines | trace-lines | trace-contours | labels | file
//! opacity = 0.4
//!
//! [[layers]]
//! name = "lines"
//! kind = "file"
//! file = "lines.png"              # a PNG or SVG the artist put in the folder
//! trace = "outlines"              # none | outlines | pieces: a PNG traced to vector pieces at import
//! ```
//!
//! PNG and SVG files in the folder that no layer names are added on top, in name order, as
//! image layers, so the artist's files count without editing the manifest. A listed file that
//! is missing, a PNG that is not the canvas size, an unknown kind or trace, and a source that
//! does not open are problems; [`Figure::load`] lists all of them.

use std::path::{Path, PathBuf};

/// A manifest as written.
#[derive(serde::Deserialize)]
struct Manifest {
    source: String,
    #[serde(default = "default_scale")]
    scale: u32,
    #[serde(default)]
    trace: TraceSettings,
    #[serde(default)]
    layers: Vec<LayerEntry>,
    /// Add the files no layer names, on top in name order.
    #[serde(default = "default_true")]
    auto: bool,
}

/// The `[trace]` table: what the flatten and potrace steps do with the picture. Every value
/// is optional and overrides the `outlined` preset of fframes-vector at the figure's scale.
/// The ones that make lines finer: `dark` (a pixel darker than this fraction of full
/// brightness is line; lower leaves the soft halo of a line out) and `line_w` (half the
/// width in source pixels a dark structure may have and still be a line).
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize)]
pub struct TraceSettings {
    /// Width of the centreline strokes in canvas pixels (layer kind `trace-lines`), 2.5 by
    /// default, and their colour, charcoal by default.
    pub line_width: Option<f32>,
    pub line_color: Option<String>,
    /// Centrelines shorter than this many pixels are dropped (default 6).
    pub line_min: Option<usize>,
    /// The contour drawing (layer kind `trace-contours`): stroke width in canvas pixels (2.2
    /// by default), colour (charcoal), and the smallest piece in canvas pixels² whose border
    /// is drawn (400).
    pub contour_width: Option<f32>,
    pub contour_color: Option<String>,
    pub contour_min_area: Option<f32>,
    /// `"picture"`: every outline piece takes the colour of the picture under it (a navy
    /// banner stays navy, dark tissue dark grey) instead of the one outline colour, black.
    pub outline_color: Option<String>,
    pub dark: Option<f32>,
    pub line_w: Option<usize>,
    pub speck: Option<usize>,
    pub tol: Option<f32>,
    pub min_piece: Option<usize>,
    pub colors: Option<usize>,
    pub turd: Option<u32>,
    pub turd_outline: Option<u32>,
    pub alpha: Option<f32>,
    pub opt: Option<f32>,
}

impl TraceSettings {
    /// The preset at `scale` with these settings over it; `speck`, `min_piece`, `line_w` and
    /// the speckle sizes are given in source pixels and scaled like the preset does.
    pub fn params(&self, scale: u32) -> fframes_vector::VectorizeParams {
        let mut p = fframes_vector::VectorizeParams::outlined(scale);
        let s = scale.max(1) as usize;
        if let Some(dark) = self.dark {
            p.flatten.dark = dark;
        }
        if let Some(line_w) = self.line_w {
            p.flatten.line_w = line_w * s;
        }
        if let Some(speck) = self.speck {
            p.flatten.speck = speck * s * s;
        }
        if let Some(tol) = self.tol {
            p.flatten.tol = tol;
        }
        if let Some(min_piece) = self.min_piece {
            p.flatten.min_piece = min_piece * s * s;
        }
        if let Some(colors) = self.colors {
            p.colors = colors;
        }
        let s2 = scale.max(1) * scale.max(1);
        if let Some(turd) = self.turd {
            p.trace.turd = turd * s2;
        }
        if let Some(turd_outline) = self.turd_outline {
            p.trace.turd_outline = turd_outline * s2;
        }
        if let Some(alpha) = self.alpha {
            p.trace.alpha = alpha;
        }
        if let Some(opt) = self.opt {
            p.trace.opt = opt;
        }
        p
    }
}

fn default_scale() -> u32 {
    3
}

fn default_true() -> bool {
    true
}

#[derive(serde::Deserialize)]
struct LayerEntry {
    name: Option<String>,
    kind: String,
    file: Option<String>,
    trace: Option<String>,
    opacity: Option<f32>,
}

/// How a PNG the artist drew is brought into the vector world at import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trace {
    /// Kept as pixels.
    None,
    /// Traced; only the line colour is kept, for line art.
    Outlines,
    /// Traced; every colour but the transparent background is kept, for colour work.
    Pieces,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// The source picture.
    Source,
    /// The colour pieces of the trace.
    TracePieces,
    /// The outline pieces of the trace.
    TraceOutlines,
    /// The centrelines of the outlines as strokes (`lines.rs`).
    TraceLines,
    /// The borders of the colour pieces as strokes and the outline pieces filled: the
    /// contour drawing of the picture, a sketch for the artist to begin on.
    TraceContours,
    /// A PNG or SVG the artist drew, by file name.
    File { file: String, trace: Trace },
    /// The lettering of `labels.toml` set as text (`labels.rs`).
    Labels,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub name: String,
    pub kind: Kind,
    pub opacity: f32,
}

impl Layer {
    /// The file this layer is drawn from, when it is a file layer.
    pub fn file(&self) -> Option<&str> {
        match &self.kind {
            Kind::File { file, .. } => Some(file),
            _ => None,
        }
    }
}

/// A figure folder, checked.
#[derive(Debug, Clone)]
pub struct Figure {
    pub root: PathBuf,
    /// File name of the source picture in `root`.
    pub source: String,
    pub scale: u32,
    /// The `[trace]` table.
    pub trace: TraceSettings,
    /// The canvas: the source picture times `scale`, in pixels.
    pub width: u32,
    pub height: u32,
    /// Bottom to top.
    pub layers: Vec<Layer>,
}

impl Figure {
    /// Reads `root/figure.toml` and checks it against the folder; every problem is listed.
    pub fn load(root: impl AsRef<Path>) -> Result<Figure, Vec<String>> {
        let root = root.as_ref().to_path_buf();
        let manifest_path = root.join("figure.toml");
        let text = std::fs::read_to_string(&manifest_path)
            .map_err(|e| vec![format!("{}: {e}", manifest_path.display())])?;
        let manifest: Manifest =
            toml::from_str(&text).map_err(|e| vec![format!("figure.toml: {}", e.message())])?;
        let mut problems = Vec::new();

        if manifest.scale == 0 {
            problems.push("scale must be at least 1".to_owned());
        }
        let (source_w, source_h) = match image::image_dimensions(root.join(&manifest.source)) {
            Ok(size) => size,
            Err(e) => {
                problems.push(format!("source {}: {e}", manifest.source));
                (0, 0)
            }
        };
        let (width, height) = (source_w * manifest.scale, source_h * manifest.scale);

        let mut layers = Vec::with_capacity(manifest.layers.len());
        for (index, entry) in manifest.layers.iter().enumerate() {
            let kind = match entry.kind.as_str() {
                "source" => Kind::Source,
                "trace-pieces" => Kind::TracePieces,
                "trace-outlines" => Kind::TraceOutlines,
                "trace-lines" => Kind::TraceLines,
                "trace-contours" => Kind::TraceContours,
                "labels" => Kind::Labels,
                "file" => {
                    let Some(file) = entry.file.clone() else {
                        problems.push(format!("layer {}: kind file needs a file", index + 1));
                        continue;
                    };
                    let trace = match entry.trace.as_deref() {
                        None | Some("none") => Trace::None,
                        Some("outlines") => Trace::Outlines,
                        Some("pieces") => Trace::Pieces,
                        Some(other) => {
                            problems.push(format!(
                                "layer {file}: trace \"{other}\" is not none, outlines or pieces"
                            ));
                            continue;
                        }
                    };
                    Kind::File { file, trace }
                }
                other => {
                    problems.push(format!(
                        "layer {}: kind \"{other}\" is not source, trace-pieces, trace-outlines, trace-lines, trace-contours, labels or file",
                        index + 1
                    ));
                    continue;
                }
            };
            let name = entry
                .name
                .clone()
                .or_else(|| entry.file.as_deref().map(stem))
                .unwrap_or_else(|| entry.kind.clone());
            layers.push(Layer { name, kind, opacity: entry.opacity.unwrap_or(1.0) });
        }

        if manifest.auto {
            let mut extra: Vec<String> = std::fs::read_dir(&root)
                .map_err(|e| vec![format!("{}: {e}", root.display())])?
                .filter_map(Result::ok)
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| {
                    let lower = name.to_ascii_lowercase();
                    (lower.ends_with(".png") || lower.ends_with(".svg"))
                        && *name != manifest.source
                        && !layers.iter().any(|layer| layer.file() == Some(name.as_str()))
                })
                .collect();
            extra.sort();
            for file in extra {
                layers.push(Layer {
                    name: stem(&file),
                    kind: Kind::File { file, trace: Trace::None },
                    opacity: 1.0,
                });
            }
        }

        for layer in &layers {
            let Kind::File { file, trace } = &layer.kind else {
                continue;
            };
            let path = root.join(file);
            if !path.is_file() {
                problems.push(format!("layer {}: {file} is not in the folder", layer.name));
                continue;
            }
            if is_png(file) {
                match image::image_dimensions(&path) {
                    Ok((w, h)) if (w, h) != (width, height) && width > 0 => problems.push(format!(
                        "layer {}: {file} is {w} x {h} px, the canvas is {width} x {height}",
                        layer.name
                    )),
                    Ok(_) => {}
                    Err(e) => problems.push(format!("layer {}: {file}: {e}", layer.name)),
                }
            } else if *trace != Trace::None {
                problems.push(format!(
                    "layer {}: {file} is an SVG, it is already vector; trace applies to PNGs",
                    layer.name
                ));
            }
        }

        if problems.is_empty() {
            Ok(Figure {
                root,
                source: manifest.source,
                scale: manifest.scale,
                trace: manifest.trace,
                width,
                height,
                layers,
            })
        } else {
            Err(problems)
        }
    }

    pub fn trace_dir(&self) -> PathBuf {
        self.root.join("trace")
    }

    pub fn export_dir(&self) -> PathBuf {
        self.root.join("export")
    }

    /// The trace of the source picture.
    pub fn pieces_path(&self) -> PathBuf {
        self.trace_dir().join("pieces.json")
    }

    /// The trace of an artist's PNG, written at import.
    pub fn artist_trace_path(&self, file: &str) -> PathBuf {
        self.trace_dir().join(format!("{}.json", stem(file)))
    }

    /// The lettering of the source picture (`labels.rs`), the author's file.
    pub fn labels_path(&self) -> PathBuf {
        self.root.join("labels.toml")
    }

    /// The source picture with the lettering erased, what the trace stage traces.
    pub fn clean_path(&self) -> PathBuf {
        self.trace_dir().join("clean.png")
    }

    /// The layers that need the labels.
    pub fn uses_labels(&self) -> bool {
        self.layers.iter().any(|layer| layer.kind == Kind::Labels)
    }

    /// The centrelines of the source trace.
    pub fn lines_path(&self) -> PathBuf {
        self.trace_dir().join("lines.json")
    }

    /// Stroke width of the centrelines in canvas pixels.
    pub fn line_width(&self) -> f32 {
        self.trace.line_width.unwrap_or(2.5)
    }

    pub fn line_color(&self) -> &str {
        self.trace.line_color.as_deref().unwrap_or("#2C3539")
    }

    /// Stroke width of the contour drawing in canvas pixels.
    pub fn contour_width(&self) -> f32 {
        self.trace.contour_width.unwrap_or(2.2)
    }

    pub fn contour_color(&self) -> &str {
        self.trace.contour_color.as_deref().unwrap_or("#2C3539")
    }

    /// The smallest piece whose border the contour drawing shows, canvas pixels².
    pub fn contour_min_area(&self) -> f32 {
        self.trace.contour_min_area.unwrap_or(400.0)
    }

    /// The layers that need the source trace.
    pub fn uses_trace(&self) -> bool {
        self.layers.iter().any(|layer| {
            matches!(layer.kind, Kind::TracePieces | Kind::TraceOutlines | Kind::TraceContours)
        })
    }

    /// The layers that need the contour drawing.
    pub fn uses_contours(&self) -> bool {
        self.layers.iter().any(|layer| matches!(layer.kind, Kind::TraceContours))
    }

    /// The layers that need the centrelines.
    pub fn uses_lines(&self) -> bool {
        self.layers.iter().any(|layer| matches!(layer.kind, Kind::TraceLines))
    }
}

pub fn is_png(file: &str) -> bool {
    file.to_ascii_lowercase().ends_with(".png")
}

pub fn is_svg(file: &str) -> bool {
    file.to_ascii_lowercase().ends_with(".svg")
}

/// `lines.png` -> `lines`.
pub fn stem(file: &str) -> String {
    Path::new(file)
        .file_stem()
        .map_or_else(|| file.to_owned(), |s| s.to_string_lossy().into_owned())
}

/// The problems, one per line.
pub fn problems_text(problems: &[String]) -> String {
    use std::fmt::Write;
    problems.iter().fold(String::new(), |mut text, p| {
        let _ = writeln!(text, "- {p}");
        text
    })
}
