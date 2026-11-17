#!/usr/bin/env python3
"""Stop-gap Free Pack sounds, made entirely in code (no recordings).

Writes PACK/sound/engine/<class>/ (engine.ini + OnLoop/OffLoop/IdleLoop.wav,
the rev loops the engine pitches by the bike's revs) for the five classes,
and PACK/sound/fx/ (landings, crashes, falls, crowd, cheers, chimes) under
the original effect names the game asks for.

These are placeholders until the pack's real recordings are added: they
are synthesised, so they are free of any licence (CC0). Every loop is
exactly periodic, so it repeats without a click.

    python3 tools/freepack/make_stopgap_sounds.py PACK

Needs numpy.
"""
import os
import sys
import wave

import numpy as np

SR = 22050
rng = np.random.default_rng(2026)


def write(path, x, sr=SR):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    x = np.asarray(x, dtype=np.float64)
    peak = np.max(np.abs(x)) or 1.0
    x = x / peak * 0.89
    data = (np.clip(x, -1, 1) * 32767).astype("<i2").tobytes()
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(sr)
        w.writeframes(data)


def circ_noise(n, lo, hi, tilt=0.0):
    """Band-limited noise that loops seamlessly (shaped in the FFT)."""
    spec = np.fft.rfft(rng.standard_normal(n))
    f = np.fft.rfftfreq(n, 1 / SR)
    band = ((f >= lo) & (f <= hi)).astype(float)
    band *= np.where(f > 0, (np.maximum(f, 1) / 1000.0) ** tilt, 0)
    return np.fft.irfft(spec * band, n)


def circ_conv(a, b):
    n = len(a)
    return np.fft.irfft(np.fft.rfft(a) * np.fft.rfft(b, n), n)


def kernel(n, resonances, decay_ms, noise=0.3, click=0.0):
    """One combustion pulse: damped resonances plus a short noise burst."""
    t = np.arange(n) / SR
    k = np.zeros(n)
    for freq, amp, q in resonances:
        k += amp * np.sin(2 * np.pi * freq * t) * np.exp(-t * freq / q)
    k *= np.exp(-t / (decay_ms / 1000.0))
    burst = rng.standard_normal(n) * np.exp(-t / 0.004) * noise
    k += burst
    if click:
        k[:12] += click * np.hanning(24)[12:] * 4
    return k


def engine_loop(rpm, two_stroke, kind, seconds=2.0):
    """A loop of `seconds` at `rpm`: on (pulling), off (closed throttle)
    or idle."""
    n = int(SR * seconds)
    per_rev = 1.0 if two_stroke else 0.5
    fires = max(1, round(rpm / 60.0 * per_rev * seconds))
    train = np.zeros(n)
    gap = n / fires
    # Uneven firing: a 4-stroke thumper is lumpy, a 2-stroke ring-dings.
    jitter = (0.06 if two_stroke else 0.035) * (2.5 if kind == "idle" else 1.0)
    for i in range(fires):
        at = int(round(i * gap + rng.normal(0, jitter) * gap)) % n
        amp = 1.0 + rng.normal(0, 0.12 if kind != "off" else 0.35)
        if kind == "off" and rng.random() < 0.18:
            amp *= 0.15  # misses on the overrun
        train[at] += max(amp, 0.05)
    klen = min(int(SR * 0.06), n)
    if two_stroke:
        res = [(780, 1.0, 9), (1650, 0.6, 12), (3100, 0.35, 16), (260, 0.5, 4)]
        decay = 7 if kind != "idle" else 12
    else:
        res = [(140, 1.2, 3), (420, 0.8, 6), (1100, 0.4, 10), (2300, 0.2, 14)]
        decay = 14 if kind != "idle" else 20
    if kind == "off":
        res = [(f * 0.9, a * 0.7, q * 1.5) for f, a, q in res]
    k = kernel(klen, res, decay, noise=0.5 if kind == "off" else 0.25, click=0.6 if kind == "off" else 0)
    body = circ_conv(train, k)
    # Intake roar and mechanical rattle, tied to the firing rate.
    fire_hz = fires / seconds
    if kind == "on":
        hiss = circ_noise(n, 200, 6000, tilt=-0.4) * 0.25
        whine = circ_noise(n, fire_hz * 6, fire_hz * 9) * 0.15
    elif kind == "off":
        hiss = circ_noise(n, 300, 5000, tilt=-0.6) * 0.12
        whine = circ_noise(n, fire_hz * 3, fire_hz * 5) * 0.08
    else:
        hiss = circ_noise(n, 400, 4000, tilt=-0.8) * 0.06
        whine = circ_noise(n, 1500, 3500) * 0.04
    rms = np.sqrt(np.mean(body ** 2)) or 1
    x = body / rms + (hiss + whine) / (np.sqrt(np.mean((hiss + whine) ** 2)) or 1) * 0.5
    # A gentle saturation, like a silencer at full chat.
    drive = {"on": 1.6, "off": 1.0, "idle": 0.8}[kind]
    x = np.tanh(x / np.max(np.abs(x)) * drive)
    # Round off the top, as a silencer and the distance do.
    f = np.fft.rfftfreq(n, 1 / SR)
    cut = 4800.0 if two_stroke else 3200.0
    return np.fft.irfft(np.fft.rfft(x) / np.sqrt(1 + (f / cut) ** 4), n)


# Class, two-stroke?, on/off/idle loop rpms, idle and top rpm (the same
# numbers the real recordings use).
CLASSES = [
    ("125", True, 11620, 9312, 1610, 1610, 12500),
    ("250", True, 10982, 3122, 1610, 1610, 11500),
    ("400", False, 5305, 6585, 1518, 1518, 9500),
    ("500", True, 7846, 6301, 1610, 1610, 9000),
    ("600", False, 4442, 3823, 1295, 1295, 8000),
]


def engines(pack):
    for cls, two, on, off, idle, low, high in CLASSES:
        d = os.path.join(pack, "sound", "engine", cls)
        write(os.path.join(d, "OnLoop.wav"), engine_loop(on, two, "on"))
        write(os.path.join(d, "OffLoop.wav"), engine_loop(off, two, "off"))
        write(os.path.join(d, "IdleLoop.wav"), engine_loop(idle, two, "idle"))
        with open(os.path.join(d, "engine.ini"), "w") as f:
            f.write(
                "; Stop-gap engine made in code (tools/freepack/make_stopgap_sounds.py)\n"
                "[Engine]\n"
                f"OnLoop = OnLoop.wav\nOffLoop = OffLoop.wav\nIdleLoop = IdleLoop.wav\n"
                f"OnLoopRpm = {on}\nOffLoopRpm = {off}\nIdleLoopRpm = {idle}\n"
                f"IdleRpm = {low}\nMaxRpm = {high}\n"
            )


def env(n, attack, release):
    t = np.arange(n) / SR
    return np.minimum(1, t / max(attack, 1e-4)) * np.exp(-t / release)


def thump(seconds, f0, f1, noise_lo, noise_hi, release):
    n = int(SR * seconds)
    t = np.arange(n) / SR
    sweep = np.sin(2 * np.pi * np.cumsum(np.linspace(f0, f1, n)) / SR)
    burst = np.fft.irfft(np.fft.rfft(rng.standard_normal(n)) * ((np.fft.rfftfreq(n, 1 / SR) > noise_lo) & (np.fft.rfftfreq(n, 1 / SR) < noise_hi)), n)
    burst /= np.max(np.abs(burst)) or 1
    return (sweep * 1.0 + burst * 0.5) * env(n, 0.002, release)


def clank(seconds, freqs):
    n = int(SR * seconds)
    t = np.arange(n) / SR
    x = sum(np.sin(2 * np.pi * f * t + rng.random() * 6) * np.exp(-t * (3 + i * 2)) for i, f in enumerate(freqs))
    return x * env(n, 0.001, 0.25)


def scrape(seconds):
    n = int(SR * seconds)
    x = circ_noise(n, 800, 7000, tilt=-0.3)
    x /= np.max(np.abs(x)) or 1
    wobble = 0.6 + 0.4 * np.sin(np.linspace(0, 2 * np.pi * rng.uniform(5, 11), n)) ** 2
    return x * wobble * np.linspace(1, 0, n) ** 1.5


def place(dst, src, at):
    i = int(at * SR)
    m = min(len(src), len(dst) - i)
    dst[i : i + m] += src[:m]


def crowd(seconds, loud, lo=250, hi=3500):
    n = int(SR * seconds)
    x = circ_noise(n, lo, hi, tilt=-0.5)
    # Voices: formant bands that swell and fall.
    for _ in range(14):
        f = rng.uniform(300, 1200)
        band = circ_noise(n, f * 0.85, f * 1.15)
        lfo = 0.5 + 0.5 * np.sin(2 * np.pi * rng.integers(1, 4) / seconds * np.arange(n) / SR + rng.random() * 6)
        x += band * lfo * 0.6
    return x / (np.max(np.abs(x)) or 1) * loud


def claps(seconds, density):
    n = int(SR * seconds)
    x = np.zeros(n)
    count = int(density * seconds)
    for _ in range(count):
        c = rng.standard_normal(int(SR * 0.012)) * np.exp(-np.arange(int(SR * 0.012)) / (SR * 0.003))
        place(x, c * rng.uniform(0.3, 1), rng.uniform(0, seconds - 0.02))
    shape = np.minimum(1, np.arange(n) / (SR * 0.4)) * np.exp(-np.maximum(0, np.arange(n) / SR - seconds * 0.5) / (seconds * 0.25))
    return x * shape + crowd(seconds, 0.15) * shape


def vowel(seconds, f1, f2, rise):
    """A crowd's 'ooh' or 'boo': many voices on two formants."""
    n = int(SR * seconds)
    t = np.arange(n) / SR
    x = np.zeros(n)
    for _ in range(30):
        f0 = rng.uniform(110, 240) * (1 + rise * t / seconds)
        ph = 2 * np.pi * np.cumsum(f0 * (1 + 0.01 * np.sin(2 * np.pi * rng.uniform(4, 7) * t))) / SR
        v = sum(np.sin(h * ph) / h for h in range(1, 12))
        x += v * rng.uniform(0.5, 1)
    spec = np.fft.rfft(x)
    f = np.fft.rfftfreq(n, 1 / SR)
    shape = np.exp(-((f - f1) / 120) ** 2) + 0.6 * np.exp(-((f - f2) / 200) ** 2) + 0.05
    x = np.fft.irfft(spec * shape, n)
    return x * np.minimum(1, t / 0.15) * np.minimum(1, (seconds - t) / 0.4)


def chime(notes, step=0.09, tail=0.5):
    n = int(SR * (step * len(notes) + tail))
    x = np.zeros(n)
    for i, f in enumerate(notes):
        m = int(SR * tail)
        t = np.arange(m) / SR
        tone = (np.sin(2 * np.pi * f * t) + 0.3 * np.sin(4 * np.pi * f * t)) * np.exp(-t / 0.18)
        place(x, tone, i * step)
    return x


def effects(pack):
    d = os.path.join(pack, "sound", "fx")
    w = lambda name, x: write(os.path.join(d, name), x)
    w("landhard01.wav", thump(0.45, 90, 45, 100, 1800, 0.12) + 0.3 * clank(0.45, [620, 1340]))
    for k in range(1, 7):
        x = np.zeros(int(SR * 1.6))
        place(x, thump(0.4, rng.uniform(70, 110), 40, 80, 2500, 0.1), 0)
        place(x, 0.6 * clank(0.6, rng.uniform(400, 2400, 3)), rng.uniform(0.02, 0.15))
        place(x, 0.5 * scrape(rng.uniform(0.6, 1.1)), rng.uniform(0.1, 0.3))
        place(x, 0.5 * thump(0.3, 80, 50, 100, 1500, 0.08), rng.uniform(0.4, 0.8))
        w(f"wreck{k:02}.wav", x)
        y = np.zeros(int(SR * 1.0))
        place(y, 0.8 * thump(0.35, rng.uniform(60, 90), 40, 80, 1200, 0.1), 0)
        place(y, 0.4 * scrape(rng.uniform(0.4, 0.7)), 0.08)
        w(f"fall{k:02}.wav", y)
    w("crowdloop.wav", crowd(6.0, 1.0))
    w("applause01.wav", claps(3.0, 260))
    w("applause03.wav", claps(4.5, 380))
    w("oh_01.wav", vowel(1.4, 400, 800, -0.15))
    w("boo_01.wav", vowel(1.6, 350, 650, -0.05))
    w("launch.wav", thump(0.5, 120, 60, 200, 4000, 0.2))
    w("waypoint.wav", chime([1046.5, 1568.0]))
    w("bonus.wav", chime([784.0, 987.8, 1174.7, 1568.0]))
    w("it.wav", chime([659.3, 523.3], step=0.14))


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    pack = sys.argv[1]
    engines(pack)
    effects(pack)
    with open(os.path.join(pack, "sound", "README.txt"), "w") as f:
        f.write(
            "Stop-gap sounds, made in code by tools/freepack/make_stopgap_sounds.py\n"
            "(CC0, no recordings). They will be replaced by real recordings in a\n"
            "later pack update.\n"
        )
    print("wrote", os.path.join(pack, "sound"))


if __name__ == "__main__":
    main()
