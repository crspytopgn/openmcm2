#!/usr/bin/env python3
"""Write CREDITS.md from sources.toml: every third-party source with its
link, author and licence."""
import os, tomllib, json, urllib.request
HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
o = urllib.request.build_opener(); o.addheaders = [("User-Agent", "openmcm2-freepack/1.0")]
def ph_info(a):
    try: return json.load(o.open(f"https://api.polyhaven.com/info/{a}"))
    except Exception: return {}
src = tomllib.load(open(os.path.join(HERE, "sources.toml"), "rb"))
L = ["# Free Pack credits", "",
     "Everything in the Free Pack is either made for OpenMCM2 (CC-BY 4.0, credit \"OpenMCM2 contributors\")",
     "or comes from the CC0 sources below. CC0 needs no credit, but we give it anyway. Thank you!", "",
     "## Skies (Poly Haven, CC0)", "", "| Stock name | HDRI | Authors |", "| --- | --- | --- |"]
for name, a in src["sky"].items():
    i = ph_info(a); au = ", ".join(i.get("authors", {}).keys()) or "Poly Haven"
    L.append(f"| {name} | [{i.get('name', a)}](https://polyhaven.com/a/{a}) | {au} |")
L += ["", "## Materials (ambientCG, CC0)", "", "| Material | Link |", "| --- | --- |"]
for m in src["materials"]["ambientcg"]:
    L.append(f"| {m} | [ambientcg.com/view?id={m}](https://ambientcg.com/view?id={m}) |")
L += ["", "## Models (Poly Haven, CC0)", "", "| Model | Authors |", "| --- | --- |"]
for a in src["models"]["polyhaven"]:
    i = ph_info(a); au = ", ".join(i.get("authors", {}).keys()) or "Poly Haven"
    L.append(f"| [{i.get('name', a)}](https://polyhaven.com/a/{a}) | {au} |")
open(os.path.join(HERE, "CREDITS.md"), "w").write("\n".join(L) + "\n")
print("wrote CREDITS.md")
