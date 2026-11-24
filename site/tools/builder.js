// The Course Builder tab. A quick builder right on the page, with the
// game's own track maths (mm2-course, in the page's small wasm): the line,
// its width and berms, jumps, the land, a map and a 3D look. Tracks are
// saved where the browser version keeps the game builder's, so the full
// Course Builder (in the game) opens on the same track, and Ride it rides
// it there. The full builder is picked to open with the player's MM2
// files, the demo or the free version.

import * as mm from "./pkg/mm2_webtools.js";
import * as fs from "../play/mm2fs.js";
import { Viewer, THREE, save as download } from "./scene3d.js";

const DEMO_PATH = "/demo/MCM2PCG.exe";
const COURSES = "/user/Courses/";
const KIT = JSON.parse(mm.course_kit());
const EVERY = 8; // the map and 3D look sample every 8th point of the ground
const enc = new TextEncoder();
const dec = new TextDecoder();

// The bakes run in a worker: a big track takes a while.
let worker = null;
let asks = 0;
const waiting = new Map();
function bakeOff(what, text) {
  if (!worker) {
    worker = new Worker(new URL("./builder-worker.js", import.meta.url), { type: "module" });
    worker.onmessage = ({ data }) => {
      const w = waiting.get(data.id);
      waiting.delete(data.id);
      data.err ? w.fail(new Error(data.err)) : w.ok(data.out);
    };
  }
  const id = ++asks;
  return new Promise((ok, fail) => {
    waiting.set(id, { ok, fail });
    worker.postMessage({ id, what, text });
  });
}

export async function mount(app, panel) {
  const $ = (id) => panel.querySelector("#" + id);
  const say = (text, bad = false) => {
    $("qb-status").textContent = text;
    $("qb-status").classList.toggle("err", bad);
  };

  // ---- the track ----
  let c = null; // the .course, as JSON
  let undo = [];
  let redo = [];
  let view = null; // the preview: heights, colours, line
  let sel = { point: -1, jump: -1 };
  let tool = "line";
  const text = () => JSON.stringify(c);
  const event = () => KIT.events.find((e) => e.id === c?.mode) || KIT.events[1];
  const remember = () => {
    undo.push(text());
    if (undo.length > 100) undo.shift();
    redo = [];
  };
  function use(json, keepSel = false) {
    c = typeof json === "string" ? JSON.parse(json) : json;
    if (!keepSel) sel = { point: -1, jump: -1 };
    changed();
  }

  // The last track saved here or in the game's builder, else a new one.
  async function lastTrack() {
    try {
      const last = await fs.userFile(COURSES + "last.txt");
      const path = last && dec.decode(last).trim();
      const bytes = path && (await fs.userFile(path));
      if (bytes) return JSON.parse(dec.decode(bytes));
    } catch (err) {
      console.warn("no last track", err);
    }
    return JSON.parse(mm.course_surprise("National", (Math.random() * 1e9) >>> 0));
  }

  // ---- the controls ----
  for (const e of KIT.events) $("qb-event").add(new Option(e.name, e.id));
  for (const j of KIT.jumps) $("qb-kind").add(new Option(j.name, j.id));
  for (const l of KIT.lands) $("qb-land").add(new Option(l.name, l.id));
  const kindAbout = () => ($("qb-kind-about").textContent = KIT.jumps.find((j) => j.id === $("qb-kind").value)?.about || "");
  kindAbout();

  // A slider: `get` reads it from the track, `set` writes it (the drag is
  // one step to undo).
  function slider(id, get, set, shown) {
    const el = $(id);
    el.addEventListener("pointerdown", remember);
    el.addEventListener("keydown", (e) => e.key.startsWith("Arrow") && !e.repeat && remember());
    el.addEventListener("input", () => {
      set(Number(el.value));
      changed(true);
    });
    return () => {
      const v = get();
      if (v === undefined) return;
      el.value = v;
      $(id + "-v").textContent = shown(v);
    };
  }
  const shows = [
    slider("qb-width", () => Math.round(c.line.points.reduce((a, p) => a + p.width, 0) / Math.max(1, c.line.points.length)),
      (v) => c.line.points.forEach((p) => (p.width = v)), (v) => `${v} ft`),
    slider("qb-berms", () => c.line.berms, (v) => (c.line.berms = v), (v) => `${Math.round(v * 100)}%`),
    slider("qb-hills", () => c.land.hills, (v) => (c.land.hills = v), (v) => `${Math.round(v * 100)}%`),
    slider("qb-rough", () => c.land.rough, (v) => (c.land.rough = v), (v) => `${Math.round(v * 100)}%`),
    slider("qb-jh", () => c.jumps[sel.jump]?.height, (v) => (c.jumps[sel.jump].height = v), (v) => `${v.toFixed(1)} ft`),
    slider("qb-jl", () => c.jumps[sel.jump]?.length, (v) => (c.jumps[sel.jump].length = v), (v) => `${Math.round(v)} ft`),
    slider("qb-ja", () => c.jumps[sel.jump]?.at, (v) => (c.jumps[sel.jump].at = v), (v) => `${Math.round(v)} ft`),
  ];
  function showControls() {
    $("qb-name").value = c.name;
    $("qb-event").value = c.mode;
    $("qb-land").value = c.land.kind;
    $("qb-ja").max = Math.round(view?.length || 1000);
    $("qb-jump").hidden = !c.jumps[sel.jump];
    if (c.jumps[sel.jump]) $("qb-kind").value = c.jumps[sel.jump].kind;
    kindAbout();
    $("qb-delpoint").disabled = sel.point < 0;
    $("qb-undo").disabled = !undo.length;
    $("qb-redo").disabled = !redo.length;
    for (const s of shows) s();
  }

  $("qb-name").addEventListener("focus", remember);
  $("qb-name").addEventListener("input", () => {
    c.name = $("qb-name").value;
    unsaved = true;
  });
  $("qb-event").addEventListener("change", () => {
    remember();
    use(mm.course_change_event(text(), $("qb-event").value));
    whole();
  });
  $("qb-surprise").addEventListener("click", () => {
    remember();
    use(mm.course_surprise($("qb-event").value, (Math.random() * 1e9) >>> 0));
    whole();
    say("A new track made up at random. Undo brings yours back.");
  });
  $("qb-new").addEventListener("click", () => {
    remember();
    use(mm.course_new($("qb-event").value));
    whole();
  });
  const step = (from, to) => {
    if (!from.length) return;
    to.push(text());
    use(from.pop());
  };
  $("qb-undo").addEventListener("click", () => step(undo, redo));
  $("qb-redo").addEventListener("click", () => step(redo, undo));
  const setTool = (t) => {
    tool = t;
    $("qb-tool-line").setAttribute("aria-pressed", String(t === "line"));
    $("qb-tool-jumps").setAttribute("aria-pressed", String(t === "jumps"));
    $("qb-hint").textContent = t === "line"
      ? "Click the ground to add points. Drag a point to move it, double-click it to take it away. Drag the ground to move the view."
      : "Click the track to drop in a jump. Drag a jump along the track, or click it to change it. Drag the ground to move the view.";
    draw();
  };
  $("qb-tool-line").addEventListener("click", () => setTool("line"));
  $("qb-tool-jumps").addEventListener("click", () => setTool("jumps"));
  $("qb-kind").addEventListener("change", () => {
    kindAbout();
    const j = c.jumps[sel.jump];
    if (!j) return;
    remember();
    const made = JSON.parse(mm.course_jump(c.mode, $("qb-kind").value, j.at));
    c.jumps[sel.jump] = made;
    changed();
  });
  $("qb-fill").addEventListener("click", () => {
    remember();
    use(mm.course_fill_jumps(text(), (Math.random() * 1e9) >>> 0));
  });
  $("qb-clearjumps").addEventListener("click", () => {
    if (!c.jumps.length) return;
    remember();
    c.jumps = [];
    sel.jump = -1;
    changed();
  });
  const delPoint = () => {
    if (sel.point < 0 || c.line.points.length <= 3) return say("A track's line needs at least three points.", true);
    remember();
    c.line.points.splice(sel.point, 1);
    sel.point = -1;
    changed();
  };
  const delJump = () => {
    if (sel.jump < 0) return;
    remember();
    c.jumps.splice(sel.jump, 1);
    sel.jump = -1;
    changed();
  };
  $("qb-delpoint").addEventListener("click", delPoint);
  $("qb-deljump").addEventListener("click", delJump);
  $("qb-land").addEventListener("change", () => {
    remember();
    c.land.kind = $("qb-land").value;
    changed();
  });
  $("qb-reland").addEventListener("click", () => {
    remember();
    c.land.seed = (Math.random() * 1e9) >>> 0;
    changed();
  });
  panel.addEventListener("keydown", (e) => {
    if (e.target.matches("input[type=text]")) return;
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z") step(e.shiftKey ? redo : undo, e.shiftKey ? undo : redo);
    else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "y") step(redo, undo);
    else if (e.key === "Delete" || e.key === "Backspace") sel.jump >= 0 ? delJump() : delPoint();
    else return;
    e.preventDefault();
  });

  // ---- the preview, made again a moment after each change ----
  let unsaved = false;
  let pending = 0;
  function changed(quick = false) {
    unsaved = true;
    cancelAnimationFrame(pending);
    pending = requestAnimationFrame(() => {
      try {
        const p = mm.course_preview(text(), EVERY);
        view = { n: p.n, step: p.step, heights: p.heights(), colours: p.colours(), line: p.line(), gates: p.gates(), length: p.length, top: p.top(), widths: p.widths() };
        p.free();
        view.s = alongLine(view.line);
        ground = null;
        mesh3d = null;
      } catch (err) {
        say(`That didn't work: ${err.message}`, true);
      }
      showControls();
      draw();
      if (!quick) keep();
    });
    showControls();
    draw();
  }
  // How far along the line each of its points is.
  function alongLine(line) {
    const s = [0];
    for (let i = 2; i < line.length; i += 2) s.push(s[s.length - 1] + Math.hypot(line[i] - line[i - 2], line[i + 1] - line[i - 1]));
    return s;
  }
  // The point `at` ft along the line, and its direction.
  function onLine(at) {
    const { line, s } = view;
    let i = Math.max(0, s.findIndex((d) => d >= at) - 1);
    if (s[s.length - 1] < at) i = s.length - 2;
    const t = Math.min(1, Math.max(0, (at - s[i]) / Math.max(1e-3, s[i + 1] - s[i])));
    const [ax, az, bx, bz] = [line[i * 2], line[i * 2 + 1], line[i * 2 + 2], line[i * 2 + 3]];
    const len = Math.hypot(bx - ax, bz - az) || 1;
    return { x: ax + (bx - ax) * t, z: az + (bz - az) * t, dx: (bx - ax) / len, dz: (bz - az) / len };
  }
  // The nearest place on the line to x, z: how far along it is and how far away.
  function nearLine(x, z) {
    const { line, s } = view;
    let best = { d: Infinity, at: 0 };
    for (let i = 0; i + 3 < line.length; i += 2) {
      const [ax, az, bx, bz] = [line[i], line[i + 1], line[i + 2], line[i + 3]];
      const l2 = (bx - ax) ** 2 + (bz - az) ** 2 || 1;
      const t = Math.max(0, Math.min(1, ((x - ax) * (bx - ax) + (z - az) * (bz - az)) / l2));
      const d = Math.hypot(ax + (bx - ax) * t - x, az + (bz - az) * t - z);
      if (d < best.d) best = { d, at: s[i / 2] + t * Math.sqrt(l2) };
    }
    return best;
  }

  // ---- the map ----
  const map = $("qb-map");
  const ctx = map.getContext("2d");
  let ground = null; // the ground drawn once into its own canvas
  let cam = { x: 0, z: 0, scale: 0.2 }; // the map's middle, ft, and pixels per ft
  const side = () => event().side;
  const minScale = () => (Math.min(map.width, map.height) / side()) * 0.9;
  const maxScale = () => minScale() * 24;
  const toScreen = (x, z) => [map.width / 2 + (x - cam.x) * cam.scale, map.height / 2 + (z - cam.z) * cam.scale];
  const toWorld = (sx, sy) => [cam.x + (sx - map.width / 2) / cam.scale, cam.z + (sy - map.height / 2) / cam.scale];
  function whole() {
    sizeMap();
    // The track, with room round it (the land goes on beyond).
    const l = view?.line || [0, 0, side(), side()];
    let [x0, z0, x1, z1] = [Infinity, Infinity, -Infinity, -Infinity];
    for (let i = 0; i < l.length; i += 2) {
      [x0, x1] = [Math.min(x0, l[i]), Math.max(x1, l[i])];
      [z0, z1] = [Math.min(z0, l[i + 1]), Math.max(z1, l[i + 1])];
    }
    const pad = 1.25;
    const fit = Math.min(map.width / ((x1 - x0) * pad || 1), map.height / ((z1 - z0) * pad || 1));
    cam = { x: (x0 + x1) / 2, z: (z0 + z1) / 2, scale: Math.max(minScale(), Math.min(maxScale(), fit)) };
    zoomShown();
    draw();
    if (v3) frame3d();
  }
  function sizeMap() {
    const r = devicePixelRatio || 1;
    const w = Math.round(map.clientWidth * r);
    const h = Math.round(map.clientHeight * r);
    if (w && (map.width !== w || map.height !== h)) {
      map.width = w;
      map.height = h;
    }
  }
  new ResizeObserver(() => {
    const was = minScale();
    sizeMap();
    cam.scale *= minScale() / was || 1;
    draw();
  }).observe(map);

  function groundPicture() {
    const { n, heights, colours } = view;
    const cv = document.createElement("canvas");
    cv.width = cv.height = n;
    const img = cv.getContext("2d").createImageData(n, n);
    // Shaded by the slope, lit from the top left.
    for (let j = 0; j < n; j++)
      for (let i = 0; i < n; i++) {
        const k = j * n + i;
        const dx = heights[Math.min(n - 1, i + 1) + j * n] - heights[Math.max(0, i - 1) + j * n];
        const dz = heights[i + Math.min(n - 1, j + 1) * n] - heights[i + Math.max(0, j - 1) * n];
        const lit = Math.max(0.45, Math.min(1.35, 1 - (dx + dz) / (view.step * 3)));
        for (let ch = 0; ch < 3; ch++) img.data[k * 4 + ch] = Math.min(255, colours[k * 3 + ch] * lit);
        img.data[k * 4 + 3] = 255;
      }
    cv.getContext("2d").putImageData(img, 0, 0);
    return cv;
  }

  function draw() {
    if (!view || map.hidden) return;
    sizeMap();
    const r = devicePixelRatio || 1;
    ctx.fillStyle = "#1a1916";
    ctx.fillRect(0, 0, map.width, map.height);
    ground ||= groundPicture();
    const [gx, gy] = toScreen(0, 0);
    const span = view.step * (view.n - 1);
    ctx.imageSmoothingEnabled = true;
    ctx.drawImage(ground, gx, gy, span * cam.scale, span * cam.scale);
    // The smoothed line.
    const { line } = view;
    ctx.lineJoin = ctx.lineCap = "round";
    ctx.strokeStyle = "#00000060";
    ctx.lineWidth = Math.max(2 * r, event().width * cam.scale);
    ctx.beginPath();
    for (let i = 0; i < line.length; i += 2) ctx.lineTo(...toScreen(line[i], line[i + 1]));
    ctx.stroke();
    ctx.strokeStyle = "#f2e6c8";
    ctx.lineWidth = 1.5 * r;
    ctx.stroke();
    // The jumps, along the line.
    c.jumps.forEach((j, k) => {
      ctx.strokeStyle = k === sel.jump ? "#ffffff" : j.kind === "Dip" ? "#6fa0d8" : "#a3c24a";
      ctx.lineWidth = Math.max(5 * r, event().width * cam.scale * 0.8);
      ctx.lineCap = "butt";
      ctx.beginPath();
      for (let d = 0; d <= j.length; d += Math.max(2, j.length / 16)) {
        const p = onLine(j.at + d);
        ctx.lineTo(...toScreen(p.x, p.z));
      }
      ctx.stroke();
    });
    // Gates for Baja and Enduro.
    ctx.strokeStyle = "#e0896b";
    ctx.lineWidth = 2 * r;
    for (let i = 0; i + 3 < view.gates.length; i += 4) {
      ctx.beginPath();
      ctx.moveTo(...toScreen(view.gates[i], view.gates[i + 1]));
      ctx.lineTo(...toScreen(view.gates[i + 2], view.gates[i + 3]));
      ctx.stroke();
    }
    // The start: a chequered bar across the line.
    if (line.length >= 4) {
      const p = onLine(c.line.closed ? c.line.start || 0 : 0);
      const w = event().width * 0.7;
      ctx.strokeStyle = "#ffffff";
      ctx.lineWidth = 4 * r;
      ctx.setLineDash([4 * r, 4 * r]);
      ctx.beginPath();
      ctx.moveTo(...toScreen(p.x - p.dz * w, p.z + p.dx * w));
      ctx.lineTo(...toScreen(p.x + p.dz * w, p.z - p.dx * w));
      ctx.stroke();
      ctx.setLineDash([]);
    }
    // The points to drag, in line mode.
    if (tool === "line")
      c.line.points.forEach((p, k) => {
        const [sx, sy] = toScreen(p.x, p.z);
        ctx.beginPath();
        ctx.arc(sx, sy, (k === sel.point ? 7 : 5) * r, 0, Math.PI * 2);
        ctx.fillStyle = k === sel.point ? "#ffffff" : k === 0 ? "#a3c24a" : "#f2e6c8";
        ctx.fill();
        ctx.strokeStyle = "#121110";
        ctx.lineWidth = 1.5 * r;
        ctx.stroke();
      });
  }

  // Pointer: drag points or jumps, click to add, drag the ground to pan,
  // two fingers or the wheel to zoom.
  const touches = new Map();
  let drag = null;
  const at = (e) => {
    const b = map.getBoundingClientRect();
    const r = map.width / b.width;
    return [(e.clientX - b.left) * r, (e.clientY - b.top) * r];
  };
  function hitPoint(sx, sy) {
    const r = (devicePixelRatio || 1) * 10;
    let best = -1;
    let bd = r;
    c.line.points.forEach((p, k) => {
      const [px, py] = toScreen(p.x, p.z);
      const d = Math.hypot(px - sx, py - sy);
      if (d < bd) [best, bd] = [k, d];
    });
    return best;
  }
  function hitJump(x, z) {
    const near = nearLine(x, z);
    if (near.d > event().width + 12 / cam.scale) return -1;
    return c.jumps.findIndex((j) => near.at >= j.at - 6 && near.at <= j.at + j.length + 6);
  }
  map.addEventListener("pointerdown", (e) => {
    map.setPointerCapture(e.pointerId);
    touches.set(e.pointerId, at(e));
    if (touches.size === 2) {
      const [a, b] = [...touches.values()];
      drag = { pinch: Math.hypot(a[0] - b[0], a[1] - b[1]), scale: cam.scale };
      return;
    }
    const [sx, sy] = at(e);
    const [x, z] = toWorld(sx, sy);
    const from = { sx, sy, moved: false };
    if (e.button === 0 && tool === "line") {
      const k = hitPoint(sx, sy);
      if (k >= 0) {
        sel = { point: k, jump: -1 };
        remember();
        drag = { ...from, point: k };
        return changed(true);
      }
    }
    if (e.button === 0 && tool === "jumps" && view) {
      const k = hitJump(x, z);
      if (k >= 0) {
        sel = { point: -1, jump: k };
        remember();
        drag = { ...from, jump: k, off: nearLine(x, z).at - c.jumps[k].at };
        return changed(true);
      }
    }
    drag = { ...from, pan: [cam.x, cam.z], x, z };
  });
  map.addEventListener("pointermove", (e) => {
    if (!touches.has(e.pointerId) || !drag) return;
    touches.set(e.pointerId, at(e));
    if (drag.pinch) {
      const [a, b] = [...touches.values()];
      setScale((drag.scale * Math.hypot(a[0] - b[0], a[1] - b[1])) / drag.pinch);
      return;
    }
    const [sx, sy] = at(e);
    if (Math.hypot(sx - drag.sx, sy - drag.sy) > 4 * (devicePixelRatio || 1)) drag.moved = true;
    if (!drag.moved) return;
    const [x, z] = toWorld(sx, sy);
    const s = side();
    if (drag.point !== undefined) {
      const p = c.line.points[drag.point];
      p.x = Math.max(0, Math.min(s, x));
      p.z = Math.max(0, Math.min(s, z));
      changed(true);
    } else if (drag.jump !== undefined) {
      c.jumps[drag.jump].at = Math.max(0, Math.min(view.length - c.jumps[drag.jump].length, nearLine(x, z).at - drag.off));
      changed(true);
    } else if (drag.pan) {
      cam.x = drag.pan[0] - (sx - drag.sx) / cam.scale;
      cam.z = drag.pan[1] - (sy - drag.sy) / cam.scale;
      draw();
    }
  });
  const up = (e) => {
    touches.delete(e.pointerId);
    const d = drag;
    if (touches.size || !d) return void (drag = null);
    drag = null;
    if (d.pinch) return;
    if (d.point !== undefined || d.jump !== undefined) {
      if (d.moved) changed();
      else undo.pop(); // only a click: nothing changed
      return;
    }
    if (d.moved || e.button !== 0 || e.type === "pointercancel") return;
    // A click on the ground.
    if (tool === "line") addPoint(d.x, d.z);
    else addJump(d.x, d.z);
  };
  map.addEventListener("pointerup", up);
  map.addEventListener("pointercancel", up);
  map.addEventListener("dblclick", (e) => {
    const k = hitPoint(...at(e));
    if (tool === "line" && k >= 0) {
      sel.point = k;
      delPoint();
    }
  });
  map.addEventListener("contextmenu", (e) => e.preventDefault());
  map.addEventListener("wheel", (e) => {
    e.preventDefault();
    const [sx, sy] = at(e);
    const [x, z] = toWorld(sx, sy);
    setScale(cam.scale * Math.pow(1.0015, -e.deltaY));
    // Zoom on the pointer.
    const [nx, nz] = toWorld(sx, sy);
    cam.x += x - nx;
    cam.z += z - nz;
    draw();
  }, { passive: false });

  // A new point goes into the line between the two it's nearest.
  function addPoint(x, z) {
    const pts = c.line.points;
    let best = pts.length;
    let bd = Infinity;
    const n = pts.length;
    for (let i = 0; i < n; i++) {
      const a = pts[i];
      const b = pts[(i + 1) % n];
      if (!c.line.closed && i === n - 1) break;
      const l2 = (b.x - a.x) ** 2 + (b.z - a.z) ** 2 || 1;
      const t = Math.max(0, Math.min(1, ((x - a.x) * (b.x - a.x) + (z - a.z) * (b.z - a.z)) / l2));
      const d = Math.hypot(a.x + (b.x - a.x) * t - x, a.z + (b.z - a.z) * t - z);
      if (d < bd) [bd, best] = [d, i + 1];
    }
    // Off the end of a point-to-point line, it goes on the nearer end.
    if (!c.line.closed && n) {
      const d0 = Math.hypot(pts[0].x - x, pts[0].z - z);
      const d1 = Math.hypot(pts[n - 1].x - x, pts[n - 1].z - z);
      if (Math.min(d0, d1) < bd * 0.8) best = d0 < d1 ? 0 : n;
    }
    remember();
    const width = pts[Math.min(best, n - 1)]?.width || event().width;
    pts.splice(best, 0, { x, z, width });
    sel = { point: best, jump: -1 };
    changed();
  }
  function addJump(x, z) {
    if (!view) return;
    const near = nearLine(x, z);
    if (near.d > event().width * 1.5 + 10 / cam.scale) return say("Click on the track to drop in a jump.", true);
    const j = JSON.parse(mm.course_jump(c.mode, $("qb-kind").value, Math.max(0, near.at - 10)));
    remember();
    c.jumps.push(j);
    c.jumps.sort((a, b) => a.at - b.at);
    sel = { point: -1, jump: c.jumps.indexOf(j) };
    say("");
    changed();
  }

  // ---- zoom, tilt and turn as sliders ----
  const zoomOf = (scale) => Math.log(scale / minScale()) / Math.log(maxScale() / minScale());
  function setScale(scale) {
    cam.scale = Math.max(minScale(), Math.min(maxScale(), scale));
    zoomShown();
    draw();
  }
  function zoomShown() {
    if (!v3) $("qb-zoom").value = zoomOf(cam.scale);
  }
  $("qb-zoom").addEventListener("input", () => {
    const t = Number($("qb-zoom").value);
    if (v3) return set3d({ zoom: t });
    setScale(minScale() * Math.pow(maxScale() / minScale(), t));
  });
  $("qb-tilt").addEventListener("input", () => set3d({ tilt: Number($("qb-tilt").value) }));
  $("qb-turn").addEventListener("input", () => set3d({ turn: Number($("qb-turn").value) }));
  $("qb-whole").addEventListener("click", whole);

  // Expand: the builder fills the browser tab (not the whole screen).
  const box = panel.querySelector(".qb-side");
  const maxed = (on) => {
    box.classList.toggle("qb-max", on);
    document.body.classList.toggle("qb-maxed", on);
    $("qb-fs").textContent = on ? "Back to the page" : "Expand";
    $("qb-fs").setAttribute("aria-pressed", String(on));
    requestAnimationFrame(() => {
      draw();
      viewer?.resize();
    });
  };
  $("qb-fs").addEventListener("click", () => maxed(!box.classList.contains("qb-max")));
  document.addEventListener("keydown", (e) => e.key === "Escape" && box.classList.contains("qb-max") && maxed(false));

  // ---- the 3D look ----
  let v3 = false;
  let viewer = null;
  let mesh3d = null;
  const sph = new THREE.Spherical();
  function show3d(on) {
    v3 = on;
    $("qb-v2d").setAttribute("aria-pressed", String(!on));
    $("qb-v3d").setAttribute("aria-pressed", String(on));
    map.hidden = on;
    $("qb-3d").hidden = !on;
    for (const el of panel.querySelectorAll(".qb-3donly")) el.hidden = !on;
    $("qb-hint").textContent = on ? "Drag to turn, right-drag to move, scroll or pinch (or the sliders) to zoom." : $("qb-hint").textContent;
    if (on) {
      viewer ||= new Viewer($("qb-3d"), { sliders: false });
      viewer.grid.visible = false;
      viewer.controls.addEventListener("change", shown3d);
      build3d(true);
    } else {
      setTool(tool);
      zoomShown();
    }
  }
  $("qb-v2d").addEventListener("click", () => show3d(false));
  $("qb-v3d").addEventListener("click", () => show3d(true));

  function build3d(reframe = false) {
    if (!v3 || !view || mesh3d) return;
    const { n, step, heights, colours, line } = view;
    const pos = new Float32Array(n * n * 3);
    const col = new Float32Array(n * n * 3);
    for (let k = 0; k < n * n; k++) {
      pos[k * 3] = (k % n) * step;
      pos[k * 3 + 1] = heights[k];
      pos[k * 3 + 2] = Math.floor(k / n) * step;
      for (let ch = 0; ch < 3; ch++) col[k * 3 + ch] = (colours[k * 3 + ch] / 255) ** 2.2;
    }
    const index = [];
    for (let j = 0; j < n - 1; j++)
      for (let i = 0; i < n - 1; i++) {
        const a = j * n + i;
        index.push(a, a + n, a + 1, a + 1, a + n, a + n + 1);
      }
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.BufferAttribute(pos, 3));
    g.setAttribute("color", new THREE.BufferAttribute(col, 3));
    g.setIndex(index);
    g.computeVertexNormals();
    const group = new THREE.Group();
    group.add(new THREE.Mesh(g, new THREE.MeshLambertMaterial({ vertexColors: true })));
    // The track as a ribbon on its bed, the jumps picked out in green.
    const { top, widths, s: along } = view;
    const m = line.length / 2;
    const rp = new Float32Array(m * 6);
    const rc = new Float32Array(m * 6);
    const dirt = [0.32, 0.2, 0.11];
    const jump = [0.42, 0.6, 0.12];
    for (let i = 0; i < m; i++) {
      const a = Math.max(0, i - 1);
      const b = Math.min(m - 1, i + 1);
      let dx = line[b * 2] - line[a * 2];
      let dz = line[b * 2 + 1] - line[a * 2 + 1];
      const l = Math.hypot(dx, dz) || 1;
      [dx, dz] = [dx / l, dz / l];
      const hw = (widths[i] || event().width) / 2;
      const y = (top[i] ?? 0) + 0.4;
      rp.set([line[i * 2] - dz * hw, y, line[i * 2 + 1] + dx * hw, line[i * 2] + dz * hw, y, line[i * 2 + 1] - dx * hw], i * 6);
      const on = c.jumps.some((j) => along[i] >= j.at && along[i] <= j.at + j.length);
      const col3 = on ? jump : dirt;
      rc.set([...col3, ...col3], i * 6);
    }
    const ri = [];
    for (let i = 0; i + 1 < m; i++) ri.push(i * 2, i * 2 + 1, i * 2 + 2, i * 2 + 1, i * 2 + 3, i * 2 + 2);
    if (c.line.closed && m > 2) ri.push((m - 1) * 2, (m - 1) * 2 + 1, 0, (m - 1) * 2 + 1, 1, 0);
    const rg = new THREE.BufferGeometry();
    rg.setAttribute("position", new THREE.BufferAttribute(rp, 3));
    rg.setAttribute("color", new THREE.BufferAttribute(rc, 3));
    rg.setIndex(ri);
    rg.computeVertexNormals();
    group.add(new THREE.Mesh(rg, new THREE.MeshLambertMaterial({ vertexColors: true, side: THREE.DoubleSide })));
    viewer.clear();
    viewer.show(group);
    mesh3d = group;
    if (reframe || !viewer.framed) frame3d();
  }
  function frame3d() {
    if (!viewer || !mesh3d) return;
    viewer.frame(mesh3d.children[1], new THREE.Vector3(0.2, 0.75, 1));
    viewer.framed = true;
    viewer.controls.maxDistance = side() * 2;
    viewer.controls.minDistance = 30;
    shown3d();
  }
  // The sliders follow the view, and set it.
  function shown3d() {
    if (!v3) return;
    const ctl = viewer.controls;
    sph.setFromVector3(viewer.camera.position.clone().sub(ctl.target));
    $("qb-zoom").value = 1 - Math.log(sph.radius / ctl.minDistance) / Math.log(ctl.maxDistance / ctl.minDistance);
    $("qb-tilt").value = 1 - sph.phi / (Math.PI / 2);
    $("qb-turn").value = ((sph.theta / (Math.PI * 2)) % 1 + 1) % 1;
  }
  function set3d({ zoom, tilt, turn }) {
    const ctl = viewer.controls;
    sph.setFromVector3(viewer.camera.position.clone().sub(ctl.target));
    if (zoom !== undefined) sph.radius = ctl.minDistance * Math.pow(ctl.maxDistance / ctl.minDistance, 1 - zoom);
    if (tilt !== undefined) sph.phi = Math.max(0.02, (1 - tilt) * (Math.PI / 2) * 0.98);
    if (turn !== undefined) sph.theta = turn * Math.PI * 2;
    viewer.camera.position.setFromSpherical(sph).add(ctl.target);
    viewer.camera.lookAt(ctl.target);
    ctl.update();
  }
  const draw2 = draw;
  draw = () => {
    draw2();
    build3d();
  };

  // ---- saving ----
  // Kept as the game's builder keeps its tracks, so it opens on them.
  async function keepCourse() {
    const stem = mm.course_stem(text());
    const path = `${COURSES}${stem}.course`;
    await fs.keepUser(path, enc.encode(JSON.stringify(c, null, 2)));
    await fs.keepUser(COURSES + "last.txt", enc.encode(path));
    unsaved = false;
    return stem;
  }
  // The .course is kept a moment after each change, so nothing is lost.
  let keepTimer = 0;
  function keep() {
    clearTimeout(keepTimer);
    keepTimer = setTimeout(() => keepCourse().catch((err) => console.warn("not kept", err)), 800);
  }
  let busy = false;
  // Save / Export, then which game it's for.
  const askFor = (on) => {
    $("qb-for").hidden = !on;
    $("qb-save").setAttribute("aria-expanded", String(on));
  };
  // A bake in the worker, with a bar. The bake can't say how far it's got,
  // so the bar follows how long the last one for this race took here
  // (or a first guess), and fills when it's done.
  const GUESS = { open: { Supercross: 4, National: 9, Baja: 15, Enduro: 9 }, orig: { Supercross: 12, National: 35, Baja: 50, Enduro: 35 } };
  async function baked(what) {
    const key = `mm2bake-${what}-${c.mode}`;
    let expect = GUESS[what === "original" ? "orig" : "open"][c.mode] || 20;
    try {
      expect = Number(localStorage.getItem(key)) || expect;
    } catch {}
    const box = $("qb-progress");
    const bar = box.querySelector("progress");
    const label = box.querySelector("span");
    box.hidden = false;
    bar.value = 0;
    const t0 = performance.now();
    const tick = setInterval(() => {
      const t = (performance.now() - t0) / 1000;
      // Eases towards the end rather than stopping, if it runs long.
      bar.value = Math.min(0.97, 1 - Math.exp(-t / expect * 1.6));
      const left = Math.max(1, Math.round(expect - t));
      label.textContent = t < expect ? `About ${left} s to go` : "Nearly there…";
    }, 200);
    try {
      const out = await bakeOff(what, text());
      bar.value = 1;
      label.textContent = "Done";
      try {
        localStorage.setItem(key, String(Math.round((performance.now() - t0) / 100) / 10));
      } catch {}
      return out;
    } finally {
      clearInterval(tick);
      setTimeout(() => (box.hidden = true), 1500);
    }
  }
  async function saveTrack() {
    const stem = await keepCourse();
    say("Saving the track for OpenMCM2…");
    const env = await baked("bake");
    await fs.keepUser(`/user/Tracks/${event().folder}/${stem}.ENV`, env);
    return { stem, env };
  }
  async function saveGame() {
    const { stem } = await saveTrack();
    say(`Saved as ${stem}. It's in the browser game's ${event().name} track list, and the full Course Builder opens on it.`);
  }
  async function exportOpen() {
    say("Making a copy for OpenMCM2…");
    const stem = await keepCourse();
    const env = await baked("bake");
    download(new Blob([env]), `${stem}.ENV`);
    say(`Downloaded ${stem}.ENV. Other players put it in OpenMCM2's TERAFORM\\${event().folder} folder (or add it on the browser game's Your own tracks).`);
  }
  async function saveOriginal() {
    say("Making a copy for MM2… (a big track takes up to a minute)");
    const stem = mm.course_stem(text());
    keepCourse().catch(() => {});
    const bytes = await baked("original");
    download(new Blob([bytes]), `${stem}.ENV`);
    say(`Downloaded ${stem}.ENV. MM2 players put it in the game's TERAFORM\\${event().folder} folder and it shows in the ${event().name} track list. OpenMCM2 loads it too.`);
  }
  $("qb-save").addEventListener("click", () => !busy && askFor($("qb-for").hidden));
  $("qb-for-no").addEventListener("click", () => askFor(false));
  const saveFor = async (how) => {
    if (busy) return;
    askFor(false);
    busy = true;
    $("qb-save").disabled = true;
    try {
      await how();
    } catch (err) {
      say(`Couldn't save the track: ${err.message}. Try a normal (not private) window.`, true);
    } finally {
      busy = false;
      $("qb-save").disabled = false;
    }
  };
  $("qb-for-game").addEventListener("click", () => saveFor(saveGame));
  $("qb-for-open").addEventListener("click", () => saveFor(exportOpen));
  $("qb-for-orig").addEventListener("click", () => saveFor(saveOriginal));
  $("qb-download").addEventListener("click", () => {
    download(new Blob([JSON.stringify(c, null, 2)], { type: "application/json" }), `${mm.course_stem(text())}.course`);
  });
  $("qb-open").addEventListener("change", async (e) => {
    const f = e.target.files[0];
    e.target.value = "";
    if (!f) return;
    try {
      const bytes = new Uint8Array(await f.arrayBuffer());
      const json = /\.env$/i.test(f.name) ? mm.course_from_env(bytes) : dec.decode(bytes);
      if (!json) return say("That track wasn't made in the Course Builder, so it can't be opened here. Copy it in the full Course Builder's Open instead.", true);
      mm.course_stem(json); // checks it reads
      remember();
      use(json);
      whole();
      say(`Opened ${f.name}.`);
    } catch (err) {
      say(`Couldn't open ${f.name}: ${err.message}`, true);
    }
  });

  // ---- the full builder, with the player's files ----
  const kept = await fs.keptGame().catch(() => null);
  const demo = !!kept?.some(([p]) => p === DEMO_PATH);
  const game = !!kept && !demo;
  const pack = !!((await fs.keptPack().catch(() => null)) || (await fs.packGet("manifest").catch(() => null)));
  const anything = game || demo || pack;
  const status = (msg, bad = false) => {
    $("b-status").textContent = msg;
    $("b-status").classList.toggle("err", bad);
  };
  const saved = [game && "your MM2 game", demo && "the MM2 demo", pack && "the free version"].filter(Boolean);
  $("b-saved").textContent = saved.length
    ? `Saved in this browser: ${saved.join(" and ")}. Open the full Course Builder and it starts with that.`
    : "Nothing saved in this browser yet. Pick one, and the full Course Builder opens on your track as soon as it's ready.";
  $("b-mm2").querySelector(".bsaved").hidden = !game;
  $("b-demo").querySelector(".bsaved").hidden = !demo;
  $("b-free").querySelector(".bsaved").hidden = !pack;
  $("b-mm2-go").hidden = !game;
  if (game) {
    $("b-folder-label").textContent = "Use another folder";
    $("b-file-label").textContent = "Use another CD image or .zip";
  }
  let ride = false;
  async function open(how) {
    try {
      say("Saving your track…");
      await keepCourse();
    } catch (err) {
      return say(`Couldn't save the track: ${err.message}. Try a normal (not private) window.`, true);
    }
    location.assign(`../play/?course-builder${how ? "=" + how : ""}${ride ? "&ride-course" : ""}`);
  }
  // Nothing saved: pick what to open with first.
  function openOrAsk(riding) {
    ride = riding;
    if (anything) return open("");
    status(riding ? "Pick what to ride your track with:" : "Pick what to open the full Course Builder with:");
    $("b-full").scrollIntoView({ behavior: "smooth", block: "start" });
  }
  $("qb-full").addEventListener("click", () => openOrAsk(false));
  $("qb-ride").addEventListener("click", () => openOrAsk(true));
  $("b-mm2-go").addEventListener("click", () => open("mm2"));
  $("b-demo-go").addEventListener("click", () => open("demo"));
  $("b-free-go").addEventListener("click", () => open("free"));
  // A game folder: saved by its path under the folder holding UI/MAINMENU.DAT,
  // as the browser version saves it.
  $("b-folder").addEventListener("change", async (e) => {
    const files = [...e.target.files];
    e.target.value = "";
    const found = files.map((f) => /^(.*?)ui\/mainmenu\.dat$/i.exec(f.webkitRelativePath)).find(Boolean);
    if (!found) return status("That folder has no UI/MAINMENU.DAT in it. Pick the folder the game is installed in, or the CD.", true);
    const root = found[1];
    await keepGame(files.filter((f) => f.webkitRelativePath.startsWith(root)).map((f) => ["/game/" + f.webkitRelativePath.slice(root.length), f]));
  });
  // A CD image or zip: saved as itself; the browser version reads it.
  $("b-file").addEventListener("change", async (e) => {
    const file = e.target.files[0];
    e.target.value = "";
    if (file) await keepGame([["/file", file]]);
  });
  async function keepGame(list) {
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

  // ---- start on the last track ----
  use(await lastTrack());
  unsaved = false;
  setTool("line");
  requestAnimationFrame(whole);
  return {};
}
