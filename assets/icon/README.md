# OpenMCM2 logo

The number plate logo: OPEN over MCM2 in chrome lettering on a black MX number plate with a
lime edge, in the spirit of 2000s motocross graphics, worn like an old
sticker. An original
drawing (nothing from the Motocross Madness 2 box art or logo). Colours:
plate `#1d1c1a`, lime `#a3c24a`, outline `#121110`,
chrome greys. Lettering is Archivo Expanded Black Italic (SIL Open Font
License), turned into outlines.

- `openmcm2-worn.png`: the main logo, sun-faded with chipped edges and scratches
  (README, launcher, social preview, Steam art).
- `openmcm2.svg`, `openmcm2-dark.svg`: the clean vector logo, the same file. The plate brings its own
  background, so it works on light and dark.
- `wordmark.svg`: OPENMCM2 on one line in chrome, for tight spaces like the
  site's top bar.
- `app-icon.png`: the app icon (1024 px), a spinning dirt bike wheel on a
  lime-edged plate tile: chrome rim and hub with the same sky and ground
  reflection as the lettering, 18 spokes, a little dirt flicking off the back
  of the tyre, and the same worn finish. The other icon files are made from it.
- `icon-256.png`: window icon for the launcher and the game.
- `logo-dark.png`: the logo at the top of the launcher.
- `openmcm2.ico`: Windows .exe icon (16 to 256 px), added by each crate's `build.rs`.
- `openmcm2.icns`: Mac app icon, added by `tools/make-mac-app.sh`.
- `social-preview.png`: 1280×640 image for the GitHub repo's social preview (Settings → General).

The Steam pictures in `assets/steam/` are made from these by
`tools/make-steam-art.py`.
