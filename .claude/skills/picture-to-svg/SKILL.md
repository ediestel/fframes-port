---
name: picture-to-svg
description: >-
  Regenerate a raster picture (AI image, photo of artwork, stained glass, poster, illustration)
  authentically as a layered vector SVG, optionally rendered as an fframes frame in Rust. Use when
  the user asks to "redo", "regenerate", "trace" or "vectorise" a picture as SVG/Rust, wants it
  "authentic" to the original, or wants an SVG that can be animated later. Pipeline: flat-style
  edit of the original through xAI image edits, 2x upscale, piece-wise flattening (outlines, then
  colour-grown pieces) with a Rust tool, per-colour potrace layers, lead/outlines last.
---

# Picture to SVG

Turn a picture into a vector SVG that matches the original piece for piece. Developed on
`examples/golden_spurs/` in `~/Desktop/Coding_Projects/fframes-main` (Battle of the Golden Spurs
stained-glass window, 2026-10-28). Everything is in this folder: scripts in `scripts/`, the full
reference run (generation, edit, result) in `example/`, and the index, user decisions and planned
animation work in `MANIFEST.md`.

## Rules learned the hard way

- **Trace, never redraw.** Freehand SVG from a verbal description is never an authentic match.
  Every shape must come from the picture.
- **Keep the composition with an image edit, not a new prompt.** Rerunning the original prompt,
  even with only the specs changed, gives a different picture each call. Send the original image
  to the edit endpoint instead.
- **When the user names "the prompt used", find it by bytes.** Match the image file's hash against
  the decoded `b64_json` in stored `image_response.json` files; the matching `image_request.json`
  holds the exact prompt. Do not guess from timestamps or session order.
- **`+dither` before `-colors`.** ImageMagick quantises at `-colors`; `+dither` after it does
  nothing, and every trace comes out speckled. Check this first when a trace shows specks.
- **Small features need pixels.** Faces about 40 px wide lose mouths and eyes. Upscale 2x before
  flattening (Mitchell filter) and double every size threshold.
- **Pieces must also end at colour changes,** not only at dark lines, or fleurs-de-lis on blue
  banners and strokes on faces vanish.
- **Thin mid-tone strokes are lines too.** Face strokes and club bands are brown, not black;
  find them by a black-hat (closing minus image) and keep their own colour, slightly darkened.
- **Look before showing.** Compare source / flattened / traced crops side by side at 2x to 3x
  on faces, small objects (flowers, spurs), and plain areas (sky, water). Never report from the
  numbers alone.
- `df -h /` before big runs: a 2x trace with 96 colours writes about 200 MB of work files.

## The whole process at a glance

| Step | What | Tool | Output |
|---|---|---|---|
| 0 | Generate the picture: query → engine writes the image prompt → Grok Imagine draws it | `scripts/chain.sh` | original PNG |
| 0b | Later, to know which prompt made a picture: match bytes | python, see rules | exact prompt |
| 1 | Flat source: original + instruction → xAI image edit | `scripts/run.sh` | `edit/image.png` |
| 2 | Upscale 2x | `run.sh` | `edit2x.png` |
| 3 | Flatten: outlines, then pieces | `run.sh` → `scripts/flatten` | `flat2x.png` |
| 4 | Trace: per-colour layers, lead last | `run.sh` → `scripts/trace.sh` | `traced.svg` |
| 5 | Render in fframes | `scripts/traced.rs` | frame 0 PNG |
| 6 | Review crops, then clean stale data | magick + browser | |

Steps 1 to 4 in one command (the edit is kept if it exists, so reruns only redo 2 to 4):

```bash
zsh ~/Desktop/Coding_Projects/fframes-main/.claude/skills/picture-to-svg/scripts/run.sh \
  original.png scripts/edit_instruction.txt out/
```

## Pipeline

### 0. Generating the original (how the Golden Spurs picture was made)

The user's query (`scripts/chain_query.md`: "Write detailed instructions that Grok Imagine can
use, whatever you think is appropriate, for this picture: ..." plus a short list of the scene's
contents) goes to the user's NTGT engine (`/evaluate`, `mode_s`, provider `xai`, tier `top@low`).
The engine's reply, the ```text block, is sent unchanged as the `prompt` to
`POST /v1/images/generations` with `grok-imagine-image-quality`, `aspect_ratio` 2:3, `n` 1,
`b64_json`. `scripts/chain.sh mode_s` does both and keeps every request and response; `direct`
does the same with grok-4.7 instead of the engine for comparison. Rules: the query is the user's
wording; do not add knowledge or style hints to it; never change the engine's text before it goes
to the image API; do not read the NTGT app's source.

The same prompt draws a different picture every call (`n` 4 shows it). Once the user likes a
picture, everything after this step works from that picture, never from the prompt again.

### 1. Flat source by image edit (xAI)

Model `grok-imagine-image-quality`, endpoint `POST https://api.x.ai/v1/images/edits`, key from the
Keychain (`security find-generic-password -a api_key_xai -w`). Body:

```json
{"model": "grok-imagine-image-quality", "prompt": "<instruction>",
 "image": {"url": "data:image/png;base64,<original>"},
 "n": 1, "response_format": "b64_json", "resolution": "2k"}
```

Instruction that worked (adapt the nouns to the picture):

```
Keep this stained-glass window exactly as it is: the same <list every element> and every lead
line in the same place, the same colours. Change only the rendering: a flat vector illustration
of the same window in front elevation, straight on, no perspective, filling the picture. Every
pane one even unshaded colour, enclosed by a solid black lead line of uniform width. Painted
detail (faces, folds, hatching) as crisp dark lines. Stone tracery and mullion in one flat dark
grey, plain dark background. No gradients, no glass texture, no glow, no light rays, no halo, no
vignette, no grain, no photographic lighting. Sharp edges, no blur, no noise. Inscriptions <...>
unchanged.
```

Save request, response and image in a run folder. The result is 1664 x 2496 PNG. Show it to the
user before going on.

### 1b. Repair defects in the flat source (before any trace)

Inspect the areas the film will show close up (faces, horses, heraldry, inscriptions) at full
size. Each defect is repaired locally; everything outside the patch stays pixel-identical.

- **Content defects** (a wrong letter, a stray patch of colour): `scripts/fix_region.sh in out
  <crop x y size> <ellipse cx cy rx ry> "<instruction>" <run dir>` sends a square crop to
  images/edits and pastes back only a feathered ellipse. The mask must have no alpha channel,
  or the whole crop is pasted (the script handles it). Locate the defect on a gridded crop
  first; a wrong centre leaves the defect and patches something else.
- **Small colour smudges** the edit model keeps: recolour only the off-colour pixels in a small
  box to the neighbouring colour with `magick -fx` (deterministic, keeps the lines).

Golden Spurs (2026-10-28): blue patch on the brown horse's neck (edit), T of KORTRIJK drawn as
a U/W (edit, wider ellipse to remove the ghost), pink-violet smudge above the fallen knight
(recolour). Result `KI_guldensporenslag_edit_fixed.png`, 13,596 px changed in total.

### 2. Upscale 2x (3x for a film with close-ups)

For close-ups use 3x and scale the size thresholds by 9 (areas) and 3 (widths): see
`MANIFEST.md` for the exact final commands. 3x takes about 10 minutes and gives visibly better
faces, eyes and hair strokes than 2x.

```bash
magick edit.png -filter Mitchell -resize 200% edit2x.png
```

### 3. Piece-wise flattening (`scripts/flatten`, Rust)

Build once: `cd scripts/flatten && cargo build --release --offline` (target dir set in
`.cargo/config.toml`; adjust it for the machine). Then:

```bash
flatten edit2x.png flat2x.png 0.42 48 8 80 24 32
#       in         out        dark min_piece line_w speck tol soft_t
```

1. Outlines: pixels darker than `dark` in thin structures (opening radius `line_w`), blobs of at
   least `speck` px, written pure black. Soft lines: black-hat >= `soft_t`, own colour x 0.8.
2. Pieces: regions grown by colour similarity (RGB distance `tol` to the running mean) between
   outlines; each becomes its mean colour; pieces < `min_piece` px merge into the largest
   neighbour.

At 1x use `0.42 12 4 20 24 32`. Lower `tol` keeps more shading (24 keeps face modelling; 36
flattens faces into one tone). Higher `min_piece` calms grass and mail texture.

### 4. Trace (`scripts/trace.sh`)

```bash
FLAT=1 MEDIAN=0 CC_AREA=0 zsh trace.sh flat2x.png out.svg 96 16 16 23% 66% 24% 7 40 32
#                                      colours stone turd lead... LEAD_W STONE_LEVEL LEAD_TURD
```

FLAT mode: no geometry masks, no stone pass; 96 colours quantised in LAB without dither; one
potrace layer per colour (`<g id="gN" data-color="#rrggbb">`) with a 1 px seam stroke; the lead
layer is exactly the pure-black outline pixels, unstroked, drawn last. Takes about 5 minutes at
2x. At 1x pass turdsize 4 and LEAD_TURD 8.

The non-FLAT mode is for tracing a photograph directly (geometry masks for glass and stone,
contrast-stretched stone pass, grey-lead rule). It is kept but the edit route above is better.

### 5. Into fframes (optional)

`scripts/traced.rs` embeds the SVG with `include_str!`, puts a wall gradient under it and renders
it as frame 0 at 1200 x 1800. Add it as a module plus a binary
(`golden-spurs-traced` in the example) and run:

```bash
cargo run --release --bin <bin> -- frame 0 -o frames_traced
```

### 6. Review and clean up

Crops at 2x to 3x, source / flattened / traced side by side, served from the scratchpad with
`python3 -m http.server 8765 --bind 127.0.0.1` and viewed in Chrome (file:// is blocked). Check
faces, hands and weapons, small objects (flowers, spurs), banners with charges, plain areas, and
the masonry. Afterwards delete the work folders, superseded traces and renders, and stale
release artefacts (`cargo clean --release -p <crate>`); keep the original, the edit request and
response, the piece map, the final SVG and its render, and the code.

## The crate: `fframes-vector` (branch `vector`, 2026-10-28)

Steps 2 to 4 and the cached tree are now one Rust crate in this workspace, `fframes-vector`,
re-exported as `fframes::vector` with the `vector` feature. It replaces `trace.sh`,
`tools/flatten` and `window_tree.rs` (kept in `scripts/` as the record of how it was found).

```bash
cargo build --release -p fframes_vector
fframes-vector vectorize edit_fixed.png window3x.json --scale 3 --colors 96 --svg window3x.svg --flat flat3x.png
```

11 s at 2x, 17 s at 3x (the shell pipeline took 5 and 10 minutes): potrace runs per colour in
parallel, no ImageMagick round trips, quantisation is median cut + k-means in Lab without
dither. Output: the piece document (`PieceDocument`, JSON: palette, one entry per piece with
id, colour, absolute path, bbox, area, group) and a plain SVG with one `<path id="pN">` per
piece. In a video: `PieceTree::new(&doc, "name")`, then `Svgr::from(tree.all())` or
`tree.group("french_banner")` inside `svgr!` (see `examples/golden_spurs/src/film.rs`).
Presets: `VectorizeParams::outlined(scale)` for pictures with drawn outlines.

## Fast animation in fframes (`scripts/window_tree.rs`, superseded by `PieceTree`)

Never pass the traced SVG to `Svgr::from_svg_string` per frame. Parse it once into a static
tree: `d` into `StaticPathData`, transforms and colours pre-parsed, a `static_hash` on every
node, the seam strokes dropped (usvgr strokes every stroked path each frame to measure its
bounds; that was the main cost). Embed it as `<g transform={camera}>{window_tree::window()}</g>`
inside `svgr!`; the Skia renderer records the window once and replays it, sharp at any zoom.
Profile with `sample <pid> 8` before optimising anything else.

## For motion later

Every piece is a closed path grouped by colour, so pieces can be lit, faded or moved. If pieces
need to be addressed individually, emit the SVG straight from `flatten` (one path per piece with
an id) instead of the per-colour potrace layers.

## Known limits

Soft painted shading (cheeks, folds) becomes two or three flat tones; 3x upscale or a separate
higher-resolution trace of face regions would go further. Dark grass blades and dense mail
hatching come through as small marks; raise `min_piece` to calm them.
