#!/usr/bin/env python3
"""Puts the "30" on the Free Pack's 30-second man's board.

The pack's `objects/30secman.glb` held up a blank white board. This
paints the board's two big faces with a bold "30" (each face reading
the right way round), its edges plain white, and embeds the picture.

    python3 board30.py IN.glb OUT.glb
"""
import io
import json
import struct
import sys

import numpy as np
from PIL import Image, ImageDraw, ImageFont

FONT = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"
BOARD = "boardwhite"


def picture():
    w, h = 768, 512
    img = Image.new("RGB", (w, h), (236, 234, 226))
    d = ImageDraw.Draw(img)
    # Taped corners, as a card stapled to its board.
    for x, y in ((40, 40), (w - 40, 40), (40, h - 40), (w - 40, h - 40)):
        d.rectangle((x - 22, y - 7, x + 22, y + 7), fill=(176, 132, 84))
        d.rectangle((x - 7, y - 22, x + 7, y + 22), fill=(176, 132, 84))
    font = ImageFont.truetype(FONT, 400)
    d.text((w / 2, h / 2 + 10), "30", font=font, fill=(18, 18, 18), anchor="mm")
    # A white strip in the corner for the board's edges.
    d.rectangle((0, 0, 12, 12), fill=(236, 234, 226))
    out = io.BytesIO()
    img.save(out, "PNG", optimize=True)
    return out.getvalue()


def main():
    src, dst = sys.argv[1], sys.argv[2]
    data = open(src, "rb").read()
    jlen = struct.unpack("<I", data[12:16])[0]
    j = json.loads(data[20:20 + jlen])
    blen = struct.unpack("<I", data[20 + jlen:24 + jlen])[0]
    binary = bytearray(data[28 + jlen:28 + jlen + blen])

    mat = next(i for i, m in enumerate(j["materials"]) if m.get("name") == BOARD)

    def view(acc):
        a = j["accessors"][acc]
        bv = j["bufferViews"][a["bufferView"]]
        n = {"VEC2": 2, "VEC3": 3}[a["type"]]
        off = bv.get("byteOffset", 0) + a.get("byteOffset", 0)
        stride = bv.get("byteStride", 4 * n)
        return a["count"], n, off, stride

    for mesh in j["meshes"]:
        for p in mesh["primitives"]:
            if p.get("material") != mat:
                continue
            at = p["attributes"]
            cnt, _, po, ps = view(at["POSITION"])
            _, _, no, ns = view(at["NORMAL"])
            _, _, uo, us = view(at["TEXCOORD_0"])
            pos = np.array([struct.unpack_from("<3f", binary, po + i * ps) for i in range(cnt)])
            nor = np.array([struct.unpack_from("<3f", binary, no + i * ns) for i in range(cnt)])
            lo, hi = pos.min(0), pos.max(0)
            for i in range(cnt):
                u = (pos[i, 0] - lo[0]) / (hi[0] - lo[0])
                v = 1.0 - (pos[i, 1] - lo[1]) / (hi[1] - lo[1])
                if nor[i, 2] > 0.5:
                    uv = (u, v)
                elif nor[i, 2] < -0.5:
                    uv = (1.0 - u, v)   # the back reads the right way round too
                else:
                    uv = (0.008, 0.012)
                struct.pack_into("<2f", binary, uo + i * us, *uv)

    png = picture()
    while len(binary) % 4:
        binary.append(0)
    j["bufferViews"].append({"buffer": 0, "byteOffset": len(binary), "byteLength": len(png)})
    binary += png
    while len(binary) % 4:
        binary.append(0)
    j["buffers"][0]["byteLength"] = len(binary)
    j["images"].append({"mimeType": "image/png", "name": "board30", "bufferView": len(j["bufferViews"]) - 1})
    j["textures"].append({"sampler": 0, "source": len(j["images"]) - 1})
    pbr = j["materials"][mat].setdefault("pbrMetallicRoughness", {})
    pbr["baseColorTexture"] = {"index": len(j["textures"]) - 1}
    pbr["baseColorFactor"] = [1.0, 1.0, 1.0, 1.0]

    js = json.dumps(j, separators=(",", ":")).encode()
    while len(js) % 4:
        js += b" "
    out = struct.pack("<III", 0x46546C67, 2, 12 + 8 + len(js) + 8 + len(binary))
    out += struct.pack("<II", len(js), 0x4E4F534A) + js
    out += struct.pack("<II", len(binary), 0x004E4942) + bytes(binary)
    open(dst, "wb").write(out)
    print(dst, len(out))


if __name__ == "__main__":
    main()
