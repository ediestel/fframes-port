//! 1 · The Setup. The three queries on the left, a bracket into the two
//! conditions on the right: grok-4.7 as a direct call, and grok-4.7 with the
//! modulation inside the model box. Beneath, the settings both calls share and
//! the check every response gets afterwards. Complete from its first frame.

use fframes::{FFramesContext, Frame, Svgr};

use crate::page::*;
use crate::ui::*;

script_scene!(SetupScene, 17.0, 31.0);

/// The three queries: (pair, domain, query).
const QUERIES: [(&str, &str, &str); 3] = [
    ("PAIR 1", "Documents", "What does this document establish?"),
    ("PAIR 2", "Mathematics", "A real 3 × 3 matrix with exactly one eigenvector. | The integral from 0 to ∞ of sin(x²)/x dx."),
    ("PAIR 3", "Robot code", "Python that moves a Spot robot, bosdyn-client 5.2.0."),
];
const Q_X: f32 = MX;
const Q_W: f32 = 760.0;
const Q_TOPS: [f32; 1] = [192.0];
const Q_SIZE: usize = 24;
const Q_LINE: f32 = 32.0;

const BRACKET_X: f32 = 960.0;
const MODEL_X: f32 = 1080.0;
const MODEL_W: f32 = 720.0;
const MODEL_H: f32 = 130.0;
const Y_A: f32 = 340.0;
const Y_B: f32 = 560.0;

const TAGS: &[&str] = &["grok-4.7 via the xAI API", "high reasoning effort", "no web search", "temperature 0.0 preferred"];
const VARIABLE: &str = "Each query is sent twice. Modulation is the only independent variable.";
const CHECK: &str = "Checked afterwards against the source: the document, the mathematics, the published package.";

impl SetupScene {
    fn draw(&self, frame: &mut Frame, ctx: &FFramesContext, _t: f32) -> Svgr<'static> {
        let mut out = Vec::new();
        // the queries, one card each, each as tall as its text
        let mut tops = [0.0_f32; 3];
        let mut hs = [0.0_f32; 3];
        let mut y = Q_TOPS[0];
        for (i, (_, _, q)) in QUERIES.iter().enumerate() {
            let lines = line_widths(&layout(frame, ctx, q, SERIF, Q_SIZE, 400, true, Q_W - 60.0)).len().max(1);
            tops[i] = y;
            hs[i] = 88.0 + (lines - 1) as f32 * Q_LINE + 34.0;
            y += hs[i] + 26.0;
        }
        for (i, (pair, domain, q)) in QUERIES.iter().enumerate() {
            let top = tops[i];
            let h = hs[i];
            let ty = Type::new(SERIF, Q_SIZE, 400, INK).width(Q_W - 60.0).line_h(Q_LINE).italic();
            let body = block(frame, ctx, Q_X + 32.0, top + 88.0, q, ty);
            let head = format!("{pair} · {domain}");
            out.push(fframes::svgr!(
                <g>
                    <rect x={Q_X} y={top} width={Q_W} height={h} rx="6" fill={SHEET} stroke={RULE} stroke-width="1.5" />
                    {label(Q_X + 32.0, top + 42.0, &head, ACCENT, 20.0, "start")}
                    {body}
                </g>
            ));
            // into the bracket
            let mid = top + h / 2.0;
            out.push(fframes::svgr!(<rect x={Q_X + Q_W} y={mid - 1.25} width={BRACKET_X - Q_X - Q_W} height="2.5" fill={INK} />));
        }
        // the bracket into both conditions
        let b_top = tops[0] + hs[0] / 2.0;
        let b_bot = tops[2] + hs[2] / 2.0;
        out.push(fframes::svgr!(<rect x={BRACKET_X - 1.25} y={b_top} width="2.5" height={b_bot - b_top} fill={INK} />));
        let mid = (Y_A + Y_B) / 2.0;
        out.push(fframes::svgr!(<rect x={BRACKET_X} y={mid - 1.25} width="40" height="2.5" fill={INK} />));
        out.push(fframes::svgr!(<rect x={BRACKET_X + 40.0 - 1.25} y={Y_A} width="2.5" height={Y_B - Y_A} fill={INK} />));
        for y in [Y_A, Y_B] {
            out.push(fframes::svgr!(<rect x={BRACKET_X + 40.0} y={y - 1.25} width={MODEL_X - BRACKET_X - 40.0} height="2.5" fill={INK} />));
        }
        // the two conditions: the same model, the modulation inside the second box
        for (y, modulated) in [(Y_A, false), (Y_B, true)] {
            let inner = if modulated {
                let mw = 270.0;
                let mx = MODEL_X + MODEL_W - mw - 28.0;
                fframes::svgr!(
                    <g>
                        <rect x={mx} y={y - 38.0} width={mw} height="76" rx="4" fill={ACCENT} />
                        <text x={mx + mw / 2.0} y={y + 11.0} text-anchor="middle" font-family={SERIF} font-size="32" font-weight="700" fill={PAPER}>"modulation"</text>
                    </g>
                )
            } else {
                Svgr::empty()
            };
            out.push(fframes::svgr!(
                <g>
                    <rect x={MODEL_X} y={y - MODEL_H / 2.0} width={MODEL_W} height={MODEL_H} rx="4" fill={SHEET} stroke={GREY} stroke-width="2" />
                    <text x={MODEL_X + 32.0} y={y + 4.0} font-family={SERIF} font-size="40" font-weight="700" fill={INK}>"grok-4.7"</text>
                    <text x={MODEL_X + 32.0} y={y + 40.0} font-family={SERIF} font-size="24" fill={GREY}>"(xAI)"</text>
                    {inner}
                </g>
            ));
        }
        out.push(block(frame, ctx, MODEL_X, Y_B + 122.0, VARIABLE, Type::new(SERIF, 28, 400, INK).width(MODEL_W).line_h(36.0).italic()));
        // the settings both calls share
        let size = 26;
        let gap = 40.0;
        let widths: Vec<f32> = TAGS.iter().map(|s| measure(frame, ctx, MONO, size, 400, false, s)).collect();
        let total = widths.iter().sum::<f32>() + gap * (TAGS.len() - 1) as f32;
        let mut x = W / 2.0 - total / 2.0;
        let y = 800.0;
        out.push(fframes::svgr!(<rect x={MX} y={y - 46.0} width={W - 2.0 * MX} height="1.5" fill={RULE} />));
        for (i, (tag, w)) in TAGS.iter().zip(&widths).enumerate() {
            let tag = (*tag).to_owned();
            out.push(fframes::svgr!(<text x={x} y={y} font-family={MONO} font-size={size} fill={INK}>{tag}</text>));
            if i + 1 < TAGS.len() {
                out.push(fframes::svgr!(<circle cx={x + w + gap / 2.0} cy={y - 8.0} r="3.5" fill={GREY} />));
            }
            x += w + gap;
        }
        let check = block(frame, ctx, W / 2.0, 900.0, CHECK, Type::new(SERIF, 32, 400, INK).width(1600.0).line_h(42.0).center());
        fframes::svgr!(
            <g>
                {stage()}
                {header(2, "The Setup", 1.0)}
                {out}
                {check}
            </g>
        )
    }
}
