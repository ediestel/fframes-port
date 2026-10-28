//! A [`PieceDocument`] as a static `svgr` tree, built once and cached by the renderers.
//!
//! Every path's data is pre-parsed into `StaticPathData` and every node carries a
//! `static_hash`, the same form the `svgr!` macro builds at compile time. The Skia renderer
//! records such a tree once as a picture and replays it under any transform, so a camera
//! move over thousands of pieces costs almost nothing per frame.
//!
//! Pieces are addressable by group: `tree.group("french_banner")` is its own cached subtree,
//! so a group can be moved, lit or faded while the rest stays still.

use crate::document::PieceDocument;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::sync::Arc;
use usvgr::svgtree::{
    AId, Attribute, EId, NestedNodeData, NestedNodeKind, StaticPathData, SvgAttributeValue, svgrtypes,
};

const HASH_SEED: u64 = 0x5b0f_1302_0002;

fn hash<T: Hash>(parts: &[T]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    HASH_SEED.hash(&mut h);
    for p in parts {
        p.hash(&mut h);
    }
    h.finish()
}

fn path_node(d: &str, fill: [u8; 3], key: u64) -> Option<NestedNodeData<'static>> {
    let segments: Vec<svgrtypes::PathSegment> = PathSegments::parse(d)?;
    let segments: &'static [svgrtypes::PathSegment] = Box::leak(segments.into_boxed_slice());
    let data: &'static StaticPathData = Box::leak(Box::new(StaticPathData::new(segments)));
    let color = svgrtypes::Color::new_rgb(fill[0], fill[1], fill[2]);
    Some(NestedNodeData {
        kind: NestedNodeKind::Element { tag_name: EId::Path },
        attrs: Box::new([
            Attribute { name: AId::D, value: SvgAttributeValue::StaticPath(data) },
            Attribute { name: AId::Fill, value: SvgAttributeValue::Color(color) },
        ]),
        children: vec![],
        static_hash: Some(key),
    })
}

struct PathSegments;
impl PathSegments {
    fn parse(d: &str) -> Option<Vec<svgrtypes::PathSegment>> {
        let v: Vec<_> = svgrtypes::PathParser::from(d).collect::<Result<_, _>>().ok()?;
        if v.is_empty() { None } else { Some(v) }
    }
}

fn group_node(children: Vec<Option<NestedNodeData<'static>>>, key: u64) -> NestedNodeData<'static> {
    NestedNodeData {
        kind: NestedNodeKind::Element { tag_name: EId::G },
        attrs: Box::new([]),
        children,
        static_hash: Some(key),
    }
}

/// The document as cached subtrees: the whole picture, and one subtree per group.
pub struct PieceTree {
    all: Arc<Vec<Option<NestedNodeData<'static>>>>,
    groups: HashMap<String, Arc<Vec<Option<NestedNodeData<'static>>>>>,
    /// Pieces that belong to no group, as one subtree.
    rest: Arc<Vec<Option<NestedNodeData<'static>>>>,
}

impl PieceTree {
    /// Builds the tree. `salt` tells pieces of different documents apart in the render cache
    /// (use the document's file name or content hash).
    #[must_use]
    pub fn new(doc: &PieceDocument, salt: &str) -> Self {
        let palette: Vec<[u8; 3]> = doc
            .palette
            .iter()
            .map(|s| {
                let c = svgrtypes::Color::from_str(s).unwrap_or(svgrtypes::Color::black());
                [c.red, c.green, c.blue]
            })
            .collect();
        let base = hash(&[salt.to_string(), doc.width.to_string(), doc.height.to_string()]);
        let mut all = Vec::with_capacity(doc.pieces.len());
        let mut by_group: HashMap<String, Vec<Option<NestedNodeData<'static>>>> = HashMap::new();
        let mut rest = Vec::new();
        for p in &doc.pieces {
            let fill = palette.get(p.color as usize).copied().unwrap_or([0, 0, 0]);
            let key = hash(&[base, u64::from(p.id), u64::from(p.color), p.d.len() as u64]);
            let Some(node) = path_node(&p.d, fill, key) else { continue };
            all.push(Some(node.clone()));
            if p.group.is_empty() {
                rest.push(Some(node));
            } else {
                by_group.entry(p.group.clone()).or_default().push(Some(node));
            }
        }
        let wrap = |nodes: Vec<Option<NestedNodeData<'static>>>, name: &str| {
            Arc::new(vec![Some(group_node(nodes, hash(&[base, hash(&[name])])))])
        };
        let groups = by_group.into_iter().map(|(k, v)| {
            let node = wrap(v, &k);
            (k, node)
        }).collect();
        PieceTree { all: wrap(all, "all"), groups, rest: wrap(rest, "rest") }
    }

    fn svgr(nodes: &Arc<Vec<Option<NestedNodeData<'static>>>>) -> fframes_svgr::Svgr<'static> {
        fframes_svgr::from_nodes(nodes.as_ref().clone())
    }

    /// The whole picture.
    #[must_use]
    pub fn all(&self) -> fframes_svgr::Svgr<'static> {
        Self::svgr(&self.all)
    }

    /// One group, or nothing if the name is unknown.
    #[must_use]
    pub fn group(&self, name: &str) -> fframes_svgr::Svgr<'static> {
        self.groups.get(name).map_or_else(fframes_svgr::empty, Self::svgr)
    }

    /// Everything that is in no group.
    #[must_use]
    pub fn rest(&self) -> fframes_svgr::Svgr<'static> {
        Self::svgr(&self.rest)
    }

    pub fn group_names(&self) -> impl Iterator<Item = &str> {
        self.groups.keys().map(String::as_str)
    }
}

/// The few `Svgr` constructors this crate needs, without depending on `fframes` (which
/// depends on this crate). `fframes::vector` wires them to the real type.
pub mod fframes_svgr {
    use usvgr::svgtree::{NestedNodeData, NestedSvgDocument};

    /// A bare nested document; `fframes` wraps it in `Svgr`.
    pub type Svgr<'a> = NestedSvgDocument<'a>;

    #[must_use]
    pub fn from_nodes(nodes: Vec<Option<NestedNodeData<'static>>>) -> Svgr<'static> {
        NestedSvgDocument::from_nodes(nodes)
    }

    #[must_use]
    pub fn empty() -> Svgr<'static> {
        NestedSvgDocument::from_nodes(vec![])
    }
}
