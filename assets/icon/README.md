# OpenMCM2 logo

The number plate logo: chrome lettering on a black MX number plate with a
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
- `wordmark.svg`: just the chrome lettering, for tight spaces like the
  site's top bar.
- `app-icon.svg`: the app icon (1024 grid), a dirt bike wheel with a chrome rim and 18 spokes;
  `app-icon-small.svg` has thicker spokes and fewer knobs, for 64 px and below.
- `icon-256.png`: window icon for the launcher and the game.
- `logo-dark.png`: the logo at the top of the launcher.
- `openmcm2.ico`: Windows .exe icon (16 to 256 px), added by each crate's `build.rs`.
- `openmcm2.icns`: Mac app icon, added by `tools/make-mac-app.sh`.
- `social-preview.png`: 1280×640 image for the GitHub repo's social preview (Settings → General).

The Steam pictures in `assets/steam/` are made from these by
`tools/make-steam-art.py`.
