// OpenMCM2's lap leaderboard server: a Cloudflare Worker with one KV
// namespace bound as LAPS. See README.md beside this file.
//
//   POST /laps   lines of track<TAB>event<TAB>course<TAB>class<TAB>rider<TAB>ms
//                (and optionally <TAB>the track's name as the game shows it)
//   GET  /board?track=&event=&course=&class=   lines of rider<TAB>ms, fastest first
//   GET  /board?track=&event=&course=   every class: rider<TAB>ms<TAB>class
//   GET  /boards?event=   the event's tracks with laps:
//                track<TAB>course<TAB>riders<TAB>name, most riders first
//   POST /playtime   anonymous playtime: lines of key<TAB>seconds to add
//   GET  /playtime   everyone's playtime added up, as key<TAB>seconds lines
//   POST /race   a host's open race, every 30 seconds while it is open:
//                key<TAB>value lines (see addRace); answers "ok", or
//                "closed" when a computer's race port can't be reached
//   POST /race/end   id<TAB>the host's id: the race is gone
//   GET  /races?kind=pc|web|room   the open races (the server browser):
//                address<TAB>name<TAB>event<TAB>track<TAB>players<TAB>racing<TAB>version
//                (a room's address is its WebSocket address)
//   GET  /room/<name>   a WebSocket into one of the always-open free roam
//                rooms (ROOMS): the game's own messages, as a host's
//   POST /ghost   a best lap's ghost (the online ghosts extra): the lap's
//                line as POST /laps takes it, then the lap's tape (.gho) in
//                base64. Kept if it is that rider's lap on its board and the
//                fastest ghost of its track and course. Answers "ok",
//                "slower", or "wait" while the lap isn't on its board yet
//   GET  /ghosts   the ghosts kept, one per track and course:
//                track<TAB>event<TAB>course<TAB>class<TAB>rider<TAB>ms<TAB>name
//   GET  /ghost?track=&event=&course=   that ghost's tape, in base64
//   POST /shared?voter=   a Course Builder track to share (its .course
//                JSON); answers ok<TAB>id<TAB>key (the key changes or
//                removes it later). With &id=&key=, a new version of it
//   GET  /shared?mode=&sort=new|top|most&q=&page=   the shared tracks:
//                id<TAB>name<TAB>author<TAB>mode<TAB>base<TAB>downloads<TAB>
//                stars x10<TAB>votes<TAB>shared (s)<TAB>bytes<TAB>updated (s)
//   GET  /shared/track?id=&voter=   one track's .course JSON
//   POST /shared/rate   id<TAB>voter<TAB>stars (1 to 5): ok<TAB>stars x10<TAB>votes
//   POST /shared/report   id<TAB>voter: hidden once three players report it
//   POST /shared/remove   id<TAB>key (or the SHARED_ADMIN secret)
//
// Each address (an IPv4 address, or an IPv6 /64) may send only so many
// laps, ghosts, playtimes, ratings and reports a minute and a day
// (LIMITS); past that the answer is 429 "slow down", which the game takes
// as a failed send and tries again later. A shared track's stars, reports
// and downloads count once per address, whatever voter name is sent.
//
// Each board keeps one lap per rider (their best), at most KEEP of them.

import { connect } from "cloudflare:sockets";
import { DurableObject } from "cloudflare:workers";
import { mask } from "./clean.js";

const KEEP = 100;
const MOST_LINES = 200;
const SHORTEST = 5000;
const LONGEST = 60 * 60 * 1000;

function clean(s) {
  return String(s ?? "").replace(/[\u0000-\u001f\u007f]/g, "").slice(0, 24).trim();
}

function boardKey(track, event, course, cls) {
  const t = clean(track).toLowerCase();
  const n = [event, course, cls].map((v) => Number.parseInt(v, 10));
  if (!t || n.some((v) => !Number.isInteger(v) || v < 0 || v > 9999)) return null;
  return `b:${t}|${n.join("|")}`;
}

function text(body, status = 200) {
  return new Response(body, {
    status,
    headers: {
      "content-type": "text/plain; charset=utf-8",
      "access-control-allow-origin": "*",
    },
  });
}

// The address a request came from, as limits and votes count it: an IPv4
// address whole, an IPv6 address by its /64 (one home's or phone's whole
// block, which it can pick new addresses from at will).
function network(ip) {
  if (!ip.includes(":")) return ip;
  const [head, tail] = ip.toLowerCase().split("::");
  const h = head ? head.split(":") : [];
  const t = tail ? tail.split(":") : [];
  const all = tail === undefined ? h : [...h, ...Array(Math.max(0, 8 - h.length - t.length)).fill("0"), ...t];
  return `${all.slice(0, 4).map((g) => g.replace(/^0+(?=.)/, "")).join(":")}::/64`;
}

async function sha(s) {
  const d = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(s));
  return [...new Uint8Array(d)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

// Who sent a request, hashed (no addresses are kept), with `what` in front
// so the same address hashes apart for each use.
function sender(request, what) {
  return sha(`${what}:${network(request.headers.get("cf-connecting-ip") ?? "")}`);
}

// How many of each send one address may make: [a minute, a day]. Far
// more than a player's game sends (laps and playtime after each race).
const LIMITS = {
  laps: [20, 400],
  playtime: [20, 400],
  ghost: [10, 150],
  rate: [30, 300],
  report: [5, 20],
};

// Whether this send is within its address's limits (and counts it).
async function allowed(request, env, kind) {
  const [minute, day] = LIMITS[kind];
  const races = env.RACES.get(env.RACES.idFromName("all"));
  return races.hit(await sender(request, "limit"), kind, minute, day);
}

const slowDown = () => text("slow down\n", 429);

async function addLaps(request, env) {
  const body = await request.text();
  if (body.length > 64 * 1024) return text("too much\n", 413);
  const byBoard = new Map();
  const titles = new Map();
  for (const line of body.split("\n").slice(0, MOST_LINES)) {
    const f = line.replace(/\r$/, "").split("\t");
    if (f.length !== 6 && f.length !== 7) continue;
    const key = boardKey(f[0], f[1], f[2], f[3]);
    const rider = mask(clean(f[4]));
    const ms = Number.parseInt(f[5], 10);
    if (!key || !rider || !(ms >= SHORTEST && ms <= LONGEST)) continue;
    if (!byBoard.has(key)) byBoard.set(key, []);
    byBoard.get(key).push([rider, ms]);
    const title = mask((f[6] ?? "").replace(/[\u0000-\u001f\u007f]/g, "").slice(0, 40).trim());
    if (title) titles.set(key, title);
  }
  for (const [key, laps] of byBoard) {
    const rows = (await env.LAPS.get(key, "json")) ?? [];
    for (const [rider, ms] of laps) {
      const i = rows.findIndex((r) => r[0].toLowerCase() === rider.toLowerCase());
      if (i < 0) rows.push([rider, ms]);
      else if (ms < rows[i][1]) rows[i] = [rider, ms];
    }
    rows.sort((a, b) => a[1] - b[1]);
    const kept = rows.slice(0, KEEP);
    await env.LAPS.put(key, JSON.stringify(kept));
    await addToIndex(env, key, kept.length, titles.get(key));
  }
  return text(`ok ${byBoard.size}\n`);
}

// Each event's index ("i:<event>"): for each track and course, its name
// and how many riders each class's board has.
async function addToIndex(env, key, riders, title) {
  const [track, event, course, cls] = key.slice(2).split("|");
  const at = `i:${event}`;
  const index = (await env.LAPS.get(at, "json")) ?? {};
  const entry = (index[`${track}|${course}`] ??= { title: "", classes: {} });
  entry.classes[cls] = riders;
  if (title) entry.title = title;
  await env.LAPS.put(at, JSON.stringify(index));
}

// The online ghosts: one tape for each track and course ("g:" and the
// board's key without its class), the fastest lap sent with its ghost,
// and their list ("ghosts").
const MOST_GHOST = 1024 * 1024;

function ghostKey(track, event, course) {
  const key = boardKey(track, event, course, 0);
  return key && `g:${key.slice(2).split("|").slice(0, 3).join("|")}`;
}

async function addGhost(request, env) {
  const body = await request.text();
  if (body.length > MOST_GHOST) return text("too much\n", 413);
  const [line = "", tape = ""] = body.split("\n");
  const f = line.replace(/\r$/, "").split("\t");
  if (f.length !== 6 && f.length !== 7) return text("which lap?\n", 400);
  const key = boardKey(f[0], f[1], f[2], f[3]);
  const at = ghostKey(f[0], f[1], f[2]);
  const rider = mask(clean(f[4]));
  const ms = Number.parseInt(f[5], 10);
  const data = tape.trim();
  if (!key || !at || !rider || !(ms >= SHORTEST && ms <= LONGEST)) return text("which lap?\n", 400);
  if (data.length < 64 || !/^[A-Za-z0-9+/]+={0,2}$/.test(data)) return text("not a ghost\n", 400);
  // Only the rider's own lap on its board: the one POST /laps put there.
  const rows = (await env.LAPS.get(key, "json")) ?? [];
  const mine = rows.find((r) => r[0].toLowerCase() === rider.toLowerCase());
  if (!mine || mine[1] > ms) return text("wait\n");
  if (mine[1] < ms) return text("slower\n");
  const list = (await env.LAPS.get("ghosts", "json")) ?? {};
  const had = list[at.slice(2)];
  if (had && had.ms <= ms) return text("slower\n");
  const title = mask((f[6] ?? "").replace(/[\u0000-\u001f\u007f]/g, "").slice(0, 40).trim());
  await env.LAPS.put(at, data);
  list[at.slice(2)] = { cls: Number.parseInt(f[3], 10), rider: mine[0], ms, title: title || (had?.title ?? "") };
  await env.LAPS.put("ghosts", JSON.stringify(list));
  return text("ok\n");
}

async function ghosts(env) {
  const list = (await env.LAPS.get("ghosts", "json")) ?? {};
  const rows = Object.entries(list).map(([k, g]) => [...k.split("|"), g.cls, g.rider, g.ms, g.title]);
  rows.sort((a, b) => a[0].localeCompare(b[0]) || a[1] - b[1] || a[2] - b[2]);
  return text(rows.map((r) => `${r.join("\t")}\n`).join(""));
}

async function ghost(url, env) {
  const q = url.searchParams;
  const at = ghostKey(q.get("track"), q.get("event"), q.get("course"));
  if (!at) return text("which ghost?\n", 400);
  const data = await env.LAPS.get(at);
  return data ? text(`${data}\n`) : text("no ghost\n", 404);
}

async function board(url, env) {
  const q = url.searchParams;
  if (q.get("class") !== null) {
    const key = boardKey(q.get("track"), q.get("event"), q.get("course"), q.get("class"));
    if (!key) return text("which board?\n", 400);
    const rows = (await env.LAPS.get(key, "json")) ?? [];
    return text(rows.map(([rider, ms]) => `${rider}\t${ms}\n`).join(""));
  }
  // Every class of the track and course, from the event's index.
  const probe = boardKey(q.get("track"), q.get("event"), q.get("course"), 0);
  if (!probe) return text("which board?\n", 400);
  const [track, event, course] = probe.slice(2).split("|");
  const index = (await env.LAPS.get(`i:${event}`, "json")) ?? {};
  const classes = Object.keys(index[`${track}|${course}`]?.classes ?? {});
  const all = [];
  for (const cls of classes) {
    const rows = (await env.LAPS.get(`b:${track}|${event}|${course}|${cls}`, "json")) ?? [];
    for (const [rider, ms] of rows) all.push([rider, ms, cls]);
  }
  all.sort((a, b) => a[1] - b[1]);
  return text(all.slice(0, KEEP).map((r) => `${r.join("\t")}\n`).join(""));
}

async function boards(url, env) {
  const event = Number.parseInt(url.searchParams.get("event"), 10);
  if (!Number.isInteger(event) || event < 0 || event > 99) return text("which event?\n", 400);
  const index = (await env.LAPS.get(`i:${event}`, "json")) ?? {};
  const rows = Object.entries(index).map(([k, v]) => {
    const [track, course] = k.split("|");
    const riders = Object.values(v.classes).reduce((a, b) => a + b, 0);
    return [track, course, riders, v.title];
  });
  rows.sort((a, b) => b[2] - a[2] || a[0].localeCompare(b[0]));
  return text(rows.map((r) => `${r.join("\t")}\n`).join(""));
}

// The playtime keys a player may send (mm2_settings::playtime), and the
// most one send may add to a key (a week; count and players are numbers).
const PLAY_KEY = /^(total|races|count|players|event:[0-9]{1,2}|track:[^\t\n]{1,24}|bike:[0-9]{1,4})$/;
const MOST_PLAY = 7 * 24 * 3600;

async function addPlaytime(request, env) {
  const body = await request.text();
  if (body.length > 16 * 1024) return text("too much\n", 413);
  const totals = (await env.LAPS.get("playtime", "json")) ?? {};
  let added = 0;
  for (const line of body.split("\n").slice(0, MOST_LINES)) {
    const [k, v] = line.replace(/\r$/, "").split("\t");
    const key = clean(k).toLowerCase();
    const n = Number.parseInt(v, 10);
    if (!PLAY_KEY.test(key) || !(n > 0)) continue;
    const most = key === "players" ? 1 : MOST_PLAY;
    totals[key] = (totals[key] ?? 0) + Math.min(n, most);
    added++;
  }
  if (added) await env.LAPS.put("playtime", JSON.stringify(totals));
  return text(`ok ${added}\n`);
}

async function playtime(env) {
  const totals = (await env.LAPS.get("playtime", "json")) ?? {};
  return text(Object.entries(totals).map(([k, v]) => `${k}\t${v}\n`).join(""));
}

// The server browser. Open races live in one Durable Object (RACES, its
// own SQLite table) so a heartbeat costs one row, not a KV write; a race
// the host stops telling of drops off after GONE.
const GONE = 90 * 1000;
const MOST_RACES = 300;
const RACES_PER_ADDRESS = 4;

function fields(body) {
  const f = {};
  for (const line of body.split("\n").slice(0, 40)) {
    const [k, ...v] = line.replace(/\r$/, "").split("\t");
    if (k) f[k.trim().toLowerCase()] = v.join("\t");
  }
  return f;
}

// Whether the computer at ip:port takes a connection: a race on a home
// computer often can't be reached from outside, and isn't worth listing.
async function reachable(ip, port) {
  try {
    const socket = connect({ hostname: ip, port });
    const opened = await Promise.race([
      socket.opened.then(() => true, () => false),
      new Promise((ok) => setTimeout(() => ok(false), 4000)),
    ]);
    socket.close().catch(() => {});
    return opened;
  } catch {
    return false;
  }
}

async function addRace(request, env) {
  const body = await request.text();
  if (body.length > 4096) return text("too much\n", 413);
  const f = fields(body);
  const id = String(f.id ?? "");
  if (!/^[0-9a-z]{16,40}$/.test(id)) return text("which race?\n", 400);
  const kind = f.kind === "web" ? "web" : "pc";
  const ip = request.headers.get("cf-connecting-ip") ?? "";
  let address;
  if (kind === "web") {
    address = String(f.code ?? "").toUpperCase();
    if (!/^[A-Z2-9]{6}$/.test(address)) return text("which code?\n", 400);
  } else {
    const port = Number.parseInt(f.port, 10);
    if (!ip || !(port >= 1024 && port <= 65535)) return text("which port?\n", 400);
    address = ip.includes(":") ? `[${ip}]:${port}` : `${ip}:${port}`;
  }
  const race = {
    id,
    kind,
    address,
    from: ip,
    name: mask(clean(f.name)) || "Race",
    event: Math.min(99, Math.max(0, Number.parseInt(f.event, 10) || 0)),
    track: (f.track ?? "").replace(/[\u0000-\u001f\u007f]/g, "").slice(0, 40).trim(),
    players: Math.min(8, Math.max(1, Number.parseInt(f.players, 10) || 1)),
    racing: f.racing === "1" ? 1 : 0,
    version: clean(f.version).slice(0, 12),
  };
  const races = env.RACES.get(env.RACES.idFromName("all"));
  if (kind === "pc") {
    const known = await races.probed(id, address);
    if (known === null) {
      const [host, port] = [ip, Number.parseInt(f.port, 10)];
      race.open = (await reachable(host, port)) ? 1 : 0;
      race.probedNow = true;
    } else race.open = known;
  } else race.open = 1;
  const said = await races.put(race);
  return text(`${said}\n`);
}

async function endRace(request, env) {
  const f = fields(await request.text());
  const races = env.RACES.get(env.RACES.idFromName("all"));
  await races.end(String(f.id ?? ""));
  return text("ok\n");
}

async function listRaces(url, env) {
  // Several kinds at once (`kinds=web,room,pc`): one request, and one
  // call to the races object, for the whole server browser.
  const kinds = url.searchParams.get("kinds");
  if (kinds) {
    const want = new Set(kinds.split(","));
    const races = env.RACES.get(env.RACES.idFromName("all"));
    const { rows, counts } = await races.listAll([...want].filter((k) => k === "web" || k === "pc"));
    const lines = rows.map((r) => [r.address, r.name, r.event, r.track, r.players, r.racing, r.version].join("\t"));
    if (want.has("room")) lines.push(...roomLines(url, counts));
    return text(lines.map((l) => `${l}\n`).join(""));
  }
  if (url.searchParams.get("kind") === "room") return listRooms(url, env);
  const kind = url.searchParams.get("kind") === "web" ? "web" : "pc";
  const races = env.RACES.get(env.RACES.idFromName("all"));
  const rows = await races.list(kind);
  return text(rows.map((r) => `${[r.address, r.name, r.event, r.track, r.players, r.racing, r.version].join("\t")}\n`).join(""));
}

export class Races extends DurableObject {
  constructor(ctx, env) {
    super(ctx, env);
    this.sql = ctx.storage.sql;
    this.sql.exec("CREATE TABLE IF NOT EXISTS rooms (name TEXT PRIMARY KEY, players INTEGER)");
    this.sql.exec("CREATE TABLE IF NOT EXISTS hits (who TEXT, kind TEXT, at INTEGER)");
    this.sql.exec("CREATE INDEX IF NOT EXISTS hits_who ON hits (who, kind, at)");
    this.sql.exec(`CREATE TABLE IF NOT EXISTS races (
      id TEXT PRIMARY KEY, kind TEXT, address TEXT, origin TEXT, name TEXT,
      event INTEGER, track TEXT, players INTEGER, racing INTEGER,
      version TEXT, open INTEGER, probed INTEGER, seen INTEGER)`);
  }

  sweep() {
    this.sql.exec("DELETE FROM races WHERE seen < ?", Date.now() - GONE);
  }

  // One send from `who` (a hashed address): false, and not counted, when
  // it has made `minute` of this kind in the last minute or `day` in the
  // last day.
  hit(who, kind, minute, day) {
    const now = Date.now();
    if (now - (this.swept ?? 0) > 60 * 1000) {
      this.sql.exec("DELETE FROM hits WHERE at < ?", now - 86400 * 1000);
      this.swept = now;
    }
    const n = (since) => this.sql.exec("SELECT COUNT(*) AS n FROM hits WHERE who = ? AND kind = ? AND at > ?", who, kind, since).one().n;
    if (n(now - 60 * 1000) >= minute || n(now - 86400 * 1000) >= day) return false;
    this.sql.exec("INSERT INTO hits VALUES (?, ?, ?)", who, kind, now);
    return true;
  }

  // A computer's race already looked at: whether it could be reached,
  // or null to look (again: one that couldn't, two minutes on).
  probed(id, address) {
    const r = this.sql.exec("SELECT open, probed FROM races WHERE id = ? AND address = ?", id, address).toArray()[0];
    if (!r) return null;
    if (!r.open && Date.now() - r.probed > 120 * 1000) return null;
    return r.open;
  }

  put(race) {
    this.sweep();
    const old = this.sql.exec("SELECT probed, address FROM races WHERE id = ?", race.id).toArray()[0];
    if (!old) {
      const count = this.sql.exec("SELECT COUNT(*) AS n FROM races").one().n;
      const mine = this.sql.exec("SELECT COUNT(*) AS n FROM races WHERE origin = ?", race.from).one().n;
      if (count >= MOST_RACES || mine >= RACES_PER_ADDRESS) return "full";
    }
    const now = Date.now();
    const probed = old && !race.probedNow ? old.probed : now;
    this.sql.exec(
      `INSERT OR REPLACE INTO races VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      race.id, race.kind, race.address, race.from, race.name, race.event, race.track,
      race.players, race.racing, race.version, race.open, probed, now,
    );
    return race.open ? "ok" : "closed";
  }

  end(id) {
    this.sql.exec("DELETE FROM races WHERE id = ?", id);
  }

  // How many are in each free roam room, as the rooms tell it.
  roomPlayers(name, players) {
    this.sql.exec("INSERT OR REPLACE INTO rooms VALUES (?, ?)", name, players);
  }

  listAll(kinds) {
    return { rows: kinds.flatMap((k) => this.list(k)), counts: this.rooms() };
  }

  rooms() {
    return Object.fromEntries(this.sql.exec("SELECT name, players FROM rooms").toArray().map((r) => [r.name, r.players]));
  }

  list(kind) {
    this.sweep();
    return this.sql
      .exec("SELECT address, name, event, track, players, racing, version FROM races WHERE kind = ? AND open = 1 ORDER BY racing, seen DESC LIMIT 100", kind)
      .toArray();
  }
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (request.method === "POST" && url.pathname === "/race") return addRace(request, env);
    if (request.method === "POST" && url.pathname === "/race/end") return endRace(request, env);
    if (request.method === "GET" && url.pathname === "/races") return listRaces(url, env);
    if (url.pathname.startsWith("/room/")) return joinRoom(request, url, env);
    if (request.method === "POST" && url.pathname === "/laps") return (await allowed(request, env, "laps")) ? addLaps(request, env) : slowDown();
    if (request.method === "GET" && url.pathname === "/board") return board(url, env);
    if (request.method === "GET" && url.pathname === "/boards") return boards(url, env);
    if (request.method === "POST" && url.pathname === "/playtime") return (await allowed(request, env, "playtime")) ? addPlaytime(request, env) : slowDown();
    if (request.method === "GET" && url.pathname === "/playtime") return playtime(env);
    if (request.method === "POST" && url.pathname === "/ghost") return (await allowed(request, env, "ghost")) ? addGhost(request, env) : slowDown();
    if (request.method === "GET" && url.pathname === "/ghosts") return ghosts(env);
    if (request.method === "GET" && url.pathname === "/ghost") return ghost(url, env);
    if (url.pathname === "/shared" || url.pathname.startsWith("/shared/")) return shared(request, url, env);
    if (request.method === "OPTIONS") return text("", 204);
    return text("OpenMCM2 lap leaderboards\n", url.pathname === "/" ? 200 : 404);
  },
};

// ---------------------------------------------------------------------
// The always-open free roam rooms. Each is one Durable Object that hosts
// a free roam session the way a player's game does (net.rs, Session):
// players come over a WebSocket, say Hello, press Ready to drop into a
// free place, and their bikes are passed to everyone else. The room
// isn't a player and rides nowhere, so all eight places are theirs.
// The messages are the game's own (net.rs, Msg::encode), one per
// WebSocket message. Each player's state rides on their socket
// (serializeAttachment), so the room can sleep between messages.

// One room in Europe and one in North America (where each Durable Object
// lives: its location hint), so everyone has one near.
export const ROOMS = {
// Both on the MM2 demo's Voodoo Basin (NAT04): everyone with the game or
// the free demo has it, and a room has no files to send.
  "voodoo-eu": { name: "OpenMCM2 Free Roam (Europe)", event: 2, track: "NAT04", title: "Voodoo Basin", where: "weur" },
  "voodoo-us": { name: "OpenMCM2 Free Roam (America)", event: 2, track: "NAT04", title: "Voodoo Basin", where: "enam" },
};
const ROOM_PLACES = 8;
const EMPTY = 254;
const MOST_MESSAGE = 1 << 20;

function roomLines(url, counts) {
  const ws = url.protocol === "http:" ? "ws:" : "wss:";
  return Object.entries(ROOMS).map(([key, r]) =>
    [`${ws}//${url.host}/room/${key}`, r.name, r.event, `Free roam: ${r.title}`, counts[key] ?? 0, 0, "room"].join("\t"),
  );
}

async function listRooms(url, env) {
  const races = env.RACES.get(env.RACES.idFromName("all"));
  const rows = roomLines(url, await races.rooms());
  return text(rows.map((r) => `${r}\n`).join(""));
}

function joinRoom(request, url, env) {
  const key = url.pathname.slice("/room/".length).toLowerCase();
  if (!ROOMS[key]) return text("no such room\n", 404);
  if (request.headers.get("upgrade")?.toLowerCase() !== "websocket") return text("a WebSocket, please\n", 426);
  return env.ROOM.get(env.ROOM.idFromName(key), { locationHint: ROOMS[key].where }).fetch(request);
}

// The game's message encoding: little-endian, strings with a length.
class Writer {
  constructor() {
    this.parts = [];
  }
  u8(v) {
    this.parts.push(Uint8Array.of(v & 255));
    return this;
  }
  u32(v) {
    const b = new Uint8Array(4);
    new DataView(b.buffer).setUint32(0, v >>> 0, true);
    this.parts.push(b);
    return this;
  }
  str(s) {
    const b = new TextEncoder().encode(s ?? "");
    this.u32(b.length);
    this.parts.push(b);
    return this;
  }
  ride(r) {
    return this.str(r.model).str(r.texture).str(r.outfit).u32(r.cc).u32(r.plate);
  }
  player(p) {
    return this.u8(p.id).str(p.name).u8(p.ready ? 1 : 0).ride(p.ride);
  }
  players(ps) {
    this.u32(ps.length);
    for (const p of ps) this.player(p);
    return this;
  }
  game(g) {
    return this.u8(g.event).u8(0).u32(99).u32(0).str(g.track).u8(0).u8(0).u8(0).u8(0).u8(0);
  }
  done() {
    const n = this.parts.reduce((a, p) => a + p.length, 0);
    const out = new Uint8Array(n);
    let at = 0;
    for (const p of this.parts) {
      out.set(p, at);
      at += p.length;
    }
    return out;
  }
}

class Reader {
  constructor(b) {
    this.b = b;
    this.at = 0;
    this.view = new DataView(b.buffer, b.byteOffset, b.byteLength);
  }
  u8() {
    if (this.at + 1 > this.b.length) throw new Error("short");
    return this.b[this.at++];
  }
  u32() {
    if (this.at + 4 > this.b.length) throw new Error("short");
    const v = this.view.getUint32(this.at, true);
    this.at += 4;
    return v;
  }
  str() {
    const n = this.u32();
    if (this.at + n > this.b.length) throw new Error("short");
    const s = new TextDecoder().decode(this.b.subarray(this.at, this.at + n));
    this.at += n;
    return s;
  }
  ride() {
    return { model: this.str().slice(0, 64), texture: this.str().slice(0, 64), outfit: this.str().slice(0, 64), cc: this.u32(), plate: this.u32() };
  }
}

const NO_RIDE = { model: "", texture: "", outfit: "", cc: 0, plate: 0 };
const M = { HELLO: 1, WELCOME: 2, PLAYERS: 3, READY: 4, RIDE: 5, CHAT: 6, GAME: 7, START: 8, BIKE: 9, FINISHED: 10, LEAVE: 11, FILE_ASK: 12, FILE_PART: 13, FILE_NONE: 14, POS: 15 };

export class Room extends DurableObject {
  // Everyone here: each socket and its player (null before Hello).
  here() {
    return this.ctx.getWebSockets().map((ws) => ({ ws, p: ws.deserializeAttachment() })).filter((x) => x.p);
  }

  async fetch(request) {
    const key = new URL(request.url).pathname.slice("/room/".length).toLowerCase();
    const taken = new Set(this.here().map((x) => x.p.id));
    if (taken.size >= ROOM_PLACES) return text("full\n", 503);
    let id = 1;
    while (taken.has(id)) id++;
    const [client, server] = Object.values(new WebSocketPair());
    this.ctx.acceptWebSocket(server);
    server.serializeAttachment({ room: key, id, hello: false, name: "", ready: false, ride: NO_RIDE, place: -1 });
    return new Response(null, { status: 101, webSocket: client });
  }

  players() {
    return this.here()
      .filter((x) => x.p.hello)
      .map((x) => x.p)
      .sort((a, b) => a.id - b.id);
  }

  send(ws, bytes) {
    try {
      ws.send(bytes);
    } catch {}
  }

  everyone(bytes, except = null) {
    for (const { ws, p } of this.here()) if (p.hello && ws !== except) this.send(ws, bytes);
  }

  tellPlayers() {
    this.everyone(new Writer().u8(M.PLAYERS).players(this.players()).done());
  }

  async count() {
    const room = this.here()[0]?.p.room;
    const n = this.players().length;
    if (!room || n === this.lastCount) return;
    this.lastCount = n;
    try {
      await this.env.RACES.get(this.env.RACES.idFromName("all")).roomPlayers(room, n);
    } catch {}
  }

  // Free roam's comings and goings (net.rs, drop_in_out): a place whose
  // player is gone or no longer ready is free again; a ready player not
  // riding takes a free place. Those riding hear who is where.
  dropInOut(game, changed = false) {
    const all = this.here().filter((x) => x.p.hello);
    const used = new Set();
    for (const x of all) {
      if (x.p.place >= 0 && !x.p.ready) {
        x.p.place = -1;
        x.ws.serializeAttachment(x.p);
        changed = true;
      }
      if (x.p.place >= 0) used.add(x.p.place);
    }
    for (const x of all) {
      if (!x.p.ready || x.p.place >= 0) continue;
      let free = 0;
      while (free < ROOM_PLACES && used.has(free)) free++;
      if (free >= ROOM_PLACES) break;
      used.add(free);
      x.p.place = free;
      x.ws.serializeAttachment(x.p);
      changed = true;
    }
    if (!changed) return;
    const places = Array.from({ length: ROOM_PLACES }, () => ({ id: EMPTY, name: "", ready: false, ride: NO_RIDE }));
    for (const x of all) if (x.p.place >= 0) places[x.p.place] = x.p;
    const start = new Writer().u8(M.START).game(game).players(places).done();
    for (const x of all) if (x.p.place >= 0) this.send(x.ws, start);
  }

  async webSocketMessage(ws, data) {
    const me = ws.deserializeAttachment();
    const room = ROOMS[me?.room];
    if (!room || typeof data === "string" || data.byteLength > MOST_MESSAGE) return;
    const b = new Uint8Array(data);
    let r;
    let kind;
    try {
      r = new Reader(b);
      kind = r.u8();
    } catch {
      return;
    }
    if (!me.hello && kind !== M.HELLO) return;
    try {
      switch (kind) {
        case M.HELLO: {
          if (me.hello) return;
          me.name = mask(r.str().replace(/[\u0000-\u001f\u007f]/g, "").slice(0, 24)) || "Rider";
          me.ride = r.ride();
          me.hello = true;
          ws.serializeAttachment(me);
          this.send(ws, new Writer().u8(M.WELCOME).u8(me.id).done());
          this.send(ws, new Writer().u8(M.GAME).game(room).done());
          this.send(ws, new Writer().u8(M.CHAT).str(`Server: Free roam at ${room.title}. Press Ready to ride!`).done());
          this.tellPlayers();
          await this.count();
          return;
        }
        case M.READY:
          me.ready = r.u8() !== 0;
          ws.serializeAttachment(me);
          this.tellPlayers();
          this.dropInOut(room);
          return;
        case M.RIDE:
          me.ride = r.ride();
          ws.serializeAttachment(me);
          this.tellPlayers();
          return;
        case M.CHAT: {
          const line = mask(r.str().slice(0, 300));
          this.everyone(new Writer().u8(M.CHAT).str(line).done());
          return;
        }
        case M.BIKE:
        case M.POS:
        case M.FINISHED:
          // Passed on as it came, to everyone else riding: those waiting
          // in the lobby don't need bikes.
          for (const x of this.here()) if (x.ws !== ws && x.p.hello && x.p.place >= 0) this.send(x.ws, b);
          return;
        case M.LEAVE:
          ws.close(1000, "left");
          await this.gone(ws);
          return;
        case M.FILE_ASK:
        case M.FILE_PART:
        case M.FILE_NONE: {
          // A lobby download: to whom (the room has no files), stamped
          // with who it's from.
          const to = b[1];
          if (b.length < 3) return;
          if (to === 0) {
            if (kind === M.FILE_ASK) {
              const out = b.slice();
              out[0] = M.FILE_NONE;
              out[1] = me.id;
              out[2] = 0;
              this.send(ws, out);
            }
            return;
          }
          const out = b.slice();
          out[2] = me.id;
          const target = this.here().find((x) => x.p.id === to && x.p.hello);
          if (target) this.send(target.ws, out);
          return;
        }
      }
    } catch {
      // A message cut short: ignored, as the game does.
    }
  }

  // A player has gone: off the list and out of their place.
  async gone(ws) {
    const me = ws.deserializeAttachment();
    ws.serializeAttachment(null);
    if (!me?.hello) return;
    this.tellPlayers();
    const room = ROOMS[me.room];
    // A place freed: those still riding hear it.
    if (room) this.dropInOut(room, me.place >= 0);
    const n = this.players().length;
    if (n !== this.lastCount) {
      this.lastCount = n;
      try {
        await this.env.RACES.get(this.env.RACES.idFromName("all")).roomPlayers(me.room, n);
      } catch {}
    }
  }

  async webSocketClose(ws) {
    await this.gone(ws);
  }

  async webSocketError(ws) {
    await this.gone(ws);
  }
}

// ---------------------------------------------------------------------
// Shared Course Builder tracks: players' .course recipes (the line, jumps,
// shaping and paint, a few KB; the game bakes the track from it), kept in one
// Durable Object's SQLite tables. Never a game's own files: the JSON is
// read and written again here, so only the builder's fields go in, and a
// copy of a track names its file without the folder it came from (the
// player who downloads it needs that track themselves).

const SHARED_MOST = 128 * 1024;
const SHARED_PART = 1000 * 1000;
const SHARED_TOTAL = 1024 * 1024 * 1024;
const SHARED_PER_DAY = 12;
const SHARED_PAGE = 60;
const SHARED_REPORTS = 3;
const MODES = ["Supercross", "National", "Baja", "Enduro"];
// The fields of a .course (mm2-course's Course); anything else is dropped.
const COURSE_KEYS = new Set(["version", "name", "author", "mode", "land", "line", "jumps", "strokes", "paint", "objects", "plants", "rivers", "flights", "look", "models", "dress", "base"]);

function line(s, most) {
  return String(s ?? "").replace(/[\u0000-\u001f\u007f]/g, "").slice(0, most).trim();
}

function random(n, abc = "abcdefghjkmnpqrstuvwxyz23456789") {
  const b = crypto.getRandomValues(new Uint8Array(n));
  return [...b].map((v) => abc[v % abc.length]).join("");
}

const voterOk = (v) => /^[0-9a-z]{12,40}$/.test(v ?? "");

// The course as it's kept: parsed, its fields checked, the player's own
// folders taken out of a copy's track, written again.
// Numbers, short text, true/false, and lists and objects of those, not
// too deep: what a builder recipe is made of.
function plain(v, depth) {
  if (depth > 12) return false;
  if (v === null || typeof v === "number" || typeof v === "boolean") return true;
  if (typeof v === "string") return v.length <= 80;
  if (Array.isArray(v)) return v.every((x) => plain(x, depth + 1));
  if (typeof v === "object") return Object.entries(v).every(([k, x]) => k.length <= 40 && plain(x, depth + 1));
  return false;
}

function tidyCourse(body) {
  let c;
  try {
    c = JSON.parse(body);
  } catch {
    return null;
  }
  if (!c || typeof c !== "object" || Array.isArray(c)) return null;
  if (!Number.isInteger(c.version) || !MODES.includes(c.mode) || typeof c.line !== "object" || c.line === null) return null;
  for (const k of Object.keys(c)) if (!COURSE_KEYS.has(k)) delete c[k];
  // A recipe only: no packed files (models, pictures), and nothing in it
  // that could carry one (no long text anywhere).
  if ((Array.isArray(c.models) && c.models.length) || c.models === null) return null;
  delete c.models;
  if (c.look?.ground || c.land?.heightmap) return null;
  if (Array.isArray(c.plants) && c.plants.some((p) => p?.map)) return null;
  c.name = mask(line(c.name, 40)) || "Shared track";
  c.author = mask(line(c.author, 24));
  if (c.base) {
    const file = String(c.base.file ?? "").split(/[\\/]/).pop();
    if (!/^[^<>:"|?*]{1,64}\.env$/i.test(file)) return null;
    c.base = { file, course: Number.isInteger(c.base.course) ? c.base.course : 0 };
  }
  if (!plain(c, 0)) return null;
  return c;
}

async function shared(request, url, env) {
  const box = env.SHARED.get(env.SHARED.idFromName("all"));
  const p = url.pathname;
  if (request.method === "GET" && p === "/shared") {
    const rows = await box.list(
      url.searchParams.get("mode") ?? "",
      url.searchParams.get("sort") ?? "new",
      line(url.searchParams.get("q"), 40),
      Math.max(0, Number.parseInt(url.searchParams.get("page") ?? "0", 10) || 0),
    );
    return text(rows.map((r) => `${[r.id, mask(r.name), mask(r.author), r.mode, r.base, r.downloads, r.stars, r.votes, r.created, r.bytes, r.updated].join("\t")}\n`).join(""));
  }
  if (request.method === "GET" && p === "/shared/track") {
    const id = url.searchParams.get("id") ?? "";
    // A download counts once per address (the voter name only says the
    // game wants it counted).
    const voter = url.searchParams.get("voter") ?? "";
    const json = await box.get(id, voterOk(voter) ? await sender(request, "voter") : "");
    if (json === null) return text("gone\n", 404);
    return new Response(json, { headers: { "content-type": "application/json; charset=utf-8", "access-control-allow-origin": "*" } });
  }
  if (request.method !== "POST") return text("which?\n", 404);
  const body = await request.text();
  if (p === "/shared") {
    if (body.length > SHARED_MOST) return text("too big\n", 413);
    const voter = url.searchParams.get("voter") ?? "";
    if (!voterOk(voter)) return text("who?\n", 400);
    const c = tidyCourse(body);
    if (!c) return text("not a track\n", 400);
    const from = await sender(request, "shared");
    const id = url.searchParams.get("id");
    const key = url.searchParams.get("key");
    const json = JSON.stringify(c);
    const meta = { name: c.name, author: c.author, mode: c.mode, base: c.base?.file ?? "" };
    if (id) {
      const r = await box.replace(id, await sha(key ?? ""), json, meta);
      return text(r === "ok" ? `ok\t${id}\t${key}\n` : `${r}\n`, r === "ok" ? 200 : 403);
    }
    const newKey = random(24, "0123456789abcdef");
    const r = await box.add(json, meta, from, voter, await sha(newKey));
    if (!r.startsWith("ok")) return text(`${r}\n`, r === "full" ? 507 : 429);
    return text(`${r}\t${newKey}\n`);
  }
  const f = body.split("\n")[0].replace(/\r$/, "").split("\t");
  if (p === "/shared/rate") {
    const stars = Number.parseInt(f[2], 10);
    if (!voterOk(f[1]) || !(stars >= 1 && stars <= 5)) return text("which?\n", 400);
    if (!(await allowed(request, env, "rate"))) return slowDown();
    // One vote per address: a new one from it replaces its last.
    const r = await box.rate(f[0], await sender(request, "voter"), stars);
    return r ? text(`ok\t${r.stars}\t${r.votes}\n`) : text("gone\n", 404);
  }
  if (p === "/shared/report") {
    if (!voterOk(f[1])) return text("who?\n", 400);
    if (!(await allowed(request, env, "report"))) return slowDown();
    // Three reports from three addresses hide a track.
    return text(`${await box.report(f[0], await sender(request, "voter"))}\n`);
  }
  if (p === "/shared/remove") {
    const admin = env.SHARED_ADMIN && f[1] === env.SHARED_ADMIN;
    return text(`${await box.remove(f[0], admin ? null : await sha(f[1] ?? ""))}\n`);
  }
  return text("which?\n", 404);
}

export class Shared extends DurableObject {
  constructor(ctx, env) {
    super(ctx, env);
    this.sql = ctx.storage.sql;
    this.sql.exec(`CREATE TABLE IF NOT EXISTS tracks (
      id TEXT PRIMARY KEY, name TEXT, author TEXT, mode TEXT, base TEXT,
      bytes INTEGER, parts INTEGER, created INTEGER, updated INTEGER,
      origin TEXT, voter TEXT, key TEXT, downloads INTEGER DEFAULT 0,
      stars_sum INTEGER DEFAULT 0, votes INTEGER DEFAULT 0, hidden INTEGER DEFAULT 0)`);
    this.sql.exec("CREATE TABLE IF NOT EXISTS parts (id TEXT, n INTEGER, data TEXT, PRIMARY KEY (id, n))");
    this.sql.exec("CREATE TABLE IF NOT EXISTS votes (id TEXT, voter TEXT, stars INTEGER, PRIMARY KEY (id, voter))");
    this.sql.exec("CREATE TABLE IF NOT EXISTS got (id TEXT, voter TEXT, PRIMARY KEY (id, voter))");
    this.sql.exec("CREATE TABLE IF NOT EXISTS reports (id TEXT, voter TEXT, PRIMARY KEY (id, voter))");
    // Test tracks put up by mistake while the server was checked
    // (2026-11-25), by the test's own voter name.
    for (const r of this.sql.exec("SELECT id FROM tracks WHERE voter = 'probetest12345'").toArray()) this.drop(r.id);
  }

  putParts(id, json) {
    this.sql.exec("DELETE FROM parts WHERE id = ?", id);
    let n = 0;
    for (let at = 0; at < json.length; at += SHARED_PART, n++) {
      this.sql.exec("INSERT INTO parts VALUES (?, ?, ?)", id, n, json.slice(at, at + SHARED_PART));
    }
    return n;
  }

  add(json, meta, origin, voter, keyHash) {
    const now = Math.floor(Date.now() / 1000);
    const today = this.sql.exec("SELECT COUNT(*) AS n FROM tracks WHERE origin = ? AND created > ?", origin, now - 86400).one().n;
    if (today >= SHARED_PER_DAY) return "slow down";
    const total = this.sql.exec("SELECT COALESCE(SUM(bytes), 0) AS n FROM tracks").one().n;
    if (total + json.length > SHARED_TOTAL) return "full";
    let id;
    do id = random(8);
    while (this.sql.exec("SELECT 1 FROM tracks WHERE id = ?", id).toArray().length);
    const parts = this.putParts(id, json);
    this.sql.exec(
      "INSERT INTO tracks (id, name, author, mode, base, bytes, parts, created, updated, origin, voter, key) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
      id, meta.name, meta.author, meta.mode, meta.base, json.length, parts, now, now, origin, voter, keyHash,
    );
    return `ok\t${id}`;
  }

  replace(id, keyHash, json, meta) {
    const r = this.sql.exec("SELECT key FROM tracks WHERE id = ?", id).toArray()[0];
    if (!r) return "gone";
    if (r.key !== keyHash) return "not yours";
    const parts = this.putParts(id, json);
    this.sql.exec(
      "UPDATE tracks SET name = ?, author = ?, mode = ?, base = ?, bytes = ?, parts = ?, updated = ? WHERE id = ?",
      meta.name, meta.author, meta.mode, meta.base, json.length, parts, Math.floor(Date.now() / 1000), id,
    );
    return "ok";
  }

  list(mode, sort, q, page) {
    const order = {
      top: "CASE WHEN votes > 0 THEN (stars_sum * 1.0 + 3 * 2) / (votes + 2) ELSE 0 END DESC, votes DESC, created DESC",
      most: "downloads DESC, created DESC",
    }[sort] ?? "updated DESC";
    const where = ["hidden = 0"];
    const args = [];
    if (MODES.includes(mode)) {
      where.push("mode = ?");
      args.push(mode);
    }
    if (q) {
      where.push("(name LIKE ? OR author LIKE ?)");
      args.push(`%${q}%`, `%${q}%`);
    }
    return this.sql
      .exec(
        `SELECT id, name, author, mode, base, downloads, CASE WHEN votes > 0 THEN (stars_sum * 10 + votes / 2) / votes ELSE 0 END AS stars, votes, created, bytes, updated
         FROM tracks WHERE ${where.join(" AND ")} ORDER BY ${order} LIMIT ? OFFSET ?`,
        ...args, SHARED_PAGE, page * SHARED_PAGE,
      )
      .toArray();
  }

  // A hidden (reported) track is gone for downloads too.
  get(id, voter) {
    const r = this.sql.exec("SELECT parts FROM tracks WHERE id = ? AND hidden = 0", id).toArray()[0];
    if (!r) return null;
    const json = this.sql.exec("SELECT data FROM parts WHERE id = ? ORDER BY n", id).toArray().map((p) => p.data).join("");
    if (voter && !this.sql.exec("SELECT 1 FROM got WHERE id = ? AND voter = ?", id, voter).toArray().length) {
      this.sql.exec("INSERT INTO got VALUES (?, ?)", id, voter);
      this.sql.exec("UPDATE tracks SET downloads = downloads + 1 WHERE id = ?", id);
    }
    return json;
  }

  rate(id, voter, stars) {
    if (!this.sql.exec("SELECT 1 FROM tracks WHERE id = ?", id).toArray().length) return null;
    this.sql.exec("INSERT OR REPLACE INTO votes VALUES (?, ?, ?)", id, voter, stars);
    const t = this.sql.exec("SELECT COALESCE(SUM(stars), 0) AS s, COUNT(*) AS n FROM votes WHERE id = ?", id).one();
    this.sql.exec("UPDATE tracks SET stars_sum = ?, votes = ? WHERE id = ?", t.s, t.n, id);
    return { stars: t.n ? Math.floor((t.s * 10 + Math.floor(t.n / 2)) / t.n) : 0, votes: t.n };
  }

  report(id, voter) {
    if (!this.sql.exec("SELECT 1 FROM tracks WHERE id = ?", id).toArray().length) return "gone";
    this.sql.exec("INSERT OR IGNORE INTO reports VALUES (?, ?)", id, voter);
    const n = this.sql.exec("SELECT COUNT(*) AS n FROM reports WHERE id = ?", id).one().n;
    if (n >= SHARED_REPORTS) this.sql.exec("UPDATE tracks SET hidden = 1 WHERE id = ?", id);
    return "ok";
  }

  // With keyHash null (the admin), any track.
  remove(id, keyHash) {
    const r = this.sql.exec("SELECT key FROM tracks WHERE id = ?", id).toArray()[0];
    if (!r) return "gone";
    if (keyHash !== null && r.key !== keyHash) return "not yours";
    this.drop(id);
    return "ok";
  }

  drop(id) {
    for (const t of ["tracks", "parts", "votes", "got", "reports"]) this.sql.exec(`DELETE FROM ${t} WHERE id = ?`, id);
  }
}
