# fframes-vector

Pictures and drawings to piece-based vector art, and that art as cached trees for
[fframes](https://fframes.studio) videos. Re-exported as `fframes::vector` with the `vector`
feature of fframes.

## What it does

A picture with drawn outlines (stained glass, comics, flat illustration, a cleaned-up
drawing) goes through four steps:

1. **Flatten** (`flatten`): outlines become pure black; every area between them becomes one
   even colour, grown from pixels of similar colour so a piece ends at an outline or at a
   colour change; thin brown strokes (faces, folds) keep their colour; specks merge into
   their neighbour.
2. **Quantise** (`quantize`): the colours reduce to a palette by median cut refined with
   k-means in CIE Lab, without dithering (dithering is what turns a trace into speckle).
3. **Trace** (`trace`): every palette colour becomes a bitmap and `potrace` turns it into
   closed curves; each closed curve with its holes is one piece. potrace is an external
   program (on the `PATH`, or named in `POTRACE`); it is not linked into the crate.
4. **Document** (`document::PieceDocument`): the palette and one `Piece` per shape with
   `id`, `color`, absolute path data `d` (`M L C Z`, pixels), `bbox`, `area` and a `group`
   name, saved as JSON and exported as plain SVG with one `<path id="pN">` per piece.

`tree::PieceTree` turns a document into a static `svgr` tree: every path pre-parsed, every
node with a `static_hash`, the form the `svgr!` macro builds at compile time. The renderers
record such a tree once and replay it under any transform, so a camera move over thousands
of pieces costs one transform per frame. The whole picture, one group, or everything outside
the groups are separate cached subtrees.

## Groups and sprites

Grouping names pieces as objects. The process is the same for every picture; only the
hints belong to one picture.

1. `raster::PieceRaster`: the pieces filled into a label image once (4096 px on the longest
   side by default), the common ground for everything below.
2. Hints become a `group::LabelLayer`, a name per pixel:
   - a zone SVG drawn in any editor over the source picture: `rect`, `circle`, `ellipse`,
     `polygon`, `polyline`, `path`, with `transform`s. The name is `data-group`, else
     `inkscape:label`, else `id`. Shapes in a named `<g>` are nested under it
     (`<g id="rider"><path id="spurs"/></g>` = `rider/spurs`); unnamed shapes in it take its
     name. Earlier shapes win where they overlap, so specific objects come first.
   - a colour mask (`--mask`, any size, from any segmentation tool or painted by hand), with
     a legend `{"horse": "#ff0000"}` or named by its colours. Transparent = no object.
   - both: the zones are drawn over the mask, as corrections.
3. Units: every piece alone, or with `--clusters` every automatic cluster as a whole, so a
   coarse hint snaps to the lead lines.
4. Vote: a unit takes the name covering most of its pixels when that is at least `--share`
   (0.5). Pieces too small to own a pixel take the name at their centre.

`cluster` is the hint-free part: pieces that face each other across at most `--gap` source
pixels of outline and look alike (CIELAB ΔE) merge, after Felzenszwalb and Huttenlocher, on
the piece graph. Clusters are not objects (brown and white glass of one horse stay two); they
are the units hints snap to, and an unnamed split to look at. `clusters` writes them as
`c0`, `c1`, ... (largest first), `rename` turns them into objects:
`{"horse": ["c12", "c40"], "battle/knights": ["knights"]}`.

Hints without drawing: `tools/propose_zones.py picture.png objects.txt zones.svg` asks a
vision model (xAI grok-4.7) for polygons per object from a text list (`name: what it looks
like`), on the picture with a labelled grid. On the Golden Spurs window, snapped with
`--clusters`, the polygons named 78 % of the glass area like the hand-drawn zones (masks
from the image editor reached 65 % at best and were dropped): good placement for large
objects, rough for small ones (lion, lances,
spurs). Treat them as a first draft to correct, and look at the check picture.

Names are paths: `tree.group("rider")` includes `rider/spurs`, `tree.others("rider")` is
everything else, `doc.group_bbox("rider")` frames it for a camera. `--check` writes an SVG
with every piece in its group's colour and the names written on: look at it before trusting
the map. On the Golden Spurs window (6,765 pieces, 25 zones) a run takes 0.3 s.

`atlas` rasterizes pieces (with the renderer fframes uses) at a density relative to the
source picture, packs them into power-of-two pages and writes PNG + JSON per page in the
TexturePacker "hash" layout that PixiJS, Phaser, Godot and Unity read. Each frame also
carries `group`, `color` and `source` (the piece's place in the source picture), so a game
can lay the pieces back into the picture. `--group` makes an atlas of one object,
`--no-outline` leaves the lead lines on the board.

## Command line

```bash
cargo build --release -p fframes_vector
fframes-vector vectorize picture.png pieces.json [--scale 2] [--colors 96] [--svg pieces.svg] [--flat flat.png]
fframes-vector group pieces.json zones.svg [--mask m.png [--legend l.json]] [--clusters] [--share 0.5] [--raster 4096] [--scale N] [--check check.svg] [-o out.json]
fframes-vector clusters pieces.json [--k 3000] [--gap 6] [--min-area 400] [--check check.svg] [-o out.json]
fframes-vector rename pieces.json names.json [--check check.svg] [-o out.json]
fframes-vector sprites pieces.json out_dir [--density 1,2,3] [--group name] [--max 4096] [--pad 2] [--no-outline]
fframes-vector svg pieces.json pieces.svg [--grouped]
```

`--scale` upscales before flattening (Catmull-Rom): 2 for most pictures, 3 when faces will be
seen close up. `--flat` writes the flattened picture, which is also the piece map. On a
1664 x 2496 picture: 11 s at 2x, 17 s at 3x (potrace runs per colour in parallel).

## In a video

```rust
use fframes::vector::{PieceDocument, PieceTree};
use std::sync::OnceLock;

static TREE: OnceLock<PieceTree> = OnceLock::new();

fn tree() -> &'static PieceTree {
    TREE.get_or_init(|| {
        let doc = PieceDocument::from_json(include_str!("../pieces.json")).expect("pieces.json");
        PieceTree::new(&doc, "my-picture")
    })
}

// in render_frame
let pieces = Svgr::from(tree().all());        // or tree().group("banner"), tree().others("banner"), tree().rest()
fframes::svgr!(<g transform={camera}>{pieces}</g>)
```

Never hand such art to `Svgr::from_svg_string` per frame: the document is parsed and every
path stroked again on every frame.

## Settings

`VectorizeParams::outlined(scale)` is the preset for pictures with drawn outlines. Its parts:

| Setting | Meaning | 1x default |
|---|---|---|
| `flatten.dark` | strongest channel below this (0..1) counts as dark | 0.42 |
| `flatten.line_w` | half width of a line; wider dark areas are not outlines | 4 |
| `flatten.speck` | dark blobs smaller than this are not outlines | 20 |
| `flatten.soft_t` | black-hat depth for soft (coloured) strokes | 32 |
| `flatten.tol` | RGB distance a pixel may differ from its piece's mean | 24 |
| `flatten.min_piece` | smaller pieces merge into the largest neighbour | 16 |
| `colors` | palette size including the outline colour | 96 |
| `trace.turd` | potrace speckle size for colour layers | 4 |
| `trace.turd_outline` | the same for the outline colour | 8 |

Areas scale with `scale²`, widths with `scale`. Lower `tol` keeps more shading, higher
`min_piece` calms grass and hatching.

## Where it came from

Developed on the Battle of the Golden Spurs stained-glass window (`examples/golden_spurs`):
an AI picture, redrawn flat by an image edit, repaired, vectorised at 3x, filmed with a
narrated camera. The lessons behind the settings (why no dithering, why pieces must end at
colour changes, why soft strokes matter) are in `.claude/skills/picture-to-svg/SKILL.md`.
