//! Die 13 Seiten, nach der Präsentation "AI and language models in
//! professional work" (AMDG Engineers, November 2026), auf Deutsch. Die Folien
//! 9, 10 und 12 sind nicht enthalten; nach "Warum AMDG Engineers" folgen die
//! Quellen als vorletzte Seite, der Abschluss wie in der Reihe der AMDG-Videos.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::narration::at;
use crate::ui::*;

type Draw = for<'a> fn(&mut Frame, &FFramesContext<'a, '_>, f32) -> Svgr<'a>;

pub const IDS: [&str; 15] = [
    "G01_Titel", "G02_Anwendungen", "G03_Wert", "G04_Grenzen", "G05_Zuverlaessigkeit", "G06_AMDG", "G07_Anfrage",
    "G10_Protokoll", "G08_Lesende", "G09_Felder", "G03b_Nutzen", "G03c_Beispiele", "G11_WarumAMDG", "G12_Quellen", "G13_Abschluss",
];

pub const DRAWS: [Draw; 15] = [s01, s02, s03, s04, s05, s06, s07, s10, s08, s09, s03b, s03c, s12, s11, s13];

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
        <g opacity={sub}>{text(204.0, 532.0, 36.0, 400, INK, "start", "Methodik, Anwendungen und Wirtschaftlichkeit")}</g>
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
        ("Dokumente", "Sprachmodelle entwerfen und übersetzen Texte, fassen sie zusammen und werten Dokumente aus."),
        ("Wissensarbeit", "Sie beantworten Fragen zu Akten, Richtlinien und technischen Unterlagen."),
        ("Kundenservice", "Sie formulieren Antworten vor und unterstützen Servicemitarbeiter."),
        ("Software", "Sie schreiben und erklären Code und helfen bei der Fehlerbehebung."),
    ];
    let body = fframes::svgr!(<g>
        {lead(t, "Sprachmodelle sind ein Teilgebiet der künstlichen Intelligenz.")}
        {cards4(f, ctx, t, 310.0, &items, 1.4, 1.6, BLUE)}
    </g>);
    page(t, "Heutige Anwendungen von Sprachmodellen", "NIST AI 600-1, Generative Artificial Intelligence Profile, Juli 2024.", body)
}

/// 3 · KI-Verbreitung und wirtschaftlicher Wert.
fn s03<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let body = fframes::svgr!(<g>
        {rise_in(t, 1.2, big(f, ctx, 130.0, 420.0, 480, "88 %", "der befragten Organisationen setzten 2025 KI ein", AMBER))}
        {rise_in(t, 2.8, big(f, ctx, 690.0, 420.0, 480, "285,9 Mrd. $", "private Investitionen in KI in den USA im Jahr 2025", AMBER))}
        {rise_in(t, at("g03_2"), big(f, ctx, 1250.0, 420.0, 500, "15 %", "mehr gelöste Anliegen pro Stunde in einer Studie im Kundenservice", GREEN))}
    </g>);
    page(
        t,
        "Verbreitung und wirtschaftlicher Nutzen von KI",
        "Verbreitung und Investitionen: Stanford AI Index 2026. Produktivität: Brynjolfsson, Li und Raymond, QJE 2025, 5.172 Servicemitarbeiter.",
        body,
    )
}

/// 3b · Wirtschaftlicher Nutzen: Personal und Finanzen.
fn s03b<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let body = fframes::svgr!(<g>
        {rise_in(t, 0.8, text(130.0, 300.0, 22.0, 500, MUTED, "start", "Personal"))}
        {rise_in(t, 0.8, text(1250.0, 300.0, 22.0, 500, MUTED, "start", "Finanzen"))}
        {rise_in(t, 1.2, big(f, ctx, 130.0, 400.0, 480, "4,5 Std.", "pro Woche kann jede Fachkraft zurückgewinnen, die sie heute für Nacharbeit an KI-Ergebnissen aufwendet", AMBER))}
        {rise_in(t, 2.8, big(f, ctx, 690.0, 400.0, 480, "37 %", "der durch KI gewonnenen Zeit, die heute durch Nacharbeit verloren geht, bleiben erhalten", AMBER))}
        {rise_in(t, 4.4, big(f, ctx, 1250.0, 400.0, 500, "8.900 €", "pro Fachkraft und Jahr lassen sich so einsparen (45 € je Stunde, 44 Arbeitswochen)", BLUE))}
        {rise_in(t, at("g03b_2"), fframes::svgr!(<g>
            {card(130.0, 610.0, 1660.0, 222.0, Some(GREEN), PANEL, LINE)}
            {text(166.0, 656.0, 22.0, 500, GREEN, "start", "Nutzen validierter KI weltweit (Näherung)")}
            {text(166.0, 704.0, 28.0, 400, INK, "start", "Nutzen generativer KI: 2,6 bis 4,4 Bio. $ Wertschöpfung im Jahr")}
            {text(166.0, 748.0, 28.0, 400, INK, "start", "davon heute verloren durch Schadensbegrenzung (Nacharbeit, 37 %): rund 1,0 bis 1,6 Bio. $")}
            {text(166.0, 792.0, 28.0, 500, GREEN, "start", "Validierte KI verringert den Aufwand für die Schadensbegrenzung ganz erheblich.")}
        </g>))}
    </g>);
    page(
        t,
        "Wirtschaftlicher Nutzen der AMDG-Modulation",
        "Nacharbeit: Zapier, Workday / Hanover Research, November 2025. Arbeitskosten: Destatis, April 2026. Wertschöpfung: McKinsey, Juni 2023. Hochrechnung als Näherung.",
        body,
    )
}

/// 3c · Beispiele aus High-Risk-Domänen.
fn s03c<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let items = [
        ("Juristische Recherche", "Lexis+ AI lieferte im Test der Stanford University bei mehr als 17 % der Anfragen falsche Angaben. Jede vermiedene falsche Angabe spart Prüfzeit."),
        ("Medizinischer Dienst", "Steigt der Anteil der zu Recht geminderten Krankenhausrechnungen um 1 Prozentpunkt, ergibt das rechnerisch 25,3 Mio. € im Jahr. Heute stehen je Fall im Schnitt rund 77 Minuten gutachterliche Arbeitszeit zur Verfügung, zu wenig für eine eingehende Prüfung; mit der AMDG-Modulation wird sie mit demselben Personal möglich."),
        ("Jede Institution", "Alle Institutionen in High-Risk-Domänen können mit der AMDG-Modulation erhebliche Kosten einsparen; weltweit wäre die Wirkung enorm."),
    ];
    let body = cards3(f, ctx, t, 290.0, 500.0, &items, 1.2, (at("g03c_2") - 1.2) / 2.0, AMBER);
    page(
        t,
        "Beispiele aus High-Risk-Domänen",
        "Recht: Stanford RegLab, Mai 2024. MD: GKV-Spitzenverband 2025 (2.219,51 € je Fall); MD Bund, Personal und Begutachtungen 2023, eigene Berechnung mit 1.600 Stunden je Vollzeitstelle.",
        body,
    )
}

/// 4 · Heutige Grenzen.
fn s04<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let items = [
        ("KI-Fehler", "Antworten können erfundene Fakten, Zitate oder technische Details enthalten."),
        ("Fehlende Voraussetzungen", "Antworten können auf Angaben beruhen, die in der Anfrage nicht enthalten sind."),
        ("Kontextabhängigkeit", "Frühere Teile eines Gesprächs können spätere Antworten beeinflussen."),
    ];
    let body = fframes::svgr!(<g>
        {lead(t, "Auch eine flüssig formulierte Antwort kann unbelegte Aussagen enthalten.")}
        {cards3(f, ctx, t, 330.0, 400.0, &items, 1.6, 2.4, RED)}
    </g>);
    page(t, "Grenzen heutiger Sprachmodelle", "NIST AI 600-1, Juli 2024, Abschnitte 2.2 und 3.", body)
}

/// 5 · Wie Zuverlässigkeit heute erreicht wird.
fn s05<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let items = [
        ("Retrieval", "Dem Modell werden die für die Anfrage relevanten Dokumente bereitgestellt."),
        ("Externe Werkzeuge", "Rechenprogramme, Datenbanken und andere Software werden eingebunden."),
        ("Validierung", "Ausgaben werden getestet und Zitate anhand ihrer Quellen geprüft."),
        ("Fachliche Prüfung", "Fachleute beurteilen das Ergebnis, bevor es verwendet wird."),
    ];
    let body = cards4(f, ctx, t, 258.0, &items, 1.4, 1.6, BLUE);
    page(t, "Wie Zuverlässigkeit heute erreicht werden soll", "NIST AI 600-1, Generative Artificial Intelligence Profile, Juli 2024, Abschnitte 2.2 und 3.", body)
}

/// 6 · AMDG-Modulation: das Prinzip einmal, dann wie (Abzeichen, Schleife),
/// wann, warum, Wirkung und Nachweis je einmal.
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
    // die Vorteile als Liste, je ein Satz
    let rows = [
        ("Alle Kernaussagen in den Antworten sind für den Leser erkennbar gekennzeichnet.", at("g06_2") + 1.0, VIOLET),
        ("Auch Aussagen, die von der Modulation nicht gesondert geprüft worden sind, sind als solche erkennbar.", at("g06_3"), AMBER),
        ("Die Fachkraft kann gekennzeichnete Aussagen gezielt überprüfen.", at("g06_3") + 8.0, GREEN),
        ("Die Leistungsfähigkeit der AMDG-Modulation lässt sich an der Qualität der gelieferten Antworten nachweisen.", at("g06_4"), GREEN),
    ];
    let mut facts = Vec::new();
    for (i, (s, at_i, color)) in rows.iter().enumerate() {
        let y = 724.0 + i as f32 * 38.0;
        facts.push(rise_in(t, *at_i, fframes::svgr!(<g>
            <circle cx="138" cy={y - 8.0} r="5" fill={color} />
            {text(160.0, y, 24.0, 400, INK, "start", s)}
        </g>)));
    }
    let body = fframes::svgr!(<g>
        {lead(t, "Prüfungen, die andere außerhalb des Sprachmodells vornehmen, erledigt AMDG Engineers innerhalb einer Modulation.")}
        {rise_in(t, at("g06_1") + 0.6, badge(130.0, 330.0, "FÜHRENDE SPRACHMODELLE WERDEN ZUR BEARBEITUNG DER AUFGABEN VERWENDET", VIOLET, VIOLET_BG, 18.0))}
        {rise_in(t, at("g06_1") + 3.0, badge(130.0, 384.0, "DIE SPRACHMODELLE BLEIBEN UNVERÄNDERT", BLUE, BLUE_BG, 18.0))}
        {rise_in(t, t_loop, fframes::svgr!(<g>
            {card(130.0, 436.0, 1660.0, 236.0, Some(VIOLET), PANEL, LINE)}
            {text(166.0, 488.0, 28.0, 500, VIOLET, "start", "Jede Aussage wird einzeln beurteilt")}
            {chips}
            {scan}
        </g>))}
        {facts}
    </g>);
    page(t, "Die Vorteile der AMDG-Modulation", "AMDG-Quelle: Eckhart Diestel, Broschüre, November 2026.", body)
}

/// 7 · Eine Anfrage mit interner Prüfung.
fn s07<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let boxes = [
        ("Eine Anfrage", "Jede Anfrage wird für sich bearbeitet, nicht als Teil eines Gesprächs. Ein Gesprächsverlauf beeinflusst oder verändert die Antwort nicht.", BLUE, at("g07_2")),
        ("Modulierte Antwort", "Die Modulation entfernt KI-Fehler aus der Antwort.", VIOLET, at("g07_3")),
        ("Die abschließende Antwort", "In der Antwort steht zu jeder Aussage das Urteil und ein Kommentar, der es begründet.", GREEN, at("g07_3") + 6.0),
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
        enter(t, at("g07_3"), 0.5),
        0.0,
        fframes::svgr!(<g transform={format!("rotate({:.1} 960 660)", t * 40.0)}>
            <circle cx="960" cy="660" r="26" fill="none" stroke={VIOLET} stroke-width="3" stroke-dasharray="10 8" />
        </g>),
    );
    page(
        t,
        "Beispiel: Wie AMDG Engineers eine Anfrage bearbeitet",
        "Beschrieben wird, was der Dienst leistet, nicht, wie er umgesetzt ist.",
        fframes::svgr!(<g>{out}{ring}</g>),
    )
}

/// 8 · Der QR-Code zum Repository, in dem die drei Tests beschrieben sind.
fn s08<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    // 41 Module mit Ruhezone, 10 px je Modul
    let side = 41.0 * 10.0;
    let qx = 960.0 - side / 2.0;
    let qy = 350.0;
    let url = crate::qr::URL.trim_start_matches("https://");
    let body = rise_in(t, 1.0, fframes::svgr!(<g>
        {crate::qr::qr(qx, qy, 10.0)}
        {mono(960.0, qy + side + 56.0, 22.0, INK, "middle", url)}
    </g>));
    page(t, "AMDG-Qualitätsnachweis der Modulationsmethode", "", body)
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
        {rise_in(t, at("g09_2"), text(160.0, 760.0, 30.0, 500, GREEN, "start", "In diesen Bereichen kann jedes Detail einer Antwort Folgen haben."))}
        {rise_in(t, at("g09_3"), text(160.0, 812.0, 30.0, 500, GREEN, "start", "Die AMDG-Modulation bietet hier entscheidende Vorteile."))}
    </g>);
    page(t, "„High-Stakes-Domänen“", "Auswahl von Bereichen; keine abgeschlossenen Einführungen oder Zertifizierungen.", body)
}

/// 10 · Das Prüfprotokoll: ein Auszug aus dem Test mit einem Begehungsbericht,
/// ins Deutsche übertragen.
fn s10<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let established = [
        "Laut Text fand die Dachinspektion am 3. März 2026 statt.",
        "Laut Text hält Mosers Bericht die Inspektion vom 5. März 2026 fest.",
        "Laut Text nahm der Eigentümer den Bericht am 9. März 2026 an.",
    ];
    let open = [
        "Ein eindeutiges Datum der Inspektion ist nicht belegt.",
        "Nicht belegt ist, dass der vorgelegte Text Mosers Bericht ist.",
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
            {card(130.0, 258.0, 1660.0, 520.0, Some(GREEN), PANEL, LINE)}
            {text(166.0, 304.0, 20.0, 500, GREEN, "start", "Prüfprotokoll zu einem Begehungsbericht, erstellt mit grok-4.7 und AMDG-Modulation")}
            {text(166.0, 344.0, 20.0, 500, MUTED, "start", "belegt")}
            {text(166.0, 548.0, 20.0, 500, MUTED, "start", "nicht belegt")}
            {items}
        </g>),
    );
    page(
        t,
        "Ein Beispiel für ein Prüfprotokoll",
        "Beispiel aus dem Begehungsbericht-Test der Paarvergleiche vom 3. November 2026.",
        fframes::svgr!(<g>{excerpt}</g>),
    )
}

/// 11 · Quellen.
fn s11<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let src = [
        ("KI-Verbreitung und Investitionen", "Stanford HAI · The 2026 AI Index Report · Economy", "hai.stanford.edu/ai-index/2026-ai-index-report/economy"),
        ("Gemessene Produktivität am Arbeitsplatz", "Brynjolfsson, Li und Raymond · Generative AI at Work · QJE 2025", "doi.org/10.1093/qje/qjae044"),
        ("Risiken und Kontrollen von Sprachmodellen", "NIST AI 600-1 · Generative Artificial Intelligence Profile · Juli 2024", "doi.org/10.6028/NIST.AI.600-1"),
        ("Nacharbeit an KI-Ergebnissen", "Zapier · Umfrage unter 1.100 KI-Nutzern · November 2025", "zapier.com/blog/ai-workslop"),
        ("Zeitverlust durch Nacharbeit", "Workday · Beyond Productivity: Measuring the Real Value of AI · 2026", "newsroom.workday.com"),
        ("Arbeitskosten in Deutschland", "Statistisches Bundesamt · Pressemitteilung Nr. 148 · April 2026", "destatis.de/DE/Presse/Pressemitteilungen/2026/04/PD26_148_624.html"),
        ("Fehler juristischer KI-Recherche", "Magesh u. a. · Stanford RegLab · Mai 2024", "arxiv.org/abs/2405.20362"),
        ("Prüfungen des Medizinischen Dienstes", "MD Bund · Bericht über Tätigkeit und Personalausstattung 2022–2023", "md-bund.de"),
        ("Gutachten und Differenzbeträge", "GKV-Spitzenverband · Jahresstatistik 2025 nach § 17c KHG", "gkv-spitzenverband.de"),
        ("Wertschöpfung generativer KI", "McKinsey · The economic potential of generative AI · Juni 2023", "mckinsey.com"),
        ("AMDG-Verfahren und Testergebnisse", "Eckhart Diestel · AMDG-Broschüre · November 2026 · Paarvergleiche 3. November 2026", "github.com/ediestel/llm_modulation_three_pairs"),
    ];
    let mut out = Vec::new();
    for (i, (what, title, link)) in src.iter().enumerate() {
        let x = 130.0 + (i / 6) as f32 * 850.0;
        let y = 280.0 + (i % 6) as f32 * 98.0;
        out.push(rise_in(t, 1.0 + i as f32 * 0.4, fframes::svgr!(<g>
            {text(x, y, 22.0, 500, AMBER, "start", what)}
            {text(x, y + 32.0, 18.0, 400, INK, "start", title)}
            {mono(x, y + 62.0, 14.0, BLUE, "start", link)}
        </g>)));
    }
    page(t, "Quellen", "Externe Quellen abgerufen am 3. und 5. November 2026.", fframes::svgr!(<g>{out}</g>))
}

/// 12 · Warum AMDG Engineers: die Seite der Reihe (Klinik, Kodierprüfung, MD),
/// für eine Organisation formuliert.
fn s12<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let w = 1560;
    let a1 = para(f, ctx, 180, 240, w, 32, 500, AMBER, "\u{201E}AMDG Engineers bearbeitet Anfragen mit führenden Sprachmodellen und entfernt mit der AMDG-Modulation KI-Fehler aus den Antworten.\u{201C}");
    let g1a = para(f, ctx, 180, 348, w, 28, 400, GREEN, "\u{201E}Ihre Organisation erhält Antworten, in denen alle Kernaussagen für den Leser erkennbar gekennzeichnet sind.\u{201C}");
    let g1b = para(f, ctx, 180, 446, w, 28, 400, GREEN, "\u{201E}Jede Anfrage wird für sich bearbeitet, nicht als Teil eines Gesprächs. So werden Drift und Halluzinationen vermieden.\u{201C}");
    let g2 = para(f, ctx, 180, 650, w, 28, 400, GREEN, "\u{201E}Die Leistungsfähigkeit der AMDG-Modulation lässt sich an der Qualität der gelieferten Antworten nachweisen.\u{201C}");
    fframes::svgr!(<g>
        {rise_in(t, 0.5, fframes::svgr!(<g>
            {card(140.0, 180.0, 1640.0, 380.0, Some(AMBER), PANEL, LINE)}
            {a1}
        </g>))}
        {rise_in(t, 4.0, g1a)}
        {rise_in(t, 8.0, g1b)}
        {rise_in(t, 12.0, g2)}
    </g>)
}

/// 13 · Abschluss, wie in der Reihe: die Aussage, die Linie, "Präsentiert von
/// AMDG Engineers".
fn s13<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let c1 = enter(t, 0.3, 0.8);
    let rule = enter(t, 1.4, 0.9);
    let pre = enter(t, 2.0, 0.8);
    fframes::svgr!(<g>
        <g opacity={c1} transform={tr(0.0, (1.0 - c1) * 24.0)}>
            {text(960.0, 400.0, 84.0, 500, AMBER, "middle", "AMDG-Modulation:")}
            {text(960.0, 510.0, 84.0, 500, AMBER, "middle", "Jede Aussage wird einzeln beurteilt.")}
        </g>
        <rect x={960.0 - 110.0 * rule} y="590" width={(220.0 * rule).max(0.5)} height="4" rx="2" fill="url(#amberBar)" opacity={rule} />
        <g opacity={pre} transform={tr(0.0, (1.0 - pre) * 16.0)}>
            <text x="960" y="680" font-family={SANS} font-size="32" font-weight="400" fill={MUTED} text-anchor="middle">"Präsentiert von "<tspan font-weight="500" fill={INK}>"AMDG Engineers"</tspan></text>
        </g>
    </g>)
}
