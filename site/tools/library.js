// The files the tools look at: the player's own game (a folder, a CD, a
// CD image or a .zip of any of these), the Free Pack (each file fetched
// from the download server only when it's looked at), and tracks the
// player adds. Everything is read right here in the browser; nothing is
// ever uploaded.

import * as iso from "../play/iso.js";
import * as zip from "../play/zip.js";
import * as mm from "./pkg/mm2_webtools.js";

// Where the Free Pack's files are, as the browser version gets them.
const params = new URLSearchParams(location.search);
export const PACK_URL = params.get("pack-url") || "https://pub-18a3400893df4f6dbcc1edfbe934a71c.r2.dev/";

const lower = (p) => p.replaceAll("\\", "/").toLowerCase();
const ext = (p) => (p.match(/\.([^./]+)$/)?.[1] || "").toLowerCase();
export { ext };

// The browser version's own storage (same site): the game kept there and
// the Free Pack files it already downloaded, so nothing comes down twice.
// Only read here, never changed.
let playDb;
async function playStore() {
  if (playDb !== undefined) return playDb;
  playDb = null;
  try {
    // Don't make the database if the browser version never did.
    if (indexedDB.databases && !(await indexedDB.databases()).some((d) => d.name === "openmcm2")) return null;
    const r = indexedDB.open("openmcm2");
    playDb = await new Promise((ok, fail) => {
      r.onsuccess = () => ok(r.result);
      r.onerror = () => fail(r.error);
      r.onupgradeneeded = () => r.transaction.abort();
    });
  } catch {
    playDb = null;
  }
  return playDb;
}
async function playGet(storeName, key) {
  const db = await playStore();
  if (!db || !db.objectStoreNames.contains(storeName)) return null;
  return new Promise((ok) => {
    const r = db.transaction(storeName).objectStore(storeName).get(key);
    r.onsuccess = () => ok(r.result ?? null);
    r.onerror = () => ok(null);
  });
}
async function playAll(storeName) {
  const db = await playStore();
  if (!db || !db.objectStoreNames.contains(storeName)) return [];
  const s = () => db.transaction(storeName).objectStore(storeName);
  const req = (r) => new Promise((ok) => ((r.onsuccess = () => ok(r.result)), (r.onerror = () => ok([]))));
  const keys = await req(s().getAllKeys());
  const values = await req(s().getAll());
  return keys.map((k, i) => [k, values[i]]);
}

/// Turns any picked files into [path from the game folder, entry]: a
/// folder of the game (or of the CD), CD images and zips, looking inside
/// them. Returns { game, pack, loose } lists.
export async function readPicked(files) {
  const all = [];
  for (const f of files) {
    const path = (f.webkitRelativePath || f.name).replaceAll("\\", "/");
    const e = ext(path);
    if (["iso", "bin", "img", "mdf", "nrg"].includes(e)) {
      const list = await iso.entries(f);
      if (list) all.push(...list.map(([p, x]) => ["GAME/" + p, x]));
    } else if (e === "zip") {
      const list = await zip.entries(f);
      // A CD image inside the zip.
      const image = list.find(([p]) => /\.(iso|bin|img|mdf|nrg)$/i.test(p));
      if (image && !list.some(([p]) => /(^|\/)res\/models\.res$/i.test(p))) {
        const blob = new Blob([await image[1].arrayBuffer()]);
        const inner = await iso.entries(blob);
        if (inner) all.push(...inner.map(([p, x]) => ["GAME/" + p, x]));
      } else all.push(...list.map(([p, x]) => [f.name.replace(/\.zip$/i, "") + "/" + p, x]));
    } else all.push([path, { size: f.size, arrayBuffer: () => f.arrayBuffer() }]);
  }
  return sort(all);
}

// Finds the game folder (the one holding RES/MODELS.RES or RES/BIKE.RES)
// and the Free Pack (the folder holding stockmap.toml) among the paths.
function sort(all) {
  const gameAt = all.map(([p]) => p.match(/^(.*?)(?:^|\/)res\/(?:models|bike|objects)\.res$/i)).find(Boolean);
  const packAt = all.map(([p]) => p.match(/^(.*?)(?:^|\/)?stockmap\.toml$/i)).find(Boolean);
  const cut = (m) => (m ? (m[1] ? m[1].replace(/\/?$/, "/") : "") : null);
  const g = cut(gameAt);
  const k = cut(packAt);
  const out = { game: [], pack: [], loose: [] };
  for (const [p, x] of all) {
    if (k !== null && p.toLowerCase().startsWith(k.toLowerCase())) out.pack.push([p.slice(k.length), x]);
    else if (g !== null && p.toLowerCase().startsWith(g.toLowerCase())) out.game.push([p.slice(g.length), x]);
    else out.loose.push([p.split("/").pop(), x]);
  }
  return out;
}

export class Library {
  constructor() {
    // root id -> Map(lower path -> { path, size, arrayBuffer })
    this.roots = new Map();
    this.labels = { game: "Your game", pack: "Free Pack", mine: "Your tracks" };
    this.archives = new Map();
    this.bytes = new Map();
    this.listeners = [];
  }
  onChange(f) {
    this.listeners.push(f);
  }
  changed() {
    this.archives.clear();
    this.bytes.clear();
    for (const f of this.listeners) f();
  }
  has(root) {
    return (this.roots.get(root)?.size || 0) > 0;
  }
  setRoot(root, list) {
    const m = new Map();
    for (const [p, x] of list) m.set(lower(p), { path: p.replaceAll("\\", "/"), size: x.size, arrayBuffer: x.arrayBuffer, sha256: x.sha256 });
    this.roots.set(root, m);
    this.changed();
  }
  addToRoot(root, list) {
    const m = this.roots.get(root) || new Map();
    for (const [p, x] of list) m.set(lower(p), { path: p, size: x.size, arrayBuffer: x.arrayBuffer });
    this.roots.set(root, m);
    this.changed();
  }
  files(root) {
    return [...(this.roots.get(root)?.values() || [])];
  }
  entry(root, path) {
    return this.roots.get(root)?.get(lower(path)) || null;
  }
  /// A file's bytes (a Uint8Array), kept for a while.
  async read(root, path) {
    const key = root + ":" + lower(path);
    if (!this.bytes.has(key)) {
      const e = this.entry(root, path);
      if (!e) throw new Error(`${path} isn't there`);
      this.bytes.set(key, e.arrayBuffer().then((b) => new Uint8Array(b)));
      // Big files aren't kept: they're read again when needed.
      this.bytes.get(key).then((b) => b.length > 64e6 && this.bytes.delete(key), () => this.bytes.delete(key));
    }
    return this.bytes.get(key);
  }
  /// An archive (.RES, .ENV, .DAT, .TEX) opened by the readers, kept.
  async archive(root, path) {
    const key = root + ":" + lower(path);
    if (!this.archives.has(key)) {
      this.archives.set(
        key,
        this.read(root, path).then((b) => new mm.Archive(path.split("/").pop(), b))
      );
      this.archives.get(key).catch(() => this.archives.delete(key));
    }
    return this.archives.get(key);
  }
  /// The game's own archive in RES, if the game is here.
  async res(name) {
    if (!this.entry("game", "RES/" + name)) return null;
    return this.archive("game", "RES/" + name).catch(() => null);
  }

  /// Loads the browser version's kept game, if there is one.
  async useKeptGame() {
    const kept = (await playAll("game")).filter(([k]) => k !== "/freepack.zip");
    if (!kept.length) return false;
    if (kept.length === 1 && kept[0][0] === "/file") {
      const kept0 = kept[0][1];
      const file = kept0 instanceof File ? kept0 : new File([kept0], "game.zip");
      const picked = await readPicked([file]);
      if (!picked.game.length) return false;
      this.setRoot("game", picked.game);
      return true;
    }
    this.setRoot(
      "game",
      kept
        .filter(([k]) => k.startsWith("/game/"))
        .map(([k, blob]) => [k.slice(6), { size: blob.size, arrayBuffer: () => blob.arrayBuffer() }])
    );
    return true;
  }
  async hasKeptGame() {
    const db = await playStore();
    if (!db || !db.objectStoreNames.contains("game")) return false;
    return new Promise((ok) => {
      const r = db.transaction("game").objectStore("game").count();
      r.onsuccess = () => ok(r.result > 0);
      r.onerror = () => ok(false);
    });
  }
  /// Tracks the player added in the browser version (kept there).
  async keptTracks() {
    return (await playAll("user"))
      .filter(([k]) => /^\/user\/Tracks\/[^/]+\/[^/]+\.env$/i.test(k))
      .map(([k, v]) => {
        const data = v instanceof Uint8Array ? v : new Uint8Array(v);
        return [k.slice("/user/Tracks/".length).replace(/^([^/]+)\//, "TERAFORM/$1/"), { size: data.length, arrayBuffer: async () => data.buffer }];
      });
  }

  /// The Free Pack from the download server: just its list of files.
  /// Each file comes down when it's opened (or from the browser
  /// version's copy, when that already has it).
  async usePackOnline() {
    const r = await fetch(PACK_URL + "freepack.json", { cache: "no-cache" });
    if (!r.ok) throw new Error(`the Free Pack download answered ${r.status}`);
    const manifest = await r.json();
    this.packVersion = manifest.version;
    this.setRoot(
      "pack",
      manifest.files.map((f) => [
        f.path,
        {
          size: f.size,
          sha256: f.sha256,
          async arrayBuffer() {
            const kept = await playGet("pack", f.sha256);
            if (kept) return kept.arrayBuffer();
            const res = await fetch(PACK_URL + "files/" + f.sha256);
            if (!res.ok) throw new Error(`${f.path}: the download answered ${res.status}`);
            return res.arrayBuffer();
          },
        },
      ])
    );
  }

  /// Where to look for a model's textures: the files it came with first.
  async textureArchives(extra = []) {
    const list = [...extra];
    for (const n of ["BIKE.RES", "MODELTGA.RES", "OBJECTS.RES", "MODELS.RES", "ECO.RES", "GLOBAL.RES"]) {
      const a = await this.res(n);
      if (a) list.push(a);
    }
    return list;
  }
  async modelArchives(extra = []) {
    const list = [...extra];
    for (const n of ["MODELS.RES", "BIKE.RES", "OBJECTS.RES", "ECO.RES"]) {
      const a = await this.res(n);
      if (a) list.push(a);
    }
    return list;
  }
}

/// Finds `name` in the first archive that has it: [archive, index].
export function findIn(archives, name) {
  for (const a of archives) {
    const i = a.find(name);
    if (i !== undefined) return [a, i];
  }
  return null;
}
