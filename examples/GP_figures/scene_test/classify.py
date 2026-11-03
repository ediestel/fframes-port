"""PPM (P6) on stdin -> one PBM mask per colour class in argv[1].
Classes by nearest sampled source colour; background = bg pixels reachable
from the border, enclosed bg-like pixels go to mattress/pillow."""
import sys
from collections import deque

out = sys.argv[1]
blank = [tuple(map(int, r.split(","))) for r in sys.argv[2:]]  # x0,y0,x1,y1 regions to clear (labels)

data = sys.stdin.buffer.read()
parts = data.split(maxsplit=4)
w, h = int(parts[1]), int(parts[2])
px = parts[4]

PAL = {
    "bg": [(242, 244, 238)],
    "blue": [(146, 171, 194), (121, 136, 147), (146, 164, 177)],
    "dark": [(40, 49, 53), (45, 58, 67), (58, 69, 76)],
    "steel": [(81, 93, 99)],
    "orange": [(191, 123, 104)],
    "pillow": [(243, 241, 229)],
    "mattress": [(224, 235, 247)],
    "side": [(189, 195, 197)],
}
flat = [(k, c) for k, cs in PAL.items() for c in cs]

cls = [None] * (w * h)
for i in range(w * h):
    r, g, b = px[3 * i], px[3 * i + 1], px[3 * i + 2]
    cls[i] = min(flat, key=lambda kc: (kc[1][0] - r) ** 2 + (kc[1][1] - g) ** 2 + (kc[1][2] - b) ** 2)[0]

for x0, y0, x1, y1 in blank:
    for y in range(y0, y1):
        for x in range(x0, x1):
            cls[y * w + x] = "bg"

# background flood from the border over bg/pillow-like light pixels
seen = bytearray(w * h)
q = deque(i for i in range(w * h) if (i % w in (0, w - 1) or i // w in (0, h - 1)) and cls[i] == "bg")
for i in q:
    seen[i] = 1
while q:
    i = q.popleft()
    x, y = i % w, i // w
    for j in ((i - 1) if x else -1, (i + 1) if x < w - 1 else -1, i - w, i + w):
        if 0 <= j < w * h and not seen[j] and cls[j] == "bg":
            seen[j] = 1
            q.append(j)
for i in range(w * h):
    if cls[i] == "bg" and not seen[i]:
        r, g, b = px[3 * i], px[3 * i + 1], px[3 * i + 2]
        cls[i] = "pillow" if r - b > 8 else "mattress"

for k in ["blue", "dark", "steel", "orange", "pillow", "mattress", "side"]:
    with open(f"{out}/{k}.pbm", "wb") as f:
        f.write(f"P4\n{w} {h}\n".encode())
        row = (w + 7) // 8
        buf = bytearray(row * h)
        for y in range(h):
            for x in range(w):
                if cls[y * w + x] == k:
                    buf[y * row + x // 8] |= 0x80 >> (x % 8)
        f.write(buf)
print(w, h, {k: cls.count(k) for k in PAL}, file=sys.stderr)
