# What I added to fframes

## Missing crates written for fframes 1.0.0-beta.7

fframes 1.0.0-beta.7 did not compile because the `fframes-media` and
`fframes-media-dir-macro` crates were missing. I wrote both so it compiled and
rendered video: FFmpeg video decoding, audio decoding, images, WebVTT
subtitles, and the media directory macro. They are on the `media-crates` branch.

## Implemented on fframes 1.3.0

One commit per feature on this branch (`port/workspace`). Each commit message
explains the design and what was left out.

- **SVG validation.** `SvgDocument` holds a string known to parse;
  `Svgr::from_svg_string` / `from_document` take SVG built at runtime (from a
  template, a file or an LLM). A structural check (empty, malformed XML,
  non-`<svg>` root) reports positioned `ValidationIssue`s, and `validate_video`
  runs at the start of `render()`.
- **Determinism.** `ContentHash`, `InputFingerprint`, `OutputHash`,
  `assert_deterministic`: the same input renders the same bytes, and changed
  input or drifting output is reported. Parity tests check that rendering frame
  by frame matches rendering a range, in any order.
- **SVG parse cache.** A per-worker cache of parsed trees, keyed by a hash of
  the SVG text, so a frame that repeats the previous document is not parsed
  again.
- **Output spec types.** Color space (primaries, transfer, matrix, range),
  pixel format and alpha mode, rational frame rate, audio spec (channel layout,
  bit depth, BWF metadata).
- **Image sequence export.** `render_image_sequence` writes one PNG, TIFF or
  OpenEXR file per frame, in parallel, honouring frame range and scaling.
- **Alpha fix.** Pixels are unpremultiplied before swscale for pixel formats
  with alpha; before, partly transparent pixels came out too dark in ProRes
  4444, VP9 alpha and similar.

## Planned, not implemented yet

- **Encoding layer refactor.** Split the encoder into a codec part that
  produces packets and an output sink that writes them, so one encoder can
  write to a file, a buffer or a stream, and backends other than FFmpeg can
  plug in. Partly done; design, status and remaining steps are on the
  `encoding-refactor` branch.
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
