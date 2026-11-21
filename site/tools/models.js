// The bikes and riders the tools can show: the game's (from RES/BIKE.RES,
// with their paints, outfits and motions) and the Free Pack's (.glb, with
// their colours and the pack's own motions).

import * as mm from "./pkg/mm2_webtools.js";
import { findIn } from "./library.js";
import { THREE, Textures, buildModel, clipOf, retarget, loadGlb, textureOf } from "./scene3d.js";

/// The frames the Free Pack's own motions count in
/// (tools/freepack/free_motions.py ORDER and BIKE).
export const FREE_RIDER = ["Pelvis", "LwrTorso", "UprLegL", "UprLegR", "UprTorso", "Head", "UprArmL", "UprArmR", "LwrArmL", "HandL", "LwrArmR", "HandR", "LwrLegL", "FootL", "LwrLegR", "FootR"];
export const FREE_BIKE = ["Bike", "HandleBars", "KickStart", "NullFrame", "SwingArm", "Shocks", "TireFront", "TireRear"];

let freeMotions;
export async function ownMotions() {
  if (!freeMotions)
    freeMotions = fetch(new URL("./pkg/free_motions.bin", import.meta.url))
      .then((r) => (r.ok ? r.arrayBuffer() : Promise.reject(new Error(r.status))))
      .then((b) => JSON.parse(mm.free_motions(new Uint8Array(b))))
      .catch(() => []);
  return freeMotions;
}

/// An .ini-like text (the game's .pb lists) as { section: { key: value } }.
function ini(text) {
  const out = {};
  let cur = null;
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    const s = line.match(/^\[(.+)\]$/);
    if (s) out[(cur = s[1].trim())] = {};
    else if (cur) {
      const kv = line.match(/^([^=]+)=(.*)$/);
      if (kv) out[cur][kv[1].trim().toLowerCase()] = kv[2].trim().replace(/^"(.*)"$/, "$1");
    }
  }
  return out;
}

/// The game's bikes, paints and outfits, as its Bike & Rider page lists
/// them, plus the player's own paints and outfits in BIKES and RIDERS.
export async function gameRoster(lib) {
  const bikeRes = await lib.res("BIKE.RES");
  if (!bikeRes) return null;
  const bikes = [];
  const outfits = [];
  let ui = null;
  if (lib.entry("game", "UI/UIRES.RES")) ui = await lib.archive("game", "UI/UIRES.RES").catch(() => null);
  const text = (n) => {
    const i = ui?.find(n);
    return i === undefined ? null : new TextDecoder("latin1").decode(ui.at(i));
  };
  const pb = text("bikes.pb");
  if (pb) {
    for (const [, m] of Object.entries(ini(pb))) {
      if (!m.name || !m.modelfile) continue;
      const n = parseInt(m.numberofbikes) || 0;
      for (let i = 1; i <= n; i++) {
        const k = String(i).padStart(2, "0");
        const tex = m["biketexture_" + k];
        if (tex) bikes.push({ name: `${m.name} ${m["bikename_" + k] || ""}`.trim(), model: m.modelfile, paint: tex });
      }
    }
  }
  if (!bikes.length)
    for (const [mcf, paint] of [["honda.mcf", "HondaCR250_08.tga"], ["yamaha.mcf", "YamahaYZ250_08.tga"], ["ktm.mcf", "KTM250SX_08.tga"], ["suzuki.mcf", "SuzukiRM250_08.tga"]])
      if (bikeRes.find(mcf) !== undefined) bikes.push({ name: mcf.replace(".mcf", ""), model: mcf, paint });
  const rb = text("riders.pb");
  if (rb) for (const [, r] of Object.entries(ini(rb))) if (r.file) outfits.push({ name: r.name || r.file, file: r.file });
  if (!outfits.length)
    for (const n of bikeRes.names()) if (/^(fox|fx|answer|nobrand)/i.test(n) && /\.tga$/i.test(n)) outfits.push({ name: n.replace(/_08\.tga$/i, ""), file: n });
  // The player's own, which the game always reads: paints go on the
  // Yamaha frame (the game's BIKE01.TGA template is laid out for it).
  for (const f of lib.files("game")) {
    const m = f.path.match(/^(BIKES|RIDERS)\/([^/]+\.tga)$/i);
    if (!m) continue;
    const own = { name: `${m[2].split(".")[0]} (yours)`, own: f.path };
    if (m[1].toUpperCase() === "BIKES") bikes.push({ ...own, model: "yamaha.mcf", paint: m[2] });
    else outfits.push({ ...own, file: m[2] });
  }
  return { bikes, outfits, bikeRes };
}

/// The texture of a paint or outfit: the game's, or the player's own file.
export async function paintTexture(lib, item, archives) {
  if (item.own) {
    const bytes = await lib.read("game", item.own);
    try {
      const pic = mm.targa(bytes);
      const c = document.createElement("canvas");
      c.width = pic.width;
      c.height = pic.height;
      const d = pic.rgba();
      c.getContext("2d").putImageData(new ImageData(new Uint8ClampedArray(d.buffer, d.byteOffset, d.length), pic.width, pic.height), 0, 0);
      return textureOf(c, item.own);
    } catch {
      return null;
    }
  }
  return new Textures(archives).get(item.paint || item.file);
}

/// A game .mcf's model, built, with its motions to make clips from.
export async function gameAnimated(lib, mcfName, { skin, prefix = "" } = {}) {
  const bikeRes = await lib.res("BIKE.RES");
  const archives = await lib.modelArchives(bikeRes ? [bikeRes] : []);
  const hit = findIn(archives, mcfName);
  if (!hit) return null;
  const info = JSON.parse(mm.mcf(hit[0].at(hit[1])));
  const stem = info.model.replace(/^.*[\\/]/, "").replace(/\.[^.]*$/, "");
  const model = findIn(archives, stem + ".slb") || findIn(archives, stem + ".slt");
  if (!model) return null;
  const json = JSON.parse(mm.model(model[0].at(model[1]), model[0].names()[model[1]]));
  const textures = new Textures(await lib.textureArchives(bikeRes ? [bikeRes] : []));
  const obj = buildModel(json, textures, { skin, prefix });
  obj.name = stem;
  const names = obj.userData.nodes.map((n) => n.name);
  const clips = new Map();
  return {
    obj,
    json,
    textures,
    motions: info.motions,
    frames: json.frames.map((f) => f.name),
    clip(name) {
      if (clips.has(name)) return clips.get(name);
      const m = info.motions.find((x) => x.name.toLowerCase() === name.toLowerCase());
      let c = null;
      const mot = m && findIn(archives, m.file);
      if (mot) {
        try {
          c = clipOf(m.name, JSON.parse(mm.motion(mot[0].at(mot[1]))), m.fps, names);
          c.userData = { looping: m.looping, fps: m.fps };
        } catch {}
      }
      clips.set(name, c);
      return c;
    },
    rest() {
      obj.userData.nodes.forEach((n, i) => {
        n.position.fromArray(json.frames[i].t);
        n.quaternion.fromArray(json.frames[i].q);
      });
    },
  };
}

/// The bike motion that goes with a rider's motion, by the game's names:
/// `b<name>` (BarHop and bBarHop), CrosOv1 and CrosBik1, WaitR and WaitB.
export function bikeMotionFor(name, bikeMotions) {
  const lc = (s) => s.toLowerCase();
  const want = [
    "b" + lc(name),
    lc(name).replace(/^crosov(\d)$/, "crosbik$1"),
    lc(name).replace(/r$/, "b"),
  ];
  return bikeMotions.find((m) => want.includes(lc(m.name)))?.name || null;
}

/// The Free Pack's bikes and riders: their .glb and colours.
export function packRoster(lib) {
  const files = lib.files("pack").map((f) => f.path);
  if (!files.length) return null;
  const pick = (re) => {
    const all = files.filter((p) => re.test(p));
    return all.find((p) => /\/web\//.test(p)) || all[0] || null;
  };
  const bikes = [];
  for (const p of files) {
    const m = p.match(/^bikes\/(?:[^/]+\/)?(?:web\/)?fp_bike_([^/.]+)\.glb$/i);
    if (!m || bikes.some((b) => b.shape === m[1])) continue;
    const shape = m[1];
    const glb = pick(new RegExp(`^bikes/(?:${shape}/)?(?:web/)?fp_bike_${shape}\\.glb$`, "i"));
    const colours = [...new Set(files.map((f) => f.match(new RegExp(`^bikes/${shape}/(?:web/)?fp_livery_${shape}_([a-z]+)\\.png$`, "i"))?.[1]).filter(Boolean))];
    const web = /\/web\//.test(glb);
    bikes.push({ shape, glb, colours, livery: (c) => `bikes/${shape}/${web ? "web/" : ""}fp_livery_${shape}_${c}.png` });
  }
  const riderGlb = pick(/^riders\/hq\/(?:web\/)?fp_rider_hq\.glb$/i);
  const web = /\/web\//.test(riderGlb || "");
  const riderColours = [...new Set(files.map((f) => f.match(/^riders\/hq\/(?:web\/)?fp_rider_hq_([a-z]+)\.png$/i)?.[1]).filter(Boolean))];
  return { bikes, rider: riderGlb && { glb: riderGlb, colours: riderColours, outfit: (c) => `riders/hq/${web ? "web/" : ""}fp_rider_hq_${c}.png` } };
}

/// A Free Pack picture as a texture for a .glb's material.
export async function packTexture(lib, path) {
  const bytes = await lib.read("pack", path);
  const bmp = await createImageBitmap(new Blob([bytes]));
  const c = document.createElement("canvas");
  c.width = bmp.width;
  c.height = bmp.height;
  c.getContext("2d").drawImage(bmp, 0, 0);
  return textureOf(c, path.split("/").pop());
}

/// A Free Pack .glb, with the first material that has a picture (its
/// paint) to swap and the node names to drive.
export async function packModel(lib, path) {
  const dir = path.split("/").slice(0, -1).join("/");
  const gltf = await loadGlb(await lib.read("pack", path), (uri) => lib.read("pack", (dir ? dir + "/" : "") + uri));
  let paint = null;
  gltf.scene.traverse((o) => {
    if (o.isMesh && !paint && o.material?.map) paint = o.material;
  });
  const rest = [];
  gltf.scene.traverse((o) => rest.push([o, o.position.clone(), o.quaternion.clone()]));
  return {
    obj: gltf.scene,
    paint,
    rest() {
      for (const [o, p, q] of rest) {
        o.position.copy(p);
        o.quaternion.copy(q);
      }
    },
    setPaint(tex) {
      if (!paint || !tex) return;
      tex.flipY = false;
      tex.colorSpace = THREE.SRGBColorSpace;
      paint.map = tex;
      paint.needsUpdate = true;
    },
  };
}

/// A clip of the pack's own motions for one model, onto its nodes by name.
export function ownClip(m, rider, nodeNames) {
  const order = rider ? FREE_RIDER : FREE_BIKE;
  const lc = new Map(nodeNames.map((n) => [n.toLowerCase(), n]));
  const names = order.map((n) => lc.get(n.toLowerCase()));
  const c = clipOf(m.name, m, m.fps, names);
  c.userData = { looping: m.looping, fps: m.fps };
  return c;
}

/// A game clip moved onto another model's nodes with the same frame
/// names, in any case (the game's rider motions on the pack's rider).
export function moveClip(clip, fromNames, toNames) {
  const lc = new Map(toNames.map((n) => [n.toLowerCase(), n]));
  const from = new Map(fromNames.map(([node, frame]) => [node, frame]));
  return retarget(clip, (node) => lc.get((from.get(node) || node).toLowerCase()));
}
