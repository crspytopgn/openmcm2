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
//
// Each board keeps one lap per rider (their best), at most KEEP of them.

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

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (request.method === "POST" && url.pathname === "/laps") return addLaps(request, env);
    if (request.method === "GET" && url.pathname === "/board") return board(url, env);
    if (request.method === "GET" && url.pathname === "/boards") return boards(url, env);
    if (request.method === "POST" && url.pathname === "/playtime") return addPlaytime(request, env);
    if (request.method === "GET" && url.pathname === "/playtime") return playtime(env);
    if (request.method === "OPTIONS") return text("", 204);
    return text("OpenMCM2 lap leaderboards\n", url.pathname === "/" ? 200 : 404);
  },
};
