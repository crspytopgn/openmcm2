"""Stock objects that are not built by objects.py: imported CC-BY models fitted
to the stock bounds (fit_import.py) and stadium shells (arena.py).

    python blender/external.py SK_DIR [NAME ...]

SK_DIR holds the Sketchfab downloads as SK_DIR/<folder>/model.glb (fetch them
with the Sketchfab download API and your own token; uids are in sources.toml).
objects.py leaves these names alone, and stockmap.py lists them by kind.
"""
import os, sys, subprocess

HERE = os.path.dirname(os.path.abspath(__file__))
PY = sys.executable

# name -> (kind, folder or None, extra fit_import args)
IMPORTS = {
    "switchyard": ("import", "substation", ["--cap=60000"]),
    "wingen_post": ("import", "turbine", ["--cap=60000", "--by=z"]),
    "shovel": ("import", "excavator", ["--cap=60000"]),
    "ht_tower": ("import", "pylon", ["--by=z", "--stretch"]),
    "ht_tower02": ("import", "pylon", ["--cap=60000"]),
}
SHELLS = ("b1b_02", "euro", "football")
# bikes: folder -> bike_import.py arguments (model name first)
BIKES = {
    "sonnie": ["mx2", "--front=+y", "--weld", "--cap=6000", "--tyre=pasted__rubber_tire1", "--finish=sonnie", "--paint=pasted__plastics1,pasted__radiator_cover1", "--livery=red"],
    "enduro": ["enduro", "--paint=Material", "--livery=blue", "--slot-wheels"],
}
KIND = {**{n: v[0] for n, v in IMPORTS.items()}, **{n: "arena" for n in SHELLS}}

if __name__ == "__main__":
    sk = sys.argv[1]; names = sys.argv[2:] or list(KIND) + list(BIKES)
    for n in names:
        if n in BIKES:
            src = os.path.join(sk, n, "sonnie.glb" if n == "sonnie" else "enduro.glb")
            subprocess.run([PY, "-I", os.path.join(HERE, "bike_import.py"), src, *BIKES[n]], check=True); continue
        if n in SHELLS:
            subprocess.run([PY, "-I", os.path.join(HERE, "arena.py"), n], check=True)
        else:
            kind, folder, extra = IMPORTS[n]
            subprocess.run([PY, "-I", os.path.join(HERE, "fit_import.py"), os.path.join(sk, folder, "model.glb"), n, *extra], check=True)
