//! Die 13 Seiten, nach der Präsentation "AI and language models in
//! professional work" (AMDG Engineers, November 2026), auf Deutsch. Die Folien
//! 9, 10 und 12 sind nicht enthalten; nach "Warum AMDG Engineers" folgen die
//! Quellen als vorletzte Seite, der Abschluss wie in der Reihe der AMDG-Videos.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::narration::at;
use crate::ui::*;

type Draw = for<'a> fn(&mut Frame, &FFramesContext<'a, '_>, f32) -> Svgr<'a>;

pub const IDS: [&str; 13] = [
    "G01_Titel", "G02_Anwendungen", "G03_Wert", "G04_Grenzen", "G05_Zuverlaessigkeit", "G06_AMDG", "G07_Anfrage",
    "G08_Lesende", "G09_Felder", "G10_Protokoll", "G11_WarumAMDG", "G12_Quellen", "G13_Abschluss",
];

pub const DRAWS: [Draw; 13] = [s01, s02, s03, s04, s05, s06, s07, s08, s09, s10, s12, s11, s13];

// ---------------------------------------------------------------------------
// gemeinsame Teile

/// Überschrift der Seite und ihre Quellenangabe.
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

/// Grobe Zeilenzahl eines Absatzes.
fn lines(s: &str, size: f32, w: f32) -> usize {
    ((s.chars().count() as f32 * size * 0.52) / w).ceil().max(1.0) as usize
}

/// Ein Leitsatz unter der Überschrift.
fn lead(t: f32, s: &str) -> Svgr<'static> {
    rise_in(t, 0.8, text(130.0, 270.0, 28.0, 400, MUTED, "start", s))
}

/// Vier Karten im 2 × 2-Raster ab `top`: (Titel, Text).
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

/// Drei Karten nebeneinander ab `top`: (Titel, Text).
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
                {para(f, ctx, (x + 34.0) as i32, (top + 62.0) as i32, 466, 30, 500, color, title)}
                {para(f, ctx, (x + 34.0) as i32, (top + 150.0) as i32, 466, 25, 400, INK, body)}
            </g>),
        ));
    }
    fframes::svgr!(<g>{out}</g>)
}

/// Eine Tabelle: Spaltenköpfe, dann Zeilen nacheinander. Spalten: (x, Breite);
/// die erste Spalte in Amber.
#[allow(clippy::too_many_arguments)]
fn table<'a>(
    f: &mut Frame,
    ctx: &FFramesContext<'a, '_>,
    t: f32,
    top: f32,
    cols: &[(f32, f32)],
    heads: &[&str],
    rows: &[&[&str]],
    size: usize,
    t0: f32,
    step: f32,
) -> Svgr<'a> {
    let s = size as f32;
    let mut out = Vec::new();
    let mut head = Vec::new();
    for ((x, _), h) in cols.iter().zip(heads) {
        head.push(text(*x, top + 37.0, 22.0, 500, MUTED, "start", h));
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
            let (fill, weight) = if c == 0 { (AMBER, 500) } else { (INK, 400) };
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

/// Eine große Zahl mit ihrer Beschriftung.
fn big<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, x: f32, y: f32, w: usize, fig: &str, label: &str, color: &str) -> Svgr<'a> {
    fframes::svgr!(<g>
        {text(x, y, 86.0, 500, color, "start", fig)}
        {para(f, ctx, x as i32, (y + 56.0) as i32, w, 26, 400, INK, label)}
    </g>)
}

// ---------------------------------------------------------------------------
// die Seiten

/// 1 · Titel.
fn s01<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let l1 = enter(t, 0.3, 0.8);
    let l2 = enter(t, 0.9, 0.8);
    let sub = enter(t, 1.7, 0.8);
    let meta = enter(t, 2.6, 0.8);
    fframes::svgr!(<g>
        <g opacity={l1} transform={tr(0.0, (1.0 - l1) * 30.0)}>
            <text x="200" y="330" font-family={SANS} font-size="96" font-weight="500" fill={INK} letter-spacing="-2">"KI und Sprachmodelle"</text>
        </g>
        <g opacity={l2} transform={tr(0.0, (1.0 - l2) * 30.0)}>
            <text x="200" y="440" font-family={SANS} font-size="96" font-weight="500" fill={AMBER} letter-spacing="-2">"in der professionellen Arbeit"</text>
        </g>
        <g opacity={sub}>{text(204.0, 532.0, 36.0, 400, INK, "start", "Anwendungen, Wirtschaftlichkeit und die Prüfung jeder Aussage")}</g>
        <g opacity={meta}>
            <rect x="204" y="590" width="220" height="4" rx="2" fill="url(#amberBar)" />
            {text(204.0, 650.0, 30.0, 500, INK, "start", "AMDG Engineers")}
            {text(204.0, 694.0, 24.0, 400, MUTED, "start", "Eckhart Diestel, Dr. univ. Gent, MD · November 2026")}
        </g>
    </g>)
}

/// 2 · Heutige Anwendungen von Sprachmodellen.
fn s02<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let items = [
        ("Dokumente", "Entwerfen, Zusammenfassen, Übersetzen und Extrahieren"),
        ("Wissensarbeit", "Fragen zu Akten, Richtlinien und technischem Material"),
        ("Kundenservice", "Antwortentwürfe und Unterstützung für Servicemitarbeitende"),
        ("Software", "Code erzeugen, erklären und Fehler beheben"),
    ];
    let body = fframes::svgr!(<g>
        {lead(t, "Sprachmodelle sind ein Teil des weiten Felds der KI.")}
        {cards4(f, ctx, t, 310.0, &items, 1.4, 1.6, BLUE)}
    </g>);
    page(t, "Heutige Anwendungen von Sprachmodellen", "NIST AI 600-1, Generative Artificial Intelligence Profile, Juli 2024.", body)
}

/// 3 · KI-Verbreitung und wirtschaftlicher Wert.
fn s03<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let body = fframes::svgr!(<g>
        {rise_in(t, 1.2, big(f, ctx, 130.0, 420.0, 480, "88 %", "der befragten Organisationen nutzten 2025 KI", AMBER))}
        {rise_in(t, 2.8, big(f, ctx, 690.0, 420.0, 480, "285,9 Mrd. $", "private KI-Investitionen in den USA 2025", AMBER))}
        {rise_in(t, at("g03_2"), big(f, ctx, 1250.0, 420.0, 500, "15 %", "mehr gelöste Anliegen pro Stunde in einer Kundenservice-Studie", GREEN))}
    </g>);
    page(
        t,
        "KI-Verbreitung und wirtschaftlicher Wert",
        "Verbreitung und Investitionen: Stanford AI Index 2026. Produktivität: Brynjolfsson, Li und Raymond, QJE 2025, 5.172 Servicemitarbeitende.",
        body,
    )
}

/// 4 · Heutige Grenzen.
fn s04<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let items = [
        ("KI-Fehler", "Erfundene Fakten, Zitate oder technische Details"),
        ("Fehlende Voraussetzungen", "Eine Antwort kann von Angaben abhängen, die die Anfrage nie enthielt"),
        ("Kontextabhängigkeit", "Frühere Gesprächsteile können spätere Antworten beeinflussen"),
    ];
    let body = fframes::svgr!(<g>
        {lead(t, "Eine flüssige Antwort kann eine unbelegte Aussage enthalten.")}
        {cards3(f, ctx, t, 330.0, 400.0, &items, 1.6, 2.4, RED)}
    </g>);
    page(t, "Heutige Grenzen", "NIST AI 600-1, Juli 2024, Abschnitte 2.2 und 3.", body)
}

/// 5 · Wie Zuverlässigkeit heute erreicht wird.
fn s05<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let items = [
        ("Retrieval", "Relevante Dokumente in den Kontext des Modells bringen"),
        ("Externe Werkzeuge", "Berechnungen, Datenbanken und Softwareausführung nutzen"),
        ("Validierung", "Ausgaben testen und Zitate an ihren Quellen prüfen"),
        ("Fachliche Prüfung", "Fachliches Urteil vor einer folgenreichen Verwendung"),
    ];
    let body = cards4(f, ctx, t, 258.0, &items, 1.4, 1.6, BLUE);
    page(t, "Wie Zuverlässigkeit heute erreicht wird", "NIST AI 600-1, Generative Artificial Intelligence Profile, Juli 2024, Abschnitte 2.2 und 3.", body)
}

/// 6 · AMDG Engineers: alle Schritte und Prüfungen in der Schleife über jede
/// Aussage; der Beweis ist die Antwort.
fn s06<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    // die Schleife läuft über die fünf Aussagen und entscheidet über jede
    let t_loop = at("g06_2");
    let sweep0 = t_loop + 3.0;
    let sweep = 6.0;
    let x0 = 166.0;
    let span = 4.0 * 314.0 + 290.0;
    let mut chips = Vec::new();
    for i in 0..5 {
        let x = x0 + i as f32 * 314.0;
        // die Schleife erreicht die Mitte der Aussage
        let hit = sweep0 + sweep * ((x + 145.0 - x0) / span);
        let p = prog(t, hit, hit + 0.6);
        let open = i == 2;
        let edge = if p > 0.0 { if open { AMBER } else { GREEN } } else { LINE };
        let mark = if open {
            show(
                p,
                0.0,
                fframes::svgr!(<g>
                    <circle cx={x + 252.0} cy="600" r="16" fill="none" stroke={AMBER} stroke-width="3" />
                    {mono(x + 252.0, 606.0, 16.0, AMBER, "middle", "?")}
                </g>),
            )
        } else {
            check(x + 252.0, 600.0, 16.0, p, GREEN)
        };
        chips.push(fframes::svgr!(<g>
            <rect x={x} y="570" width="290" height="60" rx="12" fill={BG} stroke={edge} stroke-width="1.5" />
            {text(x + 24.0, 609.0, 24.0, 400, INK, "start", &format!("Aussage {}", i + 1))}
            {mark}
        </g>));
    }
    // nach dem ersten Lauf kreist die Schleife leiser weiter
    let k = if t < sweep0 { 0.0 } else { ((t - sweep0) / sweep).fract() };
    let first = t < sweep0 + sweep;
    let scan_o = enter(t, sweep0 - 0.4, 0.4) * if first { 1.0 } else { 0.45 };
    let scan = show(
        scan_o,
        0.0,
        fframes::svgr!(<g>
            <rect x={x0 + k * span - 2.0} y="558" width="4" height="84" rx="2" fill={VIOLET} />
            <circle cx={x0 + k * span} cy="558" r="6" fill={VIOLET} />
        </g>),
    );
    let body = fframes::svgr!(<g>
        {lead(t, "Der Kern jeder Antwort ist die kritische Prüfung jeder einzelnen Aussage.")}
        {rise_in(t, at("g06_1") + 0.6, badge(130.0, 330.0, "PROPRIETÄRE MODULATION EINES FÜHRENDEN SPRACHMODELLS", VIOLET, VIOLET_BG, 18.0))}
        {rise_in(t, at("g06_1") + 3.0, badge(130.0, 384.0, "DAS ZUGRUNDE LIEGENDE MODELL BLEIBT UNVERÄNDERT", BLUE, BLUE_BG, 18.0))}
        {rise_in(t, t_loop, fframes::svgr!(<g>
            {card(130.0, 436.0, 1660.0, 236.0, Some(VIOLET), PANEL, LINE)}
            {text(166.0, 488.0, 28.0, 500, VIOLET, "start", "Die Schleife über jede Aussage")}
            {text(166.0, 530.0, 24.0, 400, INK, "start", "Alle notwendigen Schritte und Prüfungen sind in die Modulation integriert.")}
            {chips}
            {scan}
        </g>))}
        {rise_in(t, at("g06_3"), fframes::svgr!(<g>
            {check(150.0, 734.0, 20.0, prog(t, at("g06_3") + 0.3, at("g06_3") + 1.0), GREEN)}
            {text(190.0, 748.0, 40.0, 500, GREEN, "start", "Der Beweis für die Qualität der Leistung von AMDG Engineers ist die Antwort selbst.")}
        </g>))}
        {rise_in(t, at("g06_4"), text(190.0, 800.0, 26.0, 400, MUTED, "start", "Beurteilt an der Qualität der Antwort."))}
    </g>);
    page(t, "AMDG Engineers", "AMDG-Quelle: Eckhart Diestel, Broschüre, November 2026.", body)
}

/// 7 · Eine Anfrage mit interner Prüfung.
fn s07<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let boxes = [
        ("Eine Anfrage", "", BLUE, 1.0),
        ("Die Modulationsschleife", "Jede Aussage wird geprüft · offene Aussagen bleiben gekennzeichnet", VIOLET, at("g07_2")),
        ("Eine abschließende Antwort", "Mit Urteil und Kommentar zu jeder Aussage", GREEN, at("g07_2") + 5.0),
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
    let ring = show(
        enter(t, at("g07_2"), 0.5),
        0.0,
        fframes::svgr!(<g transform={format!("rotate({:.1} 960 660)", t * 40.0)}>
            <circle cx="960" cy="660" r="26" fill="none" stroke={VIOLET} stroke-width="3" stroke-dasharray="10 8" />
        </g>),
    );
    page(
        t,
        "Beispiel: So bearbeitet AMDG Engineers eine Anfrage",
        "Die Seite beschreibt die Leistung des Dienstes, nicht seine Umsetzung.",
        fframes::svgr!(<g>{out}{ring}</g>),
    )
}

/// 8 · Was der Leser erhält.
fn s08<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let rows: [&[&str]; 4] = [
        &["Belegt", "Die vollständigen Aussagen und ihre Prüfung"],
        &["Fehlende Bestandteile", "Die Mängel und was fehlt"],
        &["Offen", "Die ungeklärten Punkte und was noch zu prüfen ist"],
        &["Außerhalb des Umfangs", "Die Grenzen dessen, was das System prüfen kann"],
    ];
    let tab = table(f, ctx, t, 258.0, &[(160.0, 560.0), (760.0, 1000.0)], &["Ergebnis der Prüfung", "Was in der Antwort steht"], &rows, 30, 1.6, 1.4);
    let note = rise_in(t, 9.0, text(160.0, 790.0, 28.0, 500, GREEN, "start", "Jede Aussage trägt ihr Ergebnis und ihren Kommentar."));
    page(t, "Was der Leser erhält", "AMDG-Quelle: Eckhart Diestel, Broschüre, November 2026.", fframes::svgr!(<g>{tab}{note}</g>))
}

/// 9 · High-Stakes-Domänen: zwölf Bereiche im 4 × 3-Raster.
fn s09<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let domains = [
        "Medizin und Krankenhäuser",
        "Pharmazie",
        "Recht und Justiz",
        "Versicherung",
        "Banken und Finanzen",
        "Wirtschaftsprüfung und Steuern",
        "Luft- und Raumfahrt",
        "Energie und kritische Infrastruktur",
        "Ingenieur- und Bauwesen",
        "Öffentliche Verwaltung",
        "Sicherheit und Verteidigung",
        "Wissenschaft und Forschung",
    ];
    let mut tiles = Vec::new();
    for (i, name) in domains.iter().enumerate() {
        let x = 130.0 + (i % 4) as f32 * 421.0;
        let y = 300.0 + (i / 4) as f32 * 136.0;
        tiles.push(rise_in(
            t,
            1.4 + i as f32 * 0.35,
            fframes::svgr!(<g>
                {card(x, y, 397.0, 116.0, Some(AMBER), PANEL, LINE)}
                {para(f, ctx, (x + 30.0) as i32, (y + 48.0) as i32, 337, 26, 500, AMBER, name)}
            </g>),
        ));
    }
    let body = fframes::svgr!(<g>
        {tiles}
        {rise_in(t, at("g09_2"), text(160.0, 760.0, 30.0, 500, GREEN, "start", "Jedes Detail einer Antwort hat Konsequenzen."))}
    </g>);
    page(t, "„High-Stakes-Domänen“", "Von AMDG beschriebene Einsatzfelder, keine abgeschlossenen Einführungen oder Zertifizierungen.", body)
}

/// 10 · Das Prüfprotokoll: ein Auszug aus dem Test mit einem Begehungsbericht,
/// ins Deutsche übertragen.
fn s10<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let established = [
        "Der Text gibt an, dass die Dachinspektion am 3. März 2026 stattfand.",
        "Der Text gibt an, dass Mosers Bericht die Inspektion vom 5. März 2026 festhält.",
        "Der Text gibt an, dass der Eigentümer den Bericht am 9. März 2026 annahm.",
    ];
    let open = [
        "Er belegt kein einheitliches Datum der Inspektion.",
        "Er belegt nicht, dass der vorgelegte Text Mosers Bericht ist.",
    ];
    let a2 = at("g10_2");
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
            {mono(166.0, 304.0, 16.0, GREEN, "start", "BEISPIEL · BEGEHUNGSBERICHT · GROK-4.7+MODULATION · ÜBERTRAGEN")}
            {text(166.0, 344.0, 20.0, 500, MUTED, "start", "belegt")}
            {text(166.0, 548.0, 20.0, 500, MUTED, "start", "als nicht belegt benannt")}
            {items}
        </g>),
    );
    let side = [
        ("Präzision", "Das Urteil gilt der einzelnen Aussage."),
        ("Verantwortlichkeit", "Das Protokoll weist belegte und offene Aussagen aus."),
        ("Prüfung", "Der Leser sieht, was noch zu prüfen ist."),
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
        "Beispiel: Das Prüfprotokoll",
        "Beispiel aus dem Begehungsbericht-Test der Paarvergleiche vom 3. November 2026, ins Deutsche übertragen.",
        fframes::svgr!(<g>{cards}{excerpt}</g>),
    )
}

/// 11 · Quellen.
fn s11<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let src = [
        ("KI-Verbreitung und Investitionen", "Stanford HAI · The 2026 AI Index Report · Economy", "hai.stanford.edu/ai-index/2026-ai-index-report/economy"),
        ("Gemessene Produktivität am Arbeitsplatz", "Brynjolfsson, Li und Raymond · Generative AI at Work · QJE 2025", "doi.org/10.1093/qje/qjae044"),
        ("Risiken und Kontrollen von Sprachmodellen", "NIST AI 600-1 · Generative Artificial Intelligence Profile · Juli 2024", "doi.org/10.6028/NIST.AI.600-1"),
        ("AMDG-Verfahren und Testergebnisse", "Eckhart Diestel · AMDG-Broschüre · November 2026 · Paarvergleiche 3. November 2026", "github.com/ediestel/llm_modulation_three_pairs"),
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
    page(t, "Quellen", "Externe Quellen abgerufen am 3. November 2026.", fframes::svgr!(<g>{out}</g>))
}

/// 12 · Warum AMDG Engineers: die Seite der Reihe (Klinik, Kodierprüfung, MD),
/// für eine Organisation formuliert.
fn s12<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let w = 1560;
    let a1 = para(f, ctx, 180, 240, w, 32, 500, AMBER, "\u{201E}AMDG Engineers verbindet automatisierte Dokumentation mit der Prüfung jeder Aussage. Fehler werden erkannt und korrigiert.\u{201C}");
    let g1a = para(f, ctx, 180, 348, w, 28, 400, GREEN, "\u{201E}Ihre Organisation erhält überprüfbare Ergebnisse: mit nachvollziehbaren Quellen, dokumentierten Prüfergebnissen und ausdrücklich ausgewiesenen Prüflücken.\u{201C}");
    let g1b = para(f, ctx, 180, 446, w, 28, 400, GREEN, "\u{201E}Jede Antwort durchläuft einen iterativen Prüf- und Korrekturprozess, bis alle erkannten Falschaussagen entfernt sind.\u{201C}");
    let a2 = para(f, ctx, 180, 650, w, 32, 500, AMBER, "\u{201E}Das System prüft jede Angabe vor der KI-Verarbeitung und kontrolliert die Ergebnisse anschließend so lange, bis alle erkannten Fehler entfernt sind.\u{201C}");
    let g2 = para(f, ctx, 180, 758, w, 28, 400, GREEN, "\u{201E}Das Prüfprotokoll dokumentiert Quellen, Prüfergebnisse und die Aspekte, die nicht geprüft wurden.\u{201C}");
    fframes::svgr!(<g>
        {rise_in(t, 0.5, fframes::svgr!(<g>
            {card(140.0, 180.0, 1640.0, 380.0, Some(AMBER), PANEL, LINE)}
            {a1}
        </g>))}
        {rise_in(t, 4.0, g1a)}
        {rise_in(t, 8.0, g1b)}
        {rise_in(t, 12.0, fframes::svgr!(<g>
            {card(140.0, 590.0, 1640.0, 270.0, Some(AMBER), PANEL, LINE)}
            {a2}
        </g>))}
        {rise_in(t, 16.0, g2)}
    </g>)
}

/// 13 · Abschluss, wie in der Reihe: die Aussage, die Linie, "Präsentiert von
/// AMDG Engineers".
fn s13<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let c1 = enter(t, 0.3, 0.8);
    let rule = enter(t, 1.4, 0.9);
    let pre = enter(t, 2.0, 0.8);
    fframes::svgr!(<g>
        <g opacity={c1} transform={tr(0.0, (1.0 - c1) * 24.0)}>{text(960.0, 470.0, 84.0, 500, AMBER, "middle", "Jede Aussage trägt ihr Urteil.")}</g>
        <rect x={960.0 - 110.0 * rule} y="560" width={(220.0 * rule).max(0.5)} height="4" rx="2" fill="url(#amberBar)" opacity={rule} />
        <g opacity={pre} transform={tr(0.0, (1.0 - pre) * 16.0)}>
            <text x="960" y="650" font-family={SANS} font-size="32" font-weight="400" fill={MUTED} text-anchor="middle">"Präsentiert von "<tspan font-weight="500" fill={INK}>"AMDG Engineers"</tspan></text>
        </g>
    </g>)
}
