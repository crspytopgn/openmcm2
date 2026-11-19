// Game pads in the browser. The game reads them itself (gilrs, through the
// browser's Gamepad API), but a browser keeps every pad hidden from a page
// until one of its buttons is pressed while the page is open. This says so
// on the start page and over the game, and shows the pad once it's seen.

const CSS = `
.padrow { display: flex; flex-wrap: wrap; align-items: center; gap: .4rem .8rem; margin: 0 0 .2rem;
  padding: .7rem .9rem; background: #1a1917; border: 1px solid #34322c; border-radius: 10px; }
.padrow.found { border-color: var(--accent); background: #22241b; }
.padrow .padicon { font-size: 1.6rem; line-height: 1; filter: grayscale(1); opacity: .7; transition: opacity .2s; }
.padrow.found .padicon { filter: none; opacity: 1; }
.padrow .padtext { flex: 1 1 14rem; margin: 0; font-size: .9rem; color: var(--dim); }
.padrow .padtext b { color: var(--fg); }
.padrow .padlight { width: .7rem; height: .7rem; border-radius: 50%; border: 2px solid #55524a; flex: none; }
.padrow.found .padlight { border-color: var(--accent); }
.padrow .padlight.on { background: var(--accent); }
#padtoast { position: fixed; top: 52px; left: 50%; transform: translateX(-50%); z-index: 7;
  padding: .45rem 1rem; background: rgba(18,17,16,.9); border-left: 4px solid var(--accent);
  border-radius: 4px; font-size: .9rem; color: var(--fg); max-width: calc(100vw - 32px);
  pointer-events: none; transition: opacity .6s; }
#padtoast.gone { opacity: 0; }
`;

const SEEN = "mm2padseen";
const HINTS = "mm2padhints";
const store = {
  get(k) {
    try { return localStorage.getItem(k); } catch { return null; }
  },
  set(k, v) {
    try { localStorage.setItem(k, v); } catch {}
  },
};

// A pad's name as people know it: Chrome adds "(STANDARD GAMEPAD Vendor:
// 045e Product: 0b13)", Firefox puts "045e-0b13-" in front.
export function plainName(id) {
  let n = String(id || "").trim();
  n = n.replace(/\s*\((?=[^)]*(?:Vendor:|STANDARD GAMEPAD|XInput))[^)]*\)\s*$/, "");
  n = n.replace(/^[0-9a-f]{4}-[0-9a-f]{4}-/i, "");
  return n.trim() || "Your game pad";
}

function pads() {
  try {
    return [...(navigator.getGamepads?.() || [])].filter((p) => p && p.connected !== false);
  } catch {
    return [];
  }
}

function anyPressed(list) {
  return list.some((p) => p.buttons.some((b) => b.pressed) || p.axes.some((a) => Math.abs(a) > 0.5));
}

export function setup(row) {
  const style = document.createElement("style");
  style.textContent = CSS;
  document.head.append(style);

  const supported = typeof navigator.getGamepads === "function" && window.isSecureContext !== false;
  row.classList.add("padrow");
  row.title =
    "Game pads and joysticks work in the browser as they do in the game: Xbox, PlayStation, Switch Pro and most USB or Bluetooth pads. " +
    "Plug it in or pair it, then press any button on it while this page is open: until then the browser doesn't show it to the page. " +
    "It rides the bike straight away, with the keyboard still working too. To change which button does what, open Options, Controls in the game.";
  row.innerHTML = `<span class="padicon" aria-hidden="true">🎮</span><p class="padtext" role="status" aria-live="polite"></p><span class="padlight" aria-hidden="true"></span>`;
  const text = row.querySelector(".padtext");
  const light = row.querySelector(".padlight");

  let playing = false;
  let toastTimer = 0;
  const toast = document.createElement("div");
  toast.id = "padtoast";
  toast.hidden = true;
  document.body.append(toast);
  function say(msg, ms = 5000) {
    toast.textContent = msg;
    toast.hidden = false;
    toast.classList.remove("gone");
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      toast.classList.add("gone");
      toastTimer = setTimeout(() => (toast.hidden = true), 700);
    }, ms);
  }

  const listeners = [];
  function show() {
    const list = pads();
    row.classList.toggle("found", list.length > 0);
    if (!supported) {
      text.innerHTML = "<b>Game pads:</b> this browser doesn't let pages use them. Chrome, Edge and Firefox do. The keyboard works here.";
    } else if (list.length) {
      const names = list.map((p) => plainName(p.id));
      const who = names.length > 1 ? `${names.length} pads (${names.join(", ")}) are` : `<b>${names[0]}</b> is`;
      text.innerHTML = `${who} ready. It rides the bike straight away, no setup needed. To change its buttons, open Options, Controls in the game.`;
    } else {
      text.innerHTML =
        "<b>Using a game pad?</b> Plug it in or pair it, then <b>press any button on it</b>. Browsers only show a pad to a page after a button is pressed. The keyboard works too.";
    }
    listeners.forEach((f) => f(list.length > 0));
  }

  addEventListener("gamepadconnected", (e) => {
    store.set(SEEN, "1");
    show();
    if (playing) say(`🎮 ${plainName(e.gamepad.id)} connected. Ready to ride.`);
  });
  addEventListener("gamepaddisconnected", (e) => {
    show();
    if (playing) say(`${plainName(e.gamepad.id)} disconnected. The keyboard still works.`);
  });
  show();

  // The light blinks with the pad's buttons, so it's plain it works.
  // Some browsers only fill getGamepads() when polled, never firing
  // gamepadconnected; this catches those too.
  let count = pads().length;
  (function poll() {
    const list = pads();
    if (list.length !== count) {
      count = list.length;
      if (count) store.set(SEEN, "1");
      show();
    }
    light.classList.toggle("on", anyPressed(list));
    if (!playing) requestAnimationFrame(poll);
  })();

  return {
    // Whether a pad is plugged in and seen.
    found: () => pads().length > 0,
    onChange(f) {
      listeners.push(f);
    },
    // The game is on screen. With no pad seen yet, a word on waking one:
    // every time for someone who has used a pad here before (the browser
    // needs that press on every visit), and a few first times otherwise.
    started({ touch = false } = {}) {
      playing = true;
      if (!supported || pads().length) return;
      if (store.get(SEEN) === "1") {
        say("🎮 Using your game pad? Press any button on it to wake it up.", 7000);
        return;
      }
      const shown = Number(store.get(HINTS) || 0);
      if (touch || shown >= 3) return;
      store.set(HINTS, String(shown + 1));
      say("🎮 Got a game pad? Press any button on it to use it. Or ride with the keyboard.", 7000);
    },
  };
}
