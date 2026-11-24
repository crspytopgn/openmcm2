// Bakes a Course Builder track off the page's thread: the browser
// version's .ENV, or the original game's. Both take a few seconds or more.

import init, * as mm from "./pkg/mm2_webtools.js";

const ready = init();
onmessage = async ({ data: { id, what, text } }) => {
  await ready;
  try {
    const out = what === "original" ? mm.course_original(text) : mm.course_bake(text);
    postMessage({ id, out }, [out.buffer]);
  } catch (err) {
    postMessage({ id, err: String(err?.message || err) });
  }
};
