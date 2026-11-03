"""Side-view rigs for the Dix-Hallpike short, cut from traced silhouettes.

    python3 -I trace_side_rigs.py patient SIL.pgm LINES.pgm OUT.svg HAND.pgm
    python3 -I trace_side_rigs.py examiner SIL.pgm ARM.pgm OUT.svg

SIL is the figure's filled silhouette (white on black, plain PGM). For the
patient (couch-positions template, JVP figure, 2x grid) LINES holds the arm's
drawn outline; the forearm and hand below it stay with the trunk. For the
examiner (geriatric template, profile figure) ARM is the arm region.

Pieces overlap 8 px across each cut and carry a disc at their pivot, so a
Boolean union of the posed pieces closes every joint. The output is the
format `puppet.rs` reads: nested `<g id data-pivot>`, absolute M/L/Z rings.
"""
import json
import subprocess
import sys
import tempfile
from math import cos, pi, sin


def read_pgm(path):
    toks = open(path).read().split()
    w, h, mx = int(toks[1]), int(toks[2]), int(toks[3])
    vals = toks[4:]
    return w, h, [int(v) * 2 > mx for v in vals]


def rings_of(w, h, mask):
    """Outline rings of a mask, traced with potrace (flattened by its GeoJSON backend)."""
    with tempfile.TemporaryDirectory() as tmp:
        pbm = f"{tmp}/m.pbm"
        row = (w + 7) // 8
        buf = bytearray(row * h)
        for y in range(h):
            for x in range(w):
                if mask[y * w + x]:
                    buf[y * row + x // 8] |= 0x80 >> (x % 8)
        with open(pbm, "wb") as f:
            f.write(f"P4\n{w} {h}\n".encode() + bytes(buf))
        out = subprocess.run(
            ["potrace", "-b", "geojson", "--turdsize", "20", "--alphamax", "1.0", "-o", "-", pbm],
            capture_output=True, check=True, text=True,
        ).stdout
    rings = []
    for feat in json.loads(out)["features"]:
        geom = feat["geometry"]
        polys = geom["coordinates"] if geom["type"] == "MultiPolygon" else [geom["coordinates"]]
        for poly in polys:
            # potrace's GeoJSON has y up from the bottom edge; outer rings only
            rings.append([(x, h - y) for x, y in poly[0][:-1]])
    return rings


def disc(cx, cy, r):
    return [(cx + r * cos(k / 32 * 2 * pi), cy + r * sin(k / 32 * 2 * pi)) for k in range(32)]


def side(p, a, b):
    """> 0 left of a->b (screen coordinates)."""
    return (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])


def path(rings):
    return " ".join(
        "M " + " L ".join(f"{x:.1f} {y:.1f}" for x, y in r) + " Z" for r in rings if len(r) > 2
    )


def patient(sil_path, lines_path, hand_path):
    w, h, sil = read_pgm(sil_path)
    _, _, line = read_pgm(lines_path)
    _, _, hand = read_pgm(hand_path)
    inside = lambda x, y: 0 <= x < w and 0 <= y < h and sil[y * w + x]

    # forearm: below the arm's drawn outline from the elbow on; hand: its own pixels
    low = {}
    for x in range(430, 626):
        ys = [y for y in range(380, 440) if line[y * w + x]]
        if ys:
            low[x] = max(ys)
    forearm = [False] * (w * h)
    for y in range(h):
        for x in range(w):
            if sil[y * w + x] and ((x in low and y > low[x]) or hand[y * w + x]):
                forearm[y * w + x] = True

    # hip: the crease in the top outline between belly and thigh, and the arm line below it
    top = {x: next((y for y in range(h) if inside(x, y)), h) for x in range(540, 660)}
    cx = max(top, key=lambda x: top[x])
    crease = (cx, top[cx])
    below = (cx - 22, low.get(cx - 22, 420))
    hip = ((crease[0] + below[0]) / 2 - 6, (crease[1] + below[1]) / 2 + 12)

    # neck: back of the neck to the throat
    nb, nf = (306, 222), (379, 191)
    neck = ((nb[0] + nf[0]) / 2, (nb[1] + nf[1]) / 2)
    head_side = side((300, 120), nb, nf)

    def piece(test):
        return [sil[i] and test(i % w, i // w) for i in range(w * h)]

    # signed distance (px) to the hip cut, positive on the legs' side
    span = ((below[0] - crease[0]) ** 2 + (below[1] - crease[1]) ** 2) ** 0.5
    legs_d = lambda x, y: -side((x, y), crease, below) / span
    legs_sign = 1 if legs_d(900, 380) > 0 else -1
    nspan = ((nf[0] - nb[0]) ** 2 + (nf[1] - nb[1]) ** 2) ** 0.5
    head_d = lambda x, y: (side((x, y), nb, nf) / nspan) * (1 if head_side > 0 else -1)

    # forearm and hand lie over hip and thigh: legs and trunk keep their full
    # outline under them (the hidden buttock), the forearm is its own piece
    # turning at the elbow, so the hand can stay on the couch or the thighs
    elbow_x = min(low)
    elbow = (elbow_x + 14, low[elbow_x] - 12)
    legs = piece(lambda x, y: legs_sign * legs_d(x, y) > -4)
    trunk = piece(lambda x, y: legs_sign * legs_d(x, y) < 4 and head_d(x, y) < 4)
    arm = [forearm[i] or (sil[i] and (i % w - elbow[0]) ** 2 + (i // w - elbow[1]) ** 2 < 24 ** 2) for i in range(w * h)]
    head = piece(lambda x, y: head_d(x, y) > -4)

    groups = [
        ("legs", None, rings_of(w, h, legs)),
        ("trunk", hip, rings_of(w, h, trunk) + [disc(hip[0], hip[1], 34)]),
        [("head", neck, rings_of(w, h, head) + [disc(neck[0], neck[1], 21)]),
         ("forearm", elbow, rings_of(w, h, arm) + [disc(elbow[0], elbow[1], 18)])],
    ]
    print(f"crease {crease} hip {hip} neck {neck} elbow {elbow}", file=sys.stderr)
    return w, h, groups


def examiner(sil_path, arm_path):
    """SIL is the body as it stays when the arm swings away: in profile the
    torso lies behind the arm, so it keeps the arm's area except where the
    hand stands out in front of the thigh."""
    w, h, sil = read_pgm(sil_path)
    _, _, arm = read_pgm(arm_path)
    shoulder = (178.0, 360.0)
    groups = [
        ("body", None, rings_of(w, h, sil)),
        ("arm", shoulder, rings_of(w, h, arm) + [disc(shoulder[0], shoulder[1], 24)]),
    ]
    return w, h, groups


def write(w, h, groups, out):
    """Each entry opens a group inside the previous one; a list entry holds
    sibling leaf groups at that depth."""
    lines = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">']
    group = lambda depth, gid, pivot: "  " * depth + f'<g id="{gid}"' + (
        f' data-pivot="{pivot[0]:.1f},{pivot[1]:.1f}">' if pivot else ">")
    opened = 0
    for depth, entry in enumerate(groups, start=1):
        for gid, pivot, rings in entry if isinstance(entry, list) else [entry]:
            lines.append(group(depth, gid, pivot))
            lines.append("  " * (depth + 1) + f'<path d="{path(rings)}" />')
            if isinstance(entry, list):
                lines.append("  " * depth + "</g>")
        if not isinstance(entry, list):
            opened += 1
    for depth in reversed(range(1, opened + 1)):
        lines.append("  " * depth + "</g>")
    lines.append("</svg>")
    open(out, "w").write("\n".join(lines) + "\n")


if __name__ == "__main__":
    kind, sil_path, aux, out = sys.argv[1:5]
    w, h, groups = patient(sil_path, aux, sys.argv[5]) if kind == "patient" else examiner(sil_path, aux)
    write(w, h, groups, out)
