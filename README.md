# OpenMCM2

An open-source engine for Motocross Madness 2 (Rainbow Studios, 2000),
rebuilt from scratch in Rust from a study of the original game. It runs
natively on macOS (Metal), Windows (DX12/Vulkan) and Linux (Vulkan).

**Built with AI.** OpenMCM2 is entirely vibe coded: one fan directing
Claude Code (Anthropic's AI coding agent), which did the reverse
engineering, wrote the code and docs, and checked the result against the
original game. Every line was written by AI under human direction and
testing. Bug reports, reviews and human contributions are very welcome.

**This repository contains no game files.** Like OpenMW, the engine reads the
tracks, models, textures, sounds and menus from a copy of the game that you
own. Point it at the folder of your install (or the CD's `GAME` folder).

Project page: https://crspytopgn.github.io/openmcm2/

[![OpenMCM2 showcase video](https://img.youtube.com/vi/rP0TEnWLzNo/maxresdefault.jpg)](https://youtu.be/rP0TEnWLzNo)

Watch the showcase video: https://youtu.be/rP0TEnWLzNo

## Playing

Download a release and start **OpenMCM2** (`OpenMCM2.app` on a Mac,
`mm2-launcher` on Windows and Linux). The first time, point it at your own
copy of Motocross Madness 2: a CD image (`.iso`, `.bin`/`.cue`, `.img`,
`.mdf`, `.nrg`), the CD itself, or the folder it is installed in. It copies
the game's `GAME` folder into the port's user folder
(`~/Library/Application Support/MM2/GAME` on a Mac, `%APPDATA%\MM2\GAME` on
Windows, `~/.local/share/mm2/GAME` on Linux). Then pick the look, effects,
full screen, window size and vsync, and press Play. Nothing is ever
downloaded: OpenMCM2 ships without any game data.

The game can also be started directly with its folder, from a terminal:

```sh
./mm2-viewer "/path/to/Motocross Madness 2"
```

Started with no folder, `mm2-viewer` uses the launcher's copy of the game
(or a game folder it is run from). `mm2-launcher --import <image or folder>`
does the setup without a window. To build it yourself, install Rust from
https://rustup.rs, then:

```sh
cargo build --release -p mm2-viewer -p mm2-launcher
./target/release/mm2-launcher
```

On a Mac, `sh tools/make-mac-app.sh` builds `target/release/OpenMCM2.app`.

It opens the original front end: profiles, the main menu, single player and
its event screens, Bike & Rider, options, the Pro Circuit career, replays and
network races between copies of OpenMCM2. Every event the original has is
in: Supercross and National races, Baja and Enduro gate races, the Stunt
Quarry, Moto-Tag, ghost races and replays. Given a track `.ENV` instead of
the game folder, it starts a race on that track straight away.

**Keys** are the profile's own map (the original's factory keyboard by
default): Up/Down gas and brake, Left/Right steer, W/S lean, Z/X stunts,
Enter Kahuna, Space taunt, LShift Thrill Cam, LAlt Reverse Cam, LCtrl
clutch, Backspace back up. In a race Numpad5 (or C) cycles the camera
views, Home/End zoom, F5 to F10 switch the stats, radar, gauges, sky,
particles and shadows, [ ] or PgUp/PgDn watch the other riders. In network
races `/` opens chat. Alt+Enter (Option+Return on a Mac) switches full
screen. A gamepad works through the profile's gamepad map.

**Optional extras**, off unless asked for:

* `--enhanced` draws a gently remastered look (sun shadows, HDR, ambient
  occlusion, detailed ground, dust, widescreen race display). F3 switches
  between it and the original look while racing; the riding is the same
  either way. See [docs/enhanced-graphics.md](docs/enhanced-graphics.md).
* `--hires <pack>` uses a high-resolution texture pack that you make
  yourself from your own copy of the game. See
  [docs/hires-textures.md](docs/hires-textures.md). Packs are never shared
  here.

On Linux, building needs ALSA and libudev headers
(`sudo apt install libasound2-dev libudev-dev pkg-config`); build with
`--no-default-features` to leave gamepad support out.

## How close it is

The player's bike runs the game's full player model from
`crates/mm2-physics` (rigid body, two tyres on shocks, engine and gears,
crash rules, tricks), stepped once per frame as the original does. The
computer riders run the game's own AI model and controller. The riding is
checked against recordings of the original game: most comparison runs match
it (crash outcome, speed and pitch), but some jump landings, rough ground and
knife-edge object clips still differ, and the computer riders crash a little
more and do fewer tricks than the original's. The notes are in
[docs/physics.md](docs/physics.md).

## Formats

| Format | Used by | Status |
| --- | --- | --- |
| `RS2` archive | `RES/ENGINE.RES`, `RES/*.TEX`, `UI/*.DAT` | Reads and extracts |
| `FAOE` container | other `RES/*.RES`, `TERAFORM/**/*.ENV` | Decodes (RS2 behind a byte cipher keyed by file name) |
| LZW streams | inside `.trn` and other files | Decodes |
| `.trn` terrain | each track `.ENV` | Heightmap, normals, textures and texture tiles |
| `.scn` scene | each track `.ENV` | Start position, lights, fog, models, sounds |
| `.slb` model | `RES/MODELS.RES`, `RES/BIKE.RES` | Frames, materials, LODs, meshes |
| model textures | `RES/OBJECTS.RES`, `RES/*.TEX` | RGB555 (magenta = transparent) and ARGB4444 with mips |
| `.mcf` / `.mot` | `RES/OBJECTS.RES`, `RES/BIKE.RES` | Animated models and their motions |
| `.mbf` rider morphs | `RES/BIKE.RES` | Corrective shapes for elbows, hips and so on |
| `.col` collision | `RES/COLSEG.RES` | Solid objects |
| `.tdf` track definition | each track `.ENV` | Course, start and finish lines, gates |
| `.pb` presets | `UI/UIRES.RES` | Bikes, riders, presets |
| `.dtm` UI | `UI/*.DAT` | The original menus |
| procedural `.vue` paths | `RES/PROCVUE.RES` | Trains and other traffic that follow a path |
| Indeo 5 video | `UI/*.AVI` | The intro and credits movies |

## Tools

```sh
cargo run -p mm2-tools -- info  /path/to/GAME/RES/*.RES
cargo run -p mm2-tools -- list  /path/to/GAME/RES/ENGINE.RES
cargo run -p mm2-tools -- extract /path/to/GAME/RES/BIKE.RES extracted/bike
cargo run -p mm2-tools -- terrain /path/to/GAME/TERAFORM/SX/SX01.ENV sx01-terrain
cargo run -p mm2-tools -- model extracted/bike/Yamaha.slb
```

## Tests

`cargo test` runs without any game files. To also check every archive,
track, model and texture of your install, set `MM2_DATA` to the game folder:

```sh
MM2_DATA="/path/to/Motocross Madness 2" cargo test
```

## Credits

* The Indeo 5 decoder in `crates/mm2-indeo` is a port of
  [FFmpeg](https://ffmpeg.org)'s, by Maxim Poliakovski, and keeps FFmpeg's
  licence (below).
* Ground materials for `--enhanced` are from [ambientCG](https://ambientcg.com),
  CC0.

## Legal

OpenMCM2 is an independent fan project. It is not affiliated with, endorsed by
or sponsored by Microsoft or Rainbow Studios. Motocross Madness is a trademark
of Microsoft Corporation, and the game, its data and artwork are © Microsoft.
This repository contains no game files and no code from the original game: it
is a new implementation, and you need your own legitimately obtained copy of
Motocross Madness 2 to use it. Do not commit game files, extracted assets or
texture packs made from them.

## Licence

Licensed under either of the [Apache License, Version 2.0](LICENSE-APACHE) or
the [MIT licence](LICENSE-MIT), at your option, except `crates/mm2-indeo`,
which is under the GNU LGPL 2.1 or later
([COPYING.LGPLv2.1](crates/mm2-indeo/COPYING.LGPLv2.1)). The full source of
every release is here, so release builds can always be rebuilt with a changed
copy of that crate. Unless you state otherwise, any contribution you submit is
dual licensed as above.
