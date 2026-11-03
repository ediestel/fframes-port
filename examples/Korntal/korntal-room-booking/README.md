# How can an LLM help with room management and booking?

A 1920x1080 @ 30 fps video made with [fframes](https://github.com/dmtrKovalenko/fframes),
rendered with the Skia (Metal) backend.

| file | what |
| --- | --- |
| `src/lib.rs` | the video: scenes, animation, audio map |
| `src/main.rs` | the command line (`fframes::cli`) |
| `media/` | fonts, images and audio embedded into the binary |

## Work on it

```sh
cargo run --release -p korntal-room-booking -- timeline                      # scenes, duration, audio tracks
cargo run --release -p korntal-room-booking -- frame 1s,50%,end              # PNGs into frames/ and the problems found in them
cargo run --release -p korntal-room-booking -- strip DataScene -n 8        # contact sheet, writes strip.png
cargo run --release -p korntal-room-booking -- onion "ProductScene@0..ProductScene@1.5s"  # motion trail of an entrance, writes onion.png
cargo run --release -p korntal-room-booking -- inspect                       # missing media/fonts, clipped text, panics in any frame
cargo run --release -p korntal-room-booking -- render DataScene --draft    # a part of the video, half resolution, fast preset
cargo run --release -p korntal-room-booking -- audio analyze --waveform w.png  # loudness (LUFS), peaks, silence per scene
cargo run --release -p korntal-room-booking -- render                        # the final video, writes out.mp4
cargo run --release -p korntal-room-booking -- preview                       # real-time GPU window with sound (space, h/l, j/k, q)
```

Times accept frames (`120`), seconds (`3.2s`), `m:ss`, percentages (`50%`), scenes (`DataScene`,
`#1`) and offsets inside scenes (`DataScene@1.5s`, `@50%`, `@end`). Add `--json` to any
command for machine-readable output.
