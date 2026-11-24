// The Tools page: picks the files, switches the tabs and starts each tool
// the first time it's opened.

import init from "./pkg/mm2_webtools.js";
import { Library, readPicked } from "./library.js";
import * as zip from "../play/zip.js";

const $ = (id) => document.getElementById(id);
const MB = (n) => (n / 1048576).toFixed(n < 10485760 ? 1 : 0);
const status = (text, bad = false) => {
  $("status").textContent = text;
  $("status").classList.toggle("err", bad);
};

await init();
const lib = new Library();
export const app = { lib, tabs: {}, show: null };

function describe() {
  const parts = [];
  if (lib.has("game")) parts.push(`your game (${lib.files("game").length} files)`);
  if (lib.has("pack")) parts.push(`the Free Pack ${lib.packVersion || ""} (each file comes down when you open it)`.replace("  ", " "));
  if (lib.has("mine")) parts.push(`${lib.files("mine").length} of your own tracks`);
  status(parts.length ? "Looking at " + parts.join(", ") + "." : "Nothing picked yet. Pick your game, or use the Free Pack if you don't have the game.");
}
lib.onChange(describe);

async function usePicked(files) {
  if (!files.length) return;
  status("Reading your files…");
  try {
    const { game, pack, loose } = await readPicked([...files]);
    if (game.length) lib.setRoot("game", game);
    if (pack.length) lib.setRoot("pack", pack);
    const tracks = loose.filter(([p]) => /\.env$/i.test(p));
    if (tracks.length) lib.addToRoot("mine", tracks.map(([p, x]) => ["TERAFORM/" + p, x]));
    if (!game.length && !pack.length && !tracks.length)
      status("Those files don't look like the game, a CD of it or the Free Pack. Pick the folder with RES, TERAFORM and UI in it.", true);
  } catch (err) {
    console.warn(err);
    status(`Couldn't read those files: ${err.message}`, true);
  }
}
$("folder").addEventListener("change", (e) => usePicked(e.target.files).then(() => (e.target.value = "")));
$("file").addEventListener("change", (e) => usePicked(e.target.files).then(() => (e.target.value = "")));
$("pack").addEventListener("click", async () => {
  status("Getting the Free Pack's list of files…");
  try {
    await lib.usePackOnline();
  } catch (err) {
    status(`Couldn't reach the Free Pack: ${err.message}. Check your connection and try again.`, true);
  }
});
$("trackfile").addEventListener("change", async (e) => {
  const added = [];
  for (const f of e.target.files) {
    try {
      if (/\.env$/i.test(f.name)) added.push(["TERAFORM/" + f.name, { size: f.size, arrayBuffer: () => f.arrayBuffer() }]);
      else
        for (const [p, x] of await zip.entries(f))
          if (/\.env$/i.test(p)) added.push(["TERAFORM/" + p.split("/").pop(), x]);
    } catch (err) {
      status(`${f.name}: ${err.message}`, true);
    }
  }
  e.target.value = "";
  if (added.length) {
    lib.addToRoot("mine", added);
    (await app.show("track")).openTrack("mine", added[0][0]);
  } else status("No track (.env) found in that file.", true);
});
if (await lib.hasKeptGame()) {
  $("kept").hidden = false;
  $("kept").addEventListener("click", async () => {
    status("Reading the game kept in this browser…");
    if (!(await lib.useKeptGame())) status("The game kept in this browser couldn't be read. Pick it again above.", true);
  });
}
// Tracks added in the browser version show here too.
lib.keptTracks().then((t) => t.length && lib.addToRoot("mine", t));

// Tabs: each tool loads the first time it's shown.
const loaders = {
  files: () => import("./files.js"),
  garage: () => import("./garage.js"),
  track: () => import("./track.js"),
  paint: () => import("./paint.js"),
  builder: () => import("./builder.js"),
};
app.show = async (name) => {
  for (const b of document.querySelectorAll(".tabs button[data-tab]")) b.setAttribute("aria-selected", String(b.dataset.tab === name));
  for (const p of document.querySelectorAll(".panel")) p.hidden = p.id !== "tab-" + name;
  if (!app.tabs[name]) {
    const mod = await loaders[name]();
    app.tabs[name] = await mod.mount(app, $("tab-" + name));
  }
  history.replaceState(null, "", "#" + name);
  return app.tabs[name];
};
for (const b of document.querySelectorAll(".tabs button[data-tab]")) b.addEventListener("click", () => app.show(b.dataset.tab));

// Full screen for the open tool (the Course Builder has its own button
// over its view): the browser's own where it can, else the page filled.
let fsBox = null;
const exitBtn = Object.assign(document.createElement("button"), { className: "tbtn fsexit", textContent: "Exit full screen", hidden: true, title: "Back to the page (Esc)." });
document.body.append(exitBtn);
function fullScreen(on) {
  if (!on && fsBox) {
    fsBox.classList.remove("fsmax");
    document.body.classList.remove("qb-maxed");
    exitBtn.hidden = true;
    fsBox = null;
    if (document.fullscreenElement) document.exitFullscreen().catch(() => {});
  } else if (on) {
    const panel = document.querySelector(".panel:not([hidden])");
    if (panel?.id === "tab-builder") return panel.querySelector("#qb-fs")?.click();
    fsBox = panel?.querySelector(".side, .split");
    if (!fsBox) return;
    fsBox.classList.add("fsmax");
    document.body.classList.add("qb-maxed");
    exitBtn.hidden = false;
    fsBox.append(exitBtn);
    fsBox.requestFullscreen?.().catch(() => {});
  }
  window.dispatchEvent(new Event("resize"));
}
$("fullscreen").addEventListener("click", () => fullScreen(true));
exitBtn.addEventListener("click", () => fullScreen(false));
document.addEventListener("fullscreenchange", () => !document.fullscreenElement && fsBox && fullScreen(false));
document.addEventListener("keydown", (e) => e.key === "Escape" && fsBox && fullScreen(false));
app.show(loaders[location.hash.slice(1)] ? location.hash.slice(1) : "files");
