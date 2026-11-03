# Shared pieces

## `narration/`: the crate `shared-narration`

One resolved schedule for a narrated video, used by Klinik and Korntal (a workspace member,
`shared-narration = { path = "../../shared/narration" }`). Input: the scenes' lengths,
`lines.tsv`, `durations.tsv` (or a reading speed without voice) and the cues `(id, scene,
seconds into the scene)`. Output: scene starts and total, every line with start and end, the
captions cut from the lines, and the voice tracks for the `AudioMap`. The policies the
examples used are settings (`Policy`): 0.35 s between lines of a scene, the caption 0.35 s past
the voice, sentences over 140 characters split at punctuation, optional bridging of caption
gaps under 1 s. Missing text or duration, an unknown scene, a duplicate id and a line past the
end of its scene are errors listed together; the video does not build from guessed values.

## Plain files

Reusable parts copied from the finished Gary film (`gary-video`, 2026-10). Plain files to
copy into a new fframes project, not a crate.

| file | what | needs |
| --- | --- | --- |
| `src/timing.rs` | easing on 0..1: `prog`, `lerp`, `expo_out/in`, `cubic_in_out`, `quart_out`, `spring`, `soft`, `steps`; the top half is Gary's timeline (scene spans, voice cues, 1.25x `SPEED`) as a pattern | `fframes::Frame` |
| `src/ui.rs` | palette and faces, `keyword_column` / `keyword_row` (scrolling brainstorm words), `label`, `grade` (desaturate + tint a photo), `corners` (crop marks), `ecg_line` (live heartbeat line, calm to irregular) | `timing.rs`, the fonts below |
| `src/grain.sksl` | film grain and vignette over every frame; uniforms `uGrain`, `uVignette` | `Shader::sksl(include_str!(..))`, drawn as a full-frame `<image>` on top |
| `src/scenes/geo.rs` | camera flight over OpenStreetMap geometry (centre and scale keys, zoom in log space) | `geo_data.rs` from `tools/geo_build.py`, `timing.rs`, `ui.rs` |
| `tools/geo_build.py` | OSM / OSRM / Nominatim JSON → `src/geo_data.rs` in km (local equirectangular projection) | your own downloads, `LAT0`/`LON0` set to the place |
| `tools/sound_build.py` | synthesised placeholder score and effects: slow pad, watch ticks, bell | Python stdlib |
| `tools/heart_build.py` | one heartbeat (lub-dub) | Python stdlib |
| `tools/stamp_build.py` | stamp thud with a wooden knock | Python stdlib |
| `fonts/` | Archivo Black, Caveat, IBM Plex Mono (Medium, SemiBold), Instrument Serif Italic, all SIL Open Font License | |

Run the tools with `python3 -I tools/<name>.py`; they write into `dynamic_media/` next to `tools/`.
