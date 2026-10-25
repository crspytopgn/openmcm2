//! Reads the files off a CD image: a plain `.iso`, or a raw image such as
//! a `.bin` (from a `.cue`), `.img`, `.mdf` or `.nrg`, whatever its sector
//! size. Only the ISO 9660 file system the game's CD uses is needed: its
//! names are all 8.3 upper case.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// The sector sizes and where each sector's 2048 bytes of data start:
/// cooked, raw Mode 1, raw Mode 2 form 1, and Mode 2 without the sync.
const LAYOUTS: [(u64, u64); 4] = [(2048, 0), (2352, 16), (2352, 24), (2336, 8)];

/// Some images start with a header before the first sector (`.nrg` can
/// have a 150-sector pregap).
const STARTS: [u64; 3] = [0, 150 * 2352, 150 * 2048];

/// An open CD image.
pub struct Image {
    file: File,
    /// Bytes before sector 0.
    start: u64,
    sector: u64,
    data: u64,
    root: Entry,
}

/// A file or folder on the image.
#[derive(Debug, Clone)]
pub struct Entry {
    /// The name, without the `;1` version.
    pub name: String,
    pub dir: bool,
    /// The first sector.
    lba: u32,
    pub size: u64,
}

impl Image {
    /// Opens `path`, or for a `.cue` the image it names.
    pub fn open(path: &Path) -> io::Result<Image> {
        let path = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("cue")) {
            cue_image(path)?
        } else {
            path.to_path_buf()
        };
        let mut file = File::open(&path)?;
        for start in STARTS {
            for (sector, data) in LAYOUTS {
                let mut pvd = [0u8; 2048];
                if file.seek(SeekFrom::Start(start + 16 * sector + data)).is_err()
                    || file.read_exact(&mut pvd).is_err()
                {
                    continue;
                }
                if pvd[0] == 1 && &pvd[1..6] == b"CD001" {
                    let root = record(&pvd[156..190]).ok_or_else(|| bad("no root folder"))?;
                    return Ok(Image { file, start, sector, data, root });
                }
            }
        }
        Err(bad("not a CD image with an ISO 9660 file system"))
    }

    /// What's in the folder `dir`.
    pub fn list(&mut self, dir: &Entry) -> io::Result<Vec<Entry>> {
        let bytes = self.read(dir.lba, dir.size)?;
        let mut out = Vec::new();
        let mut at = 0;
        while at < bytes.len() {
            let len = bytes[at] as usize;
            if len == 0 {
                // Records never cross a sector: on to the next one.
                at = (at / 2048 + 1) * 2048;
                continue;
            }
            let Some(rec) = bytes.get(at..at + len) else { break };
            if let Some(e) = record(rec).filter(|e| e.name != "." && e.name != "..") {
                out.push(e);
            }
            at += len;
        }
        Ok(out)
    }

    /// The entry at `path` (`GAME/UI`), ignoring case.
    pub fn find(&mut self, path: &str) -> io::Result<Option<Entry>> {
        let mut here = self.root.clone();
        for part in path.split('/').filter(|p| !p.is_empty()) {
            match self.list(&here)?.into_iter().find(|e| e.name.eq_ignore_ascii_case(part)) {
                Some(e) => here = e,
                None => return Ok(None),
            }
        }
        Ok(Some(here))
    }

    /// A file's bytes.
    pub fn read_file(&mut self, file: &Entry) -> io::Result<Vec<u8>> {
        self.read(file.lba, file.size)
    }

    /// Every file under `dir`, with its path from there, folders first.
    pub fn walk(&mut self, dir: &Entry) -> io::Result<Vec<(PathBuf, Entry)>> {
        let mut out = Vec::new();
        let mut todo = vec![(PathBuf::new(), dir.clone())];
        while let Some((at, dir)) = todo.pop() {
            for e in self.list(&dir)? {
                let path = at.join(&e.name);
                if e.dir {
                    todo.push((path.clone(), e.clone()));
                }
                out.push((path, e));
            }
        }
        Ok(out)
    }

    fn read(&mut self, lba: u32, size: u64) -> io::Result<Vec<u8>> {
        let mut out = vec![0u8; size as usize];
        if self.sector == 2048 {
            self.file.seek(SeekFrom::Start(self.start + lba as u64 * 2048))?;
            self.file.read_exact(&mut out)?;
            return Ok(out);
        }
        for (i, chunk) in out.chunks_mut(2048).enumerate() {
            let at = self.start + (lba as u64 + i as u64) * self.sector + self.data;
            self.file.seek(SeekFrom::Start(at))?;
            self.file.read_exact(chunk)?;
        }
        Ok(out)
    }
}

/// A directory record.
fn record(rec: &[u8]) -> Option<Entry> {
    if rec.len() < 34 {
        return None;
    }
    let name_len = rec[32] as usize;
    let raw = rec.get(33..33 + name_len)?;
    let name = match raw {
        [0] => ".".to_string(),
        [1] => "..".to_string(),
        _ => {
            let s = String::from_utf8_lossy(raw);
            let s = s.split(';').next().unwrap_or("");
            s.strip_suffix('.').unwrap_or(s).to_string()
        }
    };
    Some(Entry {
        name,
        dir: rec[25] & 2 != 0,
        lba: u32::from_le_bytes(rec[2..6].try_into().ok()?),
        size: u32::from_le_bytes(rec[10..14].try_into().ok()?) as u64,
    })
}

/// The first data track's file in a `.cue`.
fn cue_image(cue: &Path) -> io::Result<PathBuf> {
    let text = std::fs::read_to_string(cue)?;
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| l.to_ascii_uppercase().starts_with("FILE "))
        .ok_or_else(|| bad("the .cue names no image file"))?;
    let rest = line[5..].trim();
    let name = match rest.strip_prefix('"') {
        Some(r) => r.split('"').next().unwrap_or(""),
        None => rest.split_whitespace().next().unwrap_or(""),
    };
    Ok(cue.parent().unwrap_or(Path::new(".")).join(name))
}

fn bad(why: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, why)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-file image built by hand: the PVD at sector 16, the root
    /// folder at 18 and the file at 19.
    fn image(sector: usize, data: usize) -> Vec<u8> {
        let mut cooked = vec![0u8; 20 * 2048];
        let rec = |name: &[u8], lba: u32, size: u32, dir: bool| {
            let mut r = vec![0u8; 33 + name.len() + (1 - name.len() % 2)];
            r[0] = r.len() as u8;
            r[2..6].copy_from_slice(&lba.to_le_bytes());
            r[10..14].copy_from_slice(&size.to_le_bytes());
            r[25] = if dir { 2 } else { 0 };
            r[32] = name.len() as u8;
            r[33..33 + name.len()].copy_from_slice(name);
            r
        };
        let pvd = &mut cooked[16 * 2048..17 * 2048];
        pvd[0] = 1;
        pvd[1..6].copy_from_slice(b"CD001");
        pvd[156..190].copy_from_slice(&rec(&[0], 18, 2048, true));
        let mut root = rec(&[0], 18, 2048, true);
        root.extend(rec(&[1], 18, 2048, true));
        root.extend(rec(b"HELLO.TXT;1", 19, 5, false));
        cooked[18 * 2048..18 * 2048 + root.len()].copy_from_slice(&root);
        cooked[19 * 2048..19 * 2048 + 5].copy_from_slice(b"hello");
        let mut raw = vec![0u8; 20 * sector];
        for i in 0..20 {
            raw[i * sector + data..i * sector + data + 2048].copy_from_slice(&cooked[i * 2048..(i + 1) * 2048]);
        }
        raw
    }

    #[test]
    fn reads_every_layout() {
        let dir = std::env::temp_dir().join(format!("mm2-iso-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (sector, data) in LAYOUTS {
            let path = dir.join(format!("t{sector}_{data}.bin"));
            std::fs::write(&path, image(sector as usize, data as usize)).unwrap();
            let mut img = Image::open(&path).unwrap();
            let f = img.find("hello.txt").unwrap().unwrap();
            assert_eq!(img.read_file(&f).unwrap(), b"hello");
        }
        std::fs::write(dir.join("t.cue"), "FILE \"t2352_16.bin\" BINARY\n  TRACK 01 MODE1/2352\n").unwrap();
        assert!(Image::open(&dir.join("t.cue")).is_ok());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
