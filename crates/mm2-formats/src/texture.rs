//! Model textures. They keep their `.tga` names but are not TGA files.
//!
//! ```text
//! u32 format (22 = RGB555, 26 = ARGB4444), u32 (2), u32 width, u32 height
//! u32 name_len, name   // palette for 8-bit cards, e.g. "RGBMaps.pal8"
//! mip chain            // as in terrain textures
//! ```

use crate::reader::Reader;
use crate::trn::{read_mips, PixelFormat, Texture};
use crate::{Error, Result};

pub fn parse(bytes: &[u8]) -> Result<Texture> {
    let mut r = Reader::new(bytes, 0);
    let format = PixelFormat::from_code(r.u32("texture format")?)
        .ok_or(Error::Corrupt("unknown texture format"))?;
    r.u32("texture version")?;
    let width = r.u32("texture width")?;
    let height = r.u32("texture height")?;
    if width != height {
        return Err(Error::Corrupt("texture is not square"));
    }
    let name_len = r.u32("palette name length")? as usize;
    r.take(name_len, "palette name")?;
    read_mips(&mut r, width, format)
}

/// The overlay pictures in `RES/GLOBAL.RES` (the speedometer, the race
/// panel, the radar): `.tga` names again, but one level of any size.
///
/// ```text
/// u32 format (11 = ARGB4444, 7 = RGB555), u32 bytes after this field,
/// u32 width, u32 height, u32 name_len, name   // the palette
/// u32 bytes (this field on), pixels (LZW unless width × height × 2)
/// ```
pub fn parse_overlay(bytes: &[u8]) -> Result<crate::tga::Image> {
    // Some are model textures, with their mip chain.
    if bytes.get(4..8) == Some(&[2, 0, 0, 0]) {
        let t = parse(bytes)?;
        return Ok(crate::tga::Image {
            width: t.size,
            height: t.size,
            rgba: t.rgba8(0),
        });
    }
    let mut r = Reader::new(bytes, 0);
    let code = r.u32("overlay format")?;
    let format = match code {
        11 => PixelFormat::Argb4444,
        7 => PixelFormat::Rgb555,
        _ => PixelFormat::from_code(code).ok_or(Error::Corrupt("unknown overlay format"))?,
    };
    r.u32("overlay size")?;
    let width = r.u32("overlay width")?;
    let height = r.u32("overlay height")?;
    let name_len = r.u32("palette name length")? as usize;
    r.take(name_len, "palette name")?;
    // The packed stream starts with its own length.
    let start = r.pos;
    let len = r.u32("overlay pixels")? as usize;
    let size = (width * height * 2) as usize;
    let data = if len == size + 4 {
        r.take(size, "overlay pixels")?.to_vec()
    } else {
        crate::lzw::decompress(&bytes[start..], size)?
    };
    let texture = Texture {
        size: width,
        format,
        levels: vec![data
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect()],
    };
    Ok(crate::tga::Image {
        width,
        height,
        rgba: texture.rgba8(0),
    })
}
