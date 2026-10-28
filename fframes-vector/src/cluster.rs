//! Automatic grouping without hints: pieces that touch across the outline and look alike
//! merge into clusters. This is the graph segmentation of Felzenszwalb and Huttenlocher, run
//! on the piece adjacency graph instead of on pixels: an edge joins two pieces that face
//! each other across at most `max_gap` of outline, weighted by their colour difference
//! (CIELAB ΔE), and two clusters merge while that difference is no larger than the
//! difference already inside each of them plus `k / area`.
//!
//! Clusters are not objects (a horse of brown and white glass stays two clusters); they are
//! the units that coarse hints snap to (`group --clusters`) and a first unnamed split of a
//! picture to look at and name.

use crate::document::PieceDocument;
use crate::quantize::lab;
use crate::raster::PieceRaster;
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Clone, Copy, Debug)]
pub struct ClusterParams {
    /// Merge eagerness: a cluster of `area` source pixels² accepts neighbours up to
    /// `k / area` ΔE further from it than its own inner difference. Higher = bigger clusters.
    pub k: f32,
    /// Widest outline two pieces may lie apart and still touch, in source-picture pixels.
    pub max_gap: f32,
    /// Shortest shared border that counts as touching, in source pixels (corners don't).
    pub min_contact: f32,
    /// Clusters smaller than this (source pixels²) join the neighbour they share the most
    /// border with.
    pub min_area: f32,
}

impl Default for ClusterParams {
    fn default() -> Self {
        ClusterParams {
            k: 3000.0,
            max_gap: 6.0,
            min_contact: 3.0,
            min_area: 400.0,
        }
    }
}

/// Shared border length in raster pixels between pieces that touch across at most
/// `max_gap` raster pixels of outline or background, keyed by piece indices `(a, b)`,
/// `a < b`. Outline pieces (palette 0) are the gaps, never nodes. Rows and columns are
/// scanned, so a border is counted once per crossing line.
#[must_use]
pub fn adjacency(
    doc: &PieceDocument,
    raster: &PieceRaster,
    max_gap: u32,
) -> HashMap<(u32, u32), u32> {
    let is_node: Vec<bool> = doc.pieces.iter().map(|p| p.color != 0).collect();
    let (w, h) = (raster.width as usize, raster.height as usize);
    let mut edges: HashMap<(u32, u32), u32> = HashMap::new();
    let mut scan = |at: &mut dyn Iterator<Item = usize>| {
        let (mut last, mut gap) = (None::<u32>, 0u32);
        for i in at {
            let l = raster.labels[i];
            if l == 0 || !is_node[l as usize - 1] {
                gap += 1;
                continue;
            }
            let p = l - 1;
            if let Some(q) = last
                && q != p
                && gap <= max_gap
            {
                *edges.entry((q.min(p), q.max(p))).or_insert(0) += 1;
            }
            last = Some(p);
            gap = 0;
        }
    };
    for y in 0..h {
        scan(&mut (y * w..(y + 1) * w));
    }
    for x in 0..w {
        scan(&mut (0..h).map(|y| y * w + x));
    }
    edges
}

struct Sets {
    parent: Vec<u32>,
    area: Vec<f32>,
    inner: Vec<f32>,
}

impl Sets {
    fn find(&mut self, mut a: u32) -> u32 {
        while self.parent[a as usize] != a {
            let up = self.parent[self.parent[a as usize] as usize];
            self.parent[a as usize] = up;
            a = up;
        }
        a
    }
    fn union(&mut self, a: u32, b: u32, w: f32) -> u32 {
        let (big, small) = if self.area[a as usize] >= self.area[b as usize] {
            (a, b)
        } else {
            (b, a)
        };
        self.parent[small as usize] = big;
        self.area[big as usize] += self.area[small as usize];
        self.inner[big as usize] = self.inner[big as usize]
            .max(self.inner[small as usize])
            .max(w);
        big
    }
}

/// The cluster of every piece, numbered by area (0 = largest); `None` for outline pieces
/// and pieces that are not visible in the raster.
#[must_use]
pub fn clusters(doc: &PieceDocument, raster: &PieceRaster, p: &ClusterParams) -> Vec<Option<u32>> {
    let n = doc.pieces.len();
    // source pixels per raster pixel
    let src = 1.0 / (raster.res * doc.scale.max(1) as f32);
    let px_area = raster.areas(n);
    let area: Vec<f32> = px_area.iter().map(|&a| a as f32 * src * src).collect();
    let gap = (p.max_gap / src).ceil() as u32;
    let contact = (p.min_contact / src).max(1.0);
    let colors: Vec<[f64; 3]> = doc
        .palette
        .iter()
        .map(|s| {
            let c = usvgr::svgtree::svgrtypes::Color::from_str(s)
                .unwrap_or_else(|_| usvgr::svgtree::svgrtypes::Color::black());
            lab([c.red, c.green, c.blue])
        })
        .collect();
    let delta = |a: usize, b: usize| {
        let (x, y) = (
            colors[doc.pieces[a].color as usize],
            colors[doc.pieces[b].color as usize],
        );
        ((x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2) + (x[2] - y[2]).powi(2)).sqrt() as f32
    };
    let adj = adjacency(doc, raster, gap);
    let mut edges: Vec<(f32, u32, u32, u32)> = adj
        .iter()
        .map(|(&(a, b), &len)| (delta(a as usize, b as usize), a, b, len))
        .collect();
    edges.sort_by(|x, y| x.0.total_cmp(&y.0).then((x.1, x.2).cmp(&(y.1, y.2))));

    let mut sets = Sets {
        parent: (0..n as u32).collect(),
        area: area.clone(),
        inner: vec![0.0; n],
    };
    for &(w, a, b, len) in &edges {
        if (len as f32) < contact {
            continue;
        }
        let (ra, rb) = (sets.find(a), sets.find(b));
        if ra == rb {
            continue;
        }
        let ta = sets.inner[ra as usize] + p.k / sets.area[ra as usize].max(1.0);
        let tb = sets.inner[rb as usize] + p.k / sets.area[rb as usize].max(1.0);
        if w <= ta.min(tb) {
            sets.union(ra, rb, w);
        }
    }

    // small clusters join the neighbour with the longest shared border
    let mut by_len = edges.clone();
    by_len.sort_by(|x, y| y.3.cmp(&x.3).then((x.1, x.2).cmp(&(y.1, y.2))));
    for _ in 0..4 {
        let mut changed = false;
        for &(w, a, b, _) in &by_len {
            let (ra, rb) = (sets.find(a), sets.find(b));
            if ra != rb && sets.area[ra as usize].min(sets.area[rb as usize]) < p.min_area {
                sets.union(ra, rb, w);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    // number roots by descending area
    let visible = |i: usize| doc.pieces[i].color != 0 && px_area[i] > 0;
    let mut roots: Vec<u32> = (0..n)
        .filter(|&i| visible(i))
        .map(|i| sets.find(i as u32))
        .collect();
    roots.sort_unstable();
    roots.dedup();
    roots.sort_by(|a, b| {
        sets.area[*b as usize]
            .total_cmp(&sets.area[*a as usize])
            .then(a.cmp(b))
    });
    let number: HashMap<u32, u32> = roots
        .iter()
        .enumerate()
        .map(|(i, &r)| (r, i as u32))
        .collect();
    (0..n)
        .map(|i| visible(i).then(|| number[&sets.find(i as u32)]))
        .collect()
}
