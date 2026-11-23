#!/usr/bin/env python3
"""Packs a finished Free Pack folder for download.

    python3 tools/freepack/pack.py <pack folder> <out folder> [version]

The pack folder has stockmap.toml at its root. Writes:

  out/freepack.json      the manifest: every file's path, size and sha256,
                         and which part it is in
  out/github/            the part zips and the manifest, for a GitHub
                         release (the launcher downloads only the parts
                         with a changed file)
  out/r2/freepack.json   the same manifest, and
  out/r2/files/<sha256>  each file under its own hash, for Cloudflare R2
                         (the browser fetches only files it doesn't have;
                         a file's address never changes, so it caches for good)

Parts are the pack's top-level folders. A big folder is split into several
parts by a hash of each file's path, so a file stays in the same part from
one version to the next, and a change re-downloads only its part.
"""
import hashlib
import json
import os
import shutil
import sys
import time
import zipfile

# About this much per part: small enough that one changed file costs
# little, few enough for a release page.
PART_BYTES = 48 << 20
# 2: the sounds are Ogg Vorbis (.ogg), which OpenMCM2 before 0.7.7 can't play.
FORMAT = 2


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def files_of(root):
    out = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames.sort()
        for name in sorted(filenames):
            if name.startswith(".") or name == "freepack.json":
                continue
            full = os.path.join(dirpath, name)
            rel = os.path.relpath(full, root).replace(os.sep, "/")
            out.append((rel, full))
    return out


def top_of(rel):
    # The menus' sounds get a part of their own, so the new look menus
    # alone (ui/, menus/, sound/ui/) download without the race sounds.
    if rel.lower().startswith("sound/ui/"):
        return "sound-ui"
    return rel.split("/", 1)[0] if "/" in rel else "base"


def part_of(rel, sizes_by_top):
    top = top_of(rel)
    count = max(1, -(-sizes_by_top[top] // PART_BYTES))
    if count == 1:
        return top
    n = int(hashlib.sha1(rel.encode()).hexdigest(), 16) % count
    return f"{top}-{n + 1}"


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    root, out = sys.argv[1], sys.argv[2]
    version = sys.argv[3] if len(sys.argv) > 3 else time.strftime("%Y.%m.%d")
    if not os.path.isfile(os.path.join(root, "stockmap.toml")):
        sys.exit(f"{root}: no stockmap.toml, so not a Free Pack folder")
    listed = files_of(root)
    sizes_by_top = {}
    for rel, full in listed:
        top = top_of(rel)
        sizes_by_top[top] = sizes_by_top.get(top, 0) + os.path.getsize(full)
    files = []
    for rel, full in listed:
        files.append({"path": rel, "size": os.path.getsize(full), "sha256": sha256(full), "part": part_of(rel, sizes_by_top)})
        print(f"\rhashed {len(files)}/{len(listed)}", end="", flush=True)
    print()
    shutil.rmtree(out, ignore_errors=True)
    os.makedirs(os.path.join(out, "github"))
    os.makedirs(os.path.join(out, "r2", "files"))
    parts = []
    for name in sorted({f["part"] for f in files}):
        members = [f for f in files if f["part"] == name]
        # The part's name carries a hash of what it holds, so each version's
        # changed parts are new release files beside the unchanged ones.
        key = hashlib.sha256("".join(f"{f['path']}:{f['sha256']}\n" for f in members).encode()).hexdigest()[:12]
        zname = f"freepack-{name}-{key}.zip"
        zpath = os.path.join(out, "github", zname)
        with zipfile.ZipFile(zpath, "w", zipfile.ZIP_STORED) as z:
            for f in members:
                # Pictures and models are compressed already; text is small.
                # A fixed date and mode, so the same files always make the
                # same zip, byte for byte, whatever the source's file times.
                info = zipfile.ZipInfo(f["path"], date_time=(1980, 1, 1, 0, 0, 0))
                info.external_attr = 0o644 << 16
                with open(os.path.join(root, f["path"]), "rb") as src, z.open(info, "w") as dst:
                    shutil.copyfileobj(src, dst, 1 << 20)
        parts.append({"name": name, "file": zname, "size": os.path.getsize(zpath), "sha256": sha256(zpath)})
        print(f"part {zname}: {len(members)} files, {os.path.getsize(zpath) >> 20} MB")
    manifest = {
        "format": FORMAT,
        "version": version,
        "size": sum(f["size"] for f in files),
        "parts": parts,
        "files": files,
    }
    text = json.dumps(manifest, indent=1)
    for where in [out, os.path.join(out, "github"), os.path.join(out, "r2")]:
        with open(os.path.join(where, "freepack.json"), "w") as f:
            f.write(text)
    for f in files:
        dest = os.path.join(out, "r2", "files", f["sha256"])
        if not os.path.exists(dest):
            shutil.copyfile(os.path.join(root, f["path"]), dest)
    print(f"{len(files)} files, {manifest['size'] >> 20} MB, {len(parts)} parts, version {version}")


if __name__ == "__main__":
    main()
