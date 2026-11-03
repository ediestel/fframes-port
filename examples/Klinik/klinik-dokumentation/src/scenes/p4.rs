//! Szene 4 · 01:30–02:20 · Prompts erstellen die Dokumente.
//! Gemeinsame Anweisungen als Textdateien; aus demselben pseudonymisierten
//! Fall verzweigen die Aufgaben-Prompts nebeneinander, jeder in einem eigenen
//! API-Aufruf; die Antworten sind die Dokumente. Ein ergänzter Satz im
//! Entlassbrief-Prompt ändert den nächsten Brief.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::ui::*;

const SHARED: [&str; 5] = ["00_rolle.md", "01_quellen.md", "02_terminologie.md", "03_sicherheit.md", "04_protokoll.md"];

/// (Aufgaben-Prompt, Dokument, Inhalt)
const TASKS: [(&str, &str, &str); 7] = [
    ("verlaufsnotiz.md", "Verlaufsnotiz", "Tag 3: afebril, CRP rückläufig"),
    ("zusammenfassung.md", "Zusammenfassung", "Pneumonie re. basal, unter Therapie gebessert"),
    ("entlassbrief.md", "Entlassbrief", "an den Hausarzt · Diagnosen · Verlauf · Medikation"),
    ("ueberweisung.md", "Überweisung", "Pneumologie · Kontroll-Röntgen in 6 Wochen"),
    ("kollegenbrief.md", "Kollegenbrief", "an die Diabetologie · Rückfrage Einstellung"),
    ("kodierpruefung.md", "Kodierprüfung", "J18.9 · I10.90 · E11.90 · Belege geprüft"),
    ("terminplanung.md", "Terminvorschlag", "Kontrolle beim Hausarzt am 11.11.2026"),
];

const ROW0: f32 = 222.0;
const STEP: f32 = 82.0;

fn typed(s: &str, t: f32, at: f32, cps: f32) -> String {
    let n = ((t - at).max(0.0) * cps) as usize;
    s.chars().take(n).collect()
}

pub fn draw<'a>(_f: &mut Frame, _ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    let focus = window(t, 29.0, 50.5, 0.6);
    let dim_other = 1.0 - 0.6 * focus;

    // shared instructions
    let mut shared = Vec::new();
    for (i, n) in SHARED.iter().enumerate() {
        shared.push(rise_in(t, 1.0 + i as f32 * 0.4, mono(150.0, 300.0 + i as f32 * 34.0, 19.0, INK, "start", n)));
    }
    let left = show(
        dim_other,
        0.0,
        fframes::svgr!(<g>
            {rise_in(t, 0.6, fframes::svgr!(<g>
                {card(120.0, 190.0, 440.0, 300.0, Some(AMBER), PANEL, LINE)}
                {icon_folder(146.0, 214.0, 40.0, 30.0, AMBER, AMBER_BG)}
                {mono(200.0, 240.0, 24.0, AMBER, "start", "prompts/")}
                {text(150.0, 276.0, 18.0, 400, MUTED, "start", "gemeinsame Anweisungen")}
            </g>))}
            {shared}
            {rise_in(t, 7.6, fframes::svgr!(<g>
                {card(120.0, 520.0, 440.0, 110.0, Some(VIOLET), PANEL, VIOLET)}
                {text(150.0, 560.0, 20.0, 500, VIOLET, "start", "Fall · pseudonymisiert")}
                {mono(150.0, 600.0, 20.0, INK, "start", "PAT-3K9X · CASE-7T2M")}
            </g>))}
        </g>),
    );

    // the API column with the LLM
    let api = rise_in(
        t,
        11.0,
        fframes::svgr!(<g>
            <rect x="1030" y="196" width="60" height="590" rx="30" fill={VIOLET_BG} stroke={VIOLET} stroke-opacity="0.5" stroke-width="1.5" />
            {llm_node(1060.0, 491.0, 38.0, t, 0.6 + 0.4 * (t * 1.8).sin(), "")}
            {text(1060.0, 182.0, 18.0, 500, VIOLET, "middle", "LLM-API")}
        </g>),
    );

    // the branches
    let mut rows = Vec::new();
    for (i, (task, doc, content)) in TASKS.iter().enumerate() {
        let y = ROW0 + i as f32 * STEP;
        let at = 11.8 + i as f32 * 0.5;
        let out_at = 18.6 + i as f32 * 1.25;
        let is_letter = i == 2;
        let o = if is_letter { 1.0 } else { dim_other };
        let fan = prog(t, at - 0.3, at + 0.4);
        let call = prog(t, at + 0.4, at + 1.4);
        let back = prog(t, out_at - 0.6, out_at);
        let rev2 = is_letter && t >= 37.2;
        let content_s = if rev2 { "+ Abschnitt in einfacher Sprache für die Patientin" } else { content };
        let content_fill = if rev2 { AMBER } else { MUTED };
        let row = fframes::svgr!(<g>
            {arrow(562.0, 575.0, 636.0, y + 30.0, fan, VIOLET, 2.0)}
            <g opacity={ease_out(fan)}>
                <rect x="640" y={y} width="330" height="60" rx="12" fill={AMBER_BG} stroke={AMBER} stroke-width="1.5" />
                {mono(660.0, y + 37.0, 18.0, AMBER, "start", task)}
            </g>
            {arrow(972.0, y + 30.0, 1026.0, y + 30.0, call, AMBER, 2.5)}
            {arrow(1094.0, y + 30.0, 1196.0, y + 30.0, back, VIOLET, 2.5)}
            <g opacity={ease_out(back)} transform={tr((1.0 - ease_out(back)) * -20.0, 0.0)}>
                <rect x="1200" y={y} width="624" height="60" rx="12" fill={PANEL} stroke={GREEN} stroke-width="1.5" />
                {text(1222.0, y + 27.0, 21.0, 500, GREEN, "start", doc)}
                {text(1222.0, y + 50.0, 17.0, 400, content_fill, "start", content_s)}
            </g>
            <g opacity={if rev2 { prog(t, 37.2, 37.8) } else { 0.0 }}>{badge(1740.0, y + 18.0, "Rev. 2", AMBER, AMBER_BG, 14.0)}</g>
        </g>);
        rows.push(show(o, 0.0, row));
        if call > 0.0 && call < 1.0 {
            rows.push(fframes::svgr!(<circle cx={lerp(972.0, 1030.0, call)} cy={y + 30.0} r="6" fill={AMBER} />));
        }
    }

    // one sentence added to the discharge-letter prompt
    let l = typed("Ergänze einen Abschnitt in einfacher Sprache für die Patientin.", t, 31.0, 22.0);
    let editor = show(
        focus,
        (1.0 - enter(t, 29.0, 0.7)) * 24.0,
        fframes::svgr!(<g>
            {card(120.0, 650.0, 860.0, 210.0, Some(AMBER), PANEL, AMBER)}
            {mono(150.0, 690.0, 18.0, AMBER, "start", "prompts/entlassbrief.md · Rev. 2")}
            {text(150.0, 734.0, 21.0, 400, MUTED, "start", "Schreibe den Entlassbrief an den Hausarzt in fachlicher Sprache.")}
            {text(150.0, 778.0, 23.0, 500, INK, "start", &l)}
            <g opacity={prog(t, 39.6, 40.2)}>{text(950.0, 836.0, 17.0, 400, MUTED, "end", "gleiche Software · Rev. 2 protokolliert")}</g>
        </g>),
    );

    fframes::svgr!(<g>{left}{api}{rows}{editor}</g>)
}
