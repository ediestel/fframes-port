# illustrator: a figure as layers, back and forth with an illustrator

A picture becomes a stack of layers that an illustrator takes into Procreate or a vector app,
draws on, and hands back; every layer comes back as its own SVG group, and fframes renders the
stack. Every step runs on its own or all in a row; nothing leaves the machine.

One figure is one folder, the exchange folder:

```
figures/dix_hallpike/
  figure.toml        the settings and the layers, bottom to top (see the file)
  source.png         the picture
  trace/             ours: pieces.json and trace.svg (groups outlines and pieces), flat.png,
                     lines.json and lines.svg (the centrelines), <name>.json per traced artist PNG
  export/            ours, for the artist: <figure>.psd with every layer the manifest uses,
                     named and in order (opens in Procreate, Photoshop, Affinity, Krita, GIMP
                     with the stack intact; needs ImageMagick's `magick`), the same layers as
                     PNGs (01_source, 02_pieces, 03_outlines, 04_lines, 05_contours, the ones
                     used), composite.png, README.txt with the steps
  10_draft.png ...   the artist's layers, PNG or SVG, dropped next to source.png
```

The stages, from the repository root (`F=examples/illustrator/figures/dix_hallpike`):

| command | what |
| --- | --- |
| `cargo run --release -p illustrator -- $F ocr` | the lettering of the picture to `labels.toml` (tesseract), once; then it is the author's file (see Labels) |
| `cargo run --release -p illustrator -- $F trace` | the picture to vector pieces with fframes-vector (potrace) at `scale`, and the centrelines of its outlines; with a `labels.toml`, the lettering is erased first (`trace/clean.png`) |
| `... $F export` | the layers as PNGs at the canvas size, and the README for the artist |
| the artist draws | layers on top, exported as PNG files (Procreate: Share, PNG files) or SVG, copied into the folder |
| `... $F import` | the artist's PNGs that a layer asks to trace become vector pieces; every file is checked (size, type) |
| `... $F all` | trace, export, import; first ocr when a `labels` layer has no `labels.toml` yet |
| `... $F frame 0`, `render`, `inspect`, `preview` | the stack, one `<g id="name" opacity>` per layer, fitted into 1920 x 1080 on cream paper |

## Layers

`kind` in `figure.toml`: `source` (the picture), `trace-pieces` (the colour pieces of the
trace), `trace-contours` (the borders of the colour pieces as strokes and the outline pieces
filled: the contour drawing of the picture, the sketch the artist begins on; `05_contours.png`
in the export), `trace-outlines` (the outline pieces, the filled dark band of every line),
`trace-lines` (the centre of every outline as a stroke of `line_width` canvas pixels), `file`
(a PNG or SVG the artist drew). Each layer has an `opacity`.

potrace traces shapes, it never draws a line along a stroke: for a picture whose lines are
real black ink, `trace-lines` gives the line art; for a painted or photographed picture, or a
web picture with grey two-pixel contours like Dix-Hallpike, `trace-contours` does, with
coarse `tol` and `min_piece` so the colour areas are few and large.

Artist files not listed are added on top in name order as image layers. A listed file can set
`opacity` and `trace`: `outlines` keeps the line colour of a PNG as vector lines, `pieces`
keeps every colour but the transparent background, `none` keeps the pixels. An SVG file is
drawn once at the canvas size and used like a PNG of the artist's (the renderer draws images,
not foreign SVG trees). A listed file that is missing, a PNG that is not the canvas size, or a
trace without a previous `import` stops the command with the list of problems.

## Labels

A picture with lettering in it (an AI figure with labels) keeps its words as text: the layer
kind `labels` sets `labels.toml` on top, one font, sharp at any size, and the trace stage
erases the lettering underneath first, so the traced picture carries none.

`ocr` writes `labels.toml` with tesseract (`brew install tesseract`; an external program, nothing
linked). The picture is read at 3x in grey with the contrast stretched, so coloured and
white-on-colour lettering reads too. Words of one line become one `[[label]]`; shapes read as
letters (short words of middling confidence) and icons read as signs are left out. Every label
has `text`, its box `x y w h` and `size` and `baseline` in source pixels, `color`, `bold`,
`erase` (paint the lettering over before tracing), `show` (set the text), `conf` (the OCR's
confidence) and optionally `fill`, the colour to paint over with when the colour around the box
is not the one behind the text (white text in a small coloured box).

One rule for every figure: headings in capitals are bold, the rest medium, and labels within a
quarter of the median size of their kind take that size. A label shrinks only when the font sets
it more than a fifth wider than its box. Erasing paints over the strokes of the lettering only,
inside a box that hugs it, so arrows and drawings beside the text stay.

The file is the author's after the first `ocr`: correct a text, set `show = false` to drop a
label (its lettering stays erased), or add a `[[label]]` by hand for lettering the OCR missed.
`ocr` never overwrites it; a new reading goes to `trace/ocr.toml`.

## Settings

The canvas is the picture times `scale` (Dix-Hallpike: 675 x 372 px at 3 = 2025 x 1116 px). The
artist's app must use exactly that size; `export/README.txt` says so.

`[trace]` overrides the fframes-vector preset: `dark` (a pixel darker than this fraction of
full brightness is line; this picture's contours are dark grey, so 0.6 instead of the preset's
0.42), `line_w`, `speck`, `tol`, `min_piece`, `colors`, `turd`, `turd_outline`, `alpha`, `opt`,
and for the centrelines `line_width`, `line_color`, `line_min`. The centrelines are the outline
mask of the flattened picture thinned to one pixel (Zhang-Suen), walked into polylines and
simplified (`src/lines.rs`); potrace fills the whole dark band of an anti-aliased line, the
stroke along its middle has the width you choose.

The two test files in the Dix-Hallpike folder, `10_draft.png` (two black frames, traced to
vector lines) and `20_arrow.svg` (a terracotta arrow, drawn as it is), stand in for the
artist's layers.
