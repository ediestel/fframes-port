//! Drawing input: scans and photos of drawings made ready for flattening.
//!
//! - [`clean_scan`]: uneven paper becomes even white. The paper's brightness is estimated
//!   with a large morphological closing (lines are thin and dark, so a maximum filter wider
//!   than any line sees only paper), every pixel is divided by it, and what is then close to
//!   white becomes pure white. Lines and colours keep their contrast.
//! - [`bridge_gaps`]: drawn outlines rarely close. Every line end (an end of the line
//!   mask's skeleton) is joined to the nearest line pixel it points at within `max_gap`, with
//!   a bridge as wide as the line, so each area between lines becomes its own piece instead
//!   of leaking into its neighbour through the gap.

use image::RgbImage;

/// Scan clean-up settings, in pixels of the image being cleaned.
#[derive(Clone, Copy, Debug)]
pub struct ScanParams {
    /// Radius of the paper estimate: wider than the widest line or solid dark area.
    pub paper_radius: usize,
    /// After levelling, pixels at least this bright (0..1, all channels) become pure white.
    pub white: f32,
}

impl ScanParams {
    /// For a scan at `scale` times its original resolution.
    #[must_use]
    pub fn new(scale: u32) -> Self {
        ScanParams {
            paper_radius: 24 * scale.max(1) as usize,
            white: 0.86,
        }
    }
}

/// Maximum (`max = true`) or minimum filter with a square of radius `r`, separable.
fn filter(v: &[u8], w: usize, h: usize, r: usize, max: bool) -> Vec<u8> {
    let pick = |a: u8, b: u8| if max { a.max(b) } else { a.min(b) };
    let mut tmp = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            let row = &v[y * w..(y + 1) * w];
            tmp[y * w + x] = row[x.saturating_sub(r)..=(x + r).min(w - 1)]
                .iter()
                .copied()
                .reduce(pick)
                .unwrap_or(0);
        }
    }
    let mut out = vec![0u8; w * h];
    for x in 0..w {
        for y in 0..h {
            out[y * w + x] = (y.saturating_sub(r)..=(y + r).min(h - 1))
                .map(|yy| tmp[yy * w + x])
                .reduce(pick)
                .unwrap_or(0);
        }
    }
    out
}

/// The picture with even white paper (see the module docs).
#[must_use]
pub fn clean_scan(img: &RgbImage, p: &ScanParams) -> RgbImage {
    let (w, h) = (img.width() as usize, img.height() as usize);
    // estimate the paper per channel (so tinted paper also becomes neutral white) on 4x
    // smaller maps: fast, and paper changes slowly. Each block gives its 75th percentile,
    // robust against grain and noise, then a closing removes the lines.
    let f = 4usize;
    let (sw, sh) = (w.div_ceil(f), h.div_ceil(f));
    let r = (p.paper_radius / f).max(1);
    let paper: Vec<Vec<f32>> = (0..3)
        .map(|ch| {
            let mut small = vec![0u8; sw * sh];
            let mut block = Vec::with_capacity(f * f);
            for sy in 0..sh {
                for sx in 0..sw {
                    block.clear();
                    for y in sy * f..((sy + 1) * f).min(h) {
                        for x in sx * f..((sx + 1) * f).min(w) {
                            block.push(img.get_pixel(x as u32, y as u32).0[ch]);
                        }
                    }
                    block.sort_unstable();
                    small[sy * sw + sx] = block[block.len() * 3 / 4];
                }
            }
            let closed = filter(&filter(&small, sw, sh, r, true), sw, sh, r, false);
            // smooth the estimate so it carries no blocks
            box_blur(&closed, sw, sh, r)
        })
        .collect();
    let white = (p.white * 255.0) as u32;
    let mut out = RgbImage::new(w as u32, h as u32);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (
                (x as f32 + 0.5) / f as f32 - 0.5,
                (y as f32 + 0.5) / f as f32 - 0.5,
            );
            let c = img.get_pixel(x as u32, y as u32).0;
            let lv: [u32; 3] = std::array::from_fn(|ch| {
                let bg = bilinear(&paper[ch], sw, sh, fx, fy).max(32.0);
                ((f32::from(c[ch]) * 255.0 / bg).round() as u32).min(255)
            });
            let px = if lv.iter().all(|&v| v >= white) {
                [255, 255, 255]
            } else {
                lv.map(|v| v as u8)
            };
            out.put_pixel(x as u32, y as u32, image::Rgb(px));
        }
    }
    out
}

fn box_blur(v: &[u8], w: usize, h: usize, r: usize) -> Vec<f32> {
    let mut tmp = vec![0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let (a, b) = (x.saturating_sub(r), (x + r).min(w - 1));
            tmp[y * w + x] =
                (a..=b).map(|xx| f32::from(v[y * w + xx])).sum::<f32>() / (b - a + 1) as f32;
        }
    }
    let mut out = vec![0f32; w * h];
    for x in 0..w {
        for y in 0..h {
            let (a, b) = (y.saturating_sub(r), (y + r).min(h - 1));
            out[y * w + x] = (a..=b).map(|yy| tmp[yy * w + x]).sum::<f32>() / (b - a + 1) as f32;
        }
    }
    out
}

fn bilinear(v: &[f32], w: usize, h: usize, x: f32, y: f32) -> f32 {
    let x = x.clamp(0.0, (w - 1) as f32);
    let y = y.clamp(0.0, (h - 1) as f32);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let a = v[y0 * w + x0] * (1.0 - fx) + v[y0 * w + x1] * fx;
    let b = v[y1 * w + x0] * (1.0 - fx) + v[y1 * w + x1] * fx;
    a * (1.0 - fy) + b * fy
}

/// Zhang-Suen thinning: the one-pixel skeleton of `mask`.
fn skeleton(mask: &[bool], w: usize, h: usize) -> Vec<bool> {
    let mut s = mask.to_vec();
    let at = |s: &[bool], x: i64, y: i64| {
        x >= 0 && y >= 0 && x < w as i64 && y < h as i64 && s[y as usize * w + x as usize]
    };
    loop {
        let mut changed = false;
        for step in 0..2 {
            let mut del = Vec::new();
            for y in 0..h as i64 {
                for x in 0..w as i64 {
                    if !s[y as usize * w + x as usize] {
                        continue;
                    }
                    // p2..p9 clockwise from north
                    let n = [
                        at(&s, x, y - 1),
                        at(&s, x + 1, y - 1),
                        at(&s, x + 1, y),
                        at(&s, x + 1, y + 1),
                        at(&s, x, y + 1),
                        at(&s, x - 1, y + 1),
                        at(&s, x - 1, y),
                        at(&s, x - 1, y - 1),
                    ];
                    let b = n.iter().filter(|&&v| v).count();
                    if !(2..=6).contains(&b) {
                        continue;
                    }
                    let a = (0..8).filter(|&i| !n[i] && n[(i + 1) % 8]).count();
                    if a != 1 {
                        continue;
                    }
                    let (p2, p4, p6, p8) = (n[0], n[2], n[4], n[6]);
                    // step 1: p2·p4·p6 = 0 and p4·p6·p8 = 0; step 2: p2·p4·p8 = 0 and p2·p6·p8 = 0
                    let ok = if step == 0 {
                        !(p4 && p6 && (p2 || p8))
                    } else {
                        !(p2 && p8 && (p4 || p6))
                    };
                    if ok {
                        del.push(y as usize * w + x as usize);
                    }
                }
            }
            changed |= !del.is_empty();
            for i in del {
                s[i] = false;
            }
        }
        if !changed {
            return s;
        }
    }
}

/// A bridge: from a line end to a line pixel, with the half-width of the stroke.
type Bridge = ((i64, i64), (i64, i64), i64);

const N8: [(i64, i64); 8] = [
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];

/// Joins line ends to the line they point at (see the module docs). `line` is the line mask
/// (true = line); bridges are added to it. Returns the number of bridges.
pub fn bridge_gaps(line: &mut [bool], w: usize, h: usize, max_gap: usize) -> usize {
    if max_gap == 0 || w < 3 || h < 3 {
        return 0;
    }
    let sk = skeleton(line, w, h);
    let inside = |x: i64, y: i64| x >= 0 && y >= 0 && x < w as i64 && y < h as i64;
    let sk_at = |x: i64, y: i64| inside(x, y) && sk[y as usize * w + x as usize];
    let walk_len = (max_gap as i64).max(6);
    let mut bridges: Vec<Bridge> = Vec::new();
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            if !sk_at(x, y) || N8.iter().filter(|(dx, dy)| sk_at(x + dx, y + dy)).count() != 1 {
                continue;
            }
            // walk back along the stroke
            let mut trail = vec![(x, y)];
            let (mut cx, mut cy) = (x, y);
            for _ in 0..walk_len {
                let next = N8
                    .iter()
                    .map(|(dx, dy)| (cx + dx, cy + dy))
                    .find(|&(nx, ny)| sk_at(nx, ny) && !trail.contains(&(nx, ny)));
                let Some(n) = next else { break };
                trail.push(n);
                (cx, cy) = n;
            }
            if trail.len() < 4 {
                continue; // a dot, not a stroke
            }
            let (bx, by) = trail[trail.len() - 1];
            let (dx, dy) = ((x - bx) as f32, (y - by) as f32);
            let len = (dx * dx + dy * dy).sqrt().max(1e-3);
            let (ux, uy) = (dx / len, dy / len);
            // stroke half-width near the end: per trail pixel the shorter of the horizontal and
            // vertical runs of line pixels through it, the median over the trail (the tip
            // itself tapers)
            let mut widths: Vec<i64> = trail
                .iter()
                .map(|&(tx, ty)| {
                    [(1, 0), (0, 1)]
                        .iter()
                        .map(|&(sx, sy)| {
                            let run = |sgn: i64| {
                                (1..=max_gap as i64)
                                    .take_while(|k| {
                                        let (px, py) = (tx + sgn * sx * k, ty + sgn * sy * k);
                                        inside(px, py) && line[py as usize * w + px as usize]
                                    })
                                    .count() as i64
                            };
                            run(1) + run(-1) + 1
                        })
                        .min()
                        .unwrap_or(1)
                })
                .collect();
            widths.sort_unstable();
            let half = widths[widths.len() / 2] / 2;
            let own = |px: i64, py: i64| {
                trail
                    .iter()
                    .any(|&(tx, ty)| (tx - px).abs() <= half + 2 && (ty - py).abs() <= half + 2)
            };
            // nearest line pixel ahead, within 50 degrees of the stroke direction
            // the skeleton ends half a stroke before the ink does: the gap is measured from
            // the ink, so the search reaches half a stroke further
            let r = max_gap as i64 + half + 1;
            let mut best: Option<((i64, i64), i64)> = None;
            for oy in -r..=r {
                for ox in -r..=r {
                    let d2 = ox * ox + oy * oy;
                    if d2 > r * r || d2 == 0 {
                        continue;
                    }
                    let (px, py) = (x + ox, y + oy);
                    if !inside(px, py) || !line[py as usize * w + px as usize] || own(px, py) {
                        continue;
                    }
                    let cos = (ox as f32 * ux + oy as f32 * uy) / (d2 as f32).sqrt();
                    if cos < 0.64 {
                        continue;
                    }
                    if best.is_none_or(|(_, b)| d2 < b) {
                        best = Some(((px, py), d2));
                    }
                }
            }
            if let Some((target, _)) = best {
                bridges.push(((x, y), target, half.max(1)));
            }
        }
    }
    for &((x0, y0), (x1, y1), half) in &bridges {
        let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
        for s in 0..=steps {
            let cx = x0 + (x1 - x0) * s / steps;
            let cy = y0 + (y1 - y0) * s / steps;
            for oy in -half..=half {
                for ox in -half..=half {
                    if ox * ox + oy * oy <= half * half && inside(cx + ox, cy + oy) {
                        line[(cy + oy) as usize * w + (cx + ox) as usize] = true;
                    }
                }
            }
        }
    }
    bridges.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A square outline 4 px wide with a gap of `gap` px in its top side.
    fn square(w: usize, gap: usize) -> Vec<bool> {
        let mut m = vec![false; w * w];
        for y in 10..50 {
            for x in 10..50 {
                let edge = !(14..46).contains(&x) || !(14..46).contains(&y);
                let in_gap = y < 14 && (30..30 + gap).contains(&x);
                m[y * w + x] = edge && !in_gap;
            }
        }
        m
    }

    fn enclosed(m: &[bool], w: usize) -> bool {
        // flood the outside from the corner; the centre must not be reached
        let mut seen = vec![false; w * w];
        let mut stack = vec![0usize];
        seen[0] = true;
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            for (nx, ny) in [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ] {
                if nx < w && ny < w && !m[ny * w + nx] && !seen[ny * w + nx] {
                    seen[ny * w + nx] = true;
                    stack.push(ny * w + nx);
                }
            }
        }
        !seen[30 * w + 30]
    }

    #[test]
    fn a_gap_is_bridged() {
        let w = 60;
        let mut m = square(w, 6);
        assert!(!enclosed(&m, w));
        let n = bridge_gaps(&mut m, w, w, 10);
        assert!(n >= 1);
        assert!(enclosed(&m, w));
    }

    #[test]
    fn a_wide_gap_stays_open() {
        let w = 60;
        let mut m = square(w, 14);
        bridge_gaps(&mut m, w, w, 8);
        assert!(!enclosed(&m, w));
    }

    #[test]
    fn uneven_paper_becomes_white() {
        // paper darkening from 250 to 170 left to right, one dark line in the middle
        let mut img = RgbImage::new(200, 60);
        for (x, y, p) in img.enumerate_pixels_mut() {
            let paper = 250.0 - 80.0 * x as f32 / 199.0;
            let v = if (98..102).contains(&x) && y > 5 {
                40.0
            } else {
                paper
            };
            *p = image::Rgb([v as u8, (v * 0.97) as u8, (v * 0.9) as u8]);
        }
        let out = clean_scan(
            &img,
            &ScanParams {
                paper_radius: 12,
                white: 0.86,
            },
        );
        assert_eq!(out.get_pixel(5, 30).0, [255, 255, 255]);
        assert_eq!(out.get_pixel(190, 30).0, [255, 255, 255]);
        assert!(out.get_pixel(100, 30).0[0] < 100, "the line stays dark");
    }
}
