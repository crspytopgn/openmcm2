# High-detail bike and rider ("HD" in the bike and outfit lists)

Both start from meshes made with TRELLIS.2 (MIT) from our own reference
sheets. Every logo and livery on them is repainted with generic panels.
The scripts were run with Blender's Python (bpy 5.x). Paths in them point
at the working folders they were made in, so set them before re-running.

1. `trellis_rider.py` fits the rider mesh onto the original 16-node rider
   frame tree. It smooth-skins the mesh into the seated pose (`posemats.py`
   writes `seated_T.json` from the free motions' `Turn2D` frame 0), then
   splits it per node, so the seated rider has no seams.
2. `trellis_bike.py` scales the bike by wheelbase onto the original pivots
   and splits it into the 8 bike frames. The fork lowers go on Shocks and
   the chain runs on SwingArm, so the drawn suspension moves them.
3. `export_hq.py bike|rider MODULE out.glb` writes a framed glb.
4. `paint/` and `bikepaint/` bake each texel's position, node and AO, then
   paint generic panels by rule. They write colour masks (rider: main,
   second, dark; bike: main, second, plus a fixed metal and rubber map).
5. `lod.py` makes the ~100k PC models. `bake_lod.py` gives the PC bike and
   both ~30-40k browser models clean UVs and re-bakes the maps. The PC rider
   keeps its own UVs because they looked cleaner.
6. `make_liveries.py` writes the 12 colourways. `pack_glb.py` swaps the glb's
   material for a placeholder, which the livery or outfit replaces in game.

The pack holds them in `bikes/hq/` and `riders/hq/`, with the browser
versions in `web/`. `tools/stockmap.py` adds the entries.
7. `make_masks.py` writes each set's paint masks (`fp_rider_hq.mask.png` and
   `.base.png`, `fp_livery_hq.mask.png` and `.base.png`). The Kit & Paint
   page mixes any kit, trim, helmet and bike colours from these in game.
   The helmet mask is the head part of the rider's paint (`kit_helmet.png`).
