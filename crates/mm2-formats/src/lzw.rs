//! The game's LZW decompressor (`Lzw.cpp` in the original).
//!
//! The stream is read MSB-first from big-endian 32-bit words. The first
//! word holds the packed size and is skipped. Codes start 9 bits wide, and
//! three codes are reserved:
//!
//! * `0x100` – end of data
//! * `0x101` – widen codes by one bit
//! * `0x102` – clear the dictionary (the next code is a literal)
//!
//! A stream that ends before `out_len` bytes is padded with zeros.

use crate::{Error, Result};

const END: u32 = 0x100;
const WIDEN: u32 = 0x101;
const CLEAR: u32 = 0x102;
const FIRST_FREE: u32 = 0x103;

struct Bits<'a> {
    src: &'a [u8],
    pos: usize,
}

impl Bits<'_> {
    fn read(&mut self, width: u32) -> Result<u32> {
        let mut v = 0;
        for _ in 0..width {
            let byte = *self.src.get(self.pos >> 3).ok_or(Error::Truncated {
                what: "LZW stream",
                offset: self.pos >> 3,
            })?;
            v = (v << 1) | u32::from((byte >> (7 - (self.pos & 7))) & 1);
            self.pos += 1;
        }
        Ok(v)
    }
}

/// Decompresses `src` into exactly `out_len` bytes.
pub fn decompress(src: &[u8], out_len: usize) -> Result<Vec<u8>> {
    let mut bits = Bits { src, pos: 32 };
    let mut out = Vec::with_capacity(out_len);
    // Each dictionary entry is (prefix code, last byte, first byte).
    let mut dict: Vec<(u32, u8, u8)> = Vec::new();
    let mut scratch = Vec::new();
    'clear: loop {
        dict.clear();
        let mut width = 9;
        let mut prev = bits.read(width)?;
        if prev == END {
            break;
        }
        if prev > 0xff {
            return Err(Error::Corrupt("LZW stream starts with a non-literal"));
        }
        out.push(prev as u8);
        loop {
            let code = bits.read(width)?;
            match code {
                END => break 'clear,
                CLEAR => continue 'clear,
                WIDEN => {
                    width += 1;
                    continue;
                }
                _ => {}
            }
            let next = FIRST_FREE + dict.len() as u32;
            let first = if code < next {
                expand(code, &dict, &mut scratch);
                out.extend(scratch.iter().rev());
                *scratch.last().unwrap()
            } else if code == next {
                // The code being defined right now: prev + first(prev).
                expand(prev, &dict, &mut scratch);
                let first = *scratch.last().unwrap();
                out.extend(scratch.iter().rev());
                out.push(first);
                first
            } else {
                return Err(Error::Corrupt("LZW code out of range"));
            };
            dict.push((prev, first, first_of(prev, &dict)));
            prev = code;
            if out.len() >= out_len {
                break 'clear;
            }
        }
    }
    // The game (0x4a1940) decodes up to the end code and leaves the rest
    // of its buffer as it was. Some track makers' tools end the stream a
    // byte early (Angel Delight II's cells), so the rest is zeros.
    out.resize(out_len, 0);
    Ok(out)
}

/// Writes the bytes of `code` into `buf` in reverse order.
fn expand(mut code: u32, dict: &[(u32, u8, u8)], buf: &mut Vec<u8>) {
    buf.clear();
    while code > 0xff {
        let (prefix, last, _) = dict[(code - FIRST_FREE) as usize];
        buf.push(last);
        code = prefix;
    }
    buf.push(code as u8);
}

fn first_of(code: u32, dict: &[(u32, u8, u8)]) -> u8 {
    if code > 0xff {
        dict[(code - FIRST_FREE) as usize].2
    } else {
        code as u8
    }
}

/// The game's dictionary holds 35023 entries; codes stay within 15 bits.
const MOST_CODE: u32 = 0x7fff;

/// Packs `data` the way [`decompress`] (and the game's `ExpandLzw`)
/// reads it: a big-endian word with the packed size, then codes from 9
/// bits wide, widened by [`WIDEN`] and restarted by [`CLEAR`] when the
/// dictionary fills, ended by [`END`] and padded to whole words.
pub fn compress(data: &[u8]) -> Vec<u8> {
    struct Out {
        bytes: Vec<u8>,
        acc: u64,
        n: u32,
    }
    impl Out {
        fn put(&mut self, code: u32, width: u32) {
            self.acc = self.acc << width | u64::from(code);
            self.n += width;
            while self.n >= 8 {
                self.n -= 8;
                self.bytes.push((self.acc >> self.n) as u8);
            }
        }
    }
    let mut out = Out { bytes: vec![0; 4], acc: 0, n: 0 };
    let mut dict: std::collections::HashMap<(u32, u8), u32> = std::collections::HashMap::new();
    let mut width = 9;
    let mut next = FIRST_FREE;
    let mut bump = 511;
    let mut current: Option<u32> = None;
    for &b in data {
        let Some(cur) = current else {
            current = Some(u32::from(b));
            continue;
        };
        if let Some(&code) = dict.get(&(cur, b)) {
            current = Some(code);
            continue;
        }
        out.put(cur, width);
        if next > MOST_CODE {
            out.put(CLEAR, width);
            dict.clear();
            width = 9;
            next = FIRST_FREE;
            bump = 511;
        } else {
            dict.insert((cur, b), next);
            next += 1;
            if next > bump {
                out.put(WIDEN, width);
                width += 1;
                bump = bump << 1 | 1;
            }
        }
        current = Some(u32::from(b));
    }
    if let Some(cur) = current {
        out.put(cur, width);
    }
    out.put(END, width);
    if out.n > 0 {
        let pad = 8 - out.n;
        out.put(0, pad);
    }
    while out.bytes.len() % 4 != 0 {
        out.bytes.push(0);
    }
    let len = out.bytes.len() as u32;
    out.bytes[..4].copy_from_slice(&len.to_be_bytes());
    out.bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Packs codes MSB-first after a 32-bit header, like the game's writer.
    fn pack(codes: &[(u32, u32)]) -> Vec<u8> {
        let mut bits: Vec<bool> = vec![false; 32];
        for &(code, width) in codes {
            bits.extend((0..width).rev().map(|i| code >> i & 1 == 1));
        }
        bits.resize(bits.len().div_ceil(32) * 32, false);
        bits.chunks(8)
            .map(|b| b.iter().fold(0u8, |acc, &bit| acc << 1 | bit as u8))
            .collect()
    }

    #[test]
    fn decodes_literals_and_back_references() {
        // "abab" + the KwKwK case: a, b, 0x103 ("ab"), 0x105 ("aba").
        let src = pack(&[
            (b'a' as u32, 9),
            (b'b' as u32, 9),
            (0x103, 9),
            (0x105, 9),
            (END, 9),
        ]);
        assert_eq!(decompress(&src, 7).unwrap(), b"abababa");
    }

    #[test]
    fn handles_widen_and_clear() {
        let src = pack(&[
            (b'x' as u32, 9),
            (WIDEN, 9),
            (b'y' as u32, 10),
            (CLEAR, 10),
            (b'z' as u32, 9),
            (END, 9),
        ]);
        assert_eq!(decompress(&src, 3).unwrap(), b"xyz");
    }

    #[test]
    fn pads_short_streams_with_zeros() {
        let src = pack(&[(b'a' as u32, 9), (END, 9)]);
        assert_eq!(decompress(&src, 3).unwrap(), b"a\0\0");
    }

    #[test]
    fn compress_round_trips() {
        let mut seed = 1u32;
        let mut data = Vec::new();
        for i in 0..300_000u32 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            data.push(if i % 7 < 4 { (i / 97) as u8 } else { (seed >> 24) as u8 });
        }
        for len in [0usize, 1, 2, 300, 0x908, 70_000, data.len()] {
            let packed = compress(&data[..len]);
            assert_eq!(packed.len() % 4, 0);
            assert_eq!(decompress(&packed, len).unwrap(), &data[..len]);
        }
    }
}
