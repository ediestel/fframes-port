//! The 24 pages, from the deck "KI für Kodierprüfungen – Nutzen für den
//! Medizinischen Dienst" (Stand 3. November 2026). Every page: the slide's
//! headline, its content as cards, a table, big figures or a chart, and its
//! source note at the foot. Texts and figures as on the slides.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::narration::at;
use crate::ui::*;

type Draw = for<'a> fn(&mut Frame, &FFramesContext<'a, '_>, f32) -> Svgr<'a>;

pub const IDS: [&str; 24] = [
    "M01_Titel", "M02_Kernnutzen", "M03_Ausgaben", "M04_Betraege", "M05_Pruefprozess", "M06_Prueffragen",
    "M07_Kodierpruefung", "M08_Klinisch", "M09_Evidenz", "M10_Kapazitaet", "M11_RegionalerMD", "M12_Wirtschaftlichkeit",
    "M13_GKV", "M14_Streitaufwand", "M15_Beteiligte", "M16_Weitere", "M17_Reform", "M18_Verantwortung",
    "M19_Anwendung", "M20_Stufen", "M21_Kennzahlen", "M22_Annahmen", "M23_Quellen", "M24_Abschluss",
];

pub const DRAWS: [Draw; 24] = [
    s01, s02, s03, s04, s05, s06, s07, s08, s09, s10, s11, s12, s13, s14, s15, s16, s17, s18, s19, s20, s21, s22, s23, s24,
];

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

/// Four cards in a 2 × 2 grid: (title, body), appearing one after another.
fn cards4<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32, items: &[(&str, &str); 4], t0: f32, step: f32) -> Svgr<'a> {
    let mut out = Vec::new();
    for (i, (title, body)) in items.iter().enumerate() {
        let x = 130.0 + (i % 2) as f32 * 845.0;
        let y = 258.0 + (i / 2) as f32 * 296.0;
        out.push(rise_in(
            t,
            t0 + i as f32 * step,
            fframes::svgr!(<g>
                {card(x, y, 815.0, 270.0, Some(AMBER), PANEL, LINE)}
                {text(x + 36.0, y + 62.0, 28.0, 500, AMBER, "start", title)}
                {para(f, ctx, (x + 36.0) as i32, (y + 112.0) as i32, 740, 23, 400, INK, body)}
            </g>),
        ));
    }
    fframes::svgr!(<g>{out}</g>)
}

/// A table: column heads, then rows appearing one after another. Columns are
/// (x, width); the first column is set in amber.
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
    let head_k = enter(t, t0 - 0.4, 0.5);
    let mut head = Vec::new();
    for ((x, _), h) in cols.iter().zip(heads) {
        head.push(text(*x, top + 36.0, 20.0, 500, MUTED, "start", h));
    }
    out.push(show(
        head_k,
        0.0,
        fframes::svgr!(<g>
            <rect x="130" y={top} width="1660" height="54" rx="10" fill={PANEL} />
            {head}
        </g>),
    ));
    let mut y = top + 64.0;
    for (r, row) in rows.iter().enumerate() {
        let n = row.iter().zip(cols).map(|(c, (_, w))| lines(c, s, *w)).max().unwrap_or(1);
        let h = n as f32 * s * 1.3 + 24.0;
        let mut cells = Vec::new();
        for (c, (cell, (x, w))) in row.iter().zip(cols).enumerate() {
            let fill = if c == 0 { AMBER } else { INK };
            let weight = if c == 0 { 500 } else { 400 };
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
fn big(x: f32, y: f32, fig: &str, label: &str, color: &str) -> Svgr<'static> {
    fframes::svgr!(<g>
        {text(x, y, 76.0, 500, color, "start", fig)}
        {text(x, y + 50.0, 24.0, 400, INK, "start", label)}
    </g>)
}

// ---------------------------------------------------------------------------
// the pages

/// 1 · Titel.
fn s01<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let l1 = enter(t, 0.3, 0.8);
    let l2 = enter(t, 0.9, 0.8);
    let sub = enter(t, 1.7, 0.8);
    let meta = enter(t, 2.6, 0.8);
    fframes::svgr!(<g>
        <g opacity={l1} transform={tr(0.0, (1.0 - l1) * 30.0)}>
            <text x="200" y="340" font-family={SANS} font-size="100" font-weight="500" fill={INK} letter-spacing="-2">"KI für Kodierprüfungen"</text>
        </g>
        <g opacity={l2} transform={tr(0.0, (1.0 - l2) * 30.0)}>
            <text x="200" y="452" font-family={SANS} font-size="64" font-weight="500" fill={AMBER}>"Nutzen für den Medizinischen Dienst"</text>
        </g>
        <g opacity={sub}>
            {text(204.0, 548.0, 34.0, 400, INK, "start", "Prozesse, Kapazität und Wirtschaftlichkeit")}
            {text(204.0, 596.0, 34.0, 400, INK, "start", "unter der Annahme vollständiger Zuverlässigkeit")}
        </g>
        <g opacity={meta}>
            <rect x="204" y="650" width="220" height="4" rx="2" fill="url(#amberBar)" />
            {mono(204.0, 704.0, 20.0, MUTED, "start", "Deutschland · Daten 2025 · Stand 3. November 2026")}
        </g>
    </g>)
}

/// 2 · Der größte direkte Nutzen liegt in der Prüfkapazität.
fn s02<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let items = [
        ("Weniger Bearbeitungszeit", "Akten erschließen, Belege zuordnen und Begründungen vorbereiten. Fachpersonal gewinnt Zeit für komplexe Fälle."),
        ("Einheitliche Bewertung", "Dieselbe Dokumentation und dieselbe Regelversion führen zu reproduzierbaren Ergebnissen."),
        ("Weniger Nacharbeit", "Jede Feststellung enthält den Aktenbeleg, die anwendbare Regel und die konkrete Begründung."),
        ("Wirtschaftlicher Einsatz", "Kapazität, tatsächlich vermeidbare Kosten und GKV-Rechnungskorrekturen bilden getrennte Nutzenkonten."),
    ];
    let body = cards4(f, ctx, t, &items, 1.4, 2.0);
    page(t, "Der größte direkte Nutzen liegt in der Prüfkapazität", "Eigene Nutzenanalyse unter der Zuverlässigkeitsannahme.", body)
}

/// 3 · 111,53 Mrd. € GKV-Ausgaben für Krankenhausbehandlung.
fn s03<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let data = [("2021", 85.87, "85,87"), ("2022", 88.11, "88,11"), ("2023", 93.95, "93,95"), ("2024", 102.21, "102,21"), ("2025", 111.53, "111,53")];
    let base = 800.0;
    let scale = 4.0; // px per Mrd €
    let mut bars = Vec::new();
    for (i, (year, v, shown)) in data.iter().enumerate() {
        let x = 200.0 + i as f32 * 190.0;
        let k = ease_out(prog(t, 1.4 + i as f32 * 0.5, 2.4 + i as f32 * 0.5));
        let h = v * scale * k;
        let color = if i == 4 { AMBER } else { BLUE };
        bars.push(fframes::svgr!(<g>
            <rect x={x} y={base - h} width="120" height={h.max(0.5)} rx="8" fill={color} opacity="0.9" />
            <g opacity={prog(k, 0.8, 1.0)}>{text(x + 60.0, base - h - 16.0, 24.0, 500, INK, "middle", shown)}</g>
            {mono(x + 60.0, base + 34.0, 18.0, MUTED, "middle", year)}
        </g>));
    }
    let side = rise_in(
        t,
        at("m03_1") + 4.0,
        fframes::svgr!(<g>
            {text(1220.0, 430.0, 88.0, 500, AMBER, "start", "+29,9 %")}
            {text(1220.0, 480.0, 24.0, 400, INK, "start", "nominaler Anstieg 2021 bis 2025")}
            <g opacity={enter(t, at("m03_2"), 0.6)}>
                {text(1220.0, 600.0, 30.0, 500, INK, "start", "Eine korrekte Vergütung hat")}
                {text(1220.0, 642.0, 30.0, 500, INK, "start", "erhebliches finanzielles Gewicht.")}
            </g>
        </g>),
    );
    let body = fframes::svgr!(<g>
        <rect x="180" y={base} width="980" height="2" fill={LINE} />
        {mono(150.0, 290.0, 16.0, FAINT, "start", "Mrd. €")}
        {bars}
        {side}
    </g>);
    page(t, "111,53 Mrd. € GKV-Ausgaben für Krankenhausbehandlung", "[3] KJ 1. Ausgaben in Mrd. €, nominal. Eigene Veränderungsrechnung.", body)
}

/// 4 · Die Prüfungen bewegen bereits erhebliche Beträge.
fn s04<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let figs = fframes::svgr!(<g>
        {rise_in(t, 1.2, big(130.0, 340.0, "1,24 Mrd. €", "rechnerische Differenzbeträge der GKV-Auswertung 2025", AMBER))}
        {rise_in(t, at("m04_2"), big(975.0, 340.0, "53,7 %", "der DRG-Prüfungen beim MD: Abrechnung nicht korrekt", RED))}
    </g>);
    let rows: [&[&str]; 4] = [
        &["MD: DRG-Prüfungen insgesamt", "1.185.314"],
        &["GKV: beanstandete Schlussrechnungen", "556.490"],
        &["GKV: mittlerer Differenzbetrag", "2.219,51 €"],
        &["GKV: zusätzliche Aufschläge, separat", "183,1 Mio. €"],
    ];
    let tab = table(f, ctx, t, 470.0, &[(160.0, 900.0), (1300.0, 460.0)], &["Kennzahl 2025", ""], &rows, 26, at("m04_2") + 2.5, 0.8);
    page(
        t,
        "Die Prüfungen bewegen bereits erhebliche Beträge",
        "[1] MD: selektierte Prüffälle. [2] GKV 2025: eigene Betragsrechnung, vor späteren Streitkorrekturen. Definitionen: Folie 23.",
        fframes::svgr!(<g>{figs}{tab}</g>),
    )
}

/// 5 · Prüfprozess: Rollen, Regelfristen und KI-Nutzen.
fn s05<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let rows: [&[&str]; 5] = [
        &["Auftrag", "Kasse: direkt binnen 4 Monaten", "Auftrag und Prüfumfang abgleichen"],
        &["Anzeige / Unterlagen", "MD: 2 Wochen. Klinik: 8 Wochen", "Fristen und Vollständigkeit prüfen"],
        &["Sachverhalt / Bewertung", "MD: fachliche Begutachtung", "Belege und Regeln zusammenführen"],
        &["Entscheidung", "Kasse: 9 Monate ab Prüfanzeige", "Begründete Stellungnahme liefern"],
        &["Erörterung", "Kasse / Klinik: nach Bestreiten", "Streitpunkte und Evidenz zuordnen"],
    ];
    let tab = table(
        f,
        ctx,
        t,
        258.0,
        &[(160.0, 440.0), (640.0, 560.0), (1240.0, 540.0)],
        &["Schritt", "Verantwortung / Regelfrist", "Beitrag einer zuverlässigen KI"],
        &rows,
        26,
        1.6,
        1.4,
    );
    page(t, "Prüfprozess: Rollen, Regelfristen und KI-Nutzen", "[4–6] Regulärer Ablauf nach PrüfvV. Fristdetails und Ausnahmen hängen vom Fall ab.", tab)
}

/// 6 · Verweildauerfragen prägen das Prüfvolumen.
fn s06<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let data = [
        ("UGVD medizinisch begründet", 35.4, "35,4 %"),
        ("Stationäre Aufnahme notwendig", 29.3, "29,3 %"),
        ("Gesamte Dauer notwendig", 22.2, "22,2 %"),
        ("Prozeduren korrekt", 21.7, "21,7 %"),
        ("Nebendiagnosen korrekt", 17.4, "17,4 %"),
        ("Hauptdiagnose korrekt", 14.8, "14,8 %"),
    ];
    let x0 = 640.0;
    let per = 28.0; // px per percent
    let mut out = Vec::new();
    for (i, (label, v, shown)) in data.iter().enumerate() {
        let y = 270.0 + i as f32 * 92.0;
        let k = ease_out(prog(t, 1.4 + i as f32 * 0.6, 2.4 + i as f32 * 0.6));
        let w = v * per * k;
        let color = if i < 3 { AMBER } else { BLUE };
        out.push(show(
            enter(t, 1.4 + i as f32 * 0.6, 0.4),
            0.0,
            fframes::svgr!(<g>
                {text(x0 - 30.0, y + 38.0, 25.0, 500, INK, "end", label)}
                <rect x={x0} y={y + 10.0} width={w.max(0.5)} height="40" rx="8" fill={color} opacity="0.9" />
                <g opacity={prog(k, 0.8, 1.0)}>{text(x0 + w + 18.0, y + 40.0, 25.0, 500, INK, "start", shown)}</g>
            </g>),
        ));
    }
    page(
        t,
        "Verweildauerfragen prägen das Prüfvolumen",
        "[1] Tab. 17, 2025. Anteil an 1.185.314 DRG-Prüfungen. Mehrfachfragen je Fall, daher keine Summe von 100 %.",
        fframes::svgr!(<g>{out}</g>),
    )
}

/// 7 · Konkreter Nutzen bei Kodierprüfungen.
fn s07<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let rows: [&[&str]; 4] = [
        &["Haupt- und Nebendiagnosen", "Dokumentation und DKR abgleichen", "Weniger manuelle Aktensuche"],
        &["OPS und Mindestmerkmale", "Jedes Merkmal mit Beleg prüfen", "Vollständiger Merkmalsnachweis"],
        &["Beatmung / Zeitmerkmale", "Zeitachse aus Einträgen rekonstruieren", "Weniger Übertragungs- und Rechenaufwand"],
        &["Zusatzentgelte / Gruppierung", "Mengen, Voraussetzungen und Ergebnis prüfen", "Finanzielle Relevanz schneller erkennen"],
    ];
    let tab = table(
        f,
        ctx,
        t,
        258.0,
        &[(160.0, 480.0), (680.0, 560.0), (1280.0, 500.0)],
        &["Prüfgegenstand", "KI-Leistung", "Vorteil für den MD"],
        &rows,
        26,
        1.6,
        1.8,
    );
    page(t, "Konkreter Nutzen bei Kodierprüfungen", "[7–10] Fachliche Grundlagen. Nutzenwirkungen: eigene Analyse.", tab)
}

/// 8 · Die Aktenauswertung unterstützt auch klinische Prüffragen.
fn s08<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let items = [
        ("Aufnahmeindikation", "Behandlungsbedarf und tatsächlich erforderliche Krankenhausmittel aus der Akte belegen."),
        ("Verweildauer", "Für jeden strittigen Tag Befunde, Maßnahmen und Gründe für eine stationäre Versorgung zusammenführen."),
        ("PEPP", "Behandlungsaufwand, Therapieeinheiten und zeitbezogene Merkmale anhand der einschlägigen Regeln prüfen."),
        ("Entlastung des Fachpersonals", "Eine belastbare Sachverhaltsgrundlage verkürzt Vorbereitung und begründende Dokumentation."),
    ];
    let body = cards4(f, ctx, t, &items, 1.4, 2.0);
    page(t, "Die Aktenauswertung unterstützt auch klinische Prüffragen", "[7] Prüfumfang DRG / PEPP. Potenzielle Wirkung unter der Zuverlässigkeitsannahme.", body)
}

/// 9 · Ein prüfbarer Evidenznachweis senkt die Nacharbeit.
fn s09<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let rows: [&[&str]; 5] = [
        &["Aktenbeleg", "Dokument, Datum und genaue Fundstelle"],
        &["Regelgrundlage", "DKR / ICD / OPS mit passender Version und Geltung"],
        &["Fachliche Begründung", "Erfülltes oder fehlendes Kriterium im konkreten Fall"],
        &["Abrechnungsfolge", "Geprüfte Kodierung und nachvollziehbare Gruppierung"],
        &["Freigabe und Verlauf", "Verantwortliche Person, Änderungen und Zeitstempel"],
    ];
    let tab = table(f, ctx, t, 258.0, &[(160.0, 560.0), (760.0, 1000.0)], &["Bestandteil", "Ergebnis je Feststellung"], &rows, 28, 1.6, 1.6);
    page(t, "Ein prüfbarer Evidenznachweis senkt die Nacharbeit", "[8–10, 15] Regel- und Begründungsanforderungen. Vorgeschlagenes Ausgabeformat.", tab)
}

/// 10 · Kapazität (Folien 10 und 11).
fn s10<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let figs = fframes::svgr!(<g>
        {rise_in(t, 1.2, big(130.0, 340.0, "474,8 VZÄ", "Kodierfachkräfte in der MD-Gemeinschaft", AMBER))}
        {rise_in(t, 2.6, big(975.0, 340.0, "36,1 %", "des gutachterlichen GKV-Ressourceneinsatzes für Krankenhausleistungen", BLUE))}
    </g>);
    let rows: [&[&str]; 3] = [
        &["10 Minuten", "158.042", "99 VZÄ", "12,6 Mio. €"],
        &["20 Minuten", "316.084", "198 VZÄ", "25,3 Mio. €"],
        &["30 Minuten", "474.126", "296 VZÄ", "37,9 Mio. €"],
    ];
    let tab = table(
        f,
        ctx,
        t,
        470.0,
        &[(160.0, 380.0), (600.0, 380.0), (1040.0, 340.0), (1440.0, 340.0)],
        &["Netto je Fall", "Stunden/Jahr", "Kapazität", "Zeitwert bei 80 €/h"],
        &rows,
        28,
        at("m10_2") + 0.4,
        1.6,
    );
    let note = rise_in(t, at("m10_2") + 6.0, text(160.0, 790.0, 22.0, 400, MUTED, "start", "Kapazität ist ein betrieblicher Nutzen. Eine Budgetersparnis entsteht erst durch tatsächlich vermeidbare Ausgaben."));
    page(
        t,
        "Frei werdende Zeit: schon Minuten je Fall ergeben relevante Kapazität",
        "[1] Personalstichtag 31.12.2025; DRG-Volumen 2025. Modell, keine gemessene KI-Wirkung. Annahmen: 80 % Nutzung, 1.600 produktive Stunden/VZÄ.",
        fframes::svgr!(<g>{figs}{tab}{note}</g>),
    )
}

/// 11 · Beispiel für einen regionalen MD mit 100.000 Fällen.
fn s11<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let rows: [&[&str]; 5] = [
        &["Jährliche Nettoentlastung", "26.667 Stunden = 16,7 VZÄ"],
        &["Bewerteter Zeitgewinn", "2,13 Mio. € / Jahr"],
        &["Laufende Zusatzkosten", "1,00 Mio. € / Jahr"],
        &["Kapazitätswert nach laufenden Kosten", "1,13 Mio. € / Jahr"],
        &["Einmalige Einführung", "1,50 Mio. €"],
    ];
    let tab = table(f, ctx, t, 258.0, &[(160.0, 760.0), (1000.0, 760.0)], &["Größe", "Beispielwert"], &rows, 28, 1.6, 1.6);
    let amort = rise_in(
        t,
        at("m11_2"),
        fframes::svgr!(<g>
            {card(130.0, 712.0, 1660.0, 120.0, Some(GREEN), PANEL, LINE)}
            {text(170.0, 760.0, 26.0, 500, GREEN, "start", "Rund 16 Monate kapazitätsbasierte Amortisation ab vollem Hochlauf.")}
            {text(170.0, 800.0, 21.0, 400, MUTED, "start", "Die monetäre Amortisation hängt von der Realisierung des Zeitwerts ab.")}
        </g>),
    );
    page(
        t,
        "Beispiel für einen regionalen MD mit 100.000 Fällen",
        "Planungsannahmen: 100.000 Fälle/Jahr, 80 % Nutzung, 20 Minuten Nettoentlastung, 80 €/h, 1.600 Stunden/VZÄ. Kosten ebenfalls angenommen.",
        fframes::svgr!(<g>{tab}{amort}</g>),
    )
}

/// 12 · Die Wirtschaftlichkeit hängt von der Realisierung ab.
fn s12<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let items = [
        ("Kosten der Einführung", "Schnittstellen, Datenaufbereitung, Vergabe, Validierung und Schulung."),
        ("Kosten des Betriebs", "Lizenzen, Infrastruktur, Modellbetrieb, Regelpflege und laufende Qualitätssicherung."),
        ("Finanzielle Realisierung", "Weniger Fremdleistungen, Mehrarbeit oder künftig nötige Neueinstellungen."),
        ("Saubere Nutzenrechnung", "Zeitwert und Personalkosteneffekt nur einmal zählen. GKV-Erstattungen separat ausweisen."),
    ];
    let body = cards4(f, ctx, t, &items, 1.4, 2.0);
    page(t, "Die Wirtschaftlichkeit hängt von der Realisierung ab", "Eigene Wirtschaftlichkeitslogik. Keine veröffentlichten MD-internen Fallkosten oder KI-Preise unterstellt.", body)
}

/// 13 · GKV-Zusatzeffekte brauchen einen eigenen Nachweis.
fn s13<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let rows: [&[&str]; 3] = [&["+0,5 Prozentpunkte", "12,6 Mio. €"], &["+1,0 Prozentpunkt", "25,3 Mio. €"], &["+2,0 Prozentpunkte", "50,5 Mio. €"]];
    let tab = table(f, ctx, t, 258.0, &[(160.0, 760.0), (1000.0, 760.0)], &["Zusätzlicher Nettoanteil geminderter Fälle", "Rechnerischer Differenzbetrag/Jahr"], &rows, 30, 1.6, 1.4);
    let note = rise_in(
        t,
        at("m13_1") + 3.0,
        fframes::svgr!(<g>
            {card(130.0, 620.0, 1660.0, 180.0, Some(AMBER), PANEL, LINE)}
            {text(170.0, 680.0, 27.0, 500, INK, "start", "Eine höhere Beanstandungsquote allein belegt keine bessere Prüfung.")}
            <g opacity={enter(t, at("m13_2"), 0.6)}>{text(170.0, 740.0, 27.0, 500, GREEN, "start", "Auch die Bestätigung korrekter Rechnungen schafft Wert.")}</g>
        </g>),
    );
    page(
        t,
        "GKV-Zusatzeffekte brauchen einen eigenen Nachweis",
        "Illustrative Sensitivität. [2] 1.138.534 Gutachten 2025. Ein unveränderter mittlerer Differenzbetrag von 2.219,51 € ist unterstellt.",
        fframes::svgr!(<g>{tab}{note}</g>),
    )
}

/// 14 · Nachvollziehbare Gutachten können Streitaufwand senken.
fn s14<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let figs = fframes::svgr!(<g>
        {rise_in(t, 1.2, big(130.0, 380.0, "126.655", "einzelfallbezogene Erörterungen 2025", AMBER))}
        {rise_in(t, 2.6, big(975.0, 380.0, "245 Mio. €", "geschätzte jährliche Personalkosten in Kliniken für MD-Prüfungen", BLUE))}
    </g>);
    let effect = rise_in(
        t,
        at("m14_2"),
        fframes::svgr!(<g>
            {card(130.0, 560.0, 1660.0, 220.0, Some(GREEN), PANEL, LINE)}
            {text(170.0, 612.0, 22.0, 500, GREEN, "start", "Potenzielle Wirkung")}
            {para(f, ctx, 170, 664, 1580, 28, 400, INK, "Klare Aktenbelege und eine konkrete Regelbegründung können Rückfragen, erneute Aktensichtung und Erörterungsaufwand verringern.")}
        </g>),
    );
    page(
        t,
        "Nachvollziehbare Gutachten können Streitaufwand senken",
        "[2] GKV-Jahresstatistik 2025. [11] DKI-Hochrechnung 2026, Allgemeinkrankenhäuser ab 100 Betten.",
        fframes::svgr!(<g>{figs}{effect}</g>),
    )
}

/// 15 · Die Vorteile verteilen sich auf mehrere Beteiligte.
fn s15<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let rows: [&[&str]; 4] = [
        &["Medizinischer Dienst", "Mehr Kapazität, weniger Nacharbeit", "Netto-Minuten und Kosten je Fall"],
        &["Krankenkassen", "Schnellere, belastbare Klärung", "Endgültiger Netto-Differenzbetrag"],
        &["Krankenhäuser", "Weniger Such- und Streitaufwand", "Bearbeitungszeit je Prüffall"],
        &["Versicherte / Solidargemeinschaft", "Effizientere Mittelverwendung", "Reale Ressourcen und korrekte Leistungsgewährung"],
    ];
    let tab = table(
        f,
        ctx,
        t,
        258.0,
        &[(160.0, 500.0), (700.0, 520.0), (1260.0, 520.0)],
        &["Beteiligte", "Potenzielle Vorteile", "Passende Messgröße"],
        &rows,
        26,
        1.6,
        1.8,
    );
    let note = rise_in(t, 10.0, text(160.0, 790.0, 22.0, 400, MUTED, "start", "Der Versorgungsnutzen entsteht mittelbar durch den besseren Einsatz von Ressourcen."));
    page(
        t,
        "Die Vorteile verteilen sich auf mehrere Beteiligte",
        "Eigene Wirkungsanalyse. Die monetären Wirkungen sind für jeden Beteiligten getrennt zu bilanzieren.",
        fframes::svgr!(<g>{tab}{note}</g>),
    )
}

/// 16 · Weitere Prüfaufgaben können dieselbe Basis nutzen.
fn s16<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let rows: [&[&str]; 4] = [
        &["OPS-Strukturen / Leistungsgruppen", "Nachweise Kriterien zuordnen", "Einrichtungsbezug und Vor-Ort-Evidenz"],
        &["Qualitätskontrollen", "Dokumente systematisch auswerten", "Jeweilige G-BA-Anforderung"],
        &["Abrechnung in der Pflege", "Verordnung und Nachweis abgleichen", "Eigene fachliche Prüfregeln"],
        &["Systemberatung / Fortbildung", "Wiederkehrende Streitfragen erkennen", "Zulässige, möglichst aggregierte Datenbasis"],
    ];
    let tab = table(
        f,
        ctx,
        t,
        258.0,
        &[(160.0, 520.0), (720.0, 520.0), (1280.0, 500.0)],
        &["Anwendung", "Übertragbarer Nutzen", "Zusätzliche Voraussetzung"],
        &rows,
        26,
        1.6,
        1.8,
    );
    page(t, "Weitere Prüfaufgaben können dieselbe Basis nutzen", "[1, 7] Aufgabenrahmen. Eigene Ableitung möglicher Anwendungen, jeweils mit eigener Validierung.", tab)
}

/// 17 · Die Reformperspektive erhöht den Wert flexibler Kapazität.
fn s17<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let left = rise_in(t, 1.2, big(130.0, 380.0, "98 %", "der befragten Kliniken erwarten ab 2027 höhere Prüfquoten.", AMBER));
    let right = rise_in(
        t,
        3.0,
        fframes::svgr!(<g>
            {card(975.0, 270.0, 815.0, 230.0, Some(BLUE), PANEL, LINE)}
            {text(1011.0, 320.0, 22.0, 500, BLUE, "start", "BMG-Mitteilung vom 10. Juli 2026")}
            {para(f, ctx, 1011, 370, 740, 26, 400, INK, "Die beschlossene Reform sieht deutlich höhere Prüfquoten und Schwellenwerte vor.")}
        </g>),
    );
    let md = rise_in(
        t,
        at("m17_2"),
        fframes::svgr!(<g>
            {card(130.0, 560.0, 1660.0, 200.0, Some(GREEN), PANEL, LINE)}
            {text(170.0, 610.0, 22.0, 500, GREEN, "start", "Für den MD")}
            {para(f, ctx, 170, 660, 1580, 28, 400, INK, "Lastspitzen mit standardisierter Aktenauswertung auffangen. Das tatsächliche Auftragswachstum bleibt offen.")}
        </g>),
    );
    page(
        t,
        "Die Reformperspektive erhöht den Wert flexibler Kapazität",
        "[11–12] Quellenstand 03.11.2026. Vor Anwendung zählt die gültige Fassung einschließlich Übergangsrecht.",
        fframes::svgr!(<g>{left}{right}{md}</g>),
    )
}

/// 18 · Auch ein zuverlässiges System braucht klare Verantwortung.
fn s18<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let items = [
        ("Verantwortung", "Der MD verantwortet die fachliche Stellungnahme. Die Krankenkasse trifft die leistungsrechtliche Entscheidung."),
        ("Sozialdatenschutz", "Zweckbindung, Zugriffsschutz und Datenminimierung gelten für Akte, Eingabe, Ausgabe und Protokolle."),
        ("Nachvollziehbarkeit", "Versionierte Regeln, gespeicherte Evidenz und dokumentierte Freigabe sichern die Prüfbarkeit."),
        ("Konkreter Verwendungszweck", "AI-Act-Einstufung und Automatisierungsgrad vor dem Einsatz fallbezogen bestimmen."),
    ];
    let body = cards4(f, ctx, t, &items, 1.4, 2.0);
    page(t, "Auch ein zuverlässiges System braucht klare Verantwortung", "[5, 13–15] Governance-Vorschlag. Rechtliche Einordnung nach dem konkreten Verwendungszweck.", body)
}

/// 19 · Der Nutzen entsteht in einer integrierten Anwendung.
fn s19<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let rows: [&[&str]; 5] = [
        &["Fallmanagement und eVV", "Prüfauftrag, Dokumente und Fristen ohne Doppelerfassung"],
        &["LLM / Dokumentenverarbeitung", "Akteninhalt, Zeitachsen und Belege korrekt erschließen"],
        &["Regelbestand und Grouper", "Fassungen korrekt anwenden und Vergütung reproduzieren"],
        &["Arbeitsplatz der Gutachtenden", "Evidenz prüfen, Ausnahmen bearbeiten und freigeben"],
        &["Betrieb und Nachweis", "Zugriffe, Versionen, Qualität und Kosten überwachen"],
    ];
    let tab = table(f, ctx, t, 258.0, &[(160.0, 620.0), (820.0, 960.0)], &["Komponente", "Erforderlicher Beitrag"], &rows, 28, 1.6, 1.6);
    page(t, "Der Nutzen entsteht in einer integrierten Anwendung", "[6, 8–10] Bestehende Standards und Regelquellen. Technisches Zielbild: eigener Vorschlag.", tab)
}

/// 20 · Ein stufenweiser Einsatz macht den Nutzen messbar.
fn s20<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let phases = [
        ("Begrenzter Prüfumfang", "Geeignete eindeutige Fälle und heutige Nettozeiten erfassen"),
        ("Retrospektive Prüfung", "Ergebnisse gegen unabhängig adjudizierte Referenz prüfen"),
        ("Schattenbetrieb", "Fehlende Dokumente, Fristen und Systemintegration testen"),
        ("Kontrollierter Betrieb", "Zeitgewinn, Qualität und Zusatzkosten vergleichend messen"),
        ("Skalierung", "Nur validierte Prüfarten und passende Regelversionen ausrollen"),
    ];
    let mut out = Vec::new();
    let line_k = ease_in_out(prog(t, 1.4, 1.4 + 4.0 * 2.4));
    out.push(fframes::svgr!(<g>
        <rect x="230" y="318" width="1360" height="4" rx="2" fill={LINE} />
        <rect x="230" y="318" width={(1360.0 * line_k).max(0.5)} height="4" rx="2" fill={GREEN} />
    </g>));
    for (i, (title, body)) in phases.iter().enumerate() {
        let x = 130.0 + i as f32 * 340.0;
        let n = format!("{}", i + 1);
        out.push(rise_in(
            t,
            1.4 + i as f32 * 2.4,
            fframes::svgr!(<g>
                <circle cx={x + 100.0} cy="320" r="30" fill={PANEL} stroke={GREEN} stroke-width="3" />
                {mono(x + 100.0, 329.0, 24.0, GREEN, "middle", &n)}
                {card(x, 390.0, 320.0, 360.0, Some(GREEN), PANEL, LINE)}
                {para(f, ctx, (x + 30.0) as i32, 444, 270, 25, 500, INK, title)}
                {para(f, ctx, (x + 30.0) as i32, 560, 270, 20, 400, MUTED, body)}
            </g>),
        ));
    }
    page(
        t,
        "Ein stufenweiser Einsatz macht den Nutzen messbar",
        "Vorgeschlagenes Vorgehen. Zeitplan und Umfang hängen von Datenzugang und Integration ab.",
        fframes::svgr!(<g>{out}</g>),
    )
}

/// 21 · Die Entscheidung braucht wenige belastbare Kennzahlen.
fn s21<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let rows: [&[&str]; 5] = [
        &["Qualität", "Fehlentscheidungen je Prüfart", "Korrekte Bestätigung und Beanstandung"],
        &["Produktivität", "Netto-Minuten je geeignetem Fall", "Echte Entlastung nach Kontrolle"],
        &["Prozess", "Durchlaufzeit und Nacharbeitsquote", "Interne Zeit und Wartezeit trennen"],
        &["Wirtschaftlichkeit", "Gesamtkosten und realisierter Nutzen", "Tragfähiger MD-Business-Case"],
        &["Akzeptanz", "Nutzung und fachliche Übersteuerung", "Einbettung in die Begutachtung"],
    ];
    let tab = table(
        f,
        ctx,
        t,
        258.0,
        &[(160.0, 400.0), (600.0, 580.0), (1220.0, 560.0)],
        &["Dimension", "Messgröße", "Entscheidungsrelevanz"],
        &rows,
        26,
        1.6,
        1.6,
    );
    page(t, "Die Entscheidung braucht wenige belastbare Kennzahlen", "Vorgeschlagene Entscheidungsgrößen. Keine pauschalen Zielwerte oder Erfolgsquoten unterstellt.", tab)
}

/// 22 · Annahmen und Bezugsgrößen.
fn s22<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let items = [
        ("Zuverlässigkeit", "Korrekte Extraktion, Regelanwendung und Begründung im definierten Umfang. Fehlende Akten bleiben eine Informationsgrenze."),
        ("Zwei Statistiken", "MD: fachliche Ergebnisse. GKV: leistungsrechtlich ausgewertete Schlussrechnungen. Die Fallzahlen sind nicht identisch."),
        ("Modellgrenzen", "Zeitgewinn, Nutzungsgrad, Stundensatz und KI-Kosten sind Annahmen. Kapazität und Budgetwirkung unterscheiden sich."),
        ("Finanzielle Bedeutung", "Rechnungskorrekturen sind Zahlungsverschiebungen. Reale Ressourceneffekte entstehen durch vermiedenen Aufwand."),
    ];
    let body = cards4(f, ctx, t, &items, 1.4, 2.0);
    page(t, "Annahmen und Bezugsgrößen", "Methodenhinweise zu den Folien 4, 6, 10–15 und 18.", body)
}

/// 23 · Quellen (Folien 24–26).
fn s23<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let src: [(&str, &str); 15] = [
        ("[1]", "MD Bund: Tätigkeit und Personalausstattung 2024–2025"),
        ("[2]", "GKV-Spitzenverband: Jahresstatistik 2025 nach § 17c KHG"),
        ("[3]", "GKV-Spitzenverband: GKV-Kennzahlen 2025"),
        ("[4]", "SGB V § 275c: Durchführung und Umfang der Prüfungen"),
        ("[5]", "DKG / GKV-SV: Prüfverfahrensvereinbarung"),
        ("[6]", "DKG: Elektronische Vorgangsübermittlung mit dem MD"),
        ("[7]", "MD Bund: Begutachtungsleitfaden DRG und PEPP 2025"),
        ("[8]", "Medizinischer Dienst: SEG 4-Kodierempfehlungen"),
        ("[9]", "BfArM: OPS 2026, Kommentar zur Version"),
        ("[10]", "Selbstverwaltungspartner: Deutsche Kodierrichtlinien 2026"),
        ("[11]", "DKI: Blitzumfrage zu MD-Prüfungen, September 2026"),
        ("[12]", "BMG: Bundestag beschließt BStabG, 10.07.2026"),
        ("[13]", "MD Nordrhein: Was passiert mit meinen Daten?"),
        ("[14]", "EU-Kommission: AI Act Service Desk, Essential Services"),
        ("[15]", "SGB V § 275: Begutachtung und Beratung"),
    ];
    let mut out = Vec::new();
    for (i, (n, s)) in src.iter().enumerate() {
        let col = i / 8;
        let row = i % 8;
        let x = 130.0 + col as f32 * 845.0;
        let y = 290.0 + row as f32 * 66.0;
        out.push(rise_in(
            t,
            1.0 + i as f32 * 0.25,
            fframes::svgr!(<g>
                {mono(x, y, 18.0, AMBER, "start", n)}
                {text(x + 70.0, y, 22.0, 400, INK, "start", s)}
            </g>),
        ));
    }
    page(
        t,
        "Quellen",
        "Alle Quellen abgerufen am 3. November 2026. Vollständige Fundstellen in den Foliennotizen der Präsentation.",
        fframes::svgr!(<g>{out}</g>),
    )
}

/// 24 · Abschluss.
fn s24<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let c1 = enter(t, 0.3, 0.8);
    let c2 = enter(t, 1.0, 0.8);
    let rule = enter(t, 1.8, 0.9);
    let pre = enter(t, 2.4, 0.8);
    fframes::svgr!(<g>
        <g opacity={c1} transform={tr(0.0, (1.0 - c1) * 24.0)}>{text(960.0, 400.0, 84.0, 500, INK, "middle", "KI für Kodierprüfungen")}</g>
        <g opacity={c2} transform={tr(0.0, (1.0 - c2) * 20.0)}>{text(960.0, 490.0, 48.0, 500, AMBER, "middle", "Nutzen für den Medizinischen Dienst")}</g>
        <rect x={960.0 - 110.0 * rule} y="560" width={(220.0 * rule).max(0.5)} height="4" rx="2" fill="url(#amberBar)" opacity={rule} />
        <g opacity={pre} transform={tr(0.0, (1.0 - pre) * 16.0)}>
            <text x="960" y="650" font-family={SANS} font-size="32" font-weight="400" fill={MUTED} text-anchor="middle">"Präsentiert von "<tspan font-weight="500" fill={INK}>"AMDG Engineers"</tspan></text>
        </g>
    </g>)
}
