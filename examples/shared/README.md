# Shared pieces

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
