// Network play in the browser (crates/mm2-viewer/src/net.rs, mod web).
// Browsers connect straight to each other over WebRTC data channels; the
// free public PeerJS server only introduces them, and its STUN and TURN
// servers help through home routers. A host is reached by its race code,
// which this page shows with a link to share.

const PEERJS = "https://cdn.jsdelivr.net/npm/peerjs@1.5.4/dist/peerjs.min.js";
const PREFIX = "openmcm2-";
// No 0/O or 1/I/L, so a code read out loud comes through.
const LETTERS = "ABCDEFGHJKMNPQRSTUVWXYZ23456789";

let loading = null;
function peerjs() {
  loading ??= new Promise((ok, fail) => {
    const s = document.createElement("script");
    s.src = PEERJS;
    s.onload = () => ok(globalThis.Peer);
    s.onerror = () => fail(new Error("the matchmaking script didn't load"));
    document.head.append(s);
  });
  return loading;
}

let peer = null; // ours: the host's, or a joiner's
const links = new Map(); // number -> { conn, queue }
let nextLink = 1;
let events = [];
let foundList = [];
let banner = null;

function newCode() {
  const r = crypto.getRandomValues(new Uint8Array(6));
  return [...r].map((b) => LETTERS[b % LETTERS.length]).join("");
}

function show(text, link) {
  if (!banner) {
    banner = document.createElement("div");
    banner.id = "racecode";
    banner.style.cssText =
      "position:fixed;bottom:8px;left:50%;transform:translateX(-50%);padding:.4rem .6rem .4rem 1rem;" +
      "background:rgba(0,0,0,.8);border:1px solid #e8a33a;border-radius:8px;color:#eee;font:15px system-ui,sans-serif;z-index:5";
    document.body.append(banner);
  }
  banner.hidden = !text;
  banner.textContent = text || "";
  if (link) {
    const b = document.createElement("button");
    b.textContent = "Copy invite link";
    b.style.cssText = "margin-left:.7rem;background:#e8a33a;color:#111;border:0;border-radius:6px;padding:.3rem .7rem;font:inherit;font-weight:600";
    b.onclick = () => navigator.clipboard?.writeText(link).then(() => (b.textContent = "Copied"));
    banner.append(b);
  }
}

function bytes(d) {
  if (d instanceof ArrayBuffer) return new Uint8Array(d);
  if (ArrayBuffer.isView(d)) return new Uint8Array(d.buffer, d.byteOffset, d.byteLength);
  return new TextEncoder().encode(String(d));
}

function track(conn) {
  const n = nextLink++;
  const link = { conn, queue: [], open: false };
  links.set(n, link);
  conn.on("open", () => {
    link.open = true;
    for (const d of link.queue) conn.send(d);
    link.queue = [];
  });
  conn.on("data", (d) => events.push(["data", n, bytes(d)]));
  const gone = () => {
    if (!links.has(n)) return;
    links.delete(n);
    events.push(["gone", n]);
  };
  conn.on("close", gone);
  conn.on("error", gone);
  return n;
}

async function host(name) {
  stop();
  const Peer = await peerjs();
  const code = newCode();
  peer = new Peer(PREFIX + code.toLowerCase());
  peer.on("open", () => {
    const url = new URL(location.href);
    url.search = "?join=" + code;
    show(`Hosting "${name}". Race code: ${code}`, url.href);
  });
  peer.on("connection", (conn) => {
    // A finder asking who we are: our session's name, then goodbye.
    if (conn.metadata?.probe) {
      conn.on("open", () => {
        conn.send(new TextEncoder().encode(name));
        setTimeout(() => conn.close(), 2000);
      });
      return;
    }
    const n = track(conn);
    conn.on("open", () => events.push(["join", n]));
  });
  peer.on("error", (e) => {
    if (e.type === "unavailable-id") host(name); // that code is taken: another
    else console.warn("hosting:", e.type, e.message);
  });
}

async function client() {
  if (peer && !peer.destroyed) return peer;
  const Peer = await peerjs();
  peer = new Peer();
  await new Promise((ok) => peer.on("open", ok));
  peer.on("error", (e) => console.warn("network:", e.type, e.message));
  return peer;
}

function join(code) {
  // Numbered now, connected once the matchmaking server answers; what is
  // sent before then waits.
  show(null);
  const n = nextLink++;
  const link = { conn: null, queue: [] };
  links.set(n, link);
  client().then((p) => {
    const conn = p.connect(PREFIX + code.toLowerCase(), { reliable: true, serialization: "raw" });
    link.conn = conn;
    conn.on("open", () => {
      for (const d of link.queue) conn.send(d);
      link.queue = [];
    });
    conn.on("data", (d) => events.push(["data", n, bytes(d)]));
    const gone = () => {
      if (!links.has(n)) return;
      links.delete(n);
      events.push(["gone", n]);
    };
    conn.on("close", gone);
    conn.on("error", gone);
    p.on("error", (e) => e.type === "peer-unavailable" && gone());
  });
  return n;
}

function send(n, data) {
  const link = links.get(n);
  if (!link) return;
  const copy = new Uint8Array(data);
  if (link.conn?.open) link.conn.send(copy);
  else link.queue.push(copy);
}

function stop() {
  for (const { conn } of links.values()) conn?.close();
  links.clear();
  events = [];
  peer?.destroy();
  peer = null;
  show(null);
}

function poll() {
  const out = events;
  events = [];
  return out;
}

async function find(code) {
  const p = await client();
  const conn = p.connect(PREFIX + code.toLowerCase(), { metadata: { probe: true }, serialization: "raw" });
  conn.on("data", (d) => {
    foundList.push([code, new TextDecoder().decode(bytes(d))]);
    conn.close();
  });
}

function found() {
  return foundList;
}

globalThis.mm2net = { host, join, send, stop, poll, find, found };

// Opened from an invite link: say how to get in.
const invited = new URLSearchParams(location.search).get("join");
export function invite() {
  if (invited) show(`Invited to race ${invited}: open Multiplayer and pick the race from the list.`);
}
