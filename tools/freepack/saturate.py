#!/usr/bin/env python3
"""Give the Free Pack's scenery a little more colour, in place.

    python3 tools/freepack/saturate.py <pack folder> <amount>

<amount> is how much the colour is lifted: 0.10 means 10 % more. It is a
gentle "vibrance" lift, not a plain saturation one:

- brightness stays put (the lift is done round each pixel's own luminance,
  in linear light), so contrast doesn't change;
- greys stay grey, and colours that are already strong rise less than dull
  ones, so nothing turns cartoonish or clips.

What it touches: the ground pictures (ground/), the skies (skies/), the trees
and plants (eco/, both the cards and the models) and the trackside objects
(objects/ and the shared textures/). Only colour pictures change: a model's
normal, roughness or other data maps are left alone. Bikes, riders, menus
and the HUD are left as they are.

Run it on the finished pack folder (after share_textures.py), before the
free-track previews are drawn and before pack.py.
"""
import io, json, os, struct, sys
import numpy as np
from PIL import Image

DIRS = ["ground", "skies", "eco", "objects", "textures"]
PICTURE = (".png", ".jpg", ".jpeg")


def lift(im, amount):
    """`im` with its colour lifted by `amount` (see the module notes)."""
    mode = im.mode
    alpha = None
    if mode in ("RGBA", "LA", "P", "PA"):
        im = im.convert("RGBA")
        alpha = im.getchannel("A")
    rgb = np.asarray(im.convert("RGB")).astype(np.float32) / 255.0
    if mode in ("L", "I", "I;16", "F"):
        return None  # grey pictures have no colour to lift
    lin = np.where(rgb <= 0.04045, rgb / 12.92, ((rgb + 0.055) / 1.055) ** 2.4)
    y = (lin @ np.array([0.2126, 0.7152, 0.0722], np.float32))[..., None]
    mx, mn = lin.max(2, keepdims=True), lin.min(2, keepdims=True)
    sat = (mx - mn) / np.maximum(mx, 1e-6)
    k = 1.0 + amount * (1.0 - sat) ** 0.5  # dull colours rise most
    out = y + (lin - y) * k
    # never push a channel out of range: pull k back where it would
    top = np.where(out.max(2, keepdims=True) > 1.0,
                   (1.0 - y) / np.maximum(mx - y, 1e-6), np.inf)
    bot = np.where(out.min(2, keepdims=True) < 0.0, y / np.maximum(y - mn, 1e-6), np.inf)
    k = np.minimum(k, np.maximum(1.0, np.minimum(top, bot)))
    out = np.clip(y + (lin - y) * k, 0.0, 1.0)
    srgb = np.where(out <= 0.0031308, out * 12.92, 1.055 * out ** (1 / 2.4) - 0.055)
    res = Image.fromarray((srgb * 255.0 + 0.5).astype(np.uint8), "RGB")
    if alpha is not None:
        res.putalpha(alpha)
    return res


def save(im, fmt, path_or_buf):
    if fmt == "JPEG":
        im.convert("RGB").save(path_or_buf, "JPEG", quality=92, subsampling=0)
    else:
        im.save(path_or_buf, "PNG", optimize=False, compress_level=6)


def lift_file(path, amount):
    im = Image.open(path)
    fmt = im.format
    res = lift(im, amount)
    if res is None:
        return False
    save(res, fmt, path)
    return True


def read_glb(path):
    b = open(path, "rb").read()
    if b[:4] != b"glTF":
        return None
    n = struct.unpack("<I", b[12:16])[0]
    doc = json.loads(b[20:20 + n])
    rest = b[20 + n:]
    binc = b""
    if len(rest) >= 8 and rest[4:8] == b"BIN\0":
        m = struct.unpack("<I", rest[:4])[0]
        binc = rest[8:8 + m]
    return doc, binc


def write_glb(path, doc, binc):
    js = json.dumps(doc, separators=(",", ":")).encode()
    js += b" " * (-len(js) % 4)
    binc += b"\0" * (-len(binc) % 4)
    parts = [struct.pack("<I4s", len(js), b"JSON"), js]
    if binc:
        parts += [struct.pack("<I4s", len(binc), b"BIN\0"), binc]
    body = b"".join(parts)
    open(path, "wb").write(struct.pack("<4sII", b"glTF", 2, 12 + len(body)) + body)


def colour_images(doc):
    """(images used as colour, images used as data maps), as index sets."""
    tex = doc.get("textures", [])
    def src(t):
        if t is None or t.get("index") is None or t["index"] >= len(tex):
            return None
        return tex[t["index"]].get("source")
    colour, data = set(), set()
    for m in doc.get("materials", []):
        pbr = m.get("pbrMetallicRoughness", {})
        for key, into in ((pbr.get("baseColorTexture"), colour),
                          (m.get("emissiveTexture"), colour),
                          (pbr.get("metallicRoughnessTexture"), data),
                          (m.get("normalTexture"), data),
                          (m.get("occlusionTexture"), data)):
            s = src(key)
            if s is not None:
                into.add(s)
        for ext in m.get("extensions", {}).values():
            for k, v in ext.items():
                if isinstance(v, dict) and "index" in v:
                    (colour if "olor" in k else data).add(src(v))
    return colour, data


def lift_glb(path, amount, ext_colour, ext_data):
    got = read_glb(path)
    if got is None:
        return 0
    doc, binc = got
    colour, data = colour_images(doc)
    here = os.path.dirname(path)
    for i, im in enumerate(doc.get("images", [])):
        if "uri" in im and not im["uri"].startswith("data:"):
            f = os.path.normpath(os.path.join(here, im["uri"]))
            if i in colour:
                ext_colour.add(f)
            if i in data:
                ext_data.add(f)
    views = doc.get("bufferViews", [])
    todo = {}
    for i in colour - data:
        im = doc["images"][i]
        if "bufferView" in im:
            todo[im["bufferView"]] = im.get("mimeType", "image/png")
    if not todo:
        return 0
    spans = sorted((v.get("byteOffset", 0), v.get("byteOffset", 0) + v["byteLength"])
                   for v in views if v.get("buffer", 0) == 0)
    if any(a[1] > b[0] for a, b in zip(spans, spans[1:])):
        print(f"  {path}: buffer views overlap, left alone")
        return 0
    out, n = bytearray(), 0
    for vi, v in enumerate(views):
        if v.get("buffer", 0) != 0:
            continue
        off = v.get("byteOffset", 0)
        chunk = binc[off:off + v["byteLength"]]
        if vi in todo:
            im = Image.open(io.BytesIO(chunk))
            res = lift(im, amount)
            if res is not None:
                buf = io.BytesIO()
                save(res, "JPEG" if "jpeg" in todo[vi] else "PNG", buf)
                chunk = buf.getvalue()
                n += 1
        out += b"\0" * (-len(out) % 4)
        v["byteOffset"] = len(out)
        v["byteLength"] = len(chunk)
        out += chunk
    if doc.get("buffers"):
        doc["buffers"][0]["byteLength"] = len(out) + (-len(out) % 4)
    write_glb(path, doc, bytes(out))
    return n


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    pack, amount = sys.argv[1], float(sys.argv[2])
    if not os.path.isfile(os.path.join(pack, "stockmap.toml")):
        sys.exit(f"{pack} isn't a pack folder (no stockmap.toml)")
    if amount == 0:
        return
    ext_colour, ext_data, pictures = set(), set(), []
    n_glb = n_emb = 0
    for d in DIRS:
        for root, _, files in os.walk(os.path.join(pack, d)):
            for f in sorted(files):
                p = os.path.normpath(os.path.join(root, f))
                if f.lower().endswith(".glb"):
                    n = lift_glb(p, amount, ext_colour, ext_data)
                    n_glb += n > 0
                    n_emb += n
                elif f.lower().endswith(PICTURE):
                    pictures.append(p)
    # A shared picture a model names changes only if some model uses it for
    # colour and none as a data map; a loose picture no model names (ground,
    # skies, tree cards) is always colour.
    named = ext_colour | ext_data
    n_pic = 0
    for p in pictures:
        if p in named and (p not in ext_colour or p in ext_data):
            continue
        n_pic += lift_file(p, amount)
    print(f"lifted colour {amount:+.0%}: {n_pic} pictures, {n_emb} pictures inside {n_glb} models")


if __name__ == "__main__":
    main()
