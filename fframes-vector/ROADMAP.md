# fframes-vector: future implementation plan

2026-10-28

fframes-vector turns one picture into a piece document that already feeds video, games and
3D. The next work adds industries, not features. Everything runs locally.

## Where it stands

State on `main` of `ediestel/fframes-port` (2026-10-29):

| Capability | What it does today | Measured on the Golden Spurs window |
| --- | --- | --- |
| Vectorize | Picture to pieces via potrace, presets for outlined art and drawings | 17 s at 3x, 6,765 pieces |
| Drawing input | Paper levelled to white, gaps in lines bridged | Dix-Hallpike plate: 198 pieces in 2.9 s |
| Objects | Hand zones or colour mask, nested names, cluster snapping, rename | 0.3 s per run |
| Cut | Pieces split exactly along zone borders, lower part runs on under the upper one | 557 pieces into 1,158 parts in 5 s; plate: line work cut at head and feet |
| Report | Unnamed pieces and pieces split between names, largest first | 263 unnamed, 179 split with rough zones; none split after cut |
| Editor | Zones and click selection, live grouping, report and cut, undo, save | Window and plate named by hand |
| Animation | Cached trees per object and for the rest, object framing | Film render 315 s to 58 s |
| Sprites | Power-of-two atlases, TexturePacker JSON | 6,097 sprites in 1.2 s |
| 3D | glTF, flat or tiles, one node per object or per piece | 66 MB as 24 px tiles, 0.3 s |

One real picture has been tested end to end. Object naming is manual by design: online
models were tried and dropped.

## Priority 1: measurements and CAD

Real units turn a scanned plan or technical drawing into quantities and fabrication files.
This opens architecture, engineering and making.

1. **Scale calibration.** The user marks two points and their real distance, and the
   document stores millimetres per pixel. A second mode reads a printed scale bar.
2. **Quantities.** Area, perimeter and length per piece and per object, in real units: room
   areas, wall lengths, glass area per colour.
3. **Line semantics.** Outline pieces become centrelines with a thickness (medial axis of the
   line mask), so a wall is a line of known width.
4. **CAD export.** DXF with one layer per object and per colour, closed polylines for areas,
   lines for centrelines. SVG in millimetres for laser cutters, plotters and CNC.
5. **Walls in 3D.** Centrelines extruded to a wall height with their thickness. Rooms become
   floor slabs, doors and windows named gaps.
6. **IFC-lite.** Walls, slabs and spaces with names and areas, enough for BIM tools to open a
   scanned plan as a model.

Done when: a scanned floor plan with one known dimension exports a DXF whose room areas match
the plan within 2 %, plus a 3D model with walls.

## Picture exchange (idea)

Select pieces in the editor, give an instruction, and let an image model (xAI Imagine)
redraw exactly that region of the source picture; the result is traced again locally and
its pieces replace the selected ones inside the selection, keeping the name. First use: the
Golden Spurs window shows French lilies on the Flemish side, where the Flemish banner (black
lion on gold) belongs. The parts exist: `fix_region.sh` (crop, edit, paste back) in the
picture-to-svg skill, `cut` for the exact replacement. Not built; the picture leaves the
machine only for this step and only on request.

## Priority 2: rigging (planned, not started)

Rigging turns named parts into a moving character. A scanned drawing becomes a game or film
character in minutes.

1. **Joints from contact.** Where two named parts touch (from the piece adjacency the
   clusters already compute), the middle of their shared border becomes a pivot: arm to body,
   head to neck.
2. **Skeleton from the object tree.** The nested names give the hierarchy
   (`body/arm/hand`). Bones run from pivot to pivot, with the root at the largest part.
3. **Pivot file.** Like the zone file, to move or add joints by hand where contact guesses
   wrong.
4. **Poses in fframes.** Each object subtree rotates about its pivot, with keyframe and easing
   helpers.
5. **Export.** Spine JSON and DragonBones for 2D cut-out, glTF skins for 3D, each with its
   atlas or meshes.

Done when: a scanned figure with five named parts waves its arm in an fframes clip and in
Spine, from one command plus a pivot file.

## Priority 3: mesh simplification and levels of detail

Light meshes make the 3D export usable in browsers, on phones, in AR and in game engines.
The window as tiles is 66 MB today. The target is under 5 MB.

1. **Shared vertices.** Front, back and walls reuse vertices; walls use one quad strip per ring.
2. **Outline simplification.** Douglas-Peucker on the rings with a tolerance in source pixels,
   keeping shared borders identical on both sides.
3. **Merged meshes.** One mesh per object and colour, and an option to merge whole objects.
4. **Levels of detail.** Two or three tolerances per object as glTF LOD nodes
   (`MSFT_lod`).
5. **Compression.** meshopt or Draco, and quantised positions.
6. **Glass look.** Transmission and emissive materials, so backlit tiles glow.

Done when: the window as tiles loads in a browser viewer under 5 MB and looks the same at
full-screen size.

## Shared-edge geometry

Neighbouring pieces should share one border instead of each tracing its own. That gives exact
jigsaw cuts, clean 3D seams, morphing between pictures and much smaller files.

1. **Planar map.** Trace the label image once as a graph of borders between regions, instead
   of one potrace pass per colour.
2. **Pieces from faces.** Every piece is a face of the graph and keeps its id, colour and group.
3. **Uses.** Puzzle cuts with tab shapes along borders, laser-cut templates without double
   lines, borders animated on their own (lead lines that draw themselves).

## Local recognition

Recognition models running offline remove the last manual steps. Nothing goes online.

1. **Object naming.** A local image-text model (a CLIP-type model) scores each cluster against
   the names in the object list; the user confirms.
2. **Text reading.** Local OCR for labels, dimensions and inscriptions; dimensions feed the
   scale calibration.
3. **Symbols.** A symbol library for electrical, P&ID and plan drawings; found symbols become
   named objects and their connections a graph (a netlist).

## More inputs

1. **Photos of real glass and mosaics.** Lead and grout detection from photos with uneven
   light and perspective correction.
2. **Comics.** Panels, speech balloons and characters as objects, for motion comics.
3. **Maps.** Regions and borders as objects, for animated historical maps.
4. **Video.** Rotoscoped frames with pieces kept consistent over time.

## More outputs

1. **Fabrication templates.** Numbered glass, mosaic or tile templates with a bill of
   materials (area per colour) and a cost.
2. **3D prints.** Reliefs, lithophanes and backlit panels as STL or 3MF.
3. **Interactive web.** A viewer with clickable objects, quizzes and guided tours, from the
   grouped SVG or the glTF.

## Motion and light

1. **Shatter.** Pieces fall and spin along their real outlines, with simple physics.
2. **Light through glass.** Light passes through the tiles in 3D and colours the floor.
3. **Per-piece motion.** Wave, shimmer, assemble and disassemble, driven per piece or object.
4. **Audio-reactive pieces.** Objects pulse with music or speech level.

## Workflow

1. **Editor.** Done: draw zones, click pieces to name them, live grouping, report and cut,
   undo (`fframes-vector-editor`, manual in its README). Still to come: set pivots and scale.
   Earlier plan: draw zones, click clusters to name them, set pivots and scale, with the check
   picture live.
2. **Batch runs.** A folder of pictures (a comic book, an archive, a set of windows) in one
   command with one settings file.
3. **Local service.** The same pipeline behind a local API for other tools.

## Sequencing

Each milestone has a test picture so results are measured, not assumed.

| Milestone | Builds on | Test picture |
| --- | --- | --- |
| M1 Drawing input on real scans | done code, tuning only | a real scanned drawing |
| M2 Mesh simplification | glTF export | the Golden Spurs window |
| M3 Measurements and CAD | drawing input | a scanned floor plan with one dimension |
| M4 Rigging | grouping, adjacency | a scanned figure drawing |
| M5 Shared-edge geometry | trace | the window and a jigsaw picture |
| M6 Local recognition | clusters | a comic page, a schematic |
| M7 Editor and batch | all of the above | a set of windows or a comic book (editor done 2026-10-29, batch open) |

Inputs, outputs and motion items attach to whichever milestone first needs them.
