//! What trained Gaussians say about the surface they sit on, ported from the kind layer of
//! the 3DGS-PLY-3DTiles converter (`src/kind`: geom3, signature, remainder, residual,
//! freeform, splatnative).
//!
//! - A splat's own shape ([`shape`]): the axes of its covariance. A flat splat is a disc in
//!   the surface; its shortest axis is the surface normal.
//! - Its neighbourhood ([`neighbourhoods`]): the covariance of the splat centres around it
//!   (the structure tensor) in cubic windows of growing size. The size whose 1D / 2D / 3D
//!   shares are most decided (lowest entropy, Demantke) says whether the splats there form a
//!   surface, a thin line or a volume; a splat with hardly any neighbours even in the largest
//!   window is a floater. The smallest axis is the local normal, the smallest share of the
//!   spread the surface variation (a measure of curvature).
//! - [`components`]: connected pieces of splats, so spray detached from the figure can be
//!   dropped. [`patches`]: regions grown from smooth seeds that take a splat only while its
//!   normal stays near the seed's (not its neighbour's), so a smooth continuum cannot run
//!   together into one region; creases become borders.
//! - [`write_subset`]: some splats of a `.ply`, copied byte for byte.

use crate::splat::Splat;
use rayon::prelude::*;
use std::collections::HashMap;

/// Eigenvalues of a symmetric 3x3 matrix, largest first (analytic, Smith 1961).
#[must_use]
pub fn eigenvalues(c: &[[f64; 3]; 3]) -> [f64; 3] {
    let (xx, yy, zz, xy, xz, yz) = (c[0][0], c[1][1], c[2][2], c[0][1], c[0][2], c[1][2]);
    let p1 = xy * xy + xz * xz + yz * yz;
    let q = (xx + yy + zz) / 3.0;
    if p1 < 1e-30 {
        let mut e = [xx, yy, zz];
        e.sort_by(|a, b| b.total_cmp(a));
        return e;
    }
    let p = (((xx - q).powi(2) + (yy - q).powi(2) + (zz - q).powi(2) + 2.0 * p1) / 6.0).sqrt();
    if p < 1e-30 {
        return [q; 3];
    }
    let (b11, b22, b33, b12, b13, b23) = ((xx - q) / p, (yy - q) / p, (zz - q) / p, xy / p, xz / p, yz / p);
    let det = b11 * (b22 * b33 - b23 * b23) - b12 * (b12 * b33 - b23 * b13) + b13 * (b12 * b23 - b22 * b13);
    let phi = (det / 2.0).clamp(-1.0, 1.0).acos() / 3.0;
    let e1 = q + 2.0 * p * phi.cos();
    let e3 = q + 2.0 * p * (phi + 2.0 * std::f64::consts::PI / 3.0).cos();
    [e1, 3.0 * q - e1 - e3, e3]
}

/// Unit eigenvector of a symmetric 3x3 matrix for eigenvalue `lambda`: the longest cross
/// product of two rows of `c - lambda I`.
#[must_use]
pub fn eigenvector(c: &[[f64; 3]; 3], lambda: f64) -> [f64; 3] {
    let r: [[f64; 3]; 3] = std::array::from_fn(|i| std::array::from_fn(|j| c[i][j] - if i == j { lambda } else { 0.0 }));
    let cross = |u: [f64; 3], v: [f64; 3]| [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
    let best = [cross(r[0], r[1]), cross(r[0], r[2]), cross(r[1], r[2])]
        .into_iter()
        .max_by(|a, b| a.iter().map(|v| v * v).sum::<f64>().total_cmp(&b.iter().map(|v| v * v).sum::<f64>()))
        .unwrap_or([0.0, 0.0, 1.0]);
    let len = best.iter().map(|v| v * v).sum::<f64>().sqrt();
    if len < 1e-10 { [0.0, 0.0, 1.0] } else { best.map(|v| v / len) }
}

/// A splat's own shape.
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    /// The shortest axis: the surface normal of a flat splat (sign undecided).
    pub normal: [f32; 3],
    /// (middle - shortest) / longest axis: 1 for a disc.
    pub planarity: f32,
    /// (longest - middle) / longest axis: 1 for a needle.
    pub linearity: f32,
}

/// The shape of a splat from its covariance.
#[must_use]
pub fn shape(splat: &Splat) -> Shape {
    let c = splat.cov.map(|r| r.map(f64::from));
    let e = eigenvalues(&c);
    let [a, b, s] = e.map(|v| v.max(0.0).sqrt());
    let a = a.max(1e-12);
    Shape {
        normal: eigenvector(&c, e[2]).map(|v| v as f32),
        planarity: ((b - s) / a) as f32,
        linearity: ((a - b) / a) as f32,
    }
}

/// What the splats around one splat form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Local {
    Surface,
    Thin,
    Volume,
    /// Hardly any neighbours even in the largest window.
    Floater,
}

/// One splat's neighbourhood at its most decided window size.
#[derive(Clone, Copy, Debug)]
pub struct Neighbourhood {
    pub local: Local,
    /// The structure tensor's smallest axis (sign undecided); zero for a floater.
    pub normal: [f32; 3],
    /// Smallest eigenvalue over the trace: 0 on a plane, 1/3 in an even cloud.
    pub variation: f32,
    /// Splats in the chosen window.
    pub neighbours: usize,
}

/// Window sizes and limits of [`neighbourhoods`].
#[derive(Clone, Copy, Debug)]
pub struct NeighbourhoodParams {
    /// Largest window: this many voxels on each side.
    pub max_ring: i32,
    /// Splats a window needs before its shape counts.
    pub min_valid: usize,
    /// Fewer splats than this in the largest window: a floater.
    pub floater: usize,
}

impl Default for NeighbourhoodParams {
    fn default() -> Self {
        NeighbourhoodParams { max_ring: 5, min_valid: 12, floater: 6 }
    }
}

/// Voxel edge for `cells` voxels across the points' extent, the extent measured between
/// the 1 % and 99 % positions on each axis so outlying splats do not make it coarse.
#[must_use]
pub fn robust_voxel(points: &[[f32; 3]], cells: usize) -> f32 {
    let extent = (0..3)
        .map(|c| {
            let mut v: Vec<f32> = points.iter().map(|p| p[c]).collect();
            v.sort_by(f32::total_cmp);
            if v.is_empty() { 0.0 } else { v[(v.len() - 1) * 99 / 100] - v[(v.len() - 1) / 100] }
        })
        .fold(1e-6_f32, f32::max);
    extent / cells.max(1) as f32
}

/// Median distance from a point to its nearest neighbour: the scale windows are measured in,
/// so they hold as many splats in a sparse model as in a dense one.
#[must_use]
pub fn spacing(points: &[[f32; 3]]) -> f32 {
    // nearest neighbours searched in the 27 cells around; cells grow until most points find one
    let mut voxel = robust_voxel(points, 512);
    loop {
        let grid = Grid::new(points, voxel);
        let mut d: Vec<f32> = points
            .par_iter()
            .enumerate()
            .filter_map(|(i, p)| {
                let c = Grid::key(p, voxel);
                let mut best = f32::MAX;
                for dz in -1..=1 {
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            for &j in grid.cell([c[0] + dx, c[1] + dy, c[2] + dz]) {
                                if j != i {
                                    best = best.min((0..3).map(|a| (points[j][a] - p[a]).powi(2)).sum::<f32>());
                                }
                            }
                        }
                    }
                }
                // farther than one cell may hide a nearer point in a cell not searched
                (best <= voxel * voxel).then(|| best.sqrt())
            })
            .collect();
        if d.len() * 2 > points.len() || voxel > 1e6 {
            d.sort_by(f32::total_cmp);
            return d.get(d.len() / 2).copied().unwrap_or(voxel);
        }
        voxel *= 2.0;
    }
}

/// Points binned in a sparse voxel grid.
struct Grid {
    voxel: f32,
    cells: HashMap<[i32; 3], Vec<usize>>,
}

impl Grid {
    fn new(points: &[[f32; 3]], voxel: f32) -> Self {
        let mut cells: HashMap<[i32; 3], Vec<usize>> = HashMap::new();
        for (i, p) in points.iter().enumerate() {
            cells.entry(Self::key(p, voxel)).or_default().push(i);
        }
        Grid { voxel, cells }
    }

    fn key(p: &[f32; 3], voxel: f32) -> [i32; 3] {
        p.map(|v| (v / voxel).floor() as i32)
    }

    fn cell(&self, k: [i32; 3]) -> &[usize] {
        self.cells.get(&k).map_or(&[], Vec::as_slice)
    }
}

/// Sums of a point set's positions and their products, relative to an origin.
#[derive(Clone, Copy, Default)]
struct Moments {
    k: usize,
    s: [f64; 3],
    ss: [[f64; 3]; 3],
}

impl Moments {
    fn add(&mut self, d: [f64; 3]) {
        self.k += 1;
        for a in 0..3 {
            self.s[a] += d[a];
            for b in 0..3 {
                self.ss[a][b] += d[a] * d[b];
            }
        }
    }

    fn covariance(&self) -> [[f64; 3]; 3] {
        let k = self.k.max(1) as f64;
        std::array::from_fn(|a| std::array::from_fn(|b| self.ss[a][b] / k - self.s[a] / k * self.s[b] / k))
    }

    /// 1D, 2D and 3D shares of the spread (they sum to 1).
    fn dimensions(&self) -> [f64; 3] {
        let [s0, s1, s2] = eigenvalues(&self.covariance()).map(|v| v.max(0.0).sqrt());
        let d = s0.max(1e-12);
        [(s0 - s1) / d, (s1 - s2) / d, s2 / d]
    }
}

/// Every splat's neighbourhood: windows of 1 to `max_ring` voxels on each side, the one
/// with the lowest entropy of its 1D / 2D / 3D shares chosen among those holding at least
/// `min_valid` splats (the largest window when none does).
#[must_use]
pub fn neighbourhoods(points: &[[f32; 3]], voxel: f32, p: &NeighbourhoodParams) -> Vec<Neighbourhood> {
    let grid = Grid::new(points, voxel);
    let entropy = |a: [f64; 3]| -a.iter().map(|&v| if v > 1e-12 { v * v.ln() } else { 0.0 }).sum::<f64>();
    points
        .par_iter()
        .map(|o| {
            let c = Grid::key(o, grid.voxel);
            let mut m = Moments::default();
            let mut best: Option<(f64, Moments)> = None;
            for r in 0..=p.max_ring {
                // the shell of the window at ring r
                for dz in -r..=r {
                    for dy in -r..=r {
                        for dx in -r..=r {
                            if dx.abs() != r && dy.abs() != r && dz.abs() != r {
                                continue;
                            }
                            for &j in grid.cell([c[0] + dx, c[1] + dy, c[2] + dz]) {
                                m.add(std::array::from_fn(|a| f64::from(points[j][a] - o[a])));
                            }
                        }
                    }
                }
                if r >= 1 && m.k >= p.min_valid.max(4) {
                    let a = m.dimensions();
                    // a window without spread (all splats in one spot) says nothing
                    if a.iter().sum::<f64>() > 0.5 {
                        let e = entropy(a);
                        if best.is_none_or(|(b, _)| e < b) {
                            best = Some((e, m));
                        }
                    }
                }
            }
            if m.k < p.floater {
                return Neighbourhood { local: Local::Floater, normal: [0.0; 3], variation: 1.0, neighbours: m.k };
            }
            let m = best.map_or(m, |(_, b)| b);
            let a = m.dimensions();
            let local = if a[1] >= a[0] && a[1] >= a[2] {
                Local::Surface
            } else if a[0] >= a[2] {
                Local::Thin
            } else {
                Local::Volume
            };
            let cov = m.covariance();
            let e = eigenvalues(&cov);
            let trace = e.iter().sum::<f64>();
            Neighbourhood {
                local,
                normal: eigenvector(&cov, e[2]).map(|v| v as f32),
                variation: if trace > 1e-20 { (e[2] / trace) as f32 } else { 0.0 },
                neighbours: m.k,
            }
        })
        .collect()
}

/// Connected pieces: points in the same or neighbouring voxels (edge `voxel`) belong
/// together. Returns each point's piece, pieces numbered by size (0 the largest), and the
/// pieces' sizes.
#[must_use]
pub fn components(points: &[[f32; 3]], voxel: f32) -> (Vec<usize>, Vec<usize>) {
    let grid = Grid::new(points, voxel);
    // union-find over the occupied voxels: points in one voxel are one piece already
    let keys: Vec<[i32; 3]> = grid.cells.keys().copied().collect();
    let index: HashMap<[i32; 3], usize> = keys.iter().enumerate().map(|(i, k)| (*k, i)).collect();
    let mut parent: Vec<usize> = (0..keys.len()).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    for (i, k) in keys.iter().enumerate() {
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if let Some(&j) = index.get(&[k[0] + dx, k[1] + dy, k[2] + dz]) {
                        let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                        if a != b {
                            parent[a] = b;
                        }
                    }
                }
            }
        }
    }
    let root: Vec<usize> = points.iter().map(|p| find(&mut parent, index[&Grid::key(p, voxel)])).collect();
    let mut size: HashMap<usize, usize> = HashMap::new();
    for &r in &root {
        *size.entry(r).or_default() += 1;
    }
    let mut order: Vec<(usize, usize)> = size.into_iter().collect();
    order.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let rank: HashMap<usize, usize> = order.iter().enumerate().map(|(i, (r, _))| (*r, i)).collect();
    (root.iter().map(|r| rank[r]).collect(), order.iter().map(|o| o.1).collect())
}

/// Limits of [`patches`].
#[derive(Clone, Copy, Debug)]
pub struct PatchParams {
    /// A splat joins while its normal is within this many degrees of the seed's.
    pub normal_deg: f32,
    /// Seeds: surface variation at most this.
    pub seed_variation: f32,
    /// Members: surface variation at most this (creases are not crossed).
    pub grow_variation: f32,
    /// Seeds need at least this many neighbours.
    pub min_neighbours: usize,
    /// A patch stops growing at this many splats.
    pub max_patch: usize,
    /// Smaller patches are dropped.
    pub min_patch: usize,
}

impl Default for PatchParams {
    fn default() -> Self {
        PatchParams { normal_deg: 24.0, seed_variation: 0.08, grow_variation: 0.13, min_neighbours: 10, max_patch: 12000, min_patch: 400 }
    }
}

/// Patches grown from the smoothest splats first: a splat joins a patch when it lies within
/// 1.6 voxels of a member, its normal within `normal_deg` of the seed's, and its surface
/// variation is low. A patch's normals therefore span at most twice the tolerance. Patches
/// largest first, as point indices.
#[must_use]
pub fn patches(points: &[[f32; 3]], hood: &[Neighbourhood], voxel: f32, p: &PatchParams) -> Vec<Vec<usize>> {
    let grid = Grid::new(points, voxel);
    let cos_tol = p.normal_deg.to_radians().cos();
    let reach2 = (voxel * 1.6).powi(2);
    let mut order: Vec<usize> = (0..points.len()).collect();
    order.sort_by(|&a, &b| hood[a].variation.total_cmp(&hood[b].variation));
    let mut taken = vec![false; points.len()];
    let mut out = Vec::new();
    for seed in order {
        let h = &hood[seed];
        if taken[seed] || h.local == Local::Floater || h.neighbours < p.min_neighbours || h.variation > p.seed_variation {
            continue;
        }
        let sn = h.normal;
        taken[seed] = true;
        let mut members = vec![seed];
        let mut at = 0;
        while at < members.len() && members.len() < p.max_patch {
            let cur = members[at];
            at += 1;
            // 1.6 voxels can reach two cells over
            let c = Grid::key(&points[cur], voxel);
            'cells: for dz in -2..=2 {
                for dy in -2..=2 {
                    for dx in -2..=2 {
                        for &j in grid.cell([c[0] + dx, c[1] + dy, c[2] + dz]) {
                            if taken[j] || hood[j].local == Local::Floater || hood[j].variation > p.grow_variation {
                                continue;
                            }
                            let d2: f32 = (0..3).map(|a| (points[j][a] - points[cur][a]).powi(2)).sum();
                            // normal signs are undecided
                            let dot = (0..3).map(|a| hood[j].normal[a] * sn[a]).sum::<f32>().abs();
                            if d2 > reach2 || dot < cos_tol {
                                continue;
                            }
                            taken[j] = true;
                            members.push(j);
                            if members.len() >= p.max_patch {
                                break 'cells;
                            }
                        }
                    }
                }
            }
        }
        if members.len() >= p.min_patch {
            out.push(members);
        }
    }
    out.sort_by(|a, b| b.len().cmp(&a.len()));
    out
}

/// Writes the vertices `keep` of the binary little-endian `.ply` at `src` to `out`, byte for
/// byte with the same properties: nothing about a splat is read or changed. Returns how many
/// were written.
pub fn write_subset(src: &std::path::Path, keep: &[usize], out: &std::path::Path) -> Result<usize, String> {
    let data = std::fs::read(src).map_err(|e| format!("{}: {e}", src.display()))?;
    let end = data.windows(11).position(|w| w == b"end_header\n").ok_or("no ply header")? + 11;
    let head = String::from_utf8_lossy(&data[..end]).into_owned();
    if !head.lines().any(|l| l.starts_with("format binary_little_endian")) {
        return Err("only binary little-endian ply is copied".into());
    }
    let (mut count, mut stride, mut in_vertex) = (None, 0, false);
    for l in head.lines() {
        let w: Vec<&str> = l.split_whitespace().collect();
        match w.as_slice() {
            ["element", name, n] => {
                in_vertex = *name == "vertex";
                if in_vertex {
                    count = n.parse::<usize>().ok();
                }
            }
            ["property", "list", ..] if in_vertex => return Err("list properties are not copied".into()),
            ["property", t, _] if in_vertex => {
                stride += match *t {
                    "char" | "uchar" | "int8" | "uint8" => 1,
                    "short" | "ushort" | "int16" | "uint16" => 2,
                    "int" | "uint" | "int32" | "uint32" | "float" | "float32" => 4,
                    "double" | "float64" => 8,
                    _ => return Err(format!("unknown ply type {t}")),
                }
            }
            _ => {}
        }
    }
    let count = count.ok_or("no vertex element")?;
    if data.len() < end + count * stride {
        return Err("ply shorter than its header says".into());
    }
    let new_head: String = head
        .lines()
        .map(|l| if l.starts_with("element vertex ") { format!("element vertex {}\n", keep.len()) } else { format!("{l}\n") })
        .collect();
    let mut bytes = new_head.into_bytes();
    bytes.reserve(keep.len() * stride);
    for &i in keep {
        if i >= count {
            return Err(format!("vertex {i} out of range"));
        }
        bytes.extend_from_slice(&data[end + i * stride..end + (i + 1) * stride]);
    }
    std::fs::write(out, bytes).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(keep.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plane(n: usize, z: f32, step: f32) -> Vec<[f32; 3]> {
        (0..n * n).map(|i| [(i % n) as f32 * step, (i / n) as f32 * step, z]).collect()
    }

    #[test]
    fn eigen_of_a_known_matrix() {
        let c = [[2.0, 1.0, 0.0], [1.0, 2.0, 0.0], [0.0, 0.0, 5.0]];
        let e = eigenvalues(&c);
        assert!((e[0] - 5.0).abs() < 1e-9 && (e[1] - 3.0).abs() < 1e-9 && (e[2] - 1.0).abs() < 1e-9);
        let v = eigenvector(&c, e[2]);
        assert!((v[0] + v[1]).abs() < 1e-9 && v[2].abs() < 1e-9);
    }

    #[test]
    fn a_disc_splat_has_its_normal_on_the_short_axis() {
        let s = Splat { centre: [0.0; 3], cov: [[0.04, 0.0, 0.0], [0.0, 0.0001, 0.0], [0.0, 0.0, 0.03]], opacity: 1.0, sh: [[0.0; 16]; 3] };
        let sh = shape(&s);
        assert!(sh.normal[1].abs() > 0.999);
        assert!(sh.planarity > 0.7 && sh.linearity < 0.2);
    }

    #[test]
    fn a_plane_is_a_surface_and_a_lone_splat_a_floater() {
        let mut pts = plane(40, 0.0, 0.01);
        pts.push([5.0, 5.0, 5.0]);
        let h = neighbourhoods(&pts, 0.01, &NeighbourhoodParams::default());
        let mid = &h[20 * 40 + 20];
        assert_eq!(mid.local, Local::Surface);
        assert!(mid.normal[2].abs() > 0.99 && mid.variation < 0.01);
        assert_eq!(h[pts.len() - 1].local, Local::Floater);
    }

    #[test]
    fn spacing_is_the_lattice_step() {
        let s = spacing(&plane(30, 0.0, 0.01));
        assert!((s - 0.01).abs() < 1e-4, "{s}");
    }

    #[test]
    fn detached_spray_is_its_own_piece() {
        let mut pts = plane(20, 0.0, 0.01);
        pts.extend(plane(3, 1.0, 0.01));
        let (piece, sizes) = components(&pts, 0.02);
        assert_eq!(sizes, vec![400, 9]);
        assert_eq!(piece[0], 0);
        assert_eq!(piece[pts.len() - 1], 1);
    }

    #[test]
    fn a_fold_becomes_two_patches() {
        // two perpendicular planes meeting along x
        let n = 40;
        let mut pts: Vec<[f32; 3]> = (0..n * n).map(|i| [(i % n) as f32 * 0.01, (i / n) as f32 * 0.01 + 0.01, 0.0]).collect();
        pts.extend((0..n * n).map(|i| [(i % n) as f32 * 0.01, 0.0, (i / n) as f32 * 0.01 + 0.01]));
        let h = neighbourhoods(&pts, 0.01, &NeighbourhoodParams { max_ring: 2, ..Default::default() });
        let p = patches(&pts, &h, 0.01, &PatchParams { min_patch: 200, ..Default::default() });
        assert_eq!(p.len(), 2);
        for patch in &p {
            let on_floor = patch.iter().filter(|&&i| i < n * n).count();
            assert!(on_floor == 0 || on_floor == patch.len());
        }
    }

    #[test]
    fn a_subset_is_copied_byte_for_byte() {
        let dir = std::env::temp_dir().join(format!("splatshape_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut ply = b"ply\nformat binary_little_endian 1.0\nelement vertex 3\nproperty float x\nproperty uchar r\nend_header\n".to_vec();
        for (x, r) in [(1.0_f32, 10_u8), (2.0, 20), (3.0, 30)] {
            ply.extend_from_slice(&x.to_le_bytes());
            ply.push(r);
        }
        let (src, out) = (dir.join("a.ply"), dir.join("b.ply"));
        std::fs::write(&src, &ply).unwrap();
        assert_eq!(write_subset(&src, &[2, 0], &out).unwrap(), 2);
        let got = std::fs::read(&out).unwrap();
        let head = b"ply\nformat binary_little_endian 1.0\nelement vertex 2\nproperty float x\nproperty uchar r\nend_header\n";
        assert_eq!(&got[..head.len()], head);
        assert_eq!(&got[head.len()..], &[&3.0_f32.to_le_bytes()[..], &[30], &1.0_f32.to_le_bytes()[..], &[10]].concat()[..]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
