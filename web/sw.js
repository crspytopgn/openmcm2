// Keeps the browser version's own files (the page, its scripts, the game
// engine) in this browser, so it starts with no internet once it has been
// played. Your game files, the Free Pack and your saves are kept by the
// page itself (mm2fs.js); this only looks after the page.
//
// - The page and version.json come from the internet first, so an update
//   shows straight away, and from here only when there's no connection
//   (both together, so a kept page never meets a newer build's files).
// - Files fetched for one build (`?v=<version>` in the address) never
//   change, so they come from here once kept. When version.json names a
//   new build, the old build's files are let go.
// - The site's pictures and fonts come from here, refreshed behind.
// - Anything from another site (the Free Pack, races, leaderboards) is
//   left alone.

const PAGE = "mm2-page";
const BUILD = "mm2-build-";

self.addEventListener("install", () => self.skipWaiting());
self.addEventListener("activate", (e) => e.waitUntil(self.clients.claim()));

self.addEventListener("fetch", (e) => {
  const req = e.request;
  if (req.method !== "GET" || req.headers.has("range")) return;
  const url = new URL(req.url);
  if (url.origin !== location.origin) return;
  if (req.mode === "navigate") return e.respondWith(page(req, new URL("./", location.href).href));
  if (url.pathname.endsWith("/version.json")) return e.respondWith(versionFile(req));
  const v = url.searchParams.get("v");
  if (v) return e.respondWith(build(req, v));
  e.respondWith(refreshed(req));
});

async function keep(cache, key, res) {
  if (res.ok && res.type === "basic") await (await caches.open(cache)).put(key, res.clone()).catch(() => {});
  return res;
}

// The internet first; the kept copy when there's no connection.
async function page(req, key) {
  try {
    return await keep(PAGE, key, await fetch(req));
  } catch (err) {
    const kept = await caches.match(key, { cacheName: PAGE });
    if (kept) return kept;
    throw err;
  }
}

async function versionFile(req) {
  const key = new URL("version.json", location.href).href;
  try {
    const res = await fetch(req);
    if (res.ok) {
      await keep(PAGE, key, res);
      const { version } = await res.clone().json();
      if (version) await letGo(version);
    }
    return res;
  } catch (err) {
    const kept = await caches.match(key, { cacheName: PAGE });
    if (kept) return kept;
    throw err;
  }
}

// One build's files: they never change.
async function build(req, v) {
  const name = BUILD + v;
  const kept = await caches.match(req, { cacheName: name });
  return kept || keep(name, req, await fetch(req));
}

// Older builds' files, once a newer build is out.
async function letGo(version) {
  for (const name of await caches.keys())
    if (name.startsWith(BUILD) && name !== BUILD + version) await caches.delete(name);
}

// The kept copy straight away, and a fresh one fetched for next time.
async function refreshed(req) {
  const kept = await caches.match(req, { cacheName: PAGE });
  const net = fetch(req).then((res) => keep(PAGE, req, res));
  if (kept) {
    net.catch(() => {});
    return kept;
  }
  return net;
}
