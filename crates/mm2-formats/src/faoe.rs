//! `FAOE` containers.
//!
//! Most `.RES` packs and every `.ENV` track are an RS2 archive run through a
//! byte-stream cipher (the game's `ResourceManager` file class):
//!
//! ```text
//! "FAOE"
//! ciphertext   // decodes to an RS2 archive; offsets count from here
//! ```
//!
//! Each plaintext byte is `key ^ c`, after which the key advances to
//! `key + c` (wrapping), where `c` is the ciphertext byte. The starting key
//! is a hash of the file's name; see [`key_for_name`].
//!
//! The game resets the key to the starting key whenever it seeks, so the
//! archive header decodes as one stream from the start, and each entry
//! decodes as its own stream from its offset.

use crate::{rs2, Error, Result};

pub const MAGIC: &[u8] = b"FAOE";

/// Starting key for a file, hashed from its name the way the game does.
///
/// The game passes lower-case names such as `bike.res` or `tag01.env`. The
/// hash works out to depend only on the extension, but this follows the
/// original over the whole name.
pub fn key_for_name(file_name: &str) -> u8 {
    file_name
        .to_ascii_lowercase()
        .bytes()
        .fold(0xfa, |k: u8, c| (k.wrapping_add(3) ^ c).wrapping_add(k))
}

/// Decodes a `FAOE` file into the RS2 archive it holds. `file_name` is
/// the file's name without directories, e.g. `BIKE.RES`.
///
/// Only the header is decoded as one stream from the start; each entry
/// is then decoded as its own stream from its offset. The old code first
/// ciphered the whole file and then copied each entry's ciphertext back
/// over it and ciphered it again — three passes over every byte, two of
/// them over entry data that the first pass had already touched for
/// nothing. Entry bytes are now touched twice (copy + cipher) and the
/// header once.
pub fn decode(bytes: &[u8], file_name: &str) -> Result<Vec<u8>> {
    let body = bytes.strip_prefix(MAGIC).ok_or(Error::BadMagic)?;
    let key = key_for_name(file_name);
    let mut plain = body.to_vec();
    let ranges = decode_header(&mut plain, key)?;
    for range in ranges {
        plain[range.clone()].copy_from_slice(&body[range.clone()]);
        cipher(&mut plain[range], key);
    }
    Ok(plain)
}

/// Decodes the RS2 header at the start of `plain` as one cipher stream
/// from `key`, in place, and returns each entry's byte range. Mirrors
/// [`rs2::Archive::parse`]'s checks, so a bad header fails the same way.
fn decode_header(plain: &mut [u8], key: u8) -> Result<Vec<std::ops::Range<usize>>> {
    let mut stream = Stream { buf: plain, pos: 0, key };
    let mut magic = [0u8; 4];
    for b in &mut magic {
        *b = stream.byte()?;
    }
    if &magic != rs2::MAGIC {
        return Err(Error::BadMagic);
    }
    let count = stream.u32()?;
    // Each entry takes at least 13 bytes, so a bogus count fails fast
    // instead of reserving a huge Vec.
    let mut ranges = Vec::with_capacity((count as usize).min(stream.buf.len() / 13));
    for _ in 0..count {
        let name_len = stream.u32()? as usize;
        let name_at = stream.pos;
        // Copy the short name out owned: the slice borrows the stream,
        // and the stream is needed again for the offset and size.
        let name: Vec<u8> = stream.bytes(name_len)?.to_vec();
        let name = name.strip_suffix(&[0]).unwrap_or(&name);
        if name.contains(&0) {
            return Err(Error::BadName { offset: name_at });
        }
        let offset = stream.u32()?;
        let size = stream.u32()?;
        let end = offset as u64 + size as u64;
        if end > stream.buf.len() as u64 {
            return Err(Error::EntryOutOfBounds {
                name: name.iter().map(|&b| b as char).collect(),
                offset,
                size,
            });
        }
        ranges.push(offset as usize..(offset + size) as usize);
    }
    Ok(ranges)
}

/// The cipher stream over a buffer, decoding bytes in place as they are
/// read, in order from the starting key.
struct Stream<'a> {
    buf: &'a mut [u8],
    pos: usize,
    key: u8,
}

impl<'a> Stream<'a> {
    fn byte(&mut self) -> Result<u8> {
        let b = *self.buf.get(self.pos).ok_or(Error::Truncated {
            what: "FAOE header",
            offset: self.pos,
        })?;
        let plain = self.key ^ b;
        self.buf[self.pos] = plain;
        self.key = self.key.wrapping_add(b);
        self.pos += 1;
        Ok(plain)
    }

    fn u32(&mut self) -> Result<u32> {
        let mut out = [0u8; 4];
        for b in &mut out {
            *b = self.byte()?;
        }
        Ok(u32::from_le_bytes(out))
    }

    fn bytes(&mut self, n: usize) -> Result<&[u8]> {
        if self.pos.checked_add(n).is_none_or(|e| e > self.buf.len()) {
            return Err(Error::Truncated {
                what: "FAOE header",
                offset: self.pos,
            });
        }
        // Decode in place, in stream order, then hand back the slice.
        for i in self.pos..self.pos + n {
            let c = self.buf[i];
            self.buf[i] = self.key ^ c;
            self.key = self.key.wrapping_add(c);
        }
        let out = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }
}

/// Decodes `buf` in place as one stream starting from `key`.
fn cipher(buf: &mut [u8], mut key: u8) {
    for b in buf {
        let c = *b;
        *b = key ^ c;
        key = key.wrapping_add(c);
    }
}

/// Encodes an RS2 archive as a `FAOE` file. The inverse of [`decode`].
pub fn encode(plain: &[u8], file_name: &str) -> Result<Vec<u8>> {
    let key = key_for_name(file_name);
    let encode_stream = |out: &mut [u8], plain: &[u8]| {
        let mut k = key;
        for (o, &p) in out.iter_mut().zip(plain) {
            *o = k ^ p;
            k = k.wrapping_add(*o);
        }
    };
    let mut body = vec![0; plain.len()];
    encode_stream(&mut body, plain);
    for e in rs2::Archive::parse(plain)?.entries() {
        let range = e.offset as usize..(e.offset + e.size) as usize;
        encode_stream(&mut body[range.clone()], &plain[range]);
    }
    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&body);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_match_the_shipped_files() {
        assert_eq!(key_for_name("uires.res"), 0x80);
        assert_eq!(key_for_name("BIKE.RES"), 0x80);
        assert_eq!(key_for_name("tag01.env"), 0x73);
    }

    #[test]
    fn round_trip() {
        let plain = rs2::build(&[("a.ini", b"[x]\n"), ("b.ini", b"[x]\n")]);
        let enc = encode(&plain, "x.res").unwrap();
        assert!(enc.starts_with(MAGIC));
        assert_ne!(&enc[4..], &plain[..]);
        assert_eq!(decode(&enc, "X.RES").unwrap(), plain);
        // Each entry is its own stream, so equal entries encode equally.
        let ar = rs2::Archive::parse(&plain).unwrap();
        let [a, b] = ar.entries() else { panic!() };
        let at = |e: &rs2::Entry| &enc[4 + e.offset as usize..][..e.size as usize];
        assert_eq!(at(a), at(b));
    }

    #[test]
    fn rejects_wrong_magic() {
        assert_eq!(decode(b"RS2\0", "a.res").unwrap_err(), Error::BadMagic);
    }

    #[test]
    fn round_trip_many_entries_and_large_payloads() {
        let big = vec![0xabu8; 1 << 20];
        let mut entries: Vec<(&str, &[u8])> = Vec::new();
        entries.push(("first.bin", &big));
        entries.push(("second.bin", b"hello"));
        entries.push(("third.bin", &big));
        let plain = rs2::build(&entries);
        let enc = encode(&plain, "big.res").unwrap();
        assert_eq!(decode(&enc, "BIG.RES").unwrap(), plain);
        // A truncated header fails instead of decoding garbage.
        assert!(decode(&enc[..20], "big.res").is_err());
    }
}
