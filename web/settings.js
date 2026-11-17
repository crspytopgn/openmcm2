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
  ["lighting", "Per-pixel lighting", true, "Light worked out for every pixel, with bumps and highlights."],
  ["shadows", "Sun shadows", true, "Every object casts a shadow from the sun."],
  ["distance", "Far draw distance and haze", true, "The whole track drawn, fading into haze."],
  ["bloom", "HDR bloom", false, "Bright lights and the sun glow softly."],
  ["ao", "Ambient occlusion", true, "Corners and contacts darkened, as ambient light is."],
  ["ground", "Detailed ground", true, "The ground re-made from photo materials."],
  ["grass", "3D grass", true, "Grass growing on the grassy ground near you."],
  ["smooth", "Smooth hills", true, "The hills round you smoothed, not faceted."],
  ["dust", "Dust clouds", true, "Clouds of dust behind the bikes, casting shadows."],
  ["mud", "Mud and spray", true, "Mud and spray caking bikes and riders."],
  ["bikes", "Glossier bikes", false, "Shiny paint and metal, and smoother riders."],
  ["shafts", "Light shafts", true, "Beams of sunlight past hills, trees and riders. Not in a browser: too heavy."],
  ["cinematic", "Replay blur and depth of field", false, "Replays filmed as through a lens. Not in a browser: too heavy."],
  ["ruts", "Lasting tyre ruts", false, "Tyre ruts that stay for the race."],
  ["scenery", "Trackside props and crowd", false, "Props along the course, a paddock and a crowd."],
  ["grade", "Colour grade", false, "A bolder look: colour, contrast, a vignette."],
  ["gtao", "Better ambient occlusion (GTAO)", true, "A truer ambient occlusion: deeper in creases and corners. Not in a browser: too heavy."],
  ["contact", "Contact shadows", true, "A thin, soft shadow where tyres, boots and props touch the ground."],
  ["ssr", "Reflections in puddles", true, "In the rain, puddles mirror the bikes and trees on screen. Not in a browser: too heavy."],
  ["skylight", "Sky light", true, "The sky's light worked out when a track loads: dimmer in dips and under banks."],
];

// In a browser, Ultra draws as High does (the game caps both: see
// graphics::Enhanced::for_browser), so it isn't offered. Medium is the
// default: High has stalled a whole Mac in Safari.
const QUALITIES = [["auto", "Auto (picks for your computer)"], ["low", "Low"], ["medium", "Medium"], ["high", "High (a fast computer)"]];
// Whether a quality preset keeps an upgrade on (Quality::has).
function presetHas(quality, name) {
  if (quality === "auto") quality = "medium"; // until the game knows the graphics card
  const off = { low: ["ao", "shafts", "smooth", "cinematic", "grass", "gtao", "contact", "ssr"], medium: ["shafts", "smooth", "gtao", "ssr"] }[quality] || [];
  return !off.includes(name) && UPGRADES.some(([n, , on]) => n === name && on);
}

const ABOUT_QUALITY = "A preset for the effects below. Auto looks at your graphics card when the game starts and picks Low, Medium or High for it, and while you race it draws the 3D scene a little smaller if frames start coming late. Medium suits most computers in a browser; High needs a fast one. Changing an effect below picks Medium with that change.";
const ABOUT_TIME = "Where the sun is: the track's own, or an hour of your choice.";
const ABOUT_WEATHER = "Clear, overcast or rain, for looks only: the riding is the same.";
const ABOUT_DETAIL = "How strongly the grain of the photo ground shows.";
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
  ["Controllers and picture", [
    { key: "pad_dead_zone", flag: "pad-dead-zone", label: "Stick dead zone", about: "How far a stick moves before it steers. 4% is the original game's.", range: [0, 50, 1, "%"], off: 4 },
    { key: "pad_sensitivity", flag: "pad-sensitivity", label: "Stick sensitivity", about: "How far a stick steers for how far it is pushed. 100% is the original game's.", range: [25, 200, 5, "%"], off: 100 },
    { key: "pad_curve", flag: "pad-curve", label: "Stick steering curve", about: "How gently a stick steers near the middle. 100% is the original game's: little steering for a small push and a lot near the edge. 0% steers in straight proportion.", range: [0, 100, 5, "%"], off: 100 },
    { key: "pad_smoothing", flag: "pad-smoothing", label: "Stick smoothing", about: "Evens out a twitchy stick's steering. 0% is the original game's.", range: [0, 90, 5, "%"], off: 0 },
    { key: "fxaa", flag: "fxaa", label: "Smoother edges", about: "Smooths stair-stepped edges after the race is drawn, in either look: the outlines of banners, bushes, tree leaves and fences, which Anti-aliasing (MSAA) does not reach. The change is small, so look at a banner edge with it off and on." },
    { key: "sharpen", flag: "sharpen", label: "Sharpening", about: "Makes the race picture crisper, in either look: the grain in the dirt and grass, banners and bikes stand out more. 0% is the original game's picture; 30 to 50% is a gentle lift, 100% is strong.", range: [0, 100, 5, "%"], off: 0 },
  ]],
  ["Tracks, bikes and riders", [
    { key: "all_gear", flag: "all-gear", label: "All gear in every event", about: "The bikes and outfits the Pro Circuit gives out, in single races too." },
    { key: "track_filter", flag: "track-filter", label: "Track filter", about: "All tracks, the base game's or only the ones you added, on the event page." },
    { key: "riders", flag: "gate-riders", label: "Riders on the gate", about: "Up to 30 riders in a single race. 0 is the original game's count.", range: [0, 30, 1, ""], off: 0 },
    { key: "no_leaderboards", flag: "no-leaderboards", label: "Keep my laps off the leaderboards", keep: true, about: "The lap leaderboards are on unless you tick this: after a race, your best laps go to the OpenMCM2 leaderboard under your rider name (High Scores, Online shows everyone's)." },
    { key: "no_share_playtime", flag: "no-share-playtime", label: "Don't add my time to everyone's", keep: true, about: "Unless you tick this, how long you play each event, track and bike class is added to everyone's on the OpenMCM2 server, with no name or anything else that says who you are." },
  ]],
  ["Network races", [
    { key: "no_lobby_downloads", flag: "no-lobby-downloads", label: "No lobby downloads", keep: true, about: "Lobby downloads are on unless you tick this: in a network race, the host's track and other riders' own paints and outfits come over if you don't have them, and yours go to them." },
    { key: "no_list_races", flag: "no-list-races", label: "Keep my races out of the server browser", keep: true, about: "Unless you tick this, a race you host shows in everyone's Host or Join list (Multiplayer) with its name, track and riders, so players anywhere can join with a click. Ticked, only people you send the race code or invite link to can join." },
  ]],
];

/// Picks a quality preset, its effects with it.
export function setQuality(s, q) {
  s.quality = q;
  for (const [n] of UPGRADES) s.upgrades[n] = presetHas(q, n);
}

export function defaults() {
  const s = {
    enhanced: false,
    quality: "auto",
    time: "",
    weather: "clear",
    detail: 0.5,
    upgrades: Object.fromEntries(UPGRADES.map(([n]) => [n, presetHas("medium", n)])),
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
      // Settings from before Medium was the browser's default go to it.
      if (!kept.v && s.quality !== "low") {
        s.quality = "medium";
        for (const [n] of UPGRADES) s.upgrades[n] = presetHas("medium", n);
      }
    } else if (localStorage.getItem("mm2look") === "enhanced") {
      s.enhanced = true; // the look picked before the settings came here
    }
  } catch {}
  return s;
}

export function save(s) {
  s.v = 2;
  try {
    localStorage.setItem(KEY, JSON.stringify(s));
  } catch {}
}

/// Whether every extra is off, as the original game played.
export function isOriginal(s) {
  // The sharing and network choices aren't extras: "Back to the original" keeps them.
  return EXTRAS.every(([, list]) => list.every((e) => e.keep || s[e.key] === (e.range ? e.off : false)));
}

export function original(s) {
  for (const [, list] of EXTRAS) for (const e of list) if (!e.keep) s[e.key] = e.range ? e.off : false;
}

/// The game's options for `s`, each as the game takes it less its "--"
/// (Settings::args).
export function args(s) {
  const a = [];
  if (s.enhanced) {
    const list = [`quality=${s.quality}`];
    // Auto picks the effects itself.
    if (s.quality !== "auto") for (const [n] of UPGRADES) {
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

function select(choices, value, change, about = "") {
  const s = el("select", { title: about });
  for (const [v, text] of choices) s.append(el("option", { value: v, textContent: text, selected: v === value }));
  s.addEventListener("change", () => change(s.value));
  return s;
}

function slider(min, max, step, unit, value, change, about = "") {
  const out = el("span", { className: "value", textContent: value + unit });
  const r = el("input", { type: "range", min, max, step, value, title: about });
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
  for (const [v, b, text, about] of [
    [false, "Original", ": the game as it looked in 2000", "The game drawn as it was in 2000. The lightest on your computer."],
    [true, "Enhanced", ": modern lighting, shadows and materials", "Per-pixel light, real shadows, haze, dust and more, each of which you can turn on or off."],
  ]) {
    const r = el("input", { type: "radio", name: "look", checked: s.enhanced === v });
    r.addEventListener("change", () => set("enhanced", v, true));
    look.append(el("label", { title: about }, r, " ", el("b", { textContent: b }), text));
  }
  look.append(el("p", { className: "hint", textContent: "While racing, the button at the top left (or F3) switches between the two. The riding is the same either way." }));
  if (s.enhanced) {
    const grid = el("div", { className: "grid" });
    grid.append(
      el("span", { textContent: "Quality", title: ABOUT_QUALITY }),
      select(QUALITIES, s.quality, (q) => {
        s.quality = q;
        for (const [n] of UPGRADES) s.upgrades[n] = presetHas(q, n);
        set("quality", q, true);
      }, ABOUT_QUALITY),
      el("span", { textContent: "Time of day", title: ABOUT_TIME }),
      select(TIMES, s.time, (v) => set("time", v), ABOUT_TIME),
      el("span", { textContent: "Weather", title: ABOUT_WEATHER }),
      select(WEATHERS, s.weather, (v) => set("weather", v), ABOUT_WEATHER),
      el("span", { textContent: "Ground detail", title: ABOUT_DETAIL }),
      el("span", { title: ABOUT_DETAIL }, ...slider(0, 1, 0.05, "", s.detail, (v) => set("detail", v), ABOUT_DETAIL)),
    );
    const effects = el("details", {}, el("summary", { textContent: "Individual effects", title: "Each effect on its own. Point at one to see what it does." }));
    const list = el("div", { className: "two" });
    for (const [n, text, , about] of UPGRADES) {
      list.append(checkbox(text, about, !!s.upgrades[n], (on) => {
        s.upgrades[n] = on;
        // Under Auto, changing an effect means choosing yourself.
        const wasAuto = s.quality === "auto";
        if (wasAuto) s.quality = "medium";
        set("upgrades", s.upgrades, wasAuto);
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
        }, e.about);
        extras.append(el("label", { className: "range", title: e.about }, e.label + " ", r, out));
      } else {
        extras.append(checkbox(e.label, e.about, !!s[e.key], (on) => set(e.key, on, true)));
      }
    }
  }
  extras.append(el("h3", { textContent: "Screen" }));
  extras.append(checkbox("Show frame rate", "The frames a second at the bottom left (F2 too).", s.show_fps, (on) => set("show_fps", on)));
  extras.append(el("p", { className: "hint", textContent: "Thanks to Daniel (github.com/dwolp) for many of these, and for the fixes that came with them." }));
  box.append(extras);
}
