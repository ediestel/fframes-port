//! The 15 pages, from the deck "AI and language models in professional work"
//! (AMDG Engineers, November 2026). Slides 1–8, 11 and 15 as in the deck;
//! slides 9, 10, 12, 13 and 14 reworked: model names instead of
//! Direct/Modulated, the actual values of the UR5e test, the cost chain with
//! the point where adjudication acts, a real excerpt of an adjudication
//! record, and a closing with one statement.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::narration::at;
use crate::ui::*;

type Draw = for<'a> fn(&mut Frame, &FFramesContext<'a, '_>, f32) -> Svgr<'a>;

pub const IDS: [&str; 15] = [
    "A01_Title", "A02_Uses", "A03_Value", "A04_Limits", "A05_Reliability", "A06_AMDG", "A07_SingleShot", "A08_Reader",
    "A09_PairedTests", "A10_UR5e", "A11_Applications", "A12_Economics", "A13_Record", "A14_Sources", "A15_Closing",
];

pub const DRAWS: [Draw; 15] = [s01, s02, s03, s04, s05, s06, s07, s08, s09, s10, s11, s12, s13, s14, s15];

// ---------------------------------------------------------------------------
// shared pieces

/// The slide's headline and its source note.
fn page<'a>(t: f32, title: &str, foot: &str, body: Svgr<'a>) -> Svgr<'a> {
    let title = title.to_owned();
    let foot = foot.to_owned();
    let k = enter(t, 0.2, 0.6);
    fframes::svgr!(<g>
        <g opacity={k} transform={tr(0.0, (1.0 - k) * 16.0)}>
            <text x="130" y="214" font-family={SANS} font-size="40" font-weight="500" fill={INK}>{title}</text>
        </g>
        {body}
        <g opacity={enter(t, 1.0, 0.6)}>{mono(130.0, 866.0, 14.0, FAINT, "start", &foot)}</g>
    </g>)
}

/// Rough line count of a paragraph.
fn lines(s: &str, size: f32, w: f32) -> usize {
    ((s.chars().count() as f32 * size * 0.52) / w).ceil().max(1.0) as usize
}

/// A lead sentence under the headline.
fn lead(t: f32, s: &str) -> Svgr<'static> {
    rise_in(t, 0.8, text(130.0, 270.0, 28.0, 400, MUTED, "start", s))
}

/// Four cards in a 2 × 2 grid from `top`: (title, body).
#[allow(clippy::too_many_arguments)]
fn cards4<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32, top: f32, items: &[(&str, &str); 4], t0: f32, step: f32, color: &'static str) -> Svgr<'a> {
    let h = (830.0 - top - 26.0) / 2.0;
    let mut out = Vec::new();
    for (i, (title, body)) in items.iter().enumerate() {
        let x = 130.0 + (i % 2) as f32 * 845.0;
        let y = top + (i / 2) as f32 * (h + 26.0);
        out.push(rise_in(
            t,
            t0 + i as f32 * step,
            fframes::svgr!(<g>
                {card(x, y, 815.0, h, Some(color), PANEL, LINE)}
                {text(x + 36.0, y + 60.0, 28.0, 500, color, "start", title)}
                {para(f, ctx, (x + 36.0) as i32, (y + 108.0) as i32, 740, 24, 400, INK, body)}
            </g>),
        ));
    }
    fframes::svgr!(<g>{out}</g>)
}

/// Three cards side by side from `top`: (title, body).
#[allow(clippy::too_many_arguments)]
fn cards3<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32, top: f32, h: f32, items: &[(&str, &str); 3], t0: f32, step: f32, color: &'static str) -> Svgr<'a> {
    let mut out = Vec::new();
    for (i, (title, body)) in items.iter().enumerate() {
        let x = 130.0 + i as f32 * 563.0;
        out.push(rise_in(
            t,
            t0 + i as f32 * step,
            fframes::svgr!(<g>
                {card(x, top, 534.0, h, Some(color), PANEL, LINE)}
                {text(x + 34.0, top + 62.0, 30.0, 500, color, "start", title)}
                {para(f, ctx, (x + 34.0) as i32, (top + 114.0) as i32, 466, 25, 400, INK, body)}
            </g>),
        ));
    }
    fframes::svgr!(<g>{out}</g>)
}

/// A table: column heads, then rows appearing one after another. Columns are
/// (x, width); the first column is set in amber. `colors` tints the other
/// columns (one colour per column).
#[allow(clippy::too_many_arguments)]
fn table<'a>(
    f: &mut Frame,
    ctx: &FFramesContext<'a, '_>,
    t: f32,
    top: f32,
    cols: &[(f32, f32)],
    heads: &[(&str, &'static str)],
    rows: &[&[&str]],
    size: usize,
    t0: f32,
    step: f32,
) -> Svgr<'a> {
    let s = size as f32;
    let mut out = Vec::new();
    let mut head = Vec::new();
    for ((x, _), (h, color)) in cols.iter().zip(heads) {
        head.push(text(*x, top + 37.0, 22.0, 500, color, "start", h));
    }
    out.push(show(
        enter(t, t0 - 0.4, 0.5),
        0.0,
        fframes::svgr!(<g>
            <rect x="130" y={top} width="1660" height="56" rx="10" fill={PANEL} />
            {head}
        </g>),
    ));
    let mut y = top + 66.0;
    for (r, row) in rows.iter().enumerate() {
        let n = row.iter().zip(cols).map(|(c, (_, w))| lines(c, s, *w)).max().unwrap_or(1);
        let h = n as f32 * s * 1.3 + 26.0;
        let mut cells = Vec::new();
        for (c, (cell, (x, w))) in row.iter().zip(cols).enumerate() {
            let fill = if c == 0 { INK } else { heads[c].1 };
            let weight = if c == 0 { 400 } else { 500 };
            cells.push(para(f, ctx, *x as i32, (y + s + 8.0) as i32, *w as usize, size, weight, fill, cell));
        }
        out.push(rise_in(
            t,
            t0 + r as f32 * step,
            fframes::svgr!(<g>
                <rect x="130" y={y + h - 1.0} width="1660" height="1.5" fill={LINE} />
                {cells}
            </g>),
        ));
        y += h;
    }
    fframes::svgr!(<g>{out}</g>)
}

/// A big figure with its label beneath.
fn big<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, x: f32, y: f32, w: usize, fig: &str, label: &str, color: &str) -> Svgr<'a> {
    fframes::svgr!(<g>
        {text(x, y, 86.0, 500, color, "start", fig)}
        {para(f, ctx, x as i32, (y + 56.0) as i32, w, 26, 400, INK, label)}
    </g>)
}

// ---------------------------------------------------------------------------
// the pages

/// 1 · Title.
fn s01<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let l1 = enter(t, 0.3, 0.8);
    let l2 = enter(t, 0.9, 0.8);
    let sub = enter(t, 1.7, 0.8);
    let meta = enter(t, 2.6, 0.8);
    fframes::svgr!(<g>
        <g opacity={l1} transform={tr(0.0, (1.0 - l1) * 30.0)}>
            <text x="200" y="330" font-family={SANS} font-size="96" font-weight="500" fill={INK} letter-spacing="-2">"AI and language models"</text>
        </g>
        <g opacity={l2} transform={tr(0.0, (1.0 - l2) * 30.0)}>
            <text x="200" y="440" font-family={SANS} font-size="96" font-weight="500" fill={AMBER} letter-spacing="-2">"in professional work"</text>
        </g>
        <g opacity={sub}>{text(204.0, 532.0, 36.0, 400, INK, "start", "Current uses, economics and claim adjudication")}</g>
        <g opacity={meta}>
            <rect x="204" y="590" width="220" height="4" rx="2" fill="url(#amberBar)" />
            {text(204.0, 650.0, 30.0, 500, INK, "start", "AMDG Engineers")}
            {text(204.0, 694.0, 24.0, 400, MUTED, "start", "Eckhart Diestel, Dr. univ. Gent, MD · November 2026")}
        </g>
    </g>)
}

/// 2 · Current uses of language models.
fn s02<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let items = [
        ("Documents", "Drafting, summarisation, translation and extraction"),
        ("Knowledge work", "Questions over records, policies and technical material"),
        ("Customer operations", "Response drafting and assistance for service staff"),
        ("Software", "Code generation, explanation and debugging"),
    ];
    let body = fframes::svgr!(<g>
        {lead(t, "Language models are one part of the wider field of AI.")}
        {cards4(f, ctx, t, 310.0, &items, 1.4, 1.6, BLUE)}
    </g>);
    page(t, "Current uses of language models", "NIST AI 600-1, Generative Artificial Intelligence Profile, July 2024.", body)
}

/// 3 · AI adoption and economic value.
fn s03<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let body = fframes::svgr!(<g>
        {rise_in(t, 1.2, big(f, ctx, 130.0, 420.0, 480, "88%", "of surveyed organisations used AI in 2025", AMBER))}
        {rise_in(t, 2.8, big(f, ctx, 690.0, 420.0, 480, "$285.9bn", "US private AI investment in 2025", AMBER))}
        {rise_in(t, at("a03_2"), big(f, ctx, 1250.0, 420.0, 500, "15%", "more issues resolved per hour in a customer support study", GREEN))}
    </g>);
    page(
        t,
        "AI adoption and economic value",
        "Adoption and investment: Stanford AI Index 2026. Productivity: Brynjolfsson, Li and Raymond, QJE 2025, 5,172 service agents.",
        body,
    )
}

/// 4 · Current limits.
fn s04<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let items = [
        ("Hallucination", "Invented facts, citations or technical details"),
        ("Missing conditions", "An answer can depend on inputs the request never supplied"),
        ("Context dependence", "Earlier conversation can influence later answers"),
    ];
    let body = fframes::svgr!(<g>
        {lead(t, "A fluent answer can contain an unsupported claim.")}
        {cards3(f, ctx, t, 330.0, 380.0, &items, 1.6, 2.4, RED)}
    </g>);
    page(t, "Current limits", "NIST AI 600-1, July 2024, sections 2.2 and 3: confidently presented false content as confabulation.", body)
}

/// 5 · How reliability is addressed today.
fn s05<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let items = [
        ("Retrieval", "Bring relevant documents into the model’s context"),
        ("External tools", "Use calculations, databases and software execution"),
        ("Validation", "Test outputs and check citations against their sources"),
        ("Professional review", "Apply domain judgement before consequential use"),
    ];
    let body = cards4(f, ctx, t, 258.0, &items, 1.4, 1.6, BLUE);
    page(t, "How reliability is addressed today", "NIST AI 600-1, Generative Artificial Intelligence Profile, July 2024, sections 2.2 and 3.", body)
}

/// 6 · AMDG Engineers.
fn s06<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let body = fframes::svgr!(<g>
        {rise_in(t, 0.8, fframes::svgr!(<g>
            {text(130.0, 360.0, 62.0, 500, INK, "start", "Every assertion is evaluated")}
            {text(130.0, 436.0, 62.0, 500, INK, "start", "and commented on.")}
        </g>))}
        {rise_in(t, 3.0, badge(130.0, 540.0, "PROPRIETARY MODULATION CONTROLS HALLUCINATION", VIOLET, VIOLET_BG, 18.0))}
        {rise_in(t, 4.4, badge(130.0, 604.0, "THE UNDERLYING MODEL REMAINS UNCHANGED", BLUE, BLUE_BG, 18.0))}
        {rise_in(t, at("a06_2") + 2.0, fframes::svgr!(<g>
            {check(150.0, 712.0, 20.0, prog(t, at("a06_2") + 2.3, at("a06_2") + 3.0), GREEN)}
            {text(190.0, 724.0, 40.0, 500, GREEN, "start", "Only whole claims pass as established.")}
        </g>))}
    </g>);
    page(t, "AMDG Engineers", "AMDG source: Eckhart Diestel, pamphlet, November 2026.", body)
}

/// 7 · Single shot with internal adjudication.
fn s07<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let boxes = [
        ("One complete request", "A standing start · No conversational carryover", BLUE, 1.0),
        ("An internal modulation loop", "Every claim is adjudicated · Unsettled claims remain identified", VIOLET, at("a07_2")),
        ("One final response", "A verdict and commentary for every assertion", GREEN, at("a07_2") + 5.0),
    ];
    let mut out = Vec::new();
    for (i, (title, body, color, at_i)) in boxes.iter().enumerate() {
        let x = 130.0 + i as f32 * 563.0;
        out.push(rise_in(
            t,
            *at_i,
            fframes::svgr!(<g>
                {card(x, 300.0, 534.0, 420.0, Some(color), PANEL, LINE)}
                {mono(x + 34.0, 352.0, 18.0, color, "start", &format!("{}", i + 1))}
                {para(f, ctx, (x + 34.0) as i32, 410, 466, 32, 500, color, title)}
                {para(f, ctx, (x + 34.0) as i32, 540, 466, 25, 400, INK, body)}
            </g>),
        ));
        if i < 2 {
            out.push(arrow(x + 538.0, 510.0, x + 559.0, 510.0, prog(t, at_i + 0.6, at_i + 1.0), FAINT, 3.0));
        }
    }
    // the loop: a turning ring in the middle card
    let ring = show(
        enter(t, at("a07_2"), 0.5),
        0.0,
        fframes::svgr!(<g transform={format!("rotate({:.1} 960 660)", t * 40.0)}>
            <circle cx="960" cy="660" r="26" fill="none" stroke={VIOLET} stroke-width="3" stroke-dasharray="10 8" />
        </g>),
    );
    page(
        t,
        "Single shot with internal adjudication",
        "The slide describes the service contract; it does not disclose implementation details.",
        fframes::svgr!(<g>{out}{ring}</g>),
    )
}

/// 8 · What the reader receives.
fn s08<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let rows: [&[&str]; 4] = [
        &["Established", "The whole claim and its adjudication"],
        &["Missing constituent", "The deficiency and what is missing"],
        &["Unsettled", "The unresolved point and what remains to check"],
        &["Outside scope", "The boundary of what the system can examine"],
    ];
    let tab = table(f, ctx, t, 258.0, &[(160.0, 560.0), (760.0, 1000.0)], &[("Claim outcome", MUTED), ("What appears in the response", INK)], &rows, 30, 1.6, 1.4);
    let note = rise_in(t, 9.0, text(160.0, 790.0, 28.0, 500, GREEN, "start", "Every assertion carries its outcome and commentary."));
    page(t, "What the reader receives", "AMDG source: Eckhart Diestel, pamphlet, November 2026.", fframes::svgr!(<g>{tab}{note}</g>))
}

/// 9 · Paired test results (reworked: model names, clear counts, the record).
fn s09<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let rows: [&[&str]; 5] = [
        &["Documents: statements the documents do not support", "2", "0"],
        &["Mathematics: statements the mathematics does not support", "1", "0"],
        &["Robot code: API errors against bosdyn-client 5.2.0", "3", "0"],
        &["Errors named in the response", "none of 6", "—"],
        &["Points named as not established", "none", "all"],
    ];
    let tab = table(
        f,
        ctx,
        t,
        258.0,
        &[(160.0, 900.0), (1120.0, 300.0), (1460.0, 320.0)],
        &[("Checked afterwards", MUTED), ("grok-4.7", RED), ("grok-4.7+modulation", GREEN)],
        &rows,
        28,
        1.6,
        1.6,
    );
    let record = rise_in(
        t,
        at("a09_2") + 4.0,
        fframes::svgr!(<g>
            {text(160.0, 790.0, 22.0, 400, MUTED, "start", "Published record:")}
            {mono(360.0, 790.0, 22.0, ACCENT_LINK, "start", "github.com/ediestel/llm_modulation_three_pairs")}
        </g>),
    );
    page(
        t,
        "Paired test results",
        "Same model, same query in each pair, one response per condition. AMDG paired tests, 3 November 2026.",
        fframes::svgr!(<g>{tab}{record}</g>),
    )
}

/// Link colour for repository addresses.
const ACCENT_LINK: &str = BLUE;

/// 10 · The UR5e safe home comparison (reworked: the actual values).
fn s10<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    // (what, grok-4.7 literal, grok-4.7 note, grok-4.7+modulation)
    let rows: [(&str, &str, &str, &str); 3] = [
        ("Robot address", "ROBOT_IP = \"192.168.1.10\"", "invented; the program connects to it", "None: the operator must supply it."),
        ("Home pose", "[0.0, -pi/2, 0.0, -pi/2, 0.0, 0.0]", "called safe without evidence for the installation", "None: the operator must supply one, verified collision-free for this mounting, tool and cell."),
        ("Action", "moveJ(HOME_Q, …)", "moves the arm to the invented pose", "Withholds action until both are supplied."),
    ];
    let mut out = Vec::new();
    out.push(rise_in(
        t,
        0.8,
        fframes::svgr!(<g>
            {badge(380.0, 262.0, "GROK-4.7", RED, RED_BG, 18.0)}
            {badge(1180.0, 262.0, "GROK-4.7+MODULATION", GREEN, GREEN_BG, 18.0)}
        </g>),
    ));
    for (i, (what, lit, note, modulated)) in rows.iter().enumerate() {
        let y = 300.0 + i as f32 * 150.0;
        let a_left = 1.4 + i as f32 * 2.0;
        let a_right = at("a10_2") + i as f32 * 2.6;
        out.push(rise_in(
            t,
            a_left,
            fframes::svgr!(<g>
                <rect x="130" y={y} width="1660" height="136" rx="12" fill={PANEL} opacity="0.7" />
                {text(160.0, y + 48.0, 24.0, 500, MUTED, "start", what)}
                <rect x="376" y={y + 22.0} width={(mono_w(lit, 22.0) + 16.0).min(760.0)} height="38" rx="6" fill={RED_BG} />
                {mono(384.0, y + 48.0, 22.0, RED, "start", lit)}
                {text(384.0, y + 98.0, 22.0, 400, INK, "start", note)}
            </g>),
        ));
        out.push(rise_in(
            t,
            a_right,
            fframes::svgr!(<g>
                {check(1196.0, y + 40.0, 15.0, prog(t, a_right + 0.3, a_right + 1.0), GREEN)}
                {para(f, ctx, 1226, (y + 48.0) as i32, 540, 22, 500, GREEN, modulated)}
            </g>),
        ));
    }
    out.push(rise_in(t, at("a10_2") + 9.0, text(160.0, 790.0, 28.0, 500, INK, "start", "Safety depended on information specific to the robot cell.")));
    page(
        t,
        "The UR5e safe home comparison",
        "UR5e comparison, 11 October 2026 · github.com/ediestel/llm_modulation_example · Not operational robot guidance.",
        fframes::svgr!(<g>{out}</g>),
    )
}

/// 11 · High-risk professional applications.
fn s11<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let items = [
        ("Legal", "Assertions and citations in letters, submissions and opinions"),
        ("Insurance", "Factual premises supporting coverage and claims decisions"),
        ("Hospitals", "Clinical assertions and unresolved questions in documentation"),
    ];
    let body = fframes::svgr!(<g>
        {cards3(f, ctx, t, 270.0, 380.0, &items, 1.4, 2.0, AMBER)}
        {rise_in(t, at("a11_2"), text(160.0, 760.0, 30.0, 500, GREEN, "start", "Each claim has a visible outcome before professional action."))}
    </g>);
    page(t, "High-risk professional applications", "Intended applications described by AMDG, not completed deployments or certifications.", body)
}

/// 12 · The economics of professional use (reworked: the cost chain and where
/// adjudication acts).
fn s12<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let steps = [
        ("Generation", "Model execution and modulation"),
        ("Review", "Time spent establishing claims"),
        ("Correction", "Repairing deficient claims"),
        ("Consequences", "Rework and errors reaching decisions"),
    ];
    let mut out = vec![lead(t, "Total cost includes what happens after the model responds.")];
    for (i, (title, body)) in steps.iter().enumerate() {
        let x = 130.0 + i as f32 * 422.0;
        let color = if i == 0 { BLUE } else { AMBER };
        out.push(rise_in(
            t,
            1.6 + i as f32 * 1.2,
            fframes::svgr!(<g>
                {card(x, 320.0, 394.0, 240.0, Some(color), PANEL, LINE)}
                {text(x + 32.0, 380.0, 30.0, 500, color, "start", title)}
                {para(f, ctx, (x + 32.0) as i32, 432, 330, 23, 400, INK, body)}
            </g>),
        ));
        if i < 3 {
            out.push(arrow(x + 398.0, 440.0, x + 418.0, 440.0, prog(t, 2.2 + i as f32 * 1.2, 2.6 + i as f32 * 1.2), FAINT, 3.0));
        }
    }
    // with adjudication, review and correction start at the named open points
    let a2 = at("a12_2");
    out.push(rise_in(
        t,
        a2,
        fframes::svgr!(<g>
            <path d="M552 572 v24 h818 v-24" fill="none" stroke={GREEN} stroke-width="3" />
            {text(961.0, 640.0, 28.0, 500, GREEN, "middle", "With adjudication: review starts at the named open points,")}
            {text(961.0, 680.0, 28.0, 500, GREEN, "middle", "not at every sentence.")}
        </g>),
    ));
    out.push(rise_in(t, a2 + 3.5, text(160.0, 790.0, 22.0, 400, MUTED, "start", "Measures: review time · residual errors · latency · total cost per accepted output")));
    page(
        t,
        "The economics of professional use",
        "An economic evaluation framework. No AMDG cost saving, ROI or latency figure is claimed.",
        fframes::svgr!(<g>{out}</g>),
    )
}

/// 13 · The record of adjudication (reworked: a real excerpt).
fn s13<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let established = [
        "It states that the roof inspection was carried out on 3 March 2026.",
        "It states that Moser’s report records the inspection of 5 March 2026.",
        "It states that the owner accepted the report on 9 March 2026.",
    ];
    let open = [
        "It does not establish a single date of the inspection.",
        "It does not establish that the supplied text is identical with Moser’s report.",
    ];
    let a2 = at("a13_2");
    let mut items = Vec::new();
    for (i, s) in established.iter().enumerate() {
        let y = 384.0 + i as f32 * 62.0;
        let at_i = a2 + 0.8 + i as f32 * 0.8;
        items.push(rise_in(t, at_i, fframes::svgr!(<g>
            {check(186.0, y - 8.0, 14.0, prog(t, at_i + 0.3, at_i + 0.9), GREEN)}
            {text(214.0, y, 22.0, 400, INK, "start", s)}
        </g>)));
    }
    for (i, s) in open.iter().enumerate() {
        let y = 590.0 + i as f32 * 62.0;
        let at_i = a2 + 3.6 + i as f32 * 0.9;
        items.push(rise_in(t, at_i, fframes::svgr!(<g>
            <circle cx="186" cy={y - 8.0} r="14" fill={AMBER_BG} stroke={AMBER} stroke-width="2" />
            {mono(186.0, y - 2.0, 16.0, AMBER, "middle", "?")}
            {text(214.0, y, 22.0, 500, AMBER, "start", s)}
        </g>)));
    }
    let excerpt = rise_in(
        t,
        a2,
        fframes::svgr!(<g>
            {card(130.0, 258.0, 1000.0, 520.0, Some(GREEN), PANEL, LINE)}
            {mono(166.0, 304.0, 16.0, GREEN, "start", "SITE REPORT TEST · GROK-4.7+MODULATION · EXCERPT")}
            {text(166.0, 344.0, 20.0, 500, MUTED, "start", "established")}
            {text(166.0, 548.0, 20.0, 500, MUTED, "start", "named as not established")}
            {items}
        </g>),
    );
    let side = [
        ("Precision", "The verdict attaches to the individual assertion."),
        ("Accountability", "The record identifies established and unsettled claims."),
        ("Review", "The reader sees what still requires examination."),
    ];
    let mut cards = Vec::new();
    for (i, (title, body)) in side.iter().enumerate() {
        let y = 258.0 + i as f32 * 178.0;
        cards.push(rise_in(t, 1.2 + i as f32 * 1.2, fframes::svgr!(<g>
            {card(1160.0, y, 630.0, 160.0, Some(BLUE), PANEL, LINE)}
            {text(1192.0, y + 52.0, 26.0, 500, BLUE, "start", title)}
            {para(f, ctx, 1192, (y + 96.0) as i32, 570, 21, 400, INK, body)}
        </g>)));
    }
    page(
        t,
        "The record of adjudication",
        "Excerpt verbatim from the site report pair of the paired tests, 3 November 2026.",
        fframes::svgr!(<g>{cards}{excerpt}</g>),
    )
}

/// 14 · Sources and technical record.
fn s14<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let src = [
        ("AI adoption and investment", "Stanford HAI · The 2026 AI Index Report · Economy", "hai.stanford.edu/ai-index/2026-ai-index-report/economy"),
        ("Measured workplace productivity", "Brynjolfsson, Li and Raymond · Generative AI at Work · QJE 2025", "doi.org/10.1093/qje/qjae044"),
        ("Language model risks and controls", "NIST AI 600-1 · Generative Artificial Intelligence Profile · July 2024", "doi.org/10.6028/NIST.AI.600-1"),
        ("AMDG mechanism and test results", "Eckhart Diestel · AMDG pamphlet · November 2026 · paired tests 3 Nov 2026 · UR5e 11 Oct 2026", "github.com/ediestel/llm_modulation_three_pairs · github.com/ediestel/llm_modulation_example"),
    ];
    let mut out = Vec::new();
    for (i, (what, title, link)) in src.iter().enumerate() {
        let y = 290.0 + i as f32 * 136.0;
        out.push(rise_in(t, 1.0 + i as f32 * 0.6, fframes::svgr!(<g>
            {text(130.0, y, 24.0, 500, AMBER, "start", what)}
            {text(130.0, y + 38.0, 22.0, 400, INK, "start", title)}
            {mono(130.0, y + 72.0, 17.0, BLUE, "start", link)}
        </g>)));
    }
    page(t, "Sources and technical record", "External sources accessed on 3 November 2026.", fframes::svgr!(<g>{out}</g>))
}

/// 15 · Closing (reworked: one statement).
fn s15<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let c1 = enter(t, 0.3, 0.8);
    let c2 = enter(t, 1.1, 0.8);
    let rule = enter(t, 2.0, 0.9);
    let c3 = enter(t, 2.6, 0.8);
    fframes::svgr!(<g>
        <g opacity={c1} transform={tr(0.0, (1.0 - c1) * 24.0)}>{text(960.0, 390.0, 110.0, 500, INK, "middle", "AMDG Engineers")}</g>
        <g opacity={c2} transform={tr(0.0, (1.0 - c2) * 20.0)}>{text(960.0, 490.0, 54.0, 500, AMBER, "middle", "Every claim carries its verdict.")}</g>
        <rect x={960.0 - 110.0 * rule} y="556" width={(220.0 * rule).max(0.5)} height="4" rx="2" fill="url(#amberBar)" opacity={rule} />
        <g opacity={c3}>
            <text x="960" y="640" font-family={SANS} font-size="28" font-weight="400" font-style="italic" fill={MUTED} text-anchor="middle">"Ad maiorem Dei gloriam · ordered to the bonum commune"</text>
            {text(960.0, 690.0, 24.0, 400, MUTED, "middle", "Eckhart Diestel, Dr. univ. Gent, MD · November 2026")}
        </g>
    </g>)
}
