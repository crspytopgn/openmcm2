#!/usr/bin/env python3
"""Turns a Free Pack's sounds into Ogg Vorbis, a fifth to a tenth the size.

    python3 tools/freepack/ogg_sounds.py <pack folder> [quality]

Every .wav under <pack>/sound becomes a .ogg beside it (same name, same
length to the sample, so loops stay seamless), and the .wav goes. The
game reads either kind (crates/mm2-formats/src/ogg.rs); the sources stay
WAVs, so each publish encodes from the lossless recordings.

Needs ffmpeg with libvorbis. Quality is Vorbis -q (default 6, about
100 kbit/s for a 44.1 kHz mono clip: clean even pitched down on the engine).
The output is the same bytes every time for the same input (bitexact), so
an unchanged sound keeps its hash and isn't downloaded again.
"""
import os
import struct
import subprocess
import sys


def wav_frames(path):
    """Sample frames in a plain PCM WAV."""
    with open(path, "rb") as f:
        data = f.read()
    if data[:4] != b"RIFF" or data[8:12] != b"WAVE":
        raise ValueError(f"{path}: not a WAV")
    pos, channels, bits, frames = 12, None, None, None
    while pos + 8 <= len(data):
        cid, size = data[pos : pos + 4], struct.unpack("<I", data[pos + 4 : pos + 8])[0]
        body = data[pos + 8 : pos + 8 + size]
        if cid == b"fmt ":
            channels, bits = struct.unpack("<H", body[2:4])[0], struct.unpack("<H", body[14:16])[0]
        elif cid == b"data":
            frames = len(body) // (channels * bits // 8)
        pos += 8 + size + (size & 1)
    if frames is None:
        raise ValueError(f"{path}: no sound in it")
    return frames


def ogg_frames(path):
    """Sample frames in an Ogg Vorbis file: its last page's granule position."""
    with open(path, "rb") as f:
        data = f.read()
    last = data.rfind(b"OggS")
    if last < 0:
        raise ValueError(f"{path}: not Ogg")
    return struct.unpack("<q", data[last + 6 : last + 14])[0]


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    root = os.path.join(sys.argv[1], "sound")
    quality = sys.argv[2] if len(sys.argv) > 2 else "6"
    before = after = count = 0
    for d, _, names in os.walk(root):
        for n in sorted(names):
            if not n.lower().endswith(".wav"):
                continue
            src = os.path.join(d, n)
            dst = src[:-4] + ".ogg"
            subprocess.run(
                ["ffmpeg", "-v", "error", "-y", "-i", src, "-map_metadata", "-1",
                 "-fflags", "+bitexact", "-flags:a", "+bitexact",
                 "-c:a", "libvorbis", "-q:a", quality, dst],
                check=True,
            )
            want, got = wav_frames(src), ogg_frames(dst)
            if want != got:
                sys.exit(f"{src}: {want} samples, but the Ogg has {got}")
            before += os.path.getsize(src)
            after += os.path.getsize(dst)
            count += 1
            os.remove(src)
    print(f"{count} sounds to Ogg Vorbis: {before / 1e6:.1f} MB -> {after / 1e6:.1f} MB")


if __name__ == "__main__":
    main()
