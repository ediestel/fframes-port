# How can an LLM help with room management and booking?

Revised video script · four minutes · revision of `korntal_room_booking_video_script.md` (the original is unchanged)

Video and service offered by **AMDG Engineers**. Proposed institutional setting: AWM / CIU Korntal.

## What the video has to convey

1. **Pseudonymisation.** Before any content is sent to the LLM, names, contact details and internal references are replaced by tokens.
2. **Data remain local.** The original data and the key that links tokens to people remain on Korntal's systems. Only tokens and the facts a task requires are transmitted.
3. **Prompts and the LLM carry out the procedure.** Input data go in; editable prompts guide the LLM; the LLM's structured answers are the output data. No dedicated booking application is built.
4. **A simple interface.** An input folder (request, database export) and an output folder (reply, record). A direct database interface is possible but not required.
5. **Validation is part of the prompt series.** A checking prompt has the LLM verify its proposal against the supplied records; reception approves.
6. **Audit trail and provenance.** Every step is logged with its inputs, prompt version and result.

```
 Korntal                                                     LLM service
 INPUT FOLDER ─► tokens ─► prompt series ─────────────────►  LLM
   request        (key stays local)                           │
   DB export                                                  ▼
 OUTPUT FOLDER ◄─ tokens matched locally ◄─ structured answer ┘
   reply to guest · database record          all steps logged (audit trail)
```

All guest details, room records, tokens and references are fictional demonstration data.

## Production profile

1920 × 1080, 30 fps, 240 s, 7,200 frames, eight scenes. English narration in a factual register; German example input and output; captions from the narration (at most two lines). Deep navy canvas: amber for prompts, blue for data, violet for the LLM, green for validated results, red for conflicts. From scene 2 a running audit line under the header shows the step just logged. AMDG Engineers credit after the opening question, discreetly during the video, and on the closing card.

| Scene | Timeline | Frames |
| --- | --- | --- |
| 1 · Question and overview | 00:00–00:20 | 0–600 |
| 2 · Input: request and database export | 00:20–00:50 | 600–1500 |
| 3 · Pseudonymisation; data remain local | 00:50–01:25 | 1500–2550 |
| 4 · Prompt and LLM evaluation | 01:25–02:05 | 2550–3750 |
| 5 · Changing the procedure in the prompt | 02:05–02:35 | 3750–4650 |
| 6 · Validation and approval | 02:35–03:00 | 4650–5400 |
| 7 · Output: reply and database record | 03:00–03:40 | 5400–6600 |
| 8 · Audit trail and closing | 03:40–04:00 | 6600–7200 |

## Scene 1 · 00:00–00:20 · Question and overview

**Visual.** The opening question; the credit "Video and service offered by AMDG Engineers"; the overview: Korntal's area on the left (input folder, pseudonymisation, prompt series, output folder), the LLM service on the right, the request path across and the answer path back.

**Narrator.**
> How can an LLM help with room management and booking? AMDG Engineers proposes a service in which editable prompts guide an LLM through the booking procedure. Requests and room records enter through an input folder; the results are written to an output folder.

## Scene 2 · 00:20–00:50 · Input: request and database export

**Visual.** Lea's fictional email ("Fiktive Beispieldaten"; text as in the original) with dates, one person and single room highlighted; saved to the input folder as DEMO-001. The read-only export DEMO-SNAPSHOT-01: DEMO-S1 single 12 m², DEMO-S2 single 18 m², DEMO-D1 double 24 m² booked 17–18 November, DEMO-S3 single 14 m² maintenance 16–17 November; saved to the input folder.

**Narrator.**
> A guest requests a single room for one person from the sixteenth to the nineteenth of November, arriving around six in the evening. The email is saved to the input folder under reference DEMO-001. Beside it, Korntal's database provides a read-only export of the relevant rooms, bookings, holds and maintenance blocks.

## Scene 3 · 00:50–01:25 · Pseudonymisation; data remain local

**Visual.** "Korntal — kontrollierter Bereich" and "Entfernte LLM-Verarbeitung"; identifying items replaced by tokens (GUEST-7Q4M, OP-8N2K, SRC-4B2R); the key and the original data stay on the Korntal side; only the tokenised content crosses; the answer returns with its tokens and is matched locally.

**Narrator.**
> Before any content is sent to the LLM, names, contact details and internal references are replaced by tokens. The key that links tokens to people, and all original data, remain on Korntal's systems. Only the tokens and the facts the task requires are transmitted. The LLM's answers carry the same tokens and are matched to the guest locally.

## Scene 4 · 01:25–02:05 · Prompt and LLM evaluation

**Visual.** The prompt as layers: shared instruction modules (1–8), current task, Korntal's operating rules, room facts (SRC-4B2R), tokenised request, answer format. The task instruction of the original with "Among eligible rooms, prefer the smallest room that satisfies the request." highlighted. The prompt goes to the LLM; the structured answer comes back: DEMO-S1 recommended, DEMO-S2 alternative, DEMO-D1 and DEMO-S3 excluded with their reasons.

**Narrator.**
> The prompt combines the shared instruction modules, the current task, Korntal's operating rules, the room facts, and the tokenised request. It instructs the LLM to check the full stay, occupancy, capacity, room type and closures, and to compare the eligible rooms. The LLM returns its evaluation in a defined structure: DEMO-S1 is recommended, DEMO-S2 is the alternative, and DEMO-D1 and DEMO-S3 are excluded, with reasons taken from the supplied records. This structured answer is the output data of the step.

## Scene 5 · 02:05–02:35 · Changing the procedure in the prompt

**Visual.** Only the selection sentence of the task prompt is edited: "…prefer the largest single room that satisfies the request, to prioritize space for the guest." The same data are evaluated again; the recommendation moves from DEMO-S1 to DEMO-S2 while the excluded rooms stay excluded; prompt revision 2 logged.

**Narrator.**
> Selection rules are part of the prompt and can be changed by authorised staff. Here, the priority is changed from the smallest to the largest suitable single room. With the same request and the same records, the recommendation changes to DEMO-S2. No software change is required; the new prompt version is recorded.

## Scene 6 · 02:35–03:00 · Validation and approval

**Visual.** "Checking prompt": the eight checks of the original with their results; the guest token resolved on the Korntal side for reception; "Geprüften Vorschlag freigeben".

**Narrator.**
> Validation is part of the prompt series. A checking prompt has the LLM verify the proposal against the supplied records: room, dates, capacity, conflicts and the selection rule. Each result is returned with its reason, and reception approves the proposal.

## Scene 7 · 03:00–03:40 · Output: reply and database record

**Visual.** Left: the reply drafted by the LLM, addressed to GUEST-7Q4M; the token is replaced by "Lea Beispiel" on the Korntal side; saved to the output folder.

> Guten Tag Lea Beispiel,
>
> vielen Dank für Ihre Anfrage. Für den 16. bis 19. November 2026 (3 Nächte) haben wir ein Einzelzimmer für Sie vorgemerkt. Ihre Ankunft gegen 18 Uhr haben wir notiert. Bitte teilen Sie uns mit, ob Sie Verpflegung wünschen. Preis und verbindliche Buchung bestätigen wir Ihnen gesondert.
>
> Freundliche Grüße
> Ihre Rezeption

Right: `room-request-OP-8N2K.json`, field by field (operation and source tokens, stay, proposed room DEMO-S2, alternative DEMO-S1, reason, open points, provenance: source snapshot, prompt revision 2, approval); saved to the output folder; Korntal's existing import writes it to the database: DEMO-DB-1042, committed, reservation approval pending.

**Narrator.**
> The approved result produces two outputs. First, a reply to the guest, drafted by the LLM in German and addressed by token; the name is inserted locally before reception sends it. Second, a database record in the agreed JSON format, including its provenance: source snapshot, prompt version and approval. Korntal's existing import transfers the record into the database. The reservation itself remains subject to approval.

## Scene 8 · 03:40–04:00 · Audit trail and closing

**Visual.** The audit trail of the case, entry by entry (request received, snapshot read, tokens issued, prompt revision 1, answer, prompt revision 2, answer, checks, approval, reply written, record written, database entry). Closing card: AMDG Engineers · Prompt-based room management with an LLM · Video and service offered by AMDG Engineers.

**Narrator.**
> Every step is logged with its inputs, prompt version and result, so each decision can be traced to its source. AMDG Engineers offers this service and this video.
