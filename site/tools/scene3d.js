// The 3D view the tools share, and turning the game's models, motions and
// pictures into three.js ones (and back out as .glb, .png and .wav).

import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
import { GLTFExporter } from "three/addons/exporters/GLTFExporter.js";
import * as mm from "./pkg/mm2_webtools.js";
import { findIn } from "./library.js";

export { THREE };

/// A 3D view in `canvas`: orbit by default, or free fly.
export class Viewer {
  constructor(canvas) {
    this.canvas = canvas;
    this.renderer = new THREE.WebGLRenderer({ canvas, antialias: true, preserveDrawingBuffer: true });
    this.renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
    this.renderer.outputColorSpace = THREE.SRGBColorSpace;
    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0x24221e);
    this.camera = new THREE.PerspectiveCamera(45, 1, 0.05, 20000);
    this.camera.position.set(6, 4, 8);
    this.controls = new OrbitControls(this.camera, canvas);
    this.controls.enableDamping = true;
    this.hemi = new THREE.HemisphereLight(0xfff4e0, 0x3a3328, 1.6);
    this.sun = new THREE.DirectionalLight(0xffffff, 2.2);
    this.sun.position.set(5, 10, 6);
    this.scene.add(this.hemi, this.sun);
    this.grid = new THREE.GridHelper(20, 20, 0x4a463d, 0x332f29);
    this.scene.add(this.grid);
    this.mixers = [];
    this.clock = new THREE.Clock();
    this.fly = null;
    this.content = new THREE.Group();
    this.scene.add(this.content);
    this.onFrame = [];
    new ResizeObserver(() => this.resize()).observe(canvas);
    this.resize();
    const loop = () => {
      requestAnimationFrame(loop);
      if (!canvas.offsetParent) return;
      const dt = Math.min(this.clock.getDelta(), 0.1);
      for (const m of this.mixers) m.update(dt);
      if (this.fly) this.fly.update(dt);
      else this.controls.update();
      for (const f of this.onFrame) f(dt);
      this.renderer.render(this.scene, this.camera);
    };
    loop();
  }
  resize() {
    const w = this.canvas.clientWidth || 1;
    const h = this.canvas.clientHeight || 1;
    this.renderer.setSize(w, h, false);
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
  }
  clear() {
    this.mixers = [];
    for (const c of [...this.content.children]) {
      this.content.remove(c);
      c.traverse((o) => {
        o.geometry?.dispose();
      });
    }
  }
  show(object) {
    this.content.add(object);
  }
  /// Points the camera at `object` from a three-quarter view.
  frame(object, from = new THREE.Vector3(1, 0.6, 1.3)) {
    const box = new THREE.Box3().setFromObject(object);
    if (box.isEmpty()) return;
    const size = box.getSize(new THREE.Vector3()).length() || 1;
    const centre = box.getCenter(new THREE.Vector3());
    this.controls.target.copy(centre);
    this.camera.position.copy(centre).add(from.clone().normalize().multiplyScalar(size * 1.1));
    this.camera.near = size / 500;
    this.camera.far = size * 50;
    this.camera.updateProjectionMatrix();
    this.grid.position.y = box.min.y;
    this.grid.scale.setScalar(Math.max(1, size / 12));
  }
  /// Plays `clip` on `root`, looping or not.
  play(root, clip, loop = true) {
    const mixer = new THREE.AnimationMixer(root);
    const action = mixer.clipAction(clip);
    action.setLoop(loop ? THREE.LoopRepeat : THREE.LoopPingPong, Infinity);
    action.clampWhenFinished = true;
    action.play();
    this.mixers.push(mixer);
    return mixer;
  }
  snapshot() {
    return new Promise((ok) => this.canvas.toBlob(ok, "image/png"));
  }
}

/// Free fly: WASD or the arrows to move, Q and E (or Page Up and Down) for
/// down and up, Shift to go faster, drag to look round. On a phone, drag
/// with one finger to look and hold the on-screen buttons to move.
export class Fly {
  constructor(viewer, speed = 60) {
    this.viewer = viewer;
    this.speed = speed;
    this.keys = new Set();
    this.held = new Set();
    this.yaw = 0;
    this.pitch = 0;
    const c = viewer.canvas;
    this.down = (e) => {
      this.drag = [e.clientX, e.clientY];
      c.setPointerCapture?.(e.pointerId);
    };
    this.move = (e) => {
      if (!this.drag) return;
      this.yaw -= (e.clientX - this.drag[0]) * 0.004;
      this.pitch = Math.max(-1.5, Math.min(1.5, this.pitch - (e.clientY - this.drag[1]) * 0.004));
      this.drag = [e.clientX, e.clientY];
    };
    this.up = () => (this.drag = null);
    this.key = (e) => {
      if (e.target.closest?.("input, select, textarea")) return;
      const k = e.code;
      if (!/^(Key[WASDQE]|Arrow|ShiftLeft|ShiftRight|PageUp|PageDown)/.test(k)) return;
      if (e.type === "keydown") this.keys.add(k);
      else this.keys.delete(k);
      if (c.matches(":hover") || document.activeElement === c) e.preventDefault();
    };
    this.wheel = (e) => {
      e.preventDefault();
      this.speed = Math.max(2, Math.min(2000, this.speed * (e.deltaY > 0 ? 0.85 : 1.18)));
      this.onSpeed?.(this.speed);
    };
  }
  start() {
    const v = this.viewer;
    v.controls.enabled = false;
    const dir = v.camera.getWorldDirection(new THREE.Vector3());
    this.yaw = Math.atan2(-dir.x, -dir.z);
    this.pitch = Math.asin(Math.max(-1, Math.min(1, dir.y)));
    const c = v.canvas;
    c.addEventListener("pointerdown", this.down);
    c.addEventListener("pointermove", this.move);
    c.addEventListener("pointerup", this.up);
    c.addEventListener("wheel", this.wheel, { passive: false });
    addEventListener("keydown", this.key);
    addEventListener("keyup", this.key);
    v.fly = this;
  }
  stop() {
    const v = this.viewer;
    const c = v.canvas;
    c.removeEventListener("pointerdown", this.down);
    c.removeEventListener("pointermove", this.move);
    c.removeEventListener("pointerup", this.up);
    c.removeEventListener("wheel", this.wheel);
    removeEventListener("keydown", this.key);
    removeEventListener("keyup", this.key);
    v.fly = null;
    v.controls.enabled = true;
    const d = v.camera.getWorldDirection(new THREE.Vector3());
    v.controls.target.copy(v.camera.position).add(d.multiplyScalar(20));
  }
  update(dt) {
    const cam = this.viewer.camera;
    cam.quaternion.setFromEuler(new THREE.Euler(this.pitch, this.yaw, 0, "YXZ"));
    const k = (...codes) => codes.some((c) => this.keys.has(c) || this.held.has(c));
    const move = new THREE.Vector3(
      (k("KeyD", "ArrowRight") ? 1 : 0) - (k("KeyA", "ArrowLeft") ? 1 : 0),
      (k("KeyE", "PageUp") ? 1 : 0) - (k("KeyQ", "PageDown") ? 1 : 0),
      (k("KeyS", "ArrowDown") ? 1 : 0) - (k("KeyW", "ArrowUp") ? 1 : 0)
    );
    if (!move.lengthSq()) return;
    const fast = k("ShiftLeft", "ShiftRight") ? 4 : 1;
    move.normalize().multiplyScalar(this.speed * fast * dt);
    const up = move.y;
    move.y = 0;
    move.applyQuaternion(cam.quaternion);
    move.y += up;
    cam.position.add(move);
    this.onMove?.(cam.position);
  }
}

/// A picture (from the readers: width, height, rgba) as a canvas.
export function canvasOf(pic) {
  const c = document.createElement("canvas");
  c.width = pic.width;
  c.height = pic.height;
  const data = pic.rgba();
  c.getContext("2d").putImageData(new ImageData(new Uint8ClampedArray(data.buffer, data.byteOffset, data.length), pic.width, pic.height), 0, 0);
  return c;
}
export const pngOf = (canvas) => new Promise((ok) => canvas.toBlob(ok, "image/png"));

/// Saves a blob as a file in the browser's downloads.
export function save(blob, name) {
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = name;
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(a.href), 30000);
}

/// A game texture's bytes as a three.js texture, or null.
export function gameTexture(bytes, name = "") {
  let pic;
  try {
    pic = mm.texture(bytes);
  } catch {
    return null;
  }
  return textureOf(canvasOf(pic), name);
}
export function textureOf(canvas, name = "") {
  const t = new THREE.CanvasTexture(canvas);
  t.name = name;
  t.colorSpace = THREE.SRGBColorSpace;
  t.wrapS = t.wrapT = THREE.RepeatWrapping;
  t.flipY = false;
  t.anisotropy = 4;
  const px = canvas.getContext("2d").getImageData(0, 0, canvas.width, canvas.height).data;
  t.userData.alpha = false;
  for (let i = 3; i < px.length; i += 4 * 7) if (px[i] < 250) { t.userData.alpha = true; break; }
  return t;
}

/// Finds and decodes textures by name in `archives`, keeping each.
export class Textures {
  constructor(archives) {
    this.archives = archives;
    this.cache = new Map();
    this.missing = new Set();
  }
  get(name) {
    if (!name) return null;
    const key = name.toLowerCase();
    if (this.cache.has(key)) return this.cache.get(key);
    const hit = findIn(this.archives, name);
    let t = null;
    if (hit) t = gameTexture(hit[0].at(hit[1]), name);
    if (!t) this.missing.add(name);
    this.cache.set(key, t);
    return t;
  }
}

const safeName = (s) => THREE.PropertyBinding.sanitizeNodeName(s || "frame");

/// A game model (the readers' JSON) as a three.js group. `nodes[i]` is
/// model frame `i`. `skin` (a texture) replaces the first material's, as
/// the game does for paints and outfits.
export function buildModel(json, textures, { skin = null, prefix = "", lit = true } = {}) {
  const root = new THREE.Group();
  const names = new Set();
  const nodes = json.frames.map((f, i) => {
    const o = new THREE.Object3D();
    let n = safeName(prefix + (f.name || "frame" + i));
    while (names.has(n)) n += "_" + i;
    names.add(n);
    o.name = n;
    o.userData.frame = f.name;
    o.position.fromArray(f.t);
    o.quaternion.fromArray(f.q);
    o.scale.fromArray(f.s);
    return o;
  });
  json.frames.forEach((f, i) => (f.parent != null && nodes[f.parent] ? nodes[f.parent] : root).add(nodes[i]));
  const mats = new Map();
  const material = (m) => {
    if (mats.has(m)) return mats.get(m);
    const name = json.materials[m] || "";
    const map = m === 0 && skin ? skin : textures?.get(name) || null;
    const opts = { side: THREE.DoubleSide, roughness: 0.85, metalness: 0 };
    let mat;
    if (map) {
      mat = new (lit ? THREE.MeshStandardMaterial : THREE.MeshBasicMaterial)({ ...opts, map });
      if (map.userData.alpha) mat.alphaTest = 0.5;
    } else mat = new (lit ? THREE.MeshStandardMaterial : THREE.MeshBasicMaterial)({ ...opts, vertexColors: true });
    if (!lit) delete mat.roughness;
    mat.name = name || "plain";
    mats.set(m, mat);
    return mat;
  };
  for (const mesh of json.meshes) {
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.Float32BufferAttribute(mesh.p, 3));
    g.setAttribute("normal", new THREE.Float32BufferAttribute(mesh.n, 3));
    g.setAttribute("uv", new THREE.Float32BufferAttribute(mesh.uv, 2));
    g.setAttribute("color", new THREE.Uint8BufferAttribute(mesh.c, 4, true));
    g.setIndex(mesh.i);
    const m = new THREE.Mesh(g, material(mesh.material));
    m.name = (nodes[mesh.frame]?.name || "mesh") + "_mesh";
    m.userData.material = mesh.material;
    (nodes[mesh.frame] || root).add(m);
  }
  root.userData.nodes = nodes;
  root.userData.materials = mats;
  return root;
}

/// A motion (the readers' JSON) as a clip for the nodes named by
/// `names[frame]` (the model's own frames by default).
export function clipOf(name, motion, fps, names) {
  const times = Array.from({ length: motion.count }, (_, k) => k / (fps || 15));
  const tracks = [];
  for (const tr of motion.tracks) {
    const node = names[tr.frame];
    if (!node) continue;
    tracks.push(new THREE.VectorKeyframeTrack(`${node}.position`, times, tr.t));
    tracks.push(new THREE.QuaternionKeyframeTrack(`${node}.quaternion`, times, tr.q));
  }
  return new THREE.AnimationClip(name, motion.count > 1 ? (motion.count - 1) / (fps || 15) : 0.01, tracks);
}

/// Renames a clip's tracks onto another model's nodes that carry the same
/// frame names (a motion made for one rider played on another).
export function retarget(clip, byFrame) {
  const tracks = [];
  for (const t of clip.tracks) {
    const [node, prop] = t.name.split(".");
    const to = byFrame(node);
    if (to) {
      const c = t.clone();
      c.name = `${to}.${prop}`;
      tracks.push(c);
    }
  }
  return new THREE.AnimationClip(clip.name, clip.duration, tracks);
}

/// Loads a .glb (from bytes); `resolve(uri)` gives the bytes of a picture
/// it keeps in a file beside it.
export async function loadGlb(bytes, resolve) {
  const manager = new THREE.LoadingManager();
  const urls = new Map();
  try {
    const len = new DataView(bytes.buffer, bytes.byteOffset).getUint32(12, true);
    const json = JSON.parse(new TextDecoder().decode(bytes.subarray(20, 20 + len)));
    for (const img of json.images || []) {
      if (img.uri && !img.uri.startsWith("data:") && resolve) {
        const data = await resolve(decodeURIComponent(img.uri)).catch(() => null);
        if (data) urls.set(img.uri, URL.createObjectURL(new Blob([data], { type: img.mimeType || "image/png" })));
      }
    }
  } catch {}
  manager.setURLModifier((u) => {
    for (const [k, v] of urls) if (u.endsWith(k)) return v;
    return u;
  });
  const gltf = await new GLTFLoader(manager).parseAsync(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.length), "pack/");
  for (const v of urls.values()) setTimeout(() => URL.revokeObjectURL(v), 60000);
  return gltf;
}

/// `object` (and `clips`) as a .glb blob.
export async function glbOf(object, clips = []) {
  const out = await new GLTFExporter().parseAsync(object, { binary: true, animations: clips, onlyVisible: true });
  return new Blob([out], { type: "model/gltf-binary" });
}
