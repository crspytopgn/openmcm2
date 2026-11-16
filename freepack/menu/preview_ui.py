"""Compose build/ui images into rough screen previews (no game needed)."""
import os, sys, tomllib
from PIL import Image, ImageDraw
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mock_main import font, BONE
B = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "build", "ui"); X = 2
def img(dat, path): return Image.open(os.path.join(B, dat, *path.split("\\"))).convert("RGBA")
def put(c, im, r): c.alpha_composite(im.resize((r[2] * X, r[3] * X), Image.LANCZOS), (r[0] * X, r[1] * X))
def main_menu(out, lit="MenuSingle", single_open=True):
    c = img("MAINMENU", r"01_Main\BackGround_01.png")
    L = tomllib.load(open(os.path.join(B, "MAINMENU.toml"), "rb"))["controls"]
    stems = {"MenuSingle": "Single", "MenuMulti": "Multi", "MenuOptions": "MenuOptions", "MenuUserProfile": "Profile", "MenuHighScore": "HighScore",
             "MenuReplay": "Replay", "Menuexit": "Exit", "MenuHelp": "MenuHelp", "MenuCredits": "Credits"}
    for k, s in stems.items(): put(c, img("MAINMENU", "01_Main\\%s%s.png" % (s, "Lit" if k == lit else "Up")), L[k]["rect"])
    if single_open:
        put(c, img("GLOBAL", r"01_Main\SinglePullDown0015.png"), tuple(L['MenuSingleAnimation']['rect']))
        d = ImageDraw.Draw(c); f = font("barlowcondensed/BarlowCondensed-Bold.ttf", 12 * X)
        for i, t in enumerate(["Baja", "Stunt Quarry", "Enduro", "Nationals", "Supercross", "Pro Circuit"]):
            d.text(((196 + 74) * X, (129 + 15 * i) * X), t, font=f, fill=BONE if i else (163, 194, 74))
        put(c, img("MAINMENU", r"01_Main\screenover.png"), (355 + 74, 113, 144, 104))
    c.convert("RGB").save(out, quality=90)
def sp_event(out):
    import subprocess, json
    c = img("SPBASE", r"SinglePlayer\SPBackGround.png")
    for k, x, w in ((1, 120, 100), (2, 220, 101), (3, 321, 99)): put(c, img("SPBASE", "SinglePlayer\\%s%02d.png" % ("TabDown" if k == 1 else "Tab", k)), (x, 0, w, 33))
    for stem, r in (("R2Back", (17, 404, 79, 76)), ("R2Help", (153, 404, 56, 76)), ("R2Joy", (276, 404, 58, 76)), ("R2Option", (391, 404, 87, 76)), ("R2Start", (546, 404, 64, 76))):
        put(c, img("GLOBAL", "Rev2GlobalSource\\%sUp.png" % stem), r)
    ox, oy = 16, 41; e = "SinglePlayer\\1_EventDialog\\"
    def P(name, r): put(c, img("SPEVENT", e + name + ".png"), (r[0] + ox, r[1] + oy, r[2], r[3]))
    P("SPEBack", (0, 0, 610, 355)); P("Screen", (5, 18, 456, 224)); P("Plate04", (72, 296, 315, 52)); P("GhostPanel", (25, 265, 415, 28))
    for y in (108, 176, 244): P("Plate01", (470, y, 130, 57))
    P("DropDown01", (470, 27, 129, 27)); P("DropDown01", (470, 77, 129, 27)); P("EvDropDown04", (27, 244, 411, 21))
    for x, y in ((518, 137), (518, 205), (518, 273), (151, 320), (272, 320)): P("SmalleditBox", (x, y, 33, 21))
    for x, y, n in ((496, 137, "Left"), (553, 137, "Right"), (496, 205, "Left"), (553, 205, "Right"), (496, 273, "Left"), (553, 273, "Right"), (5, 244, "Left"), (440, 244, "Right")):
        put(c, img("GLOBAL", "Resources\\%sArrowUp.png" % n), (x + ox, y + oy, 21, 21))
    d = ImageDraw.Draw(c); f = font("barlowcondensed/BarlowCondensed-Bold.ttf", 11 * X)
    for t, x, y in (("EVENT TYPE", 470, 7), ("RACE MODE", 470, 57), ("OPPONENTS", 473, 114), ("LAPS", 473, 181), ("RACES", 473, 250), ("Nationals", 474, 31), ("Single Race", 474, 81), ("Hillside Nationals", 32, 246), ("7", 527, 139), ("3", 527, 207), ("1", 527, 275)):
        d.text(((x + ox) * X, (y + oy) * X), t, font=f, fill=BONE)
    c.convert("RGB").save(out, quality=90)

if __name__ == "__main__":
    main_menu(sys.argv[1], single_open=False); main_menu(sys.argv[2])
    if len(sys.argv) > 3: sp_event(sys.argv[3])
