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

Options > Controls can rebind any of them.

In the game's menus (the lobby and the Course Builder's button too), the
D-pad or left stick jumps between buttons and list lines, and keeps going
while held. A clicks, B goes back, and the shoulder buttons switch tabs.
Left and right move a slider, and up or down past the last line of a list
scrolls it. The right stick moves the pointer freely. In a name or chat
box, up and down change the last letter, right adds a letter, left or X
takes one away, Y types a space, and A or B finish typing.

In the launcher, the D-pad or stick moves between the buttons (keeping
going while held), A presses the one outlined, B backs out of an open
list, the shoulder buttons step through everything in order, the
triggers go to the page before or after (Play, Graphics, Extras, Tracks,
Updates), and the right stick scrolls. The touch screen works everywhere
too.

### Rumble and gyro steering

The pad thumps on landings, knocks over sharp bumps and jolts in crashes
when the profile's Force Feedback box (Options > Controls) or the
launcher's **Controller vibration** is on.

**Gyro steering** (launcher Extras, off unless you set it) steers by
tipping the Deck, a Steam Controller or a PlayStation or Switch pad left
and right like handlebars, on top of the stick. The number is how far you
tip it for full lock: 20 to 30° is quick, 45° and more is gentle. Near
level always rides straight, whether you hold it flat or upright. The
game reads the motion sensors itself, so leave Steam's own gyro setting
for OpenMCM2 off (Steam's "Gyro to Joystick" also works, through the
stick, if you prefer it).

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
