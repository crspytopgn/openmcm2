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
if __name__ == "__main__":
    main_menu(sys.argv[1], single_open=False); main_menu(sys.argv[2])
