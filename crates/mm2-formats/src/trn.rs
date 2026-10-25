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
}

impl PixelFormat {
    pub fn from_code(code: u32) -> Option<Self> {
        match code {
            22 => Some(PixelFormat::Rgb555),
            26 => Some(PixelFormat::Argb4444),
            _ => None,
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
            })
            .collect()
    }

    /// Whether any pixel is less than fully opaque.
    pub fn has_alpha(&self) -> bool {
        self.format == PixelFormat::Argb4444 && self.levels[0].iter().any(|&p| p >> 12 != 15)
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
    if r.u32("palette flag")? != 0 {
        skip_palette(&mut r)?;
    }
    r.u32("unknown")?;
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

    let mut cells = Vec::with_capacity(cell_count.min(bytes.len() / 16));
    for _ in 0..cell_count {
        block(&mut r, 0x200)?;
        block(&mut r, 0x244)?;
        let samples = block(&mut r, CELL_BYTES)?;
        cells.push((samples, r.u32("cell base height")? as f32));
    }

    let textures = texture_offsets[..texture_count]
        .iter()
        .map(|&o| read_texture(&mut Reader::new(bytes, o)))
        .collect::<Result<Vec<_>>>()?;

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
        size,
        heights,
        normals,
        surfaces,
        scale,
        textures,
        tiles,
    })
}

/// Reads a block that is stored raw or LZW-packed with a size prefix pair.
fn block(r: &mut Reader, len: usize) -> Result<Vec<u8>> {
    let packed = r.u32("block packed size")? as usize;
    let unpacked = r.u32("block size")? as usize;
    if packed < unpacked {
        lzw::decompress(r.take(packed, "packed block")?, len)
    } else if unpacked == len {
        Ok(r.take(len, "block")?.to_vec())
    } else {
        Err(Error::Corrupt("block has an unexpected size"))
    }
}

fn skip_palette(r: &mut Reader) -> Result<()> {
    r.u32("palette start")?;
    r.u32("palette colours")?;
    let packed = r.u32("palette packing")? != 0;
    for len in [0x300, 0x8000, 0x10000] {
        if packed {
            let size = r.u32("palette block size")? as usize;
            r.take(size, "palette block")?;
        } else {
            r.take(len, "palette block")?;
        }
    }
    Ok(())
}

fn read_texture(r: &mut Reader) -> Result<Texture> {
    let size = r.u32("texture width")?;
    let height = r.u32("texture height")?;
    let format = r.u32("texture format")?;
    if size != height {
        return Err(Error::Corrupt("texture is not square"));
    }
    if format != 22 {
        return Err(Error::Corrupt("unknown terrain texture format"));
    }
    read_mips(r, size, PixelFormat::Rgb555)
}

/// Reads a mip chain: one offset per level from 1×1 up to size×size, plus
/// the end, each relative to where it is stored. A level is LZW packed
/// unless its length is exactly the raw size.
pub(crate) fn read_mips(r: &mut Reader, size: u32, format: PixelFormat) -> Result<Texture> {
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
        let len = side * side * 2;
        r.seek(w[0], "mip level")?;
        let src = r.take(w[1].saturating_sub(w[0]), "mip level")?;
        let data = if src.len() == len {
            src.to_vec()
        } else {
            lzw::decompress(src, len)?
        };
        levels.push(
            data.chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect(),
        );
    }
    levels.reverse();
    Ok(Texture {
        size,
        format,
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
