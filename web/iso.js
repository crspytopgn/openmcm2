// Reads the files off the player's CD image: a plain .iso, or a raw image
// such as a .bin (from a .cue), .img, .mdf or .nrg, whatever its sector
// size. Only the ISO 9660 file system the game's CD uses is needed. The
// same reader as the desktop launcher's (crates/mm2-launcher/src/iso.rs).

// The sector sizes and where each sector's 2048 bytes of data start:
// cooked, raw Mode 1, raw Mode 2 form 1, and Mode 2 without the sync.
const LAYOUTS = [[2048, 0], [2352, 16], [2352, 24], [2336, 8]];
// Some images start with a header before the first sector.
const STARTS = [0, 150 * 2352, 150 * 2048];

function record(v, at, len) {
  if (len < 34) return null;
  const nameLen = v.getUint8(at + 32);
  const raw = new Uint8Array(v.buffer, v.byteOffset + at + 33, nameLen);
  let name;
  if (nameLen === 1 && raw[0] <= 1) name = raw[0] ? ".." : ".";
  else {
    name = new TextDecoder("latin1").decode(raw).split(";")[0];
    if (name.endsWith(".")) name = name.slice(0, -1);
  }
  return {
    name,
    dir: (v.getUint8(at + 25) & 2) !== 0,
    lba: v.getUint32(at + 2, true),
    size: v.getUint32(at + 10, true),
  };
}

/// The files under the image's GAME folder as [path from GAME, { size,
/// arrayBuffer() }], or null when it has none.
export async function entries(file) {
  let layout = null;
  for (const start of STARTS) {
    for (const [sector, data] of LAYOUTS) {
      const at = start + 16 * sector + data;
      if (at + 2048 > file.size) continue;
      const pvd = new DataView(await file.slice(at, at + 2048).arrayBuffer());
      if (pvd.getUint8(0) === 1 && new TextDecoder().decode(new Uint8Array(pvd.buffer, 1, 5)) === "CD001") {
        layout = { start, sector, data, root: record(pvd, 156, 34) };
        break;
      }
    }
    if (layout) break;
  }
  if (!layout) throw new Error("that file is not a CD image");
  const { start, sector, data } = layout;

  async function read(lba, size) {
    if (sector === 2048) return file.slice(start + lba * 2048, start + lba * 2048 + size).arrayBuffer();
    // Raw sectors: read the run once, then take each sector's data.
    const count = Math.ceil(size / 2048);
    const from = start + lba * sector;
    const raw = new Uint8Array(await file.slice(from, from + count * sector).arrayBuffer());
    const out = new Uint8Array(size);
    for (let i = 0; i < count; i++) {
      const n = Math.min(2048, size - i * 2048);
      out.set(raw.subarray(i * sector + data, i * sector + data + n), i * 2048);
    }
    return out.buffer;
  }

  async function list(dir) {
    const v = new DataView(await read(dir.lba, dir.size));
    const out = [];
    let at = 0;
    while (at < v.byteLength) {
      const len = v.getUint8(at);
      if (len === 0) {
        // Records never cross a sector: on to the next one.
        at = (Math.floor(at / 2048) + 1) * 2048;
        continue;
      }
      if (at + len > v.byteLength) break;
      const e = record(v, at, len);
      if (e && e.name !== "." && e.name !== "..") out.push(e);
      at += len;
    }
    return out;
  }

  const game = (await list(layout.root)).find((e) => e.dir && e.name.toUpperCase() === "GAME");
  if (!game) return null;
  const out = [];
  const todo = [["", game]];
  while (todo.length) {
    const [path, dir] = todo.pop();
    for (const e of await list(dir)) {
      const p = path + e.name;
      if (e.dir) todo.push([p + "/", e]);
      else out.push([p, { size: e.size, arrayBuffer: () => read(e.lba, e.size) }]);
    }
  }
  return out;
}
