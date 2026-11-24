// The Course Builder tab. The builder is part of the game, so this picks
// what to build with and opens the browser version straight into it
// (../play/?course-builder=mm2|demo|free). Files picked here are saved
// where the browser version keeps them, so it starts with them at once.

import * as fs from "../play/mm2fs.js";

const DEMO_PATH = "/demo/MCM2PCG.exe";
const open = (how) => location.assign(`../play/?course-builder=${how}`);

export async function mount(app, panel) {
  const $ = (id) => panel.querySelector("#" + id);
  const status = (text, bad = false) => {
    $("b-status").textContent = text;
    $("b-status").classList.toggle("err", bad);
  };

  // What the browser version has saved.
  const kept = await fs.keptGame().catch(() => null);
  const demo = !!kept?.some(([p]) => p === DEMO_PATH);
  const game = !!kept && !demo;
  const pack = !!((await fs.keptPack().catch(() => null)) || (await fs.packGet("manifest").catch(() => null)));
  const saved = [game && "your MM2 game", demo && "the MM2 demo", pack && "the free version"].filter(Boolean);
  $("b-saved").textContent = saved.length
    ? `Saved in this browser: ${saved.join(" and ")}. Press its button to start building straight away.`
    : "Nothing saved in this browser yet. Pick one below and the builder opens as soon as it's ready.";
  $("b-mm2").querySelector(".bsaved").hidden = !game;
  $("b-demo").querySelector(".bsaved").hidden = !demo;
  $("b-free").querySelector(".bsaved").hidden = !pack;
  $("b-mm2-go").hidden = !game;
  if (game) {
    $("b-folder-label").textContent = "Use another folder";
    $("b-file-label").textContent = "Use another CD image or .zip";
  }

  $("b-mm2-go").addEventListener("click", () => open("mm2"));
  $("b-demo-go").addEventListener("click", () => open("demo"));
  $("b-free-go").addEventListener("click", () => open("free"));

  // A game folder: saved by its path under the folder holding UI/MAINMENU.DAT,
  // as the browser version saves it.
  $("b-folder").addEventListener("change", async (e) => {
    const files = [...e.target.files];
    e.target.value = "";
    const at = files.map((f) => /^(.*?)ui\/mainmenu\.dat$/i.exec(f.webkitRelativePath)).find(Boolean);
    if (!at) return status("That folder has no UI/MAINMENU.DAT in it. Pick the folder the game is installed in, or the CD.", true);
    const root = at[1];
    const list = files.filter((f) => f.webkitRelativePath.startsWith(root)).map((f) => ["/game/" + f.webkitRelativePath.slice(root.length), f]);
    await save(list);
  });
  // A CD image or zip: saved as itself; the browser version reads it.
  $("b-file").addEventListener("change", async (e) => {
    const file = e.target.files[0];
    e.target.value = "";
    if (file) await save([["/file", file]]);
  });
  async function save(list) {
    status("Saving your game in this browser…");
    try {
      navigator.storage?.persist?.();
      await fs.forgetGame();
      await fs.keepGame(list);
      open("mm2");
    } catch (err) {
      console.warn(err);
      status(`Couldn't save your game in this browser: ${err.message || err}. Try a normal (not private) window.`, true);
    }
  }
  return {};
}
