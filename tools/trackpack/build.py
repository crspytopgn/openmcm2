#!/usr/bin/env python3
"""Builds the popular tracks pack from tracks.json.

    python3 tools/trackpack/build.py tools/trackpack/tracks.json out

Downloads each track's zip (and picture) from MDX Racing and writes:

- out/trackpack/index.json: the list the launcher and browser read
- out/trackpack/files/<sha256>.zip: our copy of each download, unchanged
- out/trackpack/pictures/<sha256>.png: its picture (MDX's, or `picture_file`)
- out/trackpack/CREDITS.md: who made each track

Then upload out/trackpack to the R2 bucket under trackpack/ (files and
pictures first, index.json last); publish-trackpack.yml does that.
"""

import hashlib
import io
import json
import os
import sys
import time
import urllib.parse
import urllib.request
import zipfile

from PIL import Image  # pip install pillow

AGENT = "OpenMCM2-trackpack/1"


def get(url, tries=4):
    for i in range(tries):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": AGENT})
            with urllib.request.urlopen(req, timeout=120) as r:
                return r.read()
        except Exception as e:  # sites drop connections now and then
            last = e
            time.sleep(2 + 3 * i)
    raise RuntimeError(f"{url}: {last}")


def quote(url):
    p = urllib.parse.urlsplit(url)
    return urllib.parse.urlunsplit(p._replace(path=urllib.parse.quote(urllib.parse.unquote(p.path))))


def main(listing, out):
    tracks = json.load(open(listing))["tracks"]
    root = os.path.join(out, "trackpack")
    os.makedirs(os.path.join(root, "files"), exist_ok=True)
    os.makedirs(os.path.join(root, "pictures"), exist_ok=True)
    index = []
    for t in tracks:
        data = get(quote(t["mdx"]))
        if not data.startswith(b"PK"):
            sys.exit(f"{t['name']}: not a zip")
        envs = [os.path.basename(n) for n in zipfile.ZipFile(io.BytesIO(data)).namelist() if n.lower().endswith(".env")]
        if not envs:
            sys.exit(f"{t['name']}: no .env in it")
        sha = hashlib.sha256(data).hexdigest()
        if t.get("sha256") and t["sha256"] != sha:
            sys.exit(f"{t['name']}: MDX's file changed ({sha}); check it and update tracks.json")
        open(os.path.join(root, "files", sha + ".zip"), "wb").write(data)
        entry = {k: t[k] for k in ("name", "author", "event", "mdx", "laps", "starter") if k in t}
        entry.update(file=f"files/{sha}.zip", sha256=sha, size=len(data), envs=envs)
        # Its picture, as a PNG (the launcher reads PNGs): MDX's own, or
        # one of ours beside tracks.json for a track MDX has none for.
        picture = None
        if t.get("picture_file"):
            picture = open(os.path.join(os.path.dirname(listing), t["picture_file"]), "rb").read()
        elif t.get("picture_url"):
            try:
                picture = get(quote(t["picture_url"]), 2)
            except RuntimeError as e:
                print("no picture:", e)
        if picture:
            im = Image.open(io.BytesIO(picture)).convert("RGB")
            im.thumbnail((480, 240))
            im.save(os.path.join(root, "pictures", sha + ".png"), optimize=True)
            entry["picture"] = f"pictures/{sha}.png"
        else:
            sys.exit(f"{t['name']}: no picture")
        index.append(entry)
        print(f"{t['event']:9} {t['name']} ({len(data) >> 10} KB)")
    json.dump({"format": 1, "version": time.strftime("%Y.%m.%d"), "tracks": index}, open(os.path.join(root, "index.json"), "w"), indent=1)
    names = {"NATIONAL": "Nationals", "SX": "Supercross", "BAJA": "Baja", "ENDURO": "Enduro", "QUARRIES": "Stunt"}
    with open(os.path.join(root, "CREDITS.md"), "w") as f:
        f.write("# Popular tracks: who made them\n\nEach track belongs to the player who made it. All come from MDX Racing (https://mdxracing.com).\n")
        for ev, label in names.items():
            f.write(f"\n## {label}\n\n")
            for t in (t for t in index if t["event"] == ev):
                f.write(f"- {t['name']}: {('by ' + t['author']) if t.get('author') else 'maker not named on MDX'}\n")
    print(f"{len(index)} tracks, {sum(t['size'] for t in index) >> 20} MB")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    main(sys.argv[1], sys.argv[2])
