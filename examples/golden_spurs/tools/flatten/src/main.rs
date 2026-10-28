//! `flatten in.png out.png [dark=0.36] [min_piece=40] [line_w=3] [speck=16] [tol=48] [soft_t=40]`
//!
//! Piece-wise flattening of a leaded-glass picture, in two steps.
//! 1. Outlines, written as flat black: pixels darker than `dark` (strongest
//!    channel, 0..1) that belong to a structure thinner than about
//!    2 * `line_w` pixels (an opening with that radius removes it) and to a
//!    dark blob of at least `speck` pixels.
//! 2. Pieces: everything else is grown into regions of similar colour; a
//!    pixel joins a region when it is within `tol` (RGB distance) of the
//!    region's running mean, so a piece ends at an outline or at a colour
//!    change. Each piece becomes its mean colour; pieces under `min_piece`
//!    pixels take the colour of their largest neighbour.
//! The result quantises and traces without islands.

use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;

fn arg<T: std::str::FromStr>(args: &[String], i: usize, default: T) -> T {
    args.get(i).and_then(|s| s.parse().ok()).unwrap_or(default)
}

/// Connected components (4-neighbourhood) of `mask`; returns labels and sizes.
fn components(mask: &[bool], w: usize, h: usize) -> (Vec<u32>, Vec<usize>) {
    let n = w * h;
    let mut label = vec![u32::MAX; n];
    let mut sizes: Vec<usize> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    for start in 0..n {
        if !mask[start] || label[start] != u32::MAX {
            continue;
        }
        let id = sizes.len() as u32;
        sizes.push(0);
        label[start] = id;
        stack.push(start);
        while let Some(i) = stack.pop() {
            sizes[id as usize] += 1;
            let (x, y) = (i % w, i / w);
            let mut visit = |j: usize| {
                if mask[j] && label[j] == u32::MAX {
                    label[j] = id;
                    stack.push(j);
                }
            };
            if x > 0 { visit(i - 1); }
            if x + 1 < w { visit(i + 1); }
            if y > 0 { visit(i - w); }
            if y + 1 < h { visit(i + w); }
        }
    }
    (label, sizes)
}

/// Morphological closing (max, then min) of a brightness map with a square of radius `r`.
fn closing_u8(v: &[u8], w: usize, h: usize, r: usize) -> Vec<u8> {
    let pass = |src: &[u8], max: bool| -> Vec<u8> {
        let mut tmp = vec![0u8; w * h];
        for y in 0..h {
            for x in 0..w {
                let (x0, x1) = (x.saturating_sub(r), (x + r).min(w - 1));
                let it = (x0..=x1).map(|xx| src[y * w + xx]);
                tmp[y * w + x] = if max { it.max().unwrap() } else { it.min().unwrap() };
            }
        }
        let mut out = vec![0u8; w * h];
        for y in 0..h {
            let (y0, y1) = (y.saturating_sub(r), (y + r).min(h - 1));
            for x in 0..w {
                let it = (y0..=y1).map(|yy| tmp[yy * w + x]);
                out[y * w + x] = if max { it.max().unwrap() } else { it.min().unwrap() };
            }
        }
        out
    };
    let dilated = pass(v, true);
    pass(&dilated, false)
}

/// Morphological opening of `mask` with a square of radius `r`.
fn opening(mask: &[bool], w: usize, h: usize, r: usize) -> Vec<bool> {
    let pass = |src: &[bool], keep_if_all: bool| -> Vec<bool> {
        // horizontal then vertical min/max
        let mut tmp = vec![false; w * h];
        for y in 0..h {
            for x in 0..w {
                let x0 = x.saturating_sub(r);
                let x1 = (x + r).min(w - 1);
                let v = if keep_if_all { (x0..=x1).all(|xx| src[y * w + xx]) } else { (x0..=x1).any(|xx| src[y * w + xx]) };
                tmp[y * w + x] = v;
            }
        }
        let mut out = vec![false; w * h];
        for y in 0..h {
            let y0 = y.saturating_sub(r);
            let y1 = (y + r).min(h - 1);
            for x in 0..w {
                let v = if keep_if_all { (y0..=y1).all(|yy| tmp[yy * w + x]) } else { (y0..=y1).any(|yy| tmp[yy * w + x]) };
                out[y * w + x] = v;
            }
        }
        out
    };
    let eroded = pass(mask, true);
    pass(&eroded, false)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("flatten in.png out.png [dark=0.36] [min_piece=40] [line_w=3] [speck=16] [tol=48] [soft_t=40]");
        std::process::exit(2);
    }
    let dark: f32 = arg(&args, 3, 0.36);
    let min_piece: usize = arg(&args, 4, 40);
    let line_w: usize = arg(&args, 5, 3);
    let speck: usize = arg(&args, 6, 16);
    let tol: f32 = arg(&args, 7, 48.0);
    let soft_t: u8 = arg(&args, 8, 40u8);

    let decoder = png::Decoder::new(std::io::BufReader::new(File::open(&args[1]).expect("open input")));
    let mut reader = decoder.read_info().expect("png header");
    let mut buf = vec![0u8; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut buf).expect("png data");
    let (w, h) = (info.width as usize, info.height as usize);
    let ch = match info.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        other => panic!("unsupported colour type {other:?}"),
    };
    assert_eq!(info.bit_depth, png::BitDepth::Eight, "8-bit PNG expected");
    let px = |i: usize| -> [u8; 3] {
        let p = &buf[i * ch..i * ch + ch];
        if ch >= 3 { [p[0], p[1], p[2]] } else { [p[0], p[0], p[0]] }
    };
    let n = w * h;
    let thresh = (dark * 255.0) as u8;
    let darkm: Vec<bool> = (0..n).map(|i| { let c = px(i); c[0].max(c[1]).max(c[2]) < thresh }).collect();

    // lines: dark, thin (not in the opening), and not a speck
    let thick = opening(&darkm, w, h, line_w);
    let (dlabel, dsizes) = components(&darkm, w, h);
    let hard: Vec<bool> = (0..n).map(|i| darkm[i] && !thick[i] && dsizes[dlabel[i] as usize] >= speck).collect();

    // soft lines: thin structures darker than their surroundings by `soft_t`
    // (a black-hat: closing of the brightness minus the brightness), such as
    // the brown strokes of a face or the bands on a club. They keep their own
    // colour, a little darkened, instead of black.
    let bright: Vec<u8> = (0..n).map(|i| { let c = px(i); c[0].max(c[1]).max(c[2]) }).collect();
    let closed = closing_u8(&bright, w, h, line_w);
    let softm: Vec<bool> = (0..n).map(|i| !hard[i] && !darkm[i] && closed[i].saturating_sub(bright[i]) >= soft_t).collect();
    let (slabel, ssizes) = components(&softm, w, h);
    let mut ssum = vec![[0u64; 3]; ssizes.len()];
    for i in 0..n {
        if softm[i] {
            let c = px(i);
            let s = &mut ssum[slabel[i] as usize];
            s[0] += c[0] as u64;
            s[1] += c[1] as u64;
            s[2] += c[2] as u64;
        }
    }
    let soft_col: Vec<[u8; 3]> = ssum.iter().zip(&ssizes).map(|(s, &k)| {
        let k = k.max(1) as u64;
        [(s[0] / k * 80 / 100) as u8, (s[1] / k * 80 / 100) as u8, (s[2] / k * 80 / 100) as u8]
    }).collect();
    let soft: Vec<bool> = (0..n).map(|i| softm[i] && ssizes[slabel[i] as usize] >= speck).collect();
    let line: Vec<bool> = (0..n).map(|i| hard[i] || soft[i]).collect();

    // 2. pieces: regions of similar colour between the outlines. A pixel joins
    // a region when it is within `tol` of the region's running mean, so a
    // piece ends at an outline or at a colour change (gold fleurs on blue,
    // blush on skin, a brown painted stroke) even where no line divides them.
    let tol2 = tol * tol;
    let mut label = vec![u32::MAX; n];
    let mut sums: Vec<[f64; 3]> = Vec::new();
    let mut sizes: Vec<usize> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    for start in 0..n {
        if line[start] || label[start] != u32::MAX {
            continue;
        }
        let id = sums.len() as u32;
        sums.push([0.0; 3]);
        sizes.push(0);
        label[start] = id;
        stack.push(start);
        while let Some(i) = stack.pop() {
            let c = px(i);
            let s = &mut sums[id as usize];
            s[0] += c[0] as f64;
            s[1] += c[1] as f64;
            s[2] += c[2] as f64;
            sizes[id as usize] += 1;
            let k = sizes[id as usize] as f64;
            let mean = [(s[0] / k) as f32, (s[1] / k) as f32, (s[2] / k) as f32];
            let (x, y) = (i % w, i / w);
            let mut visit = |j: usize| {
                if line[j] || label[j] != u32::MAX {
                    return;
                }
                let q = px(j);
                let d = (q[0] as f32 - mean[0]).powi(2) + (q[1] as f32 - mean[1]).powi(2) + (q[2] as f32 - mean[2]).powi(2);
                if d <= tol2 {
                    label[j] = id;
                    stack.push(j);
                }
            };
            if x > 0 { visit(i - 1); }
            if x + 1 < w { visit(i + 1); }
            if y > 0 { visit(i - w); }
            if y + 1 < h { visit(i + w); }
        }
    }
    let mean: Vec<[u8; 3]> = sums.iter().zip(&sizes).map(|(s, &k)| {
        let k = k.max(1) as f64;
        [(s[0] / k) as u8, (s[1] / k) as u8, (s[2] / k) as u8]
    }).collect();

    // grain: small pieces take the colour of the largest neighbouring piece
    let mut remap: HashMap<u32, u32> = HashMap::new();
    for i in 0..n {
        let l = label[i];
        if l == u32::MAX || sizes[l as usize] >= min_piece || remap.contains_key(&l) {
            continue;
        }
        let (x, y) = (i % w, i / w);
        let mut best: Option<u32> = None;
        for dy in -3i64..=3 {
            for dx in -3i64..=3 {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 { continue; }
                let m = label[ny as usize * w + nx as usize];
                if m != u32::MAX && m != l && (best.is_none() || sizes[m as usize] > sizes[best.unwrap() as usize]) {
                    best = Some(m);
                }
            }
        }
        if let Some(b) = best {
            remap.insert(l, b);
        }
    }
    let mut small = 0usize;
    let mut out = vec![0u8; n * 3];
    for i in 0..n {
        let c = if hard[i] {
            // outlines as one flat black: no anti-aliased halo to turn into specks
            [0, 0, 0]
        } else if soft[i] {
            soft_col[slabel[i] as usize]
        } else {
            let mut l = label[i];
            let mut hops = 0;
            while let Some(&m) = remap.get(&l) { l = m; hops += 1; if hops > 8 { break; } }
            if l != label[i] { small += 1; }
            mean[l as usize]
        };
        out[i * 3..i * 3 + 3].copy_from_slice(&c);
    }

    let file = File::create(&args[2]).expect("create output");
    let mut enc = png::Encoder::new(BufWriter::new(file), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().expect("png header");
    writer.write_image_data(&out).expect("png data");
    eprintln!(
        "{w}x{h}: {} pieces, {small} grain pixels reassigned, outlines {:.1} %, dark {:.1} %",
        sizes.len(),
        100.0 * line.iter().filter(|&&b| b).count() as f32 / n as f32,
        100.0 * darkm.iter().filter(|&&b| b).count() as f32 / n as f32
    );
}
