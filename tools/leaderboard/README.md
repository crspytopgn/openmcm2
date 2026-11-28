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

## Limits

Each address (an IPv4 address, or an IPv6 /64) may send only so many laps,
playtimes, ghosts, ratings and reports a minute and a day (`LIMITS` in
`worker.js`, counted in the `Races` object's `hits` table by hashed
address). Past that the answer is 429 `slow down`; the game keeps what it
couldn't send and tries again later. A shared track's stars, reports and
downloads count once per address, so made-up voter names change nothing,
and a track hidden by reports can't be downloaded either.

## Online ghosts

With the lap leaderboards on, a new best lap goes up with its ghost. With
the online ghosts extra on (off by default), the game's Saved Ghost Race
Files list offers the server's ghosts to race. One ghost is kept for each track and course: the
fastest lap sent with one, in any class. It costs two KV writes, only
when a ghost is faster than the one kept.

- `POST /ghost`: the lap's line as `POST /laps` takes it, then on the next
  line the lap's tape (a `.gho`) in base64. The game sends it after its
  laps. The answer is `ok` (kept), `slower`, or `wait` while the lap is
  not on its board yet (the game tries again after its next race).
- `GET /ghosts`: the ghosts kept, as
  `track<TAB>event<TAB>course<TAB>class<TAB>rider<TAB>ms<TAB>name` lines.
- `GET /ghost?track=farm&event=2&course=0`: that ghost's tape, in base64.

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

## Shared Course Builder tracks

Players share their Course Builder tracks from the builder (Save / Export,
Share with other players) and get everyone's from Open, Shared tracks, or
the launcher's Tracks page. What goes up is the track's recipe only: the
builder's `.course` without any files packed in, a few KB (a dressed Baja
track is about 40 KB). The game bakes the track from it on the player's
computer. They are kept in one Durable Object (`Shared`, its own SQLite
tables, migration `v3` in `wrangler.toml`), which `npx wrangler deploy`
sets up.

The worker takes a recipe and nothing else: at most 128 KB, only the
builder's fields, no models or pictures, and no text longer than 80
characters anywhere in it, so nothing else can ride along. A copy of a
track names that track's file and never the folder it came from
(whoever downloads it needs that track). The game leaves a player's own
models, ground picture and plant maps out when it shares.

- `POST /shared?voter=ID`: the recipe, as JSON (128 KB at most). The answer
  is `ok<TAB>id<TAB>key`; the game keeps the key, which shares a new
  version (`POST /shared?voter=ID&id=ID&key=KEY`) or removes it. Each
  address can share 12 a day, and all of them can take 1 GB.
- `GET /shared?mode=National&sort=new|top|most&q=WORDS&page=0`: 60 at a
  time, as `id<TAB>name<TAB>author<TAB>mode<TAB>base<TAB>downloads<TAB>stars
  times ten<TAB>votes<TAB>shared<TAB>bytes<TAB>changed` lines (Unix seconds).
- `GET /shared/track?id=ID&voter=ID`: its JSON; a download counts once
  for each player.
- `POST /shared/rate`: `id<TAB>voter<TAB>stars` (1 to 5, one vote each,
  changeable): `ok<TAB>stars times ten<TAB>votes`.
- `POST /shared/report`: `id<TAB>voter`. Three players' reports take it
  off the list (it stays kept, `hidden = 1`, to look at).
- `POST /shared/remove`: `id<TAB>key`. With a `SHARED_ADMIN` secret set
  (`npx wrangler secret put SHARED_ADMIN`), `id<TAB>that secret` removes
  any track.

A voter is a random name each game makes for itself
(`shared-tracks.txt` in the user folder): there are no accounts.

OpenMCM2's own server runs at `https://openmcm2-laps.openmcm2-laps.workers.dev`
(the default `LEADERBOARD_URL`). To update it after changing `worker.js`,
run `npx wrangler deploy` here again.

Each board keeps one lap per rider, their best, and the best 100 riders. Laps
under 5 seconds or over an hour are turned away. There are no accounts, so
anyone could send a made-up time; if that becomes a problem, the board can be
cleared in the Cloudflare dashboard (KV, LAPS) or the worker given a shared
key.
