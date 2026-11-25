# OpenMCM2 0.8.3

* **The same ride at any frame rate.** At 30 fps a hard turn could end
  differently than at 60 or 144 fps, because the game read your keys
  once a frame. Now it reads them at every physics step, so the same
  ride gives the same result at 30, 60, 144 or 240 fps.
* **Computer riders get a moment too.** After the start and after a
  crash, computer riders don't bump into you, other bikes or objects for
  3 seconds, like the original.
* **Share your Course Builder tracks** with other players. Only the
  track's recipe is uploaded; your own models and pictures stay on your
  computer.
* **Replay editor** (Extras, off by default): a free camera, slow
  motion, loops, camera keys, and clips you can save as movies.
* **Gyro steering** on the Steam Deck, Steam Controller, PlayStation
  and Switch pads (off until you turn it up).
* **Dirt that builds up** on bikes and riders, lap by lap (Enhanced,
  off by default).
* **New Enhanced looks**: colour looks, a Next-gen preset, and grass
  that bikes push aside (all off by default).
* **Ko-fi**: you can now support OpenMCM2 on Ko-fi as well as GitHub
  Sponsors.

# OpenMCM2 0.8.2

More of the original's riding rules, found by checking the physics
against the original's code line by line:

* **A moment to get going.** After a crash you can't crash again or
  bump into other bikes for 3 seconds, like the original.
* **Your rider hits things too.** Low branches, signs and overhangs now
  hit the rider, not just the bike, as in the original. Once you're down,
  only the bike collides.
* **Cross-ups count.** Landing still crossed up or mid-trick now judges
  the bike's angle as drawn, like the original.
* **Settled starts.** At the start and after a crash, the bike sits on
  the slope the way the original places it.
* **Crashed bikes** swing their bars as they slide, like the original.

# OpenMCM2 0.8.1

* **Turns ride like the original again.** 0.8.0 could throw you off in
  ordinary turns, mostly when the rider put a foot down or leaned hard.
  The new crash check now only crashes you when your upper body hits the
  ground, the same as the original. Thanks to darkranger-red for the
  report (#199).

# OpenMCM2 0.8.0: The Big Physics Update

**It rides like MM2 again.** Grip, suspension, jumps, landings and
crashes now feel just like the original, at any frame rate.

* **Crashes like you remember.** Riders tumble like the original,
  sliding bikes kick up dust, and in Baja and Enduro you get back up
  facing the next gate.
* **Cameras and computer riders like the original.** The chase cam,
  Rider Cam and crash view are back the way they were, and the computer
  riders race like the original's.
* **Runs faster.** Smoother on every PC, and in the browser on laptops
  and Macs.
* **Build tracks for the real MM2.** Course Builder tracks now export to
  the original Motocross Madness 2.
* **New audio for the free bikes.** Engine sounds (125, 250, 400)
  recorded by crspytopgn, CC BY 4.0.
* **Also new:** bounce light and FSR upscaling in Enhanced (off by
  default), and a simpler play page.

Big thanks to Andrew Boudreau, whose Motocross Madness 2 decompilation
helped make this possible.

# OpenMCM2 0.7.8

* **Free tracks that feel ridden-in.** Smooth between the jumps, with
  berms that build into each turn, natural worn ruts along the racing
  line, rolling hills under the Nationals, a jump-filled rhythm like the
  originals on Nationals and Supercross, and far fewer banners, flags and
  bales. Rebuilt demo tracks get the ruts and berms too. Your free tracks
  rebuild once the first time you ride them.
* **A free rider who sits on the bike like the original.** Knees tucked
  in, elbows up, smooth joints working again, and a slimmer build with
  square shoulders.
* **Free bikes ride exactly like their MM2 class.** Each free bike now
  shares its class's handling and collision shape, and the free 500 and
  600 use the 250 and 400 engine sounds.
* **Our own engine recordings.** The Free Pack's 125, 250 and 400
  engines are new recordings. Engine sounds (125, 250, 400) recorded by
  crspytopgn, CC BY 4.0.
* **More colour in the free scenery.** The Free Pack's ground, skies,
  plants and objects are about 20% more colourful, closer to the
  original tracks.
* **The crowd stays in the stadium.** Free outdoor tracks no longer have
  a stadium crowd. As in the original, the crowd is only in Supercross
  stadiums and Moto-Tag arenas.
* **Smoother Rider and Helmet Cams.** No more judder when steering hard,
  and the Helmet Cam now moves with the bike as one, like the original.
* **Event clip in its frame.** With the new look menus, the main menu's
  event clip plays inside its frame again.
* **Lobby chat wraps.** Long chat lines in the multiplayer lobby wrap
  inside the box instead of running past it.

# OpenMCM2 0.7.7

* **Fresher original engine sounds.** With your own game or the demo,
  the bikes' original engine samples are now freshened as they load: a
  touch of EQ, a little more bite and level, and seamless loops. It is on
  by default. Switch it off with *Freshened engine sounds* in the
  launcher or *Original engine sounds, untouched* in the browser's
  extras. Nothing is changed on disk and no sounds are shipped. With the
  free graphics, your game's or the demo's own engines play when they
  are there.
* **A much smaller Free Pack sound download.** The Free Pack's sounds
  are now Ogg Vorbis instead of WAV. That's about 3 MB instead of 20 MB,
  and every clip keeps its exact length, so loops stay seamless. Update
  OpenMCM2 before updating the Free Pack: older versions can't play the
  new sounds, so they ask you to update first.

# OpenMCM2 0.7.6

* **More tracks for demo and Free Pack players.** Without the full game,
  the launcher's *Play* page and the browser page now suggest the
  *starter set*: ten of the most raced tracks made by players, about
  55 MB. Press *Get the starter set*, or *No thanks* to hide the
  suggestion.
* **Player-made tracks first.** Our own free tracks are no longer in the
  single race and multiplayer track lists. Switch on *Our own free
  tracks* in the extras to bring them back. An event with no other track
  still lists them, and the Pro Circuit, replays and races hosted on one
  of them work as before.
* **Remove what you downloaded.** The launcher can now remove the
  downloaded Free Pack, the new look menus and the demo, and asks once
  more before it does. In the browser the buttons say *Remove the Free
  Pack* and *Remove the demo*.
* **Free roam on Voodoo Basin.** Both always-open free roam rooms now
  run the demo's Voodoo Basin, so everyone with the game or the free demo
  can ride. Without either, the game says to get the free demo.
* **The demo rides the original rider.** A profile saved while playing
  with the free graphics no longer puts the free rider and bike into the
  demo or the original game. The free graphics keep their own picks.
* **A free rider who sits more like the original.** Less hunched over
  the bars, knees bent as in the original, and elbows up.
* **Smoother races in the browser on big screens.** Races are drawn at
  a size the graphics can keep up with, smaller while frames come late
  and sharp again once they keep time. This helps most on laptops with
  built-in graphics and high-resolution screens. Menus stay sharp.

# OpenMCM2 0.7.5

* **Players' own tracks in one click.** The launcher's *Tracks* page and
  the browser's *Your own tracks* box have a new *Most popular tracks*
  list. It holds 77 tracks made by players on MDX Racing: the Nationals
  and Supercross tracks raced most, going by the lap times posted there,
  and the Baja and Enduro maps. Start with the *starter set* (10 tracks,
  about 55 MB) or get them all. Rest the mouse on a name to see the
  track's picture. Each track belongs to the player who made it and is
  credited by name. They work with either graphics, also without the
  original game.
* **Custom Baja and Enduro maps work.** Maps with only gates, and no
  course file, now show up in the track list and can be raced.
* **Original bikes and sounds everywhere.** With the original game or the
  free demo installed, the free tracks now use the original bikes and
  engine sounds too, unless you pick free graphics.
* **Clearer free Baja tracks.** Far fewer cactuses and trees, and open
  ground round the gates, as in the original.
* **Faster, clearer track loading in the browser.** A loading bar shows
  each step, a free track builds about a third faster the first time,
  and a track that can't load says so instead of dropping back to the
  menus.
* **Playtime in the browser.** The play page shows your time riding and
  everyone's, like the launcher.

# OpenMCM2 0.7.4

* **The free rider sits on the bike like the original rider.** With the
  original game installed, the free rider used the original rider's
  poses. Those were made for the original's narrower bike, so his knees
  and shins went in through the free bikes' side panels. He now always
  uses his own poses: the original's moves and timing, with his feet on
  the free bike's pegs and his knees outside the bike.
* **A cleaner free rider kit.** The jagged black patches at the waist and
  under the arms are gone. The kit also gets a proper race number on the
  back, a plain goggle strap and deeper folds in the suit. This comes
  with a Free Pack update, which the launcher and browser fetch for you.

# OpenMCM2 0.7.3

* **Typing on phones and tablets.** In the browser version, tapping the
  game while a box wants a name now opens the phone's keyboard. That
  covers a new rider profile, chat, the race number and a track's name.
  A bar along the top shows what you're typing, with a *Done* button, as
  the keyboard can cover the box. On an iPhone the first start's
  *Create New Profile* box could not be filled in before, which left the
  menus behind it stuck.
* **The browser version starts sooner,** most of all on phones. The game
  no longer waits for the whole Free Pack to be read in first. The
  scenery and ground load while you're in the menus, and a race picked
  before they're ready waits on its loading screen.

# OpenMCM2 0.7.2

* **Smoother rebuilt tracks.** The grey walls on some rebuilt tracks,
  such as Farm and Ski Lodge, are gone. The track and the land beside it
  now follow the same ground, so the banks slope away smoothly.
* **Riders get round every rebuilt Enduro and Baja track.** Routes bend
  round buildings instead of running into them, go over culverts and
  under bridges as in the original, and computer riders no longer get
  stuck.
* **Round hay bales.** The free Farm and enduro hay bales are now big
  round bales lying on their side, like the original's. Supercross bales
  stay square. This comes with a Free Pack update, which the launcher and
  browser fetch for you.

# OpenMCM2 0.7.1

* **The demo's other tracks, rebuilt** (Extras, off by default, only with
  the free MM2 demo). The demo lists 27 more tracks that it won't let you
  ride. It keeps their route, scenery and menu pictures, but not their
  ground. Turn on *Rebuilt demo tracks* and they join each event's list,
  marked "(Rebuilt)". Each one is rebuilt the first time you ride it,
  which takes a few seconds: the original route and scenery on new
  ground, with jumps of our own. The Stunt Quarry and Tag arenas have no
  route in the demo, so they aren't included.
* **Scenery sits on the ground** on the rebuilt tracks. Hay bales no
  longer sink into the floor, and billboards, light poles, trailers and
  signs no longer float above it. Stacked crates and bales, power and
  ski-lift wires, and the rails over the trestle stay with whatever holds
  them up.

# OpenMCM2 0.7.0

* **No game? Start with the free MM2 demo.** The launcher's *Get the free
  MM2 demo* and the browser's *Play the free MM2 demo* fetch Microsoft's
  free 2000 demo from the Internet Archive when you ask. Its two tracks,
  Voodoo Basin and Donner Pass, play with the original graphics, and the
  Free Pack fills in every other event, bike and screen.
* **A Tools page.** Look inside the game in any browser: browse every
  file with previews (save models as .glb, pictures as .png, sounds as
  .wav), turn the bikes and riders through all their animations, fly
  round any track, and paint your own bike or rider skin in 3D. Nothing
  is uploaded.
* **Phones and tablets.** The browser version has touch controls: a
  thumb stick to steer and lean, gas and brake you can slide between,
  stunts, clutch and the camera. A light mode keeps phones smooth. Add
  the page to your Home Screen to play full screen, and once played it
  starts without the internet too. If something goes wrong, a button
  copies a bug report for you to paste into an issue, and a "Not
  loading?" box gives tips if a step takes too long.
* **Photo mode** (Extras, off by default). Press F in a race, or click
  both sticks, to freeze it with the race display hidden, fly the camera
  anywhere and press Enter to save a picture.
* **Race the leaderboard ghosts** (Extras, off by default). Your best
  laps go up with their ghost, and Replays > Saved Ghost Race Files
  offers the fastest online lap on each track to race against.
* **Pad vibration in the browser**, following your Force Feedback setting.
* **Free mode feels more like the original.** The free tracks now start
  with the 30-second board man and the original's 8.2 second countdown,
  every lap race has the finish arch and flag girl, and the race display
  uses bolted panels in the original's style. Ten new free tracks join
  the Free Pack (sandy coast, big autumn hills, dunes, moorland and new
  stadiums), Baja is open country, and tracks are wider with gentler
  jumps. Free bikes take off and land like the original's, and the free
  rider stands up through the turns and bends smoothly at the joints.
* **Free 125 engine:** no more rider's voice, and a real low idle.
* **Out-of-bounds slide whistle.** Ride off the edge of a free track and
  a cartoon whistle plays as you're thrown back in. It has an off switch.
* **Wrecks tumble like the original** after hitting walls, bales and
  trees, and each bike now always rides with its own class's power.
* **Smoother computer riders** when the frame rate drops below 36.
* **Steadier frame rate.** Every preset now draws a touch softer only
  while frames come late, and Ultra uses 4x anti-aliasing on screens
  bigger than 1440p, which runs far better on a 4K TV.
* **Flying dirt, rebuilt.** The roost is now a tight jet of grit, crumbs
  and clods from the back tyre. It's off by default for now, with its
  switch in Enhanced.

The Free Pack updates by itself in the launcher and the browser.

# OpenMCM2 0.6.11

* **Free Pack downloads that don't give up.** The browser version and
  the launcher now keep going when the connection drops, stalls or the
  download server is busy: each file is tried again after a short wait,
  and a half-finished part carries on from where it stopped. If it
  still can't finish, everything that came down is kept, and the
  message says which file failed and why (no space left, missing on
  the server, or the connection).

# OpenMCM2 0.6.10

* **A new free rider.** Playing with free graphics, you get a whole new
  rider in a fun 2000s factory kit: jagged shards, checker strips,
  halftone dots and a big 00 on the back, with a vented MX helmet,
  goggles, gloves and boots. All 12 colours and Kit & Paint work as
  before. This comes with a Free Pack update the launcher fetches by
  itself.
* **Rides like a racer.** The free rider now sits down into the seat
  and leans over the bars, further when standing up, and both hands
  close properly round the grips on the Ridgeback and the Kestrel.
* **Stunt scores agree in network races.** Every player's scoreboard
  now shows the same stunt points (the game sends each jump's score, as
  the original did).

# OpenMCM2 0.6.9

* **Lighter, nicer dust.** Enhanced dust clouds are about six times
  cheaper to draw and now follow your graphics preset, so Low and Medium
  (and the Steam Deck) go easy on them. Dry tracks get a thin haze that
  drifts on the wind.
* **Flying dirt.** Pull hard, spin the back tyre or slide through a turn
  and the bike throws real clods of dirt, mud in the rain. It's on in
  Enhanced, with its own switch to turn it off.
* **New paint for the free bikes.** The Ridgeback and the Kestrel get
  full graphics kits: a jagged ridge band and white plates on the
  Ridgeback, a sweeping band with flame licks on the Kestrel, much
  sharper than before. All 12 colours and Kit & Paint work as before.
  This comes with a Free Pack update the launcher fetches by itself.

# OpenMCM2 0.6.8

* **New look menus without the Free Pack.** If you play with MM2
  graphics but like the new look menus, the launcher now fetches just
  the menus instead of the whole pack, and keeps them up to date.
* **A Ready button you can read.** In the new look lobby your button
  says NOT READY or READY, and everyone else shows READY or WAITING.
* **Ride free tracks together.** Players with MM2 graphics can now join
  free roam and free track races with Free Pack players. A race never
  starts on a different track from the host's any more; the lobby says
  why instead. Quiet corners of free roam stop revving phantom engines.
* **Flowing free tracks.** Free nationals and supercross tracks swap
  their square grid corners for turns of every angle, and nationals get
  big berms, flat turns and the odd off-camber corner. Free tracks
  rebuild themselves the first time you ride them.
* **Proper race gates on free tracks.** Inflatable arches, a truss
  gantry and an inflatable finish line.
* **Your own bike sounds right in free mode.** The four-strokes play
  their recorded engine clips instead of one stretched loop.
* **Max graphics** for strong graphics cards (desktop only): Ultra drawn
  finer, with supersampling and grass, hills and shadows reaching
  further. Both parts have their own switches in the launcher.
* **Launcher:** an Exit button you can reach with a pad, and a widescreen
  window that goes full screen in Steam Deck Game Mode.
* **Free Pack downloads fixed.** The launcher no longer turns down pack
  parts with "doesn't match its checksum".
* **Course Builder:** a stick resting slightly off centre no longer
  spins the view (seen on Steam Deck).
* **Drive the scenery:** planes no longer dip their nose when you bank
  with a pad stick.
* **Browser:** clicking through bikes in the menus no longer freezes the
  picture in Safari, and the start page explains how to wake a game pad
  (press any button) and shows when it's found.

# OpenMCM2 0.6.7

* **Drive the scenery** (Extras, off by default). Get in the tracks'
  cars, trucks, trains, planes and helicopters. Each one has its own
  handling and engine sound, and the props and rotors spin. Free tracks
  and free roam get a lot of parked vehicles and aircraft to try. Press
  V (or down on the D-pad) next to one to get in or out.
* **Two free bikes.** The Free Pack now has the Ridgeback four-stroke
  and the new Kestrel two-stroke in every class, and the pack is about
  125 MB smaller. The credits list both bikes.

# OpenMCM2 0.6.6

* **Bigger mountains in free mode.** Outdoor free tracks now sit in a
  ring of tall mountains instead of low hills, and the land between the
  course and the mountains is smoother, with fewer lumps and steps. The
  free tracks rebuild themselves the first time you ride them.
* **Fixed a crash after long races.** Each new race and lap time made a
  text picture that was never thrown away, so memory slowly filled up
  (#80). Smaller leaks in the Course Builder pictures, the menu movies
  and sounds on computers with no sound device are fixed too.

# OpenMCM2 0.6.5

* **New free bikes.** Two bikes for 125, 250 and 500, the Ridgeback and
  the high-detail Ironbark, and the Drover four-stroke for 400F and
  600F. Each has a fresh paint layout, real metal and rubber materials
  and light wear, and works with Kit & Paint. The HD rider is now the
  one free rider. The browser and Low graphics load its lighter version.
* **Free tracks look less flat.** Nationals get broken dirt lips that
  build into berms, a worn groove and rollers. Baja and Enduro get rolls,
  dips and gullies off the trail. There are sponsor fences on the start
  straights, feather flags, an inflatable sponsor arch, orange checkpoint
  arches, blue tuff blocks in Supercross and a red "you" marker like the
  original's. The free tracks rebuild themselves the first time you ride
  them.
* **Computer riders:** the free Supercross start gate no longer pens
  everyone in, a rider who crashes on a free Enduro gets up where they
  fell, and the 8th rider on Nationals 1 now finds the course.
* **Menus:** scroll bars can be dragged, the wheel and trackpad scroll
  smoothly, the wrench on the Bike & Rider page opens the Garage, the
  8-bit/16-bit buttons work, and every Help button opens a new help page.
  The browser also asks to keep your saves, so they aren't cleared.

# OpenMCM2 0.6.4

* **Kit & Paint.** A new garage page lets you mix your own colours for
  the HD rider's kit and helmet and for the HD bike. Pick a ready-made
  colourway again to go back to it.

# OpenMCM2 0.6.3

The free graphics take a big step towards the original's look and feel:
the rider moves like the original, the land round the tracks looks like
a real course, and there's a new high-detail bike and rider.

* **The free rider rides like the original.** The free rider and bike
  now use all of the original's motions, the suspension moves, the rider
  puts the inside leg out in turns, and their boots and knees no longer
  poke through the bikes.
* **High-detail bike and rider** in the Free Pack's garage: an HD bike
  in 125, 250 and 500 and an HD rider, 12 colours each. The browser loads
  lighter copies of them under the same names, so online races still
  match.
* **Free tracks sit in real land.** Valleys and hillsides climb away
  from the track, hills ring the edge so the world doesn't end in a flat
  line, and the track ground has ruts along the racing line, braking
  bumps, damp patches and roost thrown onto the grass. The ground is
  shaded as it faces the sun, and the fog takes each sky's own colour.
* **Race-day dressing.** Nationals get sponsor banners, speaker and
  light poles, a TV platform, team trucks behind the pits and a blimp.
  Supercross gets banners round the stadium floor, and its start gate
  drops at the start.
* **Gentler jumps.** The free tracks' jump faces and landings are now as
  steep as the original game's, not steeper, so you and the computer
  riders crash less.
* **New Free Pack art:** lighter dirt clods, saguaro cacti with arms,
  leafy ocotillo, a real green off-course arrow, a clean "you" marker and
  six race-day banner sponsors. The free tracks rebuild themselves the
  first time you ride them.
* **Course Builder: Dress the scenery.** One button grows tall grass,
  bushes and trees round the track you've drawn. Press it again after
  changing the line and it regrows round the new one. Your own planting
  stays.
* **Freshen engine sounds** (Extras, off by default): if you have the
  game, its own engine samples are played back cleaner and fuller, at
  about the same loudness. The free bikes' sounds come from recordings
  on Freesound (see the credits).
* **Free tracks show their pictures** in the menus before they're
  built.

# OpenMCM2 0.6.2

The free graphics are now the whole game, with no MM2 files needed, and
the menus have a new look you can pick even if you own the game.

* **Full menus with the free graphics.** Without MM2 files you get every
  menu, working as in the original: single races on the free tracks, the
  garage with the free bikes and outfits, a Pro Circuit career on the
  free tracks, high scores, replays, options and the credits. The
  wording and button sounds are our own.
* **New look or original menus.** The launcher's Play page and the
  browser page now ask which menus you want, beside the graphics. The
  new look has every screen and button of the original, redrawn. It
  works with MM2 graphics too: in the browser only the menus' part of the
  Free Pack downloads. Original menus stays the default when you have
  the game, and needs its files.
* **Track pictures.** Tracks you build, and the free tracks, now carry
  their own preview picture, the same way the game's tracks do, so you
  can tell them apart on the event screen. A track with no picture gets
  one drawn from its ground.
* **Smoother and quicker:** the sky and the ground draw less work for
  the same picture, the sky-light bake is several times faster, and Auto
  graphics sets the costliest effects aside for a while if frames keep
  coming late, then gives them back.
* **Free Pack ground:** the fine ground detail no longer shows as big
  squares.
* **Browser:** Play no longer opens the Course Builder after you've used
  its button once, and time played with the free graphics is counted
  too, including the last minutes before you close the tab.

# OpenMCM2 0.6.1

The Free Pack gets much smaller, sounds much better and now plays a
proper race. The Course Builder grows plants, rivers and aircraft, and
there are free roam rooms that are always open.

* **The Free Pack is now about 590 MB instead of 1.9 GB.** Its 28 tracks
  aren't downloaded any more. Each one is a recipe built into the game,
  and the first time you ride or copy one, your computer builds it from
  the recipe and the pack's shared ground textures in a few seconds
  (about 10 to 20). When you play with free graphics they're in each
  event's track list, and in the Course Builder under *Edit a copy*.
* **Real sounds for the Free Pack.** The engines are now made from real
  public-domain recordings (a two-stroke and a four-stroke), cut into
  single engine firings and rebuilt for each class at several rev levels.
  The game blends the two nearest your revs, so nothing sounds sped up or
  slowed down. Falls, crashes, landings, the crowd and applause are real
  recordings too. All of it is CC0, from BigSoundBank (thank you, Joseph
  Sardin).
* **Free graphics fixes:** the rider now sits on the bike, and without
  the original game nothing goes missing in a race: HUD labels, rider
  names and messages, the speedo, radar and score panel, dirt roost and
  ground detail are all there, and each bike class rides with its own
  engine instead of every bike riding as a 250. The free bikes and the
  pack's bales and walls are solid, so you no longer ride through them.
* **Living courses:** the Course Builder can now plant trees and
  bushes, run rivers and streams through your track (ride through them
  for a splash), and send aircraft flying overhead.
* **Always-open free roam:** two free roam rooms that are always open,
  one in Europe and one in the US, on Copper Ridge. Pick one from the
  online races list and ride with whoever is there. No one needs to host.
  Other riders' bikes now move more smoothly too.
* **Better light in Enhanced:** realistic materials and sky reflections,
  film-like colour (with free graphics) and sunlit haze, and sharper
  shadows on Ultra. Rivers reflect the sky. The original look is
  unchanged.
* **Course Builder:** renaming a track now shows what you type, and a new
  **Race** button (also *Kind of race* on the Land panel) turns a track
  into Supercross, National, Baja or Enduro, fitting its line, jumps and
  objects to the new land.
* **Browser:** the Free Pack download now shows a clear progress bar with
  megabytes and percent, and a plain message with a retry button if it
  fails.

# OpenMCM2 0.6.0

A big one. You can now build your own tracks, and play without a copy of
the original game.

## New in 0.6.0

* **Course Builder:** make your own tracks right in the game. It's on the
  main menu (and on the launcher's Play page). Draw the line, add jumps,
  whoops and rhythm sections, shape and paint the ground with sand or mud,
  place bales, tents and trailers, then press **Ride it** and race it.
  *Surprise me* makes a whole track in one click, and *Edit a copy* starts
  from any track you already have without changing the original. It works
  with a mouse, a game pad or a Steam Deck, and in the browser too. Don't
  want it on the menu? Turn it off in the launcher's Extras.
* **Free graphics (the Free Pack):** a whole new set of art made for
  OpenMCM2: bikes and riders, a supercross stadium, 28 new tracks across
  every event (playable from 0.6.1), skies, trees, vehicles and aircraft. It's all our own work or openly licensed, so it's free
  for anyone. The launcher and the browser page now ask **MM2 graphics or
  free graphics?** and download the pack for you. Updates fetch only the
  parts that changed. Early days: without MM2 files, the free graphics
  open in the Course Builder for now. Full races and menus with the free
  art are on the way. Sound: with MM2 installed you hear the original
  game's sounds. Without it, the pack has a temporary set made in code (an
  engine for each class that follows your revs, plus landings, crashes and
  the crowd). Real engine recordings for each class replace them in a pack
  update soon, and only those files download.
* **Night races under stadium lights:** pick Night as the time of day and
  supercross stadiums are lit by their own lamp banks, with soft shadows
  from each bank (Enhanced switch "Stadium lights at night").
* **Faster and smaller:** the downloads shrank (game 45 to 30 MB, launcher
  21 to 14 MB, browser 13.7 to 11.8 MB). Auto graphics now adjusts the 3D
  resolution while you race to hold a smooth frame rate, Low uses simpler
  shadow edges, and tracks with lots of ground textures now load in
  browsers.
* **Clear loading screens in the browser:** the page shows each step as it
  loads, with a progress bar, and if something goes wrong it says what and
  what to try, in plain words.
* **New look:** a new number-plate logo, app icon and a fresh style for the
  site and launcher.
* **Fixes:** keys no longer stay held when the game loses focus, time spent
  down after a crash no longer counts against lap leaderboard times, a
  small dead centre stops stick drift leaning the rider, smoother crash
  falls, roomier speedo numbers, clearer picture extras, and the Controls
  page shows your pad's own button names.

## New in 0.5.5

* **Steam Deck and Steam Machine:** a new single-file Linux AppImage,
  with its icon and menu entry built in. Make it executable, open it, and
  add it to Steam as a non-Steam game. The release also has
  `openmcm2-steam-artwork.zip` with a cover, banner, logo and icon for
  Steam's library. Set the launch option to `--play` to skip the launcher.
  See the Steam Deck guide in `docs/steam-deck.md`.
* **Every action on your controller:** game pads now have buttons for
  everything: LB looks back, RB is the clutch, the stick clicks are Kahuna
  and Thrill Cam, and the D-pad's left and down are taunt and back up. In
  a race, D-pad up changes the camera, D-pad right is the Helmet Cam,
  View gets you back on the bike and Menu leaves the race. Anything you've
  bound yourself stays as it is.
* **No more leaning on its own:** a stick or trigger that isn't centred
  when the game starts, or a controller you haven't touched, no longer
  steers or leans the rider.
* **Menus with a controller:** in the game's menus the D-pad jumps from
  button to button, and the launcher can be used with a pad too.
* **Stick steering curve:** a new setting in Extras. The original game
  makes a stick's steering gentle near the middle and steep at the edge,
  which can feel all-or-nothing on a short stick. Turn it down for
  steering that follows the stick more evenly. 100% is the original.
* **Update note in the game:** if you start the game without the
  launcher, the main menu now tells you when a new version is out.
* **Easier online races:** Start in the lobby now readies the host and
  starts the race. The Host or Join list groups online races, free roam
  and your own network, and Internet opens that list straight away.
* **A livelier Helmet Cam:** it looks further up the track with a wider
  view, leans with the bike, looks into turns, dips on landings, and no
  longer shows your own rider (your shadow stays). The Rider Cam hides
  your rider too.

## New in 0.5.4

* **Desktop and browser race together:** desktop players and players
  in the browser version can now join the same network races and free
  roam sessions.
* **No port forwarding to host:** a race you host on desktop gets a race
  code, shown in the lobby chat. Players anywhere can join from the
  server browser, or by typing the code in TCP/IP. Joining by address
  and on your own network still work as before, and races hosted on
  0.5.3 still show in the list.
* **Free roam servers:** host a Practice session in Multiplayer and it
  becomes a free roam server, with no laps and no time limit. Anyone can
  drop in by pressing Ready in the lobby and leave whenever they like.
  It shows in the server browser as "Free roam".
* **Random checkpoints in Baja:** the Random Gate # box works as it does
  in the original. A race number from 0 to 999 gives 10 to 30 gates,
  with the trees cleared round each gate. These races don't save
  records.
* **Rider Cam from the rider's eyes:** the view sits at eye height, and
  your own rider sign is hidden in first person. Helmet Cam is now
  always there and is no longer an extra.
* **Launcher links:** links in the launcher open your web browser again,
  and the Donate card works.

## New in 0.5.3

* **Server browser:** Multiplayer's Host or Join list now shows open
  races from everywhere, under the ones on your own network, with their
  track and how many riders are in. Pick one and press Join. It works on
  desktop and in the browser (desktop and browser races are listed
  apart, as they can't race each other yet).
* **Your hosted races are listed for you:** a race you host goes in the
  list while it's open. On desktop it only shows if other players can
  reach your computer; if they can't, the lobby chat tells you to
  forward TCP port 28220 on your router. To keep your races to people
  you invite, turn off "List my races in the server browser" (launcher,
  Extras, Network races) or tick "Keep my races out of the server
  browser" in the browser's settings. The launcher's Network races card
  also shows the open races.
* **Lobby downloads on by default:** in a network race, the host's track
  and other riders' own paints and outfits come over if you don't have
  them. Turn it off in the same Network races card, or with "No lobby
  downloads" in the browser.
* **Baja and Enduro on the leaderboards:** their laps never made it on
  before; now they do.
* **Baja checkpoint arrow and radar:** the arrow points at the gate
  itself, and the radar shows gates and riders as the original does.
* **Helmet Cam:** F1 now switches it on and off in a race (fn+F1 on a
  Mac keyboard), and the view sits at the rider's goggles, with the bars
  and the track in view.
* **Custom Engine Size:** unlocked as in the original when you pick one
  of the game's own .tga paints (such as BIKE01), in single races,
  multiplayer and the Pro Circuit.
* **Screen and window size in the launcher:** pick full screen (your
  screen's own resolution or another) or a window and its size. A new
  Auto graphics preset picks Low, Medium or High for your computer.
* **Smoother at 4K:** less work for the graphics card each frame, frame
  times evened out to the screen, and the computer riders move smoothly
  between their steps. The picture is the same as before.
* **Support the project:** the launcher's Updates page has a small
  Donate on GitHub card.

## New in 0.5.2

* **Leaderboards in the game:** High Scores now has an **Online** button
  under the events. It lists the tracks with laps on the OpenMCM2
  leaderboard for Baja, Nationals, Supercross and Enduro, and each
  track's best laps from every rider, with their bike class.
* **On by default, on desktop and in the browser:** after each race the
  game itself sends your new best laps (under your rider name) and your
  playtime (with no name) to the OpenMCM2 server. To keep them to
  yourself, turn off "Lap leaderboards" in the launcher's Extras, or
  "Add my time to everyone's" on its Play page. In the browser, tick
  "Keep my laps off the leaderboards" or "Don't add my time to
  everyone's" in the settings.
* **Easier on the browser:** the browser version's Enhanced look draws
  the scene at no more than 1080p, with a lighter anti-aliasing and
  without the light shafts, replay blur and ray tracing. It starts on
  Medium quality, and if a race keeps running slowly it offers Low
  quality or the original look.
* **Calmer Enhanced look:** HDR bloom and the glossy bikes and riders
  now start off, on desktop and in the browser. Both are still there as
  effects to turn on.
* **No shadow from the signs:** the "You" and "IT" signs over the
  riders no longer cast a shadow.
* **Four new Enhanced effects, kept soft:** sky light (dips, the foot
  of banks and gullies are a little dimmer, as under a real sky),
  contact shadows (a thin shadow where tyres, boots and props meet the
  ground), better ambient occlusion (soft shading in creases) and
  reflections in puddles in the rain. Each one can be turned off. In the
  browser only sky light and contact shadows run: the other two are too
  heavy there.
* **Every option explains itself:** point at any effect or setting in
  the launcher or on the browser page to see what it does.

## New in 0.5.1

* **Shared lap leaderboards:** OpenMCM2 now has its own leaderboard
  server, built in. With the Lap leaderboards extra on, your best laps
  go on it under your rider name, and the launcher shows everyone's best
  laps (Extras, Lap leaderboards, Board). Clear the address there to keep
  your laps on your computer only. `tools/leaderboard` is the server, if
  you'd rather run your own.
* **Playtime:** the launcher's Play page shows how long you've played,
  how many races, and your most played events, tracks and bikes. Unless
  you turn off "Add my time to everyone's" there, the launcher also sends
  those times to the OpenMCM2 server, added to everyone's, with no name
  or anything else that says who you are.

## New in 0.5.0

Fixes for the browser version:

* **Right brightness:** it drew everything too dark, in every browser.
* **Sound:** it had none. Safari starts it after your first click or key.
* **Original or Enhanced:** pick the look on the start page (it's
  remembered). While playing, the button in the top-left corner shows
  which look is on and switches it.
* **The launcher's settings on the start page:** the enhanced look's
  quality, time of day, weather, ground detail and single effects, and
  the Extras, under "Settings and extras". They stay in your browser.
* **Your own tracks:** download a track's .zip from MDX Racing, then add
  it under "Your own tracks" with the event it's for. It joins that
  event's track list and stays in your browser.

Fixes for the desktop version:

* **Intel HD 3000 on OpenGL (#19):** races froze on "Get ready" on big
  tracks such as Beaver County since 0.4.0. The original look now loads
  the same amount of object data as 0.3.3; the Enhanced extras load
  only when Enhanced is on.
* **Multiplayer Ready button (#20):** it had no text. Like the original,
  it now says "Ready"; the host sees "Remove" on the other players'
  buttons, and it now sends that player out of the race.
* **Riding over the border hills (#21):** the throw back toward the
  middle of the map now plays out in full, as in the original, and you
  get up where you land instead of back at the edge.

The desktop version also brings in **Daniel's fork 1.2**
([dwolp/MCM2](https://github.com/dwolp/MCM2)). Thank you again, Daniel.
As before, everything the original game didn't have is off until you
turn it on.

* **A new launcher:** pages down the side (Play, Graphics, Extras,
  Tracks, Updates), quality presets, and every graphics setting
  explained.
* **Ray tracing** with the Enhanced look, on cards that can (NVIDIA RTX,
  AMD RX 6000 or newer): real sun shadows from everything, light shafts
  through the trees, and reflections in puddles, wet ground and paint.
  Pick the **Ray traced** preset.
* **Night races and the weather:** the Enhanced look adds a Time of Day
  and Weather choice to the Event page. At night the Nationals' light
  poles are lit and the bikes have headlights.
* **TAA anti-aliasing** (the Medium preset uses it): no more shimmering
  fences, leaves and billboards. Far trees are drawn as flat pictures,
  as the original did, so forests are faster.
* **Dust clouds that cast shadows**, and steadier lights at night.
* **Forgiving landings** (in Extras): a hard landing level on both
  wheels no longer throws you off. The original is strict about this.

New extras in the launcher's Extras page (and the browser's settings,
where they work there). Like every extra, each one is off until you turn
it on, so the game still plays and looks as the original did:

* **Add-on art for added tracks:** archives in the game's `RES/ADDONS`
  folder (such as `Rats Hole-ECO.RES`, or any `.res` in `RES/ADDONS/ECO`)
  add the trees, models and pictures a player-made track needs. They
  only add what the game's own archives don't have, so the stock tracks
  never change. The Tracks page puts a track's own `.res` files there.
* **Your own paints and outfits:** every `.tga` in the game's `BIKES`
  and `RIDERS` folders shows up on the Bike & Rider page, named after
  its file. Bike paints follow `BIKE01.TGA`'s layout, and you pick the
  engine size.
* **All gear in every event:** the bikes and outfits the Pro Circuit
  gives out, in single races too.
* **Track filter:** All tracks, Base game or Custom on the event page.
* **Stick tuning:** dead zone, sensitivity and smoothing for a pad's
  steering.
* **Frame cap, smoother edges and sharpening** over the race picture, in
  either look.
* **Helmet Cam:** a view from the rider's helmet, after User Cam.
* **More riders on the gate:** up to 30 in a single race.
* **Lap leaderboards:** your best lap on each track, event, course and
  bike class, from lap 2 on and only with the riding extras off. In
  Nationals and Supercross, a lap with more than 2 seconds off the course
  doesn't count. The launcher shows them. Sharing them needs a
  leaderboard server address in the launcher; `tools/leaderboard` has a
  small one anyone can run.
* **Lobby downloads:** in a network race, the host's track and other
  riders' own paints and outfits are sent to the players who don't have
  them. Everyone in the race needs 0.5.0 or newer with it on.

## New in 0.4.2

* **Network races in the browser:** the browser version can now host
  and join races. Players connect straight to each other (WebRTC), and
  the free public PeerJS server only introduces them, so nobody needs an
  account or a server. The host gets a race code and a "Copy invite
  link" button; whoever opens the link finds the race in the
  Multiplayer list, or the code can be typed in the TCP/IP box. Browser
  and desktop players can't race each other.

The desktop version is unchanged in this release.

## New in 0.4.1

* **More ways to add your game:** the launcher and the browser version
  now also take a `.zip` holding an installed game, the CD's files or a
  CD image, and `.mdf`/`.mds` images (including 2448-byte-sector ones).

If you're on 0.1.6 or later, the launcher will offer this update itself.

## New in 0.4.0

* **Play in your browser:** OpenMCM2 now runs in a browser with WebGPU
  (a recent Chrome or Edge, or Safari 26) at
  https://crspytopgn.github.io/openmcm2/play/. Like the desktop version,
  it plays from your own copy of the game: pick your installed game
  folder, your CD or a CD image. The files are read on your computer and
  never uploaded, and the page holds no game data. It also has touch
  controls for phones and tablets. Network races aren't in the browser
  version yet.
* **More in the Enhanced look**, each with its own off switch (see
  `docs/enhanced-graphics.md`):
  * photo materials (wood, metal, concrete, straw) on every track's
    buildings, fences, bridges and vehicles (`no-surfaces`)
  * trees and bushes that sway in the wind (`no-trees`)
  * soft shadows that blur the further they fall (`no-soft`)
  * track wear: the racing line packs down darker lap after lap
    (`no-wear`)
  * rounder bikes and riders (`no-round`)
  * floodlights in the supercross stadiums (`no-floodlights`)

  The Original look is unchanged.
* **Physics:** glancing hits on bales and barriers no longer spin the
  bike round; wall crashes now happen when the original's do.

If you're on 0.1.6 or later, the launcher will offer this update itself.

## New in 0.3.3

* **OpenGL: the sky and the ground draw properly:** on OpenGL the sky
  showed the wrong picture, and some tracks' ground (such as the
  Munchberry Farms enduro) was black (#15). Vulkan, Metal and DirectX 12
  were not affected and look the same as before.

If you're on 0.1.6 or later, the launcher will offer this update itself.

## New in 0.3.2

* **OpenGL on older graphics cards works:** the game no longer stops at
  the track on cards whose OpenGL can't read storage buffers in vertex
  shaders, such as Intel HD Graphics 2000/3000 (#13). The picture is
  the same as before.
* **The Linux build runs on older distributions:** it is now built
  against glibc 2.28, so it runs on RHEL 8 and 9, Debian 10, Ubuntu
  18.04 and later without rebuilding.
* The optional Enhanced graphics need Vulkan, Metal or DirectX 12; on
  OpenGL the game now draws the original look instead of stopping.

If you're on 0.1.6 or later, the launcher will offer this update itself.

## New in 0.3.1

* **Linux: the menus show their text again** on systems without Arial or
  a look-alike font installed (many Fedora, Arch and Void installs). The
  Linux build now carries Liberation Sans for this (#7).
* **Older graphics cards:** the game already falls back to OpenGL when
  there is no Vulkan; a new **Use OpenGL** switch in the launcher's
  Graphics section (or `--opengl`) makes it use OpenGL even when a Vulkan
  driver is there but doesn't work (#8).

If you're on 0.1.6 or later, the launcher will offer this update itself.

## New in 0.3.0

Most of this release comes from **Daniel's fork,
[dwolp/MCM2](https://github.com/dwolp/MCM2)**. Thank you, Daniel.

Fixes:

* **Track records are saved:** best laps, stunt scores and race times now
  go into the High Scores tables after a single-player race.
* **Enduro and Baja gates count from either side** for you, as in the
  original.
* **Difficulty Level works in single events:** Easy and Hard were always
  raced as Normal before.
* **Ultrawide screens:** a 21:9 view with black bars beyond it, and a race
  display that isn't stretched. The launcher's display sizes are grouped by
  shape.
* **Options > Graphics > Display Resolution** works, and the frame rate
  (F2) shows on screen, full screen too.
* **Enhanced look:** no more bright line across the ground, and 3D grass
  only grows on green ground (not on dirt, sand, rock or roads).
* Options that have no effect yet are greyed out, and a pad's shoulder
  buttons switch the menu tabs.
* Linux: menu fonts are found in any font folder; Steam Deck: the pointer
  shows in Gaming Mode and suboptimal frames no longer stall. Texture
  export works on Windows.
* The game can start without the launcher, with the launcher's settings,
  and the launcher can make a desktop shortcut.

**Extras**, all off unless you turn them on in the launcher's new Extras
section (or the game's Options > Tweaks...). "Back to the original" turns
them all off again:

* From Daniel: modernised HUD, quick get-up (G or a pad's Back), air
  steering and air braking, live gap to your best lap, steadier camera,
  field of view, controller vibration, a 1 to 10 difficulty table, cleaner
  starts for the computer riders, and one-lap National and Supercross
  races.
* New: a **km/h speedometer**, **mute in the background**, and a
  **screenshot key** (Ctrl+P saves to the Screenshots folder beside your
  saves).

If you're on 0.1.6 or later, the launcher will offer this update itself.

## New in 0.2.1

* **A logo:** OpenMCM2 now has its own tyre logo, in the launcher and as
  the window, program and Mac app icon.
* **Controllers:** pads whose triggers or axes rest at one end (not the
  middle) no longer hold gas, brake or steer on their own, and buttons
  remapped in Options > Controls now work in the race.
* **Tracks with a hidden start gate** (some player-made tracks) count laps
  properly and no longer show the start arch.
* **3D sounds** fall silent past their range (500 ft), as in the original.
* **Mac:** the launcher's self-update now works when macOS runs the app
  from a temporary copy (as it does for apps opened straight from
  Downloads).

If you're on 0.1.6 or later, the launcher will offer this update itself.

## New in 0.2.0

Riding and racing:

* **Wheels ride up steep faces:** a wheel now climbs onto a steep face,
  such as a railway crossing, instead of catching on it.
* **No more roll-overs on side slopes:** a tyre that is off the ground no
  longer counts as bottomed out, which could flip the bike on a sideways
  slope.
* **Baja and Enduro computer riders** follow the course line to the next
  gate, so they no longer stall on climbs like Morenci's.
* **Computer riders are drawn smoothly** between their physics steps.

More tracks:

* **Get tracks in the launcher:** a new More tracks section lists and
  searches the player-made tracks on MDX Racing (mdxracing.com) and
  downloads a track only when you ask for it. Tracks only offered as
  `.rar` link to the site instead. Remove takes a track out again.

Menus and replays:

* **Replay Save works**, and saving the same race twice keeps one file.
* The **Options and Joystick buttons** on the race screens work.
* Replay blur is now **off by default** in the enhanced look.

If you're on 0.1.6 or later, the launcher will offer this update itself.

## New in 0.1.8

* **Radar** now draws both edges of the track, as the original does.
* **No more black box** flashing on screen with the enhanced look. It came
  from the dirt particles under anti-aliasing, and the glow effect now
  ignores any broken pixel.
* **Windows:** any window that pops up over the game is noted in
  `last-run.log`.

If you're on 0.1.6 or later, the launcher will offer this update itself.

## New in 0.1.7

* **Windows:** the game no longer has a console window (it could flash a
  black box over the game), and the launcher minimises while you play.
* **Radar** shows the track line in Nationals and Supercross and the next
  gates in Baja and Enduro, like the original.
* **Your bike, rider and plate** are remembered in your profile.

If you're on 0.1.6, the launcher will offer this update itself.

## New in 0.1.6

* **The launcher can now update OpenMCM2 itself** (Updates section; can be
  turned off). Downloads are checked against the release's SHA-256
  checksums, and only OpenMCM2 itself is ever fetched, never game data.
  Settings and game data are kept.

## New in 0.1.5

* **Races end at the finish** like the original: you go to the podium after
  a top-three finish with Podiums on, otherwise to the results. The Stunt
  Quarry ends when time is up. This is new and hasn't been played through
  by hand yet, so reports are welcome.
* **No console window on Windows:** the launcher no longer opens one behind
  the game.

## New in 0.1.4

* **Bike packs draw properly:** bikes installed from MDX Racing bike packs
  (such as YZ125 skins), with Targa paint and text models, now show their
  paint instead of drawing white.
* **Replays keep the computer riders' paint** on their bikes.
* A missing model texture is now named in the log, to help track down
  bikes or objects that draw white.

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
