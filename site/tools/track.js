// Tracks: any track's ground, objects, trees and racing line in 3D, with
// free fly. The game's tracks, the player's (MDX Racing's, say) and the
// Free Pack's when its tracks are here.

import * as mm from "./pkg/mm2_webtools.js";
import { ext, findIn } from "./library.js";
import { Viewer, Fly, Textures, buildModel, clipOf, loadGlb, canvasOf, textureOf, glbOf, save, THREE } from "./scene3d.js";

const el = (tag, props = {}, ...kids) => {
  const e = Object.assign(document.createElement(tag), props);
  e.append(...kids);
  return e;
};
const row = (label, title, input) => el("label", { className: "row", title }, label, input);
const option = (value, text) => el("option", { value, textContent: text });
const check = (label, title, on = true) => {
  const input = el("input", { type: "checkbox", checked: on });
  return [el("label", { className: "check", title }, input, label), input];
};
const EVENTS = { NATIONAL: "Nationals", SX: "Supercross", BAJA: "Baja", ENDURO: "Enduro", QUARRIES: "Stunt", TAG: "Moto-Tag" };

export async function mount(app, panel) {
  const { lib } = app;
  const box = panel.querySelector("#track-controls");
  const canvas = panel.querySelector("#track-view");
  const viewer = new Viewer(canvas);
  viewer.grid.visible = false;
  const fly = new Fly(viewer, 80);
  const pad = panel.querySelector("#flypad");
  const hint = panel.querySelector("#track-hint");
  for (const b of pad.querySelectorAll("button")) {
    const on = (e) => (e.preventDefault(), fly.held.add(b.dataset.k));
    const off = () => fly.held.delete(b.dataset.k);
    b.addEventListener("pointerdown", on);
    b.addEventListener("pointerup", off);
    b.addEventListener("pointerleave", off);
    b.addEventListener("pointercancel", off);
  }
  let loaded = null;
  let token = 0;
  let wanted = null;

  const trackSel = el("select");
  const detail = el("select");
  [["1", "Full (slow on phones)"], ["2", "High"], ["4", "Medium"], ["8", "Low"]].forEach(([v, t]) => detail.append(option(v, t)));
  detail.value = matchMedia("(max-width: 820px)").matches ? "4" : "2";
  const [objRow, showObjects] = check("Objects", "The track's buildings, fences, signs, vehicles and other models.");
  const [treeRow, showTrees] = check("Trees and bushes", "The plants the track scatters over its ground.");
  const [lineRow, showLine] = check("Racing line", "The course riders follow, drawn in green above the ground.", false);
  const [fogRow, showFog] = check("Haze", "The track's own haze in the distance.", true);
  const mode = el("div", { className: "seg", title: "Turn round the track, or fly through it freely." });
  const orbitBtn = el("button", { textContent: "Turn", ariaPressed: "true", title: "Drag to turn round, scroll or pinch to zoom, right-drag to move." });
  const flyBtn = el("button", { textContent: "Fly", ariaPressed: "false", title: "Free fly: W A S D or the arrows to move, Q and E to sink and rise, Shift to go faster, drag to look, scroll to change speed. On a phone use the buttons." });
  mode.append(orbitBtn, flyBtn);
  const startBtn = el("button", { textContent: "Go to the start", title: "Puts the camera at the track's start, looking down the course." });
  const overBtn = el("button", { textContent: "See it all", title: "Looks down on the whole track." });
  const shotBtn = el("button", { textContent: "Save a picture", title: "Saves the view as it is now as a .png." });
  const glbBtn = el("button", { textContent: "Save as .glb", title: "Saves the ground (at the detail picked), its objects and trees as a .glb for Blender and most 3D programs. Big tracks make big files." });
  const mapBtn = el("button", { textContent: "Save the map", title: "Saves the ground's textures seen from straight above as one .png." });
  const info = el("p", { className: "hint" });

  function listTracks() {
    trackSel.replaceChildren();
    const groups = new Map();
    for (const root of ["game", "mine", "pack"]) {
      const order = Object.keys(EVENTS);
      const ev = (p) => order.indexOf(p.match(/TERAFORM\/([^/]+)\//i)?.[1]?.toUpperCase());
      const files = lib.files(root).filter((f) => ext(f.path) === "env").sort((a, b) => ev(a.path) - ev(b.path) || a.path.localeCompare(b.path, undefined, { numeric: true }));
      for (const f of files) {
        const ev = f.path.match(/TERAFORM\/([^/]+)\//i)?.[1]?.toUpperCase();
        const label = root === "mine" ? "Your tracks" : root === "pack" ? `Free Pack ${EVENTS[ev] || ""}`.trim() : EVENTS[ev] || ev || "Other";
        if (!groups.has(label)) groups.set(label, el("optgroup", { label }));
        groups.get(label).append(option(root + ":" + f.path, f.path.split("/").pop().replace(/\.env$/i, "")));
      }
    }
    trackSel.append(...groups.values());
    return trackSel.options.length;
  }

  function build() {
    box.replaceChildren();
    if (!listTracks()) {
      box.append(
        el("p", { className: "hint", textContent: "No tracks here yet. Pick your game, or add a track's .zip or .env (from MDX Racing, say) at the top of the page." }),
        el("p", { className: "hint", textContent: "The Free Pack download doesn't hold its tracks. Pick the Free Pack .zip from the desktop version to see them." })
      );
      return;
    }
    box.append(
      row("Track", "The track to show. Your own tracks and the Free Pack's are listed too when they're here.", trackSel),
      row("Ground detail", "How finely the ground is drawn. Lower is quicker to load and lighter on phones.", detail),
      objRow, treeRow, lineRow, fogRow,
      mode,
      el("div", { className: "buttons" }, startBtn, overBtn),
      el("div", { className: "buttons" }, shotBtn, mapBtn, glbBtn),
      info
    );
    if (wanted && [...trackSel.options].some((o) => o.value === wanted)) trackSel.value = wanted;
    open(trackSel.value);
  }

  // Models in this track: its own first, then the game's, then the Free
  // Pack's stand-ins (by its stockmap.toml, as the game finds them).
  let stock = null;
  async function stockMap() {
    if (stock) return stock;
    stock = new Map();
    if (!lib.entry("pack", "stockmap.toml")) return stock;
    const text = new TextDecoder().decode(await lib.read("pack", "stockmap.toml"));
    let section = "";
    for (const line of text.split(/\r?\n/)) {
      const s = line.match(/^\[(.+)\]/);
      if (s) section = s[1];
      const m = section === "model" && line.match(/^\s*"?([^"=\s]+)"?\s*=\s*\{\s*file\s*=\s*"([^"]+)"/);
      if (m) stock.set(m[1].toLowerCase(), m[2]);
    }
    return stock;
  }

  async function open(value) {
    if (!value) return;
    const my = ++token;
    const [root, path] = [value.slice(0, value.indexOf(":")), value.slice(value.indexOf(":") + 1)];
    info.textContent = "Loading the track…";
    info.classList.remove("err");
    viewer.clear();
    try {
      const env = await lib.archive(root, path);
      const names = env.names();
      const byExt = (e) => names.findIndex((n) => n.toLowerCase().endsWith("." + e));
      const scnAt = byExt("scn");
      const scene = scnAt >= 0 ? JSON.parse(mm.scene(env.at(scnAt))) : null;
      let trnAt = scene?.terrain ? env.find(scene.terrain) : undefined;
      if (trnAt === undefined) trnAt = byExt("trn");
      if (trnAt === undefined || trnAt < 0) throw new Error("this track has no ground (.trn) in it");
      const step = +detail.value;
      const ground = mm.terrain(env.at(trnAt), step, step <= 1 ? 8192 : 4096);
      if (my !== token) return;
      const span = (ground.side - 1) * ground.spacing;
      const group = new THREE.Group();
      group.name = path.split("/").pop().replace(/\.env$/i, "");
      const terrain = groundMesh(ground);
      group.add(terrain);

      // Lights, haze and sky as the track sets them.
      const fogColor = scene?.fog ? new THREE.Color(scene.fog[0] / 255, scene.fog[1] / 255, scene.fog[2] / 255) : new THREE.Color(0x9fb3c4);
      viewer.scene.background = fogColor;
      viewer.scene.fog = showFog.checked ? new THREE.Fog(fogColor, span * 0.5, span * 2) : null;
      if (scene?.sky) skyBox(scene.sky, fogColor).then((sky) => my === token && sky && (viewer.scene.background = sky));
      if (scene?.sun) {
        const d = new THREE.Vector3(...scene.sun.dir).normalize();
        viewer.sun.position.copy(d.multiplyScalar(-1000));
        viewer.sun.color.setRGB(...scene.sun.color.map((c) => Math.min(1, c / 255)));
      }
      viewer.show(group);
      loaded = { root, path, env, scene, ground, group, terrain, span, fogColor, objects: null, trees: null, line: null };
      overview();
      info.textContent = "Adding the objects…";

      const res = [];
      for (const r of scene?.resources || []) {
        const name = r.replace(/^.*[\\/]/, "");
        if (lib.entry("game", "RES/" + name)) res.push(await lib.archive("game", "RES/" + name).catch(() => null));
      }
      const textures = new Textures(await lib.textureArchives([env, ...res.filter(Boolean)]));
      const modelArchives = await lib.modelArchives([env]);
      const stockModels = await stockMap();
      const cache = new Map();
      const missing = new Set();
      const load = async (name) => {
        const stem = name.replace(/^.*[\\/]/, "").replace(/\.[^.]*$/, "").trim();
        const key = stem.toLowerCase();
        if (cache.has(key)) return cache.get(key);
        const p = (async () => {
          const hit = findIn([env], stem + ".slt") || findIn(modelArchives, stem + ".slb") || findIn(modelArchives, stem + ".slt");
          if (hit) {
            const json = JSON.parse(mm.model(hit[0].at(hit[1]), hit[0].names()[hit[1]]));
            return { obj: buildModel(json, textures), json };
          }
          const file = stockModels.get(key) || (lib.entry("pack", `objects/${key}.glb`) ? `objects/${key}.glb` : lib.entry("pack", `eco/${key}.glb`) ? `eco/${key}.glb` : null);
          if (file && lib.entry("pack", file)) {
            const dir = file.split("/").slice(0, -1).join("/");
            const gltf = await loadGlb(await lib.read("pack", file), (u) => lib.read("pack", dir + "/" + u));
            const box = new THREE.Box3().setFromObject(gltf.scene);
            return { obj: gltf.scene, json: { min: box.min.toArray(), max: box.max.toArray() } };
          }
          missing.add(stem);
          return null;
        })().catch((e) => (console.warn(name, e), missing.add(stem), null));
        cache.set(key, p);
        return p;
      };

      const objects = new THREE.Group();
      objects.name = "Objects";
      for (const p of scene?.models || []) {
        const m = await load(p.model);
        if (my !== token) return;
        if (!m) continue;
        const o = m.obj.clone();
        o.position.fromArray(p.t);
        o.quaternion.fromArray(p.q);
        o.scale.fromArray(p.s);
        objects.add(o);
      }
      // Animated models (flags, gates, cranes…), playing their motion.
      for (const a of [...(scene?.animations || []), ...(scene?.procedural || [])]) {
        const mcfHit = /\.mcf$/i.test(a.model) && findIn(modelArchives, a.model.replace(/^.*[\\/]/, ""));
        let o = null;
        if (mcfHit) {
          try {
            const info2 = JSON.parse(mm.mcf(mcfHit[0].at(mcfHit[1])));
            const m = await load(info2.model);
            if (!m?.json?.frames) continue;
            o = buildModel(m.json, textures);
            const names2 = o.userData.nodes.map((n) => n.name);
            const motion = info2.motions.find((x) => !a.motion || x.name.toLowerCase() === a.motion.toLowerCase()) || info2.motions[0];
            const mot = motion && findIn(modelArchives, motion.file);
            if (mot) {
              const clip = clipOf(motion.name, JSON.parse(mm.motion(mot[0].at(mot[1]))), motion.fps, names2);
              o.userData.clip = clip;
            }
          } catch (e) {
            console.warn(a.model, e);
          }
        } else {
          const m = await load(a.model);
          if (m) o = m.obj.clone();
        }
        if (my !== token) return;
        if (!o) continue;
        const holder = new THREE.Group();
        holder.position.fromArray(a.t);
        holder.quaternion.fromArray(a.q);
        holder.scale.fromArray(a.s);
        holder.add(o);
        objects.add(holder);
        if (o.userData.clip) viewer.play(o, o.userData.clip, true);
      }
      group.add(objects);
      loaded.objects = objects;

      // Trees and bushes, one instanced mesh per kind of plant.
      const esbAt = byExt("esb");
      if (esbAt >= 0) {
        info.textContent = "Planting the trees…";
        try {
          const eco = JSON.parse(mm.eco(env.at(esbAt), span));
          loaded.trees = await plant(eco, load);
          if (my !== token) return;
          if (loaded.trees) group.add(loaded.trees);
        } catch (e) {
          console.warn("trees", e);
        }
      }
      const tdfAt = byExt("tdf");
      if (tdfAt >= 0) {
        try {
          loaded.line = racingLine(JSON.parse(mm.track_line(env.at(tdfAt))));
          group.add(loaded.line);
        } catch (e) {
          console.warn("line", e);
        }
      }
      visibility();
      const count = objects.children.length;
      info.textContent =
        `${group.name}: ${Math.round(span)} ft across, ${count} objects${loaded.trees ? `, ${loaded.trees.userData.count} plants` : ""}.` +
        (missing.size ? ` Not found: ${[...missing].slice(0, 12).join(", ")}${missing.size > 12 ? "…" : ""}.` : "") +
        (textures.missing.size ? ` ${textures.missing.size} textures missing.` : "");
    } catch (err) {
      console.warn(err);
      if (my === token) {
        info.textContent = `Couldn't show this track: ${err.message}`;
        info.classList.add("err");
      }
    }
  }

  // The track's sky box (RES/SKIES.RES), its faces in the game's order
  // (+z, -x, -z, +x, up, down) put round the page's axes.
  async function skyBox(name, under) {
    const skies = await lib.res("SKIES.RES");
    const hit = skies && findIn([skies], name.replace(/^.*[\\/]/, ""));
    if (!hit) return null;
    const strip = canvasOf(mm.sky(hit[0].at(hit[1])));
    const s = strip.height;
    const face = (i, mirror) => {
      const c = document.createElement("canvas");
      c.width = c.height = s;
      const ctx = c.getContext("2d");
      // Below the horizon the game draws no sky: the haze's colour there.
      ctx.fillStyle = "#" + under.getHexString();
      ctx.fillRect(0, 0, s, s);
      if (mirror) ctx.setTransform(-1, 0, 0, 1, s, 0);
      ctx.drawImage(strip, i * s, 0, s, s, 0, 0, s, s);
      return c;
    };
    // px, nx, py, ny, pz, nz
    const t = new THREE.CubeTexture([face(3, true), face(1, true), face(4, true), face(5, true), face(2, true), face(0, true)]);
    t.colorSpace = THREE.SRGBColorSpace;
    t.needsUpdate = true;
    return t;
  }

  function groundMesh(g) {
    const n = g.side;
    const h = g.heights();
    const pos = new Float32Array(n * n * 3);
    const uv = new Float32Array(n * n * 2);
    const span = (n - 1) * g.spacing;
    for (let z = 0; z < n; z++)
      for (let x = 0; x < n; x++) {
        const i = z * n + x;
        pos[i * 3] = x * g.spacing;
        pos[i * 3 + 1] = h[i];
        pos[i * 3 + 2] = -z * g.spacing;
        uv[i * 2] = (x * g.spacing) / span;
        uv[i * 2 + 1] = (z * g.spacing) / span;
      }
    const idx = new Uint32Array((n - 1) * (n - 1) * 6);
    let k = 0;
    for (let z = 0; z < n - 1; z++)
      for (let x = 0; x < n - 1; x++) {
        const a = z * n + x, b = a + 1, c = a + n, d = c + 1;
        idx.set([a, b, c, b, d, c], k);
        k += 6;
      }
    const geo = new THREE.BufferGeometry();
    geo.setAttribute("position", new THREE.BufferAttribute(pos, 3));
    geo.setAttribute("uv", new THREE.BufferAttribute(uv, 2));
    geo.setIndex(new THREE.BufferAttribute(idx, 1));
    geo.computeVertexNormals();
    const side = g.map_side;
    const map = g.map();
    const c = document.createElement("canvas");
    c.width = c.height = side;
    c.getContext("2d").putImageData(new ImageData(new Uint8ClampedArray(map.buffer, map.byteOffset, map.length), side, side), 0, 0);
    const tex = textureOf(c, "ground");
    tex.wrapS = tex.wrapT = THREE.ClampToEdgeWrapping;
    tex.generateMipmaps = true;
    tex.anisotropy = viewer.renderer.capabilities.getMaxAnisotropy();
    const mesh = new THREE.Mesh(geo, new THREE.MeshStandardMaterial({ map: tex, roughness: 1, metalness: 0, side: THREE.DoubleSide }));
    mesh.name = "Ground";
    mesh.userData.mapCanvas = c;
    return mesh;
  }

  async function plant(eco, load) {
    const out = new THREE.Group();
    out.name = "Plants";
    const bySlot = new Map(eco.species.map((s) => [s.slot, s.name]));
    const groups = new Map();
    for (let i = 0; i < eco.plants.length; i += 6) {
      const slot = eco.plants[i];
      if (!groups.has(slot)) groups.set(slot, []);
      groups.get(slot).push(i);
    }
    let count = 0;
    const m4 = new THREE.Matrix4();
    for (const [slot, list] of groups) {
      const model = await load(bySlot.get(slot) + ".slt");
      if (!model) continue;
      const top = model.json.max[1];
      const spread = Math.max(Math.abs(model.json.min[0]), Math.abs(model.json.max[0]), Math.abs(model.json.min[2]), Math.abs(model.json.max[2]));
      if (!(top > 0 && spread > 0)) continue;
      model.obj.updateMatrixWorld(true);
      const meshes = [];
      model.obj.traverse((o) => o.isMesh && meshes.push(o));
      for (const mesh of meshes) {
        const inst = new THREE.InstancedMesh(mesh.geometry, mesh.material, list.length);
        list.forEach((at, k) => {
          const p = eco.plants;
          const s = new THREE.Vector3(p[at + 5] / spread, p[at + 4] / top, p[at + 5] / spread);
          m4.compose(new THREE.Vector3(p[at + 1], p[at + 2], p[at + 3]), new THREE.Quaternion(), s).multiply(mesh.matrixWorld);
          inst.setMatrixAt(k, m4);
        });
        inst.name = bySlot.get(slot);
        out.add(inst);
      }
      count += list.length;
    }
    out.userData.count = count;
    return count ? out : null;
  }

  function racingLine(line) {
    const g = new THREE.Group();
    g.name = "RacingLine";
    const mat = new THREE.LineBasicMaterial({ color: 0xa3c24a, depthTest: false, transparent: true, opacity: 0.9 });
    for (const pts of line.nodes) {
      if (pts.length < 2) continue;
      const geo = new THREE.BufferGeometry().setFromPoints(pts.map((p) => new THREE.Vector3(p[0], p[1] + 2, p[2])));
      const l = new THREE.Line(geo, mat);
      l.renderOrder = 10;
      g.add(l);
    }
    return g;
  }

  function visibility() {
    if (!loaded) return;
    if (loaded.objects) loaded.objects.visible = showObjects.checked;
    if (loaded.trees) loaded.trees.visible = showTrees.checked;
    if (loaded.line) loaded.line.visible = showLine.checked;
    if (viewer.scene.fog && !showFog.checked) viewer.scene.fog = null;
    else if (!viewer.scene.fog && showFog.checked) viewer.scene.fog = new THREE.Fog(loaded.fogColor, loaded.span * 0.5, loaded.span * 2);
  }
  function overview() {
    if (!loaded) return;
    const s = loaded.span;
    const c = new THREE.Vector3(s / 2, 0, -s / 2);
    if (fly.viewer.fly) setMode(false);
    viewer.controls.target.copy(c);
    viewer.camera.position.set(s * 0.5, s * 0.55, s * 0.35);
    viewer.camera.near = 1;
    viewer.camera.far = s * 6;
    viewer.camera.updateProjectionMatrix();
  }
  function toStart() {
    if (!loaded?.scene) return overview();
    const st = new THREE.Vector3(...loaded.scene.start);
    const d = new THREE.Vector3(...loaded.scene.startDir);
    d.y = 0;
    if (d.lengthSq() < 1e-6) d.set(0, 0, -1);
    d.normalize();
    const h = groundAt(st.x, st.z);
    viewer.camera.position.set(st.x - d.x * 45, h + 22, st.z - d.z * 45);
    viewer.controls.target.set(st.x + d.x * 40, h + 4, st.z + d.z * 40);
    viewer.camera.lookAt(viewer.controls.target);
    if (viewer.fly) fly.start();
  }
  function groundAt(x, z) {
    const g = loaded.ground;
    const n = g.side;
    const i = Math.max(0, Math.min(n - 1, Math.round(x / g.spacing)));
    const j = Math.max(0, Math.min(n - 1, Math.round(-z / g.spacing)));
    loaded.h ||= g.heights();
    return loaded.h[j * n + i] || 0;
  }
  function setMode(flying) {
    orbitBtn.ariaPressed = String(!flying);
    flyBtn.ariaPressed = String(flying);
    pad.hidden = !flying;
    if (flying) {
      fly.start();
      hint.textContent = "W A S D or arrows to fly, Q and E down and up, Shift faster, drag to look, scroll for speed.";
    } else {
      if (viewer.fly) fly.stop();
      hint.textContent = "Drag to turn, scroll or pinch to zoom, right-drag to move.";
    }
  }
  orbitBtn.onclick = () => setMode(false);
  flyBtn.onclick = () => {
    setMode(true);
    canvas.focus();
  };
  startBtn.onclick = () => toStart();
  overBtn.onclick = () => overview();
  shotBtn.onclick = async () => save(await viewer.snapshot(), (loaded?.group.name || "track") + ".png");
  mapBtn.onclick = () => loaded && loaded.terrain.userData.mapCanvas.toBlob((b) => save(b, loaded.group.name + "-map.png"), "image/png");
  glbBtn.onclick = async () => {
    if (!loaded) return;
    glbBtn.disabled = true;
    const was = glbBtn.textContent;
    glbBtn.textContent = "Saving…";
    try {
      save(await glbOf(loaded.group), loaded.group.name + ".glb");
    } catch (e) {
      info.textContent = `Couldn't save it: ${e.message}`;
    } finally {
      glbBtn.disabled = false;
      glbBtn.textContent = was;
    }
  };
  trackSel.onchange = () => open(trackSel.value);
  detail.onchange = () => open(trackSel.value);
  for (const c of [showObjects, showTrees, showLine, showFog]) c.onchange = visibility;

  lib.onChange(() => {
    const keep = trackSel.value;
    wanted = wanted || keep;
    build();
  });
  build();
  return {
    openTrack(root, path) {
      wanted = root + ":" + path;
      if ([...trackSel.options].some((o) => o.value === wanted)) {
        trackSel.value = wanted;
        open(wanted);
      } else build();
    },
  };
}
