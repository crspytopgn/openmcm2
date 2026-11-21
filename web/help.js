// Small helpers for players: the Home Screen tip on phones and tablets,
// and a bug report that copies in one tap.

const ISSUES = "https://github.com/crspytopgn/openmcm2/issues/new";

const store = {
  get(k) {
    try {
      return localStorage.getItem(k);
    } catch {
      return null;
    }
  },
  set(k, v) {
    try {
      localStorage.setItem(k, v);
    } catch {}
  },
};

/// iPhone and iPad (an iPad says it's a Mac, but has a touch screen).
export function apple() {
  return /iPhone|iPad|iPod/.test(navigator.userAgent) || (/Macintosh/.test(navigator.userAgent) && navigator.maxTouchPoints > 1);
}

/// Opened from the Home Screen (or as an installed app).
export function installed() {
  return navigator.standalone === true || matchMedia("(display-mode: standalone), (display-mode: fullscreen)").matches;
}

// Chrome and Edge on Android offer their own install box: kept for the
// tip's button.
let offer = null;
const offered = [];
addEventListener("beforeinstallprompt", (e) => {
  e.preventDefault();
  offer = e;
  offered.forEach((f) => f());
});

/// The tip on the start page: add the game to the Home Screen, so it opens
/// full screen like an app. Shown on touch screens until it's closed.
export function installTip(box, { touch }) {
  const fresh = installed();
  // Opened from the iPhone's Home Screen for the first time: it has its own
  // storage there, so say why the game files are asked for again.
  if (fresh) {
    if (!apple() || store.get("mm2homenote")) return;
    box.innerHTML = `<p><b>Playing from your Home Screen.</b> On iPhone and iPad this copy keeps its own storage,
      apart from Safari's, so pick your game files or download the Free Pack once more here. Your saves start fresh too.</p>
      <button type="button" class="plain small">Got it</button>`;
    box.hidden = false;
    box.querySelector("button").addEventListener("click", () => {
      store.set("mm2homenote", "1");
      box.hidden = true;
    });
    return;
  }
  if (!touch || store.get("mm2installtip")) return;
  const how = apple()
    ? `Tap <b>Share</b> <span aria-hidden="true">(the square with the arrow)</span>, then <b>Add to Home Screen</b>.`
    : `Open the browser's menu and choose <b>Install app</b> or <b>Add to Home screen</b>.`;
  const note = apple()
    ? `<p class="hint">The Home Screen copy keeps its own storage on iPhone and iPad, so you pick your game files or
        download the Free Pack again there, once. Saves made here in Safari don't move across.</p>`
    : "";
  box.innerHTML = `<p><b>Add it to your Home Screen</b> to play full screen, with no address bar, and to start it
      without the internet once it has been played. ${how}</p>${note}
    <div class="row"><button type="button" class="small" data-install hidden>Install</button>
      <button type="button" class="plain small" data-close>Not now</button></div>`;
  box.hidden = false;
  const install = box.querySelector("[data-install]");
  const showInstall = () => (install.hidden = !offer);
  offered.push(showInstall);
  showInstall();
  install.addEventListener("click", async () => {
    if (!offer) return;
    offer.prompt();
    const { outcome } = await offer.userChoice.catch(() => ({}));
    offer = null;
    if (outcome === "accepted") box.hidden = true;
    showInstall();
  });
  box.querySelector("[data-close]").addEventListener("click", () => {
    store.set("mm2installtip", "1");
    box.hidden = true;
  });
}

// What the graphics card says about itself, where the browser tells.
async function gpuInfo() {
  try {
    const a = await navigator.gpu?.requestAdapter({ powerPreference: "high-performance" });
    if (!a) return navigator.gpu ? "WebGPU: no adapter" : "WebGPU: not in this browser";
    const i = a.info || (await a.requestAdapterInfo?.()) || {};
    const bits = [i.vendor, i.architecture, i.device, i.description].filter(Boolean).join(" / ");
    return `WebGPU: ${bits || "adapter (no details given)"}${a.isFallbackAdapter ? " (software)" : ""}`;
  } catch (e) {
    return "WebGPU: " + e;
  }
}

async function storageInfo() {
  try {
    const e = await navigator.storage?.estimate?.();
    const kept = await navigator.storage?.persisted?.();
    if (!e) return "unknown";
    const mb = (n) => Math.round(n / 1048576);
    return `${mb(e.usage)} MB used of ${mb(e.quota)} MB${kept ? ", kept" : ""}`;
  } catch {
    return "unknown";
  }
}

/// A bug report: what this browser and device are, what was picked, the
/// loading steps and the last messages. Nothing personal, no file names.
export async function report(about) {
  const lines = [
    "OpenMCM2 browser bug report",
    `Version: ${about.version}`,
    `When: ${new Date().toISOString()}`,
    `Browser: ${navigator.userAgent}`,
    `Screen: ${screen.width}x${screen.height} @${devicePixelRatio}x, window ${innerWidth}x${innerHeight}, ${
      matchMedia("(pointer: coarse)").matches ? "touch" : "mouse"
    }${matchMedia("(any-pointer: fine)").matches ? " + mouse/trackpad" : ""}`,
    `Memory: ${navigator.deviceMemory ? navigator.deviceMemory + " GB" : "unknown"}, ${navigator.hardwareConcurrency || "?"} cores`,
    await gpuInfo(),
    `Storage: ${await storageInfo()}`,
    `From the Home Screen: ${installed() ? "yes" : "no"} · online: ${navigator.onLine ? "yes" : "no"} · offline copy: ${
      navigator.serviceWorker?.controller ? "yes" : "no"
    }`,
  ];
  for (const [k, v] of Object.entries(about.extra || {})) lines.push(`${k}: ${v}`);
  if (about.steps?.length) lines.push("", "Loading steps:", ...about.steps);
  if (about.problem) lines.push("", "Problem:", about.problem);
  if (about.recent?.length) lines.push("", "Last messages:", ...about.recent);
  return lines.join("\n");
}

/// Copies text, with the older way for browsers that refuse the new one.
/// False if neither worked (the caller shows the text to copy by hand).
export async function copy(text) {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {}
  const t = document.createElement("textarea");
  t.value = text;
  t.setAttribute("readonly", "");
  t.style.cssText = "position:fixed;left:-9999px;top:0";
  document.body.append(t);
  t.select();
  let ok = false;
  try {
    ok = document.execCommand("copy");
  } catch {}
  t.remove();
  return ok;
}

/// A button that copies a fresh report, saying so, and shows the text to
/// copy by hand if the browser won't. `make` gives the report's text.
export function copyButton(button, make, shown) {
  const label = button.textContent;
  button.addEventListener("click", async () => {
    const text = await make();
    const ok = await copy(text);
    if (shown) {
      shown.textContent = text;
      shown.hidden = ok;
    }
    button.textContent = ok ? "Copied. Paste it in a new issue" : "Copy the text below";
    setTimeout(() => (button.textContent = label), 4000);
  });
}

export const issuesUrl = `${ISSUES}?labels=browser&title=${encodeURIComponent("Browser: ")}`;
