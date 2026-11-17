#!/usr/bin/env python3
"""The Free Pack's menu sounds, made in code (our own, CC0).

    python3 tools/freepack/make_ui_sounds.py PACK

Writes PACK/sound/ui/<name>.wav under the names the menus ask for: the
pointer going over a button (Beep13, Beep09, Beep), a click (CLICK04), the
options' ratchet (Ratchet03), the pull-down panels sliding out and back
(Servo12, Servo12R) and the main menu's air piston (Piston03). Each is a
short, soft sound: a little mechanical, like a workshop, never shrill.
Needs numpy.
"""
import os
import sys
import wave

import numpy as np

RATE = 22050
rng = np.random.default_rng(2000)


def t(seconds):
    return np.arange(int(seconds * RATE)) / RATE


def env(n, attack=0.002, release=0.03):
    """A short fade in and out, so nothing clicks."""
    e = np.ones(n)
    a, r = max(1, int(attack * RATE)), max(1, int(release * RATE))
    e[:a] = np.linspace(0, 1, a)
    e[-r:] *= np.linspace(1, 0, r)
    return e


def lowpass(x, cutoff):
    """One-pole low pass, run twice."""
    k = np.exp(-2 * np.pi * cutoff / RATE)
    for _ in range(2):
        y = np.empty_like(x)
        acc = 0.0
        for i, v in enumerate(x):
            acc = (1 - k) * v + k * acc
            y[i] = acc
        x = y
    return x


def highpass(x, cutoff):
    return x - lowpass(x, cutoff)


def tick(length=0.045, tone=1900, body=420, bright=0.5):
    """A soft plastic tick: a damped knock with a little click on top."""
    s = t(length)
    knock = np.sin(2 * np.pi * body * s) * np.exp(-s * 90)
    ping = np.sin(2 * np.pi * tone * s) * np.exp(-s * 160) * bright
    click = highpass(rng.standard_normal(len(s)), 2500) * np.exp(-s * 600) * 0.35
    return (knock + ping + click) * env(len(s), 0.0005, 0.01)


def servo(length=1.45, up=True):
    """A small motor sliding a panel: a geared whine that rises (or falls)
    with a soft start and stop, over a rattle of teeth."""
    s = t(length)
    shape = np.clip(s / 0.15, 0, 1) * np.clip((length - s) / 0.25, 0, 1)
    sweep = np.linspace(0, 1, len(s)) if up else np.linspace(1, 0, len(s))
    f = 95 + 55 * sweep + 10 * shape
    phase = 2 * np.pi * np.cumsum(f) / RATE
    hum = sum(np.sin(phase * k + k) / k ** 1.3 for k in range(1, 9))
    teeth = lowpass(rng.standard_normal(len(s)), 1400) * (0.5 + 0.5 * np.sign(np.sin(phase * 7)))
    x = lowpass(hum * 0.6 + teeth * 0.25, 2600) * shape
    # A gentle bump as it reaches the end.
    stop = int((length - 0.18) * RATE)
    x[stop:] += tick(0.18, tone=700, body=160, bright=0.2)[: len(x) - stop] * 0.8
    return x


def ratchet(length=0.72, teeth=11):
    """A ratchet wound a few teeth, each click a little closer."""
    out = np.zeros(int(length * RATE))
    at = 0.02
    for i in range(teeth):
        c = tick(0.04, tone=2600 + 80 * (i % 3), body=900, bright=0.6) * (0.8 - 0.02 * i)
        p = int(at * RATE)
        out[p:p + len(c)] += c[: len(out) - p]
        at += 0.065 - 0.002 * i
    return lowpass(out, 6000)


def piston(length=1.3):
    """An air piston: a dull thunk and a hiss of air let out."""
    s = t(length)
    thunk = np.sin(2 * np.pi * (70 + 40 * np.exp(-s * 30)) * s) * np.exp(-s * 18)
    air = highpass(lowpass(rng.standard_normal(len(s)), 5000), 900)
    hiss = air * np.clip((s - 0.05) / 0.08, 0, 1) * np.exp(-np.maximum(s - 0.1, 0) * 3.2) * 0.35
    return (thunk * 0.9 + hiss) * env(len(s), 0.001, 0.2)


def save(path, mono, gain=0.55):
    mono = mono / max(1e-6, np.abs(mono).max()) * gain
    # A touch wide: the right a hair later.
    right = np.concatenate([np.zeros(6), mono[:-6]])
    pcm = (np.stack([mono, right], 1) * 32767).astype("<i2")
    with wave.open(path, "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(pcm.tobytes())


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    out = os.path.join(sys.argv[1], "sound", "ui")
    os.makedirs(out, exist_ok=True)
    sounds = {
        "Beep13": (tick(0.04, tone=2100, body=520), 0.35),
        "Beep09": (tick(0.06, tone=1650, body=440), 0.38),
        "Beep": (tick(0.05, tone=1850, body=480), 0.36),
        "CLICK04": (tick(0.05, tone=1300, body=300, bright=0.35), 0.5),
        "Ratchet03": (ratchet(), 0.45),
        "Servo12": (servo(1.43, True), 0.4),
        "Servo12R": (servo(1.54, False), 0.4),
        "Piston03": (piston(), 0.5),
    }
    for name, (x, gain) in sounds.items():
        save(os.path.join(out, name + ".wav"), x, gain)
        print("wrote", name)


if __name__ == "__main__":
    main()
