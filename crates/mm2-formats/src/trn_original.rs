//! Writing a `.trn` the original game draws and rides as it does its own.
//!
//! [`crate::trn::Build::write`] keeps only what this port reads. The
//! original also reads, for every 17 × 17 block of samples, how much each
//! sample may be left out while the terrain is far away (its level of
//! detail), and for every tree node the height range it covers, which it
//! culls by. With those left at zero the original game draws a builder
//! track nearly flat and culls it wrongly, so tracks meant for it are
//! written here with them filled in:
//!
//! ```text
//! block:  u16 first[256]    position in `order` of the first sample whose
//!                           error step is at least i
//!         u16 order[290]    the 289 samples by error step, smallest first
//!         sample[289]       zone, error step, u16 height, normal, flags
//!         u32 base height
//! node:   f32 relief, f32 unit, f32 lowest, f32 highest, u16 largest error,
//!         u16 block, u32 texture, u32 tile textures[16], u32 extra bytes,
//!         u8 22, u32 children
//! ```
//!
//! A sample's error is how far it sits from the line between the two
//! samples it is drawn between when it is left out, in height steps; the
//! step is the smallest `i` with `(i + 1) ^ (1 + i² / 255²) - 1` at least
//! that. Each tree node above the leaves also gets a block of its own: its
//! area sampled every 16 (or 256) units. The textures are listed once, as
//! in the shipped tracks, and the file starts with a 256 colour palette
//! (only drawn with in the original's software mode).

use crate::trn::{Build, MAGIC};
use crate::{Error, Result};

/// How many textures the original's terrain keeps (Terrain +0x544..+0xbe8).
pub const MOST_TEXTURES: usize = 425;
/// The kind word after the palette: every shipped track has 3, and the
/// original's riding reads it.
const KIND: u32 = 3;
/// A "no texture" slot.
const NONE: u32 = 0xfffe;
/// The byte the shipped tracks keep before each node's children.
const NODE_BYTE: u8 = 22;

/// The error curve: step `i` stands for errors up to this many height steps.
fn curve() -> [u32; 256] {
    let mut c = [0u32; 256];
    for (i, v) in c.iter_mut().enumerate() {
        let f = i as f64;
        *v = ((f + 1.0).powf(f * f / 65025.0 + 1.0) as i64 - 1).clamp(0, 65535) as u32;
    }
    c
}

/// The step for an error of `e` height steps.
fn step(curve: &[u32; 256], e: u32) -> u8 {
    curve.iter().position(|&c| c >= e).unwrap_or(255) as u8
}

/// The two samples each sample of a 17 × 17 block is drawn between when it
/// is left out, finest first. Corners have none.
fn parents() -> Vec<(usize, usize, usize)> {
    let mut out = Vec::new();
    let mut h = 1usize;
    while h < 16 {
        for z in (0..=16).step_by(h) {
            for x in (0..=16).step_by(h) {
                let (ox, oz) = (x % (2 * h) != 0, z % (2 * h) != 0);
                let (a, b) = match (ox, oz) {
                    (false, false) => continue,
                    (true, true) => {
                        if (x / (2 * h) + z / (2 * h)) % 2 == 0 {
                            ((x - h, z - h), (x + h, z + h))
                        } else {
                            ((x + h, z - h), (x - h, z + h))
                        }
                    }
                    (true, false) => ((x - h, z), (x + h, z)),
                    (false, true) => ((x, z - h), (x, z + h)),
                };
                out.push((z * 17 + x, a.1 * 17 + a.0, b.1 * 17 + b.0));
            }
        }
        h *= 2;
    }
    out
}

/// One block: raw heights, zones and normals per sample.
struct Block {
    heights: [u16; 289],
    zones: [u8; 289],
    normals: [[u8; 3]; 289],
}

impl Block {
    /// The block's bytes, and its largest error in height steps.
    fn write(&self, curve: &[u32; 256], parents: &[(usize, usize, usize)]) -> (Vec<u8>, u16) {
        let mut steps = [0u8; 289];
        for &(i, a, b) in parents {
            let mid = (u32::from(self.heights[a]) + u32::from(self.heights[b])) as f32 / 2.0;
            let e = (f32::from(self.heights[i]) - mid).abs().ceil() as u32;
            steps[i] = step(curve, e);
        }
        let mut order: Vec<u16> = (0..289).collect();
        order.sort_by_key(|&i| steps[i as usize]);
        let mut first = Vec::with_capacity(0x200);
        for s in 0..256u32 {
            let k = order.iter().take_while(|&&i| u32::from(steps[i as usize]) < s).count() as u16;
            first.extend_from_slice(&k.to_le_bytes());
        }
        let mut sorted = Vec::with_capacity(0x244);
        for i in &order {
            sorted.extend_from_slice(&i.to_le_bytes());
        }
        sorted.extend_from_slice(&0u16.to_le_bytes());
        let mut samples = Vec::with_capacity(0x908);
        for i in 0..289 {
            let n = self.normals[i];
            samples.extend_from_slice(&[self.zones[i], steps[i]]);
            samples.extend_from_slice(&self.heights[i].to_le_bytes());
            samples.extend_from_slice(&[n[0], n[1], n[2], 0]);
        }
        let mut out = Vec::new();
        for part in [first, sorted, samples] {
            packed_block(&mut out, &part);
        }
        out.extend_from_slice(&0u32.to_le_bytes()); // base height
        (out, steps.iter().map(|&s| curve[s as usize]).max().unwrap_or(0).min(65535) as u16)
    }
}

/// A block as the game reads one: its packed size and size, then LZW
/// packed bytes when they come out smaller, else the bytes as they are.
fn packed_block(out: &mut Vec<u8>, data: &[u8]) {
    let packed = crate::lzw::compress(data);
    let put = |out: &mut Vec<u8>, v: u32| out.extend_from_slice(&v.to_le_bytes());
    if packed.len() < data.len() {
        put(out, packed.len() as u32);
        put(out, data.len() as u32);
        out.extend_from_slice(&packed);
    } else {
        put(out, data.len() as u32);
        put(out, data.len() as u32);
        out.extend_from_slice(data);
    }
}

/// A palette of 256 colours (8 reds, 8 greens, 4 blues) and the lookups
/// from 16-bit colours to it, as the shipped tracks carry.
fn palette(out: &mut Vec<u8>) {
    let put = |out: &mut Vec<u8>, v: u32| out.extend_from_slice(&v.to_le_bytes());
    put(out, 0); // first colour
    put(out, 256); // colours
    put(out, 1); // each block is sized; a block its full size is stored raw
    put(out, 0x300);
    let level = |v: u32, of: u32| (v * 255 / (of - 1)) as u8;
    for i in 0..256u32 {
        out.extend_from_slice(&[level(i >> 5, 8), level(i >> 2 & 7, 8), level(i & 3, 4)]);
    }
    put(out, 0x8000);
    for c in 0..0x8000u32 {
        let (r, g, b) = (c >> 10 & 31, c >> 5 & 31, c & 31);
        out.push(((r * 7 + 15) / 31 << 5 | (g * 7 + 15) / 31 << 2 | (b * 3 + 15) / 31) as u8);
    }
    put(out, 0x10000);
    for c in 0..0x10000u32 {
        let (r, g, b) = (c >> 11 & 31, c >> 5 & 63, c & 31);
        out.push(((r * 7 + 15) / 31 << 5 | (g * 7 + 31) / 63 << 2 | (b * 3 + 15) / 31) as u8);
    }
}

impl Build<'_> {
    /// The terrain as the original game wants it (see the module notes).
    /// Reads back with [`crate::trn::parse`] like [`Build::write`]'s.
    pub fn write_original(&self) -> Result<Vec<u8>> {
        let size = self.size;
        let cells_side = (size - 1) / 16;
        if (size - 1) % 16 != 0 || cells_side == 0 || cells_side > 256 {
            return Err(Error::Corrupt("terrain size must be 16n + 1, up to 4097"));
        }
        if self.heights.len() != size * size || self.surfaces.len() != size * size {
            return Err(Error::Corrupt("heights or surfaces do not match the size"));
        }
        let tiles_side = cells_side.div_ceil(4);
        if self.tiles.len() != tiles_side * tiles_side {
            return Err(Error::Corrupt("one texture per 64-unit tile"));
        }
        if self.textures.len() > MOST_TEXTURES {
            return Err(Error::Corrupt("more textures than the original game keeps"));
        }
        let curve = curve();
        let parents = parents();
        // Heights in terrain units, never below 0, kept in 16-bit steps.
        let units: Vec<f32> = self.heights.iter().map(|h| (h / self.scale).max(0.0)).collect();
        let top = units.iter().fold(1.0f32, |m, &h| m.max(h));
        let unit = top / 65535.0;
        let raw = |x: usize, z: usize| (units[z.min(size - 1) * size + x.min(size - 1)] / unit).round().clamp(0.0, 65535.0) as u16;
        let at = |x: usize, z: usize| units[z.min(size - 1) * size + x.min(size - 1)];
        // A block of 17 × 17 samples `stride` units apart from (x0, z0).
        let block = |x0: usize, z0: usize, stride: usize| {
            let mut b = Block { heights: [0; 289], zones: [0; 289], normals: [[0; 3]; 289] };
            for j in 0..289 {
                let (x, z) = (x0 + j % 17 * stride, z0 + j / 17 * stride);
                b.heights[j] = raw(x, z);
                b.zones[j] = self.surfaces[z.min(size - 1) * size + x.min(size - 1)];
                let s = stride as f32;
                let h = |dx: isize, dz: isize| {
                    let x = (x as isize + dx * stride as isize).clamp(0, size as isize - 1) as usize;
                    let z = (z as isize + dz * stride as isize).clamp(0, size as isize - 1) as usize;
                    at(x, z)
                };
                let n = [h(-1, 0) - h(1, 0), 2.0 * s, h(0, -1) - h(0, 1)];
                let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                b.normals[j] = n.map(|v| (v / l * 127.0).round() as i8 as u8);
            }
            b
        };
        // The height range under an area, in terrain units.
        let range = |x0: usize, z0: usize, span: usize| {
            let (mut lo, mut hi) = (f32::MAX, f32::MIN);
            for z in z0..=(z0 + span).min(size - 1) {
                for x in x0..=(x0 + span).min(size - 1) {
                    let h = f32::from(raw(x, z)) * unit;
                    lo = lo.min(h);
                    hi = hi.max(h);
                }
            }
            (lo, hi)
        };

        let depth = if cells_side <= 16 { 1u32 } else { 2 };
        let blocks = cells_side.div_ceil(16);
        let cell_count = cells_side * cells_side + blocks * blocks + usize::from(depth == 2);
        if cell_count > usize::from(u16::MAX) {
            return Err(Error::Corrupt("too many cells"));
        }
        let mut out = Vec::new();
        let put = |out: &mut Vec<u8>, v: u32| out.extend_from_slice(&v.to_le_bytes());
        out.extend_from_slice(MAGIC);
        put(&mut out, 5);
        put(&mut out, 1);
        palette(&mut out);
        put(&mut out, KIND);
        out.extend_from_slice(&self.scale.to_le_bytes());
        put(&mut out, depth);
        let root_at = out.len();
        put(&mut out, 0);
        put(&mut out, cell_count as u32);
        put(&mut out, cell_count as u32);
        let cell_table = out.len();
        out.resize(out.len() + cell_count * 4, 0);
        put(&mut out, self.textures.len() as u32);
        put(&mut out, self.textures.len() as u32);
        let tex_table = out.len();
        out.resize(out.len() + self.textures.len() * 4, 0);

        // Blocks: the leaves' in row order, then each level-1 node's, then
        // the root's; alike ones (flat ground) are stored once, as in the
        // shipped tracks. Each one's largest error goes with its node.
        let mut wanted = Vec::with_capacity(cell_count);
        for cz in 0..cells_side {
            for cx in 0..cells_side {
                wanted.push((cx * 16, cz * 16, 1));
            }
        }
        for bz in 0..blocks {
            for bx in 0..blocks {
                wanted.push((bx * 256, bz * 256, 16));
            }
        }
        if depth == 2 {
            wanted.push((0, 0, 256));
        }
        let made: Vec<(Vec<u8>, u16)> = {
            // A browser has no threads.
            let threads = if cfg!(target_arch = "wasm32") { 1 } else { std::thread::available_parallelism().map_or(1, |n| n.get()).min(16) };
            let per = wanted.len().div_ceil(threads);
            let mut parts: Vec<Vec<(Vec<u8>, u16)>> = Vec::new();
            if threads == 1 {
                parts.push(wanted.iter().map(|&(x, z, s)| block(x, z, s).write(&curve, &parents)).collect());
            } else {
            std::thread::scope(|sc| {
                let jobs: Vec<_> = wanted
                    .chunks(per.max(1))
                    .map(|part| sc.spawn(|| part.iter().map(|&(x, z, s)| block(x, z, s).write(&curve, &parents)).collect::<Vec<_>>()))
                    .collect();
                for j in jobs {
                    parts.push(j.join().unwrap());
                }
            });
            }
            parts.into_iter().flatten().collect()
        };
        let mut seen: std::collections::HashMap<&[u8], u16> = std::collections::HashMap::new();
        let mut stored: Vec<usize> = Vec::new();
        let mut index = Vec::with_capacity(made.len());
        for (k, (bytes, _)) in made.iter().enumerate() {
            let next = stored.len() as u16;
            let at = *seen.entry(bytes.as_slice()).or_insert(next);
            if at == next {
                stored.push(k);
            }
            index.push(at);
        }
        let largest: Vec<u16> = made.iter().map(|m| m.1).collect();
        let cell_count = stored.len();
        // The tables were sized for every block; shrink them to the stored.
        let shrink = (wanted.len() - cell_count) * 4;
        out[cell_table - 8..cell_table - 4].copy_from_slice(&(cell_count as u32).to_le_bytes());
        out[cell_table - 4..cell_table].copy_from_slice(&(cell_count as u32).to_le_bytes());
        out.drain(cell_table + cell_count * 4..cell_table + cell_count * 4 + shrink);
        let tex_table = tex_table - shrink;
        for (i, &k) in stored.iter().enumerate() {
            let at = out.len() as u32;
            out[cell_table + i * 4..cell_table + i * 4 + 4].copy_from_slice(&at.to_le_bytes());
            out.extend_from_slice(&made[k].0);
        }

        for (t, (side, pixels)) in self.textures.iter().enumerate() {
            let side = *side;
            if !side.is_power_of_two() || side > 1024 || pixels.len() != (side * side) as usize {
                return Err(Error::Corrupt("textures must be square powers of two"));
            }
            let at = out.len() as u32;
            out[tex_table + t * 4..tex_table + t * 4 + 4].copy_from_slice(&at.to_le_bytes());
            put(&mut out, side);
            put(&mut out, side);
            put(&mut out, 0x16);
            let mut levels = vec![pixels.clone()];
            while levels.len() <= side.trailing_zeros() as usize {
                let s = (side >> levels.len()) as usize;
                levels.push(crate::trn::halve(levels.last().unwrap(), s, crate::trn::PixelFormat::Rgb555));
            }
            levels.reverse();
            let table = out.len();
            out.resize(table + (levels.len() + 1) * 4, 0);
            let mut starts = Vec::new();
            for level in &levels {
                starts.push(out.len());
                let bytes: Vec<u8> = level.iter().flat_map(|p| p.to_le_bytes()).collect();
                // A level is read as packed unless it is exactly its raw size.
                let packed = crate::lzw::compress(&bytes);
                if packed.len() < bytes.len() {
                    out.extend_from_slice(&packed);
                } else {
                    out.extend_from_slice(&bytes);
                }
            }
            starts.push(out.len());
            for (i, s) in starts.iter().enumerate() {
                let field = table + i * 4;
                out[field..field + 4].copy_from_slice(&((s - field) as u32).to_le_bytes());
            }
        }

        // The tree.
        let node = |out: &mut Vec<u8>, cell: usize, (lo, hi): (f32, f32), textures: [u32; 16]| -> usize {
            out.extend_from_slice(&1.0f32.to_le_bytes());
            out.extend_from_slice(&unit.to_le_bytes());
            out.extend_from_slice(&lo.to_le_bytes());
            out.extend_from_slice(&hi.to_le_bytes());
            out.extend_from_slice(&largest[cell].to_le_bytes());
            out.extend_from_slice(&index[cell].to_le_bytes());
            out.extend_from_slice(&NONE.to_le_bytes());
            for t in textures {
                out.extend_from_slice(&t.to_le_bytes());
            }
            out.extend_from_slice(&0u32.to_le_bytes());
            out.push(NODE_BYTE);
            let link = out.len();
            out.extend_from_slice(&0u32.to_le_bytes());
            link
        };
        let none = [NONE; 16];
        let level1 = |out: &mut Vec<u8>, bx: usize, bz: usize| -> u32 {
            let mut textures = none;
            for (i, t) in textures.iter_mut().enumerate() {
                let (tx, tz) = (bx * 4 + i % 4, bz * 4 + i / 4);
                if tx < tiles_side && tz < tiles_side {
                    if let Some(k) = self.tiles[tz * tiles_side + tx] {
                        *t = u32::from(k) * 2;
                    }
                }
            }
            let start = out.len() as u32;
            let own = cells_side * cells_side + bz * blocks + bx;
            let link = node(out, own, range(bx * 256, bz * 256, 256), textures);
            let mut children = vec![0u32; 256];
            for (j, c) in children.iter_mut().enumerate() {
                let (cx, cz) = (bx * 16 + j % 16, bz * 16 + j / 16);
                if cx < cells_side && cz < cells_side {
                    *c = out.len() as u32;
                    node(out, cz * cells_side + cx, range(cx * 16, cz * 16, 16), none);
                }
            }
            let table = out.len() as u32;
            out[link..link + 4].copy_from_slice(&table.to_le_bytes());
            for c in children {
                out.extend_from_slice(&c.to_le_bytes());
            }
            start
        };
        let root = if depth == 1 {
            level1(&mut out, 0, 0)
        } else {
            // The game reads the root straight after the textures (the
            // header's offset to it goes unread), the rest by offsets.
            let start = out.len() as u32;
            let link = node(&mut out, wanted.len() - 1, range(0, 0, size - 1), none);
            let mut children = vec![0u32; 256];
            for bz in 0..blocks {
                for bx in 0..blocks {
                    children[bz * 16 + bx] = level1(&mut out, bx, bz);
                }
            }
            let table = out.len() as u32;
            out[link..link + 4].copy_from_slice(&table.to_le_bytes());
            for c in children {
                out.extend_from_slice(&c.to_le_bytes());
            }
            start
        };
        out[root_at..root_at + 4].copy_from_slice(&root.to_le_bytes());
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The curve matches the shipped files' steps (values read from
    /// Nat01.trn blocks).
    #[test]
    fn curve_steps() {
        let c = curve();
        assert_eq!(c[0], 0);
        assert_eq!(step(&c, 0), 0);
        assert_eq!(c[219], 11751);
        assert_eq!(c[236], 25631);
        assert_eq!(c[244], 37726);
    }

    /// A rolling terrain reads back the same, with its errors sorted.
    #[test]
    fn reads_back() {
        for cells in [4usize, 20] {
            let size = cells * 16 + 1;
            let heights: Vec<f32> = (0..size * size)
                .map(|i| ((i % size) as f32 * 0.05).sin() * 20.0 + 30.0 + (i / size) as f32 * 0.1)
                .collect();
            let surfaces: Vec<u8> = (0..size * size).map(|i| (i % 3) as u8).collect();
            let textures = vec![(256u32, vec![0x7c00u16; 256 * 256]), (256, vec![0x03e0; 256 * 256])];
            let tiles_side = cells.div_ceil(4);
            let tiles: Vec<Option<u16>> = (0..tiles_side * tiles_side).map(|i| Some((i % 2) as u16)).collect();
            let b = Build { size, heights: &heights, surfaces: &surfaces, scale: 3.0, textures: &textures, tiles: &tiles };
            let bytes = b.write_original().unwrap();
            let t = crate::trn::parse(&bytes).unwrap();
            assert_eq!(t.size, size);
            for (a, h) in t.heights.iter().zip(&heights) {
                assert!((a * 3.0 - h).abs() < 0.05, "{a} vs {h}");
            }
            assert_eq!(t.surfaces, surfaces);
            assert_eq!(t.tiles.len(), tiles.len());
            assert_eq!(game_root(&bytes), root_field(&bytes), "the root must follow the textures");
        }
    }

    fn u(b: &[u8], o: usize) -> usize {
        u32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as usize
    }

    /// Where the header says the root is.
    fn root_field(b: &[u8]) -> usize {
        u(b, palette_end(b) + 12)
    }

    fn palette_end(b: &[u8]) -> usize {
        let mut o = 12;
        if u(b, 8) != 0 {
            let sized = u(b, o + 8) != 0;
            o += 12;
            for len in [0x300, 0x8000, 0x10000] {
                o += if sized { 4 + u(b, o) } else { len };
            }
        }
        o
    }

    /// Where the game reads the root: straight after the blocks and the
    /// textures, each read in turn (Terrain::Load), whatever the tables say.
    fn game_root(b: &[u8]) -> usize {
        let mut o = palette_end(b) + 16;
        let blocks = u(b, o + 4);
        o += 8 + u(b, o) * 4;
        let textures = u(b, o + 4);
        o += 8 + u(b, o) * 4;
        for _ in 0..blocks {
            for _ in 0..3 {
                o += 8 + u(b, o);
            }
            o += 4;
        }
        for _ in 0..textures {
            let w = u(b, o);
            o += 12;
            let levels = (w as u32).trailing_zeros() as usize + 1;
            let end = o + levels * 4;
            o = end + u(b, end);
        }
        o
    }
}
