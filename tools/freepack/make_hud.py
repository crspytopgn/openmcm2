#!/usr/bin/env python3
"""The Free Pack's race display pictures, drawn from scratch.

Without the game's RES/GLOBAL.RES the race display fell back to plain
modern boxes. These are the pack's own pictures in the original's
layout: a see-through bevelled panel hung from a bolted steel bracket,
the dial's marks where the game's needle points, the radar's rings
where the game draws its course. Same size and places as the game's
128x128 pictures, drawn at 4 texels a pixel.

    python3 make_hud.py OUT_DIR      -> OUT_DIR/hud/{instr1,bajascore,radar128}.png
"""
import math
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont

K = 4                      # texels per game pixel
N = 128 * K
FONT = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"

# Pixel centres in game pixels.
_ys, _xs = np.mgrid[0:N, 0:N]
X = (_xs + 0.5) / K
Y = (_ys + 0.5) / K


def cover(d):
    """Coverage of a shape from its signed distance (game px, <0 inside)."""
    return np.clip(0.5 - d * K, 0.0, 1.0)


def rounded_rect(x0, y0, x1, y1, r_tl, r_tr, r_br, r_bl):
    """Signed distance to a rectangle with its own radius at each corner."""
    cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
    hw, hh = (x1 - x0) / 2, (y1 - y0) / 2
    r = np.where(X < cx, np.where(Y < cy, r_tl, r_bl), np.where(Y < cy, r_tr, r_br))
    qx = np.abs(X - cx) - hw + r
    qy = np.abs(Y - cy) - hh + r
    return np.hypot(np.maximum(qx, 0), np.maximum(qy, 0)) + np.minimum(np.maximum(qx, qy), 0) - r


class Canvas:
    def __init__(self):
        self.rgb = np.zeros((N, N, 3))
        self.a = np.zeros((N, N))

    def over(self, colour, alpha):
        """Paints `colour` (array or rgb tuple, 0-1) with coverage `alpha`."""
        alpha = np.clip(alpha, 0, 1)
        colour = np.broadcast_to(np.asarray(colour, float), (N, N, 3)) if np.ndim(colour) < 3 else colour
        out_a = alpha + self.a * (1 - alpha)
        safe = np.where(out_a > 0, out_a, 1)
        self.rgb = (colour * alpha[..., None] + self.rgb * (self.a * (1 - alpha))[..., None]) / safe[..., None]
        self.a = out_a

    def save(self, path):
        img = np.dstack([self.rgb, self.a])
        Image.fromarray((np.clip(img, 0, 1) * 255 + 0.5).astype(np.uint8), "RGBA").save(path, optimize=True)


def panel(c, d, opacity=0.56):
    """A see-through grey panel of shape `d`, raised: a light upper-left
    rim and a dark lower-right one, faintly lighter at the top."""
    inside = cover(d)
    shade = 0.20 - 0.08 * (Y / 128.0)
    body = np.dstack([shade * 0.96, shade, shade * 0.97])
    c.over(body, inside * opacity)
    # Bevel: the light falls from the top left, so the rim's light where
    # the shape's edge faces it. The edge's facing from the distance's slope.
    gy, gx = np.gradient(d)
    n = np.hypot(gx, gy) + 1e-9
    facing = (-gx - gy) / (n * math.sqrt(2))    # +1 facing up-left
    rim = np.clip(1 - np.abs(d + 1.6) / 1.4, 0, 1) * inside
    light = np.clip(facing, 0, 1)
    dark = np.clip(-facing, 0, 1)
    c.over((0.78, 0.80, 0.80), rim * (0.25 + 0.55 * light))
    c.over((0.02, 0.02, 0.02), rim * 0.55 * dark)
    # A thin dark outline so it reads on bright sky.
    c.over((0.05, 0.05, 0.05), np.clip(1 - np.abs(d) / 0.6, 0, 1) * 0.6)


def bracket(c, x0, x1, y, bolts):
    """A steel bar from x0 to x1 centred on y, held by hex-socket bolts."""
    h = 3.0
    d = rounded_rect(x0, y - h / 2, x1, y + h / 2, 1.2, 1.2, 1.2, 1.2)
    # Shadow under it.
    sd = rounded_rect(x0 + 0.8, y - h / 2 + 1.2, x1 + 0.8, y + h / 2 + 1.2, 1.2, 1.2, 1.2, 1.2)
    c.over((0, 0, 0), cover(sd) * 0.35)
    t = np.clip((Y - (y - h / 2)) / h, 0, 1)
    steel = 0.80 - 0.45 * t + 0.10 * np.sin(t * math.pi * 2.2)
    grain = 0.03 * np.sin(X * 3.1 + np.sin(X * 0.7) * 2.0)
    v = np.clip(steel + grain, 0, 1)
    c.over(np.dstack([v * 0.92, v * 0.95, v * 1.0]), cover(d))
    for bx in bolts:
        bolt(c, bx, y)


def bolt(c, x, y, r=4.2):
    sd = np.hypot(X - x - 0.7, Y - y - 0.9) - r
    c.over((0, 0, 0), cover(sd) * 0.4)
    d = np.hypot(X - x, Y - y) - r
    # Domed head: light top left, dark bottom right.
    u = np.clip(((X - x) + (Y - y)) / (2 * r), -1, 1)
    v = 0.72 - 0.35 * u
    c.over(np.dstack([v * 0.90, v * 0.93, v * 1.0]), cover(d))
    ring = np.clip(1 - np.abs(d + 0.6) / 0.6, 0, 1)
    c.over((0.12, 0.13, 0.16), ring * 0.8)
    # Hex socket.
    ang = np.arctan2(Y - y, X - x)
    hexr = 1.7 / np.cos(((ang + math.pi / 6) % (math.pi / 3)) - math.pi / 6)
    hd = np.hypot(X - x, Y - y) - hexr
    c.over((0.10, 0.11, 0.13), cover(hd) * 0.9)


def rule(c, x0, x1, y, alpha=0.55):
    d = np.maximum(np.abs(Y - y) - 0.35, np.maximum(x0 - X, X - x1))
    c.over((0.82, 0.84, 0.82), cover(d) * alpha)


def ring(c, cx, cy, r, alpha=0.6, w=0.35):
    d = np.abs(np.hypot(X - cx, Y - cy) - r) - w
    c.over((0.82, 0.84, 0.82), cover(d) * alpha)


def text(c, s, cx, cy, pt, colour=(0.86, 0.87, 0.85)):
    """Bold figures centred on (cx, cy), with a dark edge under them."""
    font = ImageFont.truetype(FONT, int(pt * K))
    layer = Image.new("L", (N, N))
    edge = Image.new("L", (N, N))
    dl, de = ImageDraw.Draw(layer), ImageDraw.Draw(edge)
    dl.text((cx * K, cy * K), s, font=font, fill=255, anchor="mm")
    de.text((cx * K + K * 0.7, cy * K + K * 0.7), s, font=font, fill=255, anchor="mm",
            stroke_width=int(K * 0.6))
    c.over((0.03, 0.03, 0.03), np.asarray(edge) / 255.0 * 0.85)
    c.over(colour, np.asarray(layer) / 255.0)


def score_panel():
    """bajascore: the race panel at the top left; its lines' boxes are
    ruled off where the game writes them (hud.rs LINES)."""
    c = Canvas()
    d = rounded_rect(1, 4, 124, 124, 6, 6, 30, 6)
    panel(c, d)
    for y in (33.0, 49.5, 83.0, 101.0):
        rule(c, 10, 118, y)
    bracket(c, 6, 120, 6.5, (12, 114))
    return c


def radar_panel():
    """radar128: the radar at the top right, its rim 55 px round the
    centre (60, 64), a ring half way and the cross, with the player's arrow."""
    c = Canvas()
    d = rounded_rect(1, 4, 124, 124, 6, 6, 30, 6)
    panel(c, d)
    cx, cy = 60.0, 64.0
    # A slightly darker disc inside the rim, as a scope's glass.
    c.over((0.0, 0.03, 0.01), cover(np.hypot(X - cx, Y - cy) - 55) * 0.18)
    ring(c, cx, cy, 55)
    ring(c, cx, cy, 27.5, 0.45)
    inside = np.hypot(X - cx, Y - cy) < 55
    for dd in (np.abs(X - cx), np.abs(Y - cy)):
        c.over((0.82, 0.84, 0.82), cover(dd - 0.3) * inside * 0.45)
    # The red arrow, pointing up.
    w, h = 5.0, 6.0
    top, base = cy - h / 2, cy + h / 2
    t = np.clip((Y - top) / h, 0, 1)
    ad = np.maximum(np.abs(X - cx) - t * w / 2, np.maximum(top - Y, Y - base))
    c.over((0.95, 0.08, 0.05), cover(ad))
    bracket(c, 6, 120, 6.5, (12, 114))
    return c


def dial_angle(mph):
    v = min(max(mph, 0.0), 100.0)
    deg = 230.0 + 130.0 * v / 50.0 if v < 50.0 else 130.0 * (v - 50.0) / 50.0
    return math.radians(deg)


def speedo():
    """instr1: the speedometer at the bottom right. The needle turns about
    (64, 85), 0 mph at 230 degrees round from the top to 100 at 130
    (hud.rs dial_angle); marks each 5 mph out to 52, numbers each 10."""
    c = Canvas()
    cx, cy = 64.0, 85.0
    R = 60.0
    arch = np.maximum(np.hypot(X - cx, Y - cy) - R, np.maximum(Y - 125.0, -1e9))
    # Flat sides below the centre.
    arch = np.where(Y > cy, np.maximum(np.abs(X - cx) - R, Y - 125.0), arch)
    # The face's keyhole: a round boss under the hub opening to the bottom.
    hole = np.minimum(np.hypot(X - cx, Y - cy) - 17.0,
                      np.where(Y > cy, np.abs(X - cx) - (11.0 + (Y - cy) * 0.55), 1e9))
    hole = np.maximum(hole, cy - 30 - Y)
    panel(c, arch, 0.60)
    # See-through, as the face is cut away there.
    c.a = c.a * (1 - 0.6 * cover(hole) * cover(arch - 2.5))
    for mark in range(0, 101, 5):
        a = dial_angle(mark)
        r0 = 42.5 if mark % 10 == 0 else 47.5
        x0, y0 = cx + math.sin(a) * r0, cy - math.cos(a) * r0
        x1, y1 = cx + math.sin(a) * 52.0, cy - math.cos(a) * 52.0
        # Distance to the segment.
        px, py = X - x0, Y - y0
        vx, vy = x1 - x0, y1 - y0
        t = np.clip((px * vx + py * vy) / (vx * vx + vy * vy), 0, 1)
        dd = np.hypot(px - t * vx, py - t * vy) - (0.75 if mark % 10 == 0 else 0.5)
        c.over((0.85, 0.86, 0.84), cover(dd) * 0.9)
    for mark in range(0, 101, 10):
        a = dial_angle(mark)
        s = str(mark)
        r = 32.5 if mark in (0, 100) else 34.0
        text(c, s, cx + math.sin(a) * r, cy - math.cos(a) * r, 9.5 if mark < 100 else 8.5)
    bracket(c, 6, 122, 122.5, (12, 116))
    return c


def main():
    out = Path(sys.argv[1] if len(sys.argv) > 1 else ".") / "hud"
    out.mkdir(parents=True, exist_ok=True)
    for name, make in (("bajascore", score_panel), ("radar128", radar_panel), ("instr1", speedo)):
        make().save(out / f"{name}.png")
        print(out / f"{name}.png")


if __name__ == "__main__":
    main()
