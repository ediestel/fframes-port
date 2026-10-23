use crate::error::{FFramesError, Result};
use crate::svg_document::SvgDocument;
use std::{fmt, iter::FromIterator};

/// Parses a complete SVG document string. Validation runs only when the parser fails, so a valid
/// document is parsed once and an invalid one reports a positioned [`FFramesError::ValidationError`].
fn parse_runtime_document(
    document: &str,
    opt: &usvgr::Options,
    fontdb: &usvgr::fontdb::Database,
) -> Result<usvgr::Tree> {
    usvgr::Tree::from_str(document, opt, fontdb).map_err(|parser_error| {
        crate::svg_validate::validate_svg_document(document)
            .err()
            .unwrap_or(FFramesError::ParserError(parser_error))
    })
}

#[derive(Default, Clone, Debug)]
pub struct Svgr<'a> {
    #[cfg(any(not(feature = "compile-time-svgtree"), target_arch = "wasm32"))]
    pub value: String,
    #[cfg(any(not(feature = "compile-time-svgtree"), target_arch = "wasm32"))]
    pub marker: std::marker::PhantomData<&'a str>,
    #[cfg(all(feature = "compile-time-svgtree", not(target_arch = "wasm32")))]
    pub svg_tree: usvgr::svgtree::NestedSvgDocument<'a, usvgr::svgtree::NestedNodeData<'a>>,
}

impl<'a> Svgr<'a> {
    #[cfg(all(feature = "compile-time-svgtree", not(target_arch = "wasm32")))]
    pub fn into_svg_tree(
        self,
        opt: &usvgr::Options,
        cache: &mut usvgr::Cache,
        fontdb: &usvgr::fontdb::Database,
    ) -> Result<usvgr::Tree> {
        // A runtime string (`From<String>`, `from_svg_string`) is stored as a single text node.
        // As a whole tree it is a complete document and goes through the XML parser, which also
        // handles `<style>` sheets that the nested tree does not carry.
        if let Some(document) = self.runtime_document() {
            return parse_runtime_document(document, opt, fontdb);
        }

        usvgr::Tree::from_nested_svgtree_with_cache(&self.svg_tree, opt, cache, fontdb)
            .map_err(FFramesError::ParserError)
    }

    #[cfg(any(not(feature = "compile-time-svgtree"), target_arch = "wasm32"))]
    pub fn into_svg_tree(
        self,
        opt: &usvgr::Options,
        _cache: &mut usvgr::Cache,
        fontdb: &usvgr::fontdb::Database,
    ) -> Result<usvgr::Tree> {
        parse_runtime_document(&self.value, opt, fontdb)
    }

    /// The text of a complete SVG document parsed at runtime, `None` for a tree built by the
    /// `svgr!` macro at compile time. The renderer keys its parse cache on this text.
    #[cfg(all(feature = "compile-time-svgtree", not(target_arch = "wasm32")))]
    pub(crate) fn runtime_document(&self) -> Option<&str> {
        match self.svg_tree.nodes.as_slice() {
            [
                Some(usvgr::svgtree::NestedNodeData {
                    kind: usvgr::svgtree::NestedNodeKind::Text(text),
                    ..
                }),
            ] => Some(text.as_str()),
            _ => None,
        }
    }

    #[cfg(any(not(feature = "compile-time-svgtree"), target_arch = "wasm32"))]
    pub(crate) fn runtime_document(&self) -> Option<&str> {
        Some(&self.value)
    }

    /// Validates a complete SVG document produced at runtime (LLM output, user input, a template
    /// loaded from disk) and wraps it. Malformed input fails here with
    /// [`FFramesError::ValidationError`] instead of surfacing as a parser error mid-render.
    ///
    /// Fragments that are composed into a parent `svgr!` keep using `Svgr::from`.
    pub fn from_svg_string(value: String) -> Result<Svgr<'static>> {
        Ok(Self::from_document(SvgDocument::try_new(value)?))
    }

    /// Wraps a document that was already validated through [`SvgDocument::try_new`].
    pub fn from_document(document: SvgDocument) -> Svgr<'static> {
        Svgr::from(document.into_inner())
    }

    #[cfg(all(feature = "compile-time-svgtree", not(target_arch = "wasm32")))]
    pub fn as_subtree(self) -> Vec<Option<usvgr::svgtree::NestedNodeData<'a>>> {
        self.svg_tree.nodes
    }

    #[cfg(any(not(feature = "compile-time-svgtree"), target_arch = "wasm32"))]
    pub fn as_subtree(self) -> Vec<Option<usvgr::svgtree::NestedNodeData<'a>>> {
        unimplemented!(
            "Subtrees are not available when using runtime svg tree, if you see this message it means that feature flags are set incorrectly."
        )
    }

    pub fn empty() -> Self {
        Self::default()
    }
}

#[cfg(any(not(feature = "compile-time-svgtree"), target_arch = "wasm32"))]
impl From<String> for Svgr<'_> {
    fn from(value: String) -> Self {
        Svgr {
            value,
            marker: std::marker::PhantomData,
        }
    }
}

#[cfg(all(feature = "compile-time-svgtree", not(target_arch = "wasm32")))]
impl From<String> for Svgr<'_> {
    fn from(val: String) -> Self {
        use usvgr::svgtree::{NestedNodeData, NestedSvgDocument};

        Svgr {
            svg_tree: NestedSvgDocument::from_nodes(vec![
                Some(NestedNodeData {
                    kind: usvgr::svgtree::NestedNodeKind::Text(
                        usvgr::svgtree::roxmltree::StringStorage::new_owned(val)
                    ),
                    attrs: Box::new([]),
                    children: vec![],
                    static_hash: None,
                });
                1
            ]),
        }
    }
}

#[cfg(any(not(feature = "compile-time-svgtree"), target_arch = "wasm32"))]
impl<'a> FromIterator<Svgr<'a>> for Svgr<'a> {
    fn from_iter<T: IntoIterator<Item = Svgr<'a>>>(iter: T) -> Self {
        Svgr {
            marker: std::marker::PhantomData,
            value: iter
                .into_iter()
                .fold(String::new(), |acc, s| acc + &s.value),
        }
    }
}

#[cfg(all(feature = "compile-time-svgtree", not(target_arch = "wasm32")))]
impl<'a> FromIterator<Svgr<'a>> for Svgr<'a> {
    fn from_iter<T: IntoIterator<Item = Svgr<'a>>>(iter: T) -> Self {
        let mut child_nodes = usvgr::svgtree::NestedSvgDocument::from_nodes(vec![]);

        for sub_tree in iter {
            let mut nested_tree = sub_tree.svg_tree;

            child_nodes.nodes.append(&mut nested_tree.nodes);
        }

        Svgr {
            svg_tree: child_nodes,
        }
    }
}

#[cfg(any(not(feature = "compile-time-svgtree"), target_arch = "wasm32"))]
impl<'a> From<&'a str> for Svgr<'a> {
    fn from(val: &'a str) -> Self {
        Svgr {
            value: val.to_string(),
            marker: std::marker::PhantomData,
        }
    }
}

#[cfg(all(feature = "compile-time-svgtree", not(target_arch = "wasm32")))]
impl<'a> From<&'a str> for Svgr<'a> {
    fn from(val: &'a str) -> Self {
        use usvgr::svgtree::{NestedNodeData, NestedSvgDocument};

        Svgr {
            svg_tree: NestedSvgDocument::from_nodes(vec![
                Some(NestedNodeData {
                    kind: usvgr::svgtree::NestedNodeKind::Text(
                        usvgr::svgtree::roxmltree::StringStorage::Borrowed(val)
                    ),
                    attrs: Box::new([]),
                    children: vec![],
                    static_hash: None,
                });
                1
            ]),
        }
    }
}

impl<'a> From<Vec<Svgr<'a>>> for Svgr<'a> {
    fn from(val: Vec<Svgr<'a>>) -> Svgr<'a> {
        FromIterator::from_iter(val)
    }
}

#[cfg(all(feature = "compile-time-svgtree", not(target_arch = "wasm32")))]
impl fmt::Display for Svgr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.svg_tree)
    }
}

#[cfg(any(not(feature = "compile-time-svgtree"), target_arch = "wasm32"))]
impl fmt::Display for Svgr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svg_validate::ValidationIssueKind;

    fn parse(svgr: Svgr<'_>) -> Result<usvgr::Tree> {
        svgr.into_svg_tree(
            &usvgr::Options::default(),
            &mut usvgr::Cache::default(),
            &usvgr::fontdb::Database::new(),
        )
    }

    fn validation_kind(result: Result<usvgr::Tree>) -> ValidationIssueKind {
        match result {
            Err(FFramesError::ValidationError(issue)) => issue.kind,
            Err(other) => panic!("expected ValidationError, got {other:?}"),
            Ok(_) => panic!("expected ValidationError, got a tree"),
        }
    }

    #[test]
    fn from_svg_string_validates() {
        assert!(
            Svgr::from_svg_string(r#"<svg xmlns="http://www.w3.org/2000/svg"/>"#.to_owned())
                .is_ok()
        );
        assert!(matches!(
            Svgr::from_svg_string(String::new()),
            Err(FFramesError::ValidationError(_))
        ));
        assert!(matches!(
            Svgr::from_svg_string("<html/>".to_owned()),
            Err(FFramesError::ValidationError(_))
        ));
    }

    #[test]
    fn runtime_document_renders() {
        let svgr = Svgr::from_svg_string(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="5" height="5" fill="#f00"/></svg>"##
                .to_owned(),
        )
        .unwrap();
        let tree = parse(svgr).unwrap();
        assert_eq!(tree.root().children().len(), 1);
    }

    #[test]
    fn runtime_document_applies_style_sheet() {
        let svgr = Svgr::from_svg_string(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10">
                <style>.a { fill: rgb(0, 255, 0); }</style>
                <rect class="a" width="5" height="5"/>
            </svg>"#
                .to_owned(),
        )
        .unwrap();
        let tree = parse(svgr).unwrap();
        let Some(usvgr::Node::Path(path)) = tree.root().children().first() else {
            panic!("expected the rect as a path");
        };
        let usvgr::Paint::Color(color) = path.fill().expect("fill from the style sheet").paint()
        else {
            panic!("expected a color fill");
        };
        assert_eq!((color.red, color.green, color.blue), (0, 255, 0));
    }

    #[test]
    fn invalid_runtime_document_reports_validation_issue() {
        assert_eq!(
            validation_kind(parse(Svgr::from("<svg><rect".to_owned()))),
            ValidationIssueKind::MalformedXml
        );
        assert_eq!(
            validation_kind(parse(Svgr::from("<html/>"))),
            ValidationIssueKind::WrongRootElement
        );
        assert_eq!(
            validation_kind(parse(Svgr::from(""))),
            ValidationIssueKind::Empty
        );
    }
}
