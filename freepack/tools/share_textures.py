"""Moves pictures that several models carry into shared files, so the pack
holds each picture once.

    python3 tools/share_textures.py PACKDIR [--dry]

Every .glb under PACKDIR is read; a picture (by its bytes) that two or
more of them embed is written once to PACKDIR/textures/<hash>.<png|jpg>
and each model points at it with a relative glTF image uri. Pictures only
one model uses stay inside it. Lossless: the bytes are the same. Run it on
a finished pack (it can be run again; already shared pictures are kept).
"""
import glob, hashlib, json, os, struct, sys
from collections import defaultdict


def read_glb(path):
    b = open(path, "rb").read()
    assert b[:4] == b"glTF", path
    at, js, bin_ = 12, None, b""
    while at + 8 <= len(b):
        n, kind = struct.unpack("<I4s", b[at:at + 8])
        body = b[at + 8:at + 8 + n]
        if kind == b"JSON": js = json.loads(body)
        elif kind == b"BIN\0": bin_ = body
        at += 8 + ((n + 3) & ~3)
    return js, bin_


def write_glb(path, js, bin_):
    j = json.dumps(js, separators=(",", ":")).encode()
    j += b" " * (-len(j) % 4)
    bin_ += b"\0" * (-len(bin_) % 4)
    out = struct.pack("<4sII", b"glTF", 2, 12 + 8 + len(j) + (8 + len(bin_) if bin_ else 0))
    out += struct.pack("<I4s", len(j), b"JSON") + j
    if bin_: out += struct.pack("<I4s", len(bin_), b"BIN\0") + bin_
    tmp = path + ".tmp"
    open(tmp, "wb").write(out); os.replace(tmp, path)


def view_bytes(js, bin_, i):
    bv = js["bufferViews"][i]; o = bv.get("byteOffset", 0)
    return bin_[o:o + bv["byteLength"]]


def drop_views(js, bin_, drop):
    """Rebuilds the binary chunk without the buffer views in `drop` and
    renumbers every reference to the rest."""
    keep = [i for i in range(len(js.get("bufferViews", []))) if i not in drop]
    remap = {old: new for new, old in enumerate(keep)}
    out = bytearray(); views = []
    for old in keep:
        bv = dict(js["bufferViews"][old]); data = view_bytes(js, bin_, old)
        out += b"\0" * (-len(out) % 8)        # keep accessors aligned
        bv["byteOffset"] = len(out); out += data; views.append(bv)
    js["bufferViews"] = views
    for a in js.get("accessors", []):
        if "bufferView" in a: a["bufferView"] = remap[a["bufferView"]]
        sp = a.get("sparse")
        if sp:
            sp["indices"]["bufferView"] = remap[sp["indices"]["bufferView"]]
            sp["values"]["bufferView"] = remap[sp["values"]["bufferView"]]
    for im in js.get("images", []):
        if "bufferView" in im: im["bufferView"] = remap[im["bufferView"]]
    if js.get("buffers"): js["buffers"][0]["byteLength"] = len(out)
    return bytes(out)


def main():
    root = sys.argv[1]; dry = "--dry" in sys.argv
    files = sorted(p for p in glob.glob(os.path.join(root, "**", "*.glb"), recursive=True))
    users = defaultdict(set); size = {}
    for f in files:
        js, bin_ = read_glb(f)
        for im in js.get("images", []):
            if "bufferView" in im:
                d = view_bytes(js, bin_, im["bufferView"]); h = hashlib.sha1(d).hexdigest()[:20]
                users[h].add(f); size[h] = len(d)
    shared = {h for h, u in users.items() if len(u) > 1}
    before = sum(os.path.getsize(f) for f in files)
    saved = sum(size[h] * (len(users[h]) - 1) for h in shared)
    print(f"{len(files)} models, {len(shared)} pictures shared, saves {saved / 1e6:.1f} MB of {before / 1e6:.1f} MB")
    if dry: return
    tex = os.path.join(root, "textures"); os.makedirs(tex, exist_ok=True)
    for f in files:
        js, bin_ = read_glb(f); drop = set(); changed = False
        for im in js.get("images", []):
            if "bufferView" not in im: continue
            d = view_bytes(js, bin_, im["bufferView"]); h = hashlib.sha1(d).hexdigest()[:20]
            if h not in shared: continue
            ext = "jpg" if im.get("mimeType") == "image/jpeg" or d[:2] == b"\xff\xd8" else "png"
            out = os.path.join(tex, f"{h}.{ext}")
            if not os.path.exists(out): open(out, "wb").write(d)
            drop.add(im.pop("bufferView"))
            im["uri"] = os.path.relpath(out, os.path.dirname(f)).replace(os.sep, "/")
            im["mimeType"] = "image/jpeg" if ext == "jpg" else "image/png"
            changed = True
        if changed:
            write_glb(f, js, drop_views(js, bin_, drop))
    after = sum(os.path.getsize(f) for f in files) + sum(os.path.getsize(p) for p in glob.glob(os.path.join(tex, "*")))
    print(f"pack models + textures: {before / 1e6:.1f} MB -> {after / 1e6:.1f} MB")


if __name__ == "__main__":
    main()
