//! A figure as layers that go back and forth between fframes and an illustrator. One folder
//! per figure (`figure.rs`); the stages trace, export and import (`stages.rs`) can run one
//! after another or all at once; the video here renders the stack, one `<g>` per layer,
//! bottom to top, the figure fitted into a 1920 x 1080 frame on paper.
//!
//! From the repository root:
//!
//! ```sh
//! cargo run --release -p illustrator -- figures/dix_hallpike all        # trace, export, import
//! cargo run --release -p illustrator -- figures/dix_hallpike trace      # one stage
//! cargo run --release -p illustrator -- figures/dix_hallpike frame 0    # the stack as a PNG
//! cargo run --release -p illustrator -- figures/dix_hallpike render     # or any fframes command
//! ```

pub mod figure;
pub mod labels;
pub mod lines;
pub mod stages;

use std::collections::HashMap;

use fframes::vector::{PieceDocument, PieceTree};
use fframes::{AudioMap, Color, Duration, FFramesContext, FontQuery, Frame, Svgr, Video};

use figure::{Figure, Kind, Trace, is_svg};
use labels::Labels;

pub const WIDTH: usize = 1920;
pub const HEIGHT: usize = 1080;
/// Warm cream, the paper of the Thieme style.
pub const PAPER: &str = "#FAF8F4";

/// The figure with everything the render needs loaded once: the trace trees and the artist's
/// SVG texts. PNG layers come from the media folder, which is the figure folder.
pub struct FigureVideo {
    pub figure: Figure,
    trace: Option<PieceTree>,
    /// By file name: the pieces of an artist's traced PNG.
    traced: HashMap<String, PieceTree>,
    /// By file name: an artist's SVG drawn at the canvas size, premultiplied RGBA.
    svgs: HashMap<String, Vec<u8>>,
    /// The centrelines of the source trace, when a layer shows them.
    lines: Vec<lines::Line>,
    /// Path data of the colour pieces large enough to contour, when a layer shows them.
    contours: Vec<String>,
    /// The lettering set as text, when a layer shows it.
    labels: Option<Labels>,
}

impl std::fmt::Debug for FigureVideo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FigureVideo")
    }
}

impl FigureVideo {
    /// Loads the figure and its traces; every problem is listed.
    pub fn open(figure: Figure) -> Result<Self, Vec<String>> {
        let mut problems = Vec::new();
        let mut contours = Vec::new();
        let trace = if figure.uses_trace() {
            match stages::load_trace(&figure) {
                Ok(doc) => {
                    if figure.uses_contours() {
                        contours = stages::contour_paths(&doc, figure.contour_min_area());
                    }
                    Some(PieceTree::new(&doc, "source"))
                }
                Err(e) => {
                    problems.push(e);
                    None
                }
            }
        } else {
            None
        };
        let lines = if figure.uses_lines() {
            match stages::load_lines(&figure) {
                Ok(lines) => lines,
                Err(e) => {
                    problems.push(e);
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };
        let labels = if figure.uses_labels() {
            match Labels::load(&figure.labels_path()) {
                Ok(labels) => Some(labels),
                Err(e) => {
                    problems.push(format!("{e}; run the ocr stage first"));
                    None
                }
            }
        } else {
            None
        };
        let mut traced = HashMap::new();
        let mut svgs = HashMap::new();
        for layer in &figure.layers {
            let Kind::File { file, trace } = &layer.kind else {
                continue;
            };
            if is_svg(file) {
                // The renderer draws images, not foreign SVG trees: the artist's SVG is drawn
                // once at the canvas size and used as an image, like a PNG of theirs.
                let drawn = std::fs::read_to_string(figure.root.join(file))
                    .map_err(|e| format!("{file}: {e}"))
                    .and_then(|text| stages::rasterize_premultiplied(&text, figure.width, figure.height));
                match drawn {
                    Ok(rgba) => {
                        svgs.insert(file.clone(), rgba);
                    }
                    Err(e) => problems.push(format!("{file}: {e}")),
                }
            } else if *trace != Trace::None {
                let path = figure.artist_trace_path(file);
                match PieceDocument::load(&path) {
                    Ok(doc) => {
                        traced.insert(file.clone(), PieceTree::new(&doc, file));
                    }
                    Err(e) => problems.push(format!(
                        "{}: {e}; run the import stage first",
                        path.display()
                    )),
                }
            }
        }
        if problems.is_empty() {
            Ok(FigureVideo { figure, trace, traced, svgs, lines, contours, labels })
        } else {
            Err(problems)
        }
    }

    /// The artist's SVG layers drawn at the canvas size, by file name, for the media.
    pub fn svg_images(&self) -> impl Iterator<Item = (&String, &Vec<u8>)> {
        self.svgs.iter()
    }

    /// The labels as text on the canvas. A label keeps the size its lettering had in the
    /// picture and shrinks to its box when the font sets it wider.
    fn labels<'a>(&'a self, frame: &mut Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let Some(labels) = &self.labels else {
            return Svgr::empty();
        };
        let scale = self.figure.scale as f32;
        let texts: Vec<Svgr<'a>> = labels
            .labels
            .iter()
            .filter(|label| label.show && !label.text.trim().is_empty())
            .map(|label| {
                let weight: u16 = if label.bold { 700 } else { 500 };
                let mut size = label.size as f32 * scale;
                let query = FontQuery {
                    family: labels.font.as_str(),
                    size: size.round().max(1.0) as usize,
                    weight,
                    ..Default::default()
                };
                if let Some(width) = frame.text_width(ctx, query, label.text.as_str()) {
                    let room = label.w as f32 * scale;
                    // Only a label far wider than its place shrinks; small differences stay,
                    // so the sizes remain even.
                    if width as f32 > 1.2 * room && width > 0 {
                        size *= room / width as f32;
                    }
                }
                fframes::svgr!(
                    <text x={label.x as f32 * scale} y={label.baseline as f32 * scale} font-family={labels.font.as_str()} font-size={size} font-weight={weight} fill={label.color.as_str()}>{label.text.as_str()}</text>
                )
            })
            .collect();
        fframes::svgr!(<g>{texts}</g>)
    }

    fn layer<'a>(&'a self, layer: &'a figure::Layer, frame: &mut Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let (w, h) = (self.figure.width, self.figure.height);
        let body: Svgr<'a> = match &layer.kind {
            Kind::Labels => self.labels(frame, ctx),
            Kind::Source => match ctx.get_image(&self.figure.source) {
                Some(image) => fframes::svgr!(<image href={image.href()} width={w} height={h} />),
                None => Svgr::empty(),
            },
            Kind::TracePieces => self.trace.as_ref().map_or_else(Svgr::empty, |t| Svgr::from(t.group(stages::PIECES))),
            Kind::TraceOutlines => self.trace.as_ref().map_or_else(Svgr::empty, |t| Svgr::from(t.group(stages::OUTLINES))),
            Kind::TraceLines => {
                let paths: Vec<Svgr<'a>> = self
                    .lines
                    .iter()
                    .map(|line| fframes::svgr!(<path d={lines::path_d(line)} />))
                    .collect();
                fframes::svgr!(
                    <g fill="none" stroke={self.figure.line_color()} stroke-width={self.figure.line_width()} stroke-linecap="round" stroke-linejoin="round">
                        {paths}
                    </g>
                )
            }
            Kind::TraceContours => {
                let borders: Vec<Svgr<'a>> = self
                    .contours
                    .iter()
                    .map(|d| fframes::svgr!(<path d={d.as_str()} />))
                    .collect();
                let outlines = self.trace.as_ref().map_or_else(Svgr::empty, |t| Svgr::from(t.group(stages::OUTLINES)));
                fframes::svgr!(
                    <g>
                        <g fill="none" stroke={self.figure.contour_color()} stroke-width={self.figure.contour_width()} stroke-linejoin="round">
                            {borders}
                        </g>
                        {outlines}
                    </g>
                )
            }
            Kind::File { file, .. } => {
                if let Some(tree) = self.traced.get(file) {
                    Svgr::from(tree.all())
                } else {
                    match ctx.get_image(file) {
                        Some(image) => fframes::svgr!(<image href={image.href()} width={w} height={h} />),
                        None => Svgr::empty(),
                    }
                }
            }
        };
        fframes::svgr!(<g id={layer.name.clone()} opacity={layer.opacity}>{body}</g>)
    }
}

impl Video for FigureVideo {
    const FPS: usize = 30;
    const WIDTH: usize = WIDTH;
    const HEIGHT: usize = HEIGHT;
    const BACKGROUND_COLOR: Color = Color::WHITE;

    fn duration(&self) -> Duration<'_> {
        Duration::Frames(1)
    }

    fn audio(&self) -> AudioMap<'_> {
        AudioMap::none()
    }

    fn render_frame<'a>(&'a self, mut frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let (w, h) = (self.figure.width as f32, self.figure.height as f32);
        let s = (WIDTH as f32 / w).min(HEIGHT as f32 / h);
        let (x, y) = ((WIDTH as f32 - w * s) / 2.0, (HEIGHT as f32 - h * s) / 2.0);
        let layers: Vec<Svgr<'a>> =
            self.figure.layers.iter().map(|layer| self.layer(layer, &mut frame, ctx)).collect();
        fframes::svgr!(
            <svg xmlns="http://www.w3.org/2000/svg" width={WIDTH} height={HEIGHT} viewBox="0 0 1920 1080">
                <rect width="1920" height="1080" fill={PAPER} />
                <g transform={format!("translate({x:.2} {y:.2}) scale({s:.5})")}>
                    {layers}
                </g>
            </svg>
        )
    }
}
