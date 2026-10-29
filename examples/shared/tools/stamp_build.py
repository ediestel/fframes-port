"""The BURNOUT stamp, synthesized: a dry low thud with a short wooden knock on top.
Writes dynamic_media/stamp.wav. Run: python3 -I tools/stamp_build.py"""
import math, os, random, struct, wave

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
OUT = os.path.join(ROOT, "dynamic_media")
SR = 48000

random.seed(11)
n = int(0.6 * SR)
out = []
lp = 0.0
for i in range(n):
    t = i / SR
    # body: a pitch drop 110 -> 48 Hz, gone in a quarter second
    f = 48 + 62 * math.exp(-t * 30)
    body = math.sin(2 * math.pi * f * t) * math.exp(-t * 14)
    # knock: filtered noise, a few milliseconds
    lp += 0.35 * (random.uniform(-1, 1) - lp)
    knock = lp * math.exp(-t * 90)
    out.append(0.9 * body + 0.8 * knock)
peak = max(abs(s) for s in out)
with wave.open(os.path.join(OUT, "stamp.wav"), "wb") as w:
    w.setnchannels(1)
    w.setsampwidth(2)
    w.setframerate(SR)
    w.writeframes(b"".join(struct.pack("<h", int(s / peak * 0.89 * 32767)) for s in out))
