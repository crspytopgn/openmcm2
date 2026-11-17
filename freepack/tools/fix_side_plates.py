#!/usr/bin/env python3
"""Give the bike's side number plates clean, upright UVs: each side's
"plate" faces in the Bike part are projected flat onto the side (glTF
units, +y up), so the engine's number board reads the right way round from
outside on both sides. The front plate (HandleBars part) is left alone.

    python tools/fix_side_plates.py build/bikes/fp_bike_mx2.glb [...]

glTF UVs: v = 0 at the top of the picture."""
import json, struct, sys
import numpy as np

def fix(path):
    b = bytearray(open(path, "rb").read())
    n = struct.unpack("<I", b[12:16])[0]; j = json.loads(b[20:20 + n]); base = 20 + n + 8
    def view(i, c, dt):
        a = j["accessors"][i]; bv = j["bufferViews"][a["bufferView"]]
        off = base + bv.get("byteOffset", 0) + a.get("byteOffset", 0)
        return np.frombuffer(b, dt, a["count"] * c, off).reshape(-1, c), off
    done = 0
    for me in j["meshes"]:
        if me.get("name") != "Bike": continue
        for p in me["primitives"]:
            if j["materials"][p["material"]].get("name") != "plate": continue
            pos, _ = view(p["attributes"]["POSITION"], 3, np.float32)
            uv, off = view(p["attributes"]["TEXCOORD_0"], 2, np.float32)
            new = uv.copy()
            for side in (1, -1):
                s = pos[:, 0] * side > 0.05
                if not s.any(): continue
                z, y = pos[s, 2], pos[s, 1]
                # square board centred on the panel, as tall as the panel;
                # past its ends the UVs hold the board's edge (no repeats)
                # centred on the rear part of the panel (glTF +z is the back),
                # where the rider's leg doesn't hide it
                zc = z.min() + 0.68 * (z.max() - z.min()); near = np.abs(z - zc) < 0.3
                yc = (y[near].min() + y[near].max()) / 2
                half = (y[near].max() - y[near].min()) / 2 * 1.3       # board a bit taller than the panel: digits fill about 2/3 of it
                u = (zc - z) / (2 * half) + 0.5 if side > 0 else (z - zc) / (2 * half) + 0.5
                v = (yc - y) / (2 * half) + 0.5
                new[s, 0], new[s, 1] = np.clip(u, 0.01, 0.99), np.clip(v, 0.01, 0.99)
            b[off:off + new.nbytes] = new.astype(np.float32).tobytes(); done += int(len(new))
    open(path, "wb").write(b); print(path, "side plate verts", done)

for p in sys.argv[1:]: fix(p)
