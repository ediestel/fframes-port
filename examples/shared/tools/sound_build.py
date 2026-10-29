"""Placeholder score and effects, synthesized: a slow pad, watch ticks, a bell.
Writes into dynamic_media/. Run: python3 -I tools/sound_build.py"""
import math, os, random, struct, wave

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
OUT = os.path.join(ROOT, "dynamic_media")
SR = 48000


def write(name, samples):
    peak = max(1e-9, max(abs(s) for s in samples))
    k = 0.89 / peak
    with wave.open(os.path.join(OUT, name), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(b"".join(struct.pack("<h", int(s * k * 32767)) for s in samples))


def note(n):  # midi -> Hz
    return 440.0 * 2 ** ((n - 69) / 12)


# --- pad: one chord per section, soft attack, slight detune ---------------------------------
# (start s, end s, midi notes): matches the scene times in src/timing.rs
CHORDS = [
    (0.0, 18.4, [45, 52, 57, 60]),        # Gary and the rain: A minor, low, through the frozen peak; the dark is quiet
    (18.4, 41.4, [50, 57, 62, 64]),       # Gary at the doctor's, the verdict: D, open; then a quiet pause
    (42.6, 59.7, [41, 48, 57, 60, 64]),   # geography: F major 7
    (61.2, 84.2, [43, 50, 55, 59, 62]),   # time: G
    (84.2, 192.4, [48, 55, 60, 64, 67]),  # doctors + services: C
    (192.4, 203.9, [45, 52, 57, 60, 64]), # the future is now: A minor 7, open
    (203.9, 217.9, [41, 53, 57, 60, 65]), # Gary again: F
    (217.9, 227.9, [48, 55, 60, 64, 72]), # end: C, open
]
TOTAL = 227.9
# the times above are design seconds; the film plays them at SPEED (src/timing.rs)
SPEED = 1.25
CHORDS = [(s / SPEED, e / SPEED, notes) for s, e, notes in CHORDS]
TOTAL = TOTAL / SPEED
n = int(TOTAL * SR)
pad = [0.0] * n
for start, end, notes in CHORDS:
    a, b = int(max(0, start - 1.5) * SR), int(min(TOTAL, end + 1.5) * SR)
    for m in notes:
        for det in (-0.12, 0.12):
            f = note(m + det / 12 * 1.0) * (1 + det * 0.002)
            amp = 0.10 if m < 50 else 0.06
            ph = random.random() * 6.28
            for i in range(a, b):
                t = i / SR
                env = min(1.0, (t - (start - 1.5)) / 2.5, ((end + 1.5) - t) / 2.5)
                if env <= 0:
                    continue
                lfo = 0.8 + 0.2 * math.sin(t * 0.7 + m)
                pad[i] += amp * env * lfo * (math.sin(2 * math.pi * f * t + ph) + 0.15 * math.sin(4 * math.pi * f * t + ph))
# master fade
for i in range(n):
    t = i / SR
    pad[i] *= min(1.0, t / 2.0, (TOTAL - t) / 4.0)
write("pad.wav", pad)

# --- one watch tick -------------------------------------------------------------------------
random.seed(3)
tick = []
for i in range(int(0.04 * SR)):
    t = i / SR
    tick.append((random.random() * 2 - 1) * math.exp(-t * 260) * 0.6 + math.sin(2 * math.pi * 3200 * t) * math.exp(-t * 180) * 0.5)
write("tick.wav", tick)

# --- bell for 1998 ------------------------------------------------------------------------
bell = []
for i in range(int(3.5 * SR)):
    t = i / SR
    s = sum(a * math.sin(2 * math.pi * f * t) * math.exp(-t * d) for f, a, d in
            [(784, 0.6, 1.2), (1568, 0.3, 2.0), (2352, 0.15, 3.0), (3920, 0.08, 4.5)])
    bell.append(s * min(1.0, t / 0.004))
write("bell.wav", bell)
print("ok")
