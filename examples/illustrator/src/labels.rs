//! The lettering of a picture as real text. The `ocr` stage reads the words of the source
//! picture with tesseract (an external program on the `PATH`, nothing of it is linked) and
//! writes `labels.toml` next to it: one `[[label]]` per line of lettering with its text, its
//! box in source pixels, its colour and size. The file is ours once and then the author's:
//! the text is corrected there (or replaced from the book's text), a label is dropped, moved
//! or added by hand, and the stage never overwrites it.
//!
//! The trace stage fills every label box with the colour around it before it traces, so the
//! picture underneath has no lettering; the layer kind `labels` sets the text on top, in one
//! font, sharp at any size.
//!
//! ```toml
//! font = "Helvetica"
//!
//! [[label]]
//! text = "Cervix"
//! x = 330.0          # the box of the lettering in the source picture, pixels
//! y = 220.0
//! w = 62.0
//! h = 14.0
//! size = 19.0        # font size and baseline in source pixels
//! baseline = 233.7
//! color = "#1E1E1E"
//! bold = false
//! erase = true       # fill the box with the colour around it before tracing
//! show = true        # set the text in the labels layer
//! conf = 91.0        # the OCR's confidence, for review
//! ```

use std::path::Path;
use std::process::Command;

use image::{Rgb, RgbImage};

fn yes() -> bool {
    true
}

fn default_font() -> String {
    "Helvetica Neue".to_owned()
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Label {
    pub text: String,
    /// The box of the lettering in the source picture, pixels.
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// Font size and baseline in source pixels.
    pub size: f64,
    pub baseline: f64,
    pub color: String,
    #[serde(default)]
    pub bold: bool,
    #[serde(default = "yes")]
    pub erase: bool,
    #[serde(default = "yes")]
    pub show: bool,
    #[serde(default)]
    pub conf: f64,
    /// The colour the lettering is painted over with, `#RRGGBB`, when the colour around the
    /// box is not the one behind the text (white text in a small coloured box).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
}

/// `#RRGGBB` as a colour.
fn parse_hex(hex: &str) -> Option<Rgb<u8>> {
    let hex = hex.strip_prefix('#')?;
    let channel = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    Some(Rgb([channel(0)?, channel(2)?, channel(4)?]))
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Labels {
    #[serde(default = "default_font")]
    pub font: String,
    #[serde(default, rename = "label")]
    pub labels: Vec<Label>,
}

impl Labels {
    pub fn load(path: &Path) -> Result<Labels, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        toml::from_str(&text).map_err(|e| format!("{}: {}", path.display(), e.message()))
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = toml::to_string(self).map_err(|e| format!("{}: {e}", path.display()))?;
        std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// One word as tesseract reads it.
struct Word {
    key: (u32, u32, u32),
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    conf: f32,
    text: String,
}

/// The picture is read at this many times its size: small lettering reads far better.
const OCR_SCALE: u32 = 3;

/// The lines of lettering in `picture`, with box, size, colour and weight. `work` is a
/// folder for the enlarged pictures tesseract reads.
///
/// Two readings, merged: the picture in grey with the contrast stretched (coloured and
/// white-on-colour lettering) and in colour (thin dark lettering the stretch blurs). Where two
/// readings overlap, the more confident one stays.
pub fn ocr(picture: &RgbImage, work: &Path) -> Result<Labels, String> {
    let big = fframes_vector::upscale(picture, OCR_SCALE);
    let grey = work.join("ocr_input.png");
    grey_stretched(&big).save(&grey).map_err(|e| format!("{}: {e}", grey.display()))?;
    let colour = work.join("ocr_colour.png");
    big.save(&colour).map_err(|e| format!("{}: {e}", colour.display()))?;
    let mut found = read(&grey, picture, (0.0, 0.0))?;
    found.extend(read(&colour, picture, (0.0, 0.0))?);
    // Tesseract skips lettering inside boxes and panels when it lays out a whole busy figure;
    // it reads it in a piece of the figure. Tiles a third wide and half high, overlapping by
    // half a tile (5 x 3), in colour and in grey.
    let (bw, bh) = (big.width(), big.height());
    let (tw, th) = (bw / 3, bh / 2);
    let greyed = image::DynamicImage::ImageLuma8(grey_stretched(&big)).to_rgb8();
    for row in 0..3 {
        for col in 0..5 {
            let x0 = (tw / 2 * col).min(bw - tw);
            let y0 = (th / 2 * row).min(bh - th);
            let origin = (x0 as f32 / OCR_SCALE as f32, y0 as f32 / OCR_SCALE as f32);
            let tile = work.join("ocr_tile.png");
            for source in [&big, &greyed] {
                image::imageops::crop_imm(source, x0, y0, tw, th)
                    .to_image()
                    .save(&tile)
                    .map_err(|e| format!("{}: {e}", tile.display()))?;
                found.extend(read(&tile, picture, origin)?);
            }
        }
    }
    let mut labels = merge(found);
    // A drawing read as letters comes out far larger or smaller than the lettering of the
    // figure, short or unsure ("CNC" over a person icon, "wall t" in a hatching): out it goes,
    // before its erase would take the drawing with it. A large title read with confidence stays.
    let mut sizes: Vec<f64> = labels.iter().map(|l| l.size).collect();
    sizes.sort_by(f64::total_cmp);
    if let Some(&median) = sizes.get(sizes.len() / 2) {
        labels.retain(|l| {
            let odd = l.size > 2.5 * median || l.size < 0.45 * median;
            let letters = l.text.chars().filter(|c| c.is_alphanumeric()).count();
            !odd || (letters > 4 && l.conf >= 90.0)
        });
    }
    uniform_sizes(&mut labels);
    Ok(Labels { font: default_font(), labels })
}

/// Of overlapping readings the one with the most letters (a tile cuts a line short), then the
/// more confident; in reading order.
fn merge(mut found: Vec<Label>) -> Vec<Label> {
    let letters = |l: &Label| l.text.chars().filter(|c| c.is_alphanumeric()).count();
    found.sort_by(|a, b| letters(b).cmp(&letters(a)).then(b.conf.total_cmp(&a.conf)));
    let mut kept: Vec<Label> = Vec::new();
    for label in found {
        let overlaps = kept.iter().any(|k| {
            let w = (label.x + label.w).min(k.x + k.w) - label.x.max(k.x);
            let h = (label.y + label.h).min(k.y + k.h) - label.y.max(k.y);
            w > 0.0 && h > 0.0 && w * h > 0.3 * (label.w * label.h).min(k.w * k.h)
        });
        if !overlaps {
            kept.push(label);
        }
    }
    kept.sort_by(|a, b| (a.y.round(), a.x).partial_cmp(&(b.y.round(), b.x)).unwrap_or(std::cmp::Ordering::Equal));
    kept
}

/// One tesseract reading of the enlarged picture at `input`, as labels on `picture`.
fn read(input: &Path, picture: &RgbImage, origin: (f32, f32)) -> Result<Vec<Label>, String> {
    let out = Command::new("tesseract")
        .arg(input)
        .args(["-", "--psm", "11", "tsv"])
        .output()
        .map_err(|e| format!("tesseract: {e} (install it: brew install tesseract)"))?;
    if !out.status.success() {
        return Err(format!("tesseract: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let tsv = String::from_utf8_lossy(&out.stdout);
    let mut words = Vec::new();
    for row in tsv.lines().skip(1) {
        let c: Vec<&str> = row.split('\t').collect();
        if c.len() < 12 || c[0] != "5" {
            continue;
        }
        // Quotes the OCR adds where an icon touches a word; an S before a digit read as $ (S2-S4).
        let text = c[11].trim().trim_matches(|c| "\"'‘’“”«»".contains(c));
        let fixed = (1..=5).fold(text.to_owned(), |t, d| t.replace(&format!("${d}"), &format!("S{d}")));
        let text = fixed.as_str();
        let conf: f32 = c[10].parse().unwrap_or(-1.0);
        // Noise the OCR finds in drawings: low confidence. Of the words without a letter or
        // digit only the signs of labels stay (+ / & > < = - % :), read with confidence; an
        // icon read as ® or © is not lettering. A line of nothing but signs is dropped below.
        let sign = !text.chars().any(char::is_alphanumeric);
        let label_sign = text.chars().all(|c| "+/&><=-–→%:".contains(c));
        if text.is_empty() || conf < 50.0 || (sign && (!label_sign || conf < 70.0)) {
            continue;
        }
        let id = |i: usize| c[i].parse::<u32>().unwrap_or(0);
        let n = |i: usize| c[i].parse::<f32>().unwrap_or(0.0) / OCR_SCALE as f32;
        words.push(Word {
            key: (id(2), id(3), id(4)),
            left: n(6) + origin.0,
            top: n(7) + origin.1,
            width: n(8),
            height: n(9),
            conf,
            text: text.to_owned(),
        });
    }

    // Words of one tesseract line, split where the gap is wider than the letters are high:
    // two columns that tesseract read as one line become two labels.
    let mut lines: Vec<Vec<Word>> = Vec::new();
    for word in words {
        let joins = lines.last().and_then(|line| line.last()).is_some_and(|last| {
            last.key == word.key && word.left - (last.left + last.width) < 1.5 * last.height.max(word.height)
        });
        if joins {
            lines.last_mut().unwrap().push(word);
        } else {
            lines.push(vec![word]);
        }
    }

    let mut labels = Vec::new();
    for mut line in lines {
        // Signs at the ends of a line are drawings next to it (a dashed arrow read as "---"),
        // and so is a single tall sign before it (a warning triangle read as "A").
        let lettered = |w: &Word| w.text.chars().any(|c| c.is_alphanumeric() || c == '%');
        while line.last().is_some_and(|w| !lettered(w)) {
            line.pop();
        }
        while line.first().is_some_and(|w| !lettered(w)) {
            line.remove(0);
        }
        if line.len() > 1 && line[0].text.chars().count() == 1 && line[0].height > 1.3 * line[1].height {
            line.remove(0);
        }
        // An icon beside a word reads as a doubtful word at the end of its line ("il INFECTION",
        // "Of wu NUTRITION", "TODD <2", "4) Blocked"): the words below 80 at either end go.
        while line.len() > 1 && line.first().is_some_and(|w| w.conf < 80.0) {
            line.remove(0);
        }
        while line.len() > 1 && line.last().is_some_and(|w| w.conf < 80.0) {
            line.pop();
        }
        // A number in a circle before a heading reads as "4)": the circle stays drawn.
        let numbered = |t: &str| {
            let t = t.trim_end_matches([')', '.']);
            !t.is_empty() && t.len() <= 2 && t.chars().all(|c| c.is_ascii_digit())
        };
        if line.len() > 1 && numbered(&line[0].text) && line[0].text.len() > 1 {
            line.remove(0);
        }
        if line.is_empty() {
            continue;
        }
        let text = line.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ");
        let conf = line.iter().map(|w| w.conf).sum::<f32>() / line.len() as f32;
        // Shapes in a drawing that read as letters (cells as "Le", arrows as "tt") come as
        // short words with middling confidence: a line needs a word of three letters or
        // more, or very high confidence.
        let longest = line
            .iter()
            .map(|w| w.text.chars().filter(|c| c.is_alphanumeric()).count())
            .max()
            .unwrap_or(0);
        // A short value (%, ≥2, 7d) is kept when it is read with high confidence.
        let value = text.chars().any(|c| c.is_ascii_digit() || c == '%')
            && text.chars().all(|c| c.is_ascii_alphanumeric() || "%≥≤<>+-–.,/ °".contains(c));
        // a lone digit is too often a shape; % is the one single sign kept
        let short_value = value && conf >= 90.0 && (text.chars().count() >= 2 || text == "%");
        // cross marks (a calendar of ✗) read as "XXX"
        let crosses = text.chars().filter(|c| c.is_alphanumeric()).all(|c| c == 'X' || c == 'x');
        // A short single word read from a texture or a chart ("Ned", "Ade", "NG", "EES", water
        // drops as "6006") comes at 72 to 90 confidence; a real short label (CRP, QT, C6, MRI) at
        // 92 and more. A value with a sign (≥2, 0.9%, V1-V2) is not held to it.
        let short_word = line.len() == 1 && longest <= 4 && !text.chars().any(|c| "≥≤<>%.°-–/:".contains(c));
        if conf < 70.0
            || crosses
            || (short_word && conf < 91.0)
            // two letters alone ("SS", "ae", "Os", "Ne", "ww"): lowercase is a texture, and
            // capitals need 94 (CT, QT, PR read at 95 and more)
            || (short_word
                && longest == 2
                && !text.chars().any(|c| c.is_ascii_digit())
                && (text.chars().any(char::is_lowercase) || conf < 94.0))
            || (!short_value && (longest < 2 || (longest < 3 && conf < 90.0)))
        {
            continue;
        }
        let x = line.iter().map(|w| w.left).fold(f32::MAX, f32::min);
        let right = line.iter().map(|w| w.left + w.width).fold(0.0, f32::max);
        // Size and baseline by word, the lower middle of them: a word whose box took in a line
        // or an icon beside it reads too large and does not decide. The box is then clipped to
        // the lettering at that size, so erasing leaves the line or icon alone.
        let mut estimates: Vec<(f32, f32)> =
            line.iter().map(|w| size_and_baseline(&w.text, w.top, w.height)).collect();
        estimates.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (size, baseline) = estimates[(estimates.len() - 1) / 2];
        let top = line.iter().map(|w| w.top).fold(f32::MAX, f32::min);
        let low = line.iter().map(|w| w.top + w.height).fold(0.0, f32::max);
        let y = top.max(baseline - 0.8 * size);
        let bottom = low.min(baseline + 0.3 * size);
        let (w, h) = (right - x, bottom - y);
        let color = ink(picture, x, y, w, h);
        // One rule for the whole book: headings in capitals are bold, the rest medium.
        // (B12 and 25-OH-D count as capitals; a lone R or L does not)
        let bold = (text.chars().filter(|c| c.is_alphabetic()).count() >= 2
            || (text.chars().any(char::is_uppercase) && text.chars().any(|c| c.is_ascii_digit())))
            && !text.chars().any(char::is_lowercase);
        labels.push(Label {
            text,
            x: round1(x),
            y: round1(y),
            w: round1(w),
            h: round1(h),
            size: round1(size),
            baseline: round1(baseline),
            color,
            bold,
            erase: true,
            show: true,
            conf: round1(conf),
            fill: None,
        });
    }
    Ok(labels)
}

/// One size for the headings and one for the rest: every label within a quarter of the
/// median size of its kind takes that median, so the lettering is even across the figure.
/// Titles clearly larger than the rest keep their size. The baseline stays where it was read.
fn uniform_sizes(labels: &mut [Label]) {
    // A label without letters (%, ≥2) has no case to tell its weight: it takes the weight of
    // the nearest lettered label on the same line, so it matches its neighbours.
    for i in 0..labels.len() {
        if labels[i].text.chars().any(char::is_alphabetic) {
            continue;
        }
        let (b, x) = (labels[i].baseline, labels[i].x);
        let near = labels
            .iter()
            .filter(|l| l.text.chars().any(char::is_alphabetic) && (l.baseline - b).abs() < 4.0)
            .min_by(|p, q| (p.x - x).abs().total_cmp(&(q.x - x).abs()))
            .map(|l| l.bold);
        if let Some(bold) = near {
            labels[i].bold = bold;
        }
    }
    for bold in [true, false] {
        let mut sizes: Vec<f64> = labels.iter().filter(|l| l.bold == bold).map(|l| l.size).collect();
        if sizes.is_empty() {
            continue;
        }
        sizes.sort_by(f64::total_cmp);
        let median = sizes[sizes.len() / 2];
        for label in labels.iter_mut().filter(|l| l.bold == bold) {
            if (0.75 * median..=1.3 * median).contains(&label.size) {
                label.size = median;
            }
        }
    }
}

fn round1(v: f32) -> f64 {
    (f64::from(v) * 10.0).round() / 10.0
}

/// Font size and baseline from the height of the lettering, by what the letters reach:
/// capitals, digits and ascenders reach 0.72 em above the baseline (Helvetica), the
/// x-height 0.53 em, descenders 0.21 em below it.
fn size_and_baseline(text: &str, top: f32, h: f32) -> (f32, f32) {
    let descends = text.chars().any(|c| "gjpqy,;()[]".contains(c));
    let tall = text.chars().any(|c| c.is_uppercase() || c.is_ascii_digit() || "bdfhklt".contains(c));
    let reach = if tall { 0.72 } else { 0.53 };
    let size = h / (reach + if descends { 0.21 } else { 0.0 });
    (size, top + reach * size)
}

fn distance(a: Rgb<u8>, b: Rgb<u8>) -> f32 {
    (0..3).map(|i| (f32::from(a[i]) - f32::from(b[i])).powi(2)).sum::<f32>().sqrt()
}

fn hex(c: Rgb<u8>) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

/// The pixels of the box, clamped to the picture.
fn box_pixels(picture: &RgbImage, x: f32, y: f32, w: f32, h: f32) -> impl Iterator<Item = (u32, u32)> {
    let (pw, ph) = (picture.width() as i64, picture.height() as i64);
    let x0 = (x.floor() as i64).clamp(0, pw);
    let y0 = (y.floor() as i64).clamp(0, ph);
    let x1 = ((x + w).ceil() as i64).clamp(0, pw);
    let y1 = ((y + h).ceil() as i64).clamp(0, ph);
    (y0..y1).flat_map(move |py| (x0..x1).map(move |px| (px as u32, py as u32)))
}

/// The colour around a box: the most common colour of a two-pixel ring just outside it
/// (a frame line or a drawing the ring touches does not tint it), the mean of that colour.
pub fn background(picture: &RgbImage, x: f32, y: f32, w: f32, h: f32, pad: f32) -> Rgb<u8> {
    let outer: Vec<(u32, u32)> = box_pixels(picture, x - pad - 2.0, y - pad - 2.0, w + 2.0 * pad + 4.0, h + 2.0 * pad + 4.0).collect();
    let (ix0, iy0, ix1, iy1) = (x - pad, y - pad, x + w + pad, y + h + pad);
    let ring: Vec<Rgb<u8>> = outer
        .into_iter()
        .filter(|&(px, py)| {
            let (fx, fy) = (px as f32, py as f32);
            fx < ix0 || fy < iy0 || fx >= ix1 || fy >= iy1
        })
        .map(|(px, py)| *picture.get_pixel(px, py))
        .collect();
    let bucket = |p: &Rgb<u8>| (p[0] >> 4, p[1] >> 4, p[2] >> 4);
    let mut counts = std::collections::HashMap::new();
    for p in &ring {
        *counts.entry(bucket(p)).or_insert(0u32) += 1;
    }
    let Some((&top, _)) = counts.iter().max_by_key(|(_, n)| **n) else {
        return Rgb([255, 255, 255]);
    };
    let members: Vec<&Rgb<u8>> = ring.iter().filter(|p| bucket(p) == top).collect();
    let n = members.len() as u32;
    let mean = |i: usize| (members.iter().map(|p| u32::from(p[i])).sum::<u32>() / n) as u8;
    Rgb([mean(0), mean(1), mean(2)])
}

/// The colour of the lettering in a box: the pixels farthest from the background.
fn ink(picture: &RgbImage, x: f32, y: f32, w: f32, h: f32) -> String {
    let bg = background(picture, x, y, w, h, 2.0);
    let pixels: Vec<Rgb<u8>> = box_pixels(picture, x, y, w, h).map(|(px, py)| *picture.get_pixel(px, py)).collect();
    let far = pixels.iter().map(|&p| distance(p, bg)).fold(0.0, f32::max);
    if far < 1.0 {
        return "#1E1E1E".to_owned();
    }
    let strong: Vec<Rgb<u8>> = pixels.iter().copied().filter(|&p| distance(p, bg) > 0.7 * far).collect();
    let n = strong.len().max(1) as u32;
    let mean = |i: usize| (strong.iter().map(|p| u32::from(p[i])).sum::<u32>() / n) as u8;
    hex(Rgb([mean(0), mean(1), mean(2)]))
}

/// The picture with the lettering of every label that asks for it painted over in the
/// colour around it. Only the strokes go, with a pixel of their soft edge, inside a box that
/// hugs the lettering: an arrow or a drawing beside the text stays.
pub fn erase(picture: &RgbImage, labels: &Labels) -> RgbImage {
    let mut clean = picture.clone();
    for label in labels.labels.iter().filter(|l| l.erase) {
        let (x, y, w, h) = (label.x as f32, label.y as f32, label.w as f32, label.h as f32);
        // the soft edge of bold lettering reaches past the box the OCR gives
        let pad = 2.5;
        let bg = label
            .fill
            .as_deref()
            .and_then(parse_hex)
            .unwrap_or_else(|| background(picture, x, y, w, h, pad));
        let area: Vec<(u32, u32)> = box_pixels(picture, x - pad, y - pad, w + 2.0 * pad, h + 2.0 * pad).collect();
        let stroke: std::collections::HashSet<(u32, u32)> = area
            .iter()
            .copied()
            .filter(|&(px, py)| distance(*picture.get_pixel(px, py), bg) > 30.0)
            .collect();
        for &(px, py) in &area {
            let near = (-1i64..=1).any(|dy| {
                (-1i64..=1).any(|dx| stroke.contains(&((px as i64 + dx) as u32, (py as i64 + dy) as u32)))
            });
            if near {
                clean.put_pixel(px, py, bg);
            }
        }
    }
    clean
}

/// A picture in grey with its contrast stretched from the 1st to the 99th percentile.
fn grey_stretched(picture: &RgbImage) -> image::GrayImage {
    let grey = image::DynamicImage::ImageRgb8(picture.clone()).to_luma8();
    let mut histogram = [0usize; 256];
    for p in grey.pixels() {
        histogram[p[0] as usize] += 1;
    }
    let total = grey.pixels().len();
    let level = |share: f64| {
        let mut seen = 0;
        for (value, count) in histogram.iter().enumerate() {
            seen += count;
            if seen as f64 >= share * total as f64 {
                return value as f32;
            }
        }
        255.0
    };
    let (low, high) = (level(0.01), level(0.99).max(level(0.01) + 1.0));
    image::GrayImage::from_fn(grey.width(), grey.height(), |x, y| {
        let v = (f32::from(grey.get_pixel(x, y)[0]) - low) / (high - low) * 255.0;
        image::Luma([v.clamp(0.0, 255.0) as u8])
    })
}
