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

`group::GroupMap` assigns pieces to named objects from a hand-drawn zone file: an SVG of
`<rect>`, `<circle>` and `<polygon>` elements with `id="name"` in source-picture
coordinates. A piece joins the first zone in the file that contains its centre and most of
its bounding box, so the file lists specific objects first (spurs before the meadow they lie
in) and backgrounds last; shapes may share a name. `--check` writes an SVG with every piece
in its group's colour, the picture to look at before trusting the map.

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
fframes-vector group pieces.json zones.svg [--scale N] [--check check.svg] [-o out.json]
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
let pieces = Svgr::from(tree().all());        // or tree().group("banner"), tree().rest()
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
