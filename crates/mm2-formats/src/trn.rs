//! `.trn` track terrain, found inside each `TERAFORM/**/*.ENV` archive.
//!
//! Recovered from `Terrain.cpp`, `Gridbase.cpp`, `Griddraw.cpp` and
//! `Grid1.cpp` in the game. All integers are little-endian.
//!
//! ```text
//! "TRN\0", u32 version (5)
//! u32 has_palette
//!     u32, u32 colours, u32 packed
//!     palette (0x300), colour lookup (0x8000), colour lookup (0x10000)
//! u32, f32 scale, u32 depth, u32 root_offset
//! u32 n, u32 cell_count, u32 cell_offsets[n]
//! u32 n, u32 texture_count, u32 texture_offsets[n]
//! cells[cell_count]        // 17×17 height samples each
//! textures[texture_count]  // RGB555 with a full mip chain
//! quadtree                 // from root_offset; places cells and textures
//! ```
//!
//! The quadtree has `depth + 1` levels. A node at level `d` covers
//! `16 << (4 * d)` terrain units and has up to 16×16 children. Leaves
//! (level 0) each point at a cell, which gives the 17×17 heights of its
//! 16×16-unit square; cells are shared between leaves. A leaf's height
//! is `((h - base) * scale + base) * unit`, with `h` the sample's u16,
//! `base` its cell's base height and `scale`, `unit` from the leaf, so a
//! leaf can reuse another's cell turned upside down and flattened. Level-1
//! nodes split their area into 4×4 texture tiles.

use crate::lzw;
use crate::reader::Reader;
use crate::{Error, Result};

pub const MAGIC: &[u8] = b"TRN\0";

/// A decoded track terrain.
#[derive(Debug, Clone)]
pub struct Terrain {
    /// Height samples per side; the terrain spans `size - 1` units.
    pub size: usize,
    /// Heights in terrain units, row-major with rows along +z.
    pub heights: Vec<f32>,
    /// Surface normals, one per height sample (x, y up, z).
    pub normals: Vec<[i8; 3]>,
    /// Surface type per height sample (the record's first byte, 0x484890):
    /// which `[TerrainZone]` of the scene the ground is.
    pub surfaces: Vec<u8>,
    /// World units per terrain unit.
    pub scale: f32,
    /// The header word the game keeps at Terrain+0xbe8: 1 on most
    /// tracks; anything else lowers the bike's nose-over limit
    /// (Bike::UpdateCrashState) and the camera's follow rate.
    pub kind: u32,
    pub textures: Vec<Texture>,
    /// Where each texture is drawn.
    pub tiles: Vec<Tile>,
}

impl Terrain {
    pub fn height(&self, x: usize, z: usize) -> f32 {
        self.heights[z * self.size + x]
    }
}

/// A square of terrain covered by one texture, in terrain units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tile {
    pub x: u32,
    pub z: u32,
    pub size: u32,
    pub texture: u16,
}

/// How a texture stores each 16-bit pixel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// 5 bits each of red, green and blue.
    Rgb555,
    /// 4 bits each of alpha, red, green and blue.
    Argb4444,
    /// 5 bits of red, 6 of green, 5 of blue.
    Rgb565,
    /// 1 bit of alpha, then 5 bits each of red, green and blue.
    Argb1555,
}

impl PixelFormat {
    /// The shipped files' mip-chain codes.
    pub fn from_code(code: u32) -> Option<Self> {
        Stored::from_code(code).filter(|s| s.mipped && s.bytes == 2).map(|s| s.format)
    }
}

/// What one of the game's texture format codes says about the stored
/// pixels (0x516e20 bytes, 0x516ee0 packed, 0x516f30 mip chain, 0x516f80
/// the pixel layout). The shipped tracks use 22 (RGB555 mip chain); track
/// makers' tools also wrote others, such as 3 (one raw RGB565 level).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stored {
    /// The 16-bit layout the pixels are kept in once read.
    pub format: PixelFormat,
    /// Bytes per stored pixel: 1 (grey), 2, 3 (BGR) or 4 (BGRA).
    pub bytes: usize,
    /// Whether the file holds a mip chain, or one level.
    pub mipped: bool,
    /// Whether a level may be LZW packed.
    pub packed: bool,
}

impl Stored {
    pub fn from_code(code: u32) -> Option<Self> {
        let (format, bytes) = match code {
            1 | 9 | 0x10 | 0x18 => (PixelFormat::Rgb565, 3),
            2 | 5 | 7 | 0x11 | 0x14 | 0x16 => (PixelFormat::Rgb555, 2),
            3 | 8 | 0x12 | 0x17 => (PixelFormat::Rgb565, 2),
            4 | 6 | 0x13 | 0x15 => (PixelFormat::Rgb565, 1),
            10 | 0xd | 0x19 | 0x1c | 0x1f | 0x21 => (PixelFormat::Argb1555, 2),
            0xb | 0xe | 0x1a | 0x1d => (PixelFormat::Argb4444, 2),
            0xc | 0xf | 0x1b | 0x1e | 0x20 | 0x22 => (PixelFormat::Argb4444, 4),
            _ => return None,
        };
        Some(Stored {
            format,
            bytes,
            mipped: matches!(code, 0x10..=0x1e | 0x21 | 0x22),
            packed: matches!(code, 5..=0xc | 0x14..=0x1b | 0x1f..=0x22),
        })
    }

    /// Turns stored pixels into 16-bit ones; the 1-, 3- and 4-byte
    /// layouts lose their low bits. One-byte pixels are grey, or with a
    /// palette (an early terrain's, see [`parse`]) its colours' indices.
    fn pixels(&self, data: &[u8], palette: Option<&[u8]>) -> Vec<u16> {
        match self.bytes {
            1 if palette.is_some() => {
                let pal = palette.unwrap();
                data.iter()
                    .map(|&c| {
                        let p = &pal[c as usize * 3..c as usize * 3 + 3];
                        u16::from(p[0] >> 3) << 11 | u16::from(p[1] >> 2) << 5 | u16::from(p[2] >> 3)
                    })
                    .collect()
            }
            1 => data
                .iter()
                .map(|&c| u16::from(c >> 3) << 11 | u16::from(c >> 2) << 5 | u16::from(c >> 3))
                .collect(),
            3 => data
                .chunks_exact(3)
                .map(|p| u16::from(p[2] >> 3) << 11 | u16::from(p[1] >> 2) << 5 | u16::from(p[0] >> 3))
                .collect(),
            4 => data
                .chunks_exact(4)
                .map(|p| {
                    u16::from(p[3] >> 4) << 12 | u16::from(p[2] >> 4) << 8 | u16::from(p[1] >> 4) << 4 | u16::from(p[0] >> 4)
                })
                .collect(),
            _ => data
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect(),
        }
    }
}

/// A square 16-bit texture with its mip chain.
#[derive(Debug, Clone)]
pub struct Texture {
    pub size: u32,
    pub format: PixelFormat,
    /// Mip levels, largest first. Level `i` is `size >> i` pixels square.
    pub levels: Vec<Vec<u16>>,
}

impl Texture {
    /// Converts one mip level to RGBA8.
    pub fn rgba8(&self, level: usize) -> Vec<u8> {
        let format = self.format;
        self.levels[level]
            .iter()
            .flat_map(move |&p| match format {
                PixelFormat::Rgb555 => {
                    let c = |shift: u16| {
                        let v = (p >> shift & 31) as u8;
                        v << 3 | v >> 2
                    };
                    [c(10), c(5), c(0), 255]
                }
                PixelFormat::Argb4444 => {
                    let c = |shift: u16| (p >> shift & 15) as u8 * 17;
                    [c(8), c(4), c(0), c(12)]
                }
                PixelFormat::Rgb565 => {
                    let five = |v: u16| (v as u8) << 3 | (v as u8) >> 2;
                    let g = (p >> 5 & 63) as u8;
                    [five(p >> 11 & 31), g << 2 | g >> 4, five(p & 31), 255]
                }
                PixelFormat::Argb1555 => {
                    let c = |shift: u16| {
                        let v = (p >> shift & 31) as u8;
                        v << 3 | v >> 2
                    };
                    [c(10), c(5), c(0), if p & 0x8000 != 0 { 255 } else { 0 }]
                }
            })
            .collect()
    }

    /// Whether any pixel is less than fully opaque.
    pub fn has_alpha(&self) -> bool {
        match self.format {
            PixelFormat::Argb4444 => self.levels[0].iter().any(|&p| p >> 12 != 15),
            PixelFormat::Argb1555 => self.levels[0].iter().any(|&p| p & 0x8000 == 0),
            _ => false,
        }
    }
}

const CELL_SAMPLES: usize = 17;
const CELL_BYTES: usize = CELL_SAMPLES * CELL_SAMPLES * 8;
/// Child slots at or above this value mean "no texture".
const NO_TEXTURE: u32 = 0xfffe;

/// Parses a `.trn` file.
pub fn parse(bytes: &[u8]) -> Result<Terrain> {
    if !bytes.starts_with(MAGIC) {
        return Err(Error::BadMagic);
    }
    let mut r = Reader::new(bytes, MAGIC.len());
    if r.u32("version")? != 5 {
        return Err(Error::Corrupt("unsupported TRN version"));
    }
    // The demo's terrains keep their textures as indices into a palette
    // of 256 colours; the shipped game's are all 16-bit.
    let palette = if r.u32("palette flag")? != 0 { read_palette(&mut r)? } else { None };
    let kind = r.u32("kind")?;
    let scale = r.f32("scale")?;
    let depth = r.u32("depth")?;
    let root_offset = r.u32("root offset")? as usize;
    if !(1..=3).contains(&depth) {
        return Err(Error::Corrupt("TRN tree depth out of range"));
    }

    let offsets = r.u32("cell offset count")? as usize;
    let cell_count = r.u32("cell count")? as usize;
    r.take(offsets * 4, "cell offsets")?;
    let offsets = r.u32("texture offset count")? as usize;
    let texture_count = r.u32("texture count")? as usize;
    let texture_offsets: Vec<usize> = (0..offsets)
        .map(|_| r.u32("texture offset").map(|o| o as usize))
        .collect::<Result<_>>()?;
    if texture_offsets.len() < texture_count {
        return Err(Error::Corrupt("fewer texture offsets than textures"));
    }

    let mut raw_cells = Vec::with_capacity(cell_count.min(bytes.len() / 16));
    let mut packed_work = 0usize;
    for _ in 0..cell_count {
        let first = block_raw(&mut r, 0x200)?;
        let second = block_raw(&mut r, 0x244)?;
        let samples = block_raw(&mut r, CELL_BYTES)?;
        packed_work += [&first, &second, &samples]
            .iter()
            .filter(|b| b.packed < b.unpacked)
            .map(|b| b.len)
            .sum::<usize>();
        raw_cells.push((first, second, samples, r.u32("cell base height")? as f32));
    }
    // The three blocks of a cell decode independently of every other
    // cell; the first two blocks' contents are not used, but they are
    // still decoded, so a corrupt one fails exactly as before. Packed
    // blocks are the slow part of a track load, so they are decoded in
    // parallel when there is enough of that work; raw blocks are a
    // memcpy, and threading those costs more than it saves.
    let decode = |&(a, b, samples, base): &(RawBlock<'_>, RawBlock<'_>, RawBlock<'_>, f32)| {
        block_decode(&a)?;
        block_decode(&b)?;
        Ok((block_decode(&samples)?, base))
    };
    let cells: Vec<(Vec<u8>, f32)> = if packed_work >= 65_536 {
        map_parallel(&raw_cells, &decode)?
    } else {
        raw_cells.iter().map(decode).collect::<Result<_>>()?
    };

    let textures = read_textures(bytes, &texture_offsets[..texture_count], root_offset, palette.as_deref())?;

    let mut tree = Tree {
        bytes,
        nodes: Vec::new(),
    };
    tree.read(root_offset, depth, 0, 0)?;

    // The root's nominal area is larger than the track; size to the leaves.
    let size = tree
        .nodes
        .iter()
        .filter(|n| n.level == 0)
        .map(|n| n.x.max(n.z) as usize + CELL_SAMPLES)
        .max()
        .ok_or(Error::Corrupt("terrain has no leaves"))?;
    let mut heights = vec![0.0; size * size];
    let mut normals = vec![[0, 127, 0]; size * size];
    let mut surfaces = vec![0u8; size * size];
    let mut tiles = Vec::new();
    for node in &tree.nodes {
        let span = 16u32 << (4 * node.level);
        let (x0, z0) = (node.x * span / 16, node.z * span / 16);
        if node.level == 1 {
            let tile = span / 4;
            for (i, &t) in node.textures.iter().enumerate() {
                // Slots hold twice the texture index.
                if t & NO_TEXTURE != NO_TEXTURE && ((t >> 1) as usize) < textures.len() {
                    let i = i as u32;
                    tiles.push(Tile {
                        x: x0 + i % 4 * tile,
                        z: z0 + i / 4 * tile,
                        size: tile,
                        texture: (t >> 1) as u16,
                    });
                }
            }
        }
        if node.level != 0 {
            continue;
        }
        let (cell, base) = cells
            .get(node.cell as usize)
            .ok_or(Error::Corrupt("leaf points past the cells"))?;
        // A leaf may borrow another leaf's samples, flipped and squashed
        // about the cell's base height by its own scale (0x484890): the
        // far, untextured hills are filled this way.
        let k = node.scale;
        for (i, s) in cell.chunks_exact(8).enumerate() {
            let (x, z) = (
                x0 as usize + i % CELL_SAMPLES,
                z0 as usize + i / CELL_SAMPLES,
            );
            if x >= size || z >= size {
                return Err(Error::Corrupt("leaf lies outside the terrain"));
            }
            let raw = u16::from_le_bytes([s[2], s[3]]) as f32;
            heights[z * size + x] = ((raw - base) * k + base) * node.unit;
            surfaces[z * size + x] = s[0];
            normals[z * size + x] = if k == 1.0 {
                [s[4] as i8, s[5] as i8, s[6] as i8]
            } else {
                // The slope scales with the heights; keep the direction.
                let n = [s[4] as i8 as f32 * k, s[5] as i8 as f32, s[6] as i8 as f32 * k];
                let big = n.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1.0);
                n.map(|v| (v * 127.0 / big).round() as i8)
            };
        }
    }
    Ok(Terrain {
        kind,
        size,
        heights,
        normals,
        surfaces,
        scale,
        textures,
        tiles,
    })
}

/// Decodes independent items in order, in parallel when there are
/// enough of them and more than one thread, else one by one. The
/// first error by index is returned, so a corrupt file fails exactly
/// as the sequential read would. Threads are only spawned when more
/// than one is available, so targets without threads stay sequential.
fn map_parallel<'a, I, T, F>(items: &'a [I], f: &'a F) -> Result<Vec<T>>
where
    I: Sync,
    T: Send,
    F: Fn(&I) -> Result<T> + Sync,
{
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    if threads <= 1 || items.len() < 8 {
        return items.iter().map(f).collect();
    }
    let chunk = items.len().div_ceil(threads.min(items.len()));
    let mut parts: Vec<Vec<Result<T>>> = Vec::new();
    std::thread::scope(|s| {
        let mut handles = Vec::new();
        for part in items.chunks(chunk) {
            handles.push(s.spawn(move || part.iter().map(f).collect::<Vec<_>>()));
        }
        for h in handles {
            parts.push(h.join().unwrap());
        }
    });
    let mut out = Vec::with_capacity(items.len());
    for part in parts {
        for r in part {
            out.push(r?);
        }
    }
    Ok(out)
}

/// One stored block: its packed bytes as they lie in the file, and the
/// sizes its header claims, so the decode can run anywhere.
#[derive(Clone, Copy)]
struct RawBlock<'a> {
    data: &'a [u8],
    packed: usize,
    unpacked: usize,
    len: usize,
}

/// Reads a block's header and bytes, without decoding them.
fn block_raw<'a>(r: &mut Reader<'a>, len: usize) -> Result<RawBlock<'a>> {
    let packed = r.u32("block packed size")? as usize;
    let unpacked = r.u32("block size")? as usize;
    if packed >= unpacked && unpacked != len {
        return Err(Error::Corrupt("block has an unexpected size"));
    }
    let data = if packed < unpacked {
        r.take(packed, "packed block")?
    } else {
        r.take(len, "block")?
    };
    Ok(RawBlock {
        data,
        packed,
        unpacked,
        len,
    })
}

/// Decodes a block that is stored raw or LZW-packed with a size prefix pair.
fn block_decode(b: &RawBlock) -> Result<Vec<u8>> {
    if b.packed < b.unpacked {
        lzw::decompress(b.data, b.len)
    } else if b.unpacked == b.len {
        Ok(b.data.to_vec())
    } else {
        Err(Error::Corrupt("block has an unexpected size"))
    }
}

/// Reads the textures, in parallel when their offsets run in order:
/// a texture's bytes run to the next texture's offset, or to the
/// quadtree for the last one. Any failure there falls back to the
/// sequential read of the whole file, so an odd layout still loads
/// exactly as it did.
fn read_textures(
    bytes: &[u8],
    offsets: &[usize],
    root_offset: usize,
    palette: Option<&[u8]>,
) -> Result<Vec<Texture>> {
    if !offsets.is_empty()
        && offsets.windows(2).all(|w| w[0] < w[1])
        && offsets.iter().all(|&o| o < bytes.len())
    {
        let ends: Vec<usize> = offsets
            .iter()
            .enumerate()
            .map(|(i, _)| offsets.get(i + 1).copied().unwrap_or(root_offset))
            .collect();
        if ends.iter().zip(offsets).all(|(&e, &o)| e > o && e <= bytes.len()) {
            let items: Vec<(usize, usize)> = offsets.iter().copied().zip(ends).collect();
            if let Ok(textures) = map_parallel(&items, &|&(start, end)| {
                read_texture(&mut Reader::new(&bytes[start..end], 0), palette)
            }) {
                return Ok(textures);
            }
        }
    }
    offsets
        .iter()
        .map(|&o| read_texture(&mut Reader::new(bytes, o), palette))
        .collect()
}

/// The palette block: 256 RGB colours (8 bits each), then two colour
/// lookups the game draws with and the port doesn't need. Each block is
/// LZW packed (its own length first) when the packing flag is set.
fn read_palette(r: &mut Reader) -> Result<Option<Vec<u8>>> {
    r.u32("palette start")?;
    r.u32("palette colours")?;
    let packed = r.u32("palette packing")? != 0;
    let mut colours = None;
    for len in [0x300, 0x8000, 0x10000] {
        let block = if packed {
            // The length word, then that many packed bytes; the
            // decompressor wants the word in front of them.
            let size = r.u32("palette block size")?;
            let raw = r.take(size as usize, "palette block")?;
            if len == 0x300 && raw.len() == len {
                Some(raw.to_vec())
            } else if len == 0x300 {
                let stream: Vec<u8> = size.to_be_bytes().into_iter().chain(raw.iter().copied()).collect();
                lzw::decompress(&stream, len).ok()
            } else {
                None
            }
        } else {
            let raw = r.take(len, "palette block")?;
            (len == 0x300).then(|| raw.to_vec())
        };
        if len == 0x300 {
            colours = block.filter(|b| b.len() >= 0x300);
        }
    }
    Ok(colours)
}

fn read_texture(r: &mut Reader, palette: Option<&[u8]>) -> Result<Texture> {
    let size = r.u32("texture width")?;
    let height = r.u32("texture height")?;
    let code = r.u32("texture format")?;
    if size != height {
        return Err(Error::Corrupt("texture is not square"));
    }
    let stored = Stored::from_code(code).ok_or(Error::Corrupt("unknown terrain texture format"))?;
    read_image_in(r, size, stored, palette)
}

/// Reads a texture as its format code says (0x4ca7d0): a mip chain, or
/// one level with the smaller ones made from it.
pub(crate) fn read_image(r: &mut Reader, size: u32, stored: Stored) -> Result<Texture> {
    read_image_in(r, size, stored, None)
}

fn read_image_in(r: &mut Reader, size: u32, stored: Stored, palette: Option<&[u8]>) -> Result<Texture> {
    if stored.mipped {
        return read_stored_mips(r, size, stored, palette);
    }
    if !size.is_power_of_two() || size > 1024 {
        return Err(Error::Corrupt("texture size is not a power of two"));
    }
    let len = (size * size) as usize * stored.bytes;
    let data = if stored.packed {
        // The packed stream starts with its own length.
        let start = r.pos;
        let packed = r.u32("packed level size")? as usize;
        r.seek(start, "packed level")?;
        lzw::decompress(r.take(packed.max(4), "packed level")?, len)?
    } else {
        r.take(len, "texture level")?.to_vec()
    };
    let mut levels = vec![stored.pixels(&data, palette)];
    while levels.len() <= size.trailing_zeros() as usize {
        let side = (size >> levels.len()) as usize;
        levels.push(halve(levels.last().unwrap(), side, stored.format));
    }
    Ok(Texture {
        size,
        format: stored.format,
        levels,
    })
}

/// A level half the size of `level`, each pixel the mean of four.
pub(crate) fn halve(level: &[u16], side: usize, format: PixelFormat) -> Vec<u16> {
    // Each layout's fields: shift and bit count.
    let fields: &[(u16, u16)] = match format {
        PixelFormat::Rgb555 => &[(10, 5), (5, 5), (0, 5)],
        PixelFormat::Argb4444 => &[(12, 4), (8, 4), (4, 4), (0, 4)],
        PixelFormat::Rgb565 => &[(11, 5), (5, 6), (0, 5)],
        PixelFormat::Argb1555 => &[(15, 1), (10, 5), (5, 5), (0, 5)],
    };
    let wide = side * 2;
    let mut out = Vec::with_capacity(side * side);
    for y in 0..side {
        for x in 0..side {
            let four = [
                level[2 * y * wide + 2 * x],
                level[2 * y * wide + 2 * x + 1],
                level[(2 * y + 1) * wide + 2 * x],
                level[(2 * y + 1) * wide + 2 * x + 1],
            ];
            let mut p = 0u16;
            for &(shift, bits) in fields {
                let mask = (1u16 << bits) - 1;
                let sum: u16 = four.iter().map(|q| q >> shift & mask).sum();
                p |= ((sum + 2) / 4).min(mask) << shift;
            }
            out.push(p);
        }
    }
    out
}

/// Reads a mip chain: one offset per level from 1×1 up to size×size, plus
/// the end, each relative to where it is stored. A level is LZW packed
/// unless its length is exactly the raw size.
pub(crate) fn read_mips(r: &mut Reader, size: u32, format: PixelFormat) -> Result<Texture> {
    let stored = Stored {
        format,
        bytes: 2,
        mipped: true,
        packed: true,
    };
    read_stored_mips(r, size, stored, None)
}

fn read_stored_mips(r: &mut Reader, size: u32, stored: Stored, palette: Option<&[u8]>) -> Result<Texture> {
    if !size.is_power_of_two() || size > 1024 {
        return Err(Error::Corrupt("texture size is not a power of two"));
    }
    let level_count = size.trailing_zeros() as usize + 1;
    let mut bounds = Vec::with_capacity(level_count + 1);
    for _ in 0..=level_count {
        let at = r.pos;
        bounds.push(at + r.u32("mip offset")? as usize);
    }
    let mut levels = Vec::with_capacity(level_count);
    for (i, w) in bounds.windows(2).enumerate() {
        let side = 1usize << i;
        let len = side * side * stored.bytes;
        r.seek(w[0], "mip level")?;
        let src = r.take(w[1].saturating_sub(w[0]), "mip level")?;
        let data = if src.len() == len || !stored.packed {
            let mut raw = src.to_vec();
            raw.resize(len, 0);
            raw
        } else {
            lzw::decompress(src, len)?
        };
        levels.push(stored.pixels(&data, palette));
    }
    levels.reverse();
    Ok(Texture {
        size,
        format: stored.format,
        levels,
    })
}

struct Node {
    level: u32,
    /// Position in units of this node's children.
    x: u32,
    z: u32,
    scale: f32,
    unit: f32,
    cell: u16,
    textures: [u32; 16],
}

struct Tree<'a> {
    bytes: &'a [u8],
    nodes: Vec<Node>,
}

impl Tree<'_> {
    fn read(&mut self, offset: usize, level: u32, x: u32, z: u32) -> Result<()> {
        let mut r = Reader::new(self.bytes, offset);
        // Heights below this node are scaled by the first and turned into
        // terrain units by the second (0x484890, node +0x10 and +0x14).
        let scale = r.f32("node height scale")?;
        let unit = r.f32("node height unit")?;
        let _min_height = r.f32("node min height")?;
        let _max_height = r.f32("node max height")?;
        r.u16("node flags")?;
        let cell = r.u16("node cell")?;
        let has_children = r.u32("node child flag")? != 0;
        let mut textures = [0; 16];
        for t in &mut textures {
            *t = r.u32("node texture")?;
        }
        let extra = r.u32("node extra size")? as usize;
        r.take(extra, "node extra")?;
        r.u8("node byte")?;
        let children = r.u32("node children offset")? as usize;
        self.nodes.push(Node {
            level,
            x,
            z,
            scale,
            unit,
            cell,
            textures,
        });

        if level == 0 || !has_children || children == 0 {
            return Ok(());
        }
        let mut r = Reader::new(self.bytes, children);
        let table = r.take(256 * 4, "node children")?;
        for (i, b) in table.chunks_exact(4).enumerate() {
            let child = u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize;
            if child != 0 {
                let (cx, cz) = ((x + i as u32 % 16) * 16, (z + i as u32 / 16) * 16);
                self.read(child, level - 1, cx, cz)?;
            }
        }
        Ok(())
    }
}

/// A terrain to write: our own heights and textures in the game's `.trn`
/// layout, so player-made and free-pack tracks load like any other.
pub struct Build<'a> {
    /// Height samples per side, `16 × n + 1` with `n` from 1 to 256.
    pub size: usize,
    /// Heights in world units (feet), row-major with rows along +z.
    pub heights: &'a [f32],
    /// Surface zone per sample (0 to 7, the scene's `[TerrainZoneN]` - 1).
    pub surfaces: &'a [u8],
    /// World units per terrain unit (the shipped tracks use 3).
    pub scale: f32,
    /// Square RGB555 textures, each `side × side` (a power of two).
    pub textures: &'a [(u32, Vec<u16>)],
    /// The texture of each 64-unit tile, row-major along +z, or `None`.
    pub tiles: &'a [Option<u16>],
}

/// Heights are stored as whole steps of this many terrain units.
pub(crate) const WRITE_UNIT: f32 = 1.0 / 128.0;

impl Build<'_> {
    pub fn write(&self) -> Result<Vec<u8>> {
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
        let unit_h = |x: usize, z: usize| self.heights[z * size + x] / self.scale;
        // Normals from the neighbouring samples, in terrain units.
        let normal = |x: usize, z: usize| {
            let h = |x: isize, z: isize| unit_h(x.clamp(0, size as isize - 1) as usize, z.clamp(0, size as isize - 1) as usize);
            let (x, z) = (x as isize, z as isize);
            let n = [h(x - 1, z) - h(x + 1, z), 2.0, h(x, z - 1) - h(x, z + 1)];
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            n.map(|v| (v / l * 127.0).round() as i8 as u8)
        };

        let mut out = Vec::new();
        let put = |out: &mut Vec<u8>, v: u32| out.extend_from_slice(&v.to_le_bytes());
        out.extend_from_slice(MAGIC);
        put(&mut out, 5);
        put(&mut out, 0); // no palette
        put(&mut out, 0);
        out.extend_from_slice(&self.scale.to_le_bytes());
        let depth = if cells_side <= 16 { 1 } else { 2 };
        put(&mut out, depth);
        let root_at = out.len();
        put(&mut out, 0); // root offset, filled in below
        let cell_count = cells_side * cells_side;
        put(&mut out, cell_count as u32);
        put(&mut out, cell_count as u32);
        let cell_table = out.len();
        out.resize(out.len() + cell_count * 4, 0);
        put(&mut out, self.textures.len() as u32);
        put(&mut out, self.textures.len() as u32);
        let tex_table = out.len();
        out.resize(out.len() + self.textures.len() * 4, 0);

        // Cells, one per 16-unit square, in row order.
        for cz in 0..cells_side {
            for cx in 0..cells_side {
                let at = out.len() as u32;
                let i = cz * cells_side + cx;
                out[cell_table + i * 4..cell_table + i * 4 + 4].copy_from_slice(&at.to_le_bytes());
                for len in [0x200u32, 0x244] {
                    put(&mut out, len);
                    put(&mut out, len);
                    out.resize(out.len() + len as usize, 0);
                }
                put(&mut out, CELL_BYTES as u32);
                put(&mut out, CELL_BYTES as u32);
                for j in 0..CELL_SAMPLES * CELL_SAMPLES {
                    let (x, z) = (cx * 16 + j % CELL_SAMPLES, cz * 16 + j / CELL_SAMPLES);
                    let raw = (unit_h(x, z) / WRITE_UNIT).round().clamp(0.0, 65535.0) as u16;
                    let n = normal(x, z);
                    out.extend_from_slice(&[self.surfaces[z * size + x], 0]);
                    out.extend_from_slice(&raw.to_le_bytes());
                    out.extend_from_slice(&[n[0], n[1], n[2], 0]);
                }
                put(&mut out, 0); // base height
            }
        }

        // Textures: a mip chain of RGB555, smallest level first, each
        // offset counted from where it is stored.
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
                levels.push(halve(levels.last().unwrap(), s, PixelFormat::Rgb555));
            }
            levels.reverse();
            let table = out.len();
            out.resize(table + (levels.len() + 1) * 4, 0);
            let mut starts = Vec::new();
            for level in &levels {
                starts.push(out.len());
                for p in level {
                    out.extend_from_slice(&p.to_le_bytes());
                }
            }
            starts.push(out.len());
            for (i, s) in starts.iter().enumerate() {
                let field = table + i * 4;
                out[field..field + 4].copy_from_slice(&((s - field) as u32).to_le_bytes());
            }
        }

        // The tree: a root, then level-1 nodes with 4×4 texture tiles, then
        // leaves pointing at the cells.
        let node = |out: &mut Vec<u8>, cell: u16, children: bool, textures: [u32; 16]| -> usize {
            out.extend_from_slice(&1.0f32.to_le_bytes());
            out.extend_from_slice(&WRITE_UNIT.to_le_bytes());
            out.extend_from_slice(&0.0f32.to_le_bytes());
            out.extend_from_slice(&0.0f32.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&cell.to_le_bytes());
            out.extend_from_slice(&u32::from(children).to_le_bytes());
            for t in textures {
                out.extend_from_slice(&t.to_le_bytes());
            }
            out.extend_from_slice(&0u32.to_le_bytes());
            out.push(0);
            let at = out.len();
            out.extend_from_slice(&0u32.to_le_bytes());
            at
        };
        let none = [0xffff_fffeu32; 16];
        let write_level1 = |out: &mut Vec<u8>, bx: usize, bz: usize| -> u32 {
            // Block (bx, bz) of 16×16 cells, 256 terrain units.
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
            let link = node(out, 0, true, textures);
            let mut children = vec![0u32; 256];
            for (j, c) in children.iter_mut().enumerate() {
                let (cx, cz) = (bx * 16 + j % 16, bz * 16 + j / 16);
                if cx < cells_side && cz < cells_side {
                    *c = out.len() as u32;
                    node(out, (cz * cells_side + cx) as u16, false, none);
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
            write_level1(&mut out, 0, 0)
        } else {
            let blocks = cells_side.div_ceil(16);
            let mut children = vec![0u32; 256];
            for bz in 0..blocks {
                for bx in 0..blocks {
                    children[bz * 16 + bx] = write_level1(&mut out, bx, bz);
                }
            }
            let start = out.len() as u32;
            let link = node(&mut out, 0, true, none);
            let table = out.len() as u32;
            out[link..link + 4].copy_from_slice(&table.to_le_bytes());
            for c in children {
                out.extend_from_slice(&c.to_le_bytes());
            }
            start
        };
        out[root_at..root_at + 4].copy_from_slice(&root.to_le_bytes());
        if cell_count > usize::from(u16::MAX) {
            return Err(Error::Corrupt("too many cells"));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod write_tests {
    use super::*;

    /// What we write reads back with the same heights, zones and tiles.
    #[test]
    fn written_terrain_reads_back() {
        for cells in [4usize, 20] {
            let size = cells * 16 + 1;
            let heights: Vec<f32> = (0..size * size)
                .map(|i| ((i % size) as f32 * 0.05).sin() * 20.0 + 30.0 + (i / size) as f32 * 0.1)
                .collect();
            let surfaces: Vec<u8> = (0..size * size).map(|i| (i % 3) as u8).collect();
            let textures = vec![(64u32, vec![0x7c00u16; 64 * 64]), (64, vec![0x03e0; 64 * 64])];
            let tiles_side = cells.div_ceil(4);
            let tiles: Vec<Option<u16>> = (0..tiles_side * tiles_side).map(|i| Some((i % 2) as u16)).collect();
            let bytes = Build { size, heights: &heights, surfaces: &surfaces, scale: 3.0, textures: &textures, tiles: &tiles }
                .write()
                .unwrap();
            let t = parse(&bytes).unwrap();
            assert_eq!(t.size, size);
            assert_eq!(t.scale, 3.0);
            for (a, b) in t.heights.iter().zip(&heights) {
                assert!((a * 3.0 - b).abs() < 0.05, "{a} vs {b}");
            }
            assert_eq!(t.surfaces, surfaces);
            assert_eq!(t.textures.len(), 2);
            assert_eq!(t.tiles.len(), tiles.len());
            assert_eq!(t.textures[1].levels[0][0], 0x03e0);
        }
    }

    #[test]
    fn parse_timing() {
        let cells = 20usize;
        let size = cells * 16 + 1;
        let heights: Vec<f32> = (0..size * size)
            .map(|i| ((i % size) as f32 * 0.05).sin() * 20.0 + 30.0 + (i / size) as f32 * 0.1)
            .collect();
        let surfaces: Vec<u8> = (0..size * size).map(|i| (i % 3) as u8).collect();
        let textures = vec![(64u32, vec![0x7c00u16; 64 * 64]), (64, vec![0x03e0; 64 * 64])];
        let tiles_side = cells.div_ceil(4);
        let tiles: Vec<Option<u16>> = (0..tiles_side * tiles_side).map(|i| Some((i % 2) as u16)).collect();
        let bytes = Build {
            size,
            heights: &heights,
            surfaces: &surfaces,
            scale: 3.0,
            textures: &textures,
            tiles: &tiles,
        }
        .write()
        .unwrap();
        let start = std::time::Instant::now();
        let mut t = None;
        for _ in 0..20 {
            t = Some(parse(&bytes).unwrap());
        }
        let t = t.unwrap();
        assert_eq!(t.size, size);
        eprintln!(
            "trn parse: {:?} for 20x {} cells, {} bytes",
            start.elapsed(),
            cells * cells,
            bytes.len()
        );
    }

    /// Rewrites a terrain written by [`Build`] with its cells' blocks
    /// LZW-packed, as the shipped tracks store them, fixing up the
    /// offset tables and the tree's offset.
    fn pack_cells(bytes: &[u8]) -> Vec<u8> {
        let at = |b: &[u8], o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        let cc = at(bytes, 32) as usize;
        let tc = at(bytes, 40 + 4 * cc) as usize;
        let root = at(bytes, 24) as usize;
        let cell_table = 36;
        let tex_table = 44 + 4 * cc;
        let mut out = Vec::new();
        out.extend_from_slice(&bytes[..28]); // header, root offset unchanged
        out.extend_from_slice(&bytes[28..36]); // cell counts
        let new_cell_table = out.len();
        out.resize(out.len() + 4 * cc, 0);
        out.extend_from_slice(&bytes[36 + 4 * cc..44 + 4 * cc]); // texture counts
        let new_tex_table = out.len();
        out.resize(out.len() + 4 * tc, 0);
        for i in 0..cc {
            let o = at(bytes, cell_table + 4 * i) as usize;
            let here = out.len() as u32;
            out[new_cell_table + 4 * i..new_cell_table + 4 * i + 4].copy_from_slice(&here.to_le_bytes());
            let mut pos = o;
            for _ in 0..3 {
                let packed = at(bytes, pos) as usize;
                let unpacked = at(bytes, pos + 4) as usize;
                let raw = &bytes[pos + 8..pos + 8 + packed];
                let compressed = crate::lzw::compress(raw);
                if compressed.len() < unpacked {
                    out.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
                    out.extend_from_slice(&(unpacked as u32).to_le_bytes());
                    out.extend_from_slice(&compressed);
                } else {
                    out.extend_from_slice(&(packed as u32).to_le_bytes());
                    out.extend_from_slice(&(unpacked as u32).to_le_bytes());
                    out.extend_from_slice(raw);
                }
                pos += 8 + packed;
            }
            out.extend_from_slice(&bytes[pos..pos + 4]); // base height
        }
        // Textures move with the cells; their offsets shift. The tree
        // keeps its own place: its nodes start before the root node
        // (the root's children are the level-1 nodes, and the first of
        // them is where the tree begins; at depth 1 the root is itself
        // a level-1 node), and its nodes hold absolute offsets, so the
        // packed file is padded back out to the tree's start and the
        // tree is copied verbatim from there.
        let old_tex_start = if tc > 0 { at(bytes, tex_table) as usize } else { root };
        let tree_start = if at(bytes, 20) == 1 {
            root
        } else {
            let extra = at(bytes, root + 88) as usize;
            let table = at(bytes, root + 93 + extra) as usize;
            (0..256)
                .map(|i| at(bytes, table + 4 * i) as usize)
                .filter(|&o| o != 0)
                .min()
                .unwrap_or(root)
        };
        let shift = out.len() as i64 - old_tex_start as i64;
        for j in 0..tc {
            let o = at(bytes, tex_table + 4 * j) as i64 + shift;
            out[new_tex_table + 4 * j..new_tex_table + 4 * j + 4]
                .copy_from_slice(&(o as u32).to_le_bytes());
        }
        out.extend_from_slice(&bytes[old_tex_start..tree_start]);
        out.resize(tree_start, 0);
        out.extend_from_slice(&bytes[tree_start..]);
        out
    }

    #[test]
    fn packed_cells_parse_the_same() {
        let cells = 20usize;
        let size = cells * 16 + 1;
        let heights: Vec<f32> = (0..size * size)
            .map(|i| ((i % size) as f32 * 0.05).sin() * 20.0 + 30.0 + (i / size) as f32 * 0.1)
            .collect();
        let surfaces: Vec<u8> = (0..size * size).map(|i| (i % 3) as u8).collect();
        let textures = vec![(64u32, vec![0x7c00u16; 64 * 64]), (64, vec![0x03e0; 64 * 64])];
        let tiles_side = cells.div_ceil(4);
        let tiles: Vec<Option<u16>> = (0..tiles_side * tiles_side).map(|i| Some((i % 2) as u16)).collect();
        let bytes = Build {
            size,
            heights: &heights,
            surfaces: &surfaces,
            scale: 3.0,
            textures: &textures,
            tiles: &tiles,
        }
        .write()
        .unwrap();
        let plain = parse(&bytes).unwrap();
        let packed = pack_cells(&bytes);
        // The packed file is padded back out to the tree's start, so
        // it is the same length; the cells inside it are smaller.
        let start = std::time::Instant::now();
        let mut t = None;
        for _ in 0..20 {
            t = Some(parse(&packed).unwrap());
        }
        let t = t.unwrap();
        eprintln!(
            "trn parse packed: {:?} for 20x {} cells, {} bytes",
            start.elapsed(),
            cells * cells,
            packed.len()
        );
        assert_eq!(t.size, plain.size);
        assert_eq!(t.heights, plain.heights);
        assert_eq!(t.normals, plain.normals);
        assert_eq!(t.surfaces, plain.surfaces);
        assert_eq!(t.tiles, plain.tiles);
        assert_eq!(t.textures.len(), plain.textures.len());
        assert_eq!(t.textures[1].levels[0][0], 0x03e0);
    }
}

#[cfg(test)]
mod palette_tests {
    use super::*;

    /// The demo's one-byte pixels are colours of the terrain's palette,
    /// not greys.
    #[test]
    fn one_byte_pixels_take_the_palette() {
        let stored = Stored::from_code(0x15).unwrap();
        assert_eq!(stored.bytes, 1);
        let mut palette = vec![0u8; 0x300];
        palette[3..6].copy_from_slice(&[0xf8, 0x80, 0x08]);
        let px = stored.pixels(&[1, 0], Some(&palette));
        assert_eq!(px[0], 31 << 11 | 32 << 5 | 1);
        assert_eq!(px[1], 0);
        // Without one, still grey.
        assert_eq!(stored.pixels(&[0xff], None)[0], 31 << 11 | 63 << 5 | 31);
    }
}
