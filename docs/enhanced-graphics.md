# Enhanced graphics

`mm2-viewer` draws the game exactly as the original does unless it is
started with `--enhanced`. The upgrades below are the port's own, not the
game's. They change only how the game is drawn: the physics, the camera, the
clock and the game's settings are the same either way. Without `--enhanced`
the drawing is pixel for pixel what it was before these upgrades existed.

```text
./target/release/mm2-viewer ../GAME --hires ../textures-hires --enhanced
./target/release/mm2-viewer ../GAME --enhanced=no-bloom,no-ao   # all but these
./target/release/mm2-viewer ../GAME --enhanced --fullscreen --no-vsync
./target/release/mm2-viewer ../GAME --enhanced=time=dusk,weather=rain
./target/release/mm2-viewer ../GAME --enhanced=quality=medium      # a slower GPU
```

While racing with `--enhanced`, **F3** switches between the enhanced look and
the original one, so the two can be compared on the spot. **Alt+Enter**
(Option+Return on a Mac) switches full screen, with or without `--enhanced`.
**F2** (or `--show-fps`) shows the frame rate in the window's title and the
terminal, once a second.

## Speed

| Setting | What it does |
|---|---|
| `quality=low` | No anti-aliasing, AO, contact shadows, puddle reflections, light shafts, smooth hills, grass or replay lens, and plain (not soft) shadow edges; the 3D scene at most 1280×720. |
| `quality=medium` | 2× anti-aliasing (4× where the GPU has no 2×), the plain AO, no puddle reflections, light shafts or smooth hills; the scene at most 1920×1080. |
| `quality=high` | The default: everything, the scene at most 2560×1440. |
| `quality=ultra` | 8× anti-aliasing where the GPU has it, the scene at the screen's own size. |
| `resolution=1080p`, `resolution=1600x900`, `resolution=native` | The most pixels the 3D scene is drawn with. On a bigger screen (Retina, 4K) it is drawn smaller and stretched over it, with the menus and race display still drawn sharp at the screen's own size. Later items win, so `quality=medium,resolution=native` works. |
| `quality=auto` | Picks Low, Medium or High for the graphics card and screen when the game starts, and turns on `dynamic`. |
| `dynamic` | Dynamic resolution while racing: if frames start coming late (behind the screen's beat with VSync, else 60 fps or the frame cap), the 3D scene is drawn in steps down to 60% of its size each way, and back up after a while of keeping time. A step up that comes in late is undone within half a second, and the next try waits twice as long. Menus and the race display stay sharp. `no-dynamic` turns it off under Auto. |

Whatever the setting, only what can be seen is drawn: the ground, objects,
crowd and smooth hills are cut into blocks, and blocks outside the camera's
view (or outside each sun shadow's) are skipped. That holds without
`--enhanced` too, with the picture unchanged pixel for pixel.

Since 2026-10-24 the defaults keep close to the original's look (the 2026-11 additions, from living trees to floodlights, are on by default and kept just as gentle): the
scene's own sky box, no colour grade, no crowd, props or ruts, and the
ground's photo detail laid on gently.

| Upgrade | Turn off with | What it does |
|---|---|---|
| Anti-aliasing | `no-msaa` (`msaa=2`/`8` to change) | 4× MSAA on everything in 3D, with alpha to coverage, so leaves and fences fade at their edges instead of stair-stepping. |
| Lighting | `no-lighting` | Objects are lit per pixel instead of per vertex, with bumps taken from the textures' brightness (no tangents needed), a soft highlight from the sun, and brighter ambient light from the sky than from the ground. The terrain's light is painted into its textures, so it keeps that and gains only the small bumps. |
| Sun shadows | `no-shadows` | Every object casts a shadow from the scene's sun, in three cascades of 2048² out to 520 ft, filtered 3×3. In a stadium (a scene with no sky box) only the moving models (bikes, riders, people) cast, since the roof would shade the whole floor. Replaces the game's projected player shadow. Anything within 0.4 ft of the ground casts nothing on it, so painted ruts and planks don't streak it. |
| Draw distance | `no-distance` | The game's fog ends the world at 256 to 768 ft (by the Visibility slider). Instead the view goes on to 7 times the fog's span, fading into a haze that is half as thick as the fog at the fog's end. |
| HDR and bloom | `no-bloom` | The scene is drawn in 16-bit float colour and tone mapped with Khronos PBR Neutral, which leaves colours under about 0.8 as they were. Anything brighter than white (the sun's glow, glints) blooms. |
| Ambient occlusion | `no-ao` | Screen-space AO (the Alchemy estimate) at half size, 3 ft reach, blurred over 4×4 pixels: contact darkening under bikes, in ruts and where objects meet the ground. |
| Sky glow | `no-sky` | A glow round the sun, where the scene's light comes from, bright enough to bloom, and haze along the horizon that meets the land's haze. Also needed by the real sky. |
| Grade | off; `grade` turns it on | A bolder look: richer colour and a gentle S-curve of contrast after the tone mapping, and stronger AO. |
| New ground | `no-ground`; `detail=0` to `1` (default 0.5) for how strongly and how far its grain shows | Photo materials (CC0, ambientCG: dirt, packed dirt, grass, rock, sand; `assets/ground`) laid over the painted terrain by what each part is, blended by height, rock seen from three sides on cliffs. Half of the painted-in light is taken out and the ground is lit by the sun as it really falls, with the materials' bumps and a sheen. |
| Smooth hills | `no-smooth` | Within about 220 ft of the eye the ground is drawn cut 4 times finer, with heights on a smooth curve through the game's samples, so hills, crests and jump faces lose their flat facets. The drawn ground stays within 0.35 ft (about 4 inches) of the game's triangles and blends back into them at the patch's edge. Only drawn: the bikes ride the game's ground. |
| Trackside props | off; `scenery` turns them on | Outdoors, the game's own props set out along the course: rows of hay bales (oil drums in the desert) set back from the edge, sponsor banners and camera towers further out, and a paddock of parked trucks with chairs, picnic tables, a pit trailer and an outhouse beside the start. Each stands on free, fairly level ground, at least 7 ft (bales) to 30 ft (paddock) off the course and clear of the trees. Only drawn: nothing is solid. |
| Crowd | (with `scenery`) | 3D spectators, each with its own shirt, trousers, skin, hair and height. Up to 9000 stand in a stadium's front seven rows, on the stands' own surface just outside the wall. Outdoors, groups stand behind the hay bales. They shift about while they wait, and jump and wave their arms when a bike comes within about 90 ft. Only drawn. |
| Object materials | `no-surfaces` | The grain of CC0 ambientCG photo materials (wood, metal, concrete, straw) on every track's buildings, fences, bridges, poles, sheds, hangars, trailers, machines and vehicles. Each object texture is given the material it is mostly made of by name (`materials::surface`, picked by eye from the game's textures; people, bikes, signs, banners, plants and textures mixing many materials are left alone). Only the material's light and dark and its bumps are laid on, from three sides on the model at rest, so the game's own paint and colours stay; it fades out from 30 to 120 ft. |
| Living trees | `no-trees` | Trees and bushes sway in the wind, bending from the foot (the top of a 30 ft tree moves about half a foot, a little more in the rain), their leaves fluttering, in time with the grass's gusts. Seen from the shade side, toward the sun, the leaves glow a little where it shines through them. Drawn only: their shadows and the trunks the bikes hit stay where they are. |
| Soft shadows | `no-soft` | The sun's shadows soften the further they fall from what casts them, as the sun's disc blurs a real one (percentage-closer soft shadows): sharp where a tyre meets the ground, softer under a rider or a tree's crown. |
| Track wear | `no-wear` | Where the tyres run, lap after lap, the ground packs a little darker and smoother: a soft band a couple of feet wide with no grooves, building up with each bike that passes (up to 20% darker). Uses the ruts' map; `ruts` draws the old grooves instead. Only drawn. |
| Round bikes and riders | `no-round` | The bikes' and riders' facets (tyres, tanks, helmets, arms) are shaded smooth, from normals averaged with their neighbours' that face within about 65°, so they read rounder; sharper edges stay sharp. The outlines are the game's models. |
| Floodlights | `no-floodlights` | In a stadium (a scene with no sky box), soft light from four banks of lamps high in the corners, with their glints off the bikes' paint and plastic and a little off the packed dirt, on top of the scene's own light. |
| Better ambient occlusion | `no-gtao` (on with High and Ultra) | The ambient occlusion worked out from the horizons round each pixel (GTAO, Jimenez 2016) in place of the Alchemy estimate: across three slices of the view, how high the surfaces within 3 ft rise each way, and so how much of the sky's light a surface facing its normal still takes. Deeper in creases (forks, fenders, between a rider's arm and body, round props' feet), with open ground left as it was: most of the sky open counts as open, and it is softened by half. Needs `ao`. |
| Contact shadows | `no-contact` | From each pixel a short ray toward the sun, 0.8 to 2.5 ft, across the screen's depth: anything just in front of it shades it. The fine shadow where a tyre, a boot or a prop meets the ground, which the shadow maps are too coarse to draw. Half as dark as a full shadow, by day only, fading out by 90 ft. |
| Puddle reflections | `no-ssr` (on with High and Ultra, never in a browser) | In the rain, the puddles and the wet ground (marked by the ground in the scene's alpha) mirror what is on screen: the ray off the water is followed across the screen's depth to what it meets, at half size, as strongly as the water's angle reflects, fading where what it would show leaves the screen. Off when ray tracing draws the reflections. |
| Sky light | `no-skylight` | When a track loads, each terrain sample looks eight ways over the heights (out to 400 ft) for how high the land rises above its own slope there; the ambient light on the ground, the grass and the objects standing on it is dimmed by how much of the sky that leaves (squared, so open ground hardly changes): darker in dips, at the foot of banks and between hills. What stands higher above the ground sees more of the sky. |
| Realistic materials and reflections | `no-pbr` | Physically based shading, as modern engines do, for the free pack's glTF models: each material's metal and roughness, its normal map (its frame worked out from the screen, so the models need no tangents), ambient occlusion and glow pictures, lit by the sun with GGX and Smith's visibility, and by the sky all round. The sky's light is a small cube map baked on the CPU when a track loads and when its light changes (the scene's own sky box, or the real sky's colours, with a sunlit ground below), blurred down its levels for rougher surfaces, with its irradiance as nine spherical harmonics, so it costs nothing per frame and works in a browser and on old OpenGL. A white matte surface square on to the sun still comes out as the game's own light would draw it. Highlights on fine bumps and far models are widened so they don't shimmer, and the free pack's crossed-plane trees take light round their leaves instead of splitting into a lit and a dark half. The game's own models only take the sky's light in place of their even ambient, half way: a little bluer from above, warmer from the ground. |
| Grass | `no-grass` | 3D tufts on a grid round the camera (to 160 ft, sinking into the ground from 90 ft) wherever the painted ground is green, swaying in the wind, lit and shadowed like the ground. Not in ruts. Only drawn: the bikes ride the game's ground. |
| Real sky | off (the scene's own sky box shows); `clouds` turns it on, and `time=` or `weather=` turn it on unless `no-clouds` | Outdoors, a sky drawn from the sun's place in place of the sky box: bluer overhead, sun disc and glow, clouds 3000 ft up that drift and are lit from the sun's side. The sky box's distant hills stay along the horizon, and the haze takes the sky's colour. |
| Time of day | `time=dawn` (7:00), `morning`, `noon`, `afternoon`, `dusk` (18:00, golden hour) or an hour 0-24 | Moves the sun from the east through the scene's own noon direction to the west. Low sun is redder and weaker, the ambient bluer. Without it the scene's own sun is kept. |
| Weather | `weather=clear` (default), `overcast`, `rain` | Overcast greys the sky and covers it with cloud, dims the sun and lifts the ambient. Rain adds that plus falling streaks, darker wet ground and puddles in hollows and ruts that reflect the sky. Desert (Baja) tracks get overcast instead of rain. |
| Ruts | off; `ruts` turns them on | Every tyre on the ground draws its line into an 8192² map over the terrain. The ground shows the grooves, darker, and they last the whole session. Only drawn: the ground the physics rides is the game's. |
| Dust clouds | `no-dust` | Rear tyres throw up soft, lumpy puffs the warm colour of the ground, only where the game's own surfaces raise dust. A spinning tyre or a bike pulling hard throws up more, and landing a jump throws out a ring. The puffs are lit lump by lump, hang low, drift on the wind and thin away over 2 to 4 s, in place of the game's own tyre dust trail (the game's dirt chunks still fly). In the rain they throw up spray instead: streaks of muddy water in a rooster tail that fall back, with wisps of mist. |
| Bikes and riders | `no-bikes` | The moving models are drawn smooth (almost no bump from their textures) and their brighter, purer colours gloss a little, with a sharp sun highlight and the sky caught at a glancing angle. |
| Mud | `no-mud`, `mud=0.5` (how dirty the bikes start, 0 to 1) | Dirt cakes onto each bike and rider as it rides, by distance and speed: slowly in the dry, fast in the rain. Clumpy splats on the model itself (they stay put as it moves), thickest on the wheels, boots and underside, specks higher up; pale and dusty when dry, dark and shiny when wet. Mud dulls the paint's gloss. |
| Light shafts | `no-shafts` | Outdoors, marching toward the sun on the screen through the open sky, so hills, trees and riders in front of a low sun cut shafts into its glow. Weaker under cloud. |
| Replay lens | off; `cinematic` turns it on | In replays only: depth of field focused on the rider the camera follows, and motion blur from how the camera swings between frames. Racing is never blurred. |
| Texture filtering | (with `--enhanced`) | 16× anisotropic filtering instead of 8×. |

High refresh needs nothing: the bike already steps once a frame, by the
frame's length, as the original does above 50 fps. `--no-vsync` draws
without waiting for the screen.

## How it is built

- `graphics.rs`: the settings, `--enhanced` parsing and the F3 switch.
- `enhance.rs`: the shadow cascades (fitted to slices of the view, moved
  only in whole texels so their edges don't crawl) and the settings the
  shaders share (`enhanced.wgsl`, `enhanced_fns.wgsl`, put into each shader
  where it says `#include`).
- `post.rs`, `post.wgsl`: the HDR and depth targets, AO, bloom and tone
  mapping.
- `materials.rs`: the ground's photo materials, as texture arrays with mips.
- `ibl.rs`: the sky's light all round (the cube map and its harmonics),
  read by `irradiance`, `sky_reflected` and `shade_pbr` in
  `enhanced_fns.wgsl`. `mm2_formats::glb` reads each glTF material's
  metallic-roughness look into `slb::Pbr`, and `objects.rs` binds its
  pictures beside the colour (a livery keeps its model's other pictures).
- `grass.rs`/`.wgsl`, `sky_real.wgsl`, `ruts.rs`/`.wgsl`, `dust.rs`/`.wgsl`,
  `rain.rs`/`.wgsl`: the grass, the real sky, the rut map, the dust clouds
  and spray, and the rain. `smooth.rs` draws the smooth ground round the
  eye (`vs_smooth` in `terrain.wgsl`), and the game's mesh under it is cut
  away. `scenery.rs` places the props, hung on a node of their own so F3
  can hide them. `crowd.rs`/`.wgsl` draw the spectators, one small box
  figure instanced, its arms swung in the vertex shader. The mud is drawn in `objects.wgsl`, from how
  dirty each bike is (`TerrainRenderer::tyres`), carried in a spare corner
  of its models' node matrices. `enhance::daylight` works out the sun, ambient and sky
  colours for the hour and weather asked for.
- None of it touches the physics: a `--ride` gives byte-identical player and
  computer CSVs with and without `--enhanced`.
- `terrain.wgsl`, `objects.wgsl`, `sky.wgsl` each have an `fs_enhanced`
  beside the game's own `fs_main`, which is untouched. The enhanced
  pipelines are made only with `--enhanced`; F3 draws with the game's own
  pipelines into the enhanced targets and skips the tone mapping.
