# OpenMCM2 Free Pack (work in progress)

New art so OpenMCM2 can be played without a copy of Motocross Madness 2,
and so player-made tracks that borrow stock objects and skies load without
the original files. Everything here is made for OpenMCM2 or comes from CC0
sources (see [CREDITS.md](CREDITS.md)). Nothing is copied or traced from MM2.

The pack is built from scripts, so this folder stays small:

1. `python3 tools/fetch.py` downloads the CC0 sources listed in `sources.toml` into `cache/`.
2. `python3 tools/make_skies.py` makes the 14 stock sky names (cube faces, `build/skies`).
3. `python3 tools/make_brands.py` draws banners for made-up sponsors (`build/brands`).
4. `python blender/objects.py` builds a stand-in for every stock object name
   (`build/objects/<name>.glb`), each fitted to the stock object's size from
   `stock_sizes.tsv`, so a track's layout still works.
   `python blender/gates.py` then remakes the race gates over the top: the
   checkpoint arches (`waypnt`, `fp_waypnt_2` .. `fp_waypnt_4`, which free tracks
   take in turn) and the `finish` arch.
5. `python3 tools/credits.py` writes CREDITS.md.

Steps 4 uses Blender as a Python module (`pip install bpy==5.3.0 pillow numpy`).
Steps 2 needs `numpy` and `opencv-python-headless`.

Coordinates: models are in game units (feet). glTF (x, y, z) = game (x, y, -z),
Y up, the same flip the engine's `from_game` makes.

Look: realistic but soft. Natural daylight, low contrast, matte surfaces.
