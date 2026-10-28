//! The piece document: the result of vectorising a picture, one entry per piece.
//!
//! Coordinates are pixels of the vectorised image (the picture after upscaling). Outlines
//! are absolute SVG path data (`M`, `L`, `C`, `Z` only) with holes as further subpaths,
//! filled with the nonzero rule. Pieces are stored in draw order; the outline colour
//! (black, palette index 0) comes last.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PieceDocument {
    pub width: u32,
    pub height: u32,
    /// Scale of the document relative to the source picture (3 for a 3x upscale).
    pub scale: u32,
    /// `#rrggbb`, index 0 is the outline colour.
    pub palette: Vec<String>,
    pub pieces: Vec<Piece>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Piece {
    pub id: u32,
    /// Index into `palette`.
    pub color: u16,
    /// Absolute SVG path data in document pixels.
    pub d: String,
    /// `[x, y, w, h]`
    pub bbox: [f32; 4],
    /// Area of the outer contour, in pixels², for sorting and for puzzle cutting.
    pub area: f32,
    /// Object group, assigned later (e.g. `"french_banner"`); empty when unassigned.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
}

impl PieceDocument {
    pub fn save(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let json = serde_json::to_string(self).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }

    pub fn load(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        serde_json::from_str(&text).map_err(std::io::Error::other)
    }

    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }

    #[must_use]
    pub fn color_hex(&self, piece: &Piece) -> &str {
        self.palette
            .get(piece.color as usize)
            .map_or("#000000", String::as_str)
    }

    /// An SVG with one `<g id="group">` per object group (pieces in document order inside
    /// it, ungrouped pieces in `<g id="_rest">` first), one `<path id="pN">` per piece.
    #[must_use]
    pub fn to_svg_grouped(&self) -> String {
        use std::fmt::Write;
        let mut order: Vec<&str> = Vec::new();
        for p in &self.pieces {
            if !p.group.is_empty() && !order.contains(&p.group.as_str()) {
                order.push(&p.group);
            }
        }
        let mut s = String::with_capacity(self.pieces.len() * 200);
        let _ = write!(
            s,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#,
            w = self.width,
            h = self.height
        );
        let emit = |s: &mut String, name: &str, group: &str| {
            let _ = write!(s, r#"<g id="{name}">"#);
            for p in self.pieces.iter().filter(|p| p.group == group) {
                let _ = write!(
                    s,
                    r#"<path id="p{}" fill="{}" d="{}"/>"#,
                    p.id,
                    self.color_hex(p),
                    p.d
                );
            }
            s.push_str("</g>");
        };
        emit(&mut s, "_rest", "");
        for g in order {
            emit(&mut s, g, g);
        }
        s.push_str("</svg>");
        s
    }

    /// A plain SVG: one `<g>` per colour in draw order, one `<path>` per piece with its id.
    #[must_use]
    pub fn to_svg(&self) -> String {
        use std::fmt::Write;
        let mut s = String::with_capacity(self.pieces.len() * 200);
        let _ = write!(
            s,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#,
            w = self.width,
            h = self.height
        );
        let mut current: Option<u16> = None;
        for p in &self.pieces {
            if current != Some(p.color) {
                if current.is_some() {
                    s.push_str("</g>");
                }
                let _ = write!(s, r#"<g fill="{}" fill-rule="nonzero">"#, self.color_hex(p));
                current = Some(p.color);
            }
            if p.group.is_empty() {
                let _ = write!(s, r#"<path id="p{}" d="{}"/>"#, p.id, p.d);
            } else {
                let _ = write!(
                    s,
                    r#"<path id="p{}" data-group="{}" d="{}"/>"#,
                    p.id, p.group, p.d
                );
            }
        }
        if current.is_some() {
            s.push_str("</g>");
        }
        s.push_str("</svg>");
        s
    }
}
