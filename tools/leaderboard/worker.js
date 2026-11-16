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
//   GET  /races?kind=pc|web   the open races (the server browser):
//                address<TAB>name<TAB>event<TAB>track<TAB>players<TAB>racing<TAB>version
//
// Each board keeps one lap per rider (their best), at most KEEP of them.

import { connect } from "cloudflare:sockets";
import { DurableObject } from "cloudflare:workers";

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

async function addLaps(request, env) {
  const body = await request.text();
  if (body.length > 64 * 1024) return text("too much\n", 413);
  const byBoard = new Map();
  const titles = new Map();
  for (const line of body.split("\n").slice(0, MOST_LINES)) {
    const f = line.replace(/\r$/, "").split("\t");
    if (f.length !== 6 && f.length !== 7) continue;
    const key = boardKey(f[0], f[1], f[2], f[3]);
    const rider = clean(f[4]);
    const ms = Number.parseInt(f[5], 10);
    if (!key || !rider || !(ms >= SHORTEST && ms <= LONGEST)) continue;
    if (!byBoard.has(key)) byBoard.set(key, []);
    byBoard.get(key).push([rider, ms]);
    const title = (f[6] ?? "").replace(/[\u0000-\u001f\u007f]/g, "").slice(0, 40).trim();
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
    name: clean(f.name) || "Race",
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
  const kind = url.searchParams.get("kind") === "web" ? "web" : "pc";
  const races = env.RACES.get(env.RACES.idFromName("all"));
  const rows = await races.list(kind);
  return text(rows.map((r) => `${[r.address, r.name, r.event, r.track, r.players, r.racing, r.version].join("\t")}\n`).join(""));
}

export class Races extends DurableObject {
  constructor(ctx, env) {
    super(ctx, env);
    this.sql = ctx.storage.sql;
    this.sql.exec(`CREATE TABLE IF NOT EXISTS races (
      id TEXT PRIMARY KEY, kind TEXT, address TEXT, origin TEXT, name TEXT,
      event INTEGER, track TEXT, players INTEGER, racing INTEGER,
      version TEXT, open INTEGER, probed INTEGER, seen INTEGER)`);
  }

  sweep() {
    this.sql.exec("DELETE FROM races WHERE seen < ?", Date.now() - GONE);
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
    if (request.method === "POST" && url.pathname === "/laps") return addLaps(request, env);
    if (request.method === "GET" && url.pathname === "/board") return board(url, env);
    if (request.method === "GET" && url.pathname === "/boards") return boards(url, env);
    if (request.method === "POST" && url.pathname === "/playtime") return addPlaytime(request, env);
    if (request.method === "GET" && url.pathname === "/playtime") return playtime(env);
    if (request.method === "OPTIONS") return text("", 204);
    return text("OpenMCM2 lap leaderboards\n", url.pathname === "/" ? 200 : 404);
  },
};
