# History

There were two alphas of fframes. This repository is the merge of both.

| Date | |
|---|---|
| 2025-02-13 | Dmitriy Kovalenko publishes fframes `1.0.0-beta.7` on crates.io, unfinished; release candidates follow until 2025-10-30 |
| 2026-04-12 | My alpha, `fframes-workspace` (`0.1.0`): his beta finished and extended |
| 2026-09-28 | His `1.0.0` |
| 2026-10-20 | His `1.3.0`, the base of this repository |
| 2026-10-23 on | My parts merged into his 1.3.0, then fframes-vector |

When his version came out I sent him mine and took his. Some parts here are
his, some are mine; my alpha came first.

## In this repository

- His fframes 1.3.0.
- Merged from my alpha: SVG validation, determinism, SVG parse cache, colour,
  pixel format, frame rate and audio spec types, image sequence export,
  unpremultiplied alpha before swscale (see `PORTING.md`).
- New: `fframes-vector` (pictures to piece-based vector art, objects, sprites,
  glTF), `examples/golden_spurs`, `examples/shared`.

## In this folder

| Folder | What |
|---|---|
| `beta7-media-crates/` | `fframes-media` and `fframes-media-dir-macro` as written to make `1.0.0-beta.7` compile |
| `encoding-refactor/` | The unfinished split of the encoder into codec and output sink; its `README.md` has the design and where it stopped |

Parts of the alpha not merged yet: keyframe timelines, `fframes-kit`,
`fframes-fonts`, the WASM build, `MANUAL.md`, the brief schema.
