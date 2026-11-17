#!/usr/bin/env python3
"""Download the Free Pack's CC0 sources (sources.toml) into cache/.

Usage: python3 tools/fetch.py [--only sky|materials|models]
"""
import json, os, sys, tomllib, urllib.request, zipfile, io

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CACHE = os.path.join(HERE, "cache")
UA = [("User-Agent", "openmcm2-freepack/1.0")]
opener = urllib.request.build_opener(); opener.addheaders = UA
urllib.request.install_opener(opener)

def get(url, path, tries=4):
    if os.path.exists(path): return
    os.makedirs(os.path.dirname(path), exist_ok=True)
    for t in range(tries):
        try:
            urllib.request.urlretrieve(url, path + ".part"); os.replace(path + ".part", path); return
        except Exception as e:
            print(f"  retry {t+1}: {url}: {e}", file=sys.stderr)
    raise SystemExit(f"failed: {url}")

def ph(asset):
    return json.load(urllib.request.urlopen(f"https://api.polyhaven.com/files/{asset}"))

def sky(src):
    for name, asset in src["sky"].items():
        out = os.path.join(CACHE, "hdri", f"{asset}_2k.hdr")
        if os.path.exists(out): continue
        url = ph(asset)["hdri"]["2k"]["hdr"]["url"]
        print("sky", name, asset); get(url, out)

def materials(src):
    for mid in src["materials"]["ambientcg"]:
        d = os.path.join(CACHE, "ambientcg", mid)
        if os.path.isdir(d): continue
        print("material", mid)
        z = os.path.join(CACHE, "ambientcg", f"{mid}.zip")
        get(f"https://ambientcg.com/get?file={mid}_2K-JPG.zip", z)
        with zipfile.ZipFile(z) as f:
            f.extractall(d, [n for n in f.namelist() if n.endswith(".jpg")])
        os.remove(z)

def models(src):
    for asset in src["models"]["polyhaven"]:
        d = os.path.join(CACHE, "polyhaven", asset)
        if os.path.isdir(d): continue
        print("model", asset)
        g = ph(asset)["gltf"]["1k"]["gltf"]
        get(g["url"], os.path.join(d + ".part", os.path.basename(g["url"])))
        for p, i in g.get("include", {}).items():
            get(i["url"], os.path.join(d + ".part", p))
        os.replace(d + ".part", d)

if __name__ == "__main__":
    with open(os.path.join(HERE, "sources.toml"), "rb") as f: src = tomllib.load(f)
    only = sys.argv[sys.argv.index("--only") + 1] if "--only" in sys.argv else None
    for part in (sky, materials, models):
        if only in (None, part.__name__): part(src)
