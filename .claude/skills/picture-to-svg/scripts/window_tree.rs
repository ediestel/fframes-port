//! The traced window as a static `svgr` tree, built once per process.
//!
//! `Svgr::from_svg_string` makes fframes parse the whole 5.6 MB document again for
//! every frame. This module reads the SVG once and builds the same kind of tree the
//! `svgr!` macro builds at compile time:
//! - every path's `d` is parsed once into `StaticPathData`, so converting the tree
//!   for a frame parses no path strings;
//! - every node carries a `static_hash`, so the Skia renderer records the window
//!   once as a picture and replays it, at any camera transform, without redrawing
//!   7,000 paths.
//! A frame embeds the result with `{window_tree::window()}` inside `svgr!`.

use fframes::Svgr;
use fframes::roxmltree;
use fframes::usvgr::svgtree::{
    AId, Attribute, EId, NestedNodeData, NestedNodeKind, NestedSvgDocument, StaticPathData,
    StringStorage, SvgAttributeValue, svgrtypes,
};
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::sync::OnceLock;

/// Bumped when the meaning of the generated tree changes, so stale cache keys never match.
const HASH_SEED: u64 = 0x601d_5b0f_1302_0001;

fn hash_of(parts: &[&dyn Fn(&mut std::collections::hash_map::DefaultHasher)]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    HASH_SEED.hash(&mut h);
    for p in parts {
        p(&mut h);
    }
    h.finish()
}

fn text_attr(name: AId, value: &str) -> Attribute<'static> {
    Attribute {
        name,
        value: SvgAttributeValue::StringStorage(StringStorage::new_owned(value.to_string())),
    }
}

/// Converts one attribute of the traced SVG into its pre-parsed form where usvgr would
/// otherwise parse text on every frame.
fn convert_attr(name: AId, value: &str) -> Attribute<'static> {
    match name {
        AId::D => {
            let segments: Option<Vec<svgrtypes::PathSegment>> =
                svgrtypes::PathParser::from(value).collect::<Result<_, _>>().ok();
            match segments {
                Some(s) if !s.is_empty() => {
                    let segments: &'static [svgrtypes::PathSegment] = Box::leak(s.into_boxed_slice());
                    let data: &'static StaticPathData = Box::leak(Box::new(StaticPathData::new(segments)));
                    Attribute { name, value: SvgAttributeValue::StaticPath(data) }
                }
                _ => text_attr(name, value),
            }
        }
        AId::Transform => match svgrtypes::Transform::from_str(value) {
            Ok(t) => Attribute { name, value: SvgAttributeValue::Transform(t) },
            Err(_) => text_attr(name, value),
        },
        AId::Fill | AId::Stroke => match svgrtypes::Color::from_str(value) {
            Ok(c) => Attribute { name, value: SvgAttributeValue::Color(c) },
            Err(_) => text_attr(name, value),
        },
        _ => text_attr(name, value),
    }
}

fn convert_element(node: roxmltree::Node, path: &mut Vec<usize>) -> Option<NestedNodeData<'static>> {
    let tag = EId::from_str(node.tag_name().name())?;
    // The seam strokes trace.sh puts on each colour layer are 1 unit under the layer's
    // scale(0.1), i.e. 0.1 px: invisible, but usvgr strokes every path on every frame to
    // measure its bounds, which was most of the render time. They are left out.
    let attrs: Vec<Attribute<'static>> = node
        .attributes()
        .filter_map(|a| AId::from_str(a.name()).map(|aid| (aid, a.value())))
        .filter(|(aid, _)| !matches!(aid, AId::Stroke | AId::StrokeWidth | AId::StrokeLinejoin))
        .map(|(aid, v)| convert_attr(aid, v))
        .collect();
    let mut children = Vec::new();
    for (i, child) in node.children().filter(|c| c.is_element()).enumerate() {
        path.push(i);
        if let Some(c) = convert_element(child, path) {
            children.push(Some(c));
        }
        path.pop();
    }
    // a stable key per node: its position in the document and its own attributes
    let pos = path.clone();
    let own: Vec<(String, String)> = node
        .attributes()
        .map(|a| (a.name().to_string(), if a.name() == "d" { a.value().len().to_string() } else { a.value().to_string() }))
        .collect();
    let static_hash = hash_of(&[&|h| pos.hash(h), &|h| tag.to_str().hash(h), &|h| own.hash(h)]);
    Some(NestedNodeData {
        kind: NestedNodeKind::Element { tag_name: tag },
        attrs: attrs.into_boxed_slice(),
        children,
        static_hash: Some(static_hash),
    })
}

/// The window's content (every layer of the traced SVG) as one static group.
pub fn window() -> Svgr<'static> {
    static TREE: OnceLock<Vec<Option<NestedNodeData<'static>>>> = OnceLock::new();
    let nodes = TREE.get_or_init(|| {
        let text = super::film::TRACED;
        let doc = match roxmltree::Document::parse_with_options(
            text,
            roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() },
        ) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("window_tree: {e}");
                return Vec::new();
            }
        };
        let root = doc.root_element();
        let mut layers = Vec::new();
        let mut path = vec![0usize];
        for (i, child) in root.children().filter(|c| c.is_element()).enumerate() {
            path.push(i);
            if let Some(n) = convert_element(child, &mut path) {
                layers.push(Some(n));
            }
            path.pop();
        }
        let len = text.len();
        let group = NestedNodeData {
            kind: NestedNodeKind::Element { tag_name: EId::G },
            attrs: Box::new([]),
            children: layers,
            static_hash: Some(hash_of(&[&|h| "golden-spurs-window".hash(h), &|h| len.hash(h)])),
        };
        vec![Some(group)]
    });
    Svgr { svg_tree: NestedSvgDocument::from_nodes(nodes.clone()) }
}
