#!/usr/bin/env python3
"""The Free Pack's out-of-bounds slide whistle, made in code (our own, CC0).

    python3 tools/freepack/make_whistle.py PACK

Writes PACK/sound/fx/whistle_up.wav and whistle_down.wav: a cartoon slide
whistle that goes up ("wheee") as a free-mode rider is thrown back off the
world's edge, and down ("ooo") once they start to fall. The throw rises for
about 3.7 s and falls for about as long, so each half is cut to fit.

The sound is a breathy, nearly pure tone (a fipple whistle with a slide):
a soft sine with a touch of second harmonic and a little air noise riding
on the pitch, rounded off and kept below about 1.5 kHz so it never shrills,
with a soft "chiff" at the start and a slow wobble, as a hand pushing a
slide never holds quite still. Needs numpy.
"""
import os
import sys
import wave

import numpy as np

RATE = 44100
rng = np.random.default_rng(1999)


def lowpass(x, cutoff):
    """One-pole low pass, run twice."""
    k = np.exp(-2 * np.pi * cutoff / RATE)
    for _ in range(2):
        y = np.empty_like(x)
        acc = 0.0
        for i, v in enumerate(x):
            acc = v + k * (acc - v)
            y[i] = acc
        x = y
    return x


def whistle(pitch, level):
    """A slide whistle following `pitch` (Hz per sample) at `level` (0-1)."""
    n = len(pitch)
    t = np.arange(n) / RATE
    # The hand on the slide: a slow, uneven wobble (about 5 Hz) that
    # grows as the note goes on.
    wobble = 1 + 0.012 * np.sin(2 * np.pi * 5.3 * t + 0.4) * np.minimum(1, t / 0.6)
    wobble *= 1 + 0.004 * np.sin(2 * np.pi * 2.1 * t)
    f = pitch * wobble
    phase = 2 * np.pi * np.cumsum(f) / RATE
    tone = np.sin(phase) + 0.05 * np.sin(2 * phase + 0.3)
    # Air through the fipple: noise shaped around the note (ring
    # modulated up to the pitch), quieter than the tone.
    air = lowpass(rng.standard_normal(n), 350) * 6.0
    air = air * np.sin(phase + 0.7) * 0.06
    # A breath that comes and goes a little.
    breath = 1 + 0.06 * lowpass(rng.standard_normal(n), 6) * 30
    out = (tone + air) * breath * level
    # The "chiff" as the first breath hits the edge.
    chiff_n = int(0.05 * RATE)
    chiff = lowpass(rng.standard_normal(chiff_n), 1500) * np.linspace(1, 0, chiff_n) ** 2 * 0.25
    out[:chiff_n] += chiff * level[:chiff_n]
    # Rounded off: nothing sharp up where ears are most sensitive.
    return lowpass(out, 2200)


def ease(n, power):
    x = np.linspace(0, 1, n)
    return x, x ** power


def up(seconds=3.6):
    n = int(seconds * RATE)
    x, _ = ease(n, 1)
    # Quick at first, slowing as the slide runs out near the top.
    s = 1 - (1 - x) ** 2.2
    pitch = 420 * (1450 / 420) ** s
    level = np.minimum(1, x / 0.03) * (0.82 + 0.18 * s)
    # Thinning breath over the last stretch, so the top floats.
    level *= np.where(x > 0.85, 1 - (x - 0.85) / 0.15 * 0.35, 1)
    level[-int(0.04 * RATE):] *= np.linspace(1, 0, int(0.04 * RATE))
    return whistle(pitch, level)


def down(seconds=3.6):
    n = int(seconds * RATE)
    x, _ = ease(n, 1)
    # Lazy at first, then dropping faster and faster: the cartoon fall.
    s = x ** 1.6
    pitch = 1450 * (300 / 1450) ** s
    level = np.minimum(1, x / 0.02) * (1 - 0.3 * x)
    level[-int(0.25 * RATE):] *= np.linspace(1, 0, int(0.25 * RATE)) ** 1.5
    return whistle(pitch, level)


def write(path, x, peak=0.32):
    x = x / np.max(np.abs(x)) * peak
    data = (np.clip(x, -1, 1) * 32767).astype("<i2").tobytes()
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(data)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    out = os.path.join(sys.argv[1], "sound", "fx")
    os.makedirs(out, exist_ok=True)
    write(os.path.join(out, "whistle_up.wav"), up())
    write(os.path.join(out, "whistle_down.wav"), down())
    print("wrote", out)


if __name__ == "__main__":
    main()
