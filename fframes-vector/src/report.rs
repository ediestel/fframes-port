//! The doubtful pieces after grouping, so only they need a look:
//!
//! - **unnamed**: the piece got no group;
//! - **split**: a second name, or no name, covers at least `split` of the piece's pixels, so
//!   the name it got may be wrong (a zone drawn across it, a cluster vote that overrode it).
//!
//! The shares come from the same [`PieceRaster`] and [`LabelLayer`] as the vote. The report
//! is written as JSON (for an editor to jump to each piece) and marked in red on the check
//! picture.

use crate::document::PieceDocument;
use crate::group::LabelLayer;
use crate::raster::PieceRaster;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Doubt {
    Unnamed,
    Split,
}

#[derive(Clone, Debug, Serialize)]
pub struct Entry {
    pub id: u32,
    pub doubt: Doubt,
    /// The group the piece has (empty when unnamed).
    pub group: String,
    /// Outline colour (palette index 0).
    pub outline: bool,
    /// `[x, y, w, h]` in document pixels.
    pub bbox: [f32; 4],
    /// Area of the outer contour, in document pixels² (holes included).
    pub area: f32,
    /// Visible area (not covered by later pieces), in document pixels².
    pub visible: f32,
    /// Share of the piece's visible pixels per name, largest first; `""` is no name.
    pub shares: Vec<(String, f32)>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Report {
    pub pieces: usize,
    pub unnamed: usize,
    pub split: usize,
    /// Largest visible area first.
    pub entries: Vec<Entry>,
}

#[derive(Clone, Copy, Debug)]
pub struct ReportParams {
    /// Least share of the second name (or of no name) that makes a named piece doubtful.
    pub split: f32,
}

impl Default for ReportParams {
    fn default() -> Self {
        ReportParams { split: 0.3 }
    }
}

/// The doubtful pieces of `doc` (see the module docs).
#[must_use]
pub fn report(
    doc: &PieceDocument,
    raster: &PieceRaster,
    layer: &LabelLayer,
    p: &ReportParams,
) -> Report {
    // per piece: pixels per label (0 = no name), few labels each
    let mut votes: Vec<Vec<(u16, u32)>> = vec![Vec::new(); doc.pieces.len()];
    for (&pl, &zl) in raster.labels.iter().zip(&layer.labels) {
        if pl == 0 {
            continue;
        }
        let v = &mut votes[pl as usize - 1];
        match v.iter_mut().find(|(l, _)| *l == zl) {
            Some((_, c)) => *c += 1,
            None => v.push((zl, 1)),
        }
    }
    let name = |l: u16| {
        if l == 0 {
            String::new()
        } else {
            layer.names[l as usize - 1].clone()
        }
    };
    let mut r = Report {
        pieces: doc.pieces.len(),
        ..Report::default()
    };
    for (piece, v) in doc.pieces.iter().zip(&mut votes) {
        let total: u32 = v.iter().map(|(_, c)| c).sum();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let shares: Vec<(String, f32)> = v
            .iter()
            .map(|&(l, c)| (name(l), c as f32 / total.max(1) as f32))
            .collect();
        let doubt = if piece.group.is_empty() {
            Doubt::Unnamed
        } else if shares.get(1).is_some_and(|s| s.1 >= p.split) {
            Doubt::Split
        } else {
            continue;
        };
        match doubt {
            Doubt::Unnamed => r.unnamed += 1,
            Doubt::Split => r.split += 1,
        }
        r.entries.push(Entry {
            id: piece.id,
            doubt,
            group: piece.group.clone(),
            outline: piece.color == 0,
            bbox: piece.bbox,
            area: piece.area,
            visible: total as f32 / (raster.res * raster.res),
            shares,
        });
    }
    r.entries
        .sort_by(|a, b| b.visible.total_cmp(&a.visible).then(a.id.cmp(&b.id)));
    r
}

impl PieceDocument {
    /// The group check picture ([`PieceDocument::to_group_check_svg`]) with the doubtful
    /// pieces of `report` outlined: red for unnamed, yellow for split.
    #[must_use]
    pub fn to_report_svg(&self, report: &Report) -> String {
        use std::fmt::Write;
        let mut s = self.to_group_check_svg();
        if s.ends_with("</svg>") {
            s.truncate(s.len() - "</svg>".len());
        }
        let width = (self.width.max(self.height) as f32 / 1500.0).max(1.0);
        let by_id: std::collections::HashMap<u32, Doubt> =
            report.entries.iter().map(|e| (e.id, e.doubt)).collect();
        for p in &self.pieces {
            if let Some(d) = by_id.get(&p.id) {
                let colour = match d {
                    Doubt::Unnamed => "#ff2020",
                    Doubt::Split => "#ffd400",
                };
                let _ = write!(
                    s,
                    r#"<path fill="none" stroke="{colour}" stroke-width="{width:.1}" d="{}"/>"#,
                    p.d
                );
            }
        }
        s.push_str("</svg>");
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Piece;
    use crate::group::{AssignParams, GroupMap, assign};

    fn square(id: u32, x: f32, s: f32) -> Piece {
        Piece {
            id,
            color: 1,
            d: format!("M{x} 0L{} 0L{} {s}L{x} {s}Z", x + s, x + s),
            bbox: [x, 0.0, s, s],
            area: s * s,
            group: String::new(),
        }
    }

    #[test]
    fn unnamed_and_split_pieces_are_reported_largest_first() {
        let mut d = PieceDocument {
            width: 100,
            height: 20,
            scale: 1,
            palette: vec!["#000000".into(), "#2040c0".into()],
            pieces: vec![square(1, 0.0, 20.0), square(2, 30.0, 20.0), square(3, 60.0, 10.0)],
        };
        // piece 1 inside "a"; piece 2 60 % "a", 40 % "b": named "a" but split; piece 3 none
        let map = GroupMap::from_svg(
            r#"<svg><rect id="a" x="0" y="0" width="42" height="20"/>
                <rect id="b" x="42" y="0" width="10" height="20"/></svg>"#,
        );
        let raster = PieceRaster::new(&d, 1.0);
        let layer = map.layer(&raster, 1.0);
        assign(&mut d, &raster, &layer, &AssignParams::default());
        let r = report(&d, &raster, &layer, &ReportParams::default());
        assert_eq!((r.pieces, r.unnamed, r.split), (3, 1, 1));
        assert_eq!(r.entries[0].id, 2, "largest first");
        assert_eq!(r.entries[0].doubt, Doubt::Split);
        assert_eq!(r.entries[0].group, "a");
        assert_eq!(r.entries[0].shares[0].0, "a");
        assert!((r.entries[0].shares[1].1 - 0.4).abs() < 0.01);
        assert_eq!(r.entries[1].id, 3);
        assert_eq!(r.entries[1].doubt, Doubt::Unnamed);
        assert_eq!(r.entries[1].shares, [(String::new(), 1.0)]);
        let svg = d.to_report_svg(&r);
        assert_eq!(svg.matches("stroke=\"#ff2020\"").count(), 1);
        assert_eq!(svg.matches("stroke=\"#ffd400\"").count(), 1);
        assert!(svg.ends_with("</svg>"));
    }
}
