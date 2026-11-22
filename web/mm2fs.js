// The files the game sees in a browser (crates/mm2-viewer/src/fs.rs):
// the player's own game folder at /game, read into memory, and their saves
// at /user, kept in IndexedDB. Names match whatever their case, as on the
// Windows the game was made for.

const files = new Map(); // "/game/ui/mainmenu.dat" -> { path, data }
const dirs = new Map(); // "/game/ui" -> Map(lower name -> shown name)

function norm(path) {
  const parts = [];
  for (const p of path.split("/")) {
    if (p === "" || p === ".") continue;
    if (p === "..") parts.pop();
    else parts.push(p);
  }
  return "/" + parts.join("/");
}

function index(path) {
  const parts = path.split("/").slice(1);
  let dir = "";
  for (let i = 0; i < parts.length; i++) {
    const key = dir.toLowerCase() || "/";
    if (!dirs.has(key)) dirs.set(key, new Map());
    const last = i === parts.length - 1;
    const name = last ? parts[i] : parts[i] + "/";
    dirs.get(key).set(name.toLowerCase(), name);
    dir += "/" + parts[i];
  }
}

function unindex(path) {
  const cut = path.lastIndexOf("/");
  const parent = path.slice(0, cut).toLowerCase() || "/";
  const name = path.slice(cut + 1).toLowerCase();
  dirs.get(parent)?.delete(name);
  dirs.get(parent)?.delete(name + "/");
}

/// The paths of the files in memory under `prefix`.
export function paths(prefix) {
  const p = norm(prefix).toLowerCase();
  return [...files.values()].filter((f) => f.path.toLowerCase().startsWith(p + "/")).map((f) => f.path);
}

export function put(path, data) {
  path = norm(path);
  files.set(path.toLowerCase(), { path, data });
  index(path);
}

// Files still being read in (see later()): how many are left.
let waiting = 0;

/// Lists `list` ([path, blob]) as there, and reads them in one by one
/// behind the game: a race waits for ready() (crates/mm2-viewer/src/fs.rs).
/// `done(path)` is told of each, for the loading note.
export function later(list, done) {
  for (const [p] of list) {
    const path = norm(p);
    if (!files.has(path.toLowerCase())) files.set(path.toLowerCase(), { path, data: undefined });
    index(path);
  }
  waiting += list.length;
  let next = 0;
  const worker = async () => {
    while (next < list.length) {
      const [p, blob] = list[next++];
      try {
        const f = files.get(norm(p).toLowerCase());
        if (f && f.data === undefined) f.data = new Uint8Array(await blob.arrayBuffer());
      } catch (e) {
        console.warn("couldn't read", p, e);
      }
      waiting--;
      done?.(p);
    }
  };
  return Promise.all(Array.from({ length: 4 }, worker));
}

function ready() {
  return waiting === 0;
}

// ---- IndexedDB: saves, and the game folder when the player keeps it ----

let db = null;

function request(r) {
  return new Promise((ok, fail) => {
    r.onsuccess = () => ok(r.result);
    r.onerror = () => fail(r.error);
  });
}

export async function open() {
  if (db) return db;
  const r = indexedDB.open("openmcm2", 2);
  r.onupgradeneeded = () => {
    // Version 2 added the downloaded Free Pack's files, by their hash.
    for (const name of ["user", "game", "pack"]) {
      if (!r.result.objectStoreNames.contains(name)) r.result.createObjectStore(name);
    }
  };
  db = await request(r);
  return db;
}

function store(name, mode) {
  return db.transaction(name, mode).objectStore(name);
}

/// Reads the saves into memory.
export async function loadUser() {
  await open();
  const keys = await request(store("user", "readonly").getAllKeys());
  const values = await request(store("user", "readonly").getAll());
  keys.forEach((k, i) => put(k, new Uint8Array(values[i])));
}

// The Free Pack's zip is kept beside the game's files, under its own key.
const PACK = "/freepack.zip";

/// The kept game folder's files (File or Blob objects), by path.
export async function keptGame() {
  await open();
  const keys = await request(store("game", "readonly").getAllKeys());
  const values = await request(store("game", "readonly").getAll());
  const list = keys.map((k, i) => [k, values[i]]).filter(([k]) => k !== PACK);
  return list.length ? list : null;
}

/// The kept Free Pack zip, if any.
export async function keptPack() {
  await open();
  return (await request(store("game", "readonly").get(PACK))) || null;
}

export async function keepPack(file) {
  await open();
  await request(store("game", "readwrite").put(file, PACK));
}

export async function forgetPack() {
  await open();
  await request(store("game", "readwrite").delete(PACK));
}

export async function keepGame(list) {
  await open();
  const s = store("game", "readwrite");
  for (const [path, blob] of list) s.put(blob, path);
  await new Promise((ok, fail) => {
    s.transaction.oncomplete = ok;
    s.transaction.onerror = () => fail(s.transaction.error);
  });
}

/// Saved files under `prefix` (a /user path), by path.
export async function userFiles(prefix) {
  await open();
  const keys = await request(store("user", "readonly").getAllKeys());
  return keys.filter((k) => k.toLowerCase().startsWith(prefix.toLowerCase()));
}

/// One saved file's bytes, read from the saves without loading them all
/// (the start page's playtime card), or null.
export async function userFile(path) {
  await open();
  const v = await request(store("user", "readonly").get(norm(path)));
  return v ? new Uint8Array(v) : null;
}

/// Keeps a file among the saves, before or while the game runs.
export async function keepUser(path, data) {
  await open();
  write(path, data);
}

export async function forgetUser(path) {
  await open();
  remove(path);
}

// ---- The downloaded Free Pack: each file under its SHA-256, and the
// manifest it was last brought up to date with under "manifest". ----

export async function packStored() {
  await open();
  return new Set(await request(store("pack", "readonly").getAllKeys()));
}

export async function packGet(key) {
  await open();
  return (await request(store("pack", "readonly").get(key))) || null;
}

/// Several of the pack's files, in one go (one look per file is slow on
/// phones): a Map of key -> value, missing ones left out.
export async function packGetMany(keys) {
  await open();
  const s = store("pack", "readonly");
  const out = new Map();
  await Promise.all(
    [...new Set(keys)].map((k) =>
      request(s.get(k)).then((v) => {
        if (v) out.set(k, v);
      })
    )
  );
  return out;
}

export async function packPut(key, value) {
  await open();
  await request(store("pack", "readwrite").put(value, key));
}

export async function packDelete(keys) {
  await open();
  const s = store("pack", "readwrite");
  for (const k of keys) s.delete(k);
  await new Promise((ok, fail) => {
    s.transaction.oncomplete = ok;
    s.transaction.onerror = () => fail(s.transaction.error);
  });
}

/// Forgets the game's files (the Free Pack stays).
export async function forgetGame() {
  await open();
  const keys = await request(store("game", "readonly").getAllKeys());
  const s = store("game", "readwrite");
  for (const k of keys) if (k !== PACK) s.delete(k);
  await new Promise((ok, fail) => {
    s.transaction.oncomplete = ok;
    s.transaction.onerror = () => fail(s.transaction.error);
  });
}

// ---- What the game calls ----

function read(path) {
  return files.get(norm(path).toLowerCase())?.data;
}

function list(path) {
  const d = dirs.get(norm(path).toLowerCase());
  return d ? [...d.values()] : undefined;
}

function kind(path) {
  const key = norm(path).toLowerCase();
  if (files.has(key)) return 1;
  return dirs.has(key) ? 2 : 0;
}

function write(path, data) {
  path = norm(path);
  // The Course Builder's tracks and the Free Pack's built tracks join the
  // track folders (the game's, or the pack's without it) for this visit;
  // both are kept among the saves too (/user/Tracks, /user/FreeTracks).
  if (/^\/(game|freepack)\/teraform\/[^/]+\/[^/]+\.(env|lights|made)$/i.test(path)) {
    const old = files.get(path.toLowerCase());
    put(old ? old.path : path, new Uint8Array(data));
    return true;
  }
  if (!path.toLowerCase().startsWith("/user/")) return false;
  const copy = new Uint8Array(data);
  const old = files.get(path.toLowerCase());
  if (old) path = old.path;
  put(path, copy);
  if (db) store("user", "readwrite").put(copy.buffer, path);
  return true;
}

function remove(path) {
  const key = norm(path).toLowerCase();
  if (!key.startsWith("/user/")) return false;
  let found = false;
  for (const [k, f] of [...files]) {
    if (k === key || k.startsWith(key + "/")) {
      files.delete(k);
      unindex(f.path);
      if (db) store("user", "readwrite").delete(f.path);
      found = true;
    }
  }
  for (const k of [...dirs.keys()]) {
    if (k === key || k.startsWith(key + "/")) {
      dirs.delete(k);
      found = true;
    }
  }
  unindex(norm(path));
  return found;
}

// The game folder unpacked from the demo's installer (in the game, at its
// start): kept in memory for this visit, like a picked folder.
function putGame(path, data) {
  path = norm(path);
  if (!path.toLowerCase().startsWith("/game/")) return false;
  put(path, new Uint8Array(data));
  return true;
}

globalThis.mm2fs = { read, list, kind, write, remove, putGame, ready };
