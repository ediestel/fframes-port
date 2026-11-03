//! Centrelines: the outline mask of the flattened picture (every pixel the flatten step made
//! pure black) thinned to one pixel (Zhang-Suen), walked into polylines, simplified, and drawn
//! as strokes of a chosen width. That is what makes the traced lines fine: potrace fills the
//! whole dark band of an anti-aliased line, a stroke along its middle has the width you choose.
//!
//! Polylines are in canvas pixels, saved as JSON (`[[x, y], ...]` per line) next to the pieces.

use image::RgbImage;

/// One polyline, canvas pixels.
pub type Line = Vec<(f32, f32)>;

/// The outline mask: pixels the flatten step wrote as pure black.
pub fn mask_of(flat: &RgbImage) -> Vec<bool> {
    flat.pixels().map(|p| p[0] == 0 && p[1] == 0 && p[2] == 0).collect()
}

/// Zhang-Suen thinning of a mask to one-pixel-wide lines.
pub fn thin(mask: &[bool], w: usize, h: usize) -> Vec<bool> {
    let mut img = mask.to_vec();
    let at = |img: &[bool], x: isize, y: isize| -> bool {
        x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h && img[y as usize * w + x as usize]
    };
    loop {
        let mut changed = false;
        for pass in 0..2 {
            let mut remove = Vec::new();
            for y in 0..h as isize {
                for x in 0..w as isize {
                    if !at(&img, x, y) {
                        continue;
                    }
                    let p2 = at(&img, x, y - 1);
                    let p3 = at(&img, x + 1, y - 1);
                    let p4 = at(&img, x + 1, y);
                    let p5 = at(&img, x + 1, y + 1);
                    let p6 = at(&img, x, y + 1);
                    let p7 = at(&img, x - 1, y + 1);
                    let p8 = at(&img, x - 1, y);
                    let p9 = at(&img, x - 1, y - 1);
                    let ring = [p2, p3, p4, p5, p6, p7, p8, p9];
                    let b: usize = ring.iter().filter(|&&v| v).count();
                    if !(2..=6).contains(&b) {
                        continue;
                    }
                    let a = (0..8).filter(|&i| !ring[i] && ring[(i + 1) % 8]).count();
                    if a != 1 {
                        continue;
                    }
                    let (c1, c2) = if pass == 0 {
                        (p2 && p4 && p6, p4 && p6 && p8)
                    } else {
                        (p2 && p4 && p8, p2 && p6 && p8)
                    };
                    if !c1 && !c2 {
                        remove.push(y as usize * w + x as usize);
                    }
                }
            }
            if !remove.is_empty() {
                changed = true;
                for i in remove {
                    img[i] = false;
                }
            }
        }
        if !changed {
            return img;
        }
    }
}

/// The eight neighbours of a skeleton pixel that are set.
fn neighbours(skel: &[bool], w: usize, h: usize, i: usize) -> Vec<usize> {
    let (x, y) = ((i % w) as isize, (i / w) as isize);
    let mut out = Vec::with_capacity(8);
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let (nx, ny) = (x + dx, y + dy);
            if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                let j = ny as usize * w + nx as usize;
                if skel[j] {
                    out.push(j);
                }
            }
        }
    }
    out
}

/// Walks the skeleton into polylines: every chain between end points or junctions, and every
/// closed loop, each pixel used once. Lines shorter than `min_len` pixels are dropped.
pub fn walk(skel: &[bool], w: usize, h: usize, min_len: usize) -> Vec<Line> {
    let degree: Vec<u8> = (0..w * h)
        .map(|i| if skel[i] { neighbours(skel, w, h, i).len() as u8 } else { 0 })
        .collect();
    let mut used = vec![false; w * h];
    let mut lines = Vec::new();
    let point = |i: usize| ((i % w) as f32 + 0.5, (i / w) as f32 + 0.5);

    // chains start at every pixel that is not in the middle of a line, then the loops
    let starts: Vec<usize> = (0..w * h).filter(|&i| skel[i] && degree[i] != 2).collect();
    let trace_from = |start: usize, used: &mut Vec<bool>, lines: &mut Vec<Line>| {
        for first in neighbours(skel, w, h, start) {
            if used[first] && degree[first] == 2 {
                continue;
            }
            let mut line = vec![point(start)];
            let (mut prev, mut cur) = (start, first);
            used[start] = true;
            loop {
                line.push(point(cur));
                if used[cur] || degree[cur] != 2 {
                    used[cur] = true;
                    break;
                }
                used[cur] = true;
                let Some(next) = neighbours(skel, w, h, cur).into_iter().find(|&n| n != prev) else {
                    break;
                };
                prev = cur;
                cur = next;
            }
            if line.len() >= min_len.max(2) {
                lines.push(line);
            }
        }
    };
    for start in starts {
        trace_from(start, &mut used, &mut lines);
    }
    // what is left are closed loops: every pixel has two neighbours
    for start in 0..w * h {
        if skel[start] && !used[start] {
            trace_from(start, &mut used, &mut lines);
        }
    }
    lines
}

/// Douglas-Peucker simplification within `tolerance` pixels.
pub fn simplify(line: &[(f32, f32)], tolerance: f32) -> Line {
    if line.len() < 3 {
        return line.to_vec();
    }
    let (a, b) = (line[0], line[line.len() - 1]);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt();
    let (mut far, mut far_d) = (0, 0.0);
    for (i, p) in line.iter().enumerate().skip(1).take(line.len() - 2) {
        let d = if len < 1e-6 {
            ((p.0 - a.0).powi(2) + (p.1 - a.1).powi(2)).sqrt()
        } else {
            ((p.0 - a.0) * dy - (p.1 - a.1) * dx).abs() / len
        };
        if d > far_d {
            far = i;
            far_d = d;
        }
    }
    if far_d <= tolerance {
        return vec![a, b];
    }
    let mut left = simplify(&line[..=far], tolerance);
    let right = simplify(&line[far..], tolerance);
    left.pop();
    left.extend(right);
    left
}

/// The centrelines of `flat` at the canvas size: thinned, walked, simplified.
pub fn centrelines(flat: &RgbImage, min_len: usize, tolerance: f32) -> Vec<Line> {
    let (w, h) = (flat.width() as usize, flat.height() as usize);
    let skel = thin(&mask_of(flat), w, h);
    walk(&skel, w, h, min_len).iter().map(|line| simplify(line, tolerance)).collect()
}

/// SVG path data of a polyline.
pub fn path_d(line: &[(f32, f32)]) -> String {
    use std::fmt::Write;
    let mut d = String::with_capacity(line.len() * 12);
    for (i, (x, y)) in line.iter().enumerate() {
        let _ = write!(d, "{}{x:.1} {y:.1}", if i == 0 { "M" } else { "L" });
    }
    d
}

/// A standalone SVG of the lines at the canvas size.
pub fn to_svg(lines: &[Line], w: u32, h: u32, width: f32, color: &str) -> String {
    use std::fmt::Write;
    let mut s = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><g fill="none" stroke="{color}" stroke-width="{width}" stroke-linecap="round" stroke-linejoin="round">"#
    );
    for line in lines {
        let _ = write!(s, r#"<path d="{}"/>"#, path_d(line));
    }
    s.push_str("</g></svg>");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thick_bar_thins_to_one_line() {
        let (w, h) = (40, 12);
        let mut flat = RgbImage::from_pixel(w, h, image::Rgb([255, 255, 255]));
        for y in 3..9 {
            for x in 5..35 {
                flat.put_pixel(x, y, image::Rgb([0, 0, 0]));
            }
        }
        let lines = centrelines(&flat, 5, 1.0);
        assert_eq!(lines.len(), 1, "{lines:?}");
        let line = &lines[0];
        // the middle row, from one end of the bar to the other
        assert!(line.iter().all(|(_, y)| (*y - 6.0).abs() <= 1.0), "{line:?}");
        let xs: Vec<f32> = line.iter().map(|(x, _)| *x).collect();
        let (min, max) = (xs.iter().cloned().fold(f32::MAX, f32::min), xs.iter().cloned().fold(f32::MIN, f32::max));
        assert!(min <= 9.0 && max >= 30.0, "{line:?}");
        assert!(to_svg(&lines, w, h, 2.0, "#000").contains("<path d=\"M"));
    }
}
