// "Watch an AI lap": the game's computer riders lapping the track in the
// Course Builder tab's 3D view. The riders are the game's own AI and bike
// physics (site/tools/sim, built from crates/mm2-preview), loaded only
// the first time someone presses the button. They ride the race's fixed
// frame, so a faster watch runs more frames, never longer ones.

import { THREE } from "./scene3d.js";

let loaded = null;
const load = () =>
  (loaded ||= import("./sim/mm2_preview.js").then(async (m) => {
    await m.default();
    return m;
  }));

const POSE = 16;
const COLOURS = [0xd8452b, 0x2f6fd6, 0xe0b322, 0x3a9d4f, 0xf2f2f2, 0x8a4fd1, 0xe07a1f, 0x1fb3b3];

// Our own Free Pack rider and bike, cut down to a couple of thousand flat
// triangles in four block colours (ailap-rider.bin, made by
// tools/freepack/ailap_rider.py): the paint, the rider's kit, the dark
// parts and the metal. In the game's frame: x to the right, y up, z
// forward, feet.
let shapes = null;
const loadShapes = () =>
  (shapes ||= fetch(new URL("./ailap-rider.bin", import.meta.url))
    .then((r) => (r.ok ? r.arrayBuffer() : Promise.reject(new Error(`rider model: ${r.status}`))))
    .then((buf) => {
      const d = new DataView(buf);
      let o = 4;
      const out = {};
      for (const role of ["paint", "kit", "dark", "metal"]) {
        const nv = d.getUint16(o, true);
        const nt = d.getUint16(o + 2, true);
        o += 4;
        const pos = new Float32Array(nv * 3);
        for (let i = 0; i < nv * 3; i++, o += 2) pos[i] = d.getInt16(o, true) / 1000;
        const idx = new Uint16Array(nt * 3);
        for (let i = 0; i < nt * 3; i++, o += 2) idx[i] = d.getUint16(o, true);
        const g = new THREE.BufferGeometry();
        g.setAttribute("position", new THREE.BufferAttribute(pos, 3));
        g.setIndex(new THREE.BufferAttribute(idx, 1));
        const flat = g.toNonIndexed();
        flat.computeVertexNormals();
        out[role] = flat;
      }
      return out;
    }));

function rider(colour, geo) {
  const g = new THREE.Group();
  const paint = new THREE.Color(colour);
  const colours = {
    paint,
    kit: paint.clone().lerp(new THREE.Color(0xffffff), 0.45),
    dark: new THREE.Color(0x1d1c1a),
    metal: new THREE.Color(0x9c9a96),
  };
  for (const [role, geometry] of Object.entries(geo)) {
    const mesh = new THREE.Mesh(geometry, new THREE.MeshLambertMaterial({ color: colours[role], side: THREE.DoubleSide }));
    mesh.matrixAutoUpdate = false;
    g.add(mesh);
  }
  g.matrixAutoUpdate = false;
  return g;
}

// A round dot with a dark rim, for the riders seen from far off.
function dotTexture() {
  const c = document.createElement("canvas");
  c.width = c.height = 64;
  const g = c.getContext("2d");
  g.beginPath();
  g.arc(32, 32, 27, 0, Math.PI * 2);
  g.fillStyle = "#fff";
  g.fill();
  g.lineWidth = 8;
  g.strokeStyle = "rgba(15,14,12,0.85)";
  g.stroke();
  const t = new THREE.CanvasTexture(c);
  t.colorSpace = THREE.SRGBColorSpace;
  return t;
}

/// Farther than this the riders are dots, ft.
const DOTS_FROM = 260;

export class AiLap {
  constructor(viewer) {
    this.viewer = viewer;
    this.sim = null;
    this.speed = 4;
    this.follow = false;
    this.group = new THREE.Group();
    this.bikes = [];
    this.onLap = () => {};
    this.tick = (dt) => this.frame(dt);
  }

  /// Puts `count` riders on the baked track `env` (an .ENV's bytes).
  async start(env, count, supercross) {
    const [m, geo] = await Promise.all([load(), loadShapes()]);
    this.stop();
    this.sim = new m.Sim(env, count, supercross);
    for (let i = 0; i < this.sim.count(); i++) {
      const b = rider(COLOURS[i % COLOURS.length], geo);
      this.bikes.push(b);
      this.group.add(b);
    }
    // The same riders as dots, a fixed size on screen.
    const n = this.bikes.length;
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.BufferAttribute(new Float32Array(n * 3), 3));
    const col = new Float32Array(n * 3);
    for (let i = 0; i < n; i++) new THREE.Color(COLOURS[i % COLOURS.length]).toArray(col, i * 3);
    g.setAttribute("color", new THREE.BufferAttribute(col, 3));
    this.dotTex ||= dotTexture();
    this.dots = new THREE.Points(
      g,
      new THREE.PointsMaterial({ size: 11, sizeAttenuation: false, vertexColors: true, map: this.dotTex, transparent: true, alphaTest: 0.3, depthTest: false }),
    );
    this.dots.renderOrder = 2;
    this.dots.frustumCulled = false;
    this.group.add(this.dots);
    this.viewer.scene.add(this.group);
    this.viewer.onFrame.push(this.tick);
    this.frame(0);
  }

  /// The ground the riders ride on, `n` x `n` points over the world.
  ground(n) {
    return { side: this.sim.side(), heights: this.sim.heights(n), zones: this.sim.zones(n) };
  }

  height(x, z) {
    return this.sim.height(x, z);
  }

  get running() {
    return !!this.sim;
  }

  stop() {
    const f = this.viewer.onFrame.indexOf(this.tick);
    if (f >= 0) this.viewer.onFrame.splice(f, 1);
    this.viewer.scene.remove(this.group);
    for (const b of this.bikes) {
      this.group.remove(b);
      b.traverse((o) => o.material?.dispose());
    }
    this.bikes = [];
    if (this.dots) {
      this.group.remove(this.dots);
      this.dots.geometry.dispose();
      this.dots.material.dispose();
      this.dots = null;
    }
    this.ribbon = null;
    this.sim?.free();
    this.sim = null;
  }

  frame(dt) {
    if (!this.sim) return;
    this.sim.advance(dt * this.speed);
    const p = this.sim.poses();
    const x = new THREE.Vector3();
    const y = new THREE.Vector3();
    const z = new THREE.Vector3();
    let lead = null;
    // Far off, each rider is a dot instead, so it can still be seen going
    // round the whole track.
    const cam = this.viewer.camera.position;
    const dot = this.dots.geometry.attributes.position;
    for (let i = 0; i < this.bikes.length; i++) {
      const o = i * POSE;
      z.set(p[o + 3], p[o + 4], p[o + 5]);
      y.set(p[o + 6], p[o + 7], p[o + 8]);
      x.crossVectors(y, z).normalize();
      y.crossVectors(z, x).normalize();
      const b = this.bikes[i];
      const far = Math.hypot(cam.x - p[o], cam.y - p[o + 1], cam.z - p[o + 2]) > DOTS_FROM;
      b.visible = !far;
      b.matrix.makeBasis(x, y, z).setPosition(p[o], p[o + 1], p[o + 2]);
      b.matrixWorldNeedsUpdate = true;
      dot.setXYZ(i, p[o], far ? p[o + 1] + 3 : -1e6, p[o + 2]);
      if (i === 0) lead = b;
    }
    dot.needsUpdate = true;
    if (this.ribbon) this.ribbon.visible = cam.distanceTo(this.viewer.controls.target) > DOTS_FROM * 2;
    if (this.follow && lead) {
      // Keep the view's distance and angle, moved along with the first rider.
      const ctl = this.viewer.controls;
      const at = new THREE.Vector3().setFromMatrixPosition(lead.matrix);
      const off = at.clone().sub(ctl.target);
      ctl.target.add(off);
      this.viewer.camera.position.add(off);
    }
    const laps = this.sim.laps();
    this.onLap(this.sim.clock(), laps);
  }
}
