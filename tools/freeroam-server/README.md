# Always-open free roam server

OpenMCM2's own always-open free roam is a room on the leaderboard worker
(Cloudflare, see `tools/leaderboard/README.md`): nothing to run. This page
is for running one yourself, on your own computer or a VM, for example for
a LAN or your own group.

`mm2-viewer --serve` runs a multiplayer host with no window, no game files
and no rider of its own. It keeps one free roam session open for good, so
anyone can pick it from the **FREE ROAM** list in Host or Join (in the game
or the browser version) and ride, any time, without someone having to host.

It works like any other host: players reach it by its race code through the
same matchmaking server browsers use (no port forwarding needed), it lists
itself in the server browser every 30 seconds, and it only passes bikes and
chat between the players. It uses almost no CPU and about 30 MB of memory.
Each rider sends 20 small updates a second, so traffic is roughly
**1 GB per hour with 8 riders**, much less with a few.

By default it serves the Free Pack's **Copper Ridge** (`FPNAT01`, Nationals),
so players with the free graphics can join as well as MM2 owners with the
Free Pack.

## Options

```
mm2-viewer --serve [--name TEXT] [--event N] [--track STEM] [--title TEXT]
                   [--files DIR] [--welcome TEXT]
```

| Flag | Default | What it does |
|---|---|---|
| `--name` | `OpenMCM2 Free Roam` | The session's name in the list |
| `--event` | `2` | 1 Baja, 2 Nationals, 3 Supercross, 5 Enduro |
| `--track` | `FPNAT01` | The track's file stem |
| `--title` | `Copper Ridge` | The track's name in the list |
| `--files` | none | A game or Free Pack folder to send the track from to players who haven't got it |
| `--welcome` | a short hello | What each player is told on arriving |

Run more than one (say a Supercross one too) by starting another copy with
other flags; each takes the next free port.

## Where to run it

The server needs a computer that stays on. Any of these work:

| Option | Cost | Notes |
|---|---|---|
| Your own computer or a Raspberry Pi at home | Free (power only) | Easiest. Must stay on. Home upload covers 8 riders easily. |
| Oracle Cloud "Always Free" AMD micro VM (x86) | Free | Needs a card to sign up. Oracle can reclaim idle free VMs; upgrading the account to Pay As You Go stops that and stays free within the limits. 10 TB a month of traffic. |
| Google Cloud e2-micro free tier (US regions) | Free, then about $0.12 a GB | Only 1 GB of free traffic a month, so a busy evening costs a little. |
| Hetzner Cloud small VM | About €5-6 a month | 20 TB of traffic included. Reliable, simple. |
| Fly.io smallest machine | About $2-3 a month plus $0.02 a GB | Pay as you go. |

Cloudflare Workers and Containers don't fit as they are: players connect to
hosts over WebRTC (UDP), which those can't accept. A Cloudflare-only server
would need the game to also speak WebSockets, plus a relay written for a
Durable Object (free for light use, $5 a month plan for heavy use).

## Setting up on Linux

1. Make a user and a folder: `sudo useradd -m openmcm2 && sudo mkdir -p /opt/openmcm2`
2. Put the Linux release's `mm2-viewer` in `/opt/openmcm2/` (from the
   Releases page) and install its sound library: `sudo apt install libasound2`
   (Ubuntu/Debian; it isn't used, but the program links it).
3. Try it: `sudo -u openmcm2 /opt/openmcm2/mm2-viewer --serve`. It prints
   its race code, then each player who comes and goes.
4. Install the service so it starts on boot and restarts if it stops:
   `sudo cp openmcm2-freeroam.service /etc/systemd/system/ && sudo systemctl enable --now openmcm2-freeroam`
5. Watch it: `journalctl -u openmcm2-freeroam -f`

No ports need opening for browsers or for desktops joining from the list.
To let desktops join by address too, open TCP port 28220.

## On a Mac

```
"/Applications/OpenMCM2.app/Contents/MacOS/mm2-viewer" --serve
```

Keep the Mac awake (System Settings > Energy, or `caffeinate -i` before
the command).
