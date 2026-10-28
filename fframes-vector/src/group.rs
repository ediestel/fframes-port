//! The group map: pieces assigned to named objects (`french_banner`, `horse`, `spurs`).
//!
//! Zones are drawn by hand as an SVG of `<rect>`, `<circle>` and `<polygon>` elements, each
//! with `id="name"` (several shapes may share a name), in the coordinates of the source
//! picture. A piece joins the first zone in the file that contains its centre and most of
//! its bounding box, so zones are listed specific first (spurs before the meadow they lie
//! in) and backgrounds last. Pieces in no zone keep an empty group.

use crate::document::PieceDocument;
use std::collections::HashMap;
use usvgr::svgtree::svgrtypes::{PathParser, PathSegment};

#[derive(Clone, Debug)]
pub enum Shape {
    Rect { x: f32, y: f32, w: f32, h: f32 },
    Circle { cx: f32, cy: f32, r: f32 },
    Polygon(Vec<(f32, f32)>),
}

impl Shape {
    fn contains(&self, px: f32, py: f32) -> bool {
        match self {
            Shape::Rect { x, y, w, h } => px >= *x && px <= x + w && py >= *y && py <= y + h,
            Shape::Circle { cx, cy, r } => (px - cx).powi(2) + (py - cy).powi(2) <= r * r,
            Shape::Polygon(pts) => {
                // even-odd ray cast
                let mut inside = false;
                let n = pts.len();
                let mut j = n.wrapping_sub(1);
                for i in 0..n {
                    let (xi, yi) = pts[i];
                    let (xj, yj) = pts[j];
                    if (yi > py) != (yj > py) && px < (xj - xi) * (py - yi) / (yj - yi) + xi {
                        inside = !inside;
                    }
                    j = i;
                }
                inside
            }
        }
    }

    fn scaled(&self, k: f32) -> Shape {
        match self {
            Shape::Rect { x, y, w, h } => Shape::Rect {
                x: x * k,
                y: y * k,
                w: w * k,
                h: h * k,
            },
            Shape::Circle { cx, cy, r } => Shape::Circle {
                cx: cx * k,
                cy: cy * k,
                r: r * k,
            },
            Shape::Polygon(pts) => {
                Shape::Polygon(pts.iter().map(|(x, y)| (x * k, y * k)).collect())
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct GroupMap {
    pub zones: Vec<(String, Shape)>,
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let i = tag.find(&key)? + key.len();
    let j = tag[i..].find('"')? + i;
    Some(&tag[i..j])
}

fn num(tag: &str, name: &str) -> Option<f32> {
    attr(tag, name)?.trim().parse().ok()
}

impl GroupMap {
    /// Reads the zone shapes out of an SVG text. Only `rect`, `circle` and `polygon`
    /// elements with an `id` count; everything else is ignored, so the file may also hold
    /// the picture as a reference image.
    #[must_use]
    pub fn from_svg(svg: &str) -> Self {
        let mut zones = Vec::new();
        let mut rest = svg;
        while let Some(i) = rest.find('<') {
            let s = &rest[i..];
            let Some(j) = s.find('>') else { break };
            let tag = &s[..=j];
            rest = &s[j + 1..];
            let Some(id) = attr(tag, "id") else { continue };
            let shape = if tag.starts_with("<rect") {
                match (
                    num(tag, "x"),
                    num(tag, "y"),
                    num(tag, "width"),
                    num(tag, "height"),
                ) {
                    (Some(x), Some(y), Some(w), Some(h)) => Shape::Rect { x, y, w, h },
                    _ => continue,
                }
            } else if tag.starts_with("<circle") {
                match (num(tag, "cx"), num(tag, "cy"), num(tag, "r")) {
                    (Some(cx), Some(cy), Some(r)) => Shape::Circle { cx, cy, r },
                    _ => continue,
                }
            } else if tag.starts_with("<polygon") {
                let Some(points) = attr(tag, "points") else {
                    continue;
                };
                let nums: Vec<f32> = points
                    .split(|c: char| c == ',' || c.is_whitespace())
                    .filter(|t| !t.is_empty())
                    .filter_map(|t| t.parse().ok())
                    .collect();
                if nums.len() < 6 {
                    continue;
                }
                Shape::Polygon(
                    nums.as_chunks::<2>()
                        .0
                        .iter()
                        .map(|&[x, y]| (x, y))
                        .collect(),
                )
            } else {
                continue;
            };
            zones.push((id.to_string(), shape));
        }
        GroupMap { zones }
    }

    /// Assigns every piece the first zone (in file order) that contains its centre and most
    /// of its bounding box (at least 5 of a 3 x 3 grid of points over the box), so a piece
    /// spanning the whole picture joins nothing and a spur drawn inside the meadow joins
    /// `spurs` when `spurs` comes first in the file. `zone_scale` maps the zone coordinates
    /// to the document's (the document's `scale` when zones are drawn on the source
    /// picture). Returns the number of pieces per group.
    pub fn assign(&self, doc: &mut PieceDocument, zone_scale: f32) -> HashMap<String, usize> {
        let zones: Vec<(&str, Shape)> = self
            .zones
            .iter()
            .map(|(name, s)| (name.as_str(), s.scaled(zone_scale)))
            .collect();
        let mut counts: HashMap<String, usize> = HashMap::new();
        for piece in &mut doc.pieces {
            let (cx, cy) = centre(&piece.d).unwrap_or((
                piece.bbox[0] + piece.bbox[2] / 2.0,
                piece.bbox[1] + piece.bbox[3] / 2.0,
            ));
            let [bx, by, bw, bh] = piece.bbox;
            let mostly_inside = |s: &Shape| {
                let mut n = 0;
                for i in 0..3 {
                    for j in 0..3 {
                        if s.contains(
                            bx + bw * (0.1 + 0.4 * i as f32),
                            by + bh * (0.1 + 0.4 * j as f32),
                        ) {
                            n += 1;
                        }
                    }
                }
                n >= 5
            };
            let best = zones
                .iter()
                .find(|(_, s)| s.contains(cx, cy) && mostly_inside(s))
                .map(|(name, _)| *name);
            piece.group = best.unwrap_or("").to_string();
            if let Some(name) = best {
                *counts.entry(name.to_string()).or_insert(0) += 1;
            }
        }
        counts
    }
}

/// The mean of the outer contour's vertices: inside the piece for all but the most concave
/// shapes, and cheap.
fn centre(d: &str) -> Option<(f32, f32)> {
    let (mut sx, mut sy, mut n) = (0.0f64, 0.0f64, 0usize);
    for seg in PathParser::from(d) {
        match seg.ok()? {
            PathSegment::MoveTo { x, y, .. } if n > 0 => {
                // the first subpath is the outer contour; stop at the first hole
                let _ = (x, y);
                break;
            }
            PathSegment::MoveTo { x, y, .. }
            | PathSegment::LineTo { x, y, .. }
            | PathSegment::CurveTo { x, y, .. } => {
                sx += x;
                sy += y;
                n += 1;
            }
            _ => {}
        }
    }
    (n > 0).then(|| ((sx / n as f64) as f32, (sy / n as f64) as f32))
}

/// Distinct colours for a check picture, one per group name, deterministic.
#[must_use]
pub fn group_color(name: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut h);
    let v = h.finish();
    // golden-ratio hue spacing from the hash, full saturation, light enough to read
    let hue = (v % 360) as f32;
    let (r, g, b) = hsl(hue, 0.85, 0.55);
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn hsl(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match h as u32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

impl PieceDocument {
    /// An SVG where every piece is filled with its group's colour (grey when ungrouped) and
    /// the outline pieces stay black: the picture to look at after assigning groups.
    #[must_use]
    pub fn to_group_check_svg(&self) -> String {
        use std::fmt::Write;
        let mut s = String::with_capacity(self.pieces.len() * 160);
        let _ = write!(
            s,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><rect width="{w}" height="{h}" fill="#222"/>"##,
            w = self.width,
            h = self.height
        );
        for p in &self.pieces {
            let fill = if p.color == 0 {
                "#000".to_string()
            } else if p.group.is_empty() {
                "#777".to_string()
            } else {
                group_color(&p.group)
            };
            let _ = write!(s, r#"<path fill="{fill}" d="{}"/>"#, p.d);
        }
        s.push_str("</svg>");
        s
    }
}
