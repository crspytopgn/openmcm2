// Keeping rude words out of names other players see: shared tracks and
// their makers, leaderboard riders and races in the server browser. The
// same rules as the game's (crates/mm2-settings/src/clean.rs), from the
// same list (badwords.txt: LDNOOBW, CC BY 4.0).

import LIST from "./badwords.txt";

const LEET = { 0: "o", 1: "i", "!": "i", "|": "i", 3: "e", 4: "a", "@": "a", 5: "s", $: "s", 7: "t", 8: "b" };

function fold(s) {
  let out = "";
  for (const ch of String(s).toLowerCase()) {
    if (LEET[ch]) out += LEET[ch];
    else if (/\p{L}/u.test(ch) || ch === "&") out += ch;
  }
  return out;
}

const WORDS = new Set();
const JOINED = new Set();
for (const raw of LIST.split("\n")) {
  const l = raw.trim();
  if (!l || l.startsWith("#")) continue;
  const parts = l.split(/\s+/).map(fold);
  if (parts.length === 1) WORDS.add(parts[0]);
  JOINED.add(parts.join(""));
}

const ENDS = ["s", "es", "ed", "er", "ers", "ing", "y"];

// Words rude wherever they are in a longer word ("shithead"), and never
// part of an ordinary one.
const ANYWHERE = ["fuck", "shit", "wank", "nigger", "nigga", "faggot"];

function badWord(w) {
  if (!w) return false;
  if (WORDS.has(w) || ANYWHERE.some((a) => w.includes(a))) return true;
  return ENDS.some((e) => w.endsWith(e) && w.length - e.length >= 3 && WORDS.has(w.slice(0, -e.length)));
}

const PART = /[\p{L}\p{N}@$!&]/u;

// The name with each rude word starred out but its first letter; a
// listed phrase or a word spelled a letter at a time starred whole.
export function mask(name) {
  const chars = [...String(name)];
  const spans = [];
  for (let i = 0; i < chars.length; ) {
    if (PART.test(chars[i])) {
      const a = i;
      while (i < chars.length && PART.test(chars[i])) i++;
      spans.push([a, i]);
    } else i++;
  }
  const folded = spans.map(([a, b]) => fold(chars.slice(a, b).join("")));
  const bad = folded.map(badWord);
  const whole = folded.map(() => false);
  for (let k = 0; k < spans.length; k++) {
    let run = "";
    for (let n = k; n < Math.min(spans.length, k + 4); n++) {
      run += folded[n];
      if (n > k && JOINED.has(run)) for (let m = k; m <= n; m++) whole[m] = true;
    }
    let n = k;
    let spelled = "";
    while (n < spans.length && [...folded[n]].length === 1) spelled += folded[n++];
    if (n - k >= 3 && (badWord(spelled) || JOINED.has(spelled))) for (let m = k; m < n; m++) whole[m] = true;
  }
  spans.forEach(([a, b], k) => {
    for (let i = whole[k] ? a : bad[k] ? a + 1 : b; i < b; i++) chars[i] = "*";
  });
  return chars.join("");
}
