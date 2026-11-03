//! Szene 12 · 06:31–08:05 · Zeitgewinn und Nutzen. Quelle ist die
//! Patientenakte; jedes ausgewählte Dokument entsteht daraus, ohne dass jemand
//! es schreibt; Ärztinnen, Ärzte und Pflege prüfen nur noch und geben frei;
//! der Dienst arbeitet mit hoher Genauigkeit und Zuverlässigkeit. Dann Zahlen
//! aus deutschen Kliniken, jede mit ihrer Quelle auf der Karte:
//! Dokumentationsanteil (DKI 2024, HiMSS), ärztliche Dokumentationszeit je Tag
//! (MB-Monitor 2022, DKI 2024), abweichende Medikation in
//! Entlassunterlagen (Kohortenstudie Belgien 2012), Personalkosten der
//! MD-Prüfungen (DKI 2026), Rückzahlungen 2024 (GKV-Statistik).

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::ui::*;

const DOCS: [&str; 5] = [
    "Entlassbrief",
    "Kodierprüfung",
    "Überweisung",
    "Kollegenbrief",
    "Verlaufsnotiz",
];

pub fn draw<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    // --- the time the staff gains (until 26.8) ---------------------------
    let akte = rise_in(
        t,
        3.6,
        fframes::svgr!(<g>
            {card(130.0, 200.0, 340.0, 250.0, Some(BLUE), PANEL, LINE)}
            {icon_folder(166.0, 236.0, 76.0, 56.0, BLUE, BLUE_BG)}
            {text(166.0, 340.0, 28.0, 500, INK, "start", "Patientenakte")}
            {badge(166.0, 392.0, "QUELLE", BLUE, BLUE_BG, 16.0)}
        </g>),
    );
    let feed = arrow(476.0, 325.0, 534.0, 325.0, prog(t, 4.6, 5.1), BLUE, 3.0);

    let mut choices = Vec::new();
    let mut made = Vec::new();
    for (i, name) in DOCS.iter().enumerate() {
        let y = 270.0 + i as f32 * 52.0;
        let pick = 5.8 + i as f32 * 0.4;
        let tick = prog(t, pick, pick + 0.3);
        let box_fill = mix(BG, GREEN, tick);
        choices.push(fframes::svgr!(<g>
            <rect x="572" y={y - 22.0} width="26" height="26" rx="6" fill={box_fill} stroke={GREEN} stroke-width="2" />
            <path d={format!("M578 {:.1} L584 {:.1} L593 {:.1}", y - 9.0, y - 3.0, y - 16.0)} fill="none" stroke="#FFFFFF" stroke-width="3" stroke-linecap="round" opacity={tick} />
            {text(616.0, y, 22.0, 500, INK, "start", name)}
        </g>));
        let at = 7.4 + i as f32 * 0.35;
        let dy = 214.0 + i as f32 * 62.0;
        let ok = prog(t, 15.8 + i as f32 * 0.22, 16.3 + i as f32 * 0.22);
        let edge = mix(LINE, GREEN, ok);
        made.push(rise_in(
            t,
            at,
            fframes::svgr!(<g>
                <rect x="1100" y={dy} width="560" height="50" rx="12" fill={PANEL} stroke={edge} stroke-width="2" />
                {icon_doc(1118.0, dy + 9.0, 32.0, GREEN)}
                {text(1160.0, dy + 33.0, 21.0, 500, INK, "start", name)}
                {mono(1500.0, dy + 32.0, 16.0, MUTED, "start", "erstellt")}
                {check(1630.0, dy + 25.0, 13.0, ok, GREEN)}
            </g>),
        ));
    }
    let list = rise_in(
        t,
        4.9,
        fframes::svgr!(<g>
            {card(540.0, 200.0, 470.0, 330.0, Some(GREEN), PANEL, LINE)}
            {text(572.0, 236.0, 19.0, 400, MUTED, "start", "ausgewählte Dokumente")}
            {choices}
        </g>),
    );
    let out = arrow(1016.0, 365.0, 1090.0, 365.0, prog(t, 7.2, 7.7), GREEN, 3.0);
    let strike = prog(t, 9.6, 10.2);
    let no_writing = rise_in(
        t,
        9.2,
        fframes::svgr!(<g>
            {text(130.0, 500.0, 22.0, 500, MUTED, "start", "von Hand schreiben")}
            <rect x="128" y="492" width={(206.0 * ease_out(strike)).max(0.5)} height="3" fill={RED} opacity={strike} />
            <g opacity={prog(t, 10.2, 10.6)}>{text(350.0, 500.0, 22.0, 500, GREEN, "start", "entfällt")}</g>
        </g>),
    );

    // review and release
    let review = rise_in(
        t,
        12.7,
        fframes::svgr!(<g>
            {icon_person(166.0, 590.0, 52.0, INK)}
            {text(210.0, 618.0, 22.0, 500, INK, "start", "Ärztinnen, Ärzte, Pflege")}
            {button(560.0, 590.0, 200.0, 56.0, "Prüfen", GREEN, prog(t, 14.2, 14.7))}
            {button(780.0, 590.0, 220.0, 56.0, "Freigeben", GREEN, prog(t, 15.3, 15.8))}
        </g>),
    );

    // precise, reliable, and far less time per case
    let hand = ease_in_out(prog(t, 18.4, 23.0));
    let svc = ease_out(prog(t, 19.4, 20.2));
    let bars = rise_in(
        t,
        18.0,
        fframes::svgr!(<g>
            {text(1100.0, 598.0, 20.0, 500, MUTED, "start", "Zeit je Fall · von Hand")}
            <rect x="1100" y="612" width={(660.0 * hand).max(0.5)} height="22" rx="11" fill={AMBER} opacity="0.85" />
            {text(1100.0, 672.0, 20.0, 500, MUTED, "start", "mit dem Dienst · prüfen und freigeben")}
            <rect x="1100" y="686" width={(110.0 * svc).max(0.5)} height="22" rx="11" fill={GREEN} />
        </g>),
    );
    let quality = fframes::svgr!(<g>
        {rise_in(t, 21.0, badge(1100.0, 758.0, "HOHE GENAUIGKEIT", GREEN, GREEN_BG, 16.0))}
        {rise_in(t, 22.2, badge(1310.0, 758.0, "HOHE ZUVERLÄSSIGKEIT", GREEN, GREEN_BG, 16.0))}
        {rise_in(t, 24.0, text(1100.0, 826.0, 21.0, 500, INK, "start", "oft sorgfältiger, als es von Hand gelingt"))}
    </g>);
    let time_part = during(
        t,
        0.0,
        26.9,
        fframes::svgr!(<g>{akte}{feed}{list}{out}{made}{no_writing}{review}{bars}{quality}</g>),
    );

    // --- the gain in figures from German hospitals (from 27.2) -------------
    // (top-left x, y, width, appears, figure, colour, label, detail, source)
    let tiles: [(f32, f32, f32, f32, &str, &str, &str, &str, &str); 5] = [
        (
            130.0,
            230.0,
            540.0,
            32.8,
            "1/3",
            AMBER,
            "der Arbeitszeit: Dokumentation",
            "Ärzte und Pflege; Doppeldokumentation unter den meistgenannten Problemen",
            "DKI-Umfrage 2024 · HiMSS",
        ),
        (
            700.0,
            230.0,
            540.0,
            43.0,
            "3–4 h täglich",
            AMBER,
            "Dokumentation je Ärztin und Arzt",
            "32 % der Klinikärzte: vier Stunden und mehr am Tag für Verwaltung",
            "MB-Monitor 2022 · DKI 2024 · HiMSS",
        ),
        (
            1270.0,
            230.0,
            520.0,
            54.6,
            "47,6 %",
            RED,
            "der Patienten: Medikation weicht ab",
            "in den Entlassunterlagen, wenn Dokumente einzeln entstehen",
            "Kohortenstudie, Belgien 2012",
        ),
        (
            130.0,
            500.0,
            825.0,
            65.3,
            "236.000 €",
            BLUE,
            "je Haus und Jahr für MD-Prüfungen",
            "rund vier Vollkräfte; bundesweit rund 245 Mio. € im Jahr",
            "DKI-Umfrage 2026",
        ),
        (
            965.0,
            500.0,
            825.0,
            74.4,
            "1,19 Mrd. €",
            BLUE,
            "Rückzahlungen an die Kassen",
            "nach Prüfungen, bundesweit, Abrechnungsjahr 2024",
            "GKV-Statistik",
        ),
    ];
    let mut cards = Vec::new();
    for (x, y, w, at, fig, color, label, detail, src) in tiles {
        let h = 240.0;
        cards.push(rise_in(
            t,
            at,
            fframes::svgr!(<g>
                {card(x, y, w, h, Some(color), PANEL, LINE)}
                {text(x + 36.0, y + 78.0, 54.0, 500, color, "start", fig)}
                {text(x + 36.0, y + 124.0, 23.0, 500, INK, "start", label)}
                {para(f, ctx, (x + 36.0) as i32, (y + 160.0) as i32, (w - 70.0) as usize, 18, 400, MUTED, detail)}
                {mono(x + 36.0, y + h - 18.0, 14.0, FAINT, "start", src)}
            </g>),
        ));
    }
    let heading = rise_in(
        t,
        27.6,
        text(
            130.0,
            200.0,
            26.0,
            500,
            INK,
            "start",
            "Zahlen aus deutschen Kliniken",
        ),
    );
    let one_source = rise_in(
        t,
        82.6,
        fframes::svgr!(<g>
            {check(150.0, 802.0, 18.0, prog(t, 82.8, 83.5), GREEN)}
            {text(184.0, 812.0, 28.0, 500, GREEN, "start", "Eine Quelle · übereinstimmende Dokumente · jede Kodierung auf die Akte zurückführbar")}
        </g>),
    );
    let figures_part = show(
        enter(t, 27.2, 0.4),
        0.0,
        fframes::svgr!(<g>{heading}{cards}{one_source}</g>),
    );

    fframes::svgr!(<g>{time_part}{figures_part}</g>)
}
