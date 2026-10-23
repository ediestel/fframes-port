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

Left out on purpose: project-specific templates (stained glass, medieval city),
client data, and an unfinished refactor of the encoding layer.
