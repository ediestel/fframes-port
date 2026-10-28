# picture-to-svg: everything in one place

Self-contained record of the PNG-to-SVG workflow developed on 2026-10-26/28 (Battle of the Golden
Spurs window, `~/Desktop/Coding_Projects/fframes-main/examples/golden_spurs/`). The project keeps
working copies; this folder is the reference. Scripts here were checked identical to the
project's on 2026-10-28.

## Contents

| Path | What |
|---|---|
| `SKILL.md` | The workflow, rules, settings; loaded as the skill |
| `scripts/chain.sh`, `scripts/chain_query.md` | Step 0: query → NTGT engine (mode_s) → image prompt → Grok Imagine |
| `scripts/edit_instruction.txt` | Step 1: "keep everything, change only the rendering" instruction for xAI images/edits |
| `scripts/run.sh` | Steps 1-4 end to end (edit, 2x upscale, flatten, trace) |
| `scripts/fix_region.sh` | Step 1b: repair one defect by local edit, paste back a feathered ellipse only |
| `scripts/flatten/` | Rust tool: outlines, then colour-grown pieces (build: `cargo build --release --offline`) |
| `scripts/trace.sh` | ImageMagick + potrace, one layer per colour, lead last (`FLAT=1` for flat sources) |
| `scripts/traced.rs` | fframes module that renders the SVG as frame 0 (optional step 5) |
| `scripts/window_tree.rs` | The traced SVG as a cached static tree for animation (parse once, pre-parsed paths, static hashes, seam strokes dropped) |
| `scripts/film.rs` | The narrated camera film: schedule from the voice durations, eased camera per sentence |
| `example/0_generation/` | The original picture and the exact query, engine reply, image prompt and request that made it |
| `example/1_edit/` | Edit instruction, request (image field elided), the flat source, `fixes/` (the three defect repairs: crop, edit, mask, patch) and the repaired `edit_fixed.png` |
| `example/2_result/` | Final 3x piece map `flat3x.png`, final `traced.svg`, fframes render |

## Exact commands of the final result

Final (2026-10-28, 3x of the repaired source; 10.5 min, 5.6 MB, 7,201 paths):

```bash
magick edit_fixed.png -filter Mitchell -resize 300% edit3x.png
flatten edit3x.png flat3x.png 0.42 150 12 180 24 32
FLAT=1 MEDIAN=0 CC_AREA=0 zsh trace.sh flat3x.png traced.svg 96 16 36 23% 66% 24% 7 40 72
```

Earlier 2x settings (good enough, faster): resize 200%, `flatten ... 0.42 48 8 80 24 32`,
`trace.sh ... 96 16 16 23% 66% 24% 7 40 32`.

## User decisions

- Trace, never redraw; keep the composition via image edit, never re-prompt.
- Some artistic freedom is fine: flat stained-glass look and modest posterization accepted.
- The 2x result is "definitely good enough"; 3x or a separate face trace is an option for later.
- fframes step kept as an option in the skill.

## The crate (2026-10-28, branch `vector`)

`fframes-vector/` in this workspace: `flatten.rs`, `quantize.rs` (median cut + k-means in
Lab, no dither), `trace.rs` (potrace per colour, parallel, pieces with ids), `document.rs`
(`PieceDocument` JSON + SVG export), `tree.rs` (`PieceTree`, cached, per group), `main.rs`
(`fframes-vector vectorize|svg`). `fframes::vector` with feature `vector`. The film
(`examples/golden_spurs/src/film.rs`) renders from `vector/window3x.json`. Done from the list
below: 1 (ids per piece) and 3 (cached tree with `group()`).

## Not yet built (planned, for a narrated show with motion)

1. DONE: one path per piece with a stable id (`PieceDocument`).
2. DONE (2026-10-28 evening): group map `group.rs` (zones SVG, first-match in file order, centre
   + most of the bbox inside; `--check` colours pieces by group) with
   `examples/golden_spurs/vector/zones.svg` (25 zones, 6,499 of 6,765 pieces grouped, the
   rest is masonry); sprite atlases `atlas.rs` (`sprites` command, TexturePacker JSON, 1x+2x
   of the window in 1.2 s); `svg --grouped`.
   Generic grouping (2026-10-28 night): `raster.rs` (pieces as a label image), hints from any
   zone SVG shape (paths, ellipses, transforms, `<g>` nesting = `rider/spurs`) or a colour
   mask + legend, pixel-majority vote per piece or per automatic cluster (`cluster.rs`,
   Felzenszwalb on the piece graph across the lead), `clusters` + `rename` to name clusters,
   `tree.others(name)`, `doc.group_bbox(name)`. Window: 0.3 s, 6,690 of 6,765 pieces as
   before. Not yet: hints proposed by a vision model (names from the narration).
2. Group map: an outline per object (knights, horses, banners, rose, brook, militiamen, spurs,
   inscriptions); each piece joins the group covering most of it; SVG grouped by object.
3. DONE 2026-10-28 for camera motion (`scripts/window_tree.rs`): the static window as a cached
   tree, only the camera changes per frame. Full 1:54 film 315 s → 78 s. The profile showed
   usvgr re-stroking every path's 0.1 px seam stroke each frame; parsing was only a quarter
   of the cost. Still to do for per-group motion:
   (`Svgr::from_svg_string` of the whole file reparses every frame).
4. Motion that fits tiled glass: light, spotlight on the narrated group, camera zoom/pan, small
   rigid moves (banners, horse head, water shimmer). Larger moves need an inpainted background
   plate (xAI edit without the figure) or a rig (`examples/GP_figures/src/puppet.rs`).
5. Narration script and beat list (group + motion per beat) come first.

## Not yet verified

- `scripts/run.sh` has passed a syntax check only; never run end to end (about 5 min, 200 MB).
