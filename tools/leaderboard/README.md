# Lap leaderboard server

The lap leaderboards extra keeps each player's best laps on their own
computer. To share them, someone runs this small server and players put its
address in the launcher (Extras, Lap leaderboards, Leaderboard server).

It runs as a Cloudflare Worker on the free plan, with one KV namespace for the
boards. To set it up:

1. Make a Cloudflare account, then in this folder run
   `npx wrangler login`.
2. `npx wrangler kv namespace create LAPS`, and copy the id it prints into
   `wrangler.toml`.
3. `npx wrangler deploy`. It prints the address, such as
   `https://openmcm2-laps.<your-name>.workers.dev`.

To make another address the default for everyone, set `LEADERBOARD_URL`
in `crates/mm2-settings/src/lib.rs` to it before a release.

## What it takes and gives

- `POST /laps`: lines of `track<TAB>event<TAB>course<TAB>class<TAB>rider<TAB>ms`,
  as the launcher sends from `laps-pending.txt`.
- `GET /board?track=farm&event=2&course=0&class=250`: that board's laps as
  `rider<TAB>ms` lines, fastest first. Without `class`, every class's laps
  as `rider<TAB>ms<TAB>class` lines (the game's High Scores, Online).
- `GET /boards?event=2`: the event's tracks with laps, as
  `track<TAB>course<TAB>riders<TAB>name` lines. A lap line may end with a
  seventh field, the track's name as the game shows it.

- `POST /playtime`: anonymous playtime as `key<TAB>seconds` lines to add
  (`total`, `races`, `count`, `players`, `event:N`, `track:NAME`,
  `bike:CC`), and `GET /playtime` gives everyone's added up.

OpenMCM2's own server runs at `https://openmcm2-laps.openmcm2-laps.workers.dev`
(the default `LEADERBOARD_URL`). To update it after changing `worker.js`,
run `npx wrangler deploy` here again.

Each board keeps one lap per rider, their best, and the best 100 riders. Laps
under 5 seconds or over an hour are turned away. There are no accounts, so
anyone could send a made-up time; if that becomes a problem, the board can be
cleared in the Cloudflare dashboard (KV, LAPS) or the worker given a shared
key.
