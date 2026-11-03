//! Szene 12 · 06:31–07:16 · Zeitgewinn und Kosten. Quelle ist die
//! Patientenakte; jedes ausgewählte Dokument entsteht daraus, ohne dass jemand
//! es schreibt; Ärztinnen, Ärzte und Pflege prüfen nur noch und geben frei;
//! der Dienst arbeitet mit hoher Genauigkeit und Zuverlässigkeit. Abgerechnet
//! wird je erbrachter Leistung, die Kosten für das Haus regelt der Vertrag;
//! schon ein vermiedener Kodierfehler kann den Dienst bezahlen.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::ui::*;

const DOCS: [&str; 5] = ["Entlassbrief", "Kodierprüfung", "Überweisung", "Kollegenbrief", "Verlaufsnotiz"];

pub fn draw<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
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

    // --- what it costs (from 27.2) ----------------------------------------
    let mut units = Vec::new();
    let mut bar_w = 0.0;
    for i in 0..8 {
        let at = 29.0 + i as f32 * 0.32;
        bar_w += enter(t, at, 0.3) * 40.0;
        units.push(rise_in(t, at, icon_doc(170.0 + i as f32 * 62.0, 300.0, 46.0, GREEN)));
    }
    let billing = rise_in(
        t,
        27.4,
        fframes::svgr!(<g>
            {card(130.0, 200.0, 1660.0, 300.0, Some(AMBER), PANEL, LINE)}
            {text(170.0, 258.0, 28.0, 500, AMBER, "start", "Abrechnung je erbrachter Leistung")}
            {units}
            {rise_in(t, 29.4, text(170.0, 410.0, 20.0, 400, MUTED, "start", "zum Beispiel je erstelltem Dokument"))}
            <rect x="170" y="440" width="320" height="14" rx="7" fill={LINE} />
            <rect x="170" y="440" width={bar_w.max(0.5)} height="14" rx="7" fill={AMBER} />
        </g>),
    );
    let contract = rise_in(
        t,
        32.6,
        fframes::svgr!(<g>
            <rect x="1180" y="250" width="560" height="200" rx="16" fill={BG} stroke={AMBER} stroke-width="2" />
            {icon_doc(1214.0, 290.0, 80.0, AMBER)}
            <path d="M1214 412 q14 -16 28 0 t28 0" fill="none" stroke={AMBER} stroke-width="3" stroke-linecap="round" opacity={prog(t, 33.6, 34.2)} />
            {text(1310.0, 326.0, 28.0, 500, INK, "start", "Vertrag")}
            {text(1310.0, 366.0, 20.0, 400, MUTED, "start", "regelt, was es für Ihr Haus kostet")}
        </g>),
    );

    // one avoided coding error can pay for the service
    let tilt = ease_out(prog(t, 38.4, 39.6)) * 6.0;
    let a = tilt.to_radians();
    let (cx, cy, half) = (960.0, 640.0, 420.0);
    let (lx, ly) = (cx - half * a.cos(), cy - half * a.sin());
    let (rx, ry) = (cx + half * a.cos(), cy + half * a.sin());
    let pan = |x: f32, y: f32, color: &str| {
        let c = color.to_owned();
        let c2 = color.to_owned();
        fframes::svgr!(<g>
            <path d={format!("M{x:.1} {y:.1} L{:.1} {:.1} M{x:.1} {y:.1} L{:.1} {:.1}", x - 110.0, y + 54.0, x + 110.0, y + 54.0)} stroke={c} stroke-width="2" />
            <rect x={x - 140.0} y={y + 54.0} width="280" height="10" rx="5" fill={c2} />
        </g>)
    };
    let scale = rise_in(
        t,
        36.8,
        fframes::svgr!(<g>
            <path d={format!("M{cx} {:.1} L{:.1} 830 L{:.1} 830 Z", cy + 8.0, cx - 46.0, cx + 46.0)} fill={PANEL} stroke={LINE} stroke-width="2" />
            <path d={format!("M{lx:.1} {ly:.1} L{rx:.1} {ry:.1}")} stroke={INK} stroke-width="6" stroke-linecap="round" />
            <circle cx={cx} cy={cy} r="10" fill={INK} />
            {pan(lx, ly, AMBER)}
            {pan(rx, ry, GREEN)}
            {text(lx, ly + 100.0, 22.0, 500, AMBER, "middle", "Kosten des Dienstes")}
            <g opacity={enter(t, 37.6, 0.5)}>{text(rx, ry + 100.0, 22.0, 500, GREEN, "middle", "ein vermiedener Kodierfehler")}</g>
        </g>),
    );
    let few = rise_in(t, 40.2, badge(rx + 160.0, ry + 92.0, "+ EINIGE WENIGE", GREEN, GREEN_BG, 15.0));
    let cost_part = show(enter(t, 27.2, 0.4), 0.0, fframes::svgr!(<g>{billing}{contract}{scale}{few}</g>));

    fframes::svgr!(<g>{time_part}{cost_part}</g>)
}
