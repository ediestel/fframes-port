//! Szene · Jede Dokumentart: aus dem Eingangsordner (Patientengespräch und
//! Patientendaten) entsteht über eine Vorlage im Prompt jede Dokumentart,
//! vorformatiert für ihren Empfänger; das Fachwissen bringt das Frontier-LLM mit.

use fframes::{FFramesContext, Frame, Svgr};

use super::common::*;
use crate::ui::*;

/// (Format, Dokumentart, Inhalt)
const DOCS: [(&str, &str, &str); 12] = [
    ("PPTX", "Fallvorstellung", "Präsentation für die Frühbesprechung"),
    ("DOCX", "Fachgutachten", "nach der Fragestellung des Auftraggebers"),
    ("DOCX", "Post-COVID-Nachsorge", "Verlaufsbrief an die Hausarztpraxis"),
    ("DOCX", "Oberärztliche Durchsicht", "Dokumentation jüngerer Kolleginnen und Kollegen"),
    ("DOCX", "Psychotherapie", "Bericht zum Antrag an den Gutachter"),
    ("DOCX", "Neurologie", "Befundbericht mit Verlauf"),
    ("PDF", "Krankenkasse · MD", "Antwort auf eine Anfrage des Medizinischen Dienstes"),
    ("PDF", "Berufsgenossenschaft", "Bericht zum Heilverlauf"),
    ("PDF", "Rentenversicherung", "Befundbericht zum Reha-Antrag"),
    ("PDF", "Private Versicherung", "Antrag auf Kostenübernahme"),
    ("DOCX", "Patienteninformation", "in einfacher Sprache"),
    ("JSON", "Qualitätssicherung", "strukturierter Datensatz"),
];

/// When the narration names the document types (scene-local seconds).
pub const LIST_AT: f32 = 18.8;
pub const LIST_STEP: f32 = 1.6;
pub const EXPERT_AT: f32 = 40.4;

fn format_color(f: &str) -> &'static str {
    match f {
        "PPTX" => AMBER,
        "PDF" => RED,
        "JSON" => GREEN,
        _ => BLUE,
    }
}

pub fn draw<'a>(f: &mut Frame, ctx: &FFramesContext<'a, '_>, t: f32) -> Svgr<'a> {
    // the input folder: the patient interview and the patient data
    let ins = ["Anamnesegespraech.txt", "Patientendaten.txt", "Befunde_Labor.pdf", "Medikation.txt"];
    let mut in_rows = Vec::new();
    for (i, n) in ins.iter().enumerate() {
        in_rows.push(rise_in(
            t,
            6.4 + i as f32 * 0.5,
            fframes::svgr!(<g>{icon_doc(146.0, 296.0 + i as f32 * 52.0, 28.0, BLUE)}{mono(186.0, 320.0 + i as f32 * 52.0, 18.0, INK, "start", n)}</g>),
        ));
    }
    let input = rise_in(
        t,
        0.6,
        fframes::svgr!(<g>
            {card(120.0, 190.0, 460.0, 420.0, Some(BLUE), PANEL, LINE)}
            {icon_folder(146.0, 214.0, 40.0, 30.0, BLUE, BLUE_BG)}
            {mono(200.0, 240.0, 24.0, BLUE, "start", "eingang/")}
            {text(150.0, 272.0, 17.0, 400, MUTED, "start", "Patientengespräch und Patientendaten")}
            {in_rows}
            {text(150.0, 560.0, 17.0, 400, MUTED, "start", "einmal erfasst · für alle Dokumente genutzt")}
        </g>),
    );

    // template + LLM in the middle
    let hub = (750.0, 420.0);
    let mid = rise_in(
        t,
        2.0,
        fframes::svgr!(<g>
            {arrow(582.0, 400.0, 690.0, 414.0, prog(t, 2.4, 3.0), BLUE, 3.0)}
            {llm_node(hub.0, hub.1, 52.0, t, 0.5 + 0.5 * (t * 1.6).sin(), "")}
            {text(hub.0, hub.1 + 92.0, 19.0, 500, AMBER, "middle", "Vorlage im Prompt")}
            {text(hub.0, hub.1 + 118.0, 17.0, 400, MUTED, "middle", "Gliederung · Format · Empfänger")}
        </g>),
    );

    // the document types, named one after another
    let (gx, gy, tw, th) = (880.0, 190.0, 304.0, 128.0);
    let mut tiles = Vec::new();
    for (i, (fmt, name, sub)) in DOCS.iter().enumerate() {
        let col = i % 3;
        let row = i / 3;
        let x = gx + col as f32 * (tw + 16.0);
        let y = gy + row as f32 * (th + 14.0);
        let at = LIST_AT + i as f32 * LIST_STEP;
        let k = (t - at).max(0.0);
        if k <= 0.0 {
            continue;
        }
        let c = format_color(fmt);
        let line = arrow(hub.0 + 58.0, hub.1, x - 6.0, y + th / 2.0, prog(k, 0.0, 0.4), FAINT, 1.5);
        let line = show(1.0 - prog(k, 1.2, 1.8) * 0.75, 0.0, line);
        let s = spring(k * 0.9);
        let body = fframes::svgr!(<g>
            {card(x, y, tw, th, Some(c), PANEL, LINE)}
            {badge(x + 22.0, y + 30.0, fmt, c, PANEL, 14.0)}
            {text(x + 22.0, y + 74.0, 21.0, 500, INK, "start", name)}
            {para(f, ctx, (x + 22.0) as i32, (y + 101.0) as i32, 270, 15, 400, MUTED, sub)}
        </g>);
        tiles.push(line);
        tiles.push(moved(ease_out(prog(k, 0.0, 0.4)), scale_at(x + tw / 2.0, y + th / 2.0, lerp(0.86, 1.0, s)), body));
    }

    // the expert knowledge of the model
    let expert = rise_in(
        t,
        EXPERT_AT,
        fframes::svgr!(<g>
            {card(120.0, 640.0, 700.0, 214.0, Some(VIOLET), PANEL, VIOLET)}
            {text(150.0, 684.0, 21.0, 500, VIOLET, "start", "Fachwissen des Frontier-LLM")}
            {text(150.0, 724.0, 19.0, 400, INK, "start", "Innere Medizin · Neurologie · Psychotherapie")}
            {text(150.0, 756.0, 19.0, 400, INK, "start", "Infektiologie · Sozialmedizin · Begutachtung")}
            {text(150.0, 788.0, 19.0, 400, INK, "start", "Kodierung · Leitlinien · Fachsprache")}
            {text(150.0, 828.0, 17.0, 400, MUTED, "start", "Form und Vorgaben: aus den Prompts der Klinik")}
        </g>),
    );

    fframes::svgr!(<g>{input}{mid}{tiles}{expert}</g>)
}
