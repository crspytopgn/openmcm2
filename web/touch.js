// Touch controls for phones and tablets, shown only while riding. Each
// one presses the game's own key (the factory keyboard map, UI/PRESET.CTL),
// so the game sees a keyboard, as the original's keys play. In the menus a
// tap is a click and a drag moves sliders and scroll bars (main.rs).
//
// The left thumb rides a stick that appears where it lands: left and right
// steer, up leans forward and down leans back (W/S), so steering and
// leaning go together in the air. The right thumb has the gas and the
// brake, and slides from one to the other without lifting. The stunts and
// the clutch sit above them, and the small buttons along the top are the
// camera, looking back, backing up and the pause menu.
//
// They come up only on a touch screen with no mouse or trackpad (a phone,
// or a tablet on its own). A keyboard's key while riding puts them away
// until the screen is touched again, for a tablet with a keyboard.

// [label, key code, key, css class, what it does, slide group]
const BUTTONS = [
  ["Gas", "ArrowUp", "ArrowUp", "gas", "Accelerate", "pedal"],
  ["Brake", "ArrowDown", "ArrowDown", "brake", "Brake", "pedal"],
  ["Stunt 1", "KeyZ", "z", "stunt one", "Stunt 1 (Z)"],
  ["Stunt 2", "KeyX", "x", "stunt two", "Stunt 2 (X)"],
  ["Clutch", "ControlLeft", "Control", "clutch", "Clutch (hold, then let go with the gas on)"],
  ["Cam", "KeyC", "c", "top cam", "Change camera"],
  ["Look", "AltLeft", "Alt", "top look", "Look back (hold)"],
  ["Back", "Backspace", "Backspace", "top backup", "Back up (hold)"],
  ["II", "Escape", "Escape", "top quit", "Pause menu"],
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
#touch button { position: absolute; pointer-events: auto; touch-action: none;
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
#touch .one { right: calc(var(--r) + 28vmin); bottom: calc(var(--b) + 49vh); }
#touch .two { right: calc(var(--r) + 6vmin); bottom: calc(var(--b) + 30vh + 27vmin); }
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
#typed { position: fixed; left: -1000px; top: 0; width: 10px; opacity: 0; }
#turn { position: fixed; inset: 0; z-index: 30; display: grid; place-items: center; text-align: center;
  background: rgba(10,10,9,.92); color: #ece6d8; font: 600 18px/1.4 system-ui, sans-serif; padding: 24px; }
#turn[hidden] { display: none; }
#turn b { display: block; font-size: 56px; line-height: 1; margin-bottom: 12px; transform: rotate(90deg); }
`;

function send(canvas, type, code, key) {
  canvas.dispatchEvent(new KeyboardEvent(type, { code, key, bubbles: true, cancelable: true }));
}

// A short tick under the thumb, where the phone can (Android).
const buzz = () => {
  try {
    navigator.vibrate?.(8);
  } catch {}
};

export function setup(canvas) {
  const style = document.createElement("style");
  style.textContent = CSS;
  document.head.append(style);
  const pad = document.createElement("div");
  pad.id = "touch";
  pad.hidden = true;
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
  const upright = matchMedia("(orientation: portrait) and (max-width: 600px)");
  const checkTurn = () => (turn.hidden = !(playing && upright.matches));
  upright.addEventListener?.("change", checkTurn);

  // The menus' typing (a profile's name, chat): the phone's keyboard,
  // opened by a hidden text box, its letters passed on as keys.
  const typing = document.createElement("button");
  typing.id = "typing";
  typing.textContent = "⌨";
  typing.setAttribute("aria-label", "keyboard");
  typing.title = "Type (a rider's name, chat)";
  typing.hidden = true;
  const typed = document.createElement("input");
  typed.id = "typed";
  typed.autocapitalize = "off";
  typed.autocomplete = "off";
  typing.addEventListener("click", () => typed.focus());
  const press = (code, key) => {
    send(canvas, "keydown", code, key);
    send(canvas, "keyup", code, key);
  };
  typed.addEventListener("input", (e) => {
    if (e.inputType === "deleteContentBackward") press("Backspace", "Backspace");
    for (const c of e.data || "") {
      const code = /[a-z]/i.test(c) ? "Key" + c.toUpperCase() : /[0-9]/.test(c) ? "Digit" + c : c === " " ? "Space" : "";
      press(code, c);
    }
    typed.value = "";
  });
  typed.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      press("Enter", "Enter");
      typed.blur();
    } else if (e.key === "Backspace" && !typed.value) press("Backspace", "Backspace");
  });
  document.body.append(typing, typed);
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
      typing.hidden = on;
      if (on) typed.blur();
      // Let go of everything when the race ends.
      if (!on) releaseAll();
    },
    // The game is on screen (menus or riding): upright phones are asked to turn.
    playing(on) {
      playing = on;
      checkTurn();
    },
  };
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
