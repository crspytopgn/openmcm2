// Touch buttons for phones and tablets, shown only while riding. Each one
// presses the game's own key (the factory keyboard map, UI/PRESET.CTL), so
// the game sees a keyboard. In the menus a tap is a click (main.rs).

const BUTTONS = [
  // [label, key code, key, css class]
  ["◀", "ArrowLeft", "ArrowLeft", "steer left"],
  ["▶", "ArrowRight", "ArrowRight", "steer right"],
  ["Gas", "ArrowUp", "ArrowUp", "pedal gas"],
  ["Brake", "ArrowDown", "ArrowDown", "pedal brake"],
  ["Lean ▲", "KeyW", "w", "lean forward"],
  ["Lean ▼", "KeyS", "s", "lean back"],
  ["Stunt 1", "KeyZ", "z", "stunt one"],
  ["Stunt 2", "KeyX", "x", "stunt two"],
  ["✕", "Escape", "Escape", "quit"],
];

const CSS = `
#touch { position: fixed; inset: 0; pointer-events: none; user-select: none;
  -webkit-user-select: none; touch-action: none; }
#touch button { position: absolute; pointer-events: auto; touch-action: none;
  background: rgba(20,20,20,.35); color: rgba(255,255,255,.85);
  border: 2px solid rgba(255,255,255,.35); border-radius: 50%;
  font: 600 15px system-ui, sans-serif; padding: 0; }
#touch button.down { background: rgba(232,163,58,.55); }
#touch .steer { width: 84px; height: 84px; bottom: 24px; font-size: 28px; }
#touch .left { left: 20px; }
#touch .right { left: 120px; }
#touch .pedal { width: 92px; height: 92px; right: 20px; }
/* Above the speedometer, which fills the bottom right corner. */
#touch .gas { bottom: 128px; }
#touch .brake { bottom: 236px; width: 72px; height: 72px; right: 30px; }
#touch .lean { width: 64px; height: 64px; right: 132px; font-size: 12px; }
#touch .forward { bottom: 210px; }
#touch .back { bottom: 134px; }
#touch .stunt { width: 56px; height: 56px; left: 30px; font-size: 11px; }
#touch .one { bottom: 128px; }
#touch .two { bottom: 128px; left: 100px; }
#typing { position: fixed; top: 10px; left: 10px; width: 44px; height: 44px; border-radius: 8px;
  background: rgba(20,20,20,.5); color: #fff; border: 2px solid rgba(255,255,255,.4); font-size: 22px;
  padding: 0; touch-action: manipulation; }
#typed { position: fixed; left: -1000px; top: 0; width: 10px; opacity: 0; }
/* Clear of the race panel (top left) and the course map (top right). */
#touch .quit { width: 40px; height: 40px; top: 10px; left: calc(50% - 20px); border-radius: 8px; }
`;

function send(canvas, type, code, key) {
  canvas.dispatchEvent(new KeyboardEvent(type, { code, key, bubbles: true, cancelable: true }));
}

export function setup(canvas) {
  const style = document.createElement("style");
  style.textContent = CSS;
  document.head.append(style);
  const pad = document.createElement("div");
  pad.id = "touch";
  pad.hidden = true;
  const releases = [];
  for (const [label, code, key, cls] of BUTTONS) {
    const b = document.createElement("button");
    b.textContent = label;
    b.className = cls;
    b.setAttribute("aria-label", cls);
    const held = new Set();
    const up = (e) => {
      if (!held.delete(e.pointerId) || held.size) return;
      b.classList.remove("down");
      send(canvas, "keyup", code, key);
    };
    b.addEventListener("pointerdown", (e) => {
      e.preventDefault();
      b.setPointerCapture(e.pointerId);
      if (!held.size) {
        b.classList.add("down");
        send(canvas, "keydown", code, key);
      }
      held.add(e.pointerId);
    });
    releases.push(() => {
      if (!held.size) return;
      held.clear();
      b.classList.remove("down");
      send(canvas, "keyup", code, key);
    });
    b.addEventListener("pointerup", up);
    b.addEventListener("pointercancel", up);
    b.addEventListener("contextmenu", (e) => e.preventDefault());
    pad.append(b);
  }
  document.body.append(pad);
  // The menus' typing (a profile's name, chat): the phone's keyboard,
  // opened by a hidden text box, its letters passed on as keys.
  const typing = document.createElement("button");
  typing.id = "typing";
  typing.textContent = "⌨";
  typing.setAttribute("aria-label", "keyboard");
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
  return {
    show(on) {
      pad.hidden = !on;
      typing.hidden = on;
      if (on) typed.blur();
      // Let go of everything when the race ends.
      if (!on) releases.forEach((r) => r());
    },
  };
}

/// A touch screen with no mouse, or `?touch` in the address.
export function wanted() {
  return new URLSearchParams(location.search).has("touch") || matchMedia("(pointer: coarse)").matches;
}
