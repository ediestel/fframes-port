//! Scene 7 · 03:25–03:50 · Audit trail and provenance: the log of the case,
//! then the database record traced back to its request, data snapshot,
//! prompt and approval.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::ui::*;

const LOG: [(&str, &str, &str); 10] = [
    ("01", "request received", "DEMO-001 · email · sha256 4e1a…9c07"),
    ("02", "snapshot read", "DEMO-SNAPSHOT-01 · read-only · sha256 b72f…0d13"),
    ("03", "tokens issued", "GUEST-7Q4M · OP-8N2K · SRC-4B2R · key kept local"),
    ("04", "prompt run", "rev. 1 · answer DEMO-S1"),
    ("05", "prompt changed", "rev. 2 · selection rule · by reception"),
    ("06", "prompt run", "rev. 2 · answer DEMO-S2"),
    ("07", "checks", "6 passed · results and reasons stored"),
    ("08", "approval", "reception"),
    ("09", "reply written", "output/DEMO-001_antwort.txt"),
    ("10", "record imported", "DEMO-DB-1042"),
];

/// The trace from the record back: (row, time it lights up).
const TRACE: [(usize, f32); 5] = [(9, 11.0), (0, 12.8), (1, 14.6), (5, 16.4), (7, 18.4)];

pub fn draw<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let mut rows = Vec::new();
    for (i, (n, step, detail)) in LOG.iter().enumerate() {
        let y = 300.0 + i as f32 * 52.0;
        let lit = TRACE.iter().find(|(r, _)| *r == i).map_or(0.0, |(_, at)| prog(t, *at, at + 0.5));
        rows.push(rise_in(
            t,
            0.9 + i as f32 * 0.45,
            fframes::svgr!(<g>
                <rect x="190" y={y - 32.0} width="1600" height="44" rx="8" fill={GREEN_BG} opacity={lit * 0.9} />
                {mono(214.0, y, 19.0, MUTED, "start", n)}
                {text(280.0, y, 22.0, 500, INK, "start", step)}
                {mono(620.0, y, 19.0, if lit > 0.0 { GREEN } else { MUTED }, "start", detail)}
            </g>),
        ));
    }
    // the trace line along the lit rows
    let trace_k = prog(t, 11.0, 18.8);
    let trace = if trace_k > 0.0 {
        let top = 300.0 - 10.0;
        let bottom = 300.0 + 9.0 * 52.0 - 10.0;
        let y = lerp(bottom, top, ease_in_out(trace_k));
        fframes::svgr!(<g>
            <rect x="160" y={y} width="4" height={(bottom - y).max(0.5)} rx="2" fill={GREEN} />
            <circle cx="162" cy={y} r="8" fill={GREEN} />
        </g>)
    } else {
        Svgr::empty()
    };
    let log = rise_in(
        t,
        0.5,
        fframes::svgr!(<g>
            {card(120.0, 190.0, 1704.0, 660.0, Some(GREEN), PANEL, LINE)}
            {mono(150.0, 236.0, 20.0, GREEN, "start", "output/audit-OP-8N2K.log")}
            {text(1794.0, 236.0, 18.0, 400, MUTED, "end", "inputs · prompt version · answer · checks · approval")}
        </g>),
    );
    fframes::svgr!(<g>{log}{rows}{trace}</g>)
}
