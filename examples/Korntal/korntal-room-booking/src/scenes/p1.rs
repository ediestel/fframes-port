//! Scene 1 · 00:00–00:15 · Opening: the question, the credit, and the overview
//! of the loop (input folder → tokens → prompts → LLM → matched locally →
//! output folder).

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::ui::*;

pub fn draw<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let lift = ease_in_out(prog(t, 6.2, 7.4));
    let qy = lerp(440.0, 226.0, lift);
    let qs = lerp(1.0, 0.62, lift);
    let l1 = enter(t, 0.3, 0.8);
    let l2 = enter(t, 0.65, 0.8);
    let rule = enter(t, 3.8, 0.7);
    let credit = enter(t, 4.1, 0.8);
    let question = fframes::svgr!(
        <g transform={trs(960.0, qy, qs)}>
            <g opacity={l1} transform={tr(0.0, (1.0 - l1) * 34.0)}>
                <text x="0" y="0" font-family={SANS} font-size="92" font-weight="500" fill={INK} text-anchor="middle" letter-spacing="-1.5">"How can an LLM help with room"</text>
            </g>
            <g opacity={l2} transform={tr(0.0, (1.0 - l2) * 34.0)}>
                <text x="0" y="112" font-family={SANS} font-size="92" font-weight="500" fill={INK} text-anchor="middle" letter-spacing="-1.5">"management and booking?"</text>
            </g>
            <rect x={-70.0 * rule} y="176" width={(140.0 * rule).max(0.5)} height="4" rx="2" fill="url(#amberBar)" opacity={rule} />
            <g opacity={credit} transform={tr(0.0, (1.0 - credit) * 16.0)}>
                <text x="0" y="250" font-family={SANS} font-size="40" font-weight="400" fill={MUTED} text-anchor="middle">"Video and service offered by "<tspan font-weight="500" fill={INK}>"AMDG Engineers"</tspan></text>
            </g>
        </g>
    );

    // the overview
    let z = show(
        enter(t, 7.0, 0.7),
        0.0,
        fframes::svgr!(
            <g>
                <rect x={M} y="410" width="1130" height="440" rx="22" fill={BLUE_BG} opacity="0.55" stroke={BLUE} stroke-opacity="0.35" stroke-width="1.5" />
                <rect x="1266" y="410" width="558" height="440" rx="22" fill={VIOLET_BG} opacity="0.6" stroke={VIOLET} stroke-opacity="0.35" stroke-width="1.5" />
                <text x="124" y="450" font-family={SANS} font-size="24" font-weight="500" fill={BLUE}>"Korntal — all data stay here"</text>
                <text x="1294" y="450" font-family={SANS} font-size="24" font-weight="500" fill={VIOLET}>"LLM service"</text>
            </g>
        ),
    );
    let item = |at: f32, body: Svgr<'static>| rise_in(t, at, body);
    let input = item(
        7.6,
        fframes::svgr!(<g>{icon_folder(150.0, 500.0, 170.0, 120.0, BLUE, BLUE_BG)}{text(235.0, 655.0, 24.0, 500, INK, "middle", "Input folder")}{text(235.0, 685.0, 19.0, 400, MUTED, "middle", "request · DB export")}</g>),
    );
    let tokens = item(
        8.4,
        fframes::svgr!(<g>{lock(560.0, 560.0, 64.0, 1.0, BLUE)}{text(560.0, 655.0, 24.0, 500, INK, "middle", "Tokens")}{text(560.0, 685.0, 19.0, 400, MUTED, "middle", "key stays local")}</g>),
    );
    let prompts = item(
        9.2,
        fframes::svgr!(<g>{icon_doc(860.0, 500.0, 110.0, AMBER)}{text(905.0, 655.0, 24.0, 500, INK, "middle", "Prompt series")}{text(905.0, 685.0, 19.0, 400, MUTED, "middle", "editable text")}</g>),
    );
    let llm = item(10.0, llm_node(1545.0, 600.0, 66.0, t, 0.5 + 0.5 * (t * 1.6).sin(), ""));
    let output = item(
        11.6,
        fframes::svgr!(<g>{icon_folder(150.0, 720.0, 170.0, 110.0, GREEN, GREEN_BG)}{text(400.0, 770.0, 24.0, 500, INK, "start", "Output folder")}{text(400.0, 800.0, 19.0, 400, MUTED, "start", "reply · database record · audit log")}</g>),
    );
    let a = |x1: f32, y1: f32, x2: f32, y2: f32, at: f32, c: &str| arrow(x1, y1, x2, y2, prog(t, at, at + 0.6), c, 3.0);
    let flows = fframes::svgr!(
        <g>
            {a(330.0, 560.0, 510.0, 560.0, 8.2, BLUE)}
            {a(612.0, 560.0, 840.0, 560.0, 9.0, BLUE)}
            {a(960.0, 560.0, 1466.0, 590.0, 10.4, AMBER)}
            {a(1500.0, 660.0, 700.0, 780.0, 11.0, VIOLET)}
            {a(690.0, 780.0, 336.0, 780.0, 11.8, GREEN)}
        </g>
    );
    let dots = if t > 11.0 {
        fframes::svgr!(<g>
            {flow_dot(330.0, 560.0, 510.0, 560.0, t, 2.0, 0.0, BLUE)}
            {flow_dot(960.0, 560.0, 1466.0, 590.0, t, 2.6, 0.3, AMBER)}
            {flow_dot(1500.0, 660.0, 700.0, 780.0, t, 2.6, 0.6, VIOLET)}
        </g>)
    } else {
        Svgr::empty()
    };

    fframes::svgr!(<g>{question}{z}{flows}{dots}{input}{tokens}{prompts}{llm}{output}</g>)
}
