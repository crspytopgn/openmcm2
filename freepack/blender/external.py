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
    "bobcat": ("import", "skidsteer", ["--cap=40000", "--blot=0:0.285,0.458,0.325,0.484/0:0.198,0.548,0.24,0.58/0:0.423,0.492,0.467,0.514"]),
    "tractor": ("import", "tractor", ["--cap=40000", "--hue=75:170:6:0.9/40:70:40:0.15"]),
    "tractor_old": ("import", "tractorold", ["--cap=40000"]),
    "combine": ("import", "combine", ["--cap=40000"]),
    "lawnchair01": ("import", "chair", ["--cap=20000"]),
    "300zx": ("import", "kiri", ["--cap=30000"]),
    "4runner_blue": ("import", "bokaroo", ["--cap=30000"]),
    "4runner_brown": ("import", "bokaroo", ["--cap=30000", "--hue=170:250:28:0.7"]),
    "4runner_green": ("import", "bokaroo", ["--cap=30000", "--hue=170:250:115:0.6"]),
    "4runner_white": ("import", "bokaroo", ["--cap=30000", "--hue=170:250:200:0.08"]),
    "chevellered": ("import", "fairheaven", ["--cap=30000"]),
    "chevelleblue": ("import", "fairheaven", ["--cap=30000", "--hue=330:360:215:0.9/0:20:215:0.9"]),
    "cop": ("import", "chapman_cop", ["--cap=30000"]),
    "cop_navy": ("import", "negotiator_cop", ["--cap=30000"]),
    "companytruck": ("import", "conquer", ["--cap=30000"]),
    "feduptruck": ("import", "lct3000", ["--cap=30000"]),
    "famtrkstr": ("import", "shvan", ["--cap=30000"]),
    "mx3": ("import", "urban", ["--cap=30000"]),
    "mx3_brown": ("import", "urban", ["--cap=30000", "--hue=10:45:25:0.5"]),
    "sts": ("import", "fairheaven_sw", ["--cap=30000"]),
    "sts_red": ("import", "fairheaven_sw", ["--cap=30000", "--hue=150:220:0:0.9"]),
    "jeep": ("import", "lightbody", ["--cap=30000"]),
    "jeep_green": ("import", "lightbody", ["--cap=30000", "--hue=180:260:110:0.5"]),
    "jeep_grey": ("import", "lightbody", ["--cap=30000", "--hue=180:260:210:0.05"]),
    "firetruck": ("import", "civil", ["--cap=30000", "--pick=fire_truck", "--rot=34.4"]),
    "schoolbus": ("import", "civil", ["--cap=30000", "--pick=school_bus"]),
    "greyhound": ("import", "civil", ["--cap=30000", "--pick=citybus"]),
    "semicab": ("import", "semi", ["--cap=30000"]),
    "dumptruck": ("import", "dump", ["--cap=30000"]),
    "tanker": ("import", "tanker", ["--cap=30000"]),
    "watertruck": ("import", "tanker", ["--cap=30000", "--hue=0:360:0:0.3"]),
    "supercub": ("import", "piper", ["--cap=30000"]),
    "engine": ("import", "diesel", ["--cap=30000"]),
    "coalcar": ("import", "hopper", ["--cap=30000"]),
    "boxcarclosed": ("import", "boxcar", ["--cap=30000"]),
    "caboose": ("import", "caboose", ["--cap=30000"]),
}
SHELLS = ("b1b_02", "euro", "football")
# bikes: folder -> bike_import.py arguments (model name first)
BIKES = {
    "sonnie": ["mx2", "--front=+y", "--weld", "--cap=6000", "--tyre=pasted__rubber_tire1", "--finish=sonnie", "--paint=pasted__plastics1,pasted__radiator_cover1", "--livery=red"],
    "enduro": ["enduro", "--paint=Material", "--livery=blue", "--slot-wheels"],
}
PEOPLE = ("flaggirl", "30secman")   # people.py, from the rider's base body
KIND = {**{n: v[0] for n, v in IMPORTS.items()}, **{n: "arena" for n in SHELLS}, **{n: "person" for n in PEOPLE}}

if __name__ == "__main__":
    sk = sys.argv[1]; names = sys.argv[2:] or list(KIND) + list(BIKES)
    for n in names:
        if n in BIKES:
            src = os.path.join(sk, n, "sonnie.glb" if n == "sonnie" else "enduro.glb")
            subprocess.run([PY, "-I", os.path.join(HERE, "bike_import.py"), src, *BIKES[n]], check=True); continue
        if n in PEOPLE:
            subprocess.run([PY, "-I", os.path.join(HERE, "people.py"), os.path.join(sk, "man", "man.glb"), n], check=True); continue
        if n in SHELLS:
            subprocess.run([PY, "-I", os.path.join(HERE, "arena.py"), n], check=True)
        else:
            kind, folder, extra = IMPORTS[n]
            subprocess.run([PY, "-I", os.path.join(HERE, "fit_import.py"), os.path.join(sk, folder, "model.glb"), n, *extra], check=True)
