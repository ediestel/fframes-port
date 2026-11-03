# trace.py W H < gray8 raw of thieme_atlas_style_reference_sheet.png
# Cuts the front figure into segments at its joints and prints a rig SVG:
# nested <g id data-pivot> groups, one traced outline path per segment, in the
# image's pixel coordinates.
import sys
W, H = int(sys.argv[1]), int(sys.argv[2])
b = sys.stdin.buffer.read()
X0, X1, Y0, Y1 = 20, 360, 60, 830
def ink(x, y): return X0 <= x < X1 and Y0 <= y < Y1 and b[y*W + x] < 205

# joints (image px); the figure's right side is on the image's left
NECK = 195; ARMPIT = 315; ELBOW = 350; WRIST = 443; HIP = 470; KNEE = 615; ANKLE = 770; MIDX = 193.5
def arm_cut_x(y, side):  # line armpit -> top of the shoulder, between arm and torso
    return (131 + (315 - y) * (128 - 131) / (315 - 215)) if side == 'r' else (256 + (315 - y) * (259 - 256) / (315 - 215))
LAP = 8  # pieces overlap across a cut, so the rounded piece ends lie inside the neighbour and the union has no notch

RUNS = {}
def arm_run(x, y, side):
    # below the armpit an arm is its own run of ink in the row: the right arm's
    # runs start left of x 110, the left arm's end right of x 280 (the torso
    # and legs lie within x 120-268); a fixed column cut would trim the torso
    if y not in RUNS:
        runs, start = [], None
        for xx in range(X0, X1 + 1):
            if xx < X1 and ink(xx, y):
                if start is None: start = xx
            elif start is not None:
                runs.append((start, xx - 1)); start = None
        RUNS[y] = runs
    for (s, e) in RUNS[y]:
        if s <= x <= e:
            return s < 110 if side == 'r' else e > 280
    return False

def piece(name, x, y):
    side = 'r' if x < MIDX else 'l'
    if name == 'head': return y < NECK + LAP
    in_arm = (x < arm_cut_x(y, 'r') + (LAP if y < ARMPIT else 0)) if side == 'r' else (x > arm_cut_x(y, 'l') - (LAP if y < ARMPIT else 0))
    if y >= ARMPIT:
        in_arm = arm_run(x, y, side)
    leg = y >= HIP - LAP and not (x < 120 or x > 268)
    if name == 'torso':
        # the torso stops at the cut line itself; the arm reaches LAP past it
        strict = (x < arm_cut_x(y, 'r')) if side == 'r' else (x > arm_cut_x(y, 'l'))
        if y >= ARMPIT: strict = arm_run(x, y, side)
        return NECK - LAP <= y < HIP + LAP and not (y >= 215 and strict)
    s = name[-1]
    if s != side: return False
    base = name[:-2]
    if base in ('upper_arm', 'lower_arm', 'hand'):
        if not in_arm or y < 200: return False
        if base == 'upper_arm': return y < ELBOW + LAP
        if base == 'lower_arm': return ELBOW - LAP <= y < WRIST + LAP
        return y >= WRIST - LAP and y < 530
    if base in ('thigh', 'shin', 'foot'):
        if not leg or y < HIP - LAP: return False
        if (x < MIDX) != (s == 'r'): return False
        if base == 'thigh': return y < KNEE + LAP
        if base == 'shin': return KNEE - LAP <= y < ANKLE + LAP
        return y >= ANKLE - LAP
    return False

def outline(name):
    px = {(x, y) for y in range(Y0, Y1) for x in range(X0, X1) if ink(x, y) and piece(name, x, y)}
    # keep the largest 8-connected blob
    best, seen = set(), set()
    for p in px:
        if p in seen: continue
        blob, stack = set(), [p]; seen.add(p)
        while stack:
            q = stack.pop(); blob.add(q)
            for dx in (-1, 0, 1):
                for dy in (-1, 0, 1):
                    n = (q[0]+dx, q[1]+dy)
                    if n in px and n not in seen: seen.add(n); stack.append(n)
        if len(blob) > len(best): best = blob
    # fill holes: flood the outside within the bbox
    xs = [p[0] for p in best]; ys = [p[1] for p in best]
    bx0, bx1, by0, by1 = min(xs)-1, max(xs)+1, min(ys)-1, max(ys)+1
    out, stack = {(bx0, by0)}, [(bx0, by0)]
    while stack:
        q = stack.pop()
        for n in ((q[0]+1, q[1]), (q[0]-1, q[1]), (q[0], q[1]+1), (q[0], q[1]-1)):
            if bx0 <= n[0] <= bx1 and by0 <= n[1] <= by1 and n not in out and n not in best:
                out.add(n); stack.append(n)
    solid = {(x, y) for x in range(bx0, bx1+1) for y in range(by0, by1+1) if (x, y) not in out}
    # boundary edges of the pixel squares, linked into the outer loop
    edges = {}
    for (x, y) in solid:
        for (a, c, n) in (((x, y), (x+1, y), (x, y-1)), ((x+1, y), (x+1, y+1), (x+1, y)),
                          ((x+1, y+1), (x, y+1), (x, y+1)), ((x, y+1), (x, y), (x-1, y))):
            if n not in solid: edges[a] = c
    start = min(edges); loop = [start]; cur = edges[start]
    while cur != start and len(loop) < 200000:
        loop.append(cur); cur = edges[cur]
    # simplify each half of the closed loop (start -> farthest point -> start)
    far = max(range(len(loop)), key=lambda k: (loop[k][0]-loop[0][0])**2 + (loop[k][1]-loop[0][1])**2)
    simple = rdp(loop[:far+1], 0.9)[:-1] + rdp(loop[far:] + [loop[0]], 0.9)
    return smooth(simple)

def rdp(pts, eps):
    if len(pts) < 3: return pts
    (x0, y0), (x1, y1) = pts[0], pts[-1]
    dx, dy = x1-x0, y1-y0; L = (dx*dx+dy*dy) ** 0.5 or 1e-9
    i, dmax = 0, 0
    for k in range(1, len(pts)-1):
        d = abs(dy*pts[k][0] - dx*pts[k][1] + x1*y0 - y1*x0) / L
        if d > dmax: i, dmax = k, d
    if dmax > eps: return rdp(pts[:i+1], eps)[:-1] + rdp(pts[i:], eps)
    return [pts[0], pts[-1]]

def smooth(pts, n=2):  # Chaikin corner cutting, closed
    pts = pts[:-1]
    for _ in range(n):
        q = []
        for k in range(len(pts)):
            a, c = pts[k], pts[(k+1) % len(pts)]
            q += [(0.75*a[0]+0.25*c[0], 0.75*a[1]+0.25*c[1]), (0.25*a[0]+0.75*c[0], 0.25*a[1]+0.75*c[1])]
        pts = q
    return pts

def d(pts): return 'M ' + ' L '.join(f'{x:.1f} {y:.1f}' for x, y in pts) + ' Z'

P = {'torso': (193.5, 400), 'head': (193.5, NECK),
     'upper_arm_r': (113, 232), 'lower_arm_r': (106, ELBOW), 'hand_r': (88, WRIST),
     'upper_arm_l': (275, 232), 'lower_arm_l': (281, ELBOW), 'hand_l': (300, WRIST),
     'thigh_r': (156, HIP), 'shin_r': (156, KNEE), 'foot_r': (157, ANKLE),
     'thigh_l': (231, HIP), 'shin_l': (230, KNEE), 'foot_l': (230, ANKLE)}
def g(name, *kids):
    x, y = P[name]
    return f'<g id="{name}" data-pivot="{x},{y}"><path d="{d(outline(name))}" />' + ''.join(kids) + '</g>'
body = g('torso', g('head'),
         g('upper_arm_r', g('lower_arm_r', g('hand_r'))), g('upper_arm_l', g('lower_arm_l', g('hand_l'))),
         g('thigh_r', g('shin_r', g('foot_r'))), g('thigh_l', g('shin_l', g('foot_l'))))
print('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1200 896">')
print('<!-- Traced from thieme_atlas_style_reference_sheet.png (front figure) by trace.py: one outline per segment, cut at the joints, pivots in data-pivot. -->')
print(body)
print('</svg>')
