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

// A rider and bike, a few boxes each: wheels, the bike, the rider. In the
// game's frame: x to the right, y up, z forward, feet.
function rider(colour) {
  const g = new THREE.Group();
  const dark = new THREE.MeshLambertMaterial({ color: 0x1b1b1b });
  const paint = new THREE.MeshLambertMaterial({ color: colour });
  const kit = new THREE.MeshLambertMaterial({ color: new THREE.Color(colour).lerp(new THREE.Color(0xffffff), 0.35) });
  const wheel = new THREE.CylinderGeometry(1.05, 1.05, 0.35, 10);
  wheel.rotateZ(Math.PI / 2);
  const box = (w, h, d, m, x, y, z, tilt = 0) => {
    const mesh = new THREE.Mesh(new THREE.BoxGeometry(w, h, d), m);
    mesh.position.set(x, y, z);
    mesh.rotation.x = tilt;
    g.add(mesh);
    return mesh;
  };
  for (const z of [2.35, -2.35]) {
    const w = new THREE.Mesh(wheel, dark);
    w.position.set(0, 1.05, z);
    g.add(w);
  }
  box(0.7, 0.9, 3.6, paint, 0, 2.2, 0.1); // tank and seat
  box(0.5, 0.35, 1.6, paint, 0, 2.55, 2.0, -0.5); // number plate and bars
  box(0.25, 1.6, 0.25, dark, 0, 1.75, 2.0, -0.45); // forks
  box(1.1, 1.8, 0.8, kit, 0, 3.6, -0.3, 0.35); // body
  box(0.55, 0.55, 0.65, paint, 0, 4.75, 0.15); // helmet
  box(0.35, 1.3, 0.35, kit, 0.45, 2.7, 0.0, -0.6); // legs
  box(0.35, 1.3, 0.35, kit, -0.45, 2.7, 0.0, -0.6);
  box(0.25, 0.25, 1.4, kit, 0.6, 3.9, 0.9, 0.5); // arms
  box(0.25, 0.25, 1.4, kit, -0.6, 3.9, 0.9, 0.5);
  g.matrixAutoUpdate = false;
  return g;
}

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
    const m = await load();
    this.stop();
    this.sim = new m.Sim(env, count, supercross);
    for (let i = 0; i < this.sim.count(); i++) {
      const b = rider(COLOURS[i % COLOURS.length]);
      this.bikes.push(b);
      this.group.add(b);
    }
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
      b.traverse((o) => o.geometry?.dispose());
    }
    this.bikes = [];
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
    // Far off, the riders are drawn bigger, so they can still be seen
    // going round the whole track.
    const cam = this.viewer.camera.position;
    for (let i = 0; i < this.bikes.length; i++) {
      const o = i * POSE;
      z.set(p[o + 3], p[o + 4], p[o + 5]);
      y.set(p[o + 6], p[o + 7], p[o + 8]);
      x.crossVectors(y, z).normalize();
      y.crossVectors(z, x).normalize();
      const b = this.bikes[i];
      const far = Math.hypot(cam.x - p[o], cam.y - p[o + 1], cam.z - p[o + 2]);
      const k = Math.max(1, far / 120);
      b.matrix.makeBasis(x.multiplyScalar(k), y.clone().multiplyScalar(k), z.clone().multiplyScalar(k)).setPosition(p[o], p[o + 1], p[o + 2]);
      b.matrixWorldNeedsUpdate = true;
      if (i === 0) lead = b;
    }
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
