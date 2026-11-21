// The file browser: every file of the game, the Free Pack and the
// player's tracks, archives opened like folders, with previews and saving
// as .glb, .png and .wav.

import * as mm from "./pkg/mm2_webtools.js";
import { ext, findIn } from "./library.js";
import { Viewer, Textures, buildModel, clipOf, loadGlb, glbOf, canvasOf, pngOf, save, THREE } from "./scene3d.js";

const ARCHIVES = ["res", "env", "dat", "tex"];
const PICTURES = ["tga", "bmp", "png", "jpg", "jpeg", "webp"];
const TEXT = ["scn", "pb", "mcf", "ini", "txt", "toml", "md", "est", "json", "cfg", "csv", "vut", "lst", "inf"];
const size = (n) => (n < 1024 ? `${n} B` : n < 1048576 ? `${(n / 1024).toFixed(0)} KB` : `${(n / 1048576).toFixed(1)} MB`);
const el = (tag, props = {}, ...kids) => {
  const e = Object.assign(document.createElement(tag), props);
  e.append(...kids);
  return e;
};
const button = (text, title, onclick) => el("button", { textContent: text, title, onclick });

export async function mount(app, panel) {
  const { lib } = app;
  const tree = panel.querySelector("#tree");
  const pane = panel.querySelector("#preview");
  const filter = panel.querySelector("#filter");
  let viewer = null;
  let stage = null;
  let selected = null;
  let token = 0;
  // Archives opened in the tree: key -> { root, path, archive }.
  const opened = new Map();

  function stage3d() {
    if (!stage) {
      stage = el("div", { className: "stage" }, el("canvas", { tabIndex: 0, ariaLabel: "Model" }));
      pane.append(stage);
      viewer = new Viewer(stage.firstChild);
    }
    viewer.clear();
    return stage;
  }

  // ----- the tree -----
  function folderTree(files) {
    const top = { dirs: new Map(), files: [] };
    for (const f of files) {
      const parts = f.path.split("/");
      let d = top;
      for (const p of parts.slice(0, -1)) {
        const k = p.toUpperCase();
        if (!d.dirs.has(k)) d.dirs.set(k, { name: p, dirs: new Map(), files: [] });
        d = d.dirs.get(k);
      }
      d.files.push(f);
    }
    return top;
  }
  function matches(name) {
    const q = filter.value.trim().toLowerCase();
    return !q || name.toLowerCase().includes(q);
  }
  function anyMatch(d) {
    if (d.files.some((f) => matches(f.path) || ARCHIVES.includes(ext(f.path)))) return true;
    for (const s of d.dirs.values()) if (anyMatch(s)) return true;
    return false;
  }
  function details(label, fill, open = false) {
    const d = el("details", {}, el("summary", { textContent: label }));
    const body = el("div");
    d.append(body);
    let filled = false;
    const doFill = () => {
      if (filled) return;
      filled = true;
      fill(body);
    };
    d.addEventListener("toggle", () => d.open && doFill());
    if (open) {
      d.open = true;
      doFill();
    }
    return d;
  }
  function fileRow(name, bytesLen, onpick, title) {
    const row = el("div", { className: "file", tabIndex: 0, title: title || name, role: "treeitem" }, el("span", { textContent: name }), el("span", { textContent: size(bytesLen) }));
    const pick = () => {
      selected?.classList.remove("on");
      selected = row;
      row.classList.add("on");
      onpick();
    };
    row.addEventListener("click", pick);
    row.addEventListener("keydown", (e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), pick()));
    return row;
  }
  function fillDir(body, root, d) {
    const q = filter.value.trim();
    for (const s of [...d.dirs.values()].sort((a, b) => a.name.localeCompare(b.name))) {
      if (q && !anyMatch(s)) continue;
      body.append(details(s.name, (b) => fillDir(b, root, s), !!q));
    }
    const files = d.files.filter((f) => matches(f.path) || (q && ARCHIVES.includes(ext(f.path)))).sort((a, b) => a.path.localeCompare(b.path));
    for (const f of files.slice(0, 3000)) {
      const name = f.path.split("/").pop();
      if (ARCHIVES.includes(ext(name))) body.append(archiveNode(root, f));
      else body.append(fileRow(name, f.size, () => show({ root, path: f.path, name, size: f.size, bytes: () => lib.read(root, f.path) })));
    }
    if (files.length > 3000) body.append(el("div", { className: "more", textContent: `…and ${files.length - 3000} more. Type in the box above to find one.` }));
  }
  function archiveNode(root, f) {
    const name = f.path.split("/").pop();
    const d = el("details", { title: `${f.path} (${size(f.size)}): an archive. Open it to see what's inside.` }, el("summary", { textContent: `${name}  ·  ${size(f.size)}` }));
    const body = el("div");
    d.append(body);
    d.addEventListener("toggle", async () => {
      if (!d.open || body.childElementCount) return;
      body.append(el("div", { className: "more", textContent: "Opening…" }));
      try {
        const archive = await lib.archive(root, f.path);
        opened.set(root + ":" + f.path, { root, path: f.path, archive });
        body.replaceChildren();
        fillArchive(body, root, f.path, archive);
      } catch (err) {
        body.replaceChildren(el("div", { className: "more", textContent: `Not an archive the readers know (${err.message}).` }));
        body.append(fileRow(name, f.size, () => show({ root, path: f.path, name, size: f.size, bytes: () => lib.read(root, f.path) })));
      }
    });
    // A track: straight to the track viewer from its row too.
    if (ext(name) === "env") {
      d.querySelector("summary").addEventListener("dblclick", () => openTrack(root, f.path));
    }
    return d;
  }
  function fillArchive(body, root, path, archive) {
    const names = archive.names();
    const sizes = archive.sizes();
    if (ext(path) === "env")
      body.append(el("div", { className: "more" }, button("Open in the track viewer", "Shows this track in 3D in the Tracks tab, where you can fly round it.", () => openTrack(root, path))));
    let shown = 0;
    names.forEach((n, i) => {
      if (!matches(n) || shown >= 5000) return;
      shown++;
      body.append(
        fileRow(n, sizes[i], () =>
          show({ root, path: path + "/" + n, name: n, size: sizes[i], archive, archivePath: path, index: i, bytes: async () => archive.at(i) })
        )
      );
    });
    if (!shown) body.append(el("div", { className: "more", textContent: "Nothing here matches." }));
  }
  function render() {
    tree.replaceChildren();
    let any = false;
    for (const root of ["game", "pack", "mine"]) {
      if (!lib.has(root)) continue;
      any = true;
      const files = lib.files(root);
      tree.append(details(`${lib.labels[root]} · ${files.length} files`, (b) => fillDir(b, root, folderTree(files)), true));
    }
    if (!any) tree.append(el("p", { className: "hint", textContent: "Pick your files above to see them here." }));
  }
  let typing = 0;
  filter.addEventListener("input", () => {
    clearTimeout(typing);
    typing = setTimeout(render, 250);
  });
  lib.onChange(render);
  render();

  function openTrack(root, path) {
    app.show("track").then((t) => t.openTrack(root, path));
  }

  // ----- previews -----
  function head(src, kind) {
    pane.replaceChildren(
      el("h3", { textContent: src.name }),
      el("p", { className: "meta", textContent: `${kind} · ${size(src.size)} · ${src.path}` })
    );
    if (stage) stage.remove();
  }
  const actions = (...buttons) => el("div", { className: "actions" }, ...buttons);
  const saveRaw = (src) => button("Save the file as it is", "Saves this file just as it is in the game, to your computer.", async () => save(new Blob([await src.bytes()]), src.name));

  async function show(src) {
    const my = ++token;
    const e = ext(src.name);
    try {
      const bytes = await src.bytes();
      if (my !== token) return;
      if (PICTURES.includes(e)) return await showPicture(src, bytes);
      if (e === "slb" || e === "slt") return await showModel(src, bytes);
      if (e === "mcf") return await showAnimated(src, bytes);
      if (e === "glb") return await showGlb(src, bytes);
      if (e === "wav") return showSound(src, bytes);
      if (e === "cub") return showSky(src, bytes);
      if (e === "mot") return showMotion(src, bytes);
      if (e === "trn") return showTerrainNote(src, bytes);
      if (TEXT.includes(e) || looksText(bytes)) return showText(src, bytes);
      showHex(src, bytes);
    } catch (err) {
      console.warn(err);
      head(src, "File");
      pane.append(el("p", { className: "err", textContent: `Couldn't show this file: ${err.message || err}` }), actions(saveRaw(src)));
    }
  }

  function looksText(b) {
    const n = Math.min(b.length, 2048);
    if (!n) return false;
    let ok = 0;
    for (let i = 0; i < n; i++) if ((b[i] >= 32 && b[i] < 127) || b[i] === 9 || b[i] === 10 || b[i] === 13) ok++;
    return ok / n > 0.95;
  }

  async function picture(src, bytes) {
    const e = ext(src.name);
    if (["png", "jpg", "jpeg", "webp", "bmp"].includes(e)) {
      const bmp = await createImageBitmap(new Blob([bytes]));
      const c = el("canvas", { width: bmp.width, height: bmp.height });
      c.getContext("2d").drawImage(bmp, 0, 0);
      return [c, "Picture"];
    }
    // Inside an archive a .tga is usually the game's own texture format.
    const tries = src.archive ? [mm.texture, mm.targa] : [mm.targa, mm.texture];
    for (const f of tries) {
      try {
        return [canvasOf(f(bytes)), f === mm.texture ? "Game texture" : "Targa picture"];
      } catch {}
    }
    throw new Error("not a picture the readers know");
  }
  async function showPicture(src, bytes) {
    const [c, kind] = await picture(src, bytes);
    head(src, `${kind}, ${c.width}×${c.height}`);
    c.className = "pic";
    c.style.width = Math.min(c.width * (c.width <= 256 ? 2 : 1), 1024) + "px";
    const stem = src.name.replace(/\.[^.]+$/, "");
    pane.append(
      actions(
        button("Save as .png", "Saves the picture as a .png on your computer, with its see-through parts kept.", async () => save(await pngOf(c), stem + ".png")),
        saveRaw(src),
        ...(src.archive
          ? [button("Paint over this", "Opens the Skin painter with this picture on the model it belongs to, if it's a bike paint or a rider outfit.", () => app.show("paint").then((p) => p.startFrom?.(src.name)))]
          : [])
      ),
      c
    );
  }

  async function texturesFor(src) {
    const extra = src.archive ? [src.archive] : [];
    return new Textures(await lib.textureArchives(extra));
  }
  async function modelBytes(src, name) {
    // A model named by an .mcf or a scene: beside it first, then the game's.
    const stem = name.replace(/^.*[\\/]/, "").replace(/\.[^.]*$/, "").trim();
    const archives = await lib.modelArchives(src.archive ? [src.archive] : []);
    for (const n of [stem + ".slt", stem + ".slb"]) {
      // Built models win, except the text ones players add over them.
      const hit = findIn(archives, n);
      if (hit && (n.endsWith(".slb") || hit[0] === src.archive)) return [hit[0].at(hit[1]), n];
    }
    const any = findIn(archives, stem + ".slt");
    return any ? [any[0].at(any[1]), stem + ".slt"] : null;
  }

  async function showModel(src, bytes) {
    const json = JSON.parse(mm.model(bytes, src.name));
    const textures = await texturesFor(src);
    const obj = buildModel(json, textures);
    obj.name = src.name.replace(/\.[^.]+$/, "");
    const tris = json.meshes.reduce((n, m) => n + m.i.length / 3, 0);
    head(src, `Model, ${json.frames.length} frames, ${tris} triangles`);
    pane.append(
      actions(
        button("Save as .glb", "Saves the model with its textures as a .glb, which Blender and most 3D programs open.", async () => save(await glbOf(obj), obj.name + ".glb")),
        saveRaw(src)
      )
    );
    if (textures.missing.size) pane.append(el("p", { className: "hint", textContent: `Textures not found: ${[...textures.missing].join(", ")}` }));
    pane.append(stage3d());
    viewer.show(obj);
    viewer.frame(obj);
  }

  async function showAnimated(src, bytes) {
    const info = JSON.parse(mm.mcf(bytes));
    const found = await modelBytes(src, info.model);
    if (!found) return showText(src, bytes, `Its model ${info.model} isn't here.`);
    const json = JSON.parse(mm.model(found[0], found[1]));
    const textures = await texturesFor(src);
    const obj = buildModel(json, textures);
    obj.name = src.name.replace(/\.[^.]+$/, "");
    const names = obj.userData.nodes.map((n) => n.name);
    const archives = await lib.modelArchives(src.archive ? [src.archive] : []);
    const clips = [];
    for (const m of info.motions) {
      const hit = findIn(archives, m.file);
      if (!hit) continue;
      try {
        const clip = clipOf(m.name, JSON.parse(mm.motion(hit[0].at(hit[1]))), m.fps, names);
        clip.userData = { looping: m.looping };
        clips.push(clip);
      } catch {}
    }
    head(src, `Animated model ${info.model}, ${clips.length} of ${info.motions.length} motions`);
    const pick = el("select", { title: "The motion to play. The game blends several of these while you ride." });
    pick.append(el("option", { value: "-1", textContent: "Standing still" }), ...clips.map((c, i) => el("option", { value: i, textContent: `${c.name} (${c.duration.toFixed(1)} s)` })));
    const playIt = () => {
      viewer.mixers = [];
      obj.userData.nodes.forEach((n, i) => {
        n.position.fromArray(json.frames[i].t);
        n.quaternion.fromArray(json.frames[i].q);
      });
      const c = clips[+pick.value];
      if (c) viewer.play(obj, c, true);
    };
    pick.addEventListener("change", playIt);
    pane.append(
      actions(
        pick,
        button("Save as .glb", "Saves the model with its textures and every motion as an animation, as a .glb for Blender and most 3D programs.", async () =>
          save(await glbOf(obj, clips), obj.name + ".glb")
        ),
        saveRaw(src)
      )
    );
    pane.append(stage3d());
    viewer.show(obj);
    viewer.frame(obj);
    if (clips.length) {
      pick.value = "0";
      playIt();
    }
  }

  async function showGlb(src, bytes) {
    const dir = src.path.split("/").slice(0, -1).join("/");
    const gltf = await loadGlb(bytes, (uri) => lib.read(src.root, (dir ? dir + "/" : "") + uri));
    head(src, `glTF model${gltf.animations.length ? `, ${gltf.animations.length} animations` : ""}`);
    pane.append(actions(button("Save as .glb", "Saves this .glb to your computer, ready for Blender and most 3D programs.", async () => save(await glbOf(gltf.scene, gltf.animations), src.name)), saveRaw(src)));
    pane.append(stage3d());
    viewer.show(gltf.scene);
    viewer.frame(gltf.scene);
    if (gltf.animations[0]) viewer.play(gltf.scene, gltf.animations[0]);
  }

  function showSound(src, bytes) {
    const wav = mm.sound(bytes);
    const v = new DataView(wav.buffer, wav.byteOffset);
    const rate = v.getUint32(24, true);
    const ch = v.getUint16(22, true);
    const secs = (wav.length - 44) / (rate * ch * 2);
    head(src, `Sound, ${secs.toFixed(1)} s, ${rate} Hz, ${ch === 1 ? "mono" : "stereo"}`);
    const blob = new Blob([wav], { type: "audio/wav" });
    const audio = el("audio", { controls: true, src: URL.createObjectURL(blob), title: "Play the sound" });
    pane.append(audio, actions(button("Save as .wav", "Saves the sound as a plain 16-bit .wav that any program plays.", () => save(blob, src.name.replace(/\.[^.]+$/, "") + ".wav")), saveRaw(src)));
  }

  function showSky(src, bytes) {
    const c = canvasOf(mm.sky(bytes));
    head(src, `Sky box, 6 faces of ${c.height}×${c.height}`);
    c.className = "pic";
    pane.append(
      actions(button("Save as .png", "Saves the sky's six sides in one .png strip: front, left, back, right, up and down.", async () => save(await pngOf(c), src.name.replace(/\.[^.]+$/, "") + ".png")), saveRaw(src)),
      c
    );
  }

  function showMotion(src, bytes) {
    const m = JSON.parse(mm.motion(bytes));
    head(src, `Motion, ${m.count} frames for ${m.tracks.length} parts`);
    pane.append(
      el("p", { className: "hint", textContent: "A motion moves the parts of a model. Open the .mcf that names it (or the Bikes & riders tab) to see it play." }),
      actions(saveRaw(src))
    );
  }

  function showTerrainNote(src) {
    head(src, "Track ground");
    pane.append(
      el("p", { className: "hint", textContent: "The track's ground: its heights and textures." }),
      actions(
        ...(src.archivePath ? [button("Open in the track viewer", "Shows this track in 3D in the Tracks tab.", () => openTrack(src.root, src.archivePath))] : []),
        saveRaw(src)
      )
    );
  }

  function showText(src, bytes, note) {
    head(src, "Text");
    const text = new TextDecoder("latin1").decode(bytes.subarray(0, 400000));
    if (note) pane.append(el("p", { className: "hint", textContent: note }));
    pane.append(actions(saveRaw(src)), el("pre", { textContent: text }));
  }

  function showHex(src, bytes) {
    head(src, "Data");
    const lines = [];
    for (let o = 0; o < Math.min(bytes.length, 1024); o += 16) {
      const row = [...bytes.subarray(o, o + 16)];
      lines.push(
        o.toString(16).padStart(6, "0") + "  " + row.map((b) => b.toString(16).padStart(2, "0")).join(" ").padEnd(48) + "  " + row.map((b) => (b >= 32 && b < 127 ? String.fromCharCode(b) : ".")).join("")
      );
    }
    pane.append(
      el("p", { className: "hint", textContent: "No preview for this kind of file yet. Here are its first bytes." }),
      actions(saveRaw(src)),
      el("pre", { textContent: lines.join("\n") })
    );
  }

  return { render };
}
