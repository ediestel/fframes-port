use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
pub enum FontVariant {
    Monospaced(usize),
    Variable,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
pub enum FontStyle {
    /// A face that is neither italic not obliqued.
    #[default]
    Normal,
    /// A form that is generally cursive in nature.
    Italic,
    /// A typically-sloped version of the regular face.
    Oblique,
}

impl From<ttf_parser::Style> for FontStyle {
    fn from(style: ttf_parser::Style) -> Self {
        match style {
            ttf_parser::Style::Italic => FontStyle::Italic,
            ttf_parser::Style::Oblique => FontStyle::Oblique,
            ttf_parser::Style::Normal => FontStyle::Normal,
        }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
pub enum FontStretch {
    UltraCondensed,
    ExtraCondensed,
    Condensed,
    SemiCondensed,
    #[default]
    Normal,
    SemiExpanded,
    Expanded,
    ExtraExpanded,
    UltraExpanded,
}

impl std::fmt::Display for FontStretch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            FontStretch::UltraCondensed => "ultra-condensed",
            FontStretch::ExtraCondensed => "extra-condensed",
            FontStretch::Condensed => "condensed",
            FontStretch::SemiCondensed => "semi-condensed",
            FontStretch::Normal => "normal",
            FontStretch::SemiExpanded => "semi-expanded",
            FontStretch::Expanded => "expanded",
            FontStretch::ExtraExpanded => "extra-expanded",
            FontStretch::UltraExpanded => "ultra-expanded",
        })
    }
}

impl From<ttf_parser::Width> for FontStretch {
    fn from(width: ttf_parser::Width) -> Self {
        match width {
            ttf_parser::Width::UltraCondensed => FontStretch::UltraCondensed,
            ttf_parser::Width::ExtraCondensed => FontStretch::ExtraCondensed,
            ttf_parser::Width::Condensed => FontStretch::Condensed,
            ttf_parser::Width::SemiCondensed => FontStretch::SemiCondensed,
            ttf_parser::Width::Normal => FontStretch::Normal,
            ttf_parser::Width::SemiExpanded => FontStretch::SemiExpanded,
            ttf_parser::Width::Expanded => FontStretch::Expanded,
            ttf_parser::Width::ExtraExpanded => FontStretch::ExtraExpanded,
            ttf_parser::Width::UltraExpanded => FontStretch::UltraExpanded,
        }
    }
}

/// Vertical metrics of a face at one font size, in pixels: the text of a line reaches
/// `ascent` above its baseline and `descent` below it, and `line_gap` is the font's own
/// extra space between lines.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct FontMetrics {
    pub ascent: f32,
    /// A positive distance below the baseline.
    pub descent: f32,
    pub line_gap: f32,
}

impl FontMetrics {
    /// Height of one line of text without any line gap.
    pub fn height(&self) -> f32 {
        self.ascent + self.descent
    }

    /// The metrics of a parsed face at `font_size` pixels.
    pub fn from_face(face: &ttf_parser::Face<'_>, font_size: usize) -> Self {
        let scale = font_size as f32 / f32::from(face.units_per_em());
        Self {
            ascent: f32::from(face.ascender()) * scale,
            descent: -f32::from(face.descender()) * scale,
            line_gap: f32::from(face.line_gap()) * scale,
        }
    }
}

pub trait FontFace<'a>: Sync + Send + std::fmt::Debug {
    fn is_monospaced(&self) -> Option<bool>;
    fn resolve_char_width(&self, font_size: usize, char: char) -> Option<usize>;
    /// Vertical metrics at `font_size` pixels, see [`FontMetrics`].
    fn metrics(&self, font_size: usize) -> Option<FontMetrics>;

    fn font_variant(&self, font_size: usize) -> Option<FontVariant> {
        let is_monospaced = self.is_monospaced()?;

        if is_monospaced {
            Some(FontVariant::Monospaced(
                self.resolve_char_width(font_size, 'm')?,
            ))
        } else {
            Some(FontVariant::Variable)
        }
    }
}

pub trait FontSource<'a>: Sync + Send + std::fmt::Debug {
    fn add_font(&mut self, filename: String, font_data: Arc<dyn AsRef<[u8]> + Sync + Send>);

    fn resolve_font(
        &'a self,
        font_name: &str,
        font_weight: u16,
        font_style: FontStyle,
        font_stretch: FontStretch,
    ) -> Option<Box<dyn FontFace<'a> + 'a>>;
}
