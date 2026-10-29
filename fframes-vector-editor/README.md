# fframes-vector editor

The window for naming the objects of a picture. You draw a rough zone around each object
and give it a name; the editor assigns the pieces to the zones, shows the result at once and
points out the pieces that need a look. Everything runs on this machine.

It works on any piece document made by `fframes-vector vectorize`: stained glass, a scanned
drawing, a comic page, a map.

## Start

```
fframes-vector-editor pieces.json [zones.svg] [options]
```

| Option | Meaning |
| --- | --- |
| `zones.svg` | Zones to start from. Without it you start with none. |
| `-o out.json` | Where the grouped document is saved. Default: `<pieces>_grouped.json` next to `pieces.json`. |
| `--zones-out z.svg` | Where the zones are saved. Default: `<zones>.edited.svg` next to `zones.svg`, or `<pieces>_zones.svg` without one. The zone file you start from is never overwritten. |
| `--scale N` | Zone coordinates to document pixels. Default: the document's own scale, so zones are in pixels of the source picture. |
| `--cut` | Cut pieces along the zones when saving (also `c` in the window). |

Example, the Golden Spurs window:

```
target/release/fframes-vector-editor examples/golden_spurs/vector/window3x.json examples/golden_spurs/vector/zones.svg
```

## The window

- **The picture**, made of its pieces.
- **Zones**: cyan outlines, each with its name at the top left.
- **Group colours**: each piece tinted with the colour of its group.
- **Doubtful pieces**: red outline for a piece with no group, yellow for a piece split between
  names.
- **The piece under the pointer**: white outline; its number and group are in the status bar.
- **Status bar**, first line: the piece under the pointer, how many pieces are grouped, how
  many are unnamed and split, the number of zones, the look, and whether saving cuts.
- **Status bar**, second line: the last message, or the name you are typing.

Grouping and the doubtful pieces are worked out again after every change.

## Naming an object

1. Press `z`.
2. Click the corners of the zone around the object. A rough outline is enough: a piece goes
   to the zone that covers most of it.
3. `Backspace` removes the last corner, `Esc` stops.
4. Press `Enter` to close the zone.
5. Type the name and press `Enter`. The last name you used is already filled in.

Names can be nested with `/`: `rider/spurs` belongs to `rider` as well, so `rider` means the
whole rider and `rider/spurs` only the spurs.

## Naming pieces directly

For single pieces, or where a zone would be fiddly:

1. Click a piece: it is selected (filled orange). A click selects; a drag still moves the
   picture.
2. `Shift`+click adds more pieces, or removes one that is already selected.
3. Press `Enter`, type the name, press `Enter`.

The selection becomes a zone with exactly the outlines of those pieces, placed first so it
wins over rough zones. It is saved in the zone file and undone with `Cmd+Z` like any zone.
`Esc` clears the selection.

## Checking

- `n` jumps to the next doubtful piece, largest first, and the status bar shows how much of it
  lies in which zone. `N` goes back.
- `g` switches the look: the picture, the picture with group colours (mix), or group colours
  only.
- `d` hides or shows the doubtful outlines.
- A piece split between two zones usually needs either a zone border moved, or `--cut` /
  `c`, which divides the piece along the border when saving.

## Order of zones

Where zones overlap, the earlier zone in the list wins. A new zone goes to the front, because
a zone drawn later is usually the more specific one (the spurs inside the rider).

- `[` moves the zone under the pointer one place earlier, `]` one place later.
- `x` deletes the zone under the pointer.
- `Cmd+Z` (or `u`) undoes the last change to the zones, as many steps back as you like;
  `Cmd+Shift+Z` (or `U`) redoes it. Steps are kept until you quit.

When saving with cut, the order also decides which part is drawn on top: the earlier zone.

## Saving

`s` writes two files:

| File | Contents |
| --- | --- |
| The zones (`--zones-out`) | A zone SVG, one shape per zone in order. `fframes-vector group` reads it, and any SVG editor opens it. |
| The grouped document (`-o`) | The pieces with their groups, for fframes (`PieceTree`), sprites and glTF. With cut on, pieces crossing a zone border are divided along it. |

The status bar and the terminal say where the files went.

## Keys

| Key | Action |
| --- | --- |
| click | Select the piece under the pointer |
| `Shift`+click | Add the piece to the selection, or remove it |
| `Enter` (with a selection) | Name the selected pieces |
| `z` | New zone |
| click | Add a corner (while drawing a zone) |
| `Enter` | Close the zone, then confirm its name |
| `Backspace` | Remove the last corner, or the last letter of the name |
| `Esc` | Stop drawing or naming, else clear the selection; with nothing to stop or clear, quit |
| `Cmd+Z` or `u` | Undo the last change to the zones (new zone, delete, order); while drawing, the last corner |
| `Cmd+Shift+Z` or `U` | Redo |
| `x` | Delete the zone under the pointer |
| `[` / `]` | Zone under the pointer earlier / later |
| `n` / `N` | Next / previous doubtful piece |
| `g` | Look: picture, mix, groups |
| `d` | Doubtful outlines on / off |
| `f` | Fit the picture into the window |
| `c` | Cut along the zones when saving, on / off |
| `s` | Save |
| `q` | Quit |
| `?` | List the keys in the terminal |

## Mouse and trackpad

| Input | Action |
| --- | --- |
| Click | Select a piece (see above) |
| Drag (any button, when not drawing) | Move the picture |
| Two-finger scroll | Move the picture |
| Pinch, or Cmd / Ctrl + scroll | Zoom at the pointer |
| Mouse wheel | Zoom at the pointer |

## Limits of this version

- `q` and `Esc` quit without asking: save with `s` first.
- A zone cannot be reshaped or renamed in the window: delete it and draw it again, or edit the
  saved zone SVG in any SVG editor and open it again.
- Zones are polygons. Rectangles, circles and paths from a zone SVG are read, shown and used,
  and are saved as paths.
- Masks (`fframes-vector group --mask`) are not shown; only zones.

## How it is built

The window and its drawing are the `Surface` of `fframes-native-player` (Metal on macOS,
Vulkan elsewhere, the CPU as fallback). The pieces are drawn as Skia paths, the grouping,
report and cut are those of `fframes-vector` (`group`, `report`, `cut`), so the editor shows
exactly what the command line produces.
