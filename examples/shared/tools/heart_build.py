"""One heartbeat (lub-dub), synthesized: dynamic_media/heart.wav.
Run: python3 -I tools/heart_build.py"""
import math, os, struct, wave

SR = 48000
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "dynamic_media", "heart.wav")


def thump(t, f0, decay):
    # a low sine that drops in pitch, with a fast attack; the second and third
    # harmonics and a short knock make it audible on small speakers too
    if t < 0:
        return 0.0
    f = f0 * (1.0 + 0.6 * math.exp(-t * 40))
    env = math.exp(-t * decay) * min(1.0, t / 0.004)
    body = math.sin(2 * math.pi * f * t) + 0.55 * math.sin(4 * math.pi * f * t) + 0.3 * math.sin(6 * math.pi * f * t)
    knock = math.sin(2 * math.pi * 180 * t) * math.exp(-t * 70)
    return (body + 0.5 * knock) * env


n = int(0.6 * SR)
s = [0.75 * thump(i / SR, 52, 18) + 0.5 * thump(i / SR - 0.16, 64, 22) for i in range(n)]
peak = max(abs(x) for x in s)
with wave.open(OUT, "wb") as w:
    w.setnchannels(1)
    w.setsampwidth(2)
    w.setframerate(SR)
    w.writeframes(b"".join(struct.pack("<h", int(x / peak * 0.89 * 32767)) for x in s))
