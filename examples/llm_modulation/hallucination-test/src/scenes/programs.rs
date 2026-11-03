//! 4 · The two programs. Split screen under the poster's two titles:
//! program B left, program A right, both verbatim with their real line
//! numbers. B: lines 14, 18 and 25 are underlined, and a grey arm beside it
//! moves to the line-18 pose. A: lines 49 and 73 are ticked, its arm stays
//! still, and the response's own sentence types itself beneath.

use fframes::{FFramesContext, Frame, Svgr};

use crate::code::{PROGRAM_A, PROGRAM_B};
use crate::poster::*;
use crate::ui::*;

script_scene!(ProgramsScene, 92.0, 120.5);

/// The view that frames both program columns of the poster: the panes sit
/// where the columns would be.
const Z: f32 = 0.82;
fn split_cam() -> Cam {
    on_point(H_PROGRAM_B.x, H_PROGRAM_B.y, 40.0, 58.0, Z)
}


const SIZE: usize = 18;
const LINE: f32 = 28.0;
/// The panes' header bars, the code beneath them, the arms beneath that.
const BAR_TOP: f32 = 190.0;
const CODE_TOP: f32 = 238.0;
const ROWS: usize = 15;
const ARM_RULE: f32 = 682.0;
const FLOOR: f32 = 900.0;
/// Program B's window: lines 12..=26.
const B_FIRST: usize = 12;
/// Program A's window scrolls from line 45 to line 60 between these times.
const A_FIRST: (usize, usize) = (45, 60);
const A_SCROLL: (f32, f32) = (15.5, 17.0);

/// B's hallucinated lines and when each underline lands.
const B_MARKS: &[(usize, f32)] = &[(14, 3.5), (18, 5.0), (25, 6.5)];
/// A's lines that take the address and the pose as inputs.
const A_MARKS: &[(usize, f32)] = &[(49, 14.0), (73, 18.0)];

const ARM_MOVE: (f32, f32) = (7.5, 11.0);
const QUOTE: Typing = Typing::new(20.5, "\u{201C}There is no joint vector that is a safe home for every UR5e.\u{201D}", 16.0);

pub fn tones() -> Vec<f32> {
    B_MARKS.iter().map(|m| m.1).collect()
}

/// A pose of the six joints, radians.
type Q = [f32; 6];
/// Program B, line 18.
const HOME_Q: Q = [0.0, -std::f32::consts::FRAC_PI_2, 0.0, -std::f32::consts::FRAC_PI_2, 0.0, 0.0];
/// Where both arms rest before anything is sent: folded down, nothing taken from either program.
const REST_Q: Q = [0.0, -1.2, 1.6, -1.9, 0.0, 0.0];

/// The panes in screen pixels: where the poster's program columns land.
fn pane(cam: Cam, left: f32, right: f32) -> (f32, f32) {
    (cam.screen(left, 0.0).0, cam.screen(right, 0.0).0)
}

impl ProgramsScene {
    fn draw(&self, frame: &mut Frame, ctx: &FFramesContext, t: f32) -> Svgr<'static> {
        let s = window(t, 0.0, Self::LEN, 0.7);
        let content = if s > 0.0 { self.content(frame, ctx, t) } else { Svgr::empty() };
        fframes::svgr!(
            <g>
                {stage(1.0)}
                {header(4, "The Robotic Code Examples", s)}
                <g opacity={s}>{content}</g>
            </g>
        )
    }

    fn content(&self, frame: &mut Frame, ctx: &FFramesContext, t: f32) -> Svgr<'static> {
        let cam = split_cam();
        let (bx0, bx1) = pane(cam, 62.0, 1143.0);
        let (ax0, ax1) = pane(cam, 1237.0, 2318.0);
        let a_first = lerp(A_FIRST.0 as f32, A_FIRST.1 as f32, ease_in_out(prog(t, A_SCROLL.0, A_SCROLL.1)));
        let b = self.code(frame, ctx, &PROGRAM_B, B_FIRST as f32, bx0, bx1, B_MARKS, true, "pane-b", t, 0.8);
        let a = self.code(frame, ctx, &PROGRAM_A, a_first, ax0, ax1, A_MARKS, false, "pane-a", t, 1.1);
        // the arms: B's goes to the pose of its line 18, A's stays where it is
        let k = ease_in_out(prog(t, ARM_MOVE.0, ARM_MOVE.1));
        let mut q = REST_Q;
        for (j, v) in q.iter_mut().enumerate() {
            *v = lerp(REST_Q[j], HOME_Q[j], k);
        }
        let arms_in = ease_out(prog(t, 2.0, 2.6));
        let tag = ease_out(prog(t, ARM_MOVE.1 - 0.2, ARM_MOVE.1 + 0.4));
        let quote_ty = Type::new(SERIF, 30, 400, INK).italic().width(ax1 - ax0 - 230.0).line_h(40.0);
        let quote = typed_block(frame, ctx, ax0 + 210.0, 782.0, &QUOTE, quote_ty, 1.4, t);
        // an editor-style header bar over each pane
        let bar = |x0: f32, x1: f32, name: &'static str, enter: f32| {
            let k = ease_out(prog(t, enter, enter + 0.6));
            fframes::svgr!(
                <g opacity={k}>
                    <rect x={x0} y={BAR_TOP} width={x1 - x0} height={CODE_TOP - BAR_TOP - 4.0} rx="4" fill={SHEET} />
                    <rect x={x0} y={BAR_TOP} width="4" height={CODE_TOP - BAR_TOP - 4.0} fill={ACCENT} />
                    <text x={x0 + 20.0} y={BAR_TOP + 30.0} font-family={SERIF} font-size="28" font-weight="700" fill={INK}>{name}</text>
                    {label(x1 - 16.0, BAR_TOP + 29.0, "VERBATIM", GREY, 17.0, "end")}
                </g>
            )
        };
        fframes::svgr!(
            <g>
                {bar(bx0, bx1, "Program B (grok-4.7)", 0.8)}
                {bar(ax0, ax1, "Program A (grok-4.7+modulation)", 1.1)}
                {b}
                {a}
                <g opacity={arms_in}>
                    <rect x={bx0} y={ARM_RULE} width={bx1 - bx0} height="1.5" fill={RULE} />
                    <rect x={ax0} y={ARM_RULE} width={ax1 - ax0} height="1.5" fill={RULE} />
                    {arm(bx0 + 90.0, FLOOR, 150.0, &q)}
                    {arm(ax0 + 90.0, FLOOR, 150.0, &REST_Q)}
                </g>
                <g opacity={tag}>
                    {label(bx0 + 220.0, 782.0, "POSE FROM LINE 18", FLAG, 20.0, "start")}
                    <text x={bx0 + 220.0} y="820" font-family={MONO} font-size="22" fill={INK}>"[0.0, -pi/2, 0.0, -pi/2, 0.0, 0.0]"</text>
                    <text x={bx0 + 220.0} y="862" font-family={SERIF} font-size="24" font-style="italic" fill={GREY}>"commanded on execution, after connecting to 192.168.1.10"</text>
                </g>
                <g opacity={ease_out(prog(t, A_MARKS[0].1 - 2.0, A_MARKS[0].1 - 1.4))}>
                    {label(ax0 + 210.0, 744.0, "NO MOTION WITHOUT VALIDATED INPUTS", OK, 20.0, "start")}
                </g>
                {quote}
            </g>
        )
    }

    /// A code pane: a window of `ROWS` lines from line `first` (fractional
    /// while it scrolls), real line numbers, marks on the chosen lines.
    #[allow(clippy::too_many_arguments)]
    fn code(
        &self,
        frame: &mut Frame,
        ctx: &FFramesContext,
        program: &[&'static str],
        first: f32,
        x0: f32,
        x1: f32,
        marks: &[(usize, f32)],
        red: bool,
        clip_id: &'static str,
        t: f32,
        enter_at: f32,
    ) -> Svgr<'static> {
        let enter = ease_out(prog(t, enter_at, enter_at + 0.6));
        let num_w = 36.0;
        let cx = x0 + num_w + 12.0;
        let char_w = measure(frame, ctx, MONO, SIZE, 400, false, "0123456789") / 10.0;
        let top = CODE_TOP;
        let from = first.floor() as usize;
        let mut rows = Vec::new();
        for n in from..=(from + ROWS + 1).min(program.len()) {
            if n < 1 {
                continue;
            }
            let y = top + (n as f32 - first) * LINE + LINE * 0.72;
            if y < top - LINE || y > top + ROWS as f32 * LINE + LINE {
                continue;
            }
            let text = program[n - 1];
            let indent = text.chars().take_while(|c| *c == ' ').count() as f32 * char_w;
            let width = text.chars().count() as f32 * char_w - indent;
            let mark = marks.iter().find(|m| m.0 == n).map(|m| m.1);
            let k = mark.map_or(0.0, |at| prog(t, at - 0.25, at + 0.2));
            let (wash, accent) = if red { (FLAG_WASH, FLAG) } else { (OK_WASH, OK) };
            let num_fill = mix(GREY, accent, k);
            let highlight = if k > 0.0 {
                let p = mark.map_or(0.0, |at| prog(t, at - 0.2, at + 0.25));
                let flag = if red {
                    underline(cx + indent, y + 6.0, width, p, 3.0)
                } else {
                    tick(cx + indent + width + 26.0, y - 6.0, 22.0, p)
                };
                fframes::svgr!(
                    <g>
                        <rect x={x0} y={y - LINE * 0.72} width={x1 - x0} height={LINE} fill={wash} opacity={k} />
                        {flag}
                    </g>
                )
            } else {
                Svgr::empty()
            };
            let num = n.to_string();
            // SVG collapses leading spaces: the indentation is a position
            let text = text.trim_start().to_owned();
            rows.push(fframes::svgr!(
                <g>
                    {highlight}
                    <text x={x0 + num_w} y={y} text-anchor="end" font-family={MONO} font-size={SIZE} fill={num_fill}>{num}</text>
                    <text x={cx + indent} y={y} font-family={MONO} font-size={SIZE} fill={INK}>{text}</text>
                </g>
            ));
        }
        let clip = format!("url(#{clip_id})");
        let h = ROWS as f32 * LINE;
        fframes::svgr!(
            <g opacity={enter} transform={format!("translate(0 {})", (1.0 - enter) * 24.0)}>
                <defs>
                    <clipPath id={clip_id}>
                        <rect x={x0} y={top} width={x1 - x0} height={h} />
                    </clipPath>
                </defs>
                <rect x={x0} y={top} width={x1 - x0} height={h} fill={SHEET} />
                <g clip-path={clip}>{rows}</g>
                <rect x={x0} y={top} width={x1 - x0} height="1" fill={RULE} />
                <rect x={x0} y={top + h - 1.0} width={x1 - x0} height="1" fill={RULE} />
            </g>
        )
    }
}

/// A plain grey six-joint arm seen from the side, no brand styling, with
/// UR5e link lengths (metres) at `px_per_m`. `q` are the joint angles; the
/// base and the two wrist rotations about the viewing axis do not show.
fn arm(x: f32, floor: f32, px_per_m: f32, q: &Q) -> Svgr<'static> {
    let s = px_per_m;
    let (d1, a2, a3, d5) = (0.1625, 0.425, 0.3922, 0.0997);
    let dir = |th: f32| (th.cos(), th.sin());
    let shoulder = (x, floor - d1 * s);
    let t2 = q[1];
    let t3 = q[1] + q[2];
    let t4 = q[1] + q[2] + q[3];
    let elbow = (shoulder.0 + dir(t2).0 * a2 * s, shoulder.1 + dir(t2).1 * a2 * s);
    let wrist = (elbow.0 + dir(t3).0 * a3 * s, elbow.1 + dir(t3).1 * a3 * s);
    let flange = (wrist.0 + dir(t4).0 * d5 * s, wrist.1 + dir(t4).1 * d5 * s);
    let link = |a: (f32, f32), b: (f32, f32), w: f32| {
        let d = format!("M{} {} L{} {}", a.0, a.1, b.0, b.1);
        let d2 = d.clone();
        fframes::svgr!(
            <g>
                <path d={d} stroke="#4E555F" stroke-width={w + 3.0} stroke-linecap="round" fill="none" />
                <path d={d2} stroke="#9AA3AE" stroke-width={w} stroke-linecap="round" fill="none" />
            </g>
        )
    };
    let joint = |p: (f32, f32), r: f32| {
        fframes::svgr!(
            <g>
                <circle cx={p.0} cy={p.1} r={r} fill="#6B737D" />
                <circle cx={p.0} cy={p.1} r={r * 0.45} fill="#D3D4D6" />
            </g>
        )
    };
    let base = format!("M{} {} L{} {} L{} {} L{} {} Z", x - 26.0, floor, x + 26.0, floor, x + 18.0, floor - 18.0, x - 18.0, floor - 18.0);
    fframes::svgr!(
        <g>
            <rect x={x - 70.0} y={floor} width="140" height="2" fill={GREY} />
            <path d={base} fill="#6B737D" />
            {link((x, floor - 16.0), shoulder, 16.0)}
            {link(shoulder, elbow, 15.0)}
            {link(elbow, wrist, 12.0)}
            {link(wrist, flange, 10.0)}
            {joint(shoulder, 13.0)}
            {joint(elbow, 11.0)}
            {joint(wrist, 9.0)}
            <circle cx={flange.0} cy={flange.1} r="6" fill="#4E555F" />
        </g>
    )
}
