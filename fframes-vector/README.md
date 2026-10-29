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

`--cut` (zone SVG only) splits pieces that cross a zone border before the vote, for parts
that move on their own: an outline running from arm to body, a sleeve across the elbow
(`cut::cut_at_zones`). Each part takes its zone's name and keeps it after the vote. A part
under `--cut-min` (0.05) of its piece joins the piece's largest part, so a border that only
grazes a piece leaves it whole, and pieces off the borders keep their path data. Parts are
drawn bottom to top: the rest outside every zone, then the zones from last to first, so the
earlier zone, which wins where zones overlap, is on top. A lower part runs on 3 px under the
part above it: no hairline where they meet, and the overlap a joint needs when the upper part
turns. The cut itself is exact (boolean operations on an integer grid, `i_overlay`); curves of
cut pieces become segments within 0.05 px. On the Golden Spurs window 557 pieces are cut into
1,158 parts in 5 s, and the rendered picture matches the uncut one except for 396 edge pixels.

After the vote `group` lists the doubtful pieces (`report::report`), largest visible area
first: **unnamed** pieces, and **split** pieces, where a second name or no name covers at
least `--split` (0.3) of the piece, so its name may be wrong. `--report r.json` writes them
with id, box and the share of every name; the check picture outlines them, red for unnamed,
yellow for split. Only these need a look. On the window with its 25 rough zones: 263 unnamed
(the frame and the lead lines outside the zones) and 179 split; with `--cut` none split.

`fframes-vector-editor pieces.json [zones.svg]` (crate `fframes-vector-editor`) is the
window for drawing and naming zones: the pieces drawn with Skia through the native player's
`Surface`, zones over them, grouping and the report redone after every change, doubtful
pieces outlined, `n` jumps to the next one. `s` writes the zone SVG and the grouped
document (`c`: cut along the zones when saving). Keys are listed in its `main.rs`.

`figure sheet.png out.glb --height 1.7` makes a 3D figure from a sheet of three views (front,
side, back) on a plain background, as an image model draws them (`figure.rs`): the
background colour comes from the corners, the views are found as column runs (front and back
are the pair of equal width, the side the odd one, or `--order`), the volume is carved from
the front and side outlines in a voxel grid, its surface (surface nets) is textured with the
views it faces, and the sheet is embedded as the texture. No tracing. Hollows no view shows
stay filled. Golden Spurs: farmer 1.7 m and horseman 2.7 m from 2816 x 1584 sheets in
0.4 and 0.6 s, 23 and 34 MB.

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

## 3D (glTF)

`gltf::to_glb` writes the pieces as a binary glTF 2.0 for Blender, three.js, game engines
and `fframes::three_d`. Outlines are flattened within `--tolerance` document pixels, rings
sorted into contours and holes by nesting, and triangulated by ear cutting (`earcutr`).
`--depth N` turns the pieces into tiles N pixels thick (front, back, side walls). One
material per palette colour. Nodes follow the object names (`battle/french/knights` is
`knights` under `french` under `battle`, ungrouped pieces in `_rest`), each with one mesh
per object; `--per-piece` gives every piece its own node (`p<id>`, `extras.id`,
`extras.color`). Y is up, centred, the picture 2 m high unless `--unit` says otherwise.

The Golden Spurs window as 24 px tiles is 66 MB and exports in 0.3 s;
`examples/golden_spurs` (`golden-spurs-3d`) loads it with `Model::from_gltf`, orbits it and
slides the objects out one by one.

## Drawings

Scans and photos of drawings (`--preset drawing`, `VectorizeParams::drawing`):

- `drawing::clean_scan` levels the paper: a per-channel paper estimate (75th percentile per
  block, then a closing wider than any line) divides the picture, so shaded, yellowed or
  tinted paper becomes even white and lines and colours keep their contrast.
- `drawing::bridge_gaps` closes gaps in drawn outlines: every line end (an end of the line
  skeleton) is joined, with a bridge as wide as the line, to the nearest line it points at
  (within 50°) up to `--gap` source pixels away (10 in the preset). Each area between lines
  then becomes its own piece; wider openings stay open.

Tested on a synthetic scan (shaded yellow paper, noise, ink lines with gaps of 4 to 9 px and
one of 20 px): the paper came out white, every small gap closed, and the 20 px opening
stayed open. `--gap N` and `--clean` also work with the default preset.

## Command line

```bash
cargo build --release -p fframes_vector
fframes-vector vectorize picture.png pieces.json [--scale 2] [--colors 96] [--svg pieces.svg] [--flat flat.png] [--preset drawing] [--gap N] [--clean]
fframes-vector group pieces.json zones.svg [--cut [--cut-min 0.05]] [--mask m.png [--legend l.json]] [--clusters] [--split 0.3] [--report r.json] [--share 0.5] [--raster 4096] [--scale N] [--check check.svg] [-o out.json]
fframes-vector clusters pieces.json [--k 3000] [--gap 6] [--min-area 400] [--check check.svg] [-o out.json]
fframes-vector rename pieces.json names.json [--check check.svg] [-o out.json]
fframes-vector sprites pieces.json out_dir [--density 1,2,3] [--group name] [--max 4096] [--pad 2] [--no-outline]
fframes-vector svg pieces.json pieces.svg [--grouped]
fframes-vector gltf pieces.json pieces.glb [--depth N] [--per-piece] [--no-outline] [--group name] [--unit M] [--tolerance 0.5]
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
