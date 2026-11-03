//! 3 · The evidence: the three hallucinated values of grok-4.7's program,
//! each underlined, beside what grok-4.7+modulation does instead and the
//! type of hallucination, as on the poster. Complete from its first frame:
//! nothing on it animates.

use fframes::{FFramesContext, Frame, Svgr};

use crate::poster::*;
use crate::ui::*;

script_scene!(ResultScene, 23.0, 43.0);

/// A hallucinated value: grok-4.7's cell is the literal (set in mono,
/// underlined) and the rest; then grok-4.7+modulation, then the type.
struct Row {
    what: &'static str,
    literal: &'static str,
    rest: &'static str,
    modulated: &'static str,
    kind: &'static str,
}

const ROWS: &[Row] = &[
    Row {
        what: "Robot address",
        literal: "\"192.168.1.10\"",
        rest: "(line 14); the program connects to it.",
        modulated: "None: the operator must supply it.",
        kind: "factuality hallucination",
    },
    Row {
        what: "Home pose",
        literal: "[0.0, -pi/2, 0.0, -pi/2, 0.0, 0.0]",
        rest: "(line 18), called safe (line 1) without evidence of safety for the installation; the arm is moved to it.",
        modulated: "None: the operator must supply one, verified collision-free for this mounting, tool, and cell.",
        kind: "overclaim",
    },
    Row {
        what: "Position tolerance",
        literal: "0.02 rad",
        rest: "(line 25), for a check the query does not ask for.",
        modulated: "None.",
        kind: "instruction inconsistency",
    },
];

/// Nothing lands on this page, so it has no tones.
pub fn tones() -> Vec<f32> {
    Vec::new()
}

const SIZE: usize = 30;
const LINE: f32 = 40.0;
/// Column x and width: what, grok-4.7, grok-4.7+modulation, type.
const COLS: [(f32, f32); 4] = [(120.0, 250.0), (400.0, 620.0), (1060.0, 460.0), (1560.0, 260.0)];
const HEAD_Y: f32 = 196.0;
const TOP: f32 = 250.0;

impl ResultScene {
    fn draw(&self, frame: &mut Frame, ctx: &FFramesContext, _t: f32) -> Svgr<'static> {
        let table = self.table(frame, ctx);
        fframes::svgr!(
            <g>
                {stage(1.0)}
                {header(3, "The Evidence", 1.0)}
                {table}
            </g>
        )
    }

    fn table(&self, frame: &mut Frame, ctx: &FFramesContext) -> Svgr<'static> {
        let mut out = Vec::new();
        let heads = [("", INK), ("grok-4.7", FLAG), ("grok-4.7+modulation", OK), ("Hallucination", INK)];
        out.push(fframes::svgr!(<rect x="100" y={HEAD_Y - 38.0} width="1720" height="56" rx="4" fill={SHEET} />));
        for ((x, _), (h, color)) in COLS.iter().zip(heads) {
            out.push(fframes::svgr!(
                <text x={*x} y={HEAD_Y} font-family={SERIF} font-size={SIZE} font-weight="700" fill={color}>{h}</text>
            ));
        }
        out.push(fframes::svgr!(<rect x="100" y={HEAD_Y + 16.0} width="1720" height="2" fill={ACCENT} />));
        let mut y = TOP;
        for (i, row) in ROWS.iter().enumerate() {
            let cells = [
                layout(frame, ctx, row.what, SERIF, SIZE, 400, false, COLS[0].1),
                layout(frame, ctx, row.rest, SERIF, SIZE, 400, false, COLS[1].1),
                layout(frame, ctx, row.modulated, SERIF, SIZE, 400, false, COLS[2].1),
                layout(frame, ctx, row.kind, SERIF, SIZE, 400, true, COLS[3].1),
            ];
            let lines = |p: &Vec<Placed>| p.iter().map(|w| w.line).max().map_or(1, |l| l + 1);
            // grok-4.7's cell starts with the literal on a line of its own
            let n = lines(&cells[0]).max(lines(&cells[1]) + 1).max(lines(&cells[2])).max(lines(&cells[3]));
            let row_h = n as f32 * LINE + 30.0;
            let base = y + SIZE as f32 + 10.0;
            let mut cell_svgs = Vec::new();
            for (c, placed) in cells.iter().enumerate() {
                let (cx, _) = COLS[c];
                let shift = if c == 1 { LINE } else { 0.0 };
                // the type of hallucination in the signal orange, italic
                let (fill, style) = if c == 3 { (FLAG, "italic") } else { (INK, "normal") };
                // one <text> per line, so the font spaces the words
                let n = placed.iter().map(|p| p.line).max().map_or(0, |l| l + 1);
                for line in 0..n {
                    let text = placed.iter().filter(|p| p.line == line).map(|p| p.word.as_str()).collect::<Vec<_>>().join(" ");
                    cell_svgs.push(fframes::svgr!(
                        <text x={cx} y={base + shift + line as f32 * LINE} font-family={SERIF} font-size={SIZE} font-style={style} fill={fill}>{text}</text>
                    ));
                }
            }
            // the hallucinated literal: mono, washed, underlined
            let mark = {
                let lw = measure(frame, ctx, MONO, SIZE + 2, 400, false, row.literal);
                let (lx, ly) = (COLS[1].0, base);
                let text = row.literal.to_owned();
                fframes::svgr!(
                    <g>
                        <rect x={lx - 10.0} y={ly - 30.0} width={lw + 20.0} height="44" rx="4" fill={FLAG_WASH} />
                        <text x={lx} y={ly} font-family={MONO} font-size={SIZE + 2} fill={FLAG}>{text}</text>
                        {underline(lx, ly + 9.0, lw, 1.0, 3.5)}
                    </g>
                )
            };
            // alternate rows sit on a panel
            let zebra = if i % 2 == 1 { 0.7 } else { 0.0 };
            out.push(fframes::svgr!(
                <g>
                    <rect x="100" y={y} width="1720" height={row_h} fill={SHEET} opacity={zebra} />
                    {cell_svgs}
                    {mark}
                    <rect x="120" y={y + row_h - 1.0} width="1720" height="1" fill={RULE} />
                </g>
            ));
            y += row_h;
        }
        fframes::svgr!(<g>{out}</g>)
    }
}
