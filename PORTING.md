# Porting from fframes-workspace

`fframes-workspace` was an earlier fork of fframes that I extended for my own
projects. This branch (`port/workspace`) moves the generally useful parts of it
into fframes, one commit per piece, so each can be reviewed and taken or
dropped on its own.

## Ported (commits on this branch, oldest first)

| Commit | What |
|---|---|
| `6f45a7b` | SVG validation: `SvgDocument` holds a string that is known to parse |
| `f2ed3f0` | Notes on medical figures and posable outlines (`medical-figures-with-fframes.md`) |
| `080c863` | Link the system FFmpeg on Apple Silicon instead of downloading a static copy |
| `9fbb96a` | Determinism: same input renders the same frames |
| `41e33d1` | SVG parse cache |
| `61ef9eb` | Color, pixel format, frame rate and audio spec types |
| `34293c8` | Image sequence export |
| `3414af4` | Unpremultiply pixels before swscale for pixel formats with alpha |

## Not ported yet

These exist in the old workspace and are not in fframes. Tell me which ones
you'd want and I'll port them the same way.

- **Keyframe timelines** (`fframes/src/animation/keyframes.rs`, ~320 lines).
  Spring physics and cubic-bezier easing are already in fframes; keyframes are not.
- **fframes-kit**: helpers on top of fframes.
  - `procedural.rs`: SVG path generators (seeded jitter, ridge path, starfield,
    wave line, radial burst)
  - `defs.rs`: gradient, radial glow and fade `<defs>`
  - `tokens.rs`: design tokens (palette, typography, brand) loaded from JSON
  - `anim.rs`: envelope helpers (fade clamp, easings, pulse, staggered start)
- **fframes-fonts**: a small crate bundling fonts.
- **WASM build**: `wasm-pack.toml` and wasm-bindgen attributes, started but
  not finished.
- **MANUAL.md**: a manual for non-programmers (producers, marketers, data teams).

## Planned, not implemented yet

Planned in fframes-workspace and not implemented since; topics to come back to.

- **Encoding layer refactor.** Split the encoder into a codec part that
  produces packets and an output sink that writes them, so one encoder can
  write to a file, a buffer or a stream, and backends other than FFmpeg can
  plug in. Partly done; design, status and remaining steps are on the
  `encoding-refactor` branch of fframes-port.
- **Authoring helpers.** Design tokens loaded from JSON, procedural SVG
  geometry, gradient and glow `<defs>`, number formatting for on-screen data.
- **Layout.** Bounding boxes and vertical / horizontal stacking instead of
  hand-computed coordinates.
- **Compositing.** Video layers with blend modes.
- **Bundled fonts**, so text renders the same on every machine.
- **Render stages and tracing.** Named stages per frame with a trace hook.
- **Incremental rendering.** Render again only the frames affected by a change.
- **Supersampling** for antialiasing. `RenderSubstrate::Bits16` describes it;
  it is not wired into the renderer yet.
- **Plugin lifecycle.** Explicit setup, per-run and teardown phases.
- **Videos from a brief.** A JSON brief describing scenes, timing, brand and
  audio, with a validator, as input for generating the scene code.

Left out on purpose: project-specific templates (stained glass, medieval city),
client data, and an unfinished refactor of the encoding layer.
