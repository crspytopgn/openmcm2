# OpenMCM2 0.1.3

## New in 0.1.3

* **Smoother crashes:** falling off now blends between the rider's motion
  poses instead of snapping, the rider lets go of the bike partway through
  the bail and falls from there, and the body follows the pelvis as it
  tumbles, closer to how the original plays it. This is new and not yet
  well play-tested, so reports of odd-looking falls are very welcome.

## New in 0.1.2

Fixes from playtesting:

* **Custom tracks load:** community tracks (such as MDX Racing's) with other
  texture formats, short compressed terrain cells, or models packed inside
  the track or named with folder paths now open.
* **Tyres turn** with the bike's speed, as in the original.
* **No more falls in corners:** the over-the-bars check no longer throws
  riders who are leaned over in a turn.
* **Off-track arrow** only shows when you really leave the course, and a
  reset rider is put back facing the right way.

## New in 0.1.1

* **Controllers:** Xbox and PlayStation pads and flight sticks, working the
  way the original's joystick did (steer, lean, gas, brake, stunts, pad
  triggers), with every button bindable in Options > Controls and a flight
  stick's throttle lever for the gas.
* **Menus with a pad:** the stick or D-pad moves the pointer, A clicks and B
  goes back.
* **Your controller on screen:** Options > Controls names the pad in use and
  shows its own buttons (A/B/LB/RT on Xbox, Cross/Circle/L1/R2 on
  PlayStation). The stick can be bound to steer or lean.
* **Several controllers at once:** they're read together, and the one last
  touched rides.
* Enhanced look: wood, metal, concrete and straw detail on the Farm
  track's objects.

## About OpenMCM2

OpenMCM2 is an open-source Rust engine for Motocross Madness 2. **You need your own copy of the game**: nothing from the
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
