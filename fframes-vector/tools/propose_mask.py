#!/usr/bin/env python3
"""An object mask by image edit (Grok Imagine), for `fframes-vector group --mask`.

    propose_mask.py picture.png objects.txt out_dir [--model grok-imagine-image-quality]
                    [--per-object [--marker #ff00ff --marker-name magenta]]

`objects.txt` names the objects, one per line, `name: what it looks like` (names may be
nested, `battle/french/knights`). Every object gets a distinct flat colour; the image model
repaints the picture with each object filled in its colour and everything else black. The
answer is snapped to that exact palette (`magick -remap`, no dither), so the colours are
clean. `--per-object` instead asks once per object (in parallel) to repaint only that
object in a marker colour, and composes the masks, smallest on top.

Measured on the Golden Spurs window against hand-drawn zones (share of the glass area named
the same, both snapped with `--clusters`): one coloured edit 65 % (shapes follow the
picture well, but colours get bound to the wrong objects and grey details snap to grey
objects), per object 57 % (some objects repainted wrongly or in another tint), polygons from
`propose_zones.py` with grok-4.7 78 %. Look at `raw.png` before trusting a mask.

Writes into `out_dir`:

    raw.png      the model's picture
    mask.png     the snapped mask
    legend.json  {"name": "#rrggbb", ...}
    request.json the request without the image

Then:

    fframes-vector group pieces.json --mask out_dir/mask.png --legend out_dir/legend.json \
        --clusters --check check.svg

Needs `magick` (ImageMagick 7) and `XAI_API_KEY`. Only the standard library otherwise.
"""

import argparse
import base64
import json
import os
import subprocess
import sys
import urllib.request

API = "https://api.x.ai/v1/images/edits"

# Bright, well separated colours with plain names (image models follow names better than
# hex codes). Black is "no object".
COLORS = [
    ("red", "#ff0000"),
    ("green", "#00ff00"),
    ("blue", "#0000ff"),
    ("yellow", "#ffff00"),
    ("magenta", "#ff00ff"),
    ("cyan", "#00ffff"),
    ("white", "#ffffff"),
    ("orange", "#ff8000"),
    ("purple", "#8000ff"),
    ("pink", "#ff80c0"),
    ("light green", "#80ff80"),
    ("sky blue", "#80c0ff"),
    ("brown", "#8b4513"),
    ("grey", "#808080"),
    ("teal", "#008080"),
    ("olive", "#808000"),
    ("dark red", "#800000"),
    ("navy", "#000080"),
    ("lavender", "#c0a0ff"),
    ("beige", "#f0e0a0"),
]


def read_objects(path):
    objects = []
    for line in open(path, encoding="utf-8"):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        name, _, desc = line.partition(":")
        objects.append((name.strip(), desc.strip()))
    return objects


def edit(a, key, prompt, out_png, request_json):
    img = base64.b64encode(open(a.picture, "rb").read()).decode()
    body = {
        "model": a.model,
        "prompt": prompt,
        "image": {"url": "data:image/png;base64," + img},
        "n": 1,
        "response_format": "b64_json",
        "resolution": a.resolution,
    }
    with open(request_json, "w") as f:
        json.dump({**body, "image": "(picture)"}, f, indent=1)
    req = urllib.request.Request(
        API,
        data=json.dumps(body).encode(),
        headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=900) as r:
        reply = json.load(r)
    with open(out_png, "wb") as f:
        f.write(base64.b64decode(reply["data"][0]["b64_json"]))


def marked(path, marker, w, h, out):
    """White where `path` (stretched to w x h) is within 25 % of the marker colour."""
    subprocess.run(
        ["magick", path, "-resize", f"{w}x{h}!", "-fuzz", "25%", "-fill", "black", "+opaque", marker,
         "-fill", "white", "-opaque", marker, "-colorspace", "gray", "-type", "bilevel", out],
        check=True,
    )


def one_object(a, key, i, name, desc, w, h):
    """The picture with one object repainted in the marker colour; the mask is where the
    marker colour is (image editors recolour one object while keeping the rest well; asked
    for a black-and-white mask they redraw the whole picture in black and white).
    Returns (name, mask path, marked pixel count) or (name, None, 0) when the call fails."""
    import time

    what = desc or name.replace("/", " ").replace("_", " ")
    prompt = (
        f"Keep this picture exactly as it is and repaint only {what} in one flat, pure {a.marker_name} "
        f"({a.marker}), filled completely to its exact outline, every place it appears. Change nothing "
        "else: every other object, line and colour stays the same."
    )
    raw = os.path.join(a.out_dir, f"raw_{i:02d}.png")
    for attempt in range(4):
        try:
            edit(a, key, prompt, raw, os.path.join(a.out_dir, f"request_{i:02d}.json"))
            break
        except Exception as e:
            print(f"{name}: {e} (attempt {attempt + 1})", file=sys.stderr)
            time.sleep(5 * (attempt + 1))
    else:
        return name, None, 0
    mask = os.path.join(a.out_dir, f"mask_{i:02d}.png")
    marked(raw, a.marker, w, h, mask)
    mean = subprocess.run(["magick", mask, "-format", "%[fx:mean]", "info:"], capture_output=True, text=True, check=True)
    return name, mask, float(mean.stdout) * w * h


def per_object(a, key, objects, legend):
    """One call per object (in parallel), composed into one colour mask: larger objects
    first, smaller ones on top, so specific objects win where masks overlap."""
    from concurrent.futures import ThreadPoolExecutor

    out = subprocess.run(["magick", "identify", "-format", "%w %h", a.picture], capture_output=True, text=True, check=True)
    w, h = (int(v) for v in out.stdout.split())
    already = os.path.join(a.out_dir, "marker_in_picture.png")
    marked(a.picture, a.marker, w, h, already)
    share = subprocess.run(["magick", already, "-format", "%[fx:mean]", "info:"], capture_output=True, text=True, check=True)
    os.remove(already)
    if float(share.stdout) > 0.005:
        print(f"warning: {float(share.stdout):.1%} of the picture is already near {a.marker}; "
              "choose another --marker", file=sys.stderr)
    with ThreadPoolExecutor(max_workers=4) as pool:
        jobs = [pool.submit(one_object, a, key, i, n, d, w, h) for i, (n, d) in enumerate(objects)]
        results = [j.result() for j in jobs]
    mask = os.path.join(a.out_dir, "mask.png")
    subprocess.run(["magick", "-size", f"{w}x{h}", "xc:black", mask], check=True)
    for name, m, px in sorted((r for r in results if r[1]), key=lambda r: -r[2]):
        if px < 1:
            print(f"{name}: empty mask", file=sys.stderr)
            continue
        subprocess.run(
            ["magick", mask, "(", "-size", f"{w}x{h}", f"xc:{legend[name]}", ")", m, "-compose", "over", "-composite", mask],
            check=True,
        )
        print(f"{px / (w * h):6.1%}  {name}", file=sys.stderr)
    with open(os.path.join(a.out_dir, "legend.json"), "w") as f:
        json.dump(legend, f, indent=1)
    print(f"mask for {sum(1 for r in results if r[1])} of {len(objects)} objects -> {mask}", file=sys.stderr)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("picture")
    ap.add_argument("objects")
    ap.add_argument("out_dir")
    ap.add_argument("--model", default="grok-imagine-image-quality")
    ap.add_argument("--resolution", default="2k")
    ap.add_argument("--per-object", action="store_true", help="one edit per object, repainted in the marker colour")
    ap.add_argument("--marker", default="#ff00ff", help="colour the object is repainted in; absent from the picture")
    ap.add_argument("--marker-name", default="magenta")
    a = ap.parse_args()
    key = os.environ.get("XAI_API_KEY")
    if not key:
        sys.exit("XAI_API_KEY is not set")
    objects = read_objects(a.objects)
    if len(objects) > len(COLORS):
        sys.exit(f"at most {len(COLORS)} objects per mask; split the list")
    os.makedirs(a.out_dir, exist_ok=True)
    if a.per_object:
        per_object(a, key, objects, {n: c for (n, _), (_, c) in zip(objects, COLORS)})
        return

    legend = {}
    lines = []
    for (name, desc), (cname, hexc) in zip(objects, COLORS):
        legend[name] = hexc
        what = desc or name.replace("/", " ").replace("_", " ")
        lines.append(f"- {what}: {cname} ({hexc})")
    prompt = (
        "Turn this picture into a flat colour segmentation map of the same size and the same "
        "composition. Fill every object listed below completely with its one flat colour, following "
        "its exact outline in this picture. Everything that is not listed, including all outlines, "
        "lines and background, becomes pure black. No shading, no texture, no lines inside the "
        "colour areas, no text, no other colours.\n\n" + "\n".join(lines)
    )
    img = base64.b64encode(open(a.picture, "rb").read()).decode()
    body = {
        "model": a.model,
        "prompt": prompt,
        "image": {"url": "data:image/png;base64," + img},
        "n": 1,
        "response_format": "b64_json",
        "resolution": a.resolution,
    }
    with open(os.path.join(a.out_dir, "request.json"), "w") as f:
        json.dump({**body, "image": "(picture)"}, f, indent=1)
    req = urllib.request.Request(
        API,
        data=json.dumps(body).encode(),
        headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=900) as r:
        reply = json.load(r)
    raw = os.path.join(a.out_dir, "raw.png")
    with open(raw, "wb") as f:
        f.write(base64.b64decode(reply["data"][0]["b64_json"]))

    # snap to the palette (black + the legend colours), no dither
    palette = ["#000000"] + list(legend.values())
    pal_png = os.path.join(a.out_dir, "palette.png")
    subprocess.run(["magick", *[f"xc:{c}" for c in palette], "+append", pal_png], check=True)
    mask = os.path.join(a.out_dir, "mask.png")
    subprocess.run(["magick", raw, "+dither", "-remap", pal_png, mask], check=True)
    os.remove(pal_png)
    with open(os.path.join(a.out_dir, "legend.json"), "w") as f:
        json.dump(legend, f, indent=1)
    print(f"mask for {len(objects)} objects -> {mask} (legend.json beside it)", file=sys.stderr)


if __name__ == "__main__":
    main()
