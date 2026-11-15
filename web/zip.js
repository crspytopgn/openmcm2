// Reads the files out of a .zip of the game folder, for phones and tablets,
// where a page can't be handed a whole folder. Stored and deflated entries
// (what every zip tool writes) are read with the browser's own inflater.

function u16(v, o) {
  return v.getUint16(o, true);
}
function u32(v, o) {
  return v.getUint32(o, true);
}

/// The entries as [path, { size, arrayBuffer() }], files only.
export async function entries(file) {
  // The end record is in the last 64 KB + 22 bytes.
  const tailStart = Math.max(0, file.size - 65557);
  const tail = new DataView(await file.slice(tailStart).arrayBuffer());
  let end = -1;
  for (let i = tail.byteLength - 22; i >= 0; i--) {
    if (u32(tail, i) === 0x06054b50) {
      end = i;
      break;
    }
  }
  if (end < 0) throw new Error("that file is not a zip");
  const count = u16(tail, end + 10);
  const dirSize = u32(tail, end + 12);
  const dirStart = u32(tail, end + 16);
  const dir = new DataView(await file.slice(dirStart, dirStart + dirSize).arrayBuffer());
  const names = new TextDecoder();
  const out = [];
  let o = 0;
  for (let n = 0; n < count && u32(dir, o) === 0x02014b50; n++) {
    const method = u16(dir, o + 10);
    const packed = u32(dir, o + 20);
    const size = u32(dir, o + 24);
    const nameLen = u16(dir, o + 28);
    const extraLen = u16(dir, o + 30);
    const commentLen = u16(dir, o + 32);
    const local = u32(dir, o + 42);
    const name = names.decode(new Uint8Array(dir.buffer, o + 46, nameLen)).replaceAll("\\", "/");
    o += 46 + nameLen + extraLen + commentLen;
    if (name.endsWith("/")) continue;
    out.push([
      name,
      {
        size,
        async arrayBuffer() {
          const head = new DataView(await file.slice(local, local + 30).arrayBuffer());
          const start = local + 30 + u16(head, 26) + u16(head, 28);
          const data = file.slice(start, start + packed);
          if (method === 0) return data.arrayBuffer();
          if (method !== 8) throw new Error(`${name}: unsupported zip packing`);
          const stream = data.stream().pipeThrough(new DecompressionStream("deflate-raw"));
          return new Response(stream).arrayBuffer();
        },
      },
    ]);
  }
  return out;
}
