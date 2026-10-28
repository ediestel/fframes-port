#!/usr/bin/env python3
"""Zone proposals from a vision model, for `fframes-vector group`.

    propose_zones.py picture.png objects.txt zones.svg [--model grok-4.7] [--side 1280]
                     [--grid 100] [--keep-image grid.png]

`objects.txt` names the objects, one per line, `name: what it looks like` (the description
is optional; names may be nested, `battle/french/knights`). The picture is sent with a
labelled coordinate grid; the model answers with polygons per object (an object may appear
in several places). In tests only grok-4.7 (reasoning, about 5 minutes) answered in the
image's pixels; grok-4.20-non-reasoning used a scale of its own. On the Golden Spurs window
grok-4.7 polygons snapped with `--clusters` named 78 % of the glass area like hand-drawn
zones, ahead of the image-edit masks of `propose_mask.py`. The polygons are written in the
picture's own pixel coordinates as
`<polygon data-group="name">`, smallest objects first, so specific objects win where zones
overlap. Then:

    fframes-vector group pieces.json zones.svg --clusters --check check.svg

snaps the rough polygons to the pieces. Look at the check picture before trusting it.

Needs `magick` (ImageMagick 7) and `XAI_API_KEY`. Only the standard library otherwise.
"""

import argparse
import base64
import json
import os
import subprocess
import sys
import tempfile
import urllib.request

API = "https://api.x.ai/v1/chat/completions"


def image_size(path):
    out = subprocess.run(["magick", "identify", "-format", "%w %h", path], capture_output=True, text=True, check=True)
    w, h = out.stdout.split()
    return int(w), int(h)


def grid_image(src, dst, side, step):
    """The picture scaled to `side` px on its longest edge, with grid lines every `step`
    px of the scaled image, labelled with their coordinate."""
    w, h = image_size(src)
    k = side / max(w, h)
    sw, sh = round(w * k), round(h * k)
    draw = []
    for x in range(step, sw, step):
        draw += ["-draw", f"line {x},0 {x},{sh}"]
    for y in range(step, sh, step):
        draw += ["-draw", f"line 0,{y} {sw},{y}"]
    labels = []
    for x in range(step, sw, step):
        labels += ["-annotate", f"+{x + 2}+14", str(x)]
    for y in range(step, sh, step):
        labels += ["-annotate", f"+2+{y - 3}", str(y)]
    subprocess.run(
        ["magick", src, "-resize", f"{sw}x{sh}!", "-stroke", "#ff00ff", "-strokewidth", "1", "-fill", "none", *draw,
         "-stroke", "none", "-fill", "#ff00ff", "-pointsize", "13", *labels, dst],
        check=True,
    )
    return k, sw, sh


def read_objects(path):
    objects = []
    for line in open(path, encoding="utf-8"):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        name, _, desc = line.partition(":")
        objects.append((name.strip(), desc.strip()))
    return objects


def ask(model, png, sw, sh, step, objects, key):
    listing = "\n".join(f"- {n}: {d}" if d else f"- {n}" for n, d in objects)
    prompt = (
        f"The image is {sw} x {sh} pixels. The magenta grid lines are every {step} pixels, labelled with "
        f"their coordinate. For each object below, give one or more polygons (x, y in this image's pixels) "
        f"that outline it closely; one polygon per separate place the object appears. Leave out objects "
        f"you cannot find.\n\n{listing}\n\n"
        'Answer with JSON only: {"objects": [{"name": "...", "polygons": [[[x, y], ...], ...]}]}'
    )
    data = base64.b64encode(open(png, "rb").read()).decode()
    body = {
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": [
                    {"type": "image_url", "image_url": {"url": f"data:image/png;base64,{data}", "detail": "high"}},
                    {"type": "text", "text": prompt},
                ],
            }
        ],
        "response_format": {"type": "json_object"},
        "temperature": 0,
    }
    req = urllib.request.Request(
        API,
        data=json.dumps(body).encode(),
        headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=600) as r:
        reply = json.load(r)
    text = reply["choices"][0]["message"]["content"]
    return json.loads(text[text.find("{") : text.rfind("}") + 1])


def polygons(value):
    """Polygons as lists of (x, y), from what models actually return: a list of polygons,
    one bare polygon, or flat [x, y, x, y, ...] lists."""

    def points(p):
        if p and all(isinstance(v, (int, float)) for v in p):
            return list(zip(p[0::2], p[1::2]))
        return [(v[0], v[1]) for v in p if isinstance(v, (list, tuple)) and len(v) >= 2]

    if not isinstance(value, list) or not value:
        return []
    first = value[0]
    if isinstance(first, (int, float)) or (
        isinstance(first, (list, tuple)) and len(first) == 2 and isinstance(first[0], (int, float))
    ):
        value = [value]
    out = []
    for p in value:
        try:
            pts = [(float(x), float(y)) for x, y in points(p)]
        except (TypeError, ValueError):
            continue
        if len(pts) >= 3:
            out.append(pts)
    return out


def area(poly):
    return abs(sum(x0 * y1 - x1 * y0 for (x0, y0), (x1, y1) in zip(poly, poly[1:] + poly[:1]))) / 2


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("picture")
    ap.add_argument("objects")
    ap.add_argument("zones")
    ap.add_argument("--model", default="grok-4.7")
    ap.add_argument("--side", type=int, default=1280, help="longest edge of the image sent")
    ap.add_argument("--grid", type=int, default=100, help="grid step in pixels of the image sent")
    ap.add_argument("--keep-image", help="also save the gridded image here")
    a = ap.parse_args()
    key = os.environ.get("XAI_API_KEY")
    if not key:
        sys.exit("XAI_API_KEY is not set")
    objects = read_objects(a.objects)
    names = {n for n, _ in objects}
    w, h = image_size(a.picture)
    with tempfile.TemporaryDirectory() as tmp:
        png = a.keep_image or os.path.join(tmp, "grid.png")
        k, sw, sh = grid_image(a.picture, png, a.side, a.grid)
        answer = ask(a.model, png, sw, sh, a.grid, objects, key)

    shapes = []
    for o in answer.get("objects", []):
        name = str(o.get("name", "")).strip()
        if name not in names:
            print(f"ignored unknown object {name!r}", file=sys.stderr)
            continue
        for poly in polygons(o.get("polygons", [])):
            pts = [(x / k, y / k) for x, y in poly]
            if len(pts) >= 3:
                shapes.append((area(pts), name, pts))
    shapes.sort(key=lambda s: s[0])
    found = {s[1] for s in shapes}
    if shapes:
        x1 = max(x for _, _, p in shapes for x, _ in p)
        y1 = max(y for _, _, p in shapes for _, y in p)
        if x1 < 0.3 * w and y1 < 0.3 * h:
            print(f"warning: all polygons lie within {x1:.0f} x {y1:.0f} of a {w} x {h} picture; "
                  "the model answered in a scale of its own (try grok-4.7)", file=sys.stderr)
    with open(a.zones, "w", encoding="utf-8") as f:
        f.write(f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">\n')
        f.write(f"<!-- proposed by {a.model} for {os.path.basename(a.picture)}; smallest first -->\n")
        f.write(f'<image href="{os.path.abspath(a.picture)}" width="{w}" height="{h}" opacity="0.5"/>\n')
        for _, name, pts in shapes:
            p = " ".join(f"{x:.0f},{y:.0f}" for x, y in pts)
            f.write(f'<polygon data-group="{name}" points="{p}" fill="none" stroke="#ff00ff" stroke-width="3"/>\n')
        f.write("</svg>\n")
    print(f"{len(shapes)} polygons for {len(found)} of {len(objects)} objects -> {a.zones}", file=sys.stderr)
    missing = [n for n, _ in objects if n not in found]
    if missing:
        print("not found: " + ", ".join(missing), file=sys.stderr)


if __name__ == "__main__":
    main()
