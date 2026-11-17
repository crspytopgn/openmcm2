#!/usr/bin/env python3
"""The Free Pack's sounds, made from public-domain (CC0) recordings.

    python3 tools/freepack/make_sounds.py PACK [CACHE]

Downloads a handful of CC0 recordings from BigSoundBank (Joseph Sardin,
bigsoundbank.com, "free and royalty free", CC0 1.0) into CACHE (default
~/.cache/openmcm2-sounds) and writes PACK/sound/:

* engine/<class>/: each class's engine as a set of steady loops recorded
  at known revs, pulling (On), shut off (Off) and at idle, listed in
  engine.ini as `OnLoops = file@rpm, ...`. The game plays the two loops
  nearest the revs of the moment, so no loop is pitched far.

  The loops are built pitch-synchronously from single engine cycles cut
  out of real recordings: a small two-stroke pulling through its gears
  (0603) for the two-strokes and a four-stroke quad revving (0712) for the
  four-strokes. Each cycle keeps its own exhaust note; laying the cycles
  closer together or further apart sets the revs without the chipmunk or
  slowed-tape sound that pitching a whole recording gives. Bigger engines
  stretch the cycles a little, which lowers their note as a bigger pipe
  does.

* fx/: landings, falls and crashes (body falls, a jump onto a dirt road,
  falling steel bars), the crowd (a real motocross meeting, 0888) and
  applause (300 and 600 people), under the original game's names. The
  chimes and the crowd's "ooh" and "boo" are still made in code
  (make_stopgap_sounds.py).

Every loop repeats without a click. Needs numpy.
"""
import os
import sys
import urllib.request
import wave

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import make_stopgap_sounds as synth  # noqa: E402

SR = 32000
SOURCE = "https://bigsoundbank.com/UPLOAD/bwf-en/{}.wav"
RECORDINGS = {
    "0603": "75 cm3 motorcycle acceleration",
    "0712": "Quad accelerations",
    "0888": "Motocross circuit",
    "0020": "Applause 300 people",
    "0021": "Applause 600 people",
    "1837": "Jumping on dirt road 1",
    "1838": "Jumping on dirt road 2",
    "2452": "Body fall 1",
    "2453": "Body fall 2",
    "2454": "Body fall 3",
    "2455": "Body fall 4",
    "2456": "Body fall 5",
    "1771": "Falling sheet steel bar 1",
    "1774": "Falling sheet steel bar 4",
    "1776": "Fall 2 sheet steel bars 1",
}


def fetch(cache, key):
    path = os.path.join(cache, key + ".wav")
    if not os.path.exists(path):
        os.makedirs(cache, exist_ok=True)
        req = urllib.request.Request(SOURCE.format(key), headers={"User-Agent": "openmcm2-freepack"})
        with urllib.request.urlopen(req, timeout=120) as r, open(path + ".part", "wb") as f:
            f.write(r.read())
        os.replace(path + ".part", path)
    return path


def load(path):
    """Mono float samples at SR."""
    w = wave.open(path)
    sr, ch, width = w.getframerate(), w.getnchannels(), w.getsampwidth()
    raw = w.readframes(w.getnframes())
    if width == 2:
        x = np.frombuffer(raw, "<i2").astype(float) / 32768
    elif width == 3:
        b = np.frombuffer(raw, np.uint8).reshape(-1, 3)
        x = ((b[:, 0].astype(np.int32) | (b[:, 1].astype(np.int32) << 8) | (b[:, 2].astype(np.int32) << 16)) << 8 >> 8) / 8388608.0
    else:
        x = np.frombuffer(raw, "<i4").astype(float) / 2147483648.0
    if ch > 1:
        x = x.reshape(-1, ch).mean(1)
    return resample_to(x, sr, SR)


def resample_to(x, sr, to):
    if sr == to:
        return x
    n = int(round(len(x) * to / sr))
    X = np.fft.rfft(x)
    m = n // 2 + 1
    Y = np.zeros(m, complex)
    k = min(m, len(X))
    Y[:k] = X[:k]
    return np.fft.irfft(Y, n) * n / len(x)


def shelf(x, lo=None, hi=None, order=4):
    """High- and low-pass, by FFT (circular, so loops stay seamless)."""
    X = np.fft.rfft(x)
    f = np.fft.rfftfreq(len(x), 1 / SR)
    g = np.ones_like(f)
    if lo:
        g *= 1 - 1 / np.sqrt(1 + (f / lo) ** (2 * order))
    if hi:
        g /= np.sqrt(1 + (f / hi) ** (2 * order))
    return np.fft.irfft(X * g, len(x))


def write(path, x, level):
    """Writes `x` with its RMS at `level` dBFS (peaks kept under full)."""
    os.makedirs(os.path.dirname(path), exist_ok=True)
    rms = np.sqrt(np.mean(x ** 2)) or 1
    x = x / rms * 10 ** (level / 20)
    peak = np.max(np.abs(x))
    if peak > 0.97:
        x = np.tanh(x / peak * 1.4) / np.tanh(1.4) * 0.97
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes((np.clip(x, -1, 1) * 32767).astype("<i2").tobytes())


# ---------------------------------------------------------------------
# Engine cycles.


def f0_frames(x, lo, hi, hop=0.01, win=0.12):
    N, H = int(win * SR), int(hop * SR)
    out = []
    for i in range(0, len(x) - N, H):
        s = x[i : i + N] * np.hanning(N)
        rms = np.sqrt(np.mean(s ** 2))
        S = np.fft.rfft(s, 2 * N)
        ac = np.fft.irfft(np.abs(S) ** 2)[:N]
        ac /= ac[0] + 1e-12
        a, b = int(SR / hi), int(SR / lo)
        k = a + int(np.argmax(ac[a:b]))
        out.append((i + N // 2, SR / k, ac[k], rms))
    return out


def cycles(x, lo, hi, conf, floor_db):
    """Single engine cycles: (period in samples, the cycle windowed over
    two periods round its firing peak)."""
    x = shelf(x, lo=lo * 0.6)
    low = shelf(x, hi=500)
    hop = int(0.01 * SR)
    frames = f0_frames(x, lo, hi)
    good = {c // hop: f for c, f, q, r in frames if q > conf and 20 * np.log10(r + 1e-9) > floor_db}
    if not good:
        return []
    starts = sorted(good)
    pos, end = starts[0] * hop, starts[-1] * hop
    # The firing peaks all point one way.
    sign = 1.0 if np.max(low) >= -np.min(low) else -1.0
    low = low * sign
    out, last, prev = [], None, None
    while pos < end:
        f = good.get(pos // hop)
        if f is None:
            later = [s for s in starts if s * hop > pos]
            if not later:
                break
            pos, last = later[0] * hop, None
            continue
        P = int(SR / f)
        a, b = max(0, pos - P // 3), min(len(x), pos + P // 3)
        m = a + int(np.argmax(low[a:b]))
        if last is not None and m - last < 0.6 * P:
            m = last + P
        # Line the cycle up with the one before (the same point of the
        # cycle each time), so the cycles laid end to end stay in step.
        if prev is not None and last is not None:
            best, at = -np.inf, m
            for shift in range(-P // 4, P // 4 + 1):
                c = m + shift
                if c - P < 0 or c + P >= len(x) or c - last < 0.7 * P:
                    continue
                seg = low[c - P // 2 : c + P // 2]
                if len(seg) != len(prev):
                    continue
                score = float(np.dot(seg, prev))
                if score > best:
                    best, at = score, c
            m = at
        if m - P >= 0 and m + P < len(x):
            out.append((P, x[m - P : m + P] * np.hanning(2 * P)))
            prev = low[m - P // 2 : m + P // 2]
        last, pos = m, m + P
    return out


def stretch(g, ratio):
    n = max(8, int(round(len(g) * ratio)))
    return np.interp(np.linspace(0, len(g) - 1, n), np.arange(len(g)), g)


def engine_loop(pool, fire_hz, seconds, form, rng, amp_jitter, time_jitter, miss=0.0, weak=1.0):
    """A loop of whole cycles at `fire_hz` firings a second."""
    K = max(2, int(round(seconds * fire_hz)))
    n = int(round(K * SR / fire_hz))
    out = np.zeros(n)
    period = SR / fire_hz
    scaled = np.array([P * form for P, _ in pool])
    # The cycles recorded nearest these revs sound most like them.
    near = np.argsort(np.abs(scaled - period))[: max(40, len(pool) // 4)]
    for k in range(K):
        g = pool[rng.choice(near)][1]
        if form != 1.0:
            g = stretch(g, form)
        # Faster than the cycle was recorded: only its middle, two of the
        # new periods wide, so the cycles stay distinct firings.
        if len(g) > 2 * period:
            half = int(period)
            g = g[len(g) // 2 - half : len(g) // 2 + half] * np.sqrt(np.hanning(2 * half))
        a = max(0.05, 1.0 + rng.normal(0, amp_jitter))
        if rng.random() < miss:
            a *= weak
        at = int(round(k * n / K + rng.normal(0, time_jitter) * period)) - len(g) // 2
        np.add.at(out, (np.arange(len(g)) + at) % n, g * a)
    return out


# Class: two-stroke?, idle rpm, top rpm, cycle stretch, the source's
# cycles' tone (cut above, Hz).
CLASSES = [
    ("125", True, 1610, 12500, 0.92, 9000),
    ("250", True, 1610, 11500, 1.08, 7500),
    ("400", False, 1518, 9500, 1.0, 7000),
    ("500", True, 1610, 9000, 1.25, 6000),
    ("600", False, 1295, 8000, 1.18, 6000),
]


def rpm_points(low, high, count):
    return [int(round(v)) for v in np.geomspace(low, high, count)]


def engines(pack, two_cycles, four_cycles):
    for cls, two, idle, top, form, cut in CLASSES:
        rng = np.random.default_rng(int(cls))
        pool = two_cycles if two else four_cycles
        per_rev = 1.0 if two else 0.5
        d = os.path.join(pack, "sound", "engine", cls)
        for old in os.listdir(d) if os.path.isdir(d) else []:
            if old.endswith(".wav"):
                os.remove(os.path.join(d, old))
        ons, offs = [], []
        for rpm in rpm_points(idle * 1.5, top, 6):
            x = engine_loop(pool, rpm / 60 * per_rev, 2.0, form, rng, 0.10, 0.015)
            name = f"On{rpm}.wav"
            write(os.path.join(d, name), shelf(x, lo=25, hi=cut), -15)
            ons.append(f"{name}@{rpm}")
        for rpm in rpm_points(idle * 1.5, top, 4):
            # Shut off: weak, uneven firing, some missing, the top dulled.
            x = engine_loop(pool, rpm / 60 * per_rev, 2.0, form * 1.04, rng, 0.35, 0.03, miss=0.3 if two else 0.15, weak=0.15)
            name = f"Off{rpm}.wav"
            write(os.path.join(d, name), shelf(x, lo=30, hi=cut * 0.45), -21)
            offs.append(f"{name}@{rpm}")
        # Idle: a lumpy, unhurried beat (a two-stroke four-strokes at idle).
        x = engine_loop(pool, idle / 60 * per_rev, 3.0, form * 1.06, rng, 0.25, 0.05, miss=0.35 if two else 0.08, weak=0.3)
        write(os.path.join(d, "Idle.wav"), shelf(x, lo=25, hi=cut * 0.4), -21)
        with open(os.path.join(d, "engine.ini"), "w") as f:
            f.write(
                "; Made by tools/freepack/make_sounds.py from CC0 recordings (BigSoundBank)\n"
                "[Engine]\n"
                f"OnLoops = {', '.join(ons)}\n"
                f"OffLoops = {', '.join(offs)}\n"
                f"IdleLoop = Idle.wav\nIdleLoopRpm = {idle}\n"
                # The single loops older versions of the game read.
                f"OnLoop = {ons[-2].split('@')[0]}\nOnLoopRpm = {ons[-2].split('@')[1]}\n"
                f"OffLoop = {offs[1].split('@')[0]}\nOffLoopRpm = {offs[1].split('@')[1]}\n"
                f"IdleRpm = {idle}\nMaxRpm = {top}\n"
            )


# ---------------------------------------------------------------------
# Effects.


def trim(x, db=-50, pad=0.01):
    """Cuts the silence off both ends."""
    env = np.abs(x)
    on = np.nonzero(env > np.max(env) * 10 ** (db / 20))[0]
    if len(on) == 0:
        return x
    a, b = max(0, on[0] - int(pad * SR)), min(len(x), on[-1] + int(pad * SR))
    y = x[a:b].copy()
    fade = min(len(y) // 4, int(0.03 * SR))
    y[-fade:] *= np.linspace(1, 0, fade)
    return y


def mix(*parts, length=None):
    """Lays (sound, at seconds, gain) parts over one another."""
    n = length or max(int(at * SR) + len(s) for s, at, _ in parts)
    out = np.zeros(n)
    for s, at, gain in parts:
        i = int(at * SR)
        m = min(len(s), n - i)
        out[i : i + m] += s[:m] * gain
    return out


def seamless(x, seconds, start, fade=0.5):
    """A loop of `seconds` cut from `x` at `start`, its end crossfaded into
    its beginning."""
    n, f = int(seconds * SR), int(fade * SR)
    a = int(start * SR)
    body = x[a : a + n + f].copy()
    w = np.linspace(0, 1, f)
    body[:f] = body[:f] * np.sqrt(w) + body[n : n + f] * np.sqrt(1 - w)
    return body[:n]


def effects(pack, rec):
    d = os.path.join(pack, "sound", "fx")
    rng = np.random.default_rng(11)
    falls = [trim(shelf(rec[k], lo=40)) for k in ("2452", "2453", "2454", "2455", "2456")]
    dirt = [trim(shelf(rec[k], lo=35)) for k in ("1837", "1838")]
    steel = [trim(shelf(rec[k], lo=80, hi=9000)) for k in ("1771", "1774", "1776")]
    # A landing: the jump onto the dirt road, heavier.
    land = mix((stretch(dirt[0], 1.25), 0, 1.0), (falls[0], 0.0, 0.35))
    write(os.path.join(d, "landhard01.wav"), shelf(land, hi=6000), -16)
    for k in range(1, 7):
        body = falls[(k - 1) % len(falls)]
        write(os.path.join(d, f"fall{k:02}.wav"), mix((body, 0, 1.0), (dirt[k % 2], rng.uniform(0.05, 0.15), 0.4)), -16)
        # A wreck: the bike's metal clattering down with the rider.
        wreck = mix(
            (stretch(steel[k % 3], rng.uniform(1.1, 1.4)), 0, 0.55),
            (falls[k % len(falls)], rng.uniform(0.0, 0.1), 1.0),
            (dirt[(k + 1) % 2], rng.uniform(0.2, 0.45), 0.6),
            (stretch(steel[(k + 1) % 3], 1.5), rng.uniform(0.35, 0.6), 0.3),
        )
        write(os.path.join(d, f"wreck{k:02}.wav"), shelf(wreck, hi=8000), -15)
    # The crowd: a real motocross meeting, its bikes far off.
    meet = shelf(rec["0888"], lo=120, hi=6000)
    write(os.path.join(d, "crowdloop.wav"), seamless(meet, 8.0, 40.0), -24)
    write(os.path.join(d, "applause01.wav"), trim(rec["0020"][: int(9 * SR)] * np.minimum(1, np.linspace(3, 0, int(9 * SR)))), -18)
    write(os.path.join(d, "applause03.wav"), trim(rec["0021"][: int(12 * SR)] * np.minimum(1, np.linspace(3, 0, int(12 * SR)))), -17)


def main():
    if len(sys.argv) not in (2, 3):
        sys.exit(__doc__)
    pack = sys.argv[1]
    cache = sys.argv[2] if len(sys.argv) == 3 else os.path.expanduser("~/.cache/openmcm2-sounds")
    rec = {k: load(fetch(cache, k)) for k in RECORDINGS}
    two = cycles(rec["0603"], 60, 160, conf=0.55, floor_db=-30)
    four = cycles(rec["0712"], 18, 90, conf=0.45, floor_db=-40)
    print(f"{len(two)} two-stroke and {len(four)} four-stroke cycles")
    # Everything else first (the chimes, ooh and boo), then ours over it.
    synth.SR = SR
    synth.effects(pack)
    engines(pack, two, four)
    effects(pack, rec)
    with open(os.path.join(pack, "sound", "README.txt"), "w") as f:
        f.write(
            "Made by tools/freepack/make_sounds.py from public-domain (CC0) recordings\n"
            "by Joseph Sardin, BigSoundBank (https://bigsoundbank.com):\n"
            + "".join(f"  {k}  {v}\n" for k, v in RECORDINGS.items())
            + "The chimes and the crowd's ooh and boo are made in code. All CC0.\n"
        )
    print("wrote", os.path.join(pack, "sound"))


if __name__ == "__main__":
    main()
