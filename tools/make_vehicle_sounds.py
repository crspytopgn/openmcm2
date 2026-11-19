#!/usr/bin/env python3
"""The engine loops for "Drive the scenery" (mm2-viewer src/drive_sound.rs).

    python3 tools/make_vehicle_sounds.py [CACHE]

Cuts a steady stretch out of each of a few public-domain (CC0 1.0)
recordings by Joseph Sardin, BigSoundBank (https://bigsoundbank.com),
downloaded into CACHE (default ~/.cache/openmcm2-sounds), loops it with a
crossfade so it repeats without a click, evens the loudness and writes it
as 22 kHz mono into crates/mm2-viewer/src/vehicle_sounds/. The UFO's hum
is made here. The game pitches each loop with the vehicle's revs.
Needs numpy.
"""
import os
import sys
import urllib.request
import wave

import numpy as np

SR = 22050
SOURCE = "https://bigsoundbank.com/UPLOAD/bwf-en/{}.wav"
# name: (recording, from s, to s, what it is)
CUTS = {
    "car": ("0966", 10.0, 13.5, "Peugeot 107 1.0 petrol engine"),
    "v8": ("0534", 13.5, 17.5, "AC Cobra 427 V8"),
    "diesel": ("1145", 5.0, 9.0, "Iveco Daily 3.0 diesel engine"),
    "truck": ("1443", 10.0, 14.0, "truck engine, standing"),
    "tractor": ("0949", 13.0, 17.0, "old tractor making hay bales"),
    "heli": ("0264", 0.3, 4.6, "helicopter, from outside"),
    "prop": ("3004", 2.3, 4.7, "plane with engine and propeller"),
    "skid": ("2371", 4.2, 6.0, "squealing of 4 car tyres"),
}
FADE = 0.25


def load(path):
    with wave.open(path) as w:
        n, width, ch, rate = w.getnframes(), w.getsampwidth(), w.getnchannels(), w.getframerate()
        raw = w.readframes(n)
    if width == 2:
        a = np.frombuffer(raw, "<i2").astype(np.float64) / 32768
    else:
        u = np.frombuffer(raw, np.uint8).reshape(-1, 3).astype(np.int32)
        a = (((u[:, 0] | (u[:, 1] << 8) | (u[:, 2] << 16)) << 8) >> 8) / 8388608
    return a.reshape(-1, ch).mean(1), rate


def resample(a, rate):
    t = np.arange(int(len(a) * SR / rate)) * rate / SR
    return np.interp(t, np.arange(len(a)), a)


def loop(a):
    """`a` made to repeat: its last FADE seconds faded into its start."""
    f = int(FADE * SR)
    body, tail = a[f:], a[:f]
    k = np.linspace(0, 1, f)
    body[-f:] = body[-f:] * np.sqrt(1 - k) + tail * np.sqrt(k)
    return body


def even(a, db=-18.0):
    a = a - a.mean()
    rms = np.sqrt((a * a).mean())
    a = a * (10 ** (db / 20) / max(rms, 1e-9))
    return a / max(1.0, np.abs(a).max() / 0.98)


def write(path, a):
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes((np.clip(a, -1, 1) * 32767).astype("<i2").tobytes())


def ufo():
    t = np.arange(int(3.0 * SR)) / SR
    # A low hum on 60 Hz, beating a little, with a soft shimmer: whole
    # cycles of each in 3 s so it loops as it is.
    a = np.sin(2 * np.pi * 60 * t) + 0.6 * np.sin(2 * np.pi * 61 * t) + 0.25 * np.sin(2 * np.pi * 180 * t)
    a += 0.08 * np.sin(2 * np.pi * 900 * t) * (0.5 + 0.5 * np.sin(2 * np.pi * 2 * t))
    return a


def main():
    cache = sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser("~/.cache/openmcm2-sounds")
    out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "crates", "mm2-viewer", "src", "vehicle_sounds")
    os.makedirs(cache, exist_ok=True)
    os.makedirs(out, exist_ok=True)
    for name, (rec, a, b, what) in CUTS.items():
        path = os.path.join(cache, f"{rec}.wav")
        if not os.path.exists(path):
            urllib.request.urlretrieve(SOURCE.format(rec), path)
        sound, rate = load(path)
        cut = resample(sound[int(a * rate):int(b * rate)], rate)
        write(os.path.join(out, f"{name}.wav"), even(loop(cut)))
        print(f"{name}.wav: {what} ({rec}, {a}-{b} s)")
    write(os.path.join(out, "ufo.wav"), even(ufo(), -20.0))
    print("ufo.wav: made here")


main()
