//! Pictures a player brings into the builder: a height map, a ground
//! picture, a plant map, a model's texture. Kept in the `.course` as the
//! file itself (base64), so a track travels whole, and read when baked.
//!
//! Read: PNG (8 or 16 bits, grey or colour), TGA (grey, colour, 16-bit
//! grey as POV-Ray writes, packed or not) and raw 8 or 16-bit squares
//! (`.raw`/`.r16`, as terrain tools write).

use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};

/// A height map picture the land is raised from: black lowest, white
/// highest, its top edge the map's far (north) side.
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HeightMap {
    pub picture: Embedded,
    /// How tall white stands above black, ft.
    pub tallest: f32,
    /// How much of the picture to take: 1 the picture alone, less to blend
    /// it with the land's own hills.
    pub mix: f32,
    #[serde(skip)]
    read: Arc<OnceLock<Option<Picture>>>,
}

impl Default for HeightMap {
    fn default() -> Self {
        HeightMap {
            picture: Embedded::default(),
            tallest: 300.0,
            mix: 1.0,
            read: Arc::default(),
        }
    }
}

impl PartialEq for HeightMap {
    fn eq(&self, o: &Self) -> bool {
        self.picture == o.picture && self.tallest == o.tallest && self.mix == o.mix
    }
}

impl std::fmt::Debug for HeightMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HeightMap({}, {} ft, mix {})", self.picture.name, self.tallest, self.mix)
    }
}

impl HeightMap {
    pub fn new(picture: Embedded) -> HeightMap {
        HeightMap { picture, ..HeightMap::default() }
    }

    /// The picture, read once.
    pub fn read(&self) -> Option<&Picture> {
        self.read.get_or_init(|| self.picture.picture().ok()).as_ref()
    }

    /// The picture's height at `x`, `z` (ft) on land `side` ft across.
    pub fn height(&self, side: f32, x: f32, z: f32) -> Option<f32> {
        self.read().map(|p| p.grey_at(x / side, z / side) * self.tallest)
    }
}

/// Pictures read once and shared: a ground picture, a plant map.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Shared {
    pub file: Embedded,
    #[serde(skip)]
    read: Arc<OnceLock<Option<Picture>>>,
}

impl PartialEq for Shared {
    fn eq(&self, o: &Self) -> bool {
        self.file == o.file
    }
}

impl std::fmt::Debug for Shared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Shared({})", self.file.name)
    }
}

impl Shared {
    pub fn new(file: Embedded) -> Shared {
        Shared { file, read: Arc::default() }
    }

    pub fn read(&self) -> Option<&Picture> {
        self.read.get_or_init(|| self.file.picture().ok()).as_ref()
    }
}

/// A file kept inside the course.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Embedded {
    /// The file's own name, as picked.
    pub name: String,
    /// Its bytes, base64.
    pub data: String,
}

impl Embedded {
    pub fn new(name: &str, bytes: &[u8]) -> Embedded {
        Embedded {
            name: name.to_string(),
            data: encode(bytes),
        }
    }

    pub fn bytes(&self) -> Vec<u8> {
        decode(&self.data)
    }

    /// Read as a picture.
    pub fn picture(&self) -> Result<Picture, String> {
        Picture::read(&self.name, &self.bytes())
    }
}

/// A picture read in: brightness (0 to 1, 16-bit where the file has it)
/// and colour at each pixel, rows top first.
#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
    pub w: usize,
    pub h: usize,
    pub grey: Vec<f32>,
    pub rgb: Vec<[u8; 3]>,
}

impl Picture {
    pub fn read(name: &str, bytes: &[u8]) -> Result<Picture, String> {
        let lower = name.to_ascii_lowercase();
        if bytes.starts_with(b"\x89PNG") {
            return png_picture(bytes);
        }
        if lower.ends_with(".tga") {
            return tga(bytes);
        }
        if lower.ends_with(".raw") || lower.ends_with(".r16") || lower.ends_with(".r8") {
            return raw(bytes, lower.ends_with(".r8"));
        }
        // Without a telling name: a TGA has no magic, so try it, then raw.
        tga(bytes).or_else(|_| raw(bytes, false)).map_err(|_| format!("{name}: not a picture this can read (PNG, TGA or raw)"))
    }

    /// From brightness alone.
    pub fn from_grey(w: usize, h: usize, grey: Vec<f32>) -> Picture {
        let rgb = grey.iter().map(|&g| [(g.clamp(0.0, 1.0) * 255.0).round() as u8; 3]).collect();
        Picture { w, h, grey, rgb }
    }

    fn from_rgb(w: usize, h: usize, rgb: Vec<[u8; 3]>) -> Picture {
        let grey = rgb
            .iter()
            .map(|c| (0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32) / 255.0)
            .collect();
        Picture { w, h, grey, rgb }
    }

    /// Brightness at `u`, `v` (0 to 1 across and down), between pixels.
    pub fn grey_at(&self, u: f32, v: f32) -> f32 {
        let fx = (u.clamp(0.0, 1.0) * (self.w - 1) as f32).max(0.0);
        let fy = (v.clamp(0.0, 1.0) * (self.h - 1) as f32).max(0.0);
        let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(self.w - 1), (y0 + 1).min(self.h - 1));
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let g = |x: usize, y: usize| self.grey[y * self.w + x];
        let a = g(x0, y0) * (1.0 - tx) + g(x1, y0) * tx;
        let b = g(x0, y1) * (1.0 - tx) + g(x1, y1) * tx;
        a * (1.0 - ty) + b * ty
    }

    /// Colour at `u`, `v`, between pixels.
    pub fn rgb_at(&self, u: f32, v: f32) -> [f32; 3] {
        let fx = (u.clamp(0.0, 1.0) * (self.w - 1) as f32).max(0.0);
        let fy = (v.clamp(0.0, 1.0) * (self.h - 1) as f32).max(0.0);
        let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(self.w - 1), (y0 + 1).min(self.h - 1));
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let c = |x: usize, y: usize, k: usize| self.rgb[y * self.w + x][k] as f32;
        std::array::from_fn(|k| {
            let a = c(x0, y0, k) * (1.0 - tx) + c(x1, y0, k) * tx;
            let b = c(x0, y1, k) * (1.0 - tx) + c(x1, y1, k) * tx;
            a * (1.0 - ty) + b * ty
        })
    }

    /// As an 8-bit colour PNG.
    pub fn to_png(&self) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut e = png::Encoder::new(&mut out, self.w as u32, self.h as u32);
            e.set_color(png::ColorType::Rgb);
            e.set_depth(png::BitDepth::Eight);
            if let Ok(mut w) = e.write_header() {
                let flat: Vec<u8> = self.rgb.iter().flatten().copied().collect();
                let _ = w.write_image_data(&flat);
            }
        }
        out
    }
}

fn png_picture(bytes: &[u8]) -> Result<Picture, String> {
    let mut d = png::Decoder::new(std::io::Cursor::new(bytes));
    d.set_transformations(png::Transformations::EXPAND);
    let mut r = d.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; r.output_buffer_size().ok_or("picture too big")?];
    let info = r.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let (w, h) = (info.width as usize, info.height as usize);
    let deep = info.bit_depth == png::BitDepth::Sixteen;
    let chans = info.color_type.samples();
    let at = |i: usize| -> f32 {
        if deep {
            u16::from_be_bytes([buf[i * 2], buf[i * 2 + 1]]) as f32 / 65535.0
        } else {
            buf[i] as f32 / 255.0
        }
    };
    let n = w * h;
    if chans <= 2 {
        // Grey (with alpha): keep its full depth.
        let grey: Vec<f32> = (0..n).map(|p| at(p * chans)).collect();
        return Ok(Picture::from_grey(w, h, grey));
    }
    let rgb = (0..n).map(|p| std::array::from_fn(|k| (at(p * chans + k) * 255.0).round() as u8)).collect();
    let mut pic = Picture::from_rgb(w, h, rgb);
    if deep {
        // A 16-bit colour height map: brightness at full depth.
        pic.grey = (0..n).map(|p| (0.299 * at(p * chans) + 0.587 * at(p * chans + 1) + 0.114 * at(p * chans + 2)).clamp(0.0, 1.0)).collect();
    }
    Ok(pic)
}

/// A TGA: types 2, 3, 10 and 11, 8, 16, 24 or 32 bits a pixel. 16 bits in
/// a colour TGA is POV-Ray's height field (high byte red, low byte green)
/// when written as 24 bits, else 5-5-5 colour; in a grey TGA, 16-bit grey.
fn tga(b: &[u8]) -> Result<Picture, String> {
    if b.len() < 18 {
        return Err("too short".into());
    }
    let id = b[0] as usize;
    let maps = b[1];
    let kind = b[2];
    let map_len = u16::from_le_bytes([b[5], b[6]]) as usize;
    let map_bits = b[7] as usize;
    let w = u16::from_le_bytes([b[12], b[13]]) as usize;
    let h = u16::from_le_bytes([b[14], b[15]]) as usize;
    let bits = b[16] as usize;
    let top = b[17] & 0x20 != 0;
    if !matches!(kind, 2 | 3 | 10 | 11) || w == 0 || h == 0 || !matches!(bits, 8 | 16 | 24 | 32) || w * h > 8192 * 8192 {
        return Err("not a TGA this can read".into());
    }
    let start = 18 + id + if maps != 0 { map_len * map_bits.div_ceil(8) } else { 0 };
    let px = bits / 8;
    let n = w * h;
    let mut data = Vec::with_capacity(n * px);
    let mut i = start;
    if kind >= 10 {
        while data.len() < n * px {
            let head = *b.get(i).ok_or("cut short")?;
            i += 1;
            let count = (head & 0x7f) as usize + 1;
            if head & 0x80 != 0 {
                let p = b.get(i..i + px).ok_or("cut short")?;
                for _ in 0..count {
                    data.extend_from_slice(p);
                }
                i += px;
            } else {
                data.extend_from_slice(b.get(i..i + count * px).ok_or("cut short")?);
                i += count * px;
            }
        }
        data.truncate(n * px);
    } else {
        data.extend_from_slice(b.get(i..i + n * px).ok_or("cut short")?);
    }
    // Rows top first.
    let row = |y: usize| if top { y } else { h - 1 - y };
    let grey_kind = kind == 3 || kind == 11;
    let mut grey = vec![0.0; n];
    let mut rgb = vec![[0u8; 3]; n];
    for y in 0..h {
        for x in 0..w {
            let s = &data[(row(y) * w + x) * px..][..px];
            let o = y * w + x;
            match (grey_kind, bits) {
                (true, 16) => {
                    let v = u16::from_le_bytes([s[0], s[1]]);
                    grey[o] = v as f32 / 65535.0;
                    rgb[o] = [(v >> 8) as u8; 3];
                }
                (true, _) => {
                    grey[o] = s[0] as f32 / 255.0;
                    rgb[o] = [s[0]; 3];
                }
                (false, 16) => {
                    let v = u16::from_le_bytes([s[0], s[1]]);
                    let c = |k: u16| (((v >> k) & 31) * 255 / 31) as u8;
                    rgb[o] = [c(10), c(5), c(0)];
                    grey[o] = (0.299 * rgb[o][0] as f32 + 0.587 * rgb[o][1] as f32 + 0.114 * rgb[o][2] as f32) / 255.0;
                }
                (false, 8) => {
                    grey[o] = s[0] as f32 / 255.0;
                    rgb[o] = [s[0]; 3];
                }
                _ => {
                    // BGR(A).
                    rgb[o] = [s[2], s[1], s[0]];
                    grey[o] = (0.299 * s[2] as f32 + 0.587 * s[1] as f32 + 0.114 * s[0] as f32) / 255.0;
                }
            }
        }
    }
    let mut pic = Picture { w, h, grey, rgb };
    // POV-Ray's 16-bit height field: red the high byte, green the low,
    // blue empty.
    if !grey_kind && bits >= 24 && pic.rgb.iter().all(|c| c[2] == 0) && pic.rgb.iter().any(|c| c[1] != 0) {
        pic.grey = pic.rgb.iter().map(|c| ((c[0] as u16) << 8 | c[1] as u16) as f32 / 65535.0).collect();
    }
    Ok(pic)
}

/// A square of 16-bit (little-endian) or 8-bit heights.
fn raw(b: &[u8], eight: bool) -> Result<Picture, String> {
    let side16 = ((b.len() / 2) as f64).sqrt() as usize;
    if !eight && side16 > 1 && side16 * side16 * 2 == b.len() {
        let grey = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]]) as f32 / 65535.0).collect();
        return Ok(Picture::from_grey(side16, side16, grey));
    }
    let side8 = (b.len() as f64).sqrt() as usize;
    if side8 > 1 && side8 * side8 == b.len() {
        let grey = b.iter().map(|&v| v as f32 / 255.0).collect();
        return Ok(Picture::from_grey(side8, side8, grey));
    }
    Err("a raw height map must be square".into())
}

const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            if k <= c.len() {
                out.push(ABC[(n >> (18 - 6 * k)) as usize & 63] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub fn decode(text: &str) -> Vec<u8> {
    let mut val = [255u8; 256];
    for (i, &c) in ABC.iter().enumerate() {
        val[c as usize] = i as u8;
    }
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0);
    for &c in text.as_bytes() {
        let v = val[c as usize];
        if v == 255 {
            continue;
        }
        acc = acc << 6 | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips() {
        for n in 0..10 {
            let b: Vec<u8> = (0..n * 7 + 1).map(|i| (i * 37 + n) as u8).collect();
            assert_eq!(decode(&encode(&b)), b);
        }
        assert_eq!(encode(b"Man"), "TWFu");
        assert_eq!(encode(b"Ma"), "TWE=");
    }

    #[test]
    fn reads_tga_and_raw() {
        // A 2x2 grey TGA, bottom row first.
        let mut t = vec![0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 2, 0, 8, 0];
        t.extend([10, 20, 30, 40]);
        let p = tga(&t).unwrap();
        assert_eq!(p.rgb[0], [30; 3]);
        assert_eq!(p.rgb[3], [20; 3]);
        let r: Vec<u8> = [0u16, 65535, 32768, 1].iter().flat_map(|v| v.to_le_bytes()).collect();
        let p = Picture::read("x.r16", &r).unwrap();
        assert_eq!((p.w, p.grey[1]), (2, 1.0));
        let png = p.to_png();
        assert_eq!(Picture::read("a.png", &png).unwrap().rgb[1], [255; 3]);
    }
}
