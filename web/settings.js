// The launcher's settings on the start page: the look, the enhanced
// look's options and the extras, kept in this browser and handed to the
// game as its command line (through the page's address, which the game
// reads as its options). This mirrors crates/mm2-settings (Settings and
// Settings::args) and the launcher's Graphics and Extras sections: a new
// extra there wants a line in EXTRAS here too, if it works in a browser.
// Window size, VSync, OpenGL, high-res textures, the screenshot key and
// updates are the desktop's only.

// The enhanced look's switches: the name --enhanced knows, what it is,
// and whether it's on unless turned off (mm2_settings::UPGRADES).
export const UPGRADES = [
  ["lighting", "Per-pixel lighting", true],
  ["shadows", "Sun shadows", true],
  ["distance", "Far draw distance and haze", true],
  ["bloom", "HDR bloom", true],
  ["ao", "Ambient occlusion", true],
  ["ground", "Detailed ground", true],
  ["grass", "3D grass", true],
  ["smooth", "Smooth hills", true],
  ["dust", "Dust clouds", true],
  ["mud", "Mud and spray", true],
  ["bikes", "Glossier bikes", true],
  ["shafts", "Light shafts", true],
  ["cinematic", "Replay blur and depth of field", false],
  ["ruts", "Lasting tyre ruts", false],
  ["scenery", "Trackside props and crowd", false],
  ["grade", "Colour grade", false],
];

const QUALITIES = ["low", "medium", "high", "ultra"];
// Whether a quality preset keeps an upgrade on (Quality::has).
function presetHas(quality, name) {
  const off = { low: ["ao", "shafts", "smooth", "cinematic", "grass"], medium: ["shafts", "smooth"] }[quality] || [];
  return !off.includes(name) && UPGRADES.some(([n, , on]) => n === name && on);
}

const TIMES = [["", "Track's own"], ["dawn", "Dawn"], ["morning", "Morning"], ["noon", "Noon"], ["afternoon", "Afternoon"], ["dusk", "Dusk"]];
const WEATHERS = [["clear", "Clear"], ["overcast", "Overcast"], ["rain", "Rain"]];

// The extras the original game didn't have, all off unless turned on,
// by group: the setting, its flag, the label and what it does. A number
// has its range, step, unit and the original game's value.
export const EXTRAS = [
  ["Riding", [
    { key: "quick_get_up", flag: "quick-get-up", label: "Quick get-up", about: "After a crash, G or a pad's Back puts you straight back on the bike. Not in network races." },
    { key: "air_steer", flag: "air-steer", label: "Air steering", about: "How much steering in the air bends your flight. 0% is the original game.", range: [0, 200, 10, "%"], off: 0 },
    { key: "air_brake", flag: "air-brake", label: "Air braking", about: "How much braking in the air shortens your flight. 0% is the original game.", range: [0, 200, 10, "%"], off: 0 },
    { key: "clean_starts", flag: "clean-starts", label: "Cleaner starts", about: "The computer riders keep their order across the gate off the start, so fewer pile into each other." },
    { key: "forgiving_landings", flag: "forgiving-landings", label: "Forgiving landings", about: "Fewer crashes from landing a little crooked. The original game is strict." },
    { key: "one_lap", flag: "one-lap", label: "One-lap races", about: "National and Supercross races can be 1 lap long (the original starts at 2)." },
  ]],
  ["Race display and camera", [
    { key: "sharp_display", flag: "modern-hud", label: "Modernised HUD", about: "Crisp see-through race panel, radar and speedometer at your screen's resolution." },
    { key: "kmh", flag: "kmh", label: "Speedometer in km/h", about: "The speed in kilometres an hour instead of miles an hour." },
    { key: "lap_gap", flag: "lap-gap", label: "Live gap to your best lap", about: "Under the race panel: how far ahead or behind your best lap you are." },
    { key: "centred_display", flag: "centred-display", label: "Race display nearer the middle", about: "On screens wider than 16:9, the race display where it sits on a 16:9 screen." },
    { key: "steady_camera", flag: "steady-camera", label: "Steadier camera", about: "The chase camera rises and dips with the bike more gently." },
    { key: "fov", flag: "fov", label: "Field of view", about: "How wide the riding cameras see. 85° is the original game's.", range: [60, 120, 1, "°"], off: 85 },
  ]],
];

export function defaults() {
  const s = {
    enhanced: false,
    quality: "high",
    time: "",
    weather: "clear",
    detail: 0.5,
    upgrades: Object.fromEntries(UPGRADES.map(([n, , on]) => [n, on])),
    show_fps: false,
  };
  for (const [, list] of EXTRAS) for (const e of list) s[e.key] = e.range ? e.off : false;
  return s;
}

const KEY = "mm2settings";
export function load() {
  const s = defaults();
  try {
    const kept = JSON.parse(localStorage.getItem(KEY) || "null");
    if (kept) {
      Object.assign(s, kept, { upgrades: { ...s.upgrades, ...(kept.upgrades || {}) } });
    } else if (localStorage.getItem("mm2look") === "enhanced") {
      s.enhanced = true; // the look picked before the settings came here
    }
  } catch {}
  return s;
}

export function save(s) {
  try {
    localStorage.setItem(KEY, JSON.stringify(s));
  } catch {}
}

/// Whether every extra is off, as the original game played.
export function isOriginal(s) {
  return EXTRAS.every(([, list]) => list.every((e) => s[e.key] === (e.range ? e.off : false)));
}

export function original(s) {
  for (const [, list] of EXTRAS) for (const e of list) s[e.key] = e.range ? e.off : false;
}

/// The game's options for `s`, each as the game takes it less its "--"
/// (Settings::args).
export function args(s) {
  const a = [];
  if (s.enhanced) {
    const list = [`quality=${s.quality}`];
    for (const [n] of UPGRADES) {
      if (!!s.upgrades[n] !== presetHas(s.quality, n)) list.push(s.upgrades[n] ? n : `no-${n}`);
    }
    if (s.time) list.push(`time=${s.time}`);
    if (s.weather !== "clear") list.push(`weather=${s.weather}`);
    if (Math.abs(s.detail - 0.5) > 1e-3) list.push(`detail=${Number(s.detail).toFixed(2)}`);
    a.push(`enhanced=${list.join(",")}`);
  }
  if (s.show_fps) a.push("show-fps");
  for (const [, list] of EXTRAS) {
    for (const e of list) {
      if (e.range) {
        if (s[e.key] !== e.off) a.push(`${e.flag}=${s[e.key]}`);
      } else if (s[e.key]) a.push(e.flag);
    }
  }
  return a;
}

// ---- The page's controls ----

function el(tag, props = {}, ...kids) {
  const e = Object.assign(document.createElement(tag), props);
  e.append(...kids);
  return e;
}

function checkbox(label, about, checked, change) {
  const box = el("input", { type: "checkbox", checked });
  box.addEventListener("change", () => change(box.checked));
  return el("label", { className: "check", title: about || "" }, box, " " + label);
}

function select(choices, value, change) {
  const s = el("select");
  for (const [v, text] of choices) s.append(el("option", { value: v, textContent: text, selected: v === value }));
  s.addEventListener("change", () => change(s.value));
  return s;
}

function slider(min, max, step, unit, value, change) {
  const out = el("span", { className: "value", textContent: value + unit });
  const r = el("input", { type: "range", min, max, step, value });
  r.addEventListener("input", () => {
    out.textContent = r.value + unit;
    change(Number(r.value));
  });
  return [r, out];
}

/// Fills `lookBox` with the look's controls and `box` with the rest;
/// `changed` hears each change.
export function render(lookBox, box, s, changed) {
  const redo = () => render(lookBox, box, s, changed);
  const set = (k, v, again) => {
    s[k] = v;
    save(s);
    changed(s);
    if (again) redo();
  };
  box.replaceChildren();
  lookBox.replaceChildren();

  // The look.
  const look = el("fieldset", {}, el("legend", { textContent: "Look" }));
  for (const [v, b, text] of [
    [false, "Original", ": the game as it looked in 2000"],
    [true, "Enhanced", ": modern lighting, shadows and materials"],
  ]) {
    const r = el("input", { type: "radio", name: "look", checked: s.enhanced === v });
    r.addEventListener("change", () => set("enhanced", v, true));
    look.append(el("label", {}, r, " ", el("b", { textContent: b }), text));
  }
  look.append(el("p", { className: "hint", textContent: "While racing, the button at the top left (or F3) switches between the two. The riding is the same either way." }));
  if (s.enhanced) {
    const grid = el("div", { className: "grid" });
    grid.append(
      el("span", { textContent: "Quality" }),
      select(QUALITIES.map((q) => [q, q[0].toUpperCase() + q.slice(1)]), s.quality, (q) => {
        s.quality = q;
        for (const [n] of UPGRADES) s.upgrades[n] = presetHas(q, n);
        set("quality", q, true);
      }),
      el("span", { textContent: "Time of day" }),
      select(TIMES, s.time, (v) => set("time", v)),
      el("span", { textContent: "Weather" }),
      select(WEATHERS, s.weather, (v) => set("weather", v)),
      el("span", { textContent: "Ground detail" }),
      el("span", {}, ...slider(0, 1, 0.05, "", s.detail, (v) => set("detail", v))),
    );
    const effects = el("details", {}, el("summary", { textContent: "Individual effects" }));
    const list = el("div", { className: "two" });
    for (const [n, text] of UPGRADES) {
      list.append(checkbox(text, "", !!s.upgrades[n], (on) => {
        s.upgrades[n] = on;
        set("upgrades", s.upgrades);
      }));
    }
    effects.append(list);
    look.append(grid, effects);
  }
  lookBox.append(look);

  // The extras.
  const extras = el("fieldset", {}, el("legend", { textContent: "Extras" }));
  extras.append(el("p", { className: "hint", textContent: "Things the original game didn't have. They all start off, so the game plays as it did. Most of them are also in the game: Options, then Tweaks... at the bottom." }));
  const back = el("button", { className: "small", textContent: "Back to the original", disabled: isOriginal(s), title: "Turns every extra here off." });
  back.addEventListener("click", () => {
    original(s);
    set("show_fps", s.show_fps, true);
  });
  extras.append(back);
  for (const [group, list] of EXTRAS) {
    extras.append(el("h3", { textContent: group }));
    for (const e of list) {
      if (e.range) {
        const [r, out] = slider(e.range[0], e.range[1], e.range[2], e.range[3], s[e.key], (v) => {
          s[e.key] = v;
          save(s);
          changed(s);
          back.disabled = isOriginal(s);
        });
        extras.append(el("label", { className: "range", title: e.about }, e.label + " ", r, out));
      } else {
        extras.append(checkbox(e.label, e.about, !!s[e.key], (on) => set(e.key, on, true)));
      }
    }
  }
  extras.append(el("h3", { textContent: "Screen" }));
  extras.append(checkbox("Show frame rate", "The frames a second at the bottom left (F2 too).", s.show_fps, (on) => set("show_fps", on)));
  extras.append(el("p", { className: "hint", textContent: "Thanks to Daniel (github.com/dwolp) for most of these, and for the fixes that came with them." }));
  box.append(extras);
}
