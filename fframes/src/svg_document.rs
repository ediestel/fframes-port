//! `SvgDocument` — typed wrapper around a complete SVG document string.
//!
//! Holding a `SvgDocument` is a proof token: the contained string has
//! passed well-formedness + `<svg>` root validation via
//! [`SvgDocument::try_new`], or was constructed from macro-generated
//! tokens that are trusted by construction via
//! [`SvgDocument::new_unchecked`].
//!
//! The newtype exists to close the class of bugs where invalid or
//! structurally wrong SVG would silently enter the render pipeline.
//! The previous `Svgr { value: String }` layout accepted any string
//! at ingest time and surfaced problems mid-render as opaque parse
//! errors. With `SvgDocument`:
//!
//!   * Runtime strings (LLM output, user input) must flow through
//!     `try_new`, which invokes [`crate::svg_validate::validate_svg_document`]
//!     and rejects malformed input at the ingest boundary.
//!   * Macro-generated strings use `new_unchecked`: their structure is
//!     guaranteed by the `svgr!` macro's typed token expansion, and
//!     running the XML validator on every compile-time-emitted SVG
//!     would add measurable per-frame cost for zero safety gain.
//!
//! The inner `String` is private. All access goes through `as_str` /
//! `into_inner` so the validation boundary cannot be bypassed by
//! direct field mutation.

use crate::error::FFramesError;
use std::fmt;

/// A complete SVG document string that has either (a) been validated
/// via [`Self::try_new`] or (b) been produced by a trusted internal
/// constructor via [`Self::new_unchecked`] (notably the `svgr!`
/// macro). Downstream code that receives an `SvgDocument` may assume
/// the document is well-formed XML with an `<svg>` root.
#[derive(Clone, Debug, Default)]
pub struct SvgDocument {
    inner: String,
}

impl SvgDocument {
    /// Construct a validated `SvgDocument`. Runs the pre-render
    /// validator at ingest time; invalid input yields
    /// [`FFramesError::ValidationError`] with line / column / issue
    /// category.
    ///
    /// This is the canonical path for any runtime-supplied SVG string
    /// (LLM output, user input, config-loaded templates).
    pub fn try_new(value: String) -> Result<Self, FFramesError> {
        crate::svg_validate::validate_svg_document(&value)?;
        Ok(Self { inner: value })
    }

    /// Construct an `SvgDocument` without running validation.
    ///
    /// **Callers:** use only when the SVG text is produced by a
    /// trusted internal constructor whose output structure is
    /// guaranteed at the type level — notably the `svgr!` macro,
    /// which emits valid SVG tokens from a syntax-checked template.
    /// Never call this on runtime-supplied strings; use
    /// [`Self::try_new`] instead.
    ///
    /// Not marked `unsafe` because a malformed string here is a
    /// correctness failure, not a memory-safety failure: the worst
    /// case is a mid-render `ParserError` (the same as before
    /// validation existed), not undefined behaviour.
    pub fn new_unchecked(value: String) -> Self {
        Self { inner: value }
    }

    /// Borrow the inner SVG text. The validation invariant is
    /// preserved — borrows cannot invalidate it.
    pub fn as_str(&self) -> &str {
        &self.inner
    }

    /// Consume the wrapper and return the inner string. Callers that
    /// forward the text to an external parser (e.g. `usvgr::Tree::from_str`)
    /// use this when they need ownership.
    pub fn into_inner(self) -> String {
        self.inner
    }

    /// Byte length of the inner SVG text.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether the inner SVG text is zero-length. The `Default` impl
    /// produces an empty document; runtime-validated documents
    /// cannot be empty because `validate_svg_document` rejects them
    /// at `try_new` time.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl fmt::Display for SvgDocument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.inner)
    }
}

impl AsRef<str> for SvgDocument {
    fn as_ref(&self) -> &str {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_new_accepts_valid_svg() {
        let doc = SvgDocument::try_new(r#"<svg xmlns="http://www.w3.org/2000/svg"/>"#.to_owned())
            .unwrap();
        assert!(!doc.is_empty());
    }

    #[test]
    fn try_new_rejects_empty_string() {
        let err = SvgDocument::try_new(String::new()).unwrap_err();
        assert!(matches!(err, FFramesError::ValidationError(_)));
    }

    #[test]
    fn try_new_rejects_non_svg_xml() {
        let err = SvgDocument::try_new(r#"<html><body/></html>"#.to_owned()).unwrap_err();
        assert!(matches!(err, FFramesError::ValidationError(_)));
    }

    #[test]
    fn new_unchecked_skips_validation() {
        // deliberately invalid — new_unchecked is explicit about the bypass
        let doc = SvgDocument::new_unchecked("not xml at all".to_owned());
        assert_eq!(doc.as_str(), "not xml at all");
    }

    #[test]
    fn default_is_empty() {
        let doc = SvgDocument::default();
        assert!(doc.is_empty());
    }

    #[test]
    fn as_ref_exposes_str() {
        let doc = SvgDocument::new_unchecked("<svg/>".to_owned());
        let s: &str = doc.as_ref();
        assert_eq!(s, "<svg/>");
    }
}
