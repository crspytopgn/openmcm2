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
  ["grass", "3D grass", true, "Grass growing on the grassy ground near you, swaying in the wind and pushed aside as bikes ride through it. With Enhanced+ it grows thicker and taller and the wind rolls across it in waves."],
  ["smooth", "Smooth hills", true, "The hills round you smoothed, not faceted."],
  ["dust", "Dust clouds", true, "Soft clouds of dust the colour of the ground behind the bikes: thicker when you pull hard or spin, a ring of it when you land, and on a dry track a thin haze that hangs on the wind. Fewer, bigger puffs on Low and Medium, so it stays light."],
  ["mud", "Mud and spray", true, "Mud and spray caking bikes and riders."],
  ["bikes", "Glossier bikes", false, "Shiny paint and metal, and smoother riders."],
  ["shafts", "Light shafts", true, "Beams of sunlight past hills, trees and riders. Not in a browser: too heavy."],
  ["cinematic", "Replay blur and depth of field", false, "Replays filmed as through a lens. Not in a browser: too heavy."],
  ["ruts", "Tyre marks and ruts", true, "Where the bikes ride, tyre marks press into the dirt and build up lap by lap: a line ridden once is a faint mark, the racing line sinks into dark, damp ruts with loose dirt pushed up beside them, and the grass wears away. Only drawn: the ground you ride is the original's, so the riding doesn't change. Each player's game draws its own, in races, online and in replays."],
  ["scenery", "Trackside props and crowd", false, "Props along the course, a paddock and a crowd."],
  ["grade", "Colour grade", false, "A bolder look: colour, contrast, a vignette."],
  ["gtao", "Better ambient occlusion (GTAO)", true, "A truer ambient occlusion: deeper in creases and corners. Not in a browser: too heavy."],
  ["contact", "Contact shadows", true, "A thin, soft shadow where tyres, boots and props touch the ground."],
  ["ssr", "Reflections in puddles", true, "In the rain, puddles mirror the bikes and trees on screen. Not in a browser: too heavy."],
  ["skylight", "Sky light", true, "The sky's light worked out when a track loads: dimmer in dips and under banks."],
  ["pbr", "Realistic materials and reflections", true, "Materials lit as real ones are: metal, rough and smooth surfaces, fine bumps and dents, and the sky reflected in them. The free art uses it fully; the original's models just take the sky's light gently."],
  ["filmic", "Film-like colour (free graphics)", true, "With the free graphics: bright skies and sunlit metal roll off softly to white, as on film. The original game's art keeps its own colours."],
  ["air", "Sunlit haze", true, "Distant haze glows toward the sun and thins higher up, so far hilltops stand clearer. Open-air tracks only."],
  ["nightlights", "Stadium lights at night", true, "With the time of day set to Night, a stadium is lit by its own lamp banks: the floor, bikes and riders lit from above, with soft shadows from each bank."],
  ["roost", "Flying dirt (roost)", true, "Real roost: while the back tyre spins (or you pull hard, or slide) it throws a low jet of fine grit, crumbs and clods with a brown plume of dirt behind, all falling fast. Only on dirt; mud in the rain. Fewer pieces on Low."],
  ["bounce", "Bounce light", true, "Sunlight bouncing off the ground and walls: sunlit dirt throws a soft warm light into the shaded side of the bike and rider, and pale corners don't go as dark. Subtle. Costs a little on a slower computer."],
  ["closeup", "Sharp dirt up close", true, "Near your wheels the ground is real photo dirt, with clods, stones and grain, instead of the painted texture smeared up close. Needs Detailed ground on. Costs next to nothing."],
  ["wind", "Wind in the grass and trees", true, "A breezy day: gusts roll across the 3D grass in waves and the trees sway a little more. Costs next to nothing."],
  ["litair", "Sunlight in the air", true, "Sunlight in the air near the ground, with darker streaks where trees and hills shade it, so beams of light show. Faint, plain on Enhanced+. Not in a browser except with Enhanced+: too heavy."],
  ["fsr", "AMD FSR upscaling", true, "AMD FidelityFX Super Resolution: on a big screen the 3D scene is drawn smaller to stay smooth; FSR stretches it back up along its edges and gently sharpens it, so it looks close to full sharpness instead of soft. With TAA it rebuilds the picture from several frames, as FSR 2 does. A big help on laptops."],
  ["grime", "Dirt builds up on bikes and riders", true, "Dirt builds up very slowly on the bikes and riders through a race, the colour of the track you ride: a light dust on the wheels and lower bike first, creeping up the boots and legs lap by lap. Riding in another rider's roost spatters your front, a crash scuffs dirt up your side, and wet tracks cake on mud much faster. Takes the place of the quicker mud caking. Looks only, light on frame rate."],
];

// In a browser, Ultra draws as High does (the game caps both: see
// graphics::Enhanced::for_browser), so it isn't offered. Medium is the
// default: High has stalled a whole Mac in Safari.
// Enhanced+ is offered: the game keeps its look but holds the frame rate
// by setting effects aside when frames come late.
// Enhanced+ is a look of its own (beside Original and Enhanced), so the
// quality row offers the rest.
const QUALITIES = [["auto", "Auto (picks for your computer)"], ["low", "Low"], ["medium", "Medium"], ["high", "High (a fast computer)"]];
// The effects under Advanced, in groups, by their names in UPGRADES (the
// launcher's EFFECT_GROUPS). The colour grade is the colour look's.
const GROUPS = [
  ["Light and shadow", ["lighting", "shadows", "contact", "ao", "skylight", "bounce", "pbr", "shafts", "litair", "air", "bloom", "ssr", "nightlights", "filmic"]],
  ["Ground and grass", ["ground", "closeup", "smooth", "grass", "wind", "ruts", "distance"]],
  ["Dirt and mud", ["dust", "roost", "mud"]],
  ["Bikes and scenery", ["bikes", "scenery", "cinematic"]],
  ["Speed and sharpness", ["fsr"]],
];
// Pairs that do one job, shown as one choice of three: off, the first
// alone, or both (the launcher's PAIRS).
const PAIRS = {
  ao: ["gtao", "Ambient occlusion", ["Off", "Standard", "Better (GTAO)"], "Corners and contacts darkened, as ambient light is. Better: a truer ambient occlusion, deeper in creases and corners. Not in a browser: too heavy, so Better draws as Standard."],
  mud: ["grime", "Mud on bikes and riders", ["Off", "Quick caking", "Builds up slowly"], "Mud and spray on the bikes and riders. Quick caking: mud cakes on in a few minutes, as before. Builds up slowly: dirt the colour of the track creeps up the bike and rider lap by lap, roost spatters your front and a crash scuffs your side."],
};
// Whether a quality preset keeps an upgrade on (Quality::has).
function presetHas(quality, name) {
  if (quality === "auto") quality = "low"; // browsers seldom name the graphics card
  const off = { low: ["ao", "shafts", "smooth", "cinematic", "grass", "gtao", "contact", "ssr", "bounce", "litair"], medium: ["shafts", "smooth", "gtao", "ssr", "bounce", "litair"] }[quality] || [];
  if (quality === "nextgen" && ["bounce", "bloom", "bikes", "scenery"].includes(name)) return true;
  return !off.includes(name) && UPGRADES.some(([n, , on]) => n === name && on);
}

const ABOUT_QUALITY = "A preset for the effects below. Auto looks at your graphics card when the game starts and picks Low, Medium or High for it; most browsers don't say which card they have, and then it starts with about Low's effects and turns Medium's on one step at a time while there is time to spare, remembering where it got to for next time. While you race it draws the 3D scene a little smaller if frames start coming late, and if that isn't enough it turns the costliest effects off, one step at a time, giving them back once there is time to spare. It remembers what this computer needed for next time. Medium suits most computers in a browser; High needs a fast one. Changing an effect below picks Medium with that change.";
const ABOUT_TIME = "Where the sun is: the track's own, or an hour of your choice.";
const ABOUT_WEATHER = "Clear, overcast or rain, for looks only: the riding is the same.";
const ABOUT_DETAIL = "How strongly the grain of the photo ground shows.";
const TIMES = [["", "Track's own"], ["dawn", "Dawn"], ["morning", "Morning"], ["noon", "Noon"], ["afternoon", "Afternoon"], ["dusk", "Dusk"]];
const WEATHERS = [["clear", "Clear"], ["overcast", "Overcast"], ["rain", "Rain"]];
// The colour looks (mm2_settings::LOOKS), a gentle grade over the picture.
const LOOKS = [["original", "Original"], ["bold", "Bold"], ["warm", "Warm afternoon"], ["overcast", "Overcast"]];
const ABOUT_LOOK = "A gentle colour grade over the finished picture. Original: the colours as drawn. Bold: richer colour, a little more contrast, slightly darker corners. Warm afternoon: sunlit parts a touch golden, the shade lifted a little. Overcast: a little less colour and contrast, a cool tint in the shade.";
const ABOUT_NEXTGEN = "Enhanced+: Enhanced turned up, bolder than the rest. Sunlit air you can see shafts of light in, thick grass blowing in waves of wind, trees plainly moving, photo dirt sharp right up to your wheels, the real sky with clouds, bounce light, a soft glow round bright light, glossier bikes and trackside props. Heavy: for a strong computer. If frames come late it draws softer and sets effects aside.";

const ABOUT_INTRO = "Plays a video of your own in place of the original intro movie when the game starts, for example a remastered copy of the intro that you have. Pick an MP4 or WebM file your browser can play. It is kept in this browser only and is never uploaded or shared. A click or a key skips it, as with the original, and the game's own Intro Movie option still turns the intro off. If it can't be played, the original intro plays instead (or none with the free version). Off unless you pick one.";

// The extras the original game didn't have, all off unless turned on,
// by group: the setting, its flag, the label and what it does. A number
// has its range, step, unit and the original game's value.
export const EXTRAS = [
  ["Riding", [
    { key: "quick_get_up", flag: "quick-get-up", label: "Quick get-up", about: "After a crash, G or a pad's Back puts you straight back on the bike. Not in network races. Laps don't go on the leaderboards while it's on." },
    { key: "air_steer", flag: "air-steer", label: "Air steering", about: "How much steering in the air bends your flight. 0% is the original game. Laps go on the leaderboards only at 0%.", range: [0, 200, 10, "%"], off: 0 },
    { key: "air_brake", flag: "air-brake", label: "Air braking", about: "How much braking in the air shortens your flight. 0% is the original game. Laps go on the leaderboards only at 0%.", range: [0, 200, 10, "%"], off: 0 },
    { key: "clean_starts", flag: "clean-starts", label: "Cleaner starts", about: "The computer riders keep their order across the gate off the start, so fewer pile into each other." },
    { key: "forgiving_landings", flag: "forgiving-landings", label: "Forgiving landings", about: "Fewer crashes from landing a little crooked. The original game is strict. Laps don't go on the leaderboards while it's on." },
    { key: "one_lap", flag: "one-lap", label: "One-lap races", about: "National and Supercross races can be 1 lap long (the original starts at 2). A 1-lap race sends no leaderboard lap, as laps count from lap 2." },
  ]],
  ["Race display and camera", [
    { key: "sharp_display", flag: "modern-hud", label: "Modernised HUD", about: "Crisp see-through race panel, radar and speedometer at your screen's resolution." },
    { key: "kmh", flag: "kmh", label: "Speedometer in km/h", about: "The speed in kilometres an hour instead of miles an hour." },
    { key: "lap_gap", flag: "lap-gap", label: "Live gap to your best lap", about: "Under the race panel: how far ahead or behind your best lap you are." },
    { key: "centred_display", flag: "centred-display", label: "Race display nearer the middle", about: "On screens wider than 16:9, the race display where it sits on a 16:9 screen." },
    { key: "steady_camera", flag: "steady-camera", label: "Steadier camera", about: "The chase camera rises and dips with the bike more gently." },
    { key: "fov", flag: "fov", label: "Field of view", about: "How wide the riding cameras see. 85° is the original game's.", range: [60, 120, 1, "°"], off: 85 },
  ]],
  ["Controllers, sound and picture", [
    { key: "pad_dead_zone", flag: "pad-dead-zone", label: "Stick dead zone", about: "How far a stick moves before it steers. 4% is the original game's.", range: [0, 50, 1, "%"], off: 4 },
    { key: "pad_sensitivity", flag: "pad-sensitivity", label: "Stick sensitivity", about: "How far a stick steers for how far it is pushed. 100% is the original game's.", range: [25, 200, 5, "%"], off: 100 },
    { key: "pad_curve", flag: "pad-curve", label: "Stick steering curve", about: "How gently a stick steers near the middle. 100% is the original game's: little steering for a small push and a lot near the edge. 0% steers in straight proportion.", range: [0, 100, 5, "%"], off: 100 },
    { key: "pad_smoothing", flag: "pad-smoothing", label: "Stick smoothing", about: "Evens out a twitchy stick's steering. 0% is the original game's.", range: [0, 90, 5, "%"], off: 0 },
    { key: "photo_mode", flag: "photo-mode", label: "Photo mode", about: "In a race, press F (or click both sticks on a pad) to freeze it and hide the race display. Fly the camera with WASD, Q/E for down and up and Shift for faster, look round with the arrow keys or by holding the right mouse button (on a pad, the left stick flies, the right stick looks, the triggers go down and up). Enter or the pad's A saves a picture as a PNG to your downloads. F again, Escape or the pad's B carries on the race. Not in network races, where the race can't wait." },
    { key: "replay_editor", flag: "replay-editor", label: "Replay editor", about: "In a replay, extra keys on top of the original VCR player. Tab goes from the race camera to a free camera you fly (WASD, Q/E down and up, Shift faster, arrow keys or the right mouse button to look) and on to a camera path. Comma and full stop slow the replay down to 1/2, 1/4, 1/8 or 1/20 speed and back. I and O mark the start and end of a moment, and L loops it. K keeps the camera where it is as a camera key at that time; with two or more keys, the camera path glides between them as the replay plays. Backspace takes away the nearest key, T keeps the free camera aimed at the rider. R records a clip as a movie (30 frames a second, no sound) to your downloads (a WebM or MP4 file, depending on the browser): from In to Out if both are marked, otherwise until R again. H hides the player and the editor's line for a clean picture. On a pad: X changes the camera, Y adds a key, the D-pad's left and right change the speed, A pauses, Back records, and the sticks and triggers fly the free camera. Off, replays are exactly the original's." },
    { key: "fresh_sounds", flag: "fresh-sounds", label: "Freshened engine sounds", about: "Ticked, the bike engines from your own MM2 or demo files get a little more life as they load: a fuller tone, a touch more bite and loops with no clicks. Nothing is downloaded; the Free Pack keeps its own recordings. Unticked (the default), the engines sound exactly as in the original." },
    { key: "no_whistle", flag: "no-whistle", label: "No out-of-bounds whistle", keep: true, about: "The free tracks play a cartoon slide whistle as you ride off the edge of the world and get thrown back in, going up, then down as you fall, unless you tick this. Ticked, that throw is silent as in the original." },
    { key: "intro_video", flag: "intro-video", label: "Your own intro video", file: true, about: ABOUT_INTRO },
    { key: "fxaa", flag: "fxaa", label: "Smoother edges", about: "Smooths stair-stepped edges after the race is drawn, in either look: the outlines of banners, bushes, tree leaves and fences, which Anti-aliasing (MSAA) does not reach. The change is small, so look at a banner edge with it off and on." },
    { key: "sharpen", flag: "sharpen", label: "Sharpening", about: "Makes the race picture crisper, in either look: the grain in the dirt and grass, banners and bikes stand out more. 0% is the original game's picture; 30 to 50% is a gentle lift, 100% is strong.", range: [0, 100, 5, "%"], off: 0 },
  ]],
  ["Tracks, bikes and riders", [
    { key: "all_gear", flag: "all-gear", label: "All gear in every event", about: "The bikes and outfits the Pro Circuit gives out, in single races too." },
    { key: "track_filter", flag: "track-filter", label: "Track filter", about: "All tracks, the base game's or only the ones you added, on the event page." },
    { key: "drive_vehicles", flag: "drive-vehicles", label: "Drive the scenery", about: "In Practice and free roam, press V (or the pad's D-pad down) next to one of a track's cars, trucks or trains to get in and drive it, or press V anywhere to jump into the nearest plane, helicopter or blimp in the sky and fly it. Each one has its own springs, weight, power and grip. Each one sounds like what it is, and your bike goes quiet meanwhile. V again puts you back on your bike beside it. The Enduro tracks (Airfield, Farm, Open Pit, Ski Lodge, Trailer) have the most, and the free tracks get a car park by the start and planes, a helicopter, a blimp and a UFO out on open ground." },
    { key: "free_tracks", flag: "free-tracks", label: "Our own free tracks", about: "The tracks we made for the Free Pack, in the single race and multiplayer track lists. Off, they only show for an event that has no other track, so add player-made tracks (the starter set under Your own tracks) to ride those instead. The Pro Circuit with the Free Pack and races hosted on one of them work either way." },
    { key: "demo_tracks", flag: "demo-tracks", label: "Rebuilt demo tracks", about: "Only with the free MM2 demo. The demo has the full ground for just two tracks; for the others it keeps the route, the scenery and the menu pictures. This adds those 27 tracks to the event lists, rebuilt: the original's route and scenery on new ground with jumps of our own, so the hills and jumps are not the original's. Each one is built the first time you ride it, which takes a few seconds (longer in a browser). Their names end in (Rebuilt). The Stunt Quarry and Tag arenas have no route in the demo and aren't included." },
    { key: "riders", flag: "gate-riders", label: "Riders on the gate", about: "Up to 30 riders in a single race. 0 is the original game's count.", range: [0, 30, 1, ""], off: 0 },
    { key: "no_leaderboards", flag: "no-leaderboards", label: "Keep my laps off the leaderboards", keep: true, about: "The lap leaderboards are on unless you tick this: after a race, your best laps go to the OpenMCM2 leaderboard under your rider name (High Scores, Online shows everyone's), each with its ghost (where the bike was, a few times a second) so the fastest lap on each track can be raced." },
    { key: "online_ghosts", flag: "online-ghosts", label: "Race the leaderboard ghosts", about: "Off unless you turn it on. The server keeps the ghost of the fastest leaderboard lap on each track and course. In the game, Replays, Saved Ghost Race Files lists the online ghosts for your tracks under your own: pick one and Start to race it as a ghost race." },
    { key: "no_builder_menu", flag: "no-builder-menu", label: "No Course Builder on the main menu", keep: true, about: "The main menu has a shovel link on its chain, above Help, that opens the Course Builder, unless you tick this. Ticked, the main menu is exactly the original's, and the builder is still under Options, Tweaks." },
    { key: "no_share_playtime", flag: "no-share-playtime", label: "Don't add my time to everyone's", keep: true, about: "Unless you tick this, how long you play each event, track and bike class is added to everyone's on the OpenMCM2 server, with no name or anything else that says who you are." },
  ]],
  ["Network races", [
    { key: "no_lobby_downloads", flag: "no-lobby-downloads", label: "No lobby downloads", keep: true, about: "Lobby downloads are on unless you tick this: in a network race, the host's track and other riders' own paints and outfits come over if you don't have them, and yours go to them." },
    { key: "no_list_races", flag: "no-list-races", label: "Keep my races out of the server browser", keep: true, about: "Unless you tick this, a race you host shows in everyone's Host or Join list (Multiplayer) with its name, track and riders, so players anywhere can join with a click. Ticked, only people you send the race code or invite link to can join." },
  ]],
];


/// Where the own intro video is kept (the page hands over web/mm2fs.js's
/// keepIntro and forgetIntro).
let introStore = null;
export function setIntroStore(store) {
  introStore = store;
}

/// Phones and tablets: `touch` shows the light mode's switch, and `phone`
/// has it on unless turned off (the page sets these from web/touch.js).
let device = { touch: false, phone: false };
export function setDevice(d) {
  device = d;
}
/// The light mode for phones: the picture drawn at most about 720p's worth
/// of pixels (the browser stretches it over the screen) and, in the
/// enhanced look under Auto, the Low effects.
export const LIGHT_PIXELS = 1280 * 720;
const ABOUT_LIGHT = "For phones and smaller tablets: the game draws its picture at about 720p and the screen stretches it, instead of the three million or more pixels of a phone's sharp screen, so it runs smoother, cooler and longer on a charge. In the enhanced look with Auto quality it also uses the Low effects. The riding is the same. On unless you untick it on a phone.";
export function light(s) {
  return s.light ?? device.phone;
}

/// With Auto: how many steps of the costliest effects the game found it
/// had to set aside on this computer (0 to 3), kept for the next start.
const SHED_KEY = "mm2shed";
export function learnedShed() {
  try {
    const n = parseInt(localStorage.getItem(SHED_KEY) || "0", 10);
    return n >= 0 && n <= 3 ? n : 0;
  } catch {
    return 0;
  }
}
/// Whether the game has said yet (a browser that names no graphics card
/// starts at about Low until it has, and works up from there).
export function shedKnown() {
  try {
    return localStorage.getItem(SHED_KEY) !== null;
  } catch {
    return false;
  }
}
export function learnShed(n) {
  try {
    localStorage.setItem(SHED_KEY, String(Math.max(0, Math.min(3, n | 0))));
  } catch {}
}

/// Picks a quality preset, its effects with it.
export function setQuality(s, q) {
  s.quality = q;
  for (const [n] of UPGRADES) s.upgrades[n] = presetHas(q, n);
}

/// The look: "original", "enhanced" or "nextgen".
export function lookOf(s) {
  return !s.enhanced ? "original" : s.quality === "nextgen" ? "nextgen" : "enhanced";
}

/// Picks a look (and saves it). Back from Enhanced+, Auto picks the
/// quality for the computer.
export function setLook(s, v) {
  const was = lookOf(s);
  if (v === "original") s.enhanced = false;
  else {
    s.enhanced = true;
    if (v === "nextgen") setQuality(s, "nextgen");
    else if (was === "nextgen") setQuality(s, "auto");
  }
  save(s);
}

/// What each look is, for its hover (the launcher's words too).
export const LOOK_ABOUT = {
  original: "As the game drew it in 2000, pixel for pixel. The lightest on your computer, and how it starts unless you pick another.",
  enhanced: "The same game with modern light: per-pixel lighting, real shadows, haze, dust, 3D grass and more, kept subtle so it still looks like MM2. Auto fits the effects to your computer; Settings has the quality and every effect. F3 (or the button at the top left) flips back to the original look while racing to compare. The riding is the same.",
  nextgen: ABOUT_NEXTGEN,
};

export function defaults() {
  const s = {
    enhanced: false,
    quality: "auto",
    time: "",
    weather: "clear",
    look: "original",
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
      // Before v3 the ruts were the old deep grooves, off; the tyre marks
      // that build up start on now.
      if ((kept.v || 0) < 3 && kept.upgrades) delete kept.upgrades.ruts;
      // Before v4 the dirt building up was off by default and the tyre
      // marks too faint to see: both start on now.
      if ((kept.v || 0) < 4 && kept.upgrades) {
        delete kept.upgrades.ruts;
        delete kept.upgrades.grime;
      }
      Object.assign(s, kept, { upgrades: { ...s.upgrades, ...(kept.upgrades || {}) } });
      // Before the colour looks, the grade switch's one bold look.
      if (!kept.look && kept.upgrades && kept.upgrades.grade) s.look = "bold";
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
  s.v = 4;
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
  const lite = light(s);
  if (lite) a.push(`max-pixels=${LIGHT_PIXELS}`);
  if (s.enhanced) {
    // The light mode: Auto is Low.
    const list = [`quality=${lite && s.quality === "auto" ? "low" : s.quality}`];
    // Auto picks the effects itself.
    // The colour grade is the colour look's (look= below).
    if (s.quality !== "auto") for (const [n] of UPGRADES) {
      if (n !== "grade" && !!s.upgrades[n] !== presetHas(s.quality, n)) list.push(s.upgrades[n] ? n : `no-${n}`);
    }
    // Auto: the effects this computer needed set aside last time
    // (the game tells the page, mm2page.autoShed), so it starts there.
    if (s.quality === "auto" && !lite) {
      const shed = learnedShed();
      if (shed > 0 || shedKnown()) list.push(`shed=${shed}`);
    }
    if (s.time) list.push(`time=${s.time}`);
    if (s.weather !== "clear") list.push(`weather=${s.weather}`);
    if (s.look && s.look !== "original") list.push(`look=${s.look}`);
    if (Math.abs(s.detail - 0.5) > 1e-3) list.push(`detail=${Number(s.detail).toFixed(2)}`);
    a.push(`enhanced=${list.join(",")}`);
  }
  if (s.show_fps) a.push("show-fps");
  for (const [, list] of EXTRAS) {
    for (const e of list) {
      if (e.range) {
        if (s[e.key] !== e.off) a.push(`${e.flag}=${s[e.key]}`);
      } else if (e.file) {
        // The page keeps the file itself; the game asks it to play.
        if (s[e.key]) a.push(`${e.flag}=browser`);
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

/// GROUPS with any effect not in one (a new switch) in the last, so
/// none goes missing.
function grouped() {
  const shown = new Set(["grade", ...GROUPS.flatMap(([, n]) => n), ...Object.values(PAIRS).map((p) => p[0])]);
  const rest = UPGRADES.map(([n]) => n).filter((n) => !shown.has(n));
  return GROUPS.map(([g, n], i) => [g, i === GROUPS.length - 1 ? [...n, ...rest] : n]);
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

  // The look: Original, Enhanced or Enhanced+.
  const look = el("fieldset", {}, el("legend", { textContent: "Look" }));
  const nextgen = s.enhanced && s.quality === "nextgen";
  const current = !s.enhanced ? "original" : nextgen ? "nextgen" : "enhanced";
  for (const [v, b, text, about] of [
    ["original", "Original", ": the game as it looked in 2000", LOOK_ABOUT.original],
    ["enhanced", "Enhanced", ": modern light, kept subtle", LOOK_ABOUT.enhanced],
    ["nextgen", "Enhanced+", ": Enhanced, turned up", ABOUT_NEXTGEN],
  ]) {
    const r = el("input", { type: "radio", name: "look", checked: current === v });
    r.addEventListener("change", () => {
      setLook(s, v);
      set("enhanced", s.enhanced, true);
    });
    look.append(el("label", { title: about }, r, " ", el("b", { textContent: b }), text));
  }
  if (device.touch) {
    look.append(checkbox("Light mode for phones", ABOUT_LIGHT, light(s), (on) => set("light", on, true)));
  }
  look.append(el("p", { className: "hint", textContent: "While racing, the button at the top left (or F3) switches to the original look. The riding is the same with every look." }));
  if (s.enhanced) {
    const grid = el("div", { className: "grid" });
    if (!nextgen) {
      grid.append(
        el("span", { textContent: "Quality", title: ABOUT_QUALITY }),
        select(QUALITIES, s.quality, (q) => {
          setQuality(s, q);
          set("quality", q, true);
        }, ABOUT_QUALITY),
      );
    }
    grid.append(
      el("span", { textContent: "Colour look", title: ABOUT_LOOK }),
      select(LOOKS, s.look || "original", (v) => set("look", v), ABOUT_LOOK),
    );
    look.append(grid);
    if (nextgen) look.append(el("p", { className: "hint", textContent: "Enhanced+ picks its own quality: it draws a little softer and sets effects aside when frames come late." }));

    // Everything else, closed unless opened.
    const adv = el("details", {}, el("summary", { textContent: "Advanced", title: "Time of day, weather, ground detail and each effect on its own. The quality sets them all; point at one to see what it does." }));
    const more = el("div", { className: "grid" });
    more.append(
      el("span", { textContent: "Time of day", title: ABOUT_TIME }),
      select(TIMES, s.time, (v) => set("time", v), ABOUT_TIME),
      el("span", { textContent: "Weather", title: ABOUT_WEATHER }),
      select(WEATHERS, s.weather, (v) => set("weather", v), ABOUT_WEATHER),
      el("span", { textContent: "Ground detail", title: ABOUT_DETAIL }),
      el("span", { title: ABOUT_DETAIL }, ...slider(0, 1, 0.05, "", s.detail, (v) => set("detail", v), ABOUT_DETAIL)),
    );
    adv.append(more);
    // Under Auto, changing an effect means choosing yourself.
    const changeEffects = (f) => {
      f();
      const wasAuto = s.quality === "auto";
      if (wasAuto) s.quality = "medium";
      set("upgrades", s.upgrades, true);
    };
    const about = Object.fromEntries(UPGRADES.map(([n, text, , a]) => [n, [text, a]]));
    for (const [group, names] of grouped()) {
      adv.append(el("h4", { textContent: group }));
      const list = el("div", { className: "two" });
      for (const n of names) {
        const pair = PAIRS[n];
        if (pair) {
          const [second, label, words, hover] = pair;
          const level = !s.upgrades[n] ? "0" : s.upgrades[second] ? "2" : "1";
          list.append(el("label", { className: "check", title: hover }, label + " ", select(words.map((w, i) => [String(i), w]), level, (v) => changeEffects(() => {
            s.upgrades[n] = v !== "0";
            s.upgrades[second] = v === "2";
          }), hover)));
        } else {
          const [text, a] = about[n];
          list.append(checkbox(text, a, !!s.upgrades[n], (on) => changeEffects(() => (s.upgrades[n] = on))));
        }
      }
      adv.append(list);
    }
    look.append(adv);
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
      } else if (e.file) {
        extras.append(fileRow(e, s, set));
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

/// A file of the player's own (the intro video): its name, Choose and
/// Remove. The setting holds the file's name, the page's store the file.
function fileRow(e, s, set) {
  const name = el("span", { className: "hint", textContent: s[e.key] ? s[e.key] : "None: the original intro plays." });
  const choose = el("button", { className: "small", textContent: "Choose video…", title: e.about });
  const remove = el("button", { className: "small", textContent: "Remove", hidden: !s[e.key], title: "Back to the original intro movie, and the video taken out of this browser." });
  choose.addEventListener("click", () => {
    const input = el("input", { type: "file", accept: "video/mp4,video/webm,.mp4,.m4v,.webm" });
    input.addEventListener("change", async () => {
      const f = input.files[0];
      if (!f || !introStore) return;
      name.textContent = "Keeping it in this browser…";
      try {
        await introStore.keep(f);
        set(e.key, f.name, true);
      } catch (err) {
        console.warn("intro video:", err);
        name.textContent = "Couldn't keep it in this browser (not enough space?).";
      }
    });
    input.click();
  });
  remove.addEventListener("click", async () => {
    try {
      await introStore?.forget();
    } catch {}
    set(e.key, false, true);
  });
  return el("div", { className: "check", title: e.about }, e.label + " ", choose, " ", remove, " ", name);
}
