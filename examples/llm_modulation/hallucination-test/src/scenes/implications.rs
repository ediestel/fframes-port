//! 7 · The implications: six numbered points, each a bold claim with its
//! explanation beneath, verbatim, appearing one after another and staying.
//! The film ends here.

use fframes::{FFramesContext, Frame, Svgr};

use crate::poster::*;
use crate::ui::*;

script_scene!(ImplicationsScene, 190.5, 275.5);

/// (claim, explanation)
const POINTS: [(&str, &str); 6] = [
    (
        "Hallucination is a controllable behaviour, not a fixed limit of the model.",
        "grok-4.7 produced both programs, so the model already has what it needs to tell what is specified from what is assumed. Called directly, it doesn\u{2019}t use it. The modulation brings it out. This shifts the problem: reliability is no longer only a matter of training a better model; it can be engineered at the layer between the query and the model.",
    ),
    (
        "Reliability becomes a property of the system, not of the model.",
        "What should be specified, tested and certified is model plus modulation, because that is the unit that behaves. A model\u{2019}s hallucination rate quoted without its operating conditions says nothing about how it will behave in deployment.",
    ),
    (
        "The control layer is where the value sits.",
        "If one model can be moved from three fabricated parameters to none without retraining, then whoever controls the modulation controls the reliability. It works on models that are already deployed, and it costs nothing in compute.",
    ),
    (
        "It carries over to every domain where unspecified parameters must not be filled silently.",
        "A missing drug dose, a missing account number, a missing network address or a missing load limit is the same failure as the missing robot IP. In medicine, a model that fills in a dose the clinician didn\u{2019}t give has made a clinical decision without authority. A modulation that hands the value back to the human is the remedy there too.",
    ),
    (
        "For autonomous agents, this is the core issue.",
        "An agent acts on its own outputs. A faithfulness hallucination in an agent is an unauthorised decision that gets executed. A modulation that makes the model defer what it cannot know builds human-in-the-loop control into the model\u{2019}s behaviour, instead of relying on policies and checks layered on top.",
    ),
    (
        "The test becomes an acceptance criterion.",
        "\u{201C}Every literal traced to the query, the library or an explicit input\u{201D} is a mechanical check, and it can be run on any generated artefact. Together with the modulation, it turns an elusive notion, \u{201C}hallucination\u{201D}, into something you can specify and verify.",
    ),
];

/// The key words or word pairs of each point, verbatim: the reading stops
/// at each and lays a wash behind it that stays.
const KEYS: [&[&str]; 6] = [
    &["controllable behaviour", "engineered at the layer"],
    &["property of the system", "model plus modulation"],
    &["control layer", "without retraining"],
    &["every domain", "without authority"],
    &["autonomous agents", "human-in-the-loop control"],
    &["acceptance criterion", "specify and verify"],
];
/// The imagined reading of the whole page, point after point: seconds per
/// word, the stop at a key, the pause between points, when it starts.
const WORD_T: f32 = 0.2;
const KEY_STOP: f32 = 0.6;
const POINT_PAUSE: f32 = 0.4;
const PAGE_IN: f32 = 0.2;
const READ_START: f32 = 1.5;

/// The wash behind a key: FLAG at 0.38 over PAPER, solid, so washes of
/// neighbouring words do not darken where they overlap.
const KEY_WASH: &str = "#603F24";

fn norm(word: &str) -> String {
    word.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase()
}

/// The reading of one point from `start`: for every word whether it is
/// being read now, and how far the wash behind it is in (0 when it is no
/// key). Also returns when the reading of the point ends.
fn reading(words: &[String], keys: &[&str], start: f32, t: f32) -> (Vec<(bool, f32)>, f32) {
    let keys: Vec<Vec<String>> = keys.iter().map(|k| k.split(' ').map(norm).collect()).collect();
    let mut marks = vec![(false, 0.0); words.len()];
    let mut clock = start;
    let mut j = 0;
    while j < words.len() {
        let key = keys
            .iter()
            .find(|k| words.len() - j >= k.len() && k.iter().zip(&words[j..]).all(|(a, b)| a == b));
        let n = key.map_or(1, |k| k.len());
        for i in j..j + n {
            // it stays on the last word while the reading stops at a key
            let until = if key.is_some() && i == j + n - 1 { clock + WORD_T + KEY_STOP } else { clock + WORD_T };
            marks[i].0 = t >= clock && t < until;
            clock += WORD_T;
        }
        if key.is_some() {
            // the wash comes in behind the key while the reading stops there
            let p = ease_out(prog(t, clock, clock + KEY_STOP));
            for m in &mut marks[j..j + n] {
                m.1 = p;
            }
            clock += KEY_STOP;
        }
        j += n;
    }
    (marks, clock)
}

/// A paragraph set one `<text>` per line like `text_lines`, every word its
/// own `<tspan>`, so the font places each word and its marks with it: the
/// word being read turns FLAG, a key gets a wide stroke of its own glyphs
/// in a copy of the line drawn underneath. Returns (washes, text).
fn paragraph(
    placed: &[Placed],
    marks: &[(bool, f32)],
    x: f32,
    y: f32,
    ty: Type,
    at: f32,
    t: f32,
) -> (Vec<Svgr<'static>>, Vec<Svgr<'static>>) {
    let mut washes = Vec::new();
    let mut texts = Vec::new();
    if t < at {
        return (washes, texts);
    }
    let n = placed.iter().map(|p| p.line).max().map_or(0, |l| l + 1);
    for line in 0..n {
        let start = at + line as f32 * 0.15;
        let k = ease_out(prog(t, start, start + 0.5));
        if k <= 0.0 {
            continue;
        }
        let ly = y + line as f32 * ty.line_h + (1.0 - k) * 10.0;
        let on_line: Vec<usize> = (0..placed.len()).filter(|&i| placed[i].line == line).collect();
        let mut wash = Vec::new();
        let mut text = Vec::new();
        for (m, &i) in on_line.iter().enumerate() {
            let (read, p) = marks[i];
            let word = placed[i].word.clone();
            let stroke = if p > 0.0 { KEY_WASH } else { "none" };
            // the word being read, and a key once its wash comes in, in FLAG
            let fill = if read || p > 0.0 { FLAG } else { ty.fill };
            wash.push(fframes::svgr!(<tspan stroke={stroke} stroke-opacity={p}>{word.clone()}</tspan>));
            text.push(fframes::svgr!(<tspan fill={fill}>{word}</tspan>));
            if m + 1 < on_line.len() {
                wash.push(fframes::svgr!(<tspan>{" "}</tspan>));
                text.push(fframes::svgr!(<tspan>{" "}</tspan>));
            }
        }
        let sw = ty.size as f32 * 0.5;
        washes.push(fframes::svgr!(
            <text x={x} y={ly} font-family={ty.family} font-size={ty.size} font-weight={ty.weight} fill="none"
                  stroke-width={sw} stroke-linejoin="round" stroke-linecap="round" opacity={k}>{wash}</text>
        ));
        texts.push(fframes::svgr!(
            <text x={x} y={ly} font-family={ty.family} font-size={ty.size} font-weight={ty.weight}
                  fill={ty.fill} opacity={k}>{text}</text>
        ));
    }
    (washes, texts)
}

const CLAIM_SIZE: usize = 28;
const CLAIM_LINE: f32 = 36.0;
const BODY_SIZE: usize = 24;
const BODY_LINE: f32 = 31.0;
const GAP: f32 = 16.0;
const WIDTH: f32 = 1640.0;

impl ImplicationsScene {
    fn draw(&self, frame: &mut Frame, ctx: &FFramesContext, t: f32) -> Svgr<'static> {
        let c = prog(t, 0.0, 0.7) * (1.0 - prog(t, Self::LEN - 1.2, Self::LEN - 0.2));
        let claim_ty = Type::new(SERIF, CLAIM_SIZE, 700, ACCENT).width(WIDTH).line_h(CLAIM_LINE);
        let body_ty = Type::new(SERIF, BODY_SIZE, 400, INK).width(WIDTH).line_h(BODY_LINE);
        let mut out = Vec::new();
        let mut under = Vec::new();
        let mut y = 196.0;
        // the whole page at once, then read through point after point
        let k = ease_out(prog(t, PAGE_IN, PAGE_IN + 0.6));
        let mut clock = READ_START;
        for (i, (claim, body)) in POINTS.iter().enumerate() {
            let num = format!("{}.", i + 1);
            out.push(fframes::svgr!(
                <text x={MX} y={y} font-family={SERIF} font-size={CLAIM_SIZE} font-weight="700" fill={ACCENT} opacity={k}>{num}</text>
            ));
            let claim_placed = layout(frame, ctx, claim, SERIF, CLAIM_SIZE, 700, false, WIDTH);
            let claim_lines = line_widths(&claim_placed).len();
            let body_y = y + claim_lines as f32 * CLAIM_LINE;
            let body_placed = layout(frame, ctx, body, SERIF, BODY_SIZE, 400, false, WIDTH);
            let body_lines = line_widths(&body_placed).len();
            // the imagined reading of claim and explanation, with its highlights
            let norms: Vec<String> = claim_placed.iter().chain(&body_placed).map(|p| norm(&p.word)).collect();
            let (marks, end) = reading(&norms, KEYS[i], clock, t);
            let (claim_marks, body_marks) = marks.split_at(claim_placed.len());
            for (placed, marks, py, ty) in
                [(&claim_placed, claim_marks, y, claim_ty), (&body_placed, body_marks, body_y, body_ty)]
            {
                let (washes, texts) = paragraph(placed, marks, MX + 44.0, py, ty, PAGE_IN, t);
                under.extend(washes);
                out.extend(texts);
            }
            clock = end + POINT_PAUSE;
            y = body_y + (body_lines as f32 - 1.0) * BODY_LINE + GAP + CLAIM_LINE;
        }
        // highlights and the reading bar lie under the text
        fframes::svgr!(
            <g>
                {stage(1.0)}
                {header(7, "The Implications", c)}
                <g opacity={c}>{under}{out}</g>
            </g>
        )
    }
}
