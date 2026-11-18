# Free bike and rider paint (OpenMX, enduro, free rider)

Gives the free models a fresh texture layout and clean generic paint, with masks so Kit & Paint can colour them.

1. `uv_rebake.py` (Blender's Python): new UVs for the textured faces, with the painted plastics given more room
   (`HOT_K`/`COLD_K`, `WELD=1` for the faceted rider), the old liveries carried across, and position, node,
   normal and AO baked per texel.

       python uv_rebake.py -- in.glb out.glb BAKE/mx2 2048 blue=old_blue.png red=old_red.png white=old_white.png black=old_black.png

2. `design_fp.py BIKE OUTDIR` and `design_rider.py OUTDIR` (with `BAKE` set to the bake folder): paint the panels by
   rules in 3D and write the 12 colourways plus `<set>.mask.png` (R main, G trim, B helmet, A shade) and `<set>.base.png`.

3. `design_mat.py BIKE OUTDIR` writes the bike's metal/roughness and normal maps (rider's come from `design_rider.py`),
   and `attach_maps.py -- in.glb out.glb mr.png nrm.png` puts them on the model's material. The liveries replace only
   the colour, so every colourway keeps the gloss, metal and surface detail (seen with Enhanced lighting).

No logos, brands or numbers. Everything is generated, so it ships with the Free Pack under its usual licence.
