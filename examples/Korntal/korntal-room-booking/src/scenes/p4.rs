//! Scene 4 · 01:30–02:20 · Prompts carry out the procedure.
//! The prompt series as plain-text files; one run with the tokenised input
//! returns a structured answer; the selection sentence in the task prompt is
//! edited and the same data produce DEMO-S2.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::ui::*;

const FILES: [&str; 9] = [
    "01_role_authority.md",
    "02_sources_rules.md",
    "03_interpret_request.md",
    "04_evaluate_allocation.md",
    "05_commercial.md",
    "06_lifecycle.md",
    "07_result_open_issues.md",
    "08_traceability.md",
    "task_evaluate_inquiry.md",
];

const OLD: &str = "Among eligible rooms, prefer the smallest room that satisfies the request.";
const NEW1: &str = "Among eligible rooms, prefer the largest single room that";
const NEW2: &str = "satisfies the request, to prioritize space for the guest.";

fn typed(s: &str, t: f32, at: f32, cps: f32) -> String {
    let n = ((t - at).max(0.0) * cps) as usize;
    s.chars().take(n).collect()
}

pub fn draw<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    // the prompt files
    let mut files = Vec::new();
    for (i, n) in FILES.iter().enumerate() {
        let y = 290.0 + i as f32 * 30.0 + if i == 8 { 10.0 } else { 0.0 };
        let task = i == 8;
        let hl = if task { window(t, 29.0, 38.5, 0.4) } else { 0.0 };
        files.push(rise_in(
            t,
            0.9 + i as f32 * 0.35,
            fframes::svgr!(<g>
                <rect x="140" y={y - 22.0} width="460" height="30" rx="6" fill={AMBER_BG} opacity={hl} />
                {mono(152.0, y, 18.0, if task { AMBER } else { INK }, "start", n)}
            </g>),
        ));
    }
    let folder = rise_in(
        t,
        0.6,
        fframes::svgr!(<g>
            {card(120.0, 190.0, 500.0, 390.0, Some(AMBER), PANEL, LINE)}
            {icon_folder(146.0, 214.0, 40.0, 30.0, AMBER, AMBER_BG)}
            {mono(200.0, 240.0, 24.0, AMBER, "start", "prompts/")}
            {text(590.0, 240.0, 17.0, 400, MUTED, "end", "plain text · editable")}
        </g>),
    );

    // one run: prompts + tokenised input → LLM
    let input = rise_in(
        t,
        8.2,
        fframes::svgr!(<g>
            {badge(660.0, 300.0, "request GUEST-7Q4M", VIOLET, VIOLET_BG, 17.0)}
            {badge(660.0, 346.0, "records SRC-4B2R", BLUE, BLUE_BG, 17.0)}
        </g>),
    );
    let busy = window(t, 10.0, 12.2, 0.3) + window(t, 35.6, 37.0, 0.3);
    let llm = rise_in(t, 8.6, llm_node(900.0, 450.0, 50.0, t * (1.0 + 3.0 * busy), busy.max(0.4), "LLM"));
    let runs = fframes::svgr!(<g>
        {arrow(620.0, 420.0, 836.0, 446.0, prog(t, 9.0, 9.8), AMBER, 3.0)}
        {arrow(960.0, 440.0, 1090.0, 400.0, prog(t, 12.0, 12.6), VIOLET, 3.0)}
    </g>);

    // the structured answer
    let rev2 = prog(t, 36.8, 37.6);
    let rec = if rev2 > 0.5 { "\"DEMO-S2\"" } else { "\"DEMO-S1\"" };
    let alt = if rev2 > 0.5 { "\"DEMO-S1\"" } else { "\"DEMO-S2\"" };
    let flash = window(t, 36.8, 39.5, 0.3);
    let lines: [(String, &str, f32); 9] = [
        ("{".into(), INK, 12.4),
        ("  \"operation\":   \"OP-8N2K\",".into(), INK, 12.8),
        (format!("  \"recommended\": {rec},"), GREEN, 13.2),
        (format!("  \"alternative\": {alt},"), INK, 13.6),
        ("  \"excluded\": [".into(), INK, 14.0),
        ("    {\"room\": \"DEMO-D1\", \"reason\": \"booked 17–18 Nov\"},".into(), RED, 14.4),
        ("    {\"room\": \"DEMO-S3\", \"reason\": \"maintenance 16–17 Nov\"}],".into(), RED, 14.8),
        ("  \"open\": [\"price\", \"meals\", \"arrival\"]".into(), INK, 15.2),
        ("}".into(), INK, 15.4),
    ];
    let mut json = Vec::new();
    for (i, (l, c, at)) in lines.iter().enumerate() {
        let y = 296.0 + i as f32 * 36.0;
        let hl = match i {
            2 => window(t, 18.0, 22.0, 0.3).max(flash),
            3 => flash,
            5 | 6 => window(t, 21.5, 26.0, 0.3),
            _ => 0.0,
        };
        json.push(rise_in(
            t,
            *at,
            fframes::svgr!(<g>
                <rect x="1112" y={y - 25.0} width="690" height="34" rx="6" fill={if i >= 5 && i <= 6 { RED_BG } else { GREEN_BG }} opacity={hl} />
                {mono(1124.0, y, 18.0, c, "start", l)}
            </g>),
        ));
    }
    let answer = rise_in(
        t,
        12.0,
        fframes::svgr!(<g>
            {card(1100.0, 190.0, 724.0, 450.0, Some(VIOLET), PANEL, LINE)}
            {text(1124.0, 240.0, 22.0, 500, VIOLET, "start", "Answer · fixed structure")}
            <g opacity={rev2}>{badge(1660.0, 232.0, "prompt rev. 2", AMBER, AMBER_BG, 16.0)}</g>
            <g opacity={1.0 - rev2}>{badge(1660.0, 232.0, "prompt rev. 1", AMBER, AMBER_BG, 16.0)}</g>
            {json}
        </g>),
    );

    // the edit of one sentence in the task prompt
    let strike = prog(t, 30.2, 30.8);
    let l1 = typed(NEW1, t, 31.0, 26.0);
    let l2 = typed(NEW2, t, 31.0 + NEW1.chars().count() as f32 / 26.0 + 0.1, 26.0);
    let editor = rise_in(
        t,
        29.4,
        fframes::svgr!(<g>
            {card(120.0, 610.0, 940.0, 240.0, Some(AMBER), PANEL, AMBER)}
            {mono(150.0, 650.0, 18.0, AMBER, "start", "task_evaluate_inquiry.md · selection rule")}
            {text(150.0, 694.0, 22.0, 400, MUTED, "start", OLD)}
            <rect x="150" y="686" width={(760.0 * strike).max(0.5)} height="2.5" fill={MUTED} opacity={strike} />
            {text(150.0, 744.0, 25.0, 500, INK, "start", &l1)}
            {text(150.0, 782.0, 25.0, 500, INK, "start", &l2)}
            <g opacity={prog(t, 37.6, 38.2)}>{text(1030.0, 828.0, 18.0, 400, MUTED, "end", "same data · same software · rev. 2 logged")}</g>
        </g>),
    );
    let rerun = arrow(1062.0, 700.0, 1110.0, 640.0, prog(t, 35.4, 36.0), AMBER, 3.0);

    fframes::svgr!(<g>{folder}{files}{input}{runs}{llm}{answer}{editor}{rerun}</g>)
}
