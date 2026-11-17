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
    "biplane": ("import", "stearman", ["--cap=40000", "--blot=Image_10:0.049,0.357,0.131,0.44/Image_10:0.533,0.357,0.615,0.44/Image_10:0.049,0.459,0.131,0.542/Image_10:0.533,0.459,0.615,0.542"]),
    "beech18": ("import", "beech18", ["--cap=40000"]),
    "cargoplane01": ("import", "c130", ["--cap=40000", "--blot=Image_1:0.76,0.79,0.99,0.95/Image_1:0.58,0.23,0.67,0.33/Image_5:0.72,0.76,0.99,0.99/Image_5:0.58,0.23,0.68,0.33"]),
    "dc-3": ("import", "dc3", ["--cap=40000", "--blot=Image_0:0.275,0.185,0.552,0.236/Image_0:0.27,0.364,0.54,0.41/Image_0:0.705,0.185,0.815,0.222/Image_0:0.71,0.38,0.818,0.416"]),
    "bellranger": ("import", "alouette", ["--cap=40000", "--blot=Image_25:0.785,0.452,0.86,0.509/Image_25:0.879,0.452,0.955,0.509/Image_25:0.36,0.62,0.403,0.661/Image_25:0.622,0.62,0.665,0.661"]),
    "helinocam": ("import", "alouette", ["--cap=40000", "--blot=Image_25:0.785,0.452,0.86,0.509/Image_25:0.879,0.452,0.955,0.509/Image_25:0.36,0.62,0.403,0.661/Image_25:0.622,0.62,0.665,0.661"]),
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
