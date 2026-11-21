// Bikes & riders: the game's and the Free Pack's in 3D, with their
// paints, outfits and motions, saved as .glb or a picture.

import { Viewer, save, glbOf, THREE } from "./scene3d.js";
import { gameRoster, paintTexture, gameAnimated, bikeMotionFor, packRoster, packModel, packTexture, ownMotions, ownClip, moveClip } from "./models.js";

const el = (tag, props = {}, ...kids) => {
  const e = Object.assign(document.createElement(tag), props);
  e.append(...kids);
  return e;
};
const row = (label, title, input) => el("label", { className: "row", title }, label, input);
const option = (value, text) => el("option", { value, textContent: text });
const nice = (s) => s.replace(/^b(?=[A-Z])/, "").replace(/^(Wait|LookLeft|LookRight|Stretch)R$/, "$1").replace(/([a-z])([A-Z0-9])/g, "$1 $2");

export async function mount(app, panel) {
  const { lib } = app;
  const box = panel.querySelector("#garage-controls");
  const viewer = new Viewer(panel.querySelector("#garage-view"));
  let state = { set: null };
  let bike = null;
  let rider = null;
  let token = 0;

  async function build() {
    box.replaceChildren();
    const game = lib.has("game") ? await gameRoster(lib) : null;
    const pack = lib.has("pack") ? packRoster(lib) : null;
    if (!game && !pack) {
      box.append(el("p", { className: "hint", textContent: "Pick your game or the Free Pack first (at the top of the page)." }));
      return;
    }
    const sets = [game && ["game", "The game's"], pack?.bikes.length && ["pack", "Free Pack"]].filter(Boolean);
    if (!state.set || !sets.some(([k]) => k === state.set)) state.set = sets[0][0];
    if (sets.length > 1) {
      const seg = el("div", { className: "seg", title: "Whose bikes and riders to show: the original game's, or our own free ones." });
      for (const [k, label] of sets) {
        const b = el("button", { textContent: label, ariaPressed: String(state.set === k) });
        b.onclick = () => ((state.set = k), build());
        seg.append(b);
      }
      box.append(seg);
    }
    const bikeSel = el("select");
    const outfitSel = el("select");
    const motionSel = el("select");
    const showBike = el("input", { type: "checkbox", checked: true });
    const showRider = el("input", { type: "checkbox", checked: true });
    const speed = el("input", { type: "range", min: "0", max: "2", step: "0.05", value: "1" });
    const pause = el("button", { textContent: "Pause", title: "Stops the motion where it is (press again to carry on)." });

    if (state.set === "game") {
      game.bikes.forEach((b, i) => bikeSel.append(option(i, b.name)));
      game.outfits.forEach((o, i) => outfitSel.append(option(i, o.name)));
    } else {
      for (const b of pack.bikes) for (const c of b.colours.length ? b.colours : [""]) bikeSel.append(option(`${b.shape}:${c}`, `${b.shape === "mx2" ? "Ridgeback" : b.shape === "two" ? "Kestrel" : b.shape} ${c}`.trim()));
      for (const c of pack.rider?.colours || []) outfitSel.append(option(c, `Race kit, ${c}`));
      if ([...bikeSel.options].some((o) => o.value === "mx2:lime")) bikeSel.value = "mx2:lime";
      if (pack.rider?.colours.includes("lime")) outfitSel.value = "lime";
      if (!pack.rider) outfitSel.append(option("", "No rider in this pack"));
    }
    box.append(
      row("Bike", "The bike and its paint.", bikeSel),
      row("Rider", "The rider's outfit.", outfitSel),
      el("div", { className: "buttons" },
        el("label", { className: "check", title: "Show or hide the bike." }, showBike, "Bike"),
        el("label", { className: "check", title: "Show or hide the rider." }, showRider, "Rider")),
      row("Motion", "What the rider and bike are doing. The game mixes several of these as you ride; here each plays on its own.", motionSel),
      row("Speed", "How fast the motion plays.", speed),
      el("div", { className: "buttons" },
        pause,
        el("button", { textContent: "Save as .glb", title: "Saves what's shown (bike, rider or both) with its paint and the motion playing, as a .glb for Blender and most 3D programs.", onclick: () => exportGlb(false) }),
        el("button", { textContent: "Save with every motion", title: "Saves what's shown with all of its motions as separate animations in one .glb.", onclick: () => exportGlb(true) }),
        el("button", { textContent: "Save a picture", title: "Saves the view as it is now as a .png.", onclick: async () => save(await viewer.snapshot(), "bike-and-rider.png") }))
    );

    // The motions on offer: pairs of a rider's and a bike's.
    let motions = [];
    const loadAll = async () => {
      const my = ++token;
      viewer.clear();
      bike = rider = null;
      if (state.set === "game") {
        const b = game.bikes[+bikeSel.value] || game.bikes[0];
        const o = game.outfits[+outfitSel.value] || game.outfits[0];
        const archives = [game.bikeRes];
        const [paint, outfit] = await Promise.all([paintTexture(lib, b, archives), o ? paintTexture(lib, o, archives) : null]);
        const [race, ui, rRace, rUi] = await Promise.all([
          gameAnimated(lib, b.model, { skin: paint, prefix: "bike_", plate: 7 }),
          gameAnimated(lib, "ui" + b.model, { prefix: "bike_", plate: 7 }),
          gameAnimated(lib, "Rider.mcf", { skin: outfit, prefix: "rider_" }),
          gameAnimated(lib, "UIRider.mcf", { prefix: "rider_" }),
        ]);
        if (my !== token) return;
        bike = race && { ...race, kind: "game" };
        rider = rRace && { ...rRace, kind: "game" };
        const bikeMotions = [...(ui?.motions || []).map((m) => ({ ...m, from: ui })), ...(race?.motions || []).map((m) => ({ ...m, from: race }))];
        motions = [];
        for (const [src, group] of [[rUi, "On the menu"], [rRace, "Riding, tricks and crashes"]]) {
          for (const m of src?.motions || []) {
            const bn = bikeMotionFor(m.name, bikeMotions);
            const bm = bikeMotions.find((x) => x.name === bn);
            motions.push({ group, label: nice(m.name), rider: () => src.clip(m.name), bike: () => bm?.from.clip(bm.name) || null });
          }
        }
      } else {
        const [shape, colour] = bikeSel.value.split(":");
        const b = pack.bikes.find((x) => x.shape === shape) || pack.bikes[0];
        const [bm, rm, own] = await Promise.all([
          b ? packModel(lib, b.glb) : null,
          pack.rider ? packModel(lib, pack.rider.glb) : null,
          ownMotions(),
        ]);
        if (my !== token) return;
        if (bm && colour) bm.setPaint(await packTexture(lib, b.livery(colour)).catch(() => null));
        if (rm && outfitSel.value) rm.setPaint(await packTexture(lib, pack.rider.outfit(outfitSel.value)).catch(() => null));
        if (my !== token) return;
        const names = (o) => {
          const n = [];
          o.traverse((x) => x.name && n.push(x.name));
          return n;
        };
        bike = bm && { ...bm, kind: "pack" };
        rider = rm && { ...rm, kind: "pack" };
        const bikeNames = bm ? names(bm.obj) : [];
        const riderNames = rm ? names(rm.obj) : [];
        const ownBike = own.filter((m) => !m.rider);
        motions = own
          .filter((m) => m.rider)
          .map((m) => {
            const bn = bikeMotionFor(m.name, ownBike);
            const b2 = ownBike.find((x) => x.name === bn);
            return { group: "The Free Pack's own", label: nice(m.name), rider: () => rm && ownClip(m, true, riderNames), bike: () => (b2 && bm ? ownClip(b2, false, bikeNames) : null) };
          });
        // With the game here too, its motions fit the pack's rider.
        if (game && rm) {
          const rRace = await gameAnimated(lib, "Rider.mcf", { prefix: "rider_" });
          const bRace = await gameAnimated(lib, "yamaha.mcf", { prefix: "bike_" });
          if (my !== token) return;
          const map = (src) => src.obj.userData.nodes.map((n) => [n.name, n.userData.frame]);
          for (const m of rRace?.motions || []) {
            const bn = bRace && bikeMotionFor(m.name, bRace.motions);
            motions.push({
              group: "The game's",
              label: nice(m.name),
              rider: () => {
                const c = rRace.clip(m.name);
                return c && moveClip(c, map(rRace), riderNames);
              },
              bike: () => {
                const c = bn && bRace.clip(bn);
                return c && bm ? moveClip(c, map(bRace), bikeNames) : null;
              },
            });
          }
        }
      }
      motionSel.replaceChildren(option("-1", "Standing still"));
      const groups = new Map();
      motions.forEach((m, i) => {
        if (!groups.has(m.group)) groups.set(m.group, el("optgroup", { label: m.group }));
        groups.get(m.group).append(option(i, m.label));
      });
      motionSel.append(...groups.values());
      const first = motions.findIndex((m) => /^(wait r|turn 2d)$/i.test(m.label));
      motionSel.value = String(first >= 0 ? first : motions.length ? 0 : -1);
      if (bike) viewer.show(bike.obj);
      if (rider) viewer.show(rider.obj);
      visibility();
      play();
      viewer.frame(viewer.content, new THREE.Vector3(1.2, 0.5, 1));
    };
    const visibility = () => {
      if (bike) bike.obj.visible = showBike.checked;
      if (rider) rider.obj.visible = showRider.checked;
    };
    let current = { rider: null, bike: null };
    const play = () => {
      viewer.mixers = [];
      bike?.rest();
      rider?.rest();
      const m = motions[+motionSel.value];
      current = { rider: m?.rider() || null, bike: m?.bike() || null };
      const loop = true;
      for (const [part, clip] of [[rider, current.rider], [bike, current.bike]]) if (part && clip) viewer.play(part.obj, clip, loop).timeScale = +speed.value;
      pause.textContent = "Pause";
    };
    const exportGlb = async (all) => {
      const group = new THREE.Group();
      group.name = "BikeAndRider";
      const clips = [];
      const parts = [showBike.checked && bike, showRider.checked && rider].filter(Boolean);
      for (const p of parts) {
        p.rest();
        group.add(p.obj);
      }
      const pick = all ? motions : [motions[+motionSel.value]].filter(Boolean);
      for (const m of pick) {
        const tr = [];
        if (showRider.checked && rider) tr.push(...(m.rider()?.tracks || []));
        if (showBike.checked && bike) tr.push(...(m.bike()?.tracks || []));
        if (tr.length) clips.push(new THREE.AnimationClip(m.label, -1, tr));
      }
      try {
        save(await glbOf(group, clips), parts.length === 2 ? "bike-and-rider.glb" : (parts[0] === bike ? "bike.glb" : "rider.glb"));
      } finally {
        for (const p of parts) viewer.show(p.obj);
        play();
      }
    };
    bikeSel.onchange = outfitSel.onchange = loadAll;
    motionSel.onchange = play;
    showBike.onchange = showRider.onchange = visibility;
    speed.oninput = () => viewer.mixers.forEach((m) => (m.timeScale = +speed.value));
    pause.onclick = () => {
      const paused = pause.textContent === "Pause";
      viewer.mixers.forEach((m) => (m.timeScale = paused ? 0 : +speed.value));
      pause.textContent = paused ? "Carry on" : "Pause";
    };
    await loadAll();
  }
  lib.onChange(() => !panel.hidden && build());
  // Rebuilt when shown after the files changed.
  let dirty = false;
  lib.onChange(() => (dirty = panel.hidden));
  new MutationObserver(() => !panel.hidden && dirty && ((dirty = false), build())).observe(panel, { attributes: true, attributeFilter: ["hidden"] });
  await build().catch((e) => {
    console.warn(e);
    box.append(el("p", { className: "err", textContent: `Couldn't show the bikes: ${e.message}` }));
  });
  return {};
}
