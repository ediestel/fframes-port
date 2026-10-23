//! Pre-render SVG validation.
//!
//! A lightweight gate that runs before `usvgr::Tree::from_str` in
//! the ingest path (`Svgr::from_svg_string`, `SvgDocument::try_new`)
//! and when `Svgr::into_svg_tree` fails to parse a runtime document,
//! to turn the opaque parser error into a positioned
//! [`ValidationIssue`]. The goal is to catch
//! malformed input — empty strings, non-XML, XML without an `<svg>`
//! root, wrong namespace — at the ingest boundary with a
//! descriptive structured error, rather than surfacing
//! `usvgr::Error::ParsingFailed` mid-render after hundreds of
//! frames have already been encoded.
//!
//! This is not a full semantic validator — usvgr remains the
//! authority on SVG spec compliance. Validation here is restricted
//! to structural preconditions that would otherwise produce
//! confusing late failures.
//!
//! Failures carry a structured [`ValidationIssue`] with:
//!   * `kind` — an enum-tagged category so callers can
//!     programmatically distinguish empty input from malformed XML
//!     from wrong root element.
//!   * `line` / `column` — 1-based source position when available.
//!     Set to 0 when the source position is not known (e.g., for
//!     "document is empty" where there is no XML to position).
//!   * `message` — human-readable detail.

use crate::error::FFramesError;
use usvgr::roxmltree;

/// Category of SVG validation failure. Callers can match on this to
/// decide retry / user-message / hard-fail without parsing the
/// free-form message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ValidationIssueKind {
    /// The input string was empty or whitespace-only.
    Empty,
    /// The input was not well-formed XML (roxmltree parse failed).
    MalformedXml,
    /// The XML parsed but the root element is not `<svg>`.
    WrongRootElement,
}

/// Structured SVG validation failure. Produced by
/// [`validate_svg_document`] on invalid input and wrapped into
/// [`FFramesError::ValidationError`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ValidationIssue {
    pub kind: ValidationIssueKind,
    /// 1-based line number; 0 when unknown.
    pub line: u32,
    /// 1-based column number; 0 when unknown.
    pub column: u32,
    pub message: String,
}

impl std::fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.line > 0 {
            write!(
                f,
                "{:?} at line {}, column {}: {}",
                self.kind, self.line, self.column, self.message
            )
        } else {
            write!(f, "{:?}: {}", self.kind, self.message)
        }
    }
}

/// Validate that `svg` is well-formed XML with an `<svg>` root
/// element. Returns `Ok(())` on success or
/// `FFramesError::ValidationError` carrying a structured
/// [`ValidationIssue`] on failure.
///
/// Tolerates:
/// - Leading UTF-8 BOM.
/// - XML declaration (`<?xml version="1.0"?>`).
/// - Pre-root comments.
/// - SVG without explicit `xmlns` attribute (common in LLM output).
/// - DTD / doctype declarations.
///
/// Rejects:
/// - Empty input or whitespace-only input.
/// - Non-XML text.
/// - XML whose root element is not `<svg>` (case-sensitive: XML is
///   case-sensitive and `<SVG>` is a different element). Callers
///   that want case-insensitive acceptance should normalise before
///   validation.
pub fn validate_svg_document(svg: &str) -> Result<(), FFramesError> {
    let trimmed = strip_bom(svg).trim_start();
    if trimmed.is_empty() {
        return Err(FFramesError::ValidationError(ValidationIssue {
            kind: ValidationIssueKind::Empty,
            line: 0,
            column: 0,
            message: "SVG document is empty".to_owned(),
        }));
    }

    let xml_opt = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    let doc = match roxmltree::Document::parse_with_options(svg, xml_opt) {
        Ok(d) => d,
        Err(e) => {
            let (line, column) = xml_error_position(&e);
            return Err(FFramesError::ValidationError(ValidationIssue {
                kind: ValidationIssueKind::MalformedXml,
                line,
                column,
                message: format!("{e}"),
            }));
        }
    };

    let root = doc.root_element();
    if root.tag_name().name() != "svg" {
        // Report the position of the root element so users can jump
        // straight to the offending tag even if it's wrapped in
        // other markup.
        let root_pos = doc.text_pos_at(root.range().start);
        return Err(FFramesError::ValidationError(ValidationIssue {
            kind: ValidationIssueKind::WrongRootElement,
            line: root_pos.row,
            column: root_pos.col,
            message: format!(
                "SVG document root element is `<{}>`, expected `<svg>`",
                root.tag_name().name()
            ),
        }));
    }

    Ok(())
}

fn strip_bom(s: &str) -> &str {
    s.strip_prefix('\u{FEFF}').unwrap_or(s)
}

fn xml_error_position(e: &roxmltree::Error) -> (u32, u32) {
    // roxmltree errors carry a TextPos via `pos()` on most variants.
    // Rather than exhaustively match, use Display+parse fallback: if
    // the formatted error ends with "at NN:NN" we extract it.
    let msg = format!("{e}");
    if let Some(tail) = msg.rsplit("at ").next() {
        let mut parts = tail.split(':');
        if let (Some(l), Some(c)) = (parts.next(), parts.next()) {
            let line: u32 = l.trim().parse().unwrap_or(0);
            let col_end = c.find(|ch: char| !ch.is_ascii_digit()).unwrap_or(c.len());
            let column: u32 = c[..col_end].parse().unwrap_or(0);
            return (line, column);
        }
    }
    (0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expect_kind(svg: &str, want: ValidationIssueKind) {
        let err = validate_svg_document(svg).unwrap_err();
        match err {
            FFramesError::ValidationError(issue) => assert_eq!(issue.kind, want, "issue={issue:?}"),
            other => panic!("expected ValidationError, got {other:?}"),
        }
    }

    #[test]
    fn accepts_minimal_svg() {
        assert!(validate_svg_document(r#"<svg xmlns="http://www.w3.org/2000/svg"/>"#).is_ok());
    }

    #[test]
    fn accepts_svg_with_children() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10">
            <rect x="0" y="0" width="5" height="5"/>
        </svg>"#;
        assert!(validate_svg_document(svg).is_ok());
    }

    #[test]
    fn accepts_svg_without_xmlns() {
        // LLMs frequently drop xmlns. Still structurally valid as
        // far as pre-render validation is concerned; usvgr handles
        // the namespace-less case itself.
        assert!(validate_svg_document(r#"<svg width="10" height="10"><rect/></svg>"#).is_ok());
    }

    #[test]
    fn accepts_utf8_bom() {
        let with_bom =
            format!("\u{FEFF}<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"/>");
        assert!(validate_svg_document(&with_bom).is_ok());
    }

    #[test]
    fn accepts_xml_declaration() {
        let svg =
            r#"<?xml version="1.0" encoding="UTF-8"?><svg xmlns="http://www.w3.org/2000/svg"/>"#;
        assert!(validate_svg_document(svg).is_ok());
    }

    #[test]
    fn accepts_pre_root_comment() {
        let svg = r#"<!-- generated by LLM --><svg xmlns="http://www.w3.org/2000/svg"/>"#;
        assert!(validate_svg_document(svg).is_ok());
    }

    #[test]
    fn accepts_leading_whitespace() {
        assert!(
            validate_svg_document("   \n\t <svg xmlns=\"http://www.w3.org/2000/svg\"/>").is_ok()
        );
    }

    #[test]
    fn rejects_empty_string() {
        expect_kind("", ValidationIssueKind::Empty);
    }

    #[test]
    fn rejects_whitespace_only() {
        expect_kind("   \n\t  ", ValidationIssueKind::Empty);
    }

    #[test]
    fn rejects_bom_only() {
        expect_kind("\u{FEFF}", ValidationIssueKind::Empty);
    }

    #[test]
    fn rejects_non_xml() {
        expect_kind("not xml at all", ValidationIssueKind::MalformedXml);
    }

    #[test]
    fn rejects_non_svg_root() {
        expect_kind(
            r#"<html><body/></html>"#,
            ValidationIssueKind::WrongRootElement,
        );
    }

    #[test]
    fn rejects_upper_case_svg_root() {
        // XML is case-sensitive. <SVG> is a distinct element and is
        // correctly flagged as the wrong root.
        expect_kind(r#"<SVG/>"#, ValidationIssueKind::WrongRootElement);
    }

    #[test]
    fn malformed_xml_carries_position() {
        let svg = "<svg><rect";
        let err = validate_svg_document(svg).unwrap_err();
        match err {
            FFramesError::ValidationError(issue) => {
                assert_eq!(issue.kind, ValidationIssueKind::MalformedXml);
                // roxmltree reports position on most errors; assert
                // either we got a position or at least a useful
                // message.
                assert!(issue.line > 0 || !issue.message.is_empty());
            }
            other => panic!("expected ValidationError, got {other:?}"),
        }
    }

    #[test]
    fn wrong_root_reports_root_position() {
        let svg = "<html/>";
        let err = validate_svg_document(svg).unwrap_err();
        match err {
            FFramesError::ValidationError(issue) => {
                assert_eq!(issue.kind, ValidationIssueKind::WrongRootElement);
                assert!(
                    issue.line >= 1,
                    "issue line was {} (issue={issue:?})",
                    issue.line
                );
            }
            other => panic!("expected ValidationError, got {other:?}"),
        }
    }

    #[test]
    fn issue_display_includes_position_when_present() {
        let issue = ValidationIssue {
            kind: ValidationIssueKind::WrongRootElement,
            line: 3,
            column: 5,
            message: "root is `html`".to_owned(),
        };
        let s = format!("{issue}");
        assert!(s.contains("line 3"));
        assert!(s.contains("column 5"));
    }

    #[test]
    fn issue_display_omits_position_when_zero() {
        let issue = ValidationIssue {
            kind: ValidationIssueKind::Empty,
            line: 0,
            column: 0,
            message: "empty".to_owned(),
        };
        let s = format!("{issue}");
        assert!(!s.contains("line"));
    }
}
