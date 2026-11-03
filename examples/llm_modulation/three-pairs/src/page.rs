//! The pieces of a page: the stage, the quoted source, the two response
//! columns, code lines and the result tables.
//!
//! A page stands complete from its first frame. The one thing that moves is
//! the amber underline under a statement the source does not support: it is
//! drawn on a moment after the page appears, with the soft tone.
//!
//! Columns size their type to the room they have: the largest size from
//! `SIZES` at which every entry fits above the bottom margin.

use fframes::{FFramesContext, Frame, Svgr};

use crate::ui::*;

/// When, after a page appears, its underlines are drawn on.
pub const FLAG_AT: f32 = 1.4;
/// How long an underline takes; the tone sounds as it lands.
pub const FLAG_DUR: f32 = 0.6;

/// Seconds between the underlines of one page.
pub const FLAG_STEP: f32 = 1.0;

/// The lowest baseline any column may reach.
pub const BOTTOM: f32 = 1030.0;

/// A row of a pair poster: what was asked, what was planted or asked
/// exactly, and the two answers in a few words.
pub struct Row {
    pub what: &'static str,
    pub note: &'static str,
    pub direct: &'static [Entry],
    pub modulated: &'static [Entry],
}

/// Number of underlines in a list.
pub fn flag_count(list: &[Entry]) -> usize {
    list.iter().filter(|e| e.flag.is_some()).count()
}

/// Columns of the pair posters: what, grok-4.7, grok-4.7+modulation.
const G_WHAT: (f32, f32) = (MX, 340.0);
const G_DIRECT: (f32, f32) = (500.0, 620.0);
const G_MOD: (f32, f32) = (1180.0, 620.0);
const G_SIZE: usize = 28;

/// A pair poster: the column heads, one row per query, and the count
/// beneath: `counts` for grok-4.7 and grok-4.7+modulation, with what is
/// counted. Underlines land row after row.
pub fn grid(frame: &mut Frame, ctx: &FFramesContext, t: f32, rows: &[Row], counts: (&str, &str), counted: &str) -> Svgr<'static> {
    let mut out = Vec::new();
    let head_y = 214.0;
    out.push(fframes::svgr!(<rect x={MX - 20.0} y={head_y - 40.0} width={W - 2.0 * MX + 40.0} height="58" rx="4" fill={SHEET} />));
    for ((x, _), (h, color)) in [G_WHAT, G_DIRECT, G_MOD].iter().zip([("", INK), ("grok-4.7", FLAG), ("grok-4.7+modulation", OK)]) {
        out.push(fframes::svgr!(
            <text x={*x} y={head_y} font-family={SERIF} font-size="32" font-weight="700" fill={color}>{h}</text>
        ));
    }
    out.push(fframes::svgr!(<rect x={MX - 20.0} y={head_y + 18.0} width={W - 2.0 * MX + 40.0} height="2" fill={ACCENT} />));
    let line_h = G_SIZE as f32 * 1.34;
    let gap = G_SIZE as f32 * 0.55;
    let lines = |frame: &mut Frame, list: &[Entry], w: f32| -> f32 {
        list.iter()
            .map(|e| {
                let tw = if e.numbered || e.open { w - G_SIZE as f32 * 2.2 } else { w };
                line_widths(&layout(frame, ctx, e.text, SERIF, G_SIZE, 400, false, tw)).len().max(1) as f32 * line_h + gap
            })
            .sum()
    };
    let mut y = head_y + 40.0;
    let mut before = 0;
    for (i, row) in rows.iter().enumerate() {
        let what_h = 44.0 + line_widths(&layout(frame, ctx, row.note, SERIF, 24, 400, true, G_WHAT.1)).len().max(1) as f32 * 32.0;
        let h = what_h.max(lines(frame, row.direct, G_DIRECT.1)).max(lines(frame, row.modulated, G_MOD.1)) + 30.0;
        let zebra = if i % 2 == 1 { 0.7 } else { 0.0 };
        out.push(fframes::svgr!(<rect x={MX - 20.0} y={y} width={W - 2.0 * MX + 40.0} height={h} fill={SHEET} opacity={zebra} />));
        let base = y + G_SIZE as f32 + 22.0;
        let what = row.what.to_owned();
        out.push(fframes::svgr!(
            <text x={G_WHAT.0} y={base} font-family={SERIF} font-size={G_SIZE + 2} font-weight="700" fill={INK}>{what}</text>
        ));
        out.push(block(frame, ctx, G_WHAT.0, base + 40.0, row.note, Type::new(SERIF, 24, 400, GREY).width(G_WHAT.1).line_h(32.0).italic()));
        out.push(entries(frame, ctx, G_DIRECT.0, base, G_DIRECT.1, row.direct, G_SIZE, t - before as f32 * FLAG_STEP));
        out.push(entries(frame, ctx, G_MOD.0, base, G_MOD.1, row.modulated, G_SIZE, t));
        before += flag_count(row.direct);
        out.push(fframes::svgr!(<rect x={MX - 20.0} y={y + h - 1.0} width={W - 2.0 * MX + 40.0} height="1" fill={RULE} />));
        y += h;
    }
    // the count, beneath the two answer columns
    let counted = counted.to_owned();
    for ((x, _), (n, color)) in [G_DIRECT, G_MOD].iter().zip([(counts.0, FLAG), (counts.1, OK)]) {
        let n = n.to_owned();
        let c = counted.clone();
        out.push(fframes::svgr!(
            <g>
                <text x={*x} y="1010" font-family={SERIF} font-size="120" font-weight="700" fill={color}>{n}</text>
                <text x={*x + 100.0} y="990" font-family={SERIF} font-size="27" font-style="italic" fill={GREY}>{c}</text>
            </g>
        ));
    }
    fframes::svgr!(<g>{out}</g>)
}

const SIZES: [usize; 9] = [25, 24, 23, 22, 21, 20, 19, 18, 17];

/// The empty page.
pub fn stage() -> Svgr<'static> {
    fframes::svgr!(<rect width="1920" height="1080" fill={PAPER} />)
}

/// The note at the foot of every page that shows responses condensed.
pub fn condensed() -> Svgr<'static> {
    fframes::svgr!(
        <text x={W - MX} y="1058" text-anchor="end" font-family={SERIF} font-size="21" font-style="italic" fill={GREY}>"Responses condensed; quoted words verbatim. The full texts are in the manuscript."</text>
    )
}

/// One entry of a response: a paragraph or a numbered item.
#[derive(Clone, Copy)]
pub struct Entry {
    pub text: &'static str,
    /// Numbered item (the response's own list).
    pub numbered: bool,
    /// Words of the entry the source does not support: underlined in amber.
    pub flag: Option<&'static str>,
    /// The entry names what could not be established: a mint tick.
    pub open: bool,
}

pub const fn p(text: &'static str) -> Entry {
    Entry { text, numbered: false, flag: None, open: false }
}
pub const fn n(text: &'static str) -> Entry {
    Entry { text, numbered: true, flag: None, open: false }
}
/// A numbered item that names what could not be established.
pub const fn no(text: &'static str) -> Entry {
    Entry { text, numbered: true, flag: None, open: true }
}
/// A paragraph that names what could not be established.
pub const fn po(text: &'static str) -> Entry {
    Entry { text, numbered: false, flag: None, open: true }
}
/// A paragraph with words the source does not support.
pub const fn pf(text: &'static str, flag: &'static str) -> Entry {
    Entry { text, numbered: false, flag: Some(flag), open: false }
}

/// The model's name over a column, its timing beneath.
pub fn column_head(x: f32, y: f32, name: &str, color: &'static str, meta: &str) -> Svgr<'static> {
    let name = name.to_owned();
    let meta = meta.to_owned();
    fframes::svgr!(
        <g>
            <text x={x} y={y} font-family={SERIF} font-size="34" font-weight="700" fill={color}>{name}</text>
            <text x={x} y={y + 34.0} font-family={SERIF} font-size="22" font-style="italic" fill={GREY}>{meta}</text>
            <rect x={x} y={y + 52.0} width="60" height="2.5" fill={color} />
        </g>
    )
}

/// Height of `entries` set at `size` in width `w`.
fn height(frame: &mut Frame, ctx: &FFramesContext, entries: &[Entry], w: f32, size: usize) -> f32 {
    let line_h = size as f32 * 1.34;
    let gap = size as f32 * 0.55;
    let mut h = 0.0;
    for e in entries {
        let tw = if e.numbered || e.open { w - size as f32 * 2.2 } else { w };
        let lines = line_widths(&layout(frame, ctx, e.text, SERIF, size, 400, false, tw)).len().max(1);
        h += lines as f32 * line_h + gap;
    }
    h
}

/// The largest type size at which `entries` fit between `top` and `BOTTOM`.
/// Reports (once per call site and size) when even the smallest does not.
pub fn fit(frame: &mut Frame, ctx: &FFramesContext, entries: &[Entry], w: f32, top: f32) -> usize {
    for s in SIZES {
        if top + height(frame, ctx, entries, w, s) <= BOTTOM {
            return s;
        }
    }
    let s = SIZES[SIZES.len() - 1];
    let need = top + height(frame, ctx, entries, w, s);
    let first = entries.first().map_or("", |e| e.text);
    eprintln!("page overflow: column starting {:?} ends at {need:.0} > {BOTTOM}", &first[..first.len().min(40)]);
    s
}

/// One response column: who answered, how long it took, what it said.
pub struct Column<'e> {
    pub name: &'static str,
    pub color: &'static str,
    pub meta: &'static str,
    pub entries: &'e [Entry],
    pub x: f32,
    pub w: f32,
    /// Set the entries in this many side-by-side runs.
    pub runs: usize,
}

/// Columns side by side from `top`: the head, then the entries.
pub fn columns(frame: &mut Frame, ctx: &FFramesContext, top: f32, cols: &[Column], t: f32) -> Svgr<'static> {
    let mut out = Vec::new();
    let body_top = top + 96.0;
    for c in cols {
        out.push(column_head(c.x, top + 34.0, c.name, c.color, c.meta));
        let runs = c.runs.max(1);
        let gap = 40.0;
        let rw = (c.w - gap * (runs - 1) as f32) / runs as f32;
        let per = c.entries.len().div_ceil(runs);
        // one size for all runs of a column: the one that fits the fullest run
        let size = (0..runs)
            .map(|r| {
                let part = &c.entries[(r * per).min(c.entries.len())..((r + 1) * per).min(c.entries.len())];
                fit(frame, ctx, part, rw, body_top)
            })
            .min()
            .unwrap_or(SIZES[0]);
        let mut first_num = 0;
        for r in 0..runs {
            let part = &c.entries[(r * per).min(c.entries.len())..((r + 1) * per).min(c.entries.len())];
            out.push(numbered_from(frame, ctx, c.x + r as f32 * (rw + gap), body_top + size as f32, rw, part, size, t, first_num));
            first_num += part.iter().filter(|e| e.numbered).count();
        }
    }
    fframes::svgr!(<g>{out}</g>)
}

/// `entries`, with the numbering continuing after `before` items.
#[allow(clippy::too_many_arguments)]
fn numbered_from(frame: &mut Frame, ctx: &FFramesContext, x: f32, y: f32, w: f32, list: &[Entry], size: usize, t: f32, before: usize) -> Svgr<'static> {
    entries_from(frame, ctx, x, y, w, list, size, t, before)
}

/// A column of entries from baseline `y`, at size `size`. `t` is page time.
#[allow(clippy::too_many_arguments)]
pub fn entries(frame: &mut Frame, ctx: &FFramesContext, x: f32, y: f32, w: f32, list: &[Entry], size: usize, t: f32) -> Svgr<'static> {
    entries_from(frame, ctx, x, y, w, list, size, t, 0)
}

#[allow(clippy::too_many_arguments)]
fn entries_from(frame: &mut Frame, ctx: &FFramesContext, x: f32, y: f32, w: f32, list: &[Entry], size: usize, t: f32, before: usize) -> Svgr<'static> {
    let line_h = size as f32 * 1.34;
    let gap = size as f32 * 0.55;
    let gutter = size as f32 * 2.2;
    let mut out = Vec::new();
    let mut by = y;
    let mut num = before;
    let mut flags = 0;
    for e in list {
        let indent = e.numbered || e.open;
        let tx = if indent { x + gutter } else { x };
        let tw = if indent { w - gutter } else { w };
        if e.numbered {
            num += 1;
            let label = format!("{num}.");
            out.push(fframes::svgr!(
                <text x={x + gutter - size as f32 * 0.5} y={by} text-anchor="end" font-family={SERIF} font-size={size} fill={GREY}>{label}</text>
            ));
        }
        if e.open {
            // the tick sits in the gutter's left half, beside the number
            let tx0 = if e.numbered { x - size as f32 * 0.7 } else { x + size as f32 * 0.7 };
            out.push(tick(tx0, by - size as f32 * 0.32, size as f32 * 0.9, 1.0));
        }
        let placed = layout(frame, ctx, e.text, SERIF, size, 400, false, tw);
        let lines = line_widths(&placed).len().max(1);
        let ty = Type::new(SERIF, size, 400, INK).width(tw).line_h(line_h);
        // the wash and the underline go beneath the words, never over them;
        // the underlines of one call land one after another
        if let Some(flag) = e.flag {
            out.push(flag_marks(&placed, tx, by, size, line_h, flag, t - flags as f32 * FLAG_STEP));
            flags += 1;
        }
        out.push(block(frame, ctx, tx, by, e.text, ty));
        by += lines as f32 * line_h + gap;
    }
    fframes::svgr!(<g>{out}</g>)
}

/// The amber underline under the words `flag` of a laid-out entry, line by
/// line, drawn on after `FLAG_AT`.
fn flag_marks(placed: &[Placed], x: f32, y: f32, size: usize, line_h: f32, flag: &str, t: f32) -> Svgr<'static> {
    let s = size as f32;
    let want: Vec<&str> = flag.split_whitespace().collect();
    let words: Vec<&str> = placed.iter().map(|p| p.word.as_str()).collect();
    let Some(start) = (0..words.len()).find(|&i| words.len() - i >= want.len() && words[i..i + want.len()] == want[..]) else {
        return Svgr::empty();
    };
    let span = &placed[start..start + want.len()];
    let pk = prog(t, FLAG_AT, FLAG_AT + FLAG_DUR);
    let mut out = Vec::new();
    let first = span[0].line;
    let last = span[span.len() - 1].line;
    for line in first..=last {
        let on: Vec<&Placed> = span.iter().filter(|p| p.line == line).collect();
        let x0 = on.iter().map(|p| p.x).fold(f32::MAX, f32::min);
        let x1 = on.iter().map(|p| p.x + p.width).fold(0.0, f32::max);
        let base = y + line as f32 * line_h;
        // under the descenders, above the next line's capitals
        let ly = base + s * 0.2;
        let wash_y = base - s * 0.78;
        // each line of the span takes its share of the stroke
        let share = (line - first) as f32 / (last - first + 1) as f32;
        let k = prog(pk, share, share + 1.0 / (last - first + 1) as f32);
        out.push(fframes::svgr!(
            <g>
                <rect x={x + x0 - 3.0} y={wash_y} width={x1 - x0 + 6.0} height={s * 0.96} rx="3" fill={FLAG_WASH} opacity={k} />
                {underline(x + x0, ly, x1 - x0, k, (s * 0.12).max(2.0))}
            </g>
        ));
    }
    fframes::svgr!(<g>{out}</g>)
}

/// The quoted source on its panel: label, text, and the note beneath.
#[allow(clippy::too_many_arguments)]
pub fn source(frame: &mut Frame, ctx: &FFramesContext, top: f32, label_text: &str, text: &str, note: &str, size: usize) -> (Svgr<'static>, f32) {
    let w = W - 2.0 * MX - 80.0;
    let line_h = size as f32 * 1.38;
    let lines = line_widths(&layout(frame, ctx, text, SERIF, size, 400, true, w)).len().max(1);
    let h = 76.0 + lines as f32 * line_h + if note.is_empty() { 8.0 } else { 46.0 };
    let ty = Type::new(SERIF, size, 400, INK).width(w).line_h(line_h).italic();
    let body = block(frame, ctx, MX + 40.0, top + 82.0, text, ty);
    let note = note.to_owned();
    let note_y = top + 82.0 + lines as f32 * line_h + 14.0;
    let svg = fframes::svgr!(
        <g>
            <rect x={MX} y={top} width={W - 2.0 * MX} height={h} rx="6" fill={SHEET} stroke={RULE} stroke-width="1.5" />
            <rect x={MX} y={top} width="5" height={h} fill={ACCENT} />
            {label(MX + 40.0, top + 40.0, label_text, ACCENT, 20.0, "start")}
            {body}
            <text x={MX + 40.0} y={note_y} font-family={SERIF} font-size="23" font-style="italic" fill={GREY}>{note}</text>
        </g>
    );
    (svg, top + h)
}

/// A line of code with its program line number; `flag` is a part of it that
/// the published package does not have, underlined in amber.
pub fn code_line(frame: &mut Frame, ctx: &FFramesContext, x: f32, y: f32, n: usize, code: &str, flag: Option<&str>, size: usize, t: f32) -> Svgr<'static> {
    let num = format!("{n}");
    let shown = code.trim_start().to_owned();
    let mark = match flag.and_then(|f| shown.find(f).map(|i| (i, f))) {
        Some((i, f)) => {
            let x0 = measure(frame, ctx, MONO, size, 400, false, &shown[..i]);
            let fw = measure(frame, ctx, MONO, size, 400, false, f);
            let k = prog(t, FLAG_AT, FLAG_AT + FLAG_DUR);
            fframes::svgr!(
                <g>
                    <rect x={x + 70.0 + x0 - 4.0} y={y - size as f32 * 0.9} width={fw + 8.0} height={size as f32 * 1.3} rx="3" fill={FLAG_WASH} opacity={k} />
                    {underline(x + 70.0 + x0, y + 7.0, fw, k, 3.0)}
                </g>
            )
        }
        None => Svgr::empty(),
    };
    fframes::svgr!(
        <g>
            {mark}
            <text x={x + 48.0} y={y} text-anchor="end" font-family={MONO} font-size={size - 2} fill={GREY}>{num}</text>
            <text x={x + 70.0} y={y} font-family={MONO} font-size={size} fill={INK}>{shown}</text>
        </g>
    )
}

/// A result table: row labels, then the grok-4.7 and the grok-4.7+modulation
/// cell of each row. Returns the table and its bottom.
pub fn table(frame: &mut Frame, ctx: &FFramesContext, top: f32, caption: &str, rows: &[(&str, &str, &str)]) -> (Svgr<'static>, f32) {
    let size = 28;
    let line = 38.0;
    let cols = [(MX + 20.0, 760.0), (MX + 820.0, 520.0), (MX + 1380.0, 280.0)];
    let mut out = Vec::new();
    let caption = caption.to_owned();
    out.push(fframes::svgr!(
        <text x={MX} y={top} font-family={SERIF} font-size="26" font-style="italic" fill={GREY}>{caption}</text>
    ));
    let head_y = top + 62.0;
    out.push(fframes::svgr!(<rect x={MX} y={head_y - 38.0} width={W - 2.0 * MX} height="56" rx="4" fill={SHEET} />));
    for ((x, _), (h, color)) in cols.iter().zip([("", INK), ("grok-4.7", FLAG), ("grok-4.7+modulation", OK)]) {
        out.push(fframes::svgr!(
            <text x={*x} y={head_y} font-family={SERIF} font-size={size} font-weight="700" fill={color}>{h}</text>
        ));
    }
    out.push(fframes::svgr!(<rect x={MX} y={head_y + 16.0} width={W - 2.0 * MX} height="2" fill={ACCENT} />));
    let mut y = head_y + 30.0;
    for (i, (what, a, b)) in rows.iter().enumerate() {
        let cells = [*what, *a, *b];
        let placed: Vec<Vec<Placed>> = cells.iter().zip(cols).map(|(c, (_, w))| layout(frame, ctx, c, SERIF, size, 400, false, w)).collect();
        let n = placed.iter().map(|p| line_widths(p).len().max(1)).max().unwrap_or(1);
        let row_h = n as f32 * line + 22.0;
        let zebra = if i % 2 == 1 { 0.7 } else { 0.0 };
        out.push(fframes::svgr!(<rect x={MX} y={y} width={W - 2.0 * MX} height={row_h} fill={SHEET} opacity={zebra} />));
        for (c, (cx, w)) in cols.iter().enumerate() {
            let ty = Type::new(SERIF, size, 400, if c == 0 { GREY } else { INK }).width(*w).line_h(line);
            out.push(block(frame, ctx, *cx, y + size as f32 + 6.0, cells[c], ty));
        }
        out.push(fframes::svgr!(<rect x={MX} y={y + row_h - 1.0} width={W - 2.0 * MX} height="1" fill={RULE} />));
        y += row_h;
    }
    (fframes::svgr!(<g>{out}</g>), y)
}
