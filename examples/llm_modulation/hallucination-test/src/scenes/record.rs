//! 5 · Run record, under "Run metadata": the two timestamps, 16:47 and
//! 16:51 UTC, slide together into "4 minutes". Then the data availability
//! line: both SHA-256 hashes scroll in along the lower third and stay, in
//! full.

use fframes::{FFramesContext, Frame, Svgr};

use crate::poster::*;
use crate::ui::*;

script_scene!(RecordScene, 118.5, 134.0);


/// (label, timestamp) from the poster's run metadata.
const STAMPS: [(&str, &str); 2] = [("A: grok-4.7+modulation", "2026-10-11 16:47"), ("B: grok-4.7", "2026-10-11 16:51")];
const TOGETHER: (f32, f32) = (3.5, 5.0);
const HASHES: [(&str, &str, f32); 2] = [
    ("grok-4.7+modulation, SHA-256", "737377f67aba947abf120c312d2b5d5e7ce510ea38e14fd77f48fc2d09e9d39e", 8.0),
    ("grok-4.7, SHA-256", "fd943c3aa1d8d0814fb7b4a016504efbe67d7c526f5426f6ec24d8cd1209add9", 8.8),
];

/// Left edge of text inside the panels.
const PX: f32 = MX + 40.0;

impl RecordScene {
    fn draw(&self, frame: &mut Frame, ctx: &FFramesContext, t: f32) -> Svgr<'static> {
        let s = window(t, 0.0, Self::LEN, 0.7);
        let content = if s > 0.0 { self.content(frame, ctx, t) } else { Svgr::empty() };
        fframes::svgr!(
            <g>
                {stage(1.0)}
                {header(5, "Run record", s)}
                <g opacity={s}>
                    <rect x={MX} y="180" width={W - 2.0 * MX} height="340" rx="6" fill={SHEET} />
                    <rect x={MX} y="560" width={W - 2.0 * MX} height="320" rx="6" fill={SHEET} opacity={ease_out(prog(t, 6.8, 7.5))} />
                    {content}
                </g>
            </g>
        )
    }

    fn content(&self, frame: &mut Frame, ctx: &FFramesContext, t: f32) -> Svgr<'static> {
        let size = 50;
        let stamp_w = measure(frame, ctx, MONO, size, 400, false, STAMPS[0].1);
        let gap_far = 520.0;
        let gap_near = 150.0;
        let k = ease_in_out(prog(t, TOGETHER.0, TOGETHER.1));
        let gap = lerp(gap_far, gap_near, k);
        let left = W / 2.0 - gap / 2.0 - stamp_w;
        let right = W / 2.0 + gap / 2.0;
        let y = 330.0;
        let mut out = Vec::new();
        for (i, ((name, stamp), x)) in STAMPS.iter().zip([left, right]).enumerate() {
            let a = ease_out(prog(t, 1.0 + i as f32 * 0.3, 1.6 + i as f32 * 0.3));
            let name = (*name).to_owned();
            let stamp = (*stamp).to_owned();
            out.push(fframes::svgr!(
                <g opacity={a} transform={format!("translate(0 {})", (1.0 - a) * 16.0)}>
                    <text x={x} y={y - 70.0} font-family={SERIF} font-size="34" font-weight="700" fill={INK}>{name}</text>
                    <text x={x} y={y} font-family={MONO} font-size={size} fill={INK}>{stamp}</text>
                    <text x={x + stamp_w} y={y + 44.0} text-anchor="end" font-family={MONO} font-size="24" fill={GREY}>"UTC"</text>
                </g>
            ));
        }
        // the bracket between the minutes, and what it spans
        let b = ease_out(prog(t, TOGETHER.1 - 0.1, TOGETHER.1 + 0.5));
        let m0 = left + stamp_w - measure(frame, ctx, MONO, size, 400, false, "47") / 2.0;
        let m1 = right + stamp_w - measure(frame, ctx, MONO, size, 400, false, "51") / 2.0;
        let by = y + 74.0;
        let bracket = format!("M{m0} {} L{m0} {by} L{m1} {by} L{m1} {}", by - 14.0, by - 14.0);
        let len = (m1 - m0) + 28.0;
        out.push(fframes::svgr!(
            <g opacity={prog(b, 0.0, 0.1)}>
                <path d={bracket} fill="none" stroke={INK} stroke-width="2.5" stroke-dasharray={len} stroke-dashoffset={len * (1.0 - b)} />
                <text x={(m0 + m1) / 2.0} y={by + 54.0} text-anchor="middle" font-family={SERIF} font-size="44" fill={INK} opacity={b}>"4 minutes"</text>
            </g>
        ));
        // data availability: the hashes scroll in from the right and stay
        let da = ease_out(prog(t, 7.0, 7.7));
        out.push(fframes::svgr!(
            <g opacity={da}>
                <text x={PX} y="642" font-family={SERIF} font-size="30" font-weight="700" fill={INK}>"Data availability."</text>
                <text x={PX + 270.0} y="642" font-family={SERIF} font-size="30" fill={INK}>"Both calls, request and response verbatim with response times, are preserved as JSON artifacts."</text>
            </g>
        ));
        for (i, (name, hash, at)) in HASHES.iter().enumerate() {
            let k = ease_out(prog(t, *at, at + 1.6));
            let y = 742.0 + i as f32 * 62.0;
            let x = lerp(W + 40.0, PX, k);
            let name = (*name).to_owned();
            let hash = (*hash).to_owned();
            out.push(fframes::svgr!(
                <g transform={format!("translate({} 0)", x - MX)}>
                    <text x={MX} y={y} font-family={SERIF} font-size="30" fill={GREY}>{name}</text>
                    <text x={MX + 430.0} y={y} font-family={MONO} font-size="31" fill={INK}>{hash}</text>
                </g>
            ));
        }
        fframes::svgr!(<g>{out}</g>)
    }
}
