//! Cutting: pieces that cross a zone border are split along it, so every part belongs to
//! one object. Needed where a part moves on its own: an outline running from arm to body,
//! a sleeve piece across the elbow.
//!
//! The zones are those of a zone SVG ([`GroupMap`]), with the same precedence as in
//! grouping (earlier shapes win where they overlap). For every piece:
//!
//! 1. The piece is intersected with the region of every name it touches, and what lies
//!    outside all zones is the remainder (no name).
//! 2. Parts smaller than `min_share` of the piece join its largest part, so a zone border
//!    that only grazes a piece leaves it whole.
//! 3. A piece left with one part stays untouched (its path data unchanged). Otherwise its
//!    parts replace it at its place in the draw order: the largest keeps the piece's id, the
//!    others get new ids, every part carries its zone's name as its group.
//! 4. The parts are drawn bottom to top: the remainder first, then the zones from last to
//!    first in the file, so the earlier zone, which wins where zones overlap, is on top.
//!    Every part reaches `overlap` pixels under the parts above it; the top part keeps the
//!    exact cut.
//!
//! Both sides of a cut take their border from the same zone outline. Two anti-aliased edges
//! that only touch let the background show through as a hairline; a lower part that runs on
//! under the upper one covers the seam, and when the upper part moves away (a forearm
//! turning at the elbow) the lip is the overlap a joint needs. The piece's own outline is
//! unchanged. Curves of cut pieces become line segments within `tolerance` pixels; the
//! boolean operations run on an integer grid of 2^29 steps across the piece (`i_overlay`).

use crate::document::{Piece, PieceDocument};
use crate::group::GroupMap;
use crate::raster::skia_path;
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;
use svgr::tiny_skia::{LineJoin, Path, PathBuilder, PathSegment, Stroke, Transform};

type Point = [f64; 2];
type Shapes = Vec<Vec<Vec<Point>>>;

#[derive(Clone, Copy, Debug)]
pub struct CutParams {
    /// Least share of a piece's area a part must have to become a piece of its own.
    pub min_share: f32,
    /// Largest distance, in document pixels, between a curve and the segments replacing it.
    pub tolerance: f32,
    /// How far, in document pixels, a part reaches under the parts drawn above it. At
    /// least one output pixel at the smallest size the picture is shown at; the default is
    /// one source pixel of a document upscaled 3x.
    pub overlap: f32,
}

impl Default for CutParams {
    fn default() -> Self {
        CutParams {
            min_share: 0.05,
            tolerance: 0.05,
            overlap: 3.0,
        }
    }
}

/// What a cut did.
#[derive(Clone, Debug, Default)]
pub struct CutSummary {
    /// Pieces split.
    pub cut: usize,
    /// Pieces they became.
    pub parts: usize,
    /// Id and group of every part, to keep the names after a later vote.
    pub named: Vec<(u32, String)>,
}

/// A path as closed polygons (nonzero rule), curves replaced by segments within `tol`.
fn polygons(path: &Path, tol: f32) -> Vec<Vec<Point>> {
    let tol = f64::from(tol.max(1e-3));
    let mut out: Vec<Vec<Point>> = Vec::new();
    let mut cur: Vec<Point> = Vec::new();
    let pt = |p: svgr::tiny_skia::Point| [f64::from(p.x), f64::from(p.y)];
    // repeated points (a contour that ends on its start) are zero-length edges, which make
    // the overlay drop a neighbouring corner
    let push = |out: &mut Vec<Vec<Point>>, cur: &mut Vec<Point>| {
        cur.dedup();
        while cur.len() > 1 && cur.first() == cur.last() {
            cur.pop();
        }
        if cur.len() >= 3 {
            out.push(std::mem::take(cur));
        } else {
            cur.clear();
        }
    };
    for seg in path.segments() {
        match seg {
            PathSegment::MoveTo(p) => {
                push(&mut out, &mut cur);
                cur.push(pt(p));
            }
            PathSegment::LineTo(p) => cur.push(pt(p)),
            PathSegment::QuadTo(c, p) => {
                let p0 = *cur.last().unwrap_or(&pt(c));
                let (c, p) = (pt(c), pt(p));
                let m = norm([p0[0] - 2.0 * c[0] + p[0], p0[1] - 2.0 * c[1] + p[1]]);
                let n = steps(0.25 * m, tol);
                for i in 1..=n {
                    let t = f64::from(i) / f64::from(n);
                    let u = 1.0 - t;
                    cur.push([
                        u * u * p0[0] + 2.0 * u * t * c[0] + t * t * p[0],
                        u * u * p0[1] + 2.0 * u * t * c[1] + t * t * p[1],
                    ]);
                }
            }
            PathSegment::CubicTo(c1, c2, p) => {
                let p0 = *cur.last().unwrap_or(&pt(c1));
                let (c1, c2, p) = (pt(c1), pt(c2), pt(p));
                let m = norm([p0[0] - 2.0 * c1[0] + c2[0], p0[1] - 2.0 * c1[1] + c2[1]]).max(
                    norm([c1[0] - 2.0 * c2[0] + p[0], c1[1] - 2.0 * c2[1] + p[1]]),
                );
                let n = steps(0.75 * m, tol);
                for i in 1..=n {
                    let t = f64::from(i) / f64::from(n);
                    let u = 1.0 - t;
                    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    cur.push([
                        a * p0[0] + b * c1[0] + c * c2[0] + d * p[0],
                        a * p0[1] + b * c1[1] + c * c2[1] + d * p[1],
                    ]);
                }
            }
            PathSegment::Close => push(&mut out, &mut cur),
        }
    }
    push(&mut out, &mut cur);
    out
}

fn norm(v: Point) -> f64 {
    v[0].hypot(v[1])
}

/// Segments for a curve whose second differences reach `bend`, so it stays within `tol`.
fn steps(bend: f64, tol: f64) -> u32 {
    ((bend / tol).sqrt().ceil() as u32).clamp(1, 256)
}

/// Signed area of a closed polygon.
fn ring_area(ring: &[Point]) -> f64 {
    let mut a = 0.0;
    for (i, p) in ring.iter().enumerate() {
        let q = ring[(i + 1) % ring.len()];
        a += p[0] * q[1] - q[0] * p[1];
    }
    a / 2.0
}

/// Filled area of shapes (outer contours minus holes).
fn filled_area(shapes: &Shapes) -> f64 {
    shapes
        .iter()
        .map(|s| {
            let outer = s.first().map_or(0.0, |r| ring_area(r).abs());
            outer - s.iter().skip(1).map(|r| ring_area(r).abs()).sum::<f64>()
        })
        .sum()
}

fn bounds(points: impl IntoIterator<Item = Point>) -> Option<[f64; 4]> {
    points.into_iter().fold(None, |b, [x, y]| {
        Some(match b {
            None => [x, y, x, y],
            Some([a, c, d, e]) => [a.min(x), c.min(y), d.max(x), e.max(y)],
        })
    })
}

fn shapes_bounds(shapes: &Shapes) -> Option<[f64; 4]> {
    bounds(shapes.iter().flatten().flatten().copied())
}

fn overlap(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3]
}

/// One piece from `shapes`, with the colour of `like`.
fn piece_from(shapes: &Shapes, id: u32, like: &Piece, group: &str) -> Piece {
    use std::fmt::Write;
    // two decimals (0.005 px), without trailing zeros
    let num = |d: &mut String, v: f64| {
        let start = d.len();
        let _ = write!(d, "{v:.2}");
        let kept = d[start..].trim_end_matches('0').trim_end_matches('.').len();
        d.truncate(start + kept);
        if &d[start..] == "-0" {
            d.replace_range(start.., "0");
        }
    };
    let mut d = String::new();
    for ring in shapes.iter().flatten() {
        for (i, &[x, y]) in ring.iter().enumerate() {
            d.push(if i == 0 { 'M' } else { 'L' });
            num(&mut d, x);
            d.push(' ');
            num(&mut d, y);
        }
        d.push('Z');
    }
    let b = shapes_bounds(shapes).unwrap_or([0.0; 4]);
    Piece {
        id,
        color: like.color,
        d,
        bbox: [
            b[0] as f32,
            b[1] as f32,
            (b[2] - b[0]) as f32,
            (b[3] - b[1]) as f32,
        ],
        area: shapes
            .iter()
            .filter_map(|s| s.first())
            .map(|r| ring_area(r).abs())
            .sum::<f64>() as f32,
        group: group.to_string(),
    }
}

/// Shapes as a path (every contour closed).
fn shapes_path(shapes: &Shapes) -> Option<Path> {
    let mut pb = PathBuilder::new();
    for ring in shapes.iter().flatten() {
        for (i, &[x, y]) in ring.iter().enumerate() {
            if i == 0 {
                pb.move_to(x as f32, y as f32);
            } else {
                pb.line_to(x as f32, y as f32);
            }
        }
        pb.close();
    }
    pb.finish()
}

/// The band of half-width `half` along every contour of `shapes`.
fn band(shapes: &Shapes, half: f32, tol: f32) -> Vec<Vec<Point>> {
    let stroke = Stroke {
        width: 2.0 * half,
        line_join: LineJoin::Round,
        ..Stroke::default()
    };
    shapes_path(shapes)
        .and_then(|p| p.stroke(&stroke, 1.0))
        .map_or_else(Vec::new, |p| polygons(&p, tol))
}

/// `shapes` grown (`half` > 0) or shrunk (`half` < 0) by `|half|` pixels.
fn grown(shapes: &Shapes, half: f32, tol: f32) -> Shapes {
    if half == 0.0 || shapes.is_empty() {
        return shapes.clone();
    }
    let rule = if half > 0.0 {
        OverlayRule::Union
    } else {
        OverlayRule::Difference
    };
    shapes.overlay(&band(shapes, half.abs(), tol), rule, FillRule::NonZero)
}

/// The region of every name of `map` in document pixels (earlier zones win), in the order
/// names first appear, and the union of all of them.
fn regions(map: &GroupMap, zone_scale: f32, tol: f32) -> (Vec<(String, Shapes)>, Shapes) {
    let mut named: Vec<(String, Shapes)> = Vec::new();
    let mut covered: Shapes = Vec::new();
    let ts = Transform::from_scale(zone_scale, zone_scale);
    for z in &map.zones {
        let Some(path) = z.path.clone().transform(ts) else {
            continue;
        };
        let zone = polygons(&path, tol);
        if zone.is_empty() {
            continue;
        }
        let own = zone.overlay(&covered, OverlayRule::Difference, FillRule::NonZero);
        covered = covered.overlay(&zone, OverlayRule::Union, FillRule::NonZero);
        match named.iter_mut().find(|(n, _)| *n == z.name) {
            Some((_, r)) => *r = r.overlay(&own, OverlayRule::Union, FillRule::NonZero),
            None => named.push((z.name.clone(), own)),
        }
    }
    (named, covered)
}

/// Splits every piece of `doc` that crosses a border of the zones in `map` (see the module
/// docs). `zone_scale` maps zone coordinates to document pixels, as in grouping.
pub fn cut_at_zones(
    doc: &mut PieceDocument,
    map: &GroupMap,
    zone_scale: f32,
    p: &CutParams,
) -> CutSummary {
    let (named, covered) = regions(map, zone_scale, p.tolerance);
    let boxes: Vec<Option<[f64; 4]>> = named.iter().map(|(_, s)| shapes_bounds(s)).collect();
    let mut next_id = doc.pieces.iter().map(|q| q.id).max().map_or(0, |m| m + 1);
    let mut summary = CutSummary::default();
    let mut pieces = Vec::with_capacity(doc.pieces.len());
    for piece in std::mem::take(&mut doc.pieces) {
        let [x, y, w, h] = piece.bbox.map(f64::from);
        let pbox = [x, y, x + w, y + h];
        let touched: Vec<usize> = (0..named.len())
            .filter(|&i| boxes[i].is_some_and(|b| overlap(b, pbox)))
            .collect();
        let path = if touched.is_empty() {
            None
        } else {
            skia_path(&piece.d)
        };
        let Some(path) = path else {
            pieces.push(piece);
            continue;
        };
        let subj = polygons(&path, p.tolerance);
        let whole = filled_area(&subj.overlay(&Shapes::new(), OverlayRule::Union, FillRule::NonZero));
        // (name, shapes, area, draw rank): the remainder lowest, earlier zones higher
        let mut parts: Vec<(String, Shapes, f64, usize)> = Vec::new();
        for &i in &touched {
            let s = subj.overlay(&named[i].1, OverlayRule::Intersect, FillRule::NonZero);
            let a = filled_area(&s);
            if a > 0.0 {
                parts.push((named[i].0.clone(), s, a, named.len() - i));
            }
        }
        let rest = subj.overlay(&covered, OverlayRule::Difference, FillRule::NonZero);
        let a = filled_area(&rest);
        if a > 0.0 {
            parts.push((String::new(), rest, a, 0));
        }
        let min = f64::from(p.min_share) * whole;
        if parts.iter().filter(|part| part.2 >= min).count() < 2 {
            pieces.push(piece);
            continue;
        }
        // largest first; small parts join it
        parts.sort_by(|a, b| b.2.total_cmp(&a.2));
        let (small, mut kept): (Vec<_>, Vec<_>) = parts.into_iter().partition(|part| part.2 < min);
        for (_, s, _, _) in small {
            kept[0].1 = kept[0].1.overlay(&s, OverlayRule::Union, FillRule::NonZero);
        }
        summary.cut += 1;
        summary.parts += kept.len();
        let first = kept[0].0.clone();
        kept.sort_by_key(|part| part.3);
        // every part reaches `overlap` under the parts drawn above it
        let mut shapes: Vec<Shapes> = kept.iter().map(|part| part.1.clone()).collect();
        for i in 0..kept.len().saturating_sub(1) {
            let above: Vec<Vec<Point>> = kept[i + 1..]
                .iter()
                .flat_map(|part| part.1.iter().flatten().cloned())
                .collect();
            let lip = grown(&kept[i].1, p.overlap, p.tolerance).overlay(
                &above,
                OverlayRule::Intersect,
                FillRule::NonZero,
            );
            shapes[i] = shapes[i].overlay(&lip, OverlayRule::Union, FillRule::NonZero);
        }
        for ((name, ..), shapes) in kept.iter().zip(&shapes) {
            let id = if *name == first {
                piece.id
            } else {
                next_id += 1;
                next_id - 1
            };
            summary.named.push((id, name.clone()));
            pieces.push(piece_from(shapes, id, &piece, name));
        }
    }
    doc.pieces = pieces;
    summary
}

/// Gives the parts of a cut their zone names again (after a vote that may have renamed
/// them, e.g. by cluster).
pub fn keep_names(doc: &mut PieceDocument, named: &[(u32, String)]) {
    let by_id: std::collections::HashMap<u32, &str> =
        named.iter().map(|(id, n)| (*id, n.as_str())).collect();
    for piece in &mut doc.pieces {
        if let Some(n) = by_id.get(&piece.id) {
            piece.group = (*n).to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn doc(pieces: Vec<Piece>) -> PieceDocument {
        PieceDocument {
            width: 100,
            height: 100,
            scale: 1,
            palette: vec!["#000000".into(), "#2040c0".into()],
            pieces,
        }
    }

    fn filled(p: &Piece) -> f64 {
        union(std::slice::from_ref(p))
    }

    /// Area covered by `pieces` together (overlaps counted once).
    fn union(pieces: &[Piece]) -> f64 {
        let rings: Vec<Vec<Point>> = pieces
            .iter()
            .flat_map(|p| polygons(&skia_path(&p.d).unwrap(), 0.05))
            .collect();
        filled_area(&rings.overlay(&Shapes::new(), OverlayRule::Union, FillRule::NonZero))
    }

    #[test]
    fn a_piece_across_two_zones_becomes_two_that_overlap_at_the_cut() {
        let mut d = doc(vec![square(7, 1, 0.0, 0.0, 30.0), square(8, 1, 60.0, 60.0, 20.0)]);
        let untouched = d.pieces[1].d.clone();
        let map = GroupMap::from_svg(
            r#"<svg><rect id="arm" x="-5" y="-5" width="17" height="50"/>
                <rect id="body" x="12" y="-5" width="40" height="50"/></svg>"#,
        );
        let s = cut_at_zones(&mut d, &map, 1.0, &CutParams::default());
        assert_eq!((s.cut, s.parts), (1, 2));
        let names: Vec<&str> = d.pieces.iter().map(|p| p.group.as_str()).collect();
        // in place of the piece, bottom to top: "arm" comes first in the file, so on top
        assert_eq!(names, ["body", "arm", ""]);
        // the cut is at x = 12: the arm keeps it exactly, the body runs on 3 px under the
        // arm; the square's own outline stays where it was
        assert_eq!(d.pieces[0].bbox, [9.0, 0.0, 21.0, 30.0]);
        assert_eq!(d.pieces[1].bbox, [0.0, 0.0, 12.0, 30.0]);
        let both = union(&d.pieces[..2]);
        assert!((both - 900.0).abs() < 0.01, "together they are the square: {both}");
        assert_eq!(d.pieces[0].id, 7, "the largest part keeps the id");
        assert_eq!(d.pieces[1].id, 9, "the other gets a new one");
        assert_eq!(d.pieces[2].d, untouched, "pieces off the zones are not rewritten");
    }

    #[test]
    fn a_grazed_piece_stays_whole_and_its_remainder_has_no_name() {
        // the zone covers 2 % of the first square: below min_share, no cut
        let mut d = doc(vec![square(1, 1, 0.0, 0.0, 50.0), square(2, 0, 0.0, 60.0, 40.0)]);
        let before = d.pieces[0].d.clone();
        let map = GroupMap::from_svg(
            r#"<svg><rect id="head" x="49" y="0" width="30" height="50"/>
                <rect id="leg" x="0" y="60" width="20" height="40"/></svg>"#,
        );
        let s = cut_at_zones(&mut d, &map, 1.0, &CutParams::default());
        assert_eq!(d.pieces[0].d, before);
        // the outline square: half in "leg", half outside every zone
        assert_eq!(s.cut, 1);
        let groups: Vec<&str> = d.pieces[1..].iter().map(|p| p.group.as_str()).collect();
        assert_eq!(groups, ["", "leg"]);
        assert!(d.pieces[1..].iter().all(|p| p.color == 0));
    }

    #[test]
    fn earlier_zones_win_and_curves_stay_within_tolerance() {
        // a circle of radius 20 split by two overlapping zones; "hand" comes first
        let k = 0.552_284_8 * 20.0;
        let circle = Piece {
            id: 1,
            color: 1,
            d: format!(
                "M50 30C{a} 30 70 {b} 70 50C70 {c} {a} 70 50 70C{d} 70 30 {c} 30 50C30 {b} {d} 30 50 30Z",
                a = 50.0 + k,
                b = 50.0 - k,
                c = 50.0 + k,
                d = 50.0 - k
            ),
            bbox: [30.0, 30.0, 40.0, 40.0],
            area: 1256.6,
            group: String::new(),
        };
        let flat = filled(&circle);
        // segments inside the curve by at most the tolerance: area off by < tolerance x perimeter
        let exact = std::f64::consts::PI * 400.0;
        let perimeter = std::f64::consts::TAU * 20.0;
        assert!((flat - exact).abs() < 0.05 * perimeter, "{flat} vs {exact}");
        let mut d = doc(vec![circle]);
        let map = GroupMap::from_svg(
            r#"<svg><rect id="hand" x="40" y="0" width="100" height="100"/>
                <rect id="arm" x="0" y="0" width="60" height="100"/></svg>"#,
        );
        cut_at_zones(&mut d, &map, 1.0, &CutParams::default());
        // the cut itself loses nothing: together the parts fill the flattened piece, up to
        // the rounding of written coordinates (0.005 px) along the outline
        let total = union(&d.pieces);
        assert!((total - flat).abs() < 0.005 * perimeter, "{total} vs {flat}");
        // the cut is at x = 40 (hand wins over arm and is on top); the arm runs on 3 px
        // under the hand
        let groups: Vec<&str> = d.pieces.iter().map(|p| p.group.as_str()).collect();
        assert_eq!(groups, ["arm", "hand"]);
        let hand = &d.pieces[1];
        assert!((hand.bbox[0] - 40.0).abs() < 0.002, "hand from x = {}", hand.bbox[0]);
        let arm_end = d.pieces[0].bbox[0] + d.pieces[0].bbox[2];
        assert!((arm_end - 43.0).abs() < 0.002, "arm to x = {arm_end}");
    }
}
