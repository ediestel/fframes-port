//! 2 · The outcome. The literal values of grok-4.7's program ride a rail
//! with their line numbers to one check against the poster's definitions.
//! An accepted value drops into the "Accepted" panel and gets a tick; a
//! hallucinated one runs off the end into the "Hallucinated" panel, is
//! underlined and shows its type. The count, 0 and 3, closes the scene.

use fframes::{FFramesContext, Frame, Svgr};

use crate::poster::*;
use crate::ui::*;

script_scene!(TestScene, 14.5, 23.0);

/// The one check on the rail.
const CHECK: &str = "checked against the definitions";
const CHECK_X: f32 = 960.0;
const RAIL_Y: f32 = 290.0;
const RAIL_X0: f32 = 160.0;
const RAIL_END: f32 = 1680.0;
const GAP: f32 = 150.0;
const CHIP_SIZE: usize = 30;
const CHIP_H: f32 = 52.0;

/// The two panels the literals end in: x and width.
const PANEL_TOP: f32 = 450.0;
const PANEL_BOTTOM: f32 = 870.0;
const ACCEPTED: (f32, f32) = (MX, 780.0);
const HALLUCINATED: (f32, f32) = (960.0, 840.0);
/// Chip centre lines inside a panel.
const ROWS_Y: [f32; 3] = [580.0, 680.0, 780.0];

/// A literal of grok-4.7's program: its text, its line, the type of
/// hallucination it is (none for an accepted value), when it sets off.
struct Literal {
    text: &'static str,
    line: usize,
    kind: Option<&'static str>,
    at: f32,
}

const LITERALS: &[Literal] = &[
    Literal { text: "0.25", line: 21, kind: None, at: 0.8 },
    Literal { text: "7", line: 41, kind: None, at: 1.6 },
    Literal { text: "\"192.168.1.10\"", line: 14, kind: Some("factuality hallucination"), at: 2.4 },
    Literal { text: "[0.0, -pi/2, 0.0, -pi/2, 0.0, 0.0]", line: 18, kind: Some("overclaim"), at: 3.2 },
    Literal { text: "0.02", line: 25, kind: Some("instruction inconsistency"), at: 4.0 },
];

const MOVE: f32 = 0.5;
const PAUSE: f32 = 0.1;
const RUN_OFF: f32 = 0.4;
const FALL: f32 = 0.3;
const DROP: f32 = 0.3;


/// Where a literal is: centre x, centre y, how far it is flagged (1) or
/// accepted (-1), and how far it has settled (for the tick or the underline).
struct Pose {
    x: f32,
    y: f32,
    verdict: f32,
    settled: f32,
}

impl Literal {
    /// Time the literal leaves the check (after its pause there).
    fn leaves(&self) -> f32 {
        self.at + MOVE + PAUSE
    }

    /// When a hallucinated literal lands in its panel.
    fn lands(&self) -> f32 {
        self.leaves() + RUN_OFF + FALL
    }

    /// `slot` is the literal's row in its panel.
    fn pose(&self, t: f32, w: f32, slot: usize) -> Option<Pose> {
        if t < self.at {
            return None;
        }
        let y_rail = RAIL_Y - CHIP_H / 2.0 - 6.0;
        let x0 = RAIL_X0 + w / 2.0;
        if t < self.at + MOVE {
            return Some(Pose { x: lerp(x0, CHECK_X, ease_in_out(prog(t, self.at, self.at + MOVE))), y: y_rail, verdict: 0.0, settled: 0.0 });
        }
        let x = CHECK_X;
        let out = self.leaves();
        if t < out {
            return Some(Pose { x, y: y_rail, verdict: 0.0, settled: 0.0 });
        }
        if self.kind.is_none() {
            // down from the check into the accepted panel
            let k = prog(t, out, out + DROP);
            let tx = ACCEPTED.0 + 40.0 + w / 2.0;
            return Some(Pose {
                x: lerp(x, tx, ease_in_out(k)),
                y: lerp(y_rail, ROWS_Y[slot], ease_in_out(k)),
                verdict: -prog(t, out + DROP * 0.6, out + DROP + 0.2),
                settled: prog(t, out + DROP, out + DROP + 0.5),
            });
        }
        // off the end of the rail, short of the frame edge for a long literal
        let end_x = (RAIL_END + 20.0).min(W - 40.0 - w / 2.0);
        if t < out + RUN_OFF {
            let k = ease_in(prog(t, out, out + RUN_OFF));
            return Some(Pose { x: lerp(x, end_x, k), y: y_rail, verdict: 0.0, settled: 0.0 });
        }
        // straight down to its row, then left into place, clear of the literals already there
        let k = prog(t, out + RUN_OFF, out + RUN_OFF + FALL);
        let tx = HALLUCINATED.0 + 40.0 + w / 2.0;
        Some(Pose {
            x: lerp(end_x, tx, ease_in_out(k * 2.0 - 1.0)),
            y: lerp(y_rail, ROWS_Y[slot], ease_in_out(k * 2.0)),
            verdict: prog(t, out + RUN_OFF, out + RUN_OFF + 0.4),
            settled: prog(t, self.lands(), self.lands() + 0.45),
        })
    }
}

/// Seconds (scene time) at which a hallucinated literal is underlined.
pub fn tones() -> Vec<f32> {
    LITERALS.iter().filter(|l| l.kind.is_some()).map(|l| l.lands()).collect()
}

impl TestScene {
    fn draw(&self, frame: &mut Frame, ctx: &FFramesContext, t: f32) -> Svgr<'static> {
        let s = window(t, 0.0, Self::LEN, 0.3);
        let content = if s > 0.0 { self.content(frame, ctx, t) } else { Svgr::empty() };
        fframes::svgr!(
            <g>
                {stage(1.0)}
                {header(2, "The Outcome", s)}
                <g opacity={s}>{content}</g>
            </g>
        )
    }

    fn content(&self, frame: &mut Frame, ctx: &FFramesContext, t: f32) -> Svgr<'static> {
        let mut out = Vec::new();
        // the two panels the literals end in, each shown as its first literal heads for it
        let first_drop = LITERALS.iter().find(|l| l.kind.is_none()).map_or(99.0, |l| l.leaves());
        let first_fall = LITERALS.iter().find(|l| l.kind.is_some()).map_or(99.0, |l| l.leaves());
        for ((x, w), name, note, color, at) in [
            (ACCEPTED, "ACCEPTED", "generated within the query constraints", OK, first_drop - 0.4),
            (HALLUCINATED, "HALLUCINATED", "generated outside the query constraints", FLAG, first_fall - 0.4),
        ] {
            let k = ease_out(prog(t, at, at + 0.6));
            out.push(fframes::svgr!(
                <g opacity={k}>
                    <rect x={x} y={PANEL_TOP} width={w} height={PANEL_BOTTOM - PANEL_TOP} rx="6" fill={SHEET} stroke={RULE} stroke-width="1.5" />
                    <rect x={x} y={PANEL_TOP} width="5" height={PANEL_BOTTOM - PANEL_TOP} fill={color} />
                    {label(x + 40.0, PANEL_TOP + 50.0, name, color, 24.0, "start")}
                    <text x={x + w - 30.0} y={PANEL_TOP + 50.0} text-anchor="end" font-family={SERIF} font-size="27"
                          font-style="italic" fill={GREY}>{note}</text>
                </g>
            ));
        }
        // the count beneath the panels once the last literal has landed:
        // 0 for grok-4.7+modulation on the left, 3 for grok-4.7 on the right
        let done = LITERALS.iter().filter(|l| l.kind.is_some()).map(|l| l.lands()).fold(0.0, f32::max) + 0.3;
        let k = ease_out(prog(t, done, done + 0.3));
        for ((x, _), n, color, name) in [
            (ACCEPTED, "0", OK, "grok-4.7+modulation"),
            (HALLUCINATED, "3", FLAG, "grok-4.7"),
        ] {
            out.push(fframes::svgr!(
                <g opacity={k}>
                    <text x={x + 40.0} y="1012" font-family={SERIF} font-size="120" font-weight="700" fill={color}>{n}</text>
                    <text x={x + 130.0} y="962" font-family={SERIF} font-size="30" fill={INK}>{name}</text>
                    <text x={x + 130.0} y="1002" font-family={SERIF} font-size="26" font-style="italic" fill={GREY}>"hallucinated literal values"</text>
                </g>
            ));
        }
        // the rail, drawn on, broken at the one check
        let rail = ease_out(prog(t, 0.1, 0.6));
        let reach = lerp(RAIL_X0, RAIL_END, rail);
        for (a, b) in [(RAIL_X0, CHECK_X - GAP / 2.0), (CHECK_X + GAP / 2.0, RAIL_END)] {
            let b = b.min(reach);
            if b > a {
                out.push(fframes::svgr!(<rect x={a} y={RAIL_Y - 2.0} width={b - a} height="4" fill={GREY} />));
            }
        }
        // the check: a mark either side of the break, its name beneath; it lights while a literal is at it
        let a = ease_out(prog(t, 0.2, 0.6));
        let testing = LITERALS
            .iter()
            .map(|l| window(t, l.at + MOVE - 0.1, l.leaves() + 0.1, 0.1))
            .fold(0.0, f32::max);
        let fill = mix(GREY, INK, testing);
        let post = mix(GREY, ACCENT, testing);
        out.push(fframes::svgr!(
            <g opacity={a} transform={format!("translate(0 {})", (1.0 - a) * 16.0)}>
                <rect x={CHECK_X - GAP / 2.0 - 2.0} y={RAIL_Y - 20.0} width="4" height="40" fill={post.clone()} />
                <rect x={CHECK_X + GAP / 2.0 - 2.0} y={RAIL_Y - 20.0} width="4" height="40" fill={post} />
                <text x={CHECK_X} y={RAIL_Y + 62.0} text-anchor="middle" font-family={SERIF} font-size="32" font-style="italic" fill={fill}>{CHECK}</text>
            </g>
        ));
        // the end of the rail
        if rail >= 1.0 {
            out.push(fframes::svgr!(<rect x={RAIL_END - 2.0} y={RAIL_Y - 20.0} width="4" height="40" fill={GREY} />));
        }
        // each panel fills from its top row
        let (mut accepted, mut hallucinated) = (0, 0);
        for lit in LITERALS {
            let w = measure(frame, ctx, MONO, CHIP_SIZE, 400, false, lit.text) + 36.0;
            let slot = if lit.kind.is_none() { &mut accepted } else { &mut hallucinated };
            let my_slot = *slot;
            *slot += 1;
            let Some(p) = lit.pose(t, w, my_slot) else { continue };
            out.push(self.chip(lit, &p, w, t));
        }
        fframes::svgr!(<g>{out}</g>)
    }

    fn chip(&self, lit: &Literal, p: &Pose, w: f32, t: f32) -> Svgr<'static> {
        let enter = ease_out(prog(t, lit.at, lit.at + 0.35));
        let red = p.verdict.max(0.0);
        let green = (-p.verdict).max(0.0);
        let stroke = if red > 0.0 { mix(GREY, FLAG, red) } else { mix(GREY, OK, green) };
        let fill = if red > 0.0 { mix(SHEET, FLAG_WASH, red) } else { mix(SHEET, OK_WASH, green) };
        let text_fill = mix(INK, FLAG, red);
        let x = p.x - w / 2.0;
        let y = p.y - CHIP_H / 2.0;
        let line = format!("line {}", lit.line);
        let text = lit.text.to_owned();
        // an accepted value gets a tick; a hallucinated one its underline and, beside it, its type
        let mark = match lit.kind {
            None => tick(p.x + w / 2.0 + 34.0, p.y, 34.0, p.settled),
            Some(kind) => {
                let kind = format!("({kind})");
                fframes::svgr!(
                    <g>
                        {underline(x, y + CHIP_H + 8.0, w, p.settled, 4.0)}
                        <text x={x + w + 20.0} y={p.y + 9.0} font-family={SERIF} font-size="26" font-style="italic"
                              fill={FLAG} opacity={p.settled}>{kind}</text>
                    </g>
                )
            }
        };
        fframes::svgr!(
            <g opacity={enter}>
                <rect x={x} y={y} width={w} height={CHIP_H} rx="6" fill={fill} stroke={stroke} stroke-width="2.5" />
                <text x={p.x} y={p.y + 10.0} text-anchor="middle" font-family={MONO} font-size={CHIP_SIZE} fill={text_fill}>{text}</text>
                {label(x, y - 12.0, &line, GREY, 20.0, "start")}
                {mark}
            </g>
        )
    }
}
