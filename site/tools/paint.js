// The skin painter: paint a bike or a rider in 3D (or on the flat picture
// beside it) and save it as a skin the game picks up: a .tga for the
// game's BIKES or RIDERS folder, kept for the browser version too, or a
// .png for the Free Pack's bikes and riders.

import * as mm from "./pkg/mm2_webtools.js";
import { Viewer, save, pngOf, textureOf, THREE } from "./scene3d.js";
import { gameRoster, paintTexture, gameAnimated, packRoster, packModel, packTexture } from "./models.js";

const el = (tag, props = {}, ...kids) => {
  const e = Object.assign(document.createElement(tag), props);
  e.append(...kids);
  return e;
};
const row = (label, title, input) => el("label", { className: "row", title }, label, input);
const option = (value, text) => el("option", { value, textContent: text });
const SWATCHES = ["#f4f1ea", "#151515", "#c8202b", "#e4572e", "#f2b705", "#a3c24a", "#1e8c45", "#1b9aaa", "#1f4e9c", "#14213d", "#6c3fa0", "#8a8f98"];

// The browser version keeps players' files in its own storage; skins
// saved there go into the game's BIKES and RIDERS folders when it starts.
async function keepForBrowser(path, bytes) {
  const r = indexedDB.open("openmcm2", 2);
  r.onupgradeneeded = () => {
    for (const name of ["user", "game", "pack"]) if (!r.result.objectStoreNames.contains(name)) r.result.createObjectStore(name);
  };
  const db = await new Promise((ok, fail) => ((r.onsuccess = () => ok(r.result)), (r.onerror = () => fail(r.error))));
  await new Promise((ok, fail) => {
    const t = db.transaction("user", "readwrite");
    t.objectStore("user").put(bytes, path);
    t.oncomplete = ok;
    t.onerror = () => fail(t.error);
  });
  db.close();
}

export async function mount(app, panel) {
  const { lib } = app;
  const box = panel.querySelector("#paint-controls");
  const canvas = panel.querySelector("#paint-view");
  const hintEl = panel.querySelector("#paint-hint");
  const viewer = new Viewer(canvas);
  let target = null; // { obj, material, kind, ... }
  let paper = null; // the canvas being painted
  let original = null; // the picture painting started from
  let texture = null;
  let undo = [];
  let redo = [];
  let startWith = null;
  const brush = { tool: "brush", color: "#c8202b", size: 12, opacity: 1, soft: 0.3, mirror: false, stamp: null };
  let turning = false;

  // ----- painting -----
  function dab(u, v) {
    const ctx = paper.getContext("2d");
    const W = paper.width;
    const H = paper.height;
    u = ((u % 1) + 1) % 1;
    v = ((v % 1) + 1) % 1;
    const x = u * W;
    const y = v * H;
    const r = (brush.size / 512) * W;
    ctx.save();
    if (brush.tool === "erase") {
      // Back to the picture painting started from.
      ctx.beginPath();
      ctx.arc(x, y, r, 0, Math.PI * 2);
      ctx.clip();
      ctx.globalAlpha = brush.opacity;
      ctx.drawImage(original, 0, 0);
    } else if (brush.tool === "stamp" && brush.stamp) {
      const s = brush.stamp;
      const w = r * 2;
      const h = (w * s.height) / s.width;
      ctx.globalAlpha = brush.opacity;
      ctx.drawImage(s, x - w / 2, y - h / 2, w, h);
    } else {
      const g = ctx.createRadialGradient(x, y, 0, x, y, r);
      const c = brush.color;
      g.addColorStop(0, c);
      g.addColorStop(Math.max(0.01, 1 - brush.soft), c);
      g.addColorStop(1, c + "00");
      ctx.globalAlpha = brush.opacity;
      ctx.fillStyle = g;
      ctx.beginPath();
      ctx.arc(x, y, r, 0, Math.PI * 2);
      ctx.fill();
    }
    ctx.restore();
    dirty = true;
  }
  // Fills the patch of like colour around (u, v), as a paint bucket.
  function fill(u, v) {
    const ctx = paper.getContext("2d");
    const W = paper.width;
    const H = paper.height;
    const x0 = Math.floor((((u % 1) + 1) % 1) * W);
    const y0 = Math.floor((((v % 1) + 1) % 1) * H);
    const img = ctx.getImageData(0, 0, W, H);
    const d = img.data;
    const at = (x, y) => (y * W + x) * 4;
    const s = at(x0, y0);
    const [r0, g0, b0] = [d[s], d[s + 1], d[s + 2]];
    const [r1, g1, b1] = [1, 3, 5].map((i) => parseInt(brush.color.slice(i, i + 2), 16));
    const tol = 40 + brush.soft * 80;
    const seen = new Uint8Array(W * H);
    const stack = [[x0, y0]];
    const a = brush.opacity;
    while (stack.length) {
      const [x, y] = stack.pop();
      if (x < 0 || y < 0 || x >= W || y >= H || seen[y * W + x]) continue;
      seen[y * W + x] = 1;
      const i = at(x, y);
      if (Math.abs(d[i] - r0) + Math.abs(d[i + 1] - g0) + Math.abs(d[i + 2] - b0) > tol) continue;
      // Keeps the shading: the new colour times how light the old was.
      const light = (d[i] + d[i + 1] + d[i + 2]) / Math.max(1, r0 + g0 + b0);
      d[i] = d[i] * (1 - a) + Math.min(255, r1 * light) * a;
      d[i + 1] = d[i + 1] * (1 - a) + Math.min(255, g1 * light) * a;
      d[i + 2] = d[i + 2] * (1 - a) + Math.min(255, b1 * light) * a;
      stack.push([x + 1, y], [x - 1, y], [x, y + 1], [x, y - 1]);
    }
    ctx.putImageData(img, 0, 0);
    dirty = true;
  }
  function pickColour(u, v) {
    const x = Math.floor((((u % 1) + 1) % 1) * paper.width);
    const y = Math.floor((((v % 1) + 1) % 1) * paper.height);
    const p = paper.getContext("2d").getImageData(x, y, 1, 1).data;
    brush.color = "#" + [p[0], p[1], p[2]].map((c) => c.toString(16).padStart(2, "0")).join("");
    colourInput.value = brush.color;
  }
  function remember() {
    undo.push(paper.getContext("2d").getImageData(0, 0, paper.width, paper.height));
    if (undo.length > 30) undo.shift();
    redo = [];
  }
  let dirty = false;
  viewer.onFrame.push(() => {
    if (dirty && texture) {
      texture.needsUpdate = true;
      dirty = false;
      flat.getContext("2d").drawImage(paper, 0, 0, flat.width, flat.height);
      if (showUv.checked) drawUv();
    }
  });

  // Where the pointer meets the paintable surface: its picture spot.
  const ray = new THREE.Raycaster();
  const meshes = () => {
    const list = [];
    target?.obj.traverse((o) => o.isMesh && o.material === target.material && list.push(o));
    return list;
  };
  function hitAt(e) {
    const r = canvas.getBoundingClientRect();
    const p = new THREE.Vector2(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1);
    ray.setFromCamera(p, viewer.camera);
    return ray.intersectObjects(meshes(), false)[0] || null;
  }
  // The same spot on the other side (the model's left and right mirror
  // in x): found by looking at the mirrored point from outside.
  function mirrorHit(hit) {
    const root = target.obj;
    root.updateMatrixWorld(true);
    const inv = root.matrixWorld.clone().invert();
    const p = hit.point.clone().applyMatrix4(inv);
    const n = hit.face.normal.clone().transformDirection(hit.object.matrixWorld).transformDirection(inv);
    p.x = -p.x;
    n.x = -n.x;
    const from = p.clone().addScaledVector(n, 0.6).applyMatrix4(root.matrixWorld);
    const dir = n.clone().negate().transformDirection(root.matrixWorld);
    ray.set(from, dir);
    const h = ray.intersectObjects(meshes(), false)[0];
    return h && h.distance < 1.5 ? h : null;
  }
  function apply(hit, first) {
    if (!hit?.uv) return;
    if (brush.tool === "pick") return pickColour(hit.uv.x, hit.uv.y);
    if (brush.tool === "fill") {
      if (!first) return;
      remember();
      fill(hit.uv.x, hit.uv.y);
      if (brush.mirror) {
        const m = mirrorHit(hit);
        if (m?.uv) fill(m.uv.x, m.uv.y);
      }
      return;
    }
    if (first) remember();
    dab(hit.uv.x, hit.uv.y);
    if (brush.mirror) {
      const m = mirrorHit(hit);
      if (m?.uv) dab(m.uv.x, m.uv.y);
    }
  }
  let painting = false;
  let last = null;
  canvas.addEventListener("pointerdown", (e) => {
    if (!target || turning || e.button !== 0) return;
    const hit = hitAt(e);
    if (!hit) return; // off the model: turn it instead
    e.stopImmediatePropagation();
    viewer.controls.enabled = false;
    painting = true;
    canvas.setPointerCapture(e.pointerId);
    apply(hit, true);
    last = [e.clientX, e.clientY];
  }, true);
  canvas.addEventListener("pointermove", (e) => {
    if (!painting) return;
    // Fills in between quick moves so strokes stay unbroken.
    const steps = Math.max(1, Math.ceil(Math.hypot(e.clientX - last[0], e.clientY - last[1]) / 4));
    for (let i = 1; i <= steps; i++) {
      const t = i / steps;
      apply(hitAt({ clientX: last[0] + (e.clientX - last[0]) * t, clientY: last[1] + (e.clientY - last[1]) * t }), false);
    }
    last = [e.clientX, e.clientY];
  });
  const end = () => {
    painting = false;
    viewer.controls.enabled = true;
  };
  canvas.addEventListener("pointerup", end);
  canvas.addEventListener("pointercancel", end);
  addEventListener("keydown", (e) => {
    if (panel.hidden || e.target.closest?.("input, select, textarea")) return;
    if (e.code === "Space") (turning = true), e.preventDefault();
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z") (e.shiftKey ? redoOne() : undoOne()), e.preventDefault();
  });
  addEventListener("keyup", (e) => e.code === "Space" && (turning = false));

  // ----- the controls -----
  const targetSel = el("select");
  const startSel = el("select");
  const colourInput = el("input", { type: "color", value: brush.color, title: "The colour to paint with." });
  const size = el("input", { type: "range", min: "2", max: "120", value: String(brush.size) });
  const opacity = el("input", { type: "range", min: "0.05", max: "1", step: "0.05", value: "1" });
  const soft = el("input", { type: "range", min: "0", max: "1", step: "0.05", value: String(brush.soft) });
  const mirror = el("input", { type: "checkbox" });
  const showUv = el("input", { type: "checkbox" });
  const nameInput = el("input", { type: "text", value: "MYSKIN", maxLength: 8, spellcheck: false, title: "The skin's name, up to 8 letters or numbers. The game lists it by this name." });
  nameInput.style.cssText = "font:inherit;padding:7px 8px;background:#24221e;color:var(--fg);border:1px solid var(--line);border-radius:4px";
  const stampInput = el("input", { type: "file", accept: "image/*", hidden: true });
  const tools = el("div", { className: "seg", title: "What a click or drag on the model does." });
  const toolButtons = [
    ["brush", "Brush", "Paints with the colour."],
    ["fill", "Fill", "Fills the patch of like colour you click, keeping its shading. Softness sets how alike."],
    ["stamp", "Sticker", "Puts a picture of your own where you click (pick one first). Size sets how big."],
    ["erase", "Rub out", "Brushes back the paint you started from."],
    ["pick", "Pick", "Takes the colour from where you click."],
  ].map(([k, label, title]) => {
    const b = el("button", { textContent: label, title, ariaPressed: String(brush.tool === k) });
    b.onclick = () => {
      if (k === "stamp" && !brush.stamp) return stampInput.click();
      brush.tool = k;
      toolButtons.forEach((x) => (x.ariaPressed = String(x === b)));
    };
    return b;
  });
  tools.append(...toolButtons);
  stampInput.onchange = async () => {
    const f = stampInput.files[0];
    if (!f) return;
    brush.stamp = await createImageBitmap(f);
    brush.tool = "stamp";
    toolButtons.forEach((x) => (x.ariaPressed = String(x.textContent === "Sticker")));
  };
  const swatches = el("div", { className: "swatches", title: "Quick colours." });
  for (const c of SWATCHES) {
    const b = el("button", { title: c });
    b.style.background = c;
    b.onclick = () => ((brush.color = c), (colourInput.value = c), brush.tool === "pick" && toolButtons[0].click());
    swatches.append(b);
  }
  colourInput.oninput = () => (brush.color = colourInput.value);
  size.oninput = () => (brush.size = +size.value);
  opacity.oninput = () => (brush.opacity = +opacity.value);
  soft.oninput = () => (brush.soft = +soft.value);
  mirror.onchange = () => (brush.mirror = mirror.checked);
  const flat = el("canvas", { width: 512, height: 512, title: "The flat picture the model wears. You can paint here too." });
  const uvLayer = el("canvas", { width: 512, height: 512 });
  uvLayer.style.cssText = "position:absolute;inset:0;width:100%;height:100%;pointer-events:none";
  const flatWrap = el("div", { className: "uvwrap" }, flat, uvLayer);
  flatWrap.style.position = "relative";
  showUv.onchange = () => {
    uvLayer.getContext("2d").clearRect(0, 0, 512, 512);
    if (showUv.checked) drawUv();
  };
  function drawUv() {
    const ctx = uvLayer.getContext("2d");
    ctx.clearRect(0, 0, 512, 512);
    ctx.strokeStyle = "rgba(163,194,74,0.7)";
    ctx.lineWidth = 0.6;
    ctx.beginPath();
    for (const m of meshes()) {
      const uv = m.geometry.attributes.uv;
      const idx = m.geometry.index;
      if (!uv) continue;
      const n = idx ? idx.count : uv.count;
      for (let i = 0; i < n; i += 3) {
        for (let k = 0; k <= 3; k++) {
          const v = idx ? idx.getX(i + (k % 3)) : i + (k % 3);
          const x = (((uv.getX(v) % 1) + 1) % 1) * 512;
          const y = (((uv.getY(v) % 1) + 1) % 1) * 512;
          k ? ctx.lineTo(x, y) : ctx.moveTo(x, y);
        }
      }
    }
    ctx.stroke();
  }
  // Painting on the flat picture.
  let flatPainting = false;
  const flatUv = (e) => {
    const r = flat.getBoundingClientRect();
    return { uv: new THREE.Vector2((e.clientX - r.left) / r.width, (e.clientY - r.top) / r.height) };
  };
  flat.addEventListener("pointerdown", (e) => {
    if (!paper) return;
    flatPainting = true;
    flat.setPointerCapture(e.pointerId);
    const m = brush.mirror;
    brush.mirror = false;
    apply(flatUv(e), true);
    brush.mirror = m;
  });
  flat.addEventListener("pointermove", (e) => {
    if (!flatPainting || brush.tool === "fill") return;
    const m = brush.mirror;
    brush.mirror = false;
    apply(flatUv(e), false);
    brush.mirror = m;
  });
  flat.addEventListener("pointerup", () => (flatPainting = false));
  flat.style.touchAction = "none";

  const undoOne = () => {
    if (!undo.length) return;
    redo.push(paper.getContext("2d").getImageData(0, 0, paper.width, paper.height));
    paper.getContext("2d").putImageData(undo.pop(), 0, 0);
    dirty = true;
  };
  const redoOne = () => {
    if (!redo.length) return;
    undo.push(paper.getContext("2d").getImageData(0, 0, paper.width, paper.height));
    paper.getContext("2d").putImageData(redo.pop(), 0, 0);
    dirty = true;
  };
  const note = el("p", { className: "hint" });
  const saveBtn = el("button", { className: "primary" });
  const keepBtn = el("button", { textContent: "Use it in the browser version", title: "Keeps the skin in this browser, so the browser version's Bike & Rider page lists it (with your own game picked there)." });
  const pngBtn = el("button", { textContent: "Save as .png", title: "Saves the flat picture as a .png, to keep working on it in another paint program." });
  const loadBtn = el("button", { textContent: "Open a picture…", title: "Puts a picture of your own (.png, .jpg or .tga, laid out like the skin) on the model to carry on painting." });
  const openInput = el("input", { type: "file", accept: "image/*,.tga", hidden: true });

  function sets() {
    const list = [];
    if (lib.has("game")) {
      list.push(["game-bike", "Your own bike paint (the game's Yamaha frame)"]);
      list.push(["game-rider", "Your own rider outfit"]);
    }
    const pack = lib.has("pack") ? packRoster(lib) : null;
    for (const b of pack?.bikes || []) list.push(["pack-bike:" + b.shape, `Free Pack bike: ${b.shape === "mx2" ? "Ridgeback" : b.shape === "two" ? "Kestrel" : b.shape}`]);
    if (pack?.rider) list.push(["pack-rider", "Free Pack rider kit"]);
    return list;
  }

  async function build() {
    box.replaceChildren();
    const list = sets();
    if (!list.length) {
      box.append(el("p", { className: "hint", textContent: "Pick your game or the Free Pack first (at the top of the page)." }));
      return;
    }
    targetSel.replaceChildren(...list.map(([v, t]) => option(v, t)));
    box.append(
      row("Paint", "What to paint. Your own bike paints go on the game's Yamaha frame, as the game does for paints in its BIKES folder.", targetSel),
      row("Start from", "The paint or outfit to start from.", startSel),
      tools,
      el("div", { className: "buttons" }, colourInput, swatches),
      row("Size", "How big the brush or sticker is.", size),
      row("Strength", "How strongly each stroke covers.", opacity),
      row("Softness", "How soft the brush's edge is (for Fill: how alike the colours it fills must be).", soft),
      el("label", { className: "check", title: "Paints both sides at once, left and right." }, mirror, "Paint both sides"),
      el("div", { className: "buttons" },
        el("button", { textContent: "Undo", title: "Takes back the last stroke (Ctrl+Z).", onclick: undoOne }),
        el("button", { textContent: "Redo", title: "Puts back the stroke you took back (Ctrl+Shift+Z).", onclick: redoOne }),
        el("button", { textContent: "Start again", title: "Goes back to the paint you started from.", onclick: () => { remember(); paper.getContext("2d").drawImage(original, 0, 0); dirty = true; } })),
      flatWrap,
      el("label", { className: "check", title: "Shows where the model's parts sit on the flat picture." }, showUv, "Show the layout"),
      row("Name", "The skin's name, up to 8 letters or numbers.", nameInput),
      el("div", { className: "buttons" }, saveBtn, keepBtn, pngBtn, loadBtn),
      note,
      stampInput,
      openInput
    );
    if (startWith && list.some(([v]) => v === startWith.set)) targetSel.value = startWith.set;
    await setTarget();
  }

  async function setTarget() {
    const value = targetSel.value;
    viewer.clear();
    target = null;
    startSel.replaceChildren();
    const isGame = value.startsWith("game-");
    keepBtn.hidden = !isGame;
    if (isGame) {
      const roster = await gameRoster(lib);
      const bike = value === "game-bike";
      const items = bike ? roster.bikes.filter((b) => b.model.toLowerCase() === "yamaha.mcf" || b.own) : roster.outfits;
      // Any bike's paint fits the Yamaha frame only if made for it.
      items.forEach((it, i) => startSel.append(option(i, it.name)));
      startSel.append(option("white", "Plain white"));
      if (startWith?.file) {
        const i = items.findIndex((it) => (it.paint || it.file || "").toLowerCase() === startWith.file.toLowerCase());
        if (i >= 0) startSel.value = String(i);
      }
      const pickStart = async () => {
        const it = items[+startSel.value];
        const tex = it ? await paintTexture(lib, it, [roster.bikeRes]) : null;
        const img = tex?.image;
        const w = img?.width || 256;
        const h = img?.height || 256;
        const start = el("canvas", { width: w, height: h });
        const ctx = start.getContext("2d");
        if (img) ctx.drawImage(img, 0, 0);
        else (ctx.fillStyle = "#f4f1ea"), ctx.fillRect(0, 0, w, h);
        await usePicture(start);
        if (!target) {
          const m = await gameAnimated(lib, bike ? "yamaha.mcf" : "Rider.mcf", { skin: texture, prefix: "p_", plate: bike ? 7 : 0 });
          if (!m) throw new Error("the model isn't in this game");
          const mat = m.obj.userData.materials.get(0);
          target = { obj: m.obj, material: mat, kind: bike ? "bike" : "rider", game: true };
          viewer.show(m.obj);
          viewer.frame(m.obj, new THREE.Vector3(1.2, 0.4, 0.9));
        } else {
          target.material.map = texture;
          target.material.needsUpdate = true;
        }
        saveBtn.textContent = "Save as .tga";
        saveBtn.title = bike
          ? "Saves the paint as a .tga. Put it in your game's BIKES folder: it shows on the Bike & Rider page as a (User Map) bike, in OpenMCM2 and the original game alike."
          : "Saves the outfit as a .tga. Put it in your game's RIDERS folder: it shows in the outfits on the Bike & Rider page, in OpenMCM2 and the original game alike.";
        note.textContent = bike
          ? "Saved paints go in your game's BIKES folder, and show as a (User Map) bike."
          : "Saved outfits go in your game's RIDERS folder, and show in the outfits list.";
      };
      startSel.onchange = () => pickStart().catch(fail);
      await pickStart();
    } else {
      const pack = packRoster(lib);
      const bike = value.startsWith("pack-bike:");
      const b = bike && pack.bikes.find((x) => x.shape === value.split(":")[1]);
      const colours = bike ? b.colours : pack.rider.colours;
      colours.forEach((c) => startSel.append(option(c, c[0].toUpperCase() + c.slice(1))));
      startSel.value = colours.includes("white") ? "white" : colours[0];
      const model = await packModel(lib, bike ? b.glb : pack.rider.glb);
      const pickStart = async () => {
        const path = bike ? b.livery(startSel.value) : pack.rider.outfit(startSel.value);
        const tex = await packTexture(lib, path);
        const start = el("canvas", { width: tex.image.width, height: tex.image.height });
        start.getContext("2d").drawImage(tex.image, 0, 0);
        await usePicture(start);
        model.setPaint(texture);
      };
      await pickStart();
      target = { obj: model.obj, material: model.paint, kind: bike ? "bike" : "rider", pack: true, shape: b?.shape };
      viewer.show(model.obj);
      viewer.frame(model.obj, new THREE.Vector3(1.2, 0.4, 0.9));
      startSel.onchange = () => pickStart().catch(fail);
      saveBtn.textContent = "Save as .png";
      saveBtn.title = "Saves the paint as a .png with the name the Free Pack uses, ready to put in the desktop version's FreePack folder.";
      const file = bike ? `bikes/${b.shape}/fp_livery_${b.shape}_white.png` : "riders/hq/fp_rider_hq_white.png";
      note.textContent = `For the desktop version: put it in your FreePack folder as ${file} (in place of White), then pick White on the Kit & Paint page. A Free Pack update puts the original back, so keep a copy.`;
    }
    if (showUv.checked) drawUv();
  }

  async function usePicture(start) {
    original = start;
    paper = el("canvas", { width: start.width, height: start.height });
    paper.getContext("2d", { willReadFrequently: true });
    paper.getContext("2d").drawImage(start, 0, 0);
    texture = textureOf(paper, "paint");
    texture.flipY = false;
    undo = [];
    redo = [];
    dirty = true;
  }
  const fail = (e) => {
    console.warn(e);
    note.textContent = `Something went wrong: ${e.message}`;
    note.classList.add("err");
  };
  const name = () => (nameInput.value.replace(/[^A-Za-z0-9_-]/g, "").slice(0, 8) || "MYSKIN").toUpperCase();
  function rgba() {
    const d = paper.getContext("2d").getImageData(0, 0, paper.width, paper.height).data;
    return new Uint8Array(d.buffer.slice(0));
  }
  saveBtn.onclick = async () => {
    if (!paper) return;
    if (target?.game) save(new Blob([mm.encode_tga(paper.width, paper.height, rgba())]), name() + ".TGA");
    else {
      const file = target.kind === "bike" ? `fp_livery_${target.shape}_white.png` : "fp_rider_hq_white.png";
      save(await pngOf(paper), file);
    }
  };
  keepBtn.onclick = async () => {
    if (!paper || !target?.game) return;
    const folder = target.kind === "bike" ? "BIKES" : "RIDERS";
    try {
      await keepForBrowser(`/user/Skins/${folder}/${name()}.TGA`, mm.encode_tga(paper.width, paper.height, rgba()));
      note.classList.remove("err");
      note.textContent = `Kept as ${name()} for the browser version. Open it with your own game and it's on the Bike & Rider page.`;
    } catch (e) {
      fail(e);
    }
  };
  pngBtn.onclick = async () => paper && save(await pngOf(paper), name().toLowerCase() + ".png");
  loadBtn.onclick = () => openInput.click();
  openInput.onchange = async () => {
    const f = openInput.files[0];
    if (!f || !paper) return;
    remember();
    const ctx = paper.getContext("2d");
    if (/\.tga$/i.test(f.name)) {
      const pic = mm.targa(new Uint8Array(await f.arrayBuffer()));
      const c = el("canvas", { width: pic.width, height: pic.height });
      const d = pic.rgba();
      c.getContext("2d").putImageData(new ImageData(new Uint8ClampedArray(d.buffer, d.byteOffset, d.length), pic.width, pic.height), 0, 0);
      ctx.drawImage(c, 0, 0, paper.width, paper.height);
    } else ctx.drawImage(await createImageBitmap(f), 0, 0, paper.width, paper.height);
    dirty = true;
    openInput.value = "";
  };
  targetSel.onchange = () => {
    startWith = null;
    setTarget().catch(fail);
  };

  let stale = false;
  lib.onChange(() => (panel.hidden ? (stale = true) : build().catch(fail)));
  new MutationObserver(() => !panel.hidden && stale && ((stale = false), build().catch(fail))).observe(panel, { attributes: true, attributeFilter: ["hidden"] });
  await build().catch(fail);
  return {
    // From the file browser: start from a paint or outfit by its name.
    async startFrom(file) {
      const roster = lib.has("game") ? await gameRoster(lib) : null;
      const isBike = roster?.bikes.some((b) => b.paint?.toLowerCase() === file.toLowerCase());
      startWith = { set: isBike ? "game-bike" : "game-rider", file };
      await build().catch(fail);
    },
  };
}
