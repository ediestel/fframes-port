//! 1 · The query. The whole poster with the link to the paper, then in one
//! cut the page: the query as the reference line for the two conditions
//! beneath it, grok-4.7 alone and grok-4.7+modulation, and one shared row of
//! settings. Nothing animates: both stand complete from their first frame.

use fframes::{FFramesContext, Frame, Svgr};

use crate::poster::*;
use crate::ui::*;

script_scene!(QueryScene, 2.5, 14.5);

/// The poster gives way to the page here: long enough to read the link.
const POSTER: f32 = 5.0;
/// A time after every build-up of the page: it is drawn as it stands then.
const BUILT: f32 = 99.0;

/// The query, set in front of the two lanes it goes into.
const QUERY: &str = "\u{201C}Write the Python code that returns a Universal Robots UR5e arm to a safe home pose at reduced speed, using the ur_rtde library.\u{201D}";
const Q_SIZE: usize = 30;
const Q_W: f32 = 460.0;
const Q_LINE: f32 = 42.0;
/// The bracket from the query to both lanes.
const BRACKET_X: f32 = 625.0;

const Y_B: f32 = 450.0;
const Y_A: f32 = 640.0;
const LANE_B_AT: f32 = 4.6;
const LANE_A_AT: f32 = 4.8;
const TAGS_AT: f32 = 5.2;
const TAGS: &[&str] = &["grok-4.7", "same provider", "temperature 0.7", "no prior context", "one call each"];

/// One model box per lane; the modulation sits inside the second.
const MODEL_X: f32 = 1180.0;
const MODEL_W: f32 = 540.0;
const MODEL_H: f32 = 120.0;
const METHOD_W: f32 = 250.0;
const METHOD_H: f32 = 76.0;

impl QueryScene {
    fn draw(&self, frame: &mut Frame, ctx: &FFramesContext, t: f32) -> Svgr<'static> {
        let s = if t < POSTER { 0.0 } else { 1.0 };
        let c = s;
        let content = if c > 0.0 { self.content(frame, ctx, BUILT) } else { Svgr::empty() };
        // where to read the full paper, over the poster while it is shown
        let link = 1.0 - s;
        fframes::svgr!(
            <g>
                {poster(ctx, FULL)}
                <g opacity={link}>
                    <rect x="0" y="968" width="1920" height="112" fill={PAPER} opacity="0.92" />
                    <text x={W / 2.0} y="1036" text-anchor="middle" font-family={SERIF} font-size="34" fill={INK}>
                        "Full paper available at: "
                        <tspan font-family={MONO} fill={ACCENT}>"https://github.com/ediestel/llm_modulation_example"</tspan>
                    </text>
                </g>
                {stage(s)}
                {header(1, "The Study", c)}
                <g opacity={c}>{content}</g>
            </g>
        )
    }

    fn content(&self, frame: &mut Frame, ctx: &FFramesContext, t: f32) -> Svgr<'static> {
        // the query in front of the lanes, centred on them, a bracket into both
        let ty = Type::new(SERIF, Q_SIZE, 400, INK).width(Q_W).line_h(Q_LINE);
        let n = line_widths(&layout(frame, ctx, QUERY, SERIF, Q_SIZE, 400, false, Q_W)).len();
        let mid = (Y_B + Y_A) / 2.0;
        let top = mid - (n as f32 - 1.0) * Q_LINE / 2.0 + Q_SIZE as f32 * 0.35;
        let query = block(frame, ctx, MX, top, QUERY, ty);
        let lanes = self.lanes(t);
        let tags = self.tags(frame, ctx, t);
        fframes::svgr!(
            <g>
                {query}
                <rect x={MX + Q_W + 10.0} y={mid - 1.25} width={BRACKET_X - MX - Q_W - 10.0} height="2.5" fill={INK} />
                <rect x={BRACKET_X - 1.25} y={Y_B - 1.25} width="2.5" height={Y_A - Y_B + 2.5} fill={INK} />
                <rect x={BRACKET_X} y={Y_B - 1.25} width="20" height="2.5" fill={INK} />
                <rect x={BRACKET_X} y={Y_A - 1.25} width="20" height="2.5" fill={INK} />
                {lanes}
                {tags}
            </g>
        )
    }

    /// The two conditions: the query into grok-4.7 twice, each call its own
    /// box; in the second the modulation sits inside the model.
    fn lanes(&self, t: f32) -> Svgr<'static> {
        if t < LANE_B_AT {
            return Svgr::empty();
        }
        let wire_b = ease_out(prog(t, LANE_B_AT + 0.6, LANE_B_AT + 1.6));
        let wire_a = ease_out(prog(t, LANE_A_AT + 0.6, LANE_A_AT + 1.6));
        // the wires run on from the bracket's arms
        let chip_end = BRACKET_X + 20.0;
        let wire = |x0: f32, x1: f32, y: f32, k: f32| {
            fframes::svgr!(<rect x={x0} y={y - 1.25} width={((x1 - x0) * k).max(0.5)} height="2.5" fill={INK} opacity={prog(k, 0.0, 0.05)} />)
        };
        let model = |y: f32, k: f32, modulated: bool| {
            let inner = if modulated {
                let mx = MODEL_X + MODEL_W - METHOD_W - 24.0;
                fframes::svgr!(
                    <g>
                        <rect x={mx} y={y - METHOD_H / 2.0} width={METHOD_W} height={METHOD_H} rx="4" fill={ACCENT} />
                        <text x={mx + METHOD_W / 2.0} y={y + 11.0} text-anchor="middle" font-family={SERIF} font-size="32"
                              font-weight="700" fill={PAPER}>"modulation"</text>
                    </g>
                )
            } else {
                Svgr::empty()
            };
            fframes::svgr!(
                <g opacity={k}>
                    <rect x={MODEL_X} y={y - MODEL_H / 2.0} width={MODEL_W} height={MODEL_H} rx="4" fill={SHEET} stroke={GREY} stroke-width="2" />
                    <text x={MODEL_X + 32.0} y={y + 4.0} font-family={SERIF} font-size="38" font-weight="700" fill={INK}>"grok-4.7"</text>
                    <text x={MODEL_X + 32.0} y={y + 38.0} font-family={SERIF} font-size="24" fill={GREY}>"(xAI)"</text>
                    {inner}
                </g>
            )
        };
        fframes::svgr!(
            <g>
                {wire(chip_end, MODEL_X, Y_B, wire_b)}
                {wire(chip_end, MODEL_X, Y_A, wire_a)}
                {model(Y_B, ease_out(prog(t, LANE_B_AT + 0.4, LANE_B_AT + 1.1)), false)}
                {model(Y_A, ease_out(prog(t, LANE_A_AT + 0.4, LANE_A_AT + 1.1)), true)}
            </g>
        )
    }

    /// One shared row beneath both lanes: what the two calls had in common.
    fn tags(&self, frame: &mut Frame, ctx: &FFramesContext, t: f32) -> Svgr<'static> {
        if t < TAGS_AT {
            return Svgr::empty();
        }
        let size = 32;
        let gap = 46.0;
        let widths: Vec<f32> = TAGS.iter().map(|s| measure(frame, ctx, MONO, size, 400, false, s)).collect();
        let total = widths.iter().sum::<f32>() + gap * (TAGS.len() - 1) as f32;
        let mut x = W / 2.0 - total / 2.0;
        let y = 862.0;
        let rule = ease_out(prog(t, TAGS_AT, TAGS_AT + 0.8));
        let mut out = vec![fframes::svgr!(
            <rect x={W / 2.0 - total / 2.0 * rule - 20.0} y={y - 54.0} width={(total * rule + 40.0).max(0.5)} height="1.5" fill={RULE} />
        )];
        for (i, (tag, w)) in TAGS.iter().zip(&widths).enumerate() {
            let at = TAGS_AT + 0.1 + i as f32 * 0.08;
            let k = ease_out(prog(t, at, at + 0.5));
            let tag = (*tag).to_owned();
            out.push(fframes::svgr!(
                <text x={x} y={y + (1.0 - k) * 12.0} font-family={MONO} font-size={size} fill={INK} opacity={k}>{tag}</text>
            ));
            if i + 1 < TAGS.len() {
                out.push(fframes::svgr!(
                    <circle cx={x + w + gap / 2.0} cy={y - 9.0} r="3.5" fill={GREY} opacity={k} />
                ));
            }
            x += w + gap;
        }
        fframes::svgr!(<g>{out}</g>)
    }
}
