// Touch controls for phones and tablets, shown only while riding. Each
// one presses the game's own key (the factory keyboard map, UI/PRESET.CTL),
// so the game sees a keyboard, as the original's keys play. In the menus a
// tap is a click and a drag moves sliders and scroll bars (main.rs).
//
// The left thumb rides a stick that appears where it lands: left and right
// steer, up leans forward and down leans back, so steering and leaning go
// together in the air. It is a real analog stick to the game (a game pad
// with only a left stick, below), read as the original read a joystick,
// so a little push steers a little; where the game can't see it, it
// presses the arrow keys and W/S instead. The right thumb has the gas and the
// brake, and slides from one to the other without lifting. The stunts and
// the clutch sit above them, and the small buttons along the top are the
// camera, looking back, backing up and leaving the race (the game's Esc).
//
// They come up on a touch screen with no mouse or trackpad (a phone, or a
// tablet on its own). A screen that has both (a phone with a stylus, a
// tablet with a trackpad, a touch laptop) keeps them away until a finger
// touches it while riding. A keyboard's key while riding puts them away
// until the screen is touched again, for a tablet with a keyboard.

// [label, key code, key, css class, what it does, slide group]
const BUTTONS = [
  ["Gas", "ArrowUp", "ArrowUp", "gas", "Accelerate", "pedal"],
  ["Brake", "ArrowDown", "ArrowDown", "brake", "Brake", "pedal"],
  ["Stunt 1", "KeyZ", "z", "stunt s1", "Stunt 1 (Z)"],
  ["Stunt 2", "KeyX", "x", "stunt s2", "Stunt 2 (X)"],
  ["Clutch", "ControlLeft", "Control", "clutch", "Clutch (hold, then let go with the gas on)"],
  ["Cam", "KeyC", "c", "top cam", "Change camera"],
  ["Look", "AltLeft", "Alt", "top look", "Look back (hold)"],
  ["Back", "Backspace", "Backspace", "top backup", "Back up (hold)"],
  ["Exit", "Escape", "Escape", "top quit", "Leave the race, back to the menus (the game's Esc)"],
];

// The stick: how far from its middle (a share of its reach) a thumb must
// go to steer, and to lean, which wants a firmer push so that steering
// alone doesn't lean the bike too.
const STEER_AT = 0.28;
const LEAN_AT = 0.5;

const CSS = `
#touch { position: fixed; inset: 0; pointer-events: none; user-select: none;
  -webkit-user-select: none; -webkit-touch-callout: none; touch-action: none;
  --l: env(safe-area-inset-left, 0px); --r: env(safe-area-inset-right, 0px);
  --b: env(safe-area-inset-bottom, 0px); --t: env(safe-area-inset-top, 0px); }
#touch button { position: absolute; display: block; margin: 0; pointer-events: auto; touch-action: none;
  background: rgba(20,20,20,.32); color: rgba(255,255,255,.88);
  border: 2px solid rgba(255,255,255,.38); border-radius: 50%;
  font: 700 clamp(11px, 3.4vmin, 17px) system-ui, sans-serif; padding: 0;
  -webkit-tap-highlight-color: transparent; text-shadow: 0 1px 2px #000; }
#touch button.down { background: rgba(163,194,74,.55); border-color: rgba(220,240,160,.8); }
/* The right thumb, clear of the speedometer (the bottom right 27% of the
   screen's height) and the course map (top right). */
#touch .gas { width: min(24vmin, 130px); height: min(24vmin, 130px); right: calc(var(--r) + 2.5vmin); bottom: calc(var(--b) + 30vh); }
#touch .brake { width: min(19vmin, 104px); height: min(19vmin, 104px); right: calc(var(--r) + 30vh); bottom: calc(var(--b) + 3vmin); }
#touch .stunt { width: min(14vmin, 78px); height: min(14vmin, 78px); font-size: clamp(10px, 2.8vmin, 14px); }
#touch .s1 { right: calc(var(--r) + 28vmin); bottom: calc(var(--b) + 49vh); }
#touch .s2 { right: calc(var(--r) + 6vmin); bottom: calc(var(--b) + 30vh + 27vmin); }
#touch .clutch { width: min(13vmin, 72px); height: min(13vmin, 72px); font-size: clamp(9px, 2.6vmin, 13px);
  right: calc(var(--r) + 30vh + 21vmin); bottom: calc(var(--b) + 4vmin); }
/* Along the top, between the race panel (top left) and the course map. */
#touch .top { width: min(11vmin, 64px); height: min(8vmin, 44px); top: calc(var(--t) + 1.5vmin); border-radius: 8px;
  font-size: clamp(9px, 2.6vmin, 13px); }
#touch .cam { left: calc(50% - 25vmin); }
#touch .look { left: calc(50% - 12.5vmin); }
#touch .backup { left: calc(50% + 1.5vmin); }
#touch .quit { left: calc(50% + 14vmin); }
#touch .stick { position: absolute; width: min(30vmin, 170px); height: min(30vmin, 170px); margin: max(-15vmin, -85px) 0 0 max(-15vmin, -85px);
  border-radius: 50%; border: 2px solid rgba(255,255,255,.3); background: rgba(20,20,20,.22);
  left: calc(var(--l) + 20vmin); top: calc(100% - var(--b) - 20vmin); }
#touch .stick.held { border-color: rgba(255,255,255,.5); }
#touch .knob { position: absolute; left: 50%; top: 50%; width: min(13vmin, 74px); height: min(13vmin, 74px);
  margin: max(-6.5vmin, -37px) 0 0 max(-6.5vmin, -37px); border-radius: 50%; background: rgba(255,255,255,.35);
  border: 2px solid rgba(255,255,255,.55); }
#touch .stick span { position: absolute; color: rgba(255,255,255,.6); font: 700 clamp(8px, 2.3vmin, 12px) system-ui, sans-serif;
  left: 50%; transform: translateX(-50%); white-space: nowrap; text-shadow: 0 1px 2px #000; }
#touch .stick .up { top: -3.6vmin; }
#touch .stick .dn { bottom: -3.6vmin; }
#touch.away { display: none; }
#touch .zone { position: absolute; left: 0; bottom: 0; width: 45%; top: 14vmin; pointer-events: auto; touch-action: none; }
#typing { position: fixed; top: 10px; left: calc(50% - 22px); width: 44px; height: 44px; border-radius: 8px;
  background: rgba(20,20,20,.5); color: #fff; border: 2px solid rgba(255,255,255,.4); font-size: 22px;
  padding: 0; touch-action: manipulation; }
/* Kept on the screen (a phone may not open its keyboard for a box off it),
   16px so the phone doesn't zoom in on it, and see-through. */
#typed { position: fixed; left: 0; top: 0; width: 1px; height: 1px; padding: 0; border: 0; opacity: 0;
  font-size: 16px; pointer-events: none; }
#typebar { position: fixed; z-index: 25; top: calc(env(safe-area-inset-top, 0px) + 8px); left: 50%; transform: translateX(-50%);
  display: flex; gap: 10px; align-items: center; max-width: calc(100% - 32px); padding: 6px 6px 6px 14px; border-radius: 10px;
  background: rgba(16,16,14,.9); border: 1px solid rgba(255,255,255,.3); color: #fff; font: 600 18px system-ui, sans-serif; }
#typebar[hidden] { display: none; }
#typebar span { min-width: 8em; white-space: pre; overflow: hidden; text-overflow: ellipsis; }
#typebar span::after { content: "▏"; animation: blink 1s steps(1) infinite; }
@keyframes blink { 50% { opacity: 0; } }
#typebar button { font: 700 15px system-ui, sans-serif; padding: 6px 12px; border-radius: 7px; border: 0;
  background: rgba(163,194,74,.85); color: #111; }
#backnote { position: fixed; z-index: 26; top: 30%; left: 50%; transform: translateX(-50%); padding: 10px 18px; border-radius: 10px;
  background: rgba(16,16,14,.88); border: 1px solid rgba(255,255,255,.3); color: #fff; font: 600 17px system-ui, sans-serif;
  pointer-events: none; }
#backnote[hidden] { display: none; }
#turn { position: fixed; inset: 0; z-index: 30; display: grid; place-items: center; text-align: center;
  background: rgba(10,10,9,.92); color: #ece6d8; font: 600 18px/1.4 system-ui, sans-serif; padding: 24px; }
#turn[hidden] { display: none; }
#turn b { display: block; font-size: 56px; line-height: 1; margin-bottom: 12px; transform: rotate(90deg); }
`;

// The stick as a game pad: one with a left stick and nothing else, added
// to what the browser's getGamepads() lists, which is where the game reads
// pads (gilrs). The page's own pad note (gamepad.js) passes it over, as it
// isn't a pad anyone plugged in. `null` where the browser has no pads.
const PAD_ID = "OpenMCM2 touch stick";
function touchPad() {
  const nav = navigator;
  const real = nav.getGamepads?.bind(nav);
  if (!real) return null;
  const pad = {
    id: PAD_ID,
    index: 7,
    connected: true,
    mapping: "standard",
    timestamp: 0,
    axes: [0, 0, 0, 0],
    // A standard pad's 17 buttons, none of them pressed.
    buttons: Array.from({ length: 17 }, () => ({ pressed: false, touched: false, value: 0 })),
    openmcm2Touch: true,
  };
  try {
    Object.defineProperty(nav, "getGamepads", {
      configurable: true,
      value() {
        const list = [...(real() || [])];
        // In the slot it asked for, or the first one free after it.
        let at = pad.index;
        while (list[at] && list[at] !== pad) at++;
        pad.index = at;
        list[at] = pad;
        for (let i = 0; i < at; i++) list[i] ??= null;
        return list;
      },
    });
  } catch {
    return null;
  }
  return {
    set(x, y) {
      if (pad.axes[0] === x && pad.axes[1] === y) return;
      pad.axes = [x, y, 0, 0];
      pad.timestamp = performance.now();
    },
  };
}

/// Whether a pad from getGamepads() is the touch stick.
export const isTouchPad = (p) => !!p?.openmcm2Touch;

function send(canvas, type, code, key) {
  canvas.dispatchEvent(new KeyboardEvent(type, { code, key, bubbles: true, cancelable: true }));
}

// A short tick under the thumb, where the phone can (Android).
const buzz = () => {
  try {
    navigator.vibrate?.(8);
  } catch {}
};

export function setup(canvas, { hidden = false } = {}) {
  const style = document.createElement("style");
  style.textContent = CSS;
  document.head.append(style);
  const pad = document.createElement("div");
  pad.id = "touch";
  pad.hidden = true;
  // A screen with a mouse or trackpad too: away until a finger touches it.
  if (hidden) pad.classList.add("away");
  const releases = [];

  // A key held by any number of fingers: down with the first, up with the last.
  const key = (code, k) => {
    const held = new Set();
    return {
      press(id) {
        if (!held.size) send(canvas, "keydown", code, k);
        held.add(id);
      },
      release(id) {
        if (held.delete(id) && !held.size) send(canvas, "keyup", code, k);
      },
      clear() {
        if (held.size) send(canvas, "keyup", code, k);
        held.clear();
      },
      get down() {
        return held.size > 0;
      },
    };
  };

  // The buttons. A finger that slides off one onto another of its group
  // (gas and brake) lets go of the first and presses the second.
  const buttons = [];
  for (const [label, code, k, cls, about, group] of BUTTONS) {
    const b = document.createElement("button");
    b.textContent = label;
    b.className = cls;
    b.setAttribute("aria-label", about);
    b.title = about;
    const entry = { b, group, key: key(code, k) };
    buttons.push(entry);
    pad.append(b);
  }
  const on = new Map(); // pointer id -> the button entry it holds
  const look = () => buttons.forEach((e) => e.b.classList.toggle("down", e.key.down));
  const hold = (id, entry) => {
    const was = on.get(id);
    if (was === entry) return;
    was?.key.release(id);
    if (entry) {
      on.set(id, entry);
      entry.key.press(id);
      buzz();
    } else on.delete(id);
    look();
  };
  for (const entry of buttons) {
    const { b } = entry;
    b.addEventListener("pointerdown", (e) => {
      e.preventDefault();
      b.setPointerCapture?.(e.pointerId);
      hold(e.pointerId, entry);
    });
    b.addEventListener("pointermove", (e) => {
      const held = on.get(e.pointerId);
      if (!held?.group) return;
      const under = document.elementFromPoint(e.clientX, e.clientY);
      const next = buttons.find((x) => x.b === under);
      if (next && next !== held && next.group === held.group) hold(e.pointerId, next);
    });
    const up = (e) => hold(e.pointerId, null);
    b.addEventListener("pointerup", up);
    b.addEventListener("pointercancel", up);
    b.addEventListener("lostpointercapture", up);
    b.addEventListener("contextmenu", (e) => e.preventDefault());
  }
  releases.push(() => {
    on.clear();
    buttons.forEach((e) => e.key.clear());
    look();
  });

  // The stick, in the left part of the screen: it starts where the thumb
  // lands, and the thumb's way from there steers and leans.
  const zone = document.createElement("div");
  zone.className = "zone";
  const stick = document.createElement("div");
  stick.className = "stick";
  const knob = document.createElement("div");
  knob.className = "knob";
  const upText = Object.assign(document.createElement("span"), { className: "up", textContent: "Lean fwd" });
  const dnText = Object.assign(document.createElement("span"), { className: "dn", textContent: "Lean back" });
  stick.append(knob, upText, dnText);
  zone.setAttribute("aria-label", "Steer and lean: slide left or right to steer, up or down to lean");
  pad.append(zone, stick);
  const analog = new URLSearchParams(location.search).has("touchkeys") ? null : touchPad();
  const dirs = {
    left: key("ArrowLeft", "ArrowLeft"),
    right: key("ArrowRight", "ArrowRight"),
    fwd: key("KeyW", "w"),
    back: key("KeyS", "s"),
  };
  let thumb = null; // { id, x, y } where it landed
  const rest = () => {
    stick.style.left = stick.style.top = "";
    knob.style.transform = "";
    stick.classList.remove("held");
  };
  const steer = (dx, dy) => {
    const reach = stick.offsetWidth / 2 || 60;
    let x = dx / reach;
    let y = dy / reach;
    const len = Math.hypot(x, y);
    if (len > 1) (x /= len), (y /= len);
    knob.style.transform = `translate(${x * reach}px, ${y * reach}px)`;
    if (analog) {
      // The whole reach is the stick's whole travel; the game has its own
      // dead zone and steering curve, as for any pad.
      analog.set(x, y);
      return;
    }
    const want = { left: x < -STEER_AT, right: x > STEER_AT, fwd: y < -LEAN_AT, back: y > LEAN_AT };
    for (const [d, k] of Object.entries(dirs)) {
      if (want[d] && !k.down) {
        k.press("stick");
        buzz();
      } else if (!want[d] && k.down) k.release("stick");
    }
  };
  const letGo = () => {
    thumb = null;
    analog?.set(0, 0);
    for (const k of Object.values(dirs)) k.clear();
    rest();
  };
  zone.addEventListener("pointerdown", (e) => {
    e.preventDefault();
    if (thumb) return;
    zone.setPointerCapture?.(e.pointerId);
    thumb = { id: e.pointerId, x: e.clientX, y: e.clientY };
    stick.style.left = e.clientX + "px";
    stick.style.top = e.clientY + "px";
    stick.classList.add("held");
    steer(0, 0);
  });
  zone.addEventListener("pointermove", (e) => {
    if (thumb?.id !== e.pointerId) return;
    steer(e.clientX - thumb.x, e.clientY - thumb.y);
  });
  const zoneUp = (e) => {
    if (thumb?.id === e.pointerId) letGo();
  };
  zone.addEventListener("pointerup", zoneUp);
  zone.addEventListener("pointercancel", zoneUp);
  zone.addEventListener("lostpointercapture", zoneUp);
  zone.addEventListener("contextmenu", (e) => e.preventDefault());
  releases.push(letGo);
  document.body.append(pad);

  // A phone held upright: the game wants it on its side.
  const turn = document.createElement("div");
  turn.id = "turn";
  turn.hidden = true;
  turn.innerHTML = "<div><b>📱</b>Turn your phone on its side to ride.</div>";
  document.body.append(turn);
  let playing = false;

  // The screen stays on while the game is up (a replay, a race watched,
  // a long load), where the browser lets a page ask: it lets go by itself
  // when the page is hidden, so it is asked for again on coming back.
  let awake = null;
  const stayAwake = async () => {
    if (!playing || document.hidden || awake || !navigator.wakeLock) return;
    try {
      awake = await navigator.wakeLock.request("screen");
      awake.addEventListener("release", () => (awake = null));
    } catch {}
  };
  document.addEventListener("visibilitychange", stayAwake);

  // A phone's back button or gesture: the game's Escape (back a menu),
  // not away from the page and the game with it. A page in full screen
  // leaves it first, as Android does; the next back is the game's. In a
  // race, where Escape ends it, a back asks for a second one first, as a
  // thumb steering near the screen's edge can make the gesture by chance.
  let trapped = false;
  let backAt = -1e9;
  const trapBack = () => {
    if (trapped) return;
    trapped = true;
    try {
      history.pushState({ mm2back: true }, "");
    } catch {}
  };
  const note = document.createElement("div");
  note.id = "backnote";
  note.hidden = true;
  note.textContent = "Back again to leave the race";
  let noteTimer = 0;
  addEventListener("popstate", () => {
    if (!trapped) return;
    trapped = false;
    trapBack();
    const now = performance.now();
    if (riding && now - backAt > 2500) {
      backAt = now;
      note.hidden = false;
      clearTimeout(noteTimer);
      noteTimer = setTimeout(() => (note.hidden = true), 2500);
      return;
    }
    note.hidden = true;
    send(canvas, "keydown", "Escape", "Escape");
    send(canvas, "keyup", "Escape", "Escape");
  });
  document.body.append(note);
  const upright = matchMedia("(orientation: portrait) and (max-width: 600px)");
  const checkTurn = () => (turn.hidden = !(playing && upright.matches));
  upright.addEventListener?.("change", checkTurn);

  // The menus' typing (a profile's name, chat, a course's name): the
  // phone's keyboard, opened by a hidden text box, its letters passed on
  // as keys. The game says when a box wants typing (typing() below); a
  // phone opens its keyboard only from a tap, so the next tap on the game
  // (the one that picked the box, or any after) opens it, and the ⌨
  // button does too. A bar along the top shows what the box holds, as the
  // keyboard may cover the box itself.
  const typing = document.createElement("button");
  typing.id = "typing";
  typing.textContent = "⌨";
  typing.setAttribute("aria-label", "keyboard");
  typing.title = "Type (a rider's name, chat)";
  typing.hidden = true;
  const typed = document.createElement("input");
  typed.id = "typed";
  typed.type = "text";
  typed.setAttribute("autocapitalize", "none");
  typed.setAttribute("autocorrect", "off");
  typed.setAttribute("autocomplete", "off");
  typed.setAttribute("enterkeyhint", "done");
  typed.spellcheck = false;
  const bar = document.createElement("div");
  bar.id = "typebar";
  bar.hidden = true;
  const shown = document.createElement("span");
  const done = document.createElement("button");
  done.textContent = "Done";
  bar.append(shown, done);
  // The box keeps one space, so the keyboard's delete key always has
  // something to take (an empty box sends nothing for it on some phones).
  const KEEP = " ";
  const reset = () => {
    typed.value = KEEP;
    try {
      typed.setSelectionRange(1, 1);
    } catch {}
  };
  let wanted = null; // what the game's box holds, or null
  let riding = false;
  const open = () => {
    reset();
    typed.focus({ preventScroll: true });
  };
  typing.addEventListener("click", open);
  // A tap on the game while a box wants typing: up comes the keyboard.
  // On touchend and pointerup, the moments a phone allows it; the game
  // has heard the tap by then and said whether a box has the keyboard.
  const tapped = (e) => {
    if (e.pointerType && e.pointerType !== "touch") return;
    if (wanted !== null && document.activeElement !== typed) open();
  };
  canvas.addEventListener("pointerup", tapped);
  canvas.addEventListener("touchend", tapped);
  const press = (code, key) => {
    send(canvas, "keydown", code, key);
    send(canvas, "keyup", code, key);
  };
  const codeOf = (c) => (/[a-z]/i.test(c) ? "Key" + c.toUpperCase() : /[0-9]/.test(c) ? "Digit" + c : c === " " ? "Space" : "");
  typed.addEventListener("input", (e) => {
    const now = typed.value;
    if (e.inputType?.startsWith("delete")) {
      // Delete keys: one press each, however many letters went.
      const gone = Math.max(1, KEEP.length - now.length);
      for (let i = 0; i < gone; i++) press("Backspace", "Backspace");
    } else {
      // Letters, or a word the keyboard put in: what's new after the kept space.
      const text = now.startsWith(KEEP) ? now.slice(KEEP.length) : now;
      for (const c of text) press(codeOf(c), c);
    }
    reset();
  });
  typed.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      press("Enter", "Enter");
      typed.blur();
    }
  });
  done.addEventListener("pointerdown", (e) => e.preventDefault());
  done.addEventListener("click", () => {
    press("Enter", "Enter");
    typed.blur();
  });
  const showBar = () => {
    bar.hidden = !(wanted !== null && document.activeElement === typed);
    typing.style.visibility = bar.hidden ? "" : "hidden";
  };
  typed.addEventListener("focus", showBar);
  typed.addEventListener("blur", () => {
    showBar();
    canvas.focus({ preventScroll: true });
  });
  document.body.append(typing, typed, bar);
  const releaseAll = () => releases.forEach((r) => r());
  // A real keyboard while riding: the buttons keep out of the way until a
  // finger is back on the screen.
  addEventListener(
    "keydown",
    (e) => {
      if (!e.isTrusted || pad.hidden || e.target === typed || pad.classList.contains("away")) return;
      releaseAll();
      pad.classList.add("away");
    },
    true
  );
  addEventListener(
    "pointerdown",
    (e) => {
      if (e.pointerType === "touch") pad.classList.remove("away");
    },
    true
  );
  // A call, the home screen or another app: nothing stays held down.
  addEventListener("blur", releaseAll);
  document.addEventListener("visibilitychange", () => document.hidden && releaseAll());
  return {
    show(on) {
      pad.hidden = !on;
      riding = on;
      typing.hidden = on || wanted === null;
      if (on) typed.blur();
      // Let go of everything when the race ends.
      if (!on) releaseAll();
    },
    // A box wants typing, holding `text`, or none (null): the game says.
    typing(text) {
      wanted = text ?? null;
      shown.textContent = wanted ?? "";
      // The ⌨ button is there only while a box wants typing.
      typing.hidden = riding || wanted === null;
      if (wanted === null && document.activeElement === typed) typed.blur();
      showBar();
    },
    // The game is on screen (menus or riding): upright phones are asked to turn.
    playing(on) {
      playing = on;
      checkTurn();
      if (on) {
        stayAwake();
        trapBack();
      } else awake?.release().catch(() => {});
    },
  };
}

/// Any touch screen, even one with a mouse or trackpad too (`?touch` and
/// `?notouch` as for wanted()): the touch buttons are set up, kept away
/// until a finger is on the screen unless wanted().
export function any() {
  const q = new URLSearchParams(location.search);
  if (q.has("touch")) return true;
  if (q.has("notouch")) return false;
  return matchMedia("(any-pointer: coarse)").matches || navigator.maxTouchPoints > 0;
}

/// A touch screen with no mouse or trackpad, or `?touch` in the address
/// (`?notouch` keeps them off). A laptop's touch screen, or a tablet with a
/// trackpad, plays with the keyboard and mouse it has.
export function wanted() {
  const q = new URLSearchParams(location.search);
  if (q.has("touch")) return true;
  if (q.has("notouch")) return false;
  return matchMedia("(any-pointer: coarse)").matches && !matchMedia("(any-pointer: fine)").matches;
}

/// A phone (or `?phone`): a touch screen whose short side is a phone's.
/// It gets the light mode unless that's turned off.
export function phone() {
  if (new URLSearchParams(location.search).has("phone")) return true;
  return matchMedia("(pointer: coarse)").matches && Math.min(screen.width, screen.height) <= 600;
}
