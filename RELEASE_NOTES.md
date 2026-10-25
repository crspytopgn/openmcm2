# OpenMCM2 0.1.0

The first public build of OpenMCM2, an open-source Rust engine for
Motocross Madness 2. **You need your own copy of the game**: nothing from the
game is included here.

OpenMCM2 is entirely vibe coded: it was written by Claude Code, an AI coding
agent, directed and play-tested by one fan. Expect rough edges, and please
report anything that plays differently from the original.

Watch the showcase video: https://youtu.be/rP0TEnWLzNo

## Running it

Unpack the archive for your system and start **OpenMCM2**: `OpenMCM2.app`
on a Mac, `mm2-launcher` on Windows and Linux. The first time, choose your
own copy of the game: a CD image (`.iso`, `.bin`/`.cue`, `.img`, `.mdf`,
`.nrg`), the CD itself, or an installed folder. The launcher copies the game
data into its own folder, lets you pick the look, effects, full screen and
window size, and starts the game. Nothing is downloaded.

On macOS the app is not notarised, so the first start may be blocked:
right-click `OpenMCM2.app`, choose Open, and confirm (or run
`xattr -dr com.apple.quarantine OpenMCM2.app`).

You can still start the game directly from a terminal:
`./mm2-viewer "/path/to/Motocross Madness 2"`.

## What works

* The original menus, race display and options, from your game's own files.
* Every event: Supercross, Nationals, Baja and Enduro gate races, Stunt
  Quarry, Moto-Tag, ghost races, replays and the Pro Circuit career.
* The original player bike physics and computer riders, sound, tricks,
  crashes and animations.
* Network races between copies of OpenMCM2 (not with the original game).
* An optional `--enhanced` look, and `--hires` for texture packs you make
  yourself.

## Known differences from the original

* Some jump landings, rough ground and knife-edge object clips still ride
  differently.
* Computer riders crash a little more often and do fewer tricks.

Report anything that plays differently from the original in the issues.

## Licences

OpenMCM2 is MIT or Apache-2.0. The Indeo 5 movie decoder (`mm2-indeo`, a
port of FFmpeg's) is LGPL-2.1-or-later; its full source is in this
repository at this tag.
