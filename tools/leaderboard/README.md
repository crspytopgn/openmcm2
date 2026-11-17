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

## The server browser

The same worker keeps the open network races for the game's Host or Join
list. They live in one Durable Object (`Races`, its own small SQLite
table), which the free plan allows and `npx wrangler deploy` sets up from
`wrangler.toml`; nothing else needs making.

- `POST /race`: a host's race, every 30 seconds while it is open, as
  `key<TAB>value` lines: `id` (the host's own random name for the race),
  `kind` (`pc` or `web`), `port` (a computer's) or `code` (a browser's race
  code), `name`, `event`, `track`, `players`, `racing`, `version`. The
  answer is `ok`, `full`, or `closed` when the worker couldn't connect to
  a computer's race port (it tries once, and again every two minutes).
- `POST /race/end`: `id<TAB>…`, and the race is off the list.
- `GET /races?kind=pc` (or `web`): the open races that can be reached, as
  `address<TAB>name<TAB>event<TAB>track<TAB>players<TAB>racing<TAB>version`
  lines. A computer's address is the one its word came from, with its port.

- `GET /races?kind=room`: the always-open free roam rooms, in the same
  lines, with each room's WebSocket address and how many are in it.
- `GET /room/<name>` (a WebSocket): into a free roam room. Each room
  (`ROOMS` in `worker.js`, one `Room` Durable Object each) hosts a free
  roam session as a player's game would: players say Hello, press Ready
  to drop into one of the 8 places, and their bikes and chat are passed
  to the others. The room isn't a player. Messages are the game's own
  (`net.rs`, `Msg`), one per WebSocket message. Add a room by adding a
  line to `ROOMS` (its event, track file and name) and deploying.

Cost: on the free plan each incoming WebSocket message counts as 1/20 of
a request, and riders send 20 a second, so the free 100,000 requests a
day cover roughly 25 rider-hours a day; past that the rooms stop until
midnight UTC. The $5 a month Workers plan covers far more.

A race not heard of for 90 seconds drops off. Each address can list four
races at once, and the list holds at most 300.

OpenMCM2's own server runs at `https://openmcm2-laps.openmcm2-laps.workers.dev`
(the default `LEADERBOARD_URL`). To update it after changing `worker.js`,
run `npx wrangler deploy` here again.

Each board keeps one lap per rider, their best, and the best 100 riders. Laps
under 5 seconds or over an hour are turned away. There are no accounts, so
anyone could send a made-up time; if that becomes a problem, the board can be
cleared in the Cloudflare dashboard (KV, LAPS) or the worker given a shared
key.
