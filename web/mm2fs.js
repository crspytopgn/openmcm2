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
  const r = indexedDB.open("openmcm2", 1);
  r.onupgradeneeded = () => {
    r.result.createObjectStore("user");
    r.result.createObjectStore("game");
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

/// The kept game folder's files (File or Blob objects), by path.
export async function keptGame() {
  await open();
  const keys = await request(store("game", "readonly").getAllKeys());
  if (!keys.length) return null;
  const values = await request(store("game", "readonly").getAll());
  return keys.map((k, i) => [k, values[i]]);
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

/// Keeps a file among the saves, before or while the game runs.
export async function keepUser(path, data) {
  await open();
  write(path, data);
}

export async function forgetUser(path) {
  await open();
  remove(path);
}

export async function forgetGame() {
  await open();
  await request(store("game", "readwrite").clear());
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

globalThis.mm2fs = { read, list, kind, write, remove };
