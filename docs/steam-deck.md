# Steam Deck and Steam Machine

OpenMCM2 runs on SteamOS like any other Linux game. The easiest way is
the AppImage: one file with the launcher, the game and its icon.

## Install

1. Switch to **Desktop Mode** (Steam button, Power, Switch to Desktop).
2. Download `openmcm2-vX.Y.Z-linux-x86_64.AppImage` from the
   [releases page](https://github.com/crspytopgn/openmcm2/releases/latest).
   Put it somewhere it can stay, for example a `Games` folder in your
   home folder.
3. Right-click it, choose **Properties**, then **Permissions**, and tick
   **Is executable**.
4. Open it once. The launcher asks for your copy of Motocross Madness 2:
   a CD image, the CD in a USB drive, or the folder it's installed in.
   It copies the game into `~/.local/share/mm2`, so the CD or image isn't
   needed after that.
5. In Steam (still in Desktop Mode): **Games**, **Add a Non-Steam Game to
   My Library**, **Browse**, pick the AppImage, then **Add Selected
   Programs**. Rename the entry to OpenMCM2 if you like (right-click,
   **Properties**).
6. Go back to Gaming Mode. OpenMCM2 is in your library under
   **Non-Steam**.

## Make it look nice in the library

The release has `openmcm2-steam-artwork.zip` with the pictures Steam uses
(they're also in [assets/steam](../assets/steam)):

| File | Where it goes |
| --- | --- |
| `capsule-600x900.png` | The cover in the library grid |
| `wide-920x430.png` | The wide picture on the home screen's recent games |
| `hero-1920x620.png` | The banner at the top of the game's page |
| `logo.png` | The logo over the banner |
| `icon-512.png` | The icon |

In Desktop Mode, in Steam's library:

* Right-click OpenMCM2, **Manage**, **Set custom artwork**, and pick
  `capsule-600x900.png`. Do the same in the grid's wide view for
  `wide-920x430.png`.
* Open the game's page, right-click the empty banner, **Set Custom
  Background**, and pick `hero-1920x620.png`. Right-click it again,
  **Set Custom Logo**, and pick `logo.png`.
* Right-click OpenMCM2, **Properties**, and click the square beside its
  name to choose `icon-512.png`.

## Skip the launcher

Once the game is set up, you can go straight into the game from Steam:
in the game's **Properties**, set **Launch Options** to `--play`. It
uses the settings you last picked in the launcher. Remove `--play` to
get the launcher back (to change the look, add tracks or update).

## Controls

Steam presents the Deck's controls (and a Steam Controller) as an Xbox
pad, which OpenMCM2 reads as any Xbox pad. Leave the controller layout on
Steam's standard **Gamepad** template.

In a race:

| Control | Does |
| --- | --- |
| Left stick | Steer (left and right) and lean (up and down) |
| A, or right trigger | Gas |
| B, or left trigger | Brake |
| X and Y | Stunts |
| LB | Reverse Cam (look back) |
| RB | Clutch |
| Left stick click | Kahuna |
| Right stick click | Thrill Cam |
| D-pad up | Next camera view |
| D-pad right | Helmet Cam |
| D-pad left | Taunt |
| D-pad down | Back up |
| View (the left middle button) | Get back on the bike |
| Menu (the right middle button) | Leave the race |

Options > Controls can rebind any of them. In the game's menus, the
D-pad jumps between buttons, the left stick moves the pointer, A clicks,
B goes back, and the shoulder buttons switch tabs. In the launcher, the
D-pad or stick moves between the buttons, A presses the one outlined, and
the shoulder buttons step through everything in order. The touch screen
works everywhere too.

## Good to know

* **Graphics.** The Auto graphics setting picks Medium on a Deck. The
  screen is 1280x800; full screen uses it whole.
* **Saves and settings** live in `~/.local/share/mm2`, so they survive
  updates and reinstalling the AppImage.
* **Updates.** The launcher's Updates page can update the AppImage in
  place.
* **The `.tar.gz` download** works too, if you'd rather: add
  `mm2-launcher` from its folder to Steam the same way.

This hasn't been tried on every Deck setup yet. If something doesn't
work, please [open an issue](https://github.com/crspytopgn/openmcm2/issues)
with the file `~/.local/share/mm2/last-run.log`.
