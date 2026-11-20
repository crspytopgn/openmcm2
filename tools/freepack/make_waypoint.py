#!/usr/bin/env python3
"""Warmer stand-ins for the Free Pack's checkpoint sound, made in code (CC0).

    python3 tools/freepack/make_waypoint.py OUT [horn|clack|bell]

The free waypoint.wav (a gate or lap passed, with Waypoint Chime on) was a
two-note sine chime that sounded clinical. These are trackside sounds
instead:

  horn   a short blip on a stadium air horn, a brassy two-note chord
  clack  a marshal's wooden clapper and a flag snapped beside it
  bell   two quick strikes on a marshal's hand bell, low and warm

With a name, writes OUT/sound/fx/waypoint.wav from that one; without, all
three as OUT/waypoint_<name>.wav to listen to. Needs numpy.
"""
import os
import sys
import wave

import numpy as np

RATE = 44100
rng = np.random.default_rng(2002)


def t_of(seconds):
    return np.arange(int(seconds * RATE)) / RATE


def biquad(x, kind, f, q):
    """An RBJ biquad: 'lp', 'hp' or 'bp' (constant peak gain)."""
    w = 2 * np.pi * f / RATE
    a = np.sin(w) / (2 * q)
    c = np.cos(w)
    if kind == "lp":
        b = [(1 - c) / 2, 1 - c, (1 - c) / 2]
    elif kind == "hp":
        b = [(1 + c) / 2, -(1 + c), (1 + c) / 2]
    else:
        b = [a, 0.0, -a]
    a0, a1, a2 = 1 + a, -2 * c, 1 - a
    b0, b1, b2 = (v / a0 for v in b)
    a1, a2 = a1 / a0, a2 / a0
    y = np.empty_like(x)
    x1 = x2 = y1 = y2 = 0.0
    for i, v in enumerate(x):
        o = b0 * v + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2
        x2, x1, y2, y1 = x1, v, y1, o
        y[i] = o
    return y


def place(out, x, at):
    i = int(at * RATE)
    n = min(len(x), len(out) - i)
    out[i:i + n] += x[:n]


def horn():
    """A stadium air horn's quick blip: a brassy chord that bends up into
    pitch as the air comes, held a moment, then cut."""
    seconds = 0.34
    t = t_of(seconds)
    out = np.zeros(len(t))
    for f0, level in [(349.2, 1.0), (440.0, 0.8), (523.3, 0.45)]:
        # The reed takes a moment to come up to pitch.
        f = f0 * (1 - 0.06 * np.exp(-t / 0.025))
        ph = 2 * np.pi * np.cumsum(f) / RATE
        # A band-limited buzz (odd and even harmonics, falling off).
        buzz = sum(np.sin(h * ph) / h ** 0.9 for h in range(1, int(7000 / f0)))
        out += buzz * level
    # The horn's bell: a formant around 1.1 kHz, the top rolled off so it
    # honks rather than rasps.
    out = biquad(biquad(out, "lp", 2400, 0.7), "lp", 3500, 0.7) + 0.4 * biquad(out, "bp", 1100, 1.2)
    env = np.minimum(1, t / 0.012) * np.where(t > seconds - 0.07, (seconds - t) / 0.07, 1)
    env *= 1 - 0.15 * t / seconds
    return np.tanh(out * env * 0.35)


def clack():
    """A marshal's wooden clapper, two blocks struck together, with a
    flag snapped tight beside it."""
    out = np.zeros(int(0.45 * RATE))
    # The blocks: a sharp click ringing a few woody modes for a few
    # hundredths of a second.
    t = t_of(0.12)
    click = rng.standard_normal(len(t)) * np.exp(-t / 0.0015)
    wood = sum(
        a * np.sin(2 * np.pi * f * t + rng.uniform(0, 6)) * np.exp(-t / d)
        for f, a, d in [(720, 1.0, 0.030), (1310, 0.7, 0.020), (2240, 0.45, 0.012), (3480, 0.25, 0.008)]
    )
    block = wood + 0.6 * biquad(click, "bp", 2500, 0.8)
    place(out, block, 0.0)
    # The flag: cloth cracking tight, a burst of fluttering, bright noise
    # that dies fast.
    t = t_of(0.22)
    flutter = 0.55 + 0.45 * np.sign(np.sin(2 * np.pi * 38 * t + 0.4 * np.sin(2 * np.pi * 9 * t)))
    snap = biquad(rng.standard_normal(len(t)), "hp", 1500, 0.7) * flutter
    snap *= np.minimum(1, t / 0.004) * np.exp(-t / 0.06)
    crack = biquad(rng.standard_normal(len(t)), "bp", 3800, 1.5) * np.exp(-t / 0.006) * 1.5
    place(out, 0.55 * (snap + crack), 0.07)
    return out


def bell():
    """A marshal's brass hand bell struck twice, low and warm, a
    hand-bell's uneven partials rather than pure tones."""
    out = np.zeros(int(0.9 * RATE))
    t = t_of(0.75)
    f0 = 587.3
    partials = [(1.0, 1.0, 0.45), (2.0, 0.35, 0.25), (2.42, 0.45, 0.20), (3.0, 0.18, 0.14), (4.16, 0.12, 0.08)]
    for at, level in [(0.0, 1.0), (0.12, 0.7)]:
        tone = sum(
            a * np.sin(2 * np.pi * f0 * r * (1 + 0.0015 * np.sin(2 * np.pi * 4.5 * t)) * t) * np.exp(-t / d)
            for r, a, d in partials
        )
        # The clapper's tap.
        tap = biquad(rng.standard_normal(len(t)), "bp", 2800, 1.0) * np.exp(-t / 0.002) * 0.4
        strike = (tone + tap) * np.minimum(1, t / 0.0015)
        place(out, strike * level, at)
    # A little rounder: the brightest partials softened.
    return biquad(out, "lp", 5000, 0.7)


SOUNDS = {"horn": horn, "clack": clack, "bell": bell}
# Peaks that sound about as loud as each other (and as the old chime).
PEAKS = {"horn": 0.4, "clack": 0.7, "bell": 0.6}


def write(path, x, peak=0.6):
    x = x / np.max(np.abs(x)) * peak
    tail = int(0.004 * RATE)
    x[-tail:] *= np.linspace(1, 0, tail)
    data = (np.clip(x, -1, 1) * 32767).astype("<i2").tobytes()
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(data)


def main():
    if len(sys.argv) not in (2, 3) or (len(sys.argv) == 3 and sys.argv[2] not in SOUNDS):
        sys.exit(__doc__)
    out = sys.argv[1]
    if len(sys.argv) == 3:
        d = os.path.join(out, "sound", "fx")
        os.makedirs(d, exist_ok=True)
        write(os.path.join(d, "waypoint.wav"), SOUNDS[sys.argv[2]](), PEAKS[sys.argv[2]])
        print("wrote", os.path.join(d, "waypoint.wav"))
    else:
        os.makedirs(out, exist_ok=True)
        for name, make in SOUNDS.items():
            write(os.path.join(out, f"waypoint_{name}.wav"), make(), PEAKS[name])
        print("wrote", out)


if __name__ == "__main__":
    main()
