#!/usr/bin/env python3
"""Marks the controls whose Free Pack picture already shows its label
(`text = false` in ui/<DIALOG>.toml), so the game doesn't write its own
text over it: the page tabs along the top of a screen, whose pictures
have the tab's name on them, and the panel titles listed in TITLES.

Usage: no_text.py PACK_DIR   (reads PACK_DIR/menus/*.dtm, edits PACK_DIR/ui/*.toml)
"""
import os, re, sys, glob

# Titles drawn into a panel's picture: dialog -> controls.
TITLES = {"MAINMENU": ["rbwLabel1"]}
# Screens whose tab pictures carry the tab's name (the others' are plain).
NAMED_TABS = {"SPBASE", "MPBASE"}

def controls(dtm):
    """(name, props) for each control of a .dtm."""
    out, name, props = [], None, {}
    for line in open(dtm, encoding="latin-1"):
        line = line.strip()
        m = re.match(r"\[(.+)\]$", line)
        if m:
            if name: out.append((name, props))
            name, props = m.group(1).strip(), {}
            if name.lower() in ("set_control", "set_information", "set_anim"): name = None
        elif "=" in line and name:
            k, v = line.split("=", 1)
            props[k.strip().lower()] = v.split("'")[0].strip().strip('"')
    if name: out.append((name, props))
    return out

def main(pack):
    for dtm in sorted(glob.glob(os.path.join(pack, "menus", "*.dtm"))):
        stem = os.path.basename(dtm)[:-4]
        if not os.path.isdir(os.path.join(pack, "ui", stem)): continue
        names = list(TITLES.get(stem, []))
        for name, p in controls(dtm):
            xywh = [v.strip() for v in p.get("xywh", "").split(",")]
            if stem in NAMED_TABS and p.get("type", "").upper() == "RADIOBUTTON" and len(xywh) == 4 and xywh[1] == "0":
                names.append(name)
        if not names: continue
        toml = os.path.join(pack, "ui", stem + ".toml")
        text = open(toml).read() if os.path.exists(toml) else "# Free Pack layout for %s\n" % stem
        for n in names:
            head = "[controls.%s]" % n
            if head in text:
                block = text.split(head, 1)[1].split("\n[", 1)[0]
                if "text = false" in block: continue
                text = text.replace(head, head + "\ntext = false  # the picture shows the label", 1)
            else:
                text += "\n%s\ntext = false  # the picture shows the label\n" % head
        open(toml, "w").write(text)
        print(stem, ", ".join(names))

if __name__ == "__main__":
    main(sys.argv[1])
