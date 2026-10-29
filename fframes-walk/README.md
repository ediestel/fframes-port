# fframes-walk

Walk through a room with your models in it: a stained-glass window in a church, a mosaic on
a wall, a statue, a drawing. The room is built from sizes in metres, the models are any glTF
files, and everything is rendered by fframes' own 3D renderer (`fframes-three-d`) on the GPU,
in the native player's window. The same scene can go into a film.

## Start

```
fframes-walk --window window.glb [--model statue.glb:left:2:0:3] [--room 14,28,10]
```

| Option | Meaning |
| --- | --- |
| `--window file.glb[:wall[:height[:sill[:offset]]]]` | A window: an opening is cut into the wall, the glass shows the daylight coming through it in its own colours, lead and dark paint stay dark, and the sun shines in. Default: back wall, 6 m high, lower edge 1.5 m above the floor, in the middle. |
| `--model file.glb[:wall[:height[:sill[:offset]]]]` | Any other model, standing just in front of the wall. Default: back wall, 2 m high, on the floor. |
| wall | `back` (ahead at the start), `front`, `left` or `right` |
| offset | Metres along the wall from its middle, to the right seen from inside |
| `--room W,D,H` | Room width, depth and height in metres. Default: high enough for the tallest model plus 2.5 m, twice as deep as wide. |
| `--glow N` | Brightness of the daylight behind the windows. Default 1.6. |
| `--at x,z` | Where to start, metres from the middle of the floor (z towards the back wall is negative). Default: near the front wall, facing the back wall. |
| `--quality Q` | Render resolution as a share of the window, 0.1 to 1. Default 0.5; higher is sharper and slower. |
| `--snapshot out.png` | Render one 1280 x 800 picture from the start and quit. |
| `--video out.mp4` | Render a walk from the start up to the first window, ending with a look up at it, and encode it with `ffmpeg` (H.264). With `--seconds S` (default 15) and `--size WxH` (default 1920x1080). |
| `--demo` | With `--video`: a scripted walk instead (walk in, step left, look around, step right, run, step back, approach looking up) with the controls drawn top right, W A S D, Shift and the mouse lit while held, at the speeds of the window. |
| `--captions file.txt` | Text over the video: one caption per line, `start-end|line|line` in seconds; lines after the first are smaller. Lines starting with `#` are comments. |

## A video to send

```
fframes-walk --window window.glb --video walk.mp4 --captions captions.txt
```

A controls demo, 18 s, 540 frames, rendered in 14 s, 8.8 MB:

```
fframes-walk --window window.glb --video walk_demo.mp4 --seconds 18 --demo --captions captions.txt
```

The Golden Spurs example (`examples/golden_spurs/walk_captions.txt`, `walk_demo_captions.txt`): 15 s at 1920 x 1080,
450 frames, rendered in 12 to 14 s on an Apple M1 GPU, 12.5 MB. An MP4 plays on any device,
unlike the program itself, which needs an Apple Silicon Mac with Homebrew FFmpeg.

Several `--window` and `--model` options can be given; each is placed on its wall.

## Moving

| Input | Action |
| --- | --- |
| `W` `A` `S` `D` | Walk forward, left, back, right |
| `↑` `↓` | Walk forward, back |
| `←` `→` | Turn |
| `Shift` | Walk faster |
| Drag with the mouse, or scroll with two fingers | Look around |
| `R` | Back to the start |
| `Q` or `Esc` | Quit |

The walls stop you; you walk at eye height (1.7 m). The status bar shows the frame rate and
where you are.

## A stained-glass window from fframes-vector

```
fframes-vector gltf pieces.json window.glb --tolerance 1
fframes-walk --window window.glb
```

Flat pieces (no `--depth`) are lightest; `--tolerance 1` keeps curves smooth to a pixel and
the file small (the Golden Spurs window: 12.8 MB). Each object of the piece document is its
own node, so a scene for a film can move them.

## The room

A polished stone floor that mirrors the window, stone walls, a ceiling, a dim interior light
and the sun coming in through the first window. Glass is unlit: it shows the colour of the
glass times the daylight behind it, as stained glass does from inside, so it is the brightest
thing in the room. Measured on the Golden Spurs window: the glass keeps the colourfulness of
the source picture (24 %) at about twice the brightness of the walls.

## Limits of this version

- The room is a box: no aisles, columns, vaults or arches yet.
- The sun lights the floor in white; coloured light from the glass on the floor is planned.
- Models stand on walls; free placement in the room is planned.
