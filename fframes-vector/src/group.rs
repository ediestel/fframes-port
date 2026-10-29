//! Grouping: pieces assigned to named objects (`horse`, `rider/spurs`).
//!
//! The process is the same for every picture; only the hints are picture-specific.
//!
//! 1. **Hints** become a [`LabelLayer`], a name per pixel:
//!    - a zone SVG ([`GroupMap::from_svg`]): any `rect`, `circle`, `ellipse`, `polygon`,
//!      `polyline` or `path`, drawn by hand in any editor over the source picture. The name
//!      is `data-group`, else `inkscape:label`, else `id`; shapes inside a named `<g>` take
//!      the group's name as a prefix (`<g id="rider"><path id="spurs"/></g>` = `rider/spurs`),
//!      unnamed shapes inside a named `<g>` take its name. `transform` attributes count.
//!      Earlier shapes win where shapes overlap, so specific objects come first.
//!    - a colour mask ([`LabelLayer::from_mask`]): a PNG of any size where each colour is
//!      one object, from any segmentation tool or painted by hand; names come from a legend
//!      (`{"horse": "#ff0000"}`) or are the colours themselves. Transparent = no object.
//!
//!    Layers stack ([`LabelLayer::under`]): a hand-drawn correction over a machine mask.
//! 2. **Units** are what gets a name: every piece on its own, or every automatic cluster
//!    ([`crate::cluster`]) as a whole, so a coarse hint snaps to the outlines of the glass.
//! 3. **Vote**: a unit takes the name covering the largest share of its pixels in the
//!    [`PieceRaster`] when that share is at least `share`; otherwise it stays unnamed.
//!
//! Names are paths: `tree.group("rider")` includes `rider/spurs`.

use crate::cluster::{self, ClusterParams};
use crate::document::PieceDocument;
use crate::raster::{PieceRaster, covered_pixels, skia_path};
use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;
use svgr::tiny_skia::{Path, PathBuilder, Rect, Transform};
use usvgr::svgtree::svgrtypes;

/// Whether a piece group `group` belongs to the object `name` (equal, or below it).
#[must_use]
pub fn in_group(group: &str, name: &str) -> bool {
    group == name
        || (group.len() > name.len()
            && group.starts_with(name)
            && group.as_bytes()[name.len()] == b'/')
}

/// One named hint shape, in source-picture coordinates.
#[derive(Clone, Debug)]
pub struct Zone {
    pub name: String,
    pub path: Path,
}

impl Zone {
    /// A closed polygon through `points` (source-picture coordinates); `None` for fewer
    /// than three points.
    #[must_use]
    pub fn polygon(name: &str, points: &[(f32, f32)]) -> Option<Zone> {
        if points.len() < 3 {
            return None;
        }
        let mut pb = PathBuilder::new();
        pb.move_to(points[0].0, points[0].1);
        for &(x, y) in &points[1..] {
            pb.line_to(x, y);
        }
        pb.close();
        Some(Zone {
            name: name.to_string(),
            path: pb.finish()?,
        })
    }

    /// A zone with exactly the outlines of the pieces at `indices` in `doc` (holes stay
    /// holes), in source-picture coordinates: `zone_scale` document pixels per unit, as in
    /// [`GroupMap::layer`]. `None` when no outline parses.
    #[must_use]
    pub fn from_pieces(
        name: &str,
        doc: &PieceDocument,
        indices: &[usize],
        zone_scale: f32,
    ) -> Option<Zone> {
        let d: Vec<&str> = indices
            .iter()
            .filter_map(|&i| doc.pieces.get(i))
            .map(|p| p.d.as_str())
            .collect();
        let k = 1.0 / zone_scale;
        let path = skia_path(&d.join(" "))?.transform(Transform::from_scale(k, k))?;
        Some(Zone {
            name: name.to_string(),
            path,
        })
    }

    /// The outline as absolute SVG path data, two decimals.
    #[must_use]
    pub fn path_data(&self) -> String {
        use svgr::tiny_skia::PathSegment;
        let mut d = String::new();
        let points = |d: &mut String, command: char, points: &[svgr::tiny_skia::Point]| {
            d.push(command);
            for (i, p) in points.iter().enumerate() {
                if i > 0 {
                    d.push(' ');
                }
                push_num(d, p.x);
                d.push(' ');
                push_num(d, p.y);
            }
        };
        for seg in self.path.segments() {
            match seg {
                PathSegment::MoveTo(p) => points(&mut d, 'M', &[p]),
                PathSegment::LineTo(p) => points(&mut d, 'L', &[p]),
                PathSegment::QuadTo(c, p) => points(&mut d, 'Q', &[c, p]),
                PathSegment::CubicTo(a, b, p) => points(&mut d, 'C', &[a, b, p]),
                PathSegment::Close => d.push('Z'),
            }
        }
        d
    }
}

/// Hint shapes read from an SVG, in file order (earlier wins).
#[derive(Clone, Debug, Default)]
pub struct GroupMap {
    pub zones: Vec<Zone>,
}

/// The value of attribute `name` in a start tag (either quote style), if present.
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let bytes = tag.as_bytes();
    let mut from = 0;
    while let Some(k) = tag[from..].find(name) {
        let at = from + k;
        from = at + name.len();
        let before_ok = at > 0 && bytes[at - 1].is_ascii_whitespace();
        let rest = tag[from..].trim_start();
        if !before_ok || !rest.starts_with('=') {
            continue;
        }
        let rest = rest[1..].trim_start();
        let quote = rest.chars().next()?;
        if quote != '"' && quote != '\'' {
            continue;
        }
        let end = rest[1..].find(quote)?;
        return Some(&rest[1..=end]);
    }
    None
}

fn num(tag: &str, name: &str) -> Option<f32> {
    let v = attr(tag, name)?.trim();
    v.trim_end_matches("px").parse().ok()
}

fn points(tag: &str) -> Vec<(f32, f32)> {
    let nums: Vec<f32> = attr(tag, "points")
        .unwrap_or("")
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .filter_map(|t| t.parse().ok())
        .collect();
    nums.as_chunks::<2>()
        .0
        .iter()
        .map(|&[x, y]| (x, y))
        .collect()
}

fn transform_of(tag: &str) -> Transform {
    attr(tag, "transform")
        .and_then(|t| svgrtypes::Transform::from_str(t).ok())
        .map_or(Transform::identity(), |t| {
            Transform::from_row(
                t.a as f32, t.b as f32, t.c as f32, t.d as f32, t.e as f32, t.f as f32,
            )
        })
}

/// `v` with two decimals and no trailing zeros (`12.5`, `3`, `-0.25`).
fn push_num(d: &mut String, v: f32) {
    use std::fmt::Write;
    let start = d.len();
    let _ = write!(d, "{v:.2}");
    let kept = d[start..].trim_end_matches('0').trim_end_matches('.').len();
    d.truncate(start + kept);
    if &d[start..] == "-0" {
        d.replace_range(start.., "0");
    }
}

/// The end of the start tag at the beginning of `s`: the first `>` outside quotes.
fn tag_end(s: &str) -> Option<usize> {
    let mut quote: Option<u8> = None;
    for (i, &b) in s.as_bytes().iter().enumerate() {
        match (quote, b) {
            (None, b'"' | b'\'') => quote = Some(b),
            (Some(q), _) if b == q => quote = None,
            (None, b'>') => return Some(i),
            _ => {}
        }
    }
    None
}

/// XML character references in an attribute value decoded (`&amp;` to `&`, `&#65;` to `A`).
fn unescape(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut rest = v;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let decoded = tail.find(';').and_then(|e| {
            let c = match &tail[1..e] {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                n => n
                    .strip_prefix("#x")
                    .map(|h| u32::from_str_radix(h, 16))
                    .or_else(|| n.strip_prefix('#').map(str::parse))
                    .and_then(Result::ok)
                    .and_then(char::from_u32),
            }?;
            Some((c, e + 1))
        });
        if let Some((c, len)) = decoded {
            out.push(c);
            rest = &tail[len..];
        } else {
            out.push('&');
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

fn name_of(tag: &str) -> Option<&str> {
    attr(tag, "data-group")
        .or_else(|| attr(tag, "inkscape:label"))
        .or_else(|| attr(tag, "id"))
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn shape_path(tag: &str, element: &str) -> Option<Path> {
    let at = |a: &str| num(tag, a).unwrap_or(0.0);
    match element {
        "rect" => Some(PathBuilder::from_rect(Rect::from_xywh(
            at("x"),
            at("y"),
            num(tag, "width")?,
            num(tag, "height")?,
        )?)),
        "circle" => PathBuilder::from_circle(at("cx"), at("cy"), num(tag, "r")?),
        "ellipse" => {
            let (rx, ry) = (num(tag, "rx")?, num(tag, "ry")?);
            PathBuilder::from_oval(Rect::from_xywh(
                at("cx") - rx,
                at("cy") - ry,
                2.0 * rx,
                2.0 * ry,
            )?)
        }
        "polygon" | "polyline" => {
            let pts = points(tag);
            if pts.len() < 3 {
                return None;
            }
            let mut pb = PathBuilder::new();
            pb.move_to(pts[0].0, pts[0].1);
            for &(x, y) in &pts[1..] {
                pb.line_to(x, y);
            }
            pb.close();
            pb.finish()
        }
        "path" => skia_path(attr(tag, "d")?),
        _ => None,
    }
}

impl GroupMap {
    /// Reads the hint shapes out of an SVG text. Elements without a name (own or from a
    /// surrounding `<g>`) are ignored, so the file may also hold the picture as a
    /// reference image and unnamed drawing aids.
    #[must_use]
    pub fn from_svg(svg: &str) -> Self {
        let mut zones = Vec::new();
        // open <g> elements: (name, transform up to and including this group)
        let mut stack: Vec<(Option<String>, Transform)> = Vec::new();
        let mut rest = svg;
        while let Some(i) = rest.find('<') {
            let s = &rest[i..];
            if let Some(body) = s.strip_prefix("<!--") {
                rest = body.find("-->").map_or("", |e| &body[e + 3..]);
                continue;
            }
            let Some(j) = tag_end(s) else { break };
            let tag = &s[..=j];
            rest = &s[j + 1..];
            if tag.starts_with("</g") {
                stack.pop();
                continue;
            }
            let parent_ts = stack.last().map_or(Transform::identity(), |g| g.1);
            let parent_name = stack.last().and_then(|g| g.0.clone());
            let element: String = tag[1..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == ':' || *c == '-')
                .collect();
            if element == "g" {
                if !tag.ends_with("/>") {
                    let name = match (&parent_name, name_of(tag)) {
                        (Some(p), Some(o)) => Some(format!("{p}/{o}")),
                        (p, o) => o.map(str::to_string).or_else(|| p.clone()),
                    };
                    stack.push((name, parent_ts.pre_concat(transform_of(tag))));
                }
                continue;
            }
            let name = match (&parent_name, name_of(tag)) {
                (Some(p), Some(o)) => format!("{p}/{o}"),
                (Some(p), None) => p.clone(),
                (None, Some(o)) => o.to_string(),
                (None, None) => continue,
            };
            let Some(path) = shape_path(tag, &element) else {
                continue;
            };
            let Some(path) = path.transform(parent_ts.pre_concat(transform_of(tag))) else {
                continue;
            };
            zones.push(Zone {
                name: unescape(&name),
                path,
            });
        }
        GroupMap { zones }
    }

    /// The zones as a zone SVG of `width x height` (the source picture), one `path` per
    /// zone in order, named with `data-group`; [`GroupMap::from_svg`] reads it back.
    #[must_use]
    pub fn to_svg(&self, width: u32, height: u32) -> String {
        use std::fmt::Write;
        let mut s = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">"#
        );
        s.push_str("\n<!-- Zones: earlier shapes win where they overlap. -->\n");
        for z in &self.zones {
            let name = z
                .name
                .replace('&', "&amp;")
                .replace('"', "&quot;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            let _ = writeln!(
                s,
                r#"<path data-group="{name}" fill="none" stroke="red" d="{}"/>"#,
                z.path_data()
            );
        }
        s.push_str("</svg>\n");
        s
    }

    /// The zones as a label layer over `raster`. `zone_scale` maps zone coordinates to
    /// document pixels (the document's `scale` when zones are drawn on the source picture).
    #[must_use]
    pub fn layer(&self, raster: &PieceRaster, zone_scale: f32) -> LabelLayer {
        let mut names: Vec<String> = Vec::new();
        let mut index = Vec::with_capacity(self.zones.len());
        for z in &self.zones {
            let i = names.iter().position(|n| *n == z.name).unwrap_or_else(|| {
                names.push(z.name.clone());
                names.len() - 1
            });
            index.push(i as u16 + 1);
        }
        let k = zone_scale * raster.res;
        let mut labels = vec![0u16; raster.labels.len()];
        // last zone first, so earlier zones overwrite later ones
        for (z, &label) in self.zones.iter().zip(&index).rev() {
            for px in covered_pixels(
                &z.path,
                Transform::from_scale(k, k),
                raster.width,
                raster.height,
            ) {
                labels[px as usize] = label;
            }
        }
        LabelLayer { names, labels }
    }

    /// Assigns groups piece by piece (share 0.5, raster 4096): the short form of
    /// [`assign`] with this map as the only layer.
    pub fn assign(&self, doc: &mut PieceDocument, zone_scale: f32) -> BTreeMap<String, usize> {
        let raster = PieceRaster::fit(doc, 4096);
        let layer = self.layer(&raster, zone_scale);
        assign(doc, &raster, &layer, &AssignParams::default())
    }
}

/// A name (or none) per raster pixel.
#[derive(Clone, Debug)]
pub struct LabelLayer {
    pub names: Vec<String>,
    /// Index + 1 into `names`; 0 = no name.
    pub labels: Vec<u16>,
}

impl LabelLayer {
    /// A colour mask as a layer over `raster`; the mask is stretched to the document.
    /// With a legend, each pixel takes the nearest legend colour within 48 (RGB distance)
    /// and other colours mean no object; without one, every colour is an object named by
    /// its hex code. Transparent pixels (alpha < 128) are no object.
    #[must_use]
    pub fn from_mask(
        mask: &image::RgbaImage,
        legend: Option<&[(String, [u8; 3])]>,
        raster: &PieceRaster,
    ) -> Self {
        let (mw, mh) = mask.dimensions();
        let mut names: Vec<String> = legend
            .map(|l| l.iter().map(|(n, _)| n.clone()).collect())
            .unwrap_or_default();
        let mut by_color: HashMap<[u8; 3], u16> = HashMap::new();
        let mut labels = vec![0u16; raster.labels.len()];
        let (w, h) = (raster.width as usize, raster.height as usize);
        for y in 0..h {
            let my = ((y as u64 * u64::from(mh)) / h as u64) as u32;
            for x in 0..w {
                let mx = ((x as u64 * u64::from(mw)) / w as u64) as u32;
                let p = mask.get_pixel(mx, my).0;
                if p[3] < 128 {
                    continue;
                }
                let rgb = [p[0], p[1], p[2]];
                let label = *by_color.entry(rgb).or_insert_with(|| {
                    if let Some(l) = legend {
                        l.iter()
                            .enumerate()
                            .map(|(i, (_, c))| {
                                let d: i32 = c
                                    .iter()
                                    .zip(&rgb)
                                    .map(|(&a, &b)| (i32::from(a) - i32::from(b)).pow(2))
                                    .sum();
                                (d, i)
                            })
                            .min()
                            .filter(|(d, _)| *d <= 48 * 48)
                            .map_or(0, |(_, i)| i as u16 + 1)
                    } else {
                        names.push(format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]));
                        names.len() as u16
                    }
                });
                labels[y * w + x] = label;
            }
        }
        LabelLayer { names, labels }
    }

    /// This layer with `other` showing through wherever this one has no name.
    #[must_use]
    pub fn under(mut self, other: &LabelLayer) -> Self {
        let mut map: Vec<u16> = vec![0];
        for n in &other.names {
            let i = self.names.iter().position(|m| m == n).unwrap_or_else(|| {
                self.names.push(n.clone());
                self.names.len() - 1
            });
            map.push(i as u16 + 1);
        }
        for (l, &o) in self.labels.iter_mut().zip(&other.labels) {
            if *l == 0 {
                *l = map[o as usize];
            }
        }
        self
    }
}

/// Reads a legend: a JSON object of name to `#rrggbb`.
///
/// # Errors
/// When the text is not such an object or a colour does not parse.
pub fn legend_from_json(text: &str) -> Result<Vec<(String, [u8; 3])>, String> {
    let map: BTreeMap<String, String> = serde_json::from_str(text).map_err(|e| e.to_string())?;
    map.into_iter()
        .map(|(name, hex)| {
            let c = svgrtypes::Color::from_str(&hex).map_err(|e| format!("{name}: {e}"))?;
            Ok((name, [c.red, c.green, c.blue]))
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct AssignParams {
    /// Least share of a unit's pixels the winning name must cover.
    pub share: f32,
    /// Vote per automatic cluster instead of per piece.
    pub clusters: Option<ClusterParams>,
}

impl Default for AssignParams {
    fn default() -> Self {
        AssignParams {
            share: 0.5,
            clusters: None,
        }
    }
}

/// Names every piece by the vote of its unit (see the module docs) and returns the number
/// of pieces per name. Pieces that win no vote get an empty group.
pub fn assign(
    doc: &mut PieceDocument,
    raster: &PieceRaster,
    layer: &LabelLayer,
    p: &AssignParams,
) -> BTreeMap<String, usize> {
    let n = doc.pieces.len();
    // unit per piece: its cluster, or itself (outline pieces always vote alone)
    let units: Vec<u32> = match &p.clusters {
        Some(cp) => {
            let c = cluster::clusters(doc, raster, cp);
            let base = c.iter().flatten().max().map_or(0, |m| m + 1);
            c.iter()
                .enumerate()
                .map(|(i, c)| c.unwrap_or(base + i as u32))
                .collect()
        }
        None => (0..n as u32).collect(),
    };
    let n_units = units.iter().max().map_or(0, |m| *m as usize + 1);
    let mut votes: HashMap<(u32, u16), u32> = HashMap::new();
    let mut totals = vec![0u32; n_units];
    for (&pl, &zl) in raster.labels.iter().zip(&layer.labels) {
        if pl == 0 {
            continue;
        }
        let u = units[pl as usize - 1];
        totals[u as usize] += 1;
        if zl > 0 {
            *votes.entry((u, zl)).or_insert(0) += 1;
        }
    }
    let mut best: Vec<(u32, u16)> = vec![(0, 0); n_units];
    for (&(u, zl), &c) in &votes {
        let b = &mut best[u as usize];
        if c > b.0 || (c == b.0 && zl < b.1) {
            *b = (c, zl);
        }
    }
    // pieces too small or too hidden to own a raster pixel take the name at their centre
    let at_centre = |bbox: [f32; 4]| {
        let x = ((bbox[0] + bbox[2] / 2.0) * raster.res) as usize;
        let y = ((bbox[1] + bbox[3] / 2.0) * raster.res) as usize;
        let (w, h) = (raster.width as usize, raster.height as usize);
        layer.labels[y.min(h - 1) * w + x.min(w - 1)]
    };
    let mut counts = BTreeMap::new();
    for (piece, &u) in doc.pieces.iter_mut().zip(&units) {
        let (mut c, mut zl) = best[u as usize];
        let mut total = totals[u as usize];
        if total == 0 {
            zl = at_centre(piece.bbox);
            (c, total) = (1, 1);
        }
        if zl > 0 && total > 0 && c as f32 >= p.share * total as f32 {
            let name = &layer.names[zl as usize - 1];
            piece.group.clone_from(name);
            *counts.entry(name.clone()).or_insert(0) += 1;
        } else {
            piece.group.clear();
        }
    }
    counts
}

/// Renames groups: `{"horse": ["c12", "c40"], "battle/knights": ["knights"]}` gives the
/// pieces of clusters `c12` and `c40` the name `horse` and nests `knights` under `battle`.
/// Groups not listed keep their names. Returns the number of renamed pieces.
pub fn rename(doc: &mut PieceDocument, names: &BTreeMap<String, Vec<String>>) -> usize {
    let mut to: HashMap<&str, &str> = HashMap::new();
    for (new, olds) in names {
        for old in olds {
            to.insert(old.as_str(), new.as_str());
        }
    }
    let mut n = 0;
    for p in &mut doc.pieces {
        if let Some(new) = to.get(p.group.as_str()) {
            p.group = (*new).to_string();
            n += 1;
        }
    }
    n
}

/// Writes the clusters as groups `c0`, `c1`, ... (largest first) for review and naming;
/// returns the number of clusters.
pub fn name_clusters(doc: &mut PieceDocument, raster: &PieceRaster, p: &ClusterParams) -> usize {
    let c = cluster::clusters(doc, raster, p);
    for (piece, c) in doc.pieces.iter_mut().zip(&c) {
        piece.group = c.map_or_else(String::new, |c| format!("c{c}"));
    }
    c.iter().flatten().max().map_or(0, |m| *m as usize + 1)
}

/// Distinct colours for a check picture, one per group name, deterministic.
#[must_use]
pub fn group_color(name: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut h);
    let v = h.finish();
    let hue = (v % 360) as f32;
    let light = 0.45 + ((v >> 16) % 3) as f32 * 0.1;
    let (r, g, b) = hsl(hue, 0.8, light);
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
    /// An SVG where every piece is filled with its group's colour (grey when ungrouped),
    /// the outline pieces stay black, and each group's name is written on its largest
    /// piece: the picture to look at after assigning groups.
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
        let mut largest: BTreeMap<&str, &crate::document::Piece> = BTreeMap::new();
        for p in &self.pieces {
            let fill = if p.color == 0 {
                "#000".to_string()
            } else if p.group.is_empty() {
                "#777".to_string()
            } else {
                group_color(&p.group)
            };
            let _ = write!(s, r#"<path fill="{fill}" d="{}"/>"#, p.d);
            if !p.group.is_empty() {
                let e = largest.entry(&p.group).or_insert(p);
                if p.area > e.area {
                    *e = p;
                }
            }
        }
        // smaller names when there are many (hundreds of clusters: zoom into the SVG)
        let crowd = (40.0 / largest.len().max(1) as f32).sqrt().min(1.0);
        let size = (self.width.max(self.height) as f32 / 90.0 * crowd).max(6.0);
        for (name, p) in largest {
            let (cx, cy) = (p.bbox[0] + p.bbox[2] / 2.0, p.bbox[1] + p.bbox[3] / 2.0);
            let _ = write!(
                s,
                r##"<text x="{cx:.0}" y="{cy:.0}" font-family="sans-serif" font-size="{size:.0}" text-anchor="middle" fill="#fff" stroke="#000" stroke-width="{sw:.1}" paint-order="stroke">{name}</text>"##,
                sw = size / 6.0
            );
        }
        s.push_str("</svg>");
        s
    }

    /// A copy whose palette shows the groups instead of the glass: every group one colour
    /// ([`group_color`]), ungrouped pieces grey, outlines black. Same pieces and ids, so it
    /// builds a [`crate::PieceTree`] that can cross-fade with the real picture.
    #[must_use]
    pub fn colored_by_group(&self) -> PieceDocument {
        let mut palette = vec!["#000000".to_string(), "#777777".to_string()];
        let mut index: HashMap<&str, u16> = HashMap::new();
        let mut pieces = self.pieces.clone();
        for (p, src) in pieces.iter_mut().zip(&self.pieces) {
            p.color = if src.color == 0 {
                0
            } else if src.group.is_empty() {
                1
            } else {
                *index.entry(&src.group).or_insert_with(|| {
                    palette.push(group_color(&src.group));
                    (palette.len() - 1) as u16
                })
            };
        }
        PieceDocument {
            width: self.width,
            height: self.height,
            scale: self.scale,
            palette,
            pieces,
        }
    }

    /// The bounding box `[x, y, w, h]` of the object `name` (with its sub-objects), in
    /// document pixels; `None` when no piece belongs to it.
    #[must_use]
    pub fn group_bbox(&self, name: &str) -> Option<[f32; 4]> {
        let mut b: Option<[f32; 4]> = None;
        for p in self.pieces.iter().filter(|p| in_group(&p.group, name)) {
            let [x, y, w, h] = p.bbox;
            b = Some(match b {
                None => [x, y, x + w, y + h],
                Some([a, c, d, e]) => [a.min(x), c.min(y), d.max(x + w), e.max(y + h)],
            });
        }
        b.map(|[a, c, d, e]| [a, c, d - a, e - c])
    }

    /// Every object name, with the parents of nested names (`rider` for `rider/spurs`).
    #[must_use]
    pub fn group_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for p in &self.pieces {
            let mut at = 0;
            while at < p.group.len() {
                let end = p.group[at..].find('/').map_or(p.group.len(), |k| at + k);
                let name = &p.group[..end];
                if !names.iter().any(|n| n == name) {
                    names.push(name.to_string());
                }
                at = end + 1;
            }
        }
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Piece;

    fn square(id: u32, color: u16, x: f32, y: f32, s: f32) -> Piece {
        Piece {
            id,
            color,
            d: format!("M{x} {y}L{} {y}L{} {}L{x} {}Z", x + s, x + s, y + s, y + s),
            bbox: [x, y, s, s],
            area: s * s,
            group: String::new(),
        }
    }

    /// Two blue squares and a red one in a row, black outline strips between them.
    fn doc() -> PieceDocument {
        PieceDocument {
            width: 100,
            height: 40,
            scale: 1,
            palette: vec!["#000000".into(), "#2040c0".into(), "#c02020".into()],
            pieces: vec![
                square(1, 1, 0.0, 0.0, 30.0),
                square(2, 1, 34.0, 0.0, 30.0),
                square(3, 2, 68.0, 0.0, 30.0),
                Piece {
                    id: 4,
                    color: 0,
                    d: "M30 0L34 0L34 30L30 30ZM64 0L68 0L68 30L64 30Z".into(),
                    bbox: [30.0, 0.0, 38.0, 30.0],
                    area: 240.0,
                    group: String::new(),
                },
            ],
        }
    }

    #[test]
    fn zone_svg_names_nesting_transforms_and_comments() {
        let svg = r#"<svg><!-- a comment with > inside -->
            <g id="rider" transform="translate(10 0)">
              <rect id="spurs" x="0" y="0" width="5" height="5"/>
              <path inkscape:label="boot" id="path12" d="m 0,10 h 5 v 5 h -5 z"/>
              <circle cx="0" cy="30" r="2"/>
            </g>
            <ellipse data-group="sun" id="e1" cx="50" cy="50" rx="3" ry="2"/>
            <rect x="0" y="0" width="1" height="1"/>
        </svg>"#;
        let m = GroupMap::from_svg(svg);
        let names: Vec<&str> = m.zones.iter().map(|z| z.name.as_str()).collect();
        assert_eq!(names, ["rider/spurs", "rider/boot", "rider", "sun"]);
        let b = m.zones[0].path.bounds();
        assert!((b.left() - 10.0).abs() < 1e-3, "group transform applied");
    }

    #[test]
    fn zones_written_as_svg_read_back_the_same() {
        let a = Zone::polygon("body/arm", &[(1.0, 2.0), (30.5, 2.0), (30.5, 40.0)]).unwrap();
        let b = Zone::polygon("say \"hi\" & <go>", &[(0.0, 0.0), (5.0, 0.0), (5.0, 5.0)]).unwrap();
        assert!(Zone::polygon("x", &[(0.0, 0.0), (1.0, 1.0)]).is_none());
        let map = GroupMap { zones: vec![a, b] };
        let back = GroupMap::from_svg(&map.to_svg(100, 50));
        let names: Vec<&str> = back.zones.iter().map(|z| z.name.as_str()).collect();
        assert_eq!(names, ["body/arm", "say \"hi\" & <go>"]);
        assert_eq!(back.zones[0].path.bounds(), map.zones[0].path.bounds());
        assert_eq!(map.zones[0].path_data(), "M1 2L30.5 2L30.5 40Z");
    }

    #[test]
    fn a_zone_from_pieces_names_exactly_those_pieces() {
        let mut d = doc();
        // the second blue square and the red one, the zone in source pixels at scale 2
        let z = Zone::from_pieces("chosen", &d, &[1, 2], 2.0).unwrap();
        let b = z.path.bounds();
        assert!((b.left() - 17.0).abs() < 1e-3 && (b.right() - 49.0).abs() < 1e-3);
        let raster = PieceRaster::new(&d, 1.0);
        let layer = GroupMap { zones: vec![z] }.layer(&raster, 2.0);
        assign(&mut d, &raster, &layer, &AssignParams::default());
        let groups: Vec<&str> = d.pieces.iter().map(|p| p.group.as_str()).collect();
        assert_eq!(groups, ["", "chosen", "chosen", ""]);
    }

    #[test]
    fn vote_per_piece_and_per_cluster() {
        let mut d = doc();
        let raster = PieceRaster::new(&d, 1.0);
        // a zone over the first square, a sliver of the second and the outline between
        // them (half of the outline piece's pixels: it joins too, share >= 0.5)
        let map = GroupMap::from_svg(
            r#"<svg><g id="obj"><rect id="a" x="0" y="0" width="40" height="30"/></g></svg>"#,
        );
        let layer = map.layer(&raster, 1.0);
        let counts = assign(&mut d, &raster, &layer, &AssignParams::default());
        assert_eq!(d.pieces[0].group, "obj/a");
        assert_eq!(d.pieces[1].group, "", "a sliver is no majority");
        assert_eq!(d.pieces[3].group, "obj/a");
        assert_eq!(counts.get("obj/a"), Some(&2));
        // the two blue squares are one cluster, so the sliver no longer decides alone
        let p = AssignParams {
            share: 0.5,
            clusters: Some(ClusterParams {
                min_area: 0.0,
                ..ClusterParams::default()
            }),
        };
        assign(&mut d, &raster, &layer, &p);
        assert_eq!(d.pieces[0].group, d.pieces[1].group);
        assert_eq!(d.pieces[2].group, "");
    }

    #[test]
    fn clusters_join_like_colours_across_the_outline() {
        let d = doc();
        let raster = PieceRaster::new(&d, 1.0);
        let c = cluster::clusters(
            &d,
            &raster,
            &ClusterParams {
                k: 10.0,
                max_gap: 6.0,
                min_contact: 3.0,
                min_area: 0.0,
            },
        );
        assert_eq!(c[0], c[1]);
        assert_ne!(c[1], c[2]);
        assert_eq!(c[3], None, "outline pieces are no cluster");
    }

    #[test]
    fn hierarchy_helpers() {
        assert!(in_group("rider/spurs", "rider"));
        assert!(in_group("rider", "rider"));
        assert!(!in_group("riders", "rider"));
        let mut d = doc();
        d.pieces[0].group = "rider/spurs".into();
        d.pieces[2].group = "horse".into();
        assert_eq!(d.group_names(), ["rider", "rider/spurs", "horse"]);
        assert_eq!(d.group_bbox("rider"), Some([0.0, 0.0, 30.0, 30.0]));
        let names = BTreeMap::from([("battle/rider".to_string(), vec!["rider/spurs".to_string()])]);
        assert_eq!(rename(&mut d, &names), 1);
        assert_eq!(d.pieces[0].group, "battle/rider");
    }
}
