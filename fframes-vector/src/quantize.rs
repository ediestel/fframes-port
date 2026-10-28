//! Palette reduction without dithering: median cut over the image's distinct colours,
//! refined by k-means in CIE Lab, then every pixel takes its nearest palette colour.
//! Pure black `[0,0,0]` (the outlines from [`crate::flatten`]) is always its own entry.

use std::collections::HashMap;

fn srgb_to_linear(c: u8) -> f64 {
    let c = f64::from(c) / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// sRGB to CIE Lab (D65).
#[must_use]
pub fn lab(c: [u8; 3]) -> [f64; 3] {
    let (r, g, b) = (srgb_to_linear(c[0]), srgb_to_linear(c[1]), srgb_to_linear(c[2]));
    let x = (0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b) / 0.950_47;
    let y = 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175 * b;
    let z = (0.019_333_9 * r + 0.119_192 * g + 0.950_304_1 * b) / 1.088_83;
    let f = |t: f64| if t > 0.008_856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

fn dist2(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

/// Returns the palette (black first) and the image with every pixel replaced by its palette
/// index. `colors` counts black.
#[must_use]
pub fn quantize(rgb: &[u8], colors: usize) -> (Vec<[u8; 3]>, Vec<u16>) {
    let n = rgb.len() / 3;
    let mut counts: HashMap<[u8; 3], u64> = HashMap::new();
    for i in 0..n {
        let c = [rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2]];
        if c != [0, 0, 0] {
            *counts.entry(c).or_insert(0) += 1;
        }
    }
    let mut uniq: Vec<([u8; 3], u64)> = counts.into_iter().collect();
    uniq.sort_unstable(); // deterministic order

    // median cut on the distinct colours, weighted by pixel count
    let target = colors.saturating_sub(1).max(1);
    let mut boxes: Vec<Vec<usize>> = vec![(0..uniq.len()).collect()];
    while boxes.len() < target {
        // split the box with the largest weighted channel range
        let mut best: Option<(usize, usize, f64)> = None;
        for (bi, b) in boxes.iter().enumerate() {
            if b.len() < 2 {
                continue;
            }
            let weight: u64 = b.iter().map(|&i| uniq[i].1).sum();
            for ch in 0..3 {
                let (lo, hi) = b.iter().fold((255u8, 0u8), |(lo, hi), &i| (lo.min(uniq[i].0[ch]), hi.max(uniq[i].0[ch])));
                let score = f64::from(hi - lo) * (weight as f64).sqrt();
                if best.is_none_or(|(_, _, s)| score > s) {
                    best = Some((bi, ch, score));
                }
            }
        }
        let Some((bi, ch, _)) = best else { break };
        let mut b = boxes.swap_remove(bi);
        b.sort_unstable_by_key(|&i| (uniq[i].0[ch], i));
        let total: u64 = b.iter().map(|&i| uniq[i].1).sum();
        let mut acc = 0u64;
        let mut cut = 1;
        for (k, &i) in b.iter().enumerate() {
            acc += uniq[i].1;
            if acc * 2 >= total {
                cut = (k + 1).clamp(1, b.len() - 1);
                break;
            }
        }
        let rest = b.split_off(cut);
        boxes.push(b);
        boxes.push(rest);
    }

    // k-means refinement in Lab
    let labs: Vec<[f64; 3]> = uniq.iter().map(|(c, _)| lab(*c)).collect();
    let mut centers: Vec<[f64; 3]> = boxes
        .iter()
        .map(|b| {
            let w: f64 = b.iter().map(|&i| uniq[i].1 as f64).sum::<f64>().max(1.0);
            let mut s = [0.0; 3];
            for &i in b {
                for k in 0..3 {
                    s[k] += labs[i][k] * uniq[i].1 as f64;
                }
            }
            [s[0] / w, s[1] / w, s[2] / w]
        })
        .collect();
    let mut assign = vec![0usize; uniq.len()];
    for _ in 0..8 {
        for (i, l) in labs.iter().enumerate() {
            assign[i] = (0..centers.len())
                .min_by(|&a, &b| dist2(l, &centers[a]).total_cmp(&dist2(l, &centers[b])))
                .unwrap_or(0);
        }
        let mut sums = vec![[0.0f64; 4]; centers.len()];
        for (i, &a) in assign.iter().enumerate() {
            let w = uniq[i].1 as f64;
            for k in 0..3 {
                sums[a][k] += labs[i][k] * w;
            }
            sums[a][3] += w;
        }
        for (c, s) in centers.iter_mut().zip(&sums) {
            if s[3] > 0.0 {
                *c = [s[0] / s[3], s[1] / s[3], s[2] / s[3]];
            }
        }
    }

    // palette colour = the pixel-weighted mean sRGB of its members (exact, no Lab round trip)
    let mut rgb_sums = vec![[0.0f64; 4]; centers.len()];
    for (i, &a) in assign.iter().enumerate() {
        let w = uniq[i].1 as f64;
        for k in 0..3 {
            rgb_sums[a][k] += f64::from(uniq[i].0[k]) * w;
        }
        rgb_sums[a][3] += w;
    }
    let mut palette: Vec<[u8; 3]> = vec![[0, 0, 0]];
    let mut index_of_center = vec![0u16; centers.len()];
    for (ci, s) in rgb_sums.iter().enumerate() {
        if s[3] > 0.0 {
            index_of_center[ci] = palette.len() as u16;
            palette.push([
                (s[0] / s[3]).round() as u8,
                (s[1] / s[3]).round() as u8,
                (s[2] / s[3]).round() as u8,
            ]);
        }
    }
    let lookup: HashMap<[u8; 3], u16> =
        uniq.iter().zip(&assign).map(|((c, _), &a)| (*c, index_of_center[a])).collect();
    let indices: Vec<u16> = (0..n)
        .map(|i| {
            let c = [rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2]];
            if c == [0, 0, 0] { 0 } else { lookup.get(&c).copied().unwrap_or(0) }
        })
        .collect();
    (palette, indices)
}
