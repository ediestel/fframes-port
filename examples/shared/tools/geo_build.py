"""Turns the OSM / OSRM / Nominatim downloads in assets_raw/geo into src/geo_data.rs.

Coordinates become kilometres in a local equirectangular projection around
(LAT0, LON0); y grows southwards like SVG. Run: python3 -I tools/geo_build.py
"""
import json, math, os

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
GEO = os.path.join(ROOT, "assets_raw", "geo")
LAT0, LON0 = 53.0, 13.4
KX = 111.32 * math.cos(math.radians(LAT0))
KY = 110.57


def proj(lon, lat):
    return ((lon - LON0) * KX, -(lat - LAT0) * KY)


def load(name):
    with open(os.path.join(GEO, name)) as f:
        return json.load(f)


def simplify(pts, tol):
    """Douglas-Peucker on a list of (x, y)."""
    if len(pts) < 3:
        return pts
    keep = [False] * len(pts)
    keep[0] = keep[-1] = True
    stack = [(0, len(pts) - 1)]
    while stack:
        a, b = stack.pop()
        ax, ay = pts[a]
        bx, by = pts[b]
        dx, dy = bx - ax, by - ay
        n = math.hypot(dx, dy) or 1e-9
        best, idx = -1.0, -1
        for i in range(a + 1, b):
            px, py = pts[i]
            d = abs(dy * (px - ax) - dx * (py - ay)) / n if n > 1e-9 else math.hypot(px - ax, py - ay)
            if d > best:
                best, idx = d, i
        if best > tol:
            keep[idx] = True
            stack += [(a, idx), (idx, b)]
    return [p for p, k in zip(pts, keep) if k]


def area(ring):
    s = 0.0
    for (x1, y1), (x2, y2) in zip(ring, ring[1:] + ring[:1]):
        s += x1 * y2 - x2 * y1
    return abs(s) / 2


def centroid(ring):
    xs = [p[0] for p in ring]
    ys = [p[1] for p in ring]
    return sum(xs) / len(xs), sum(ys) / len(ys)


def d_of(rings, closed=True, nd=3):
    out = []
    for r in rings:
        if len(r) < 2:
            continue
        out.append("M" + " L".join(f"{x:.{nd}f} {y:.{nd}f}" for x, y in r) + (" Z" if closed else ""))
    return " ".join(out)


def stitch(ways):
    """Join open way geometries that share end points into rings."""
    ways = [list(w) for w in ways if len(w) > 1]
    rings = []
    while ways:
        cur = ways.pop(0)
        changed = True
        while changed and cur[0] != cur[-1]:
            changed = False
            for i, w in enumerate(ways):
                if w[0] == cur[-1]:
                    cur += w[1:]
                elif w[-1] == cur[-1]:
                    cur += w[::-1][1:]
                elif w[-1] == cur[0]:
                    cur = w[:-1] + cur
                elif w[0] == cur[0]:
                    cur = w[::-1][:-1] + cur
                else:
                    continue
                ways.pop(i)
                changed = True
                break
        rings.append(cur)
    return rings


def geom(g):
    return [(p["lon"], p["lat"]) for p in g]


def polygons(elements, pred):
    """Outer rings (in lon/lat) of the ways and multipolygons matching pred."""
    out = []
    for e in elements:
        t = e.get("tags", {})
        if not pred(t):
            continue
        if e["type"] == "way" and "geometry" in e:
            g = geom(e["geometry"])
            if g[0] == g[-1]:
                out.append((t, g))
        elif e["type"] == "relation":
            outer = [geom(m["geometry"]) for m in e.get("members", []) if m.get("role") == "outer" and "geometry" in m]
            for r in stitch(outer):
                if r[0] == r[-1]:
                    out.append((t, r))
    return out


def P(ring, tol):
    return simplify([proj(*p) for p in ring], tol)


rs = []  # rust source


def emit_str(name, value, doc):
    rs.append(f"/// {doc}\npub const {name}: &str = \"{value}\";\n")


def emit_pts(name, pts, doc):
    body = ", ".join(f"({x:.3f}, {y:.3f})" for x, y in pts)
    rs.append(f"/// {doc}\npub const {name}: &[(f32, f32)] = &[{body}];\n")


# --- states: land outlines from Natural Earth 10m (public domain) -------------
# (the Nominatim boundaries include the coastal waters, so the coast with Rügen
# and Usedom did not show; ne_de.json / ne_mv.json are the Natural Earth
# admin-0 Germany and admin-1 Mecklenburg-Vorpommern geometries)
de = load("ne_de.json")
de_rings = [P(poly[0], 0.8) for poly in de["coordinates"]]
de_rings = [r for r in de_rings if area(r) > 30]
emit_str("GERMANY", d_of(de_rings, nd=1), "Germany (land), simplified to ~1 km.")
mv = load("ne_mv.json")
mv_rings = [P(poly[0], 0.3) for poly in mv["coordinates"]]
mv_rings = [r for r in mv_rings if area(r) > 2]
emit_str("MV", d_of(mv_rings, nd=2), "Mecklenburg-Vorpommern (land, with Rügen and Usedom).")

# --- Berlin -------------------------------------------------------------------
b = load("berlin.json")["elements"][0]
outer = [geom(m["geometry"]) for m in b["members"] if m.get("role") == "outer" and "geometry" in m]
brings = [P(r, 0.15) for r in stitch(outer)]
emit_str("BERLIN", d_of([max(brings, key=area)], nd=2), "Berlin city limits.")

# --- route Berlin -> Feldberg (OSRM) ---------------------------------------------
route = [proj(lon, lat) for lon, lat in load("route.json")["routes"][0]["geometry"]["coordinates"]]
route = simplify(route, 0.05)
emit_pts("ROUTE", route, "Driving route Berlin Mitte -> Klinik am Haussee (OSRM, 136 km).")

# --- lakes between Berlin and Feldberg --------------------------------------------
lakes = []
for t, r in polygons(load("lakes.json")["elements"], lambda t: t.get("natural") == "water"):
    pr = P(r, 0.04)
    a = area(pr)
    if a > 0.04 and len(pr) > 3:
        lakes.append((pr, a))
lakes.sort(key=lambda l: centroid(l[0])[1])  # south to north: they appear along the drive
rs.append("/// Lakes between Berlin and Feldberg: (path, centroid x, centroid y, area km2), south to north.\n"
          "pub const LAKES: &[(&str, f32, f32, f32)] = &[\n")
for pr, a in lakes:
    cx, cy = centroid(pr)
    rs.append(f"    (\"{d_of([pr], nd=2)}\", {cx:.2f}, {cy:.2f}, {a:.3f}),\n")
rs.append("];\n")

# --- Feldberg detail ----------------------------------------------------------------
fel = load("feldberg.json")["elements"]
water = polygons(fel, lambda t: t.get("natural") == "water")
forest = polygons(fel, lambda t: t.get("natural") == "wood" or t.get("landuse") == "forest")
haussee = [r for t, r in water if "Haussee" in t.get("name", "")]
emit_str("FELD_WATER", d_of([P(r, 0.006) for t, r in water if "Haussee" not in t.get("name", "")]), "Lakes around Feldberg.")
emit_str("FELD_FOREST", d_of([P(r, 0.01) for t, r in forest]), "Forest around Feldberg.")
hs = max((P(r, 0.004) for r in haussee), key=area)
emit_str("HAUSSEE", d_of([hs]), "The Haussee.")
hcx, hcy = centroid(hs)
rs.append(f"/// Centre of the Haussee.\npub const HAUSSEE_C: (f32, f32) = ({hcx:.3f}, {hcy:.3f});\n")
roads = [geom(e["geometry"]) for e in fel if e["type"] == "way" and e.get("tags", {}).get("highway") in
         ("secondary", "tertiary", "residential", "unclassified", "primary") and "geometry" in e]
emit_str("FELD_ROADS", d_of([P(r, 0.006) for r in roads], closed=False), "Roads in Feldberg.")
bld = [geom(e["geometry"]) for e in fel if e["type"] == "way" and "building" in e.get("tags", {}) and "geometry" in e]
emit_str("FELD_BUILDINGS", d_of([P(r, 0.002) for r in bld]), "Buildings in Feldberg.")

# --- trails -------------------------------------------------------------------------------
tr = load("trails.json")["elements"]
ways = [P(geom(e["geometry"]), 0.004) for e in tr if e["type"] == "way" and "geometry" in e]
# trails near the Haussee first, so they draw outwards from the lake
ways.sort(key=lambda w: min(math.hypot(x - hcx, y - hcy) for x, y in w))
rs.append("/// Footpaths and tracks, nearest to the Haussee first: (path, distance from the lake centre km).\n"
          "pub const TRAILS: &[(&str, f32)] = &[\n")
for w in ways:
    dist = min(math.hypot(x - hcx, y - hcy) for x, y in w)
    rs.append(f"    (\"{d_of([w], closed=False)}\", {dist:.3f}),\n")
rs.append("];\n")

# --- clinic -----------------------------------------------------------------------------
cl = load("clinic.json")["elements"]
crings = [P(r, 0.001) for t, r in polygons(cl, lambda t: True)]
crings.sort(key=area)
emit_str("CLINIC_BUILDING", d_of([crings[0]]), "Klinik am Haussee, building outline.")
emit_str("CLINIC_GROUNDS", d_of([crings[-1]]), "Klinik am Haussee, grounds.")
ccx, ccy = centroid(crings[0])
rs.append(f"/// Centre of the clinic building.\npub const CLINIC_C: (f32, f32) = ({ccx:.3f}, {ccy:.3f});\n")
bx, by = proj(13.4050, 52.5200)
rs.append(f"/// Berlin Mitte.\npub const BERLIN_C: (f32, f32) = ({bx:.3f}, {by:.3f});\n")

header = ("//! Map geometry, generated by tools/geo_build.py from OpenStreetMap data\n"
          "//! (© OpenStreetMap contributors, ODbL), an OSRM route and Nominatim outlines.\n"
          f"//! Kilometres in an equirectangular projection around {LAT0}N {LON0}E; y points south.\n\n")
with open(os.path.join(ROOT, "src", "geo_data.rs"), "w") as f:
    f.write(header + "\n".join(rs))
print("lakes", len(lakes), "trails", len(ways), "water", len(water), "forest", len(forest), "haussee", len(haussee),
      "clinic", [round(area(r), 4) for r in crings], "route pts", len(route))
