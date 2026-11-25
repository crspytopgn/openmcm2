//! The replay editor's clips on a desktop: the frames drawn, as JPEG
//! pictures one after another in a movie file that plays almost anywhere
//! with nothing to install (Motion JPEG): an AVI on Windows and Linux, a
//! QuickTime movie on a Mac. Pictures are squeezed and written on a
//! thread of their own, so drawing doesn't wait on the disk.

use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::thread::JoinHandle;

/// Frames a second a clip is made at.
pub const FPS: u32 = crate::replay_edit::CLIP_FPS;
/// How good each JPEG picture is, of 100.
const QUALITY: u8 = 85;
/// A clip stops itself short of this (an AVI can't hold 2 GB).
const MOST_BYTES: u64 = 1_900_000_000;

/// The movie's kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Avi,
    Mov,
}

impl Kind {
    /// The kind this computer's own player opens.
    pub fn native() -> Kind {
        if cfg!(target_os = "macos") {
            Kind::Mov
        } else {
            Kind::Avi
        }
    }

    pub fn ext(self) -> &'static str {
        match self {
            Kind::Avi => "avi",
            Kind::Mov => "mov",
        }
    }
}

/// A picture as read back from the screen: 4 bytes a pixel.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// Blue first (BGRA) rather than red (RGBA).
    pub bgra: bool,
    pub pixels: Vec<u8>,
}

/// A clip being made.
pub struct Clip {
    pub path: PathBuf,
    send: Option<SyncSender<Frame>>,
    done: Option<JoinHandle<Result<u32, String>>>,
    pub frames: u32,
}

impl Clip {
    /// Starts a clip at `path`.
    pub fn start(path: &Path, kind: Kind) -> std::io::Result<Clip> {
        let file = std::io::BufWriter::new(std::fs::File::create(path)?);
        // A few pictures may wait; more and drawing waits for the disk.
        let (send, take) = sync_channel::<Frame>(4);
        let done = std::thread::spawn(move || -> Result<u32, String> {
            let mut out = Movie::new(file, kind);
            let mut size = None;
            for f in take {
                // The window's size is the clip's from its first picture;
                // pictures from after a resize are left out.
                if *size.get_or_insert((f.width, f.height)) != (f.width, f.height) {
                    continue;
                }
                let jpeg = encode(&f)?;
                if out.bytes() + jpeg.len() as u64 > MOST_BYTES {
                    break;
                }
                out.frame(&jpeg).map_err(|e| e.to_string())?;
            }
            let (w, h) = size.unwrap_or((0, 0));
            out.finish(w, h).map_err(|e| e.to_string())
        });
        Ok(Clip {
            path: path.to_path_buf(),
            send: Some(send),
            done: Some(done),
            frames: 0,
        })
    }

    /// Adds a picture; false once the clip can take no more.
    pub fn push(&mut self, frame: Frame) -> bool {
        let Some(s) = &self.send else { return false };
        if s.send(frame).is_err() {
            self.send = None;
            return false;
        }
        self.frames += 1;
        true
    }

    /// Ends the clip and waits for its last pictures to be written: how
    /// many frames it holds.
    pub fn finish(mut self) -> Result<u32, String> {
        self.send = None;
        match self.done.take().map(|d| d.join()) {
            Some(Ok(r)) => r,
            _ => Err("the clip's writer stopped".into()),
        }
    }
}

fn encode(f: &Frame) -> Result<Vec<u8>, String> {
    let (w, h) = (u16::try_from(f.width).map_err(|_| "too wide")?, u16::try_from(f.height).map_err(|_| "too tall")?);
    let mut out = Vec::with_capacity(f.pixels.len() / 10);
    let colour = if f.bgra { jpeg_encoder::ColorType::Bgra } else { jpeg_encoder::ColorType::Rgba };
    jpeg_encoder::Encoder::new(&mut out, QUALITY)
        .encode(&f.pixels, w, h, colour)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

/// A Motion JPEG movie written as it goes: the pictures first, and what
/// says where each one is once they are all in.
struct Movie<W: Write + Seek> {
    out: W,
    kind: Kind,
    /// Where each picture starts (for an AVI, from the `movi` list's
    /// name) and how long it is.
    index: Vec<(u64, u32)>,
    /// Bytes written.
    at: u64,
}

impl<W: Write + Seek> Movie<W> {
    fn new(out: W, kind: Kind) -> Self {
        Movie { out, kind, index: Vec::new(), at: 0 }
    }

    fn bytes(&self) -> u64 {
        self.at
    }

    fn put(&mut self, b: &[u8]) -> std::io::Result<()> {
        if self.at == 0 {
            // The head, sizes filled in at the end.
            let head = match self.kind {
                Kind::Avi => vec![0u8; AVI_HEAD],
                Kind::Mov => mov_head(),
            };
            self.out.write_all(&head)?;
            self.at = head.len() as u64;
        }
        self.out.write_all(b)?;
        self.at += b.len() as u64;
        Ok(())
    }

    fn frame(&mut self, jpeg: &[u8]) -> std::io::Result<()> {
        if self.at == 0 {
            self.put(&[])?;
        }
        match self.kind {
            Kind::Avi => {
                // From the "movi" name, 4 bytes into the list.
                let from = self.at - (AVI_HEAD as u64 - 4);
                self.index.push((from, jpeg.len() as u32));
                let mut chunk = Vec::with_capacity(jpeg.len() + 9);
                chunk.extend_from_slice(b"00dc");
                chunk.extend_from_slice(&(jpeg.len() as u32).to_le_bytes());
                chunk.extend_from_slice(jpeg);
                if jpeg.len() % 2 == 1 {
                    chunk.push(0);
                }
                self.put(&chunk)
            }
            Kind::Mov => {
                self.index.push((self.at, jpeg.len() as u32));
                self.put(jpeg)
            }
        }
    }

    fn finish(mut self, width: u32, height: u32) -> std::io::Result<u32> {
        if self.at == 0 {
            self.put(&[])?;
        }
        let frames = self.index.len() as u32;
        match self.kind {
            Kind::Avi => {
                let mut idx = Vec::with_capacity(8 + self.index.len() * 16);
                idx.extend_from_slice(b"idx1");
                idx.extend_from_slice(&(self.index.len() as u32 * 16).to_le_bytes());
                for &(at, len) in &self.index {
                    idx.extend_from_slice(b"00dc");
                    idx.extend_from_slice(&0x10u32.to_le_bytes());
                    idx.extend_from_slice(&(at as u32).to_le_bytes());
                    idx.extend_from_slice(&len.to_le_bytes());
                }
                let movi = self.at - (AVI_HEAD as u64 - 4);
                self.put(&idx)?;
                let total = self.at;
                let biggest = self.index.iter().map(|i| i.1).max().unwrap_or(0) + 8;
                let head = avi_head(width, height, frames, biggest, total as u32 - 8, movi as u32);
                self.out.seek(SeekFrom::Start(0))?;
                self.out.write_all(&head)?;
            }
            Kind::Mov => {
                let mdat = self.at - MOV_FTYP as u64;
                let moov = mov_moov(width, height, &self.index);
                self.put(&moov)?;
                self.out.seek(SeekFrom::Start(MOV_FTYP as u64))?;
                self.out.write_all(&(mdat as u32).to_be_bytes())?;
            }
        }
        self.out.flush()?;
        Ok(frames)
    }
}

/// The AVI's head up to and including the `movi` list's name.
const AVI_HEAD: usize = 12 + 12 + 64 + 12 + 64 + 48 + 12;

fn avi_head(w: u32, h: u32, frames: u32, biggest: u32, riff: u32, movi: u32) -> Vec<u8> {
    let mut b = Vec::with_capacity(AVI_HEAD);
    let d = |b: &mut Vec<u8>, v: u32| b.extend_from_slice(&v.to_le_bytes());
    b.extend_from_slice(b"RIFF");
    d(&mut b, riff);
    b.extend_from_slice(b"AVI LIST");
    d(&mut b, 4 + 64 + 12 + 64 + 48);
    b.extend_from_slice(b"hdrlavih");
    d(&mut b, 56);
    d(&mut b, 1_000_000 / FPS);
    d(&mut b, biggest * FPS);
    d(&mut b, 0);
    d(&mut b, 0x10); // has an index
    d(&mut b, frames);
    d(&mut b, 0);
    d(&mut b, 1); // streams
    d(&mut b, biggest);
    d(&mut b, w);
    d(&mut b, h);
    b.extend_from_slice(&[0; 16]);
    b.extend_from_slice(b"LIST");
    d(&mut b, 4 + 64 + 48);
    b.extend_from_slice(b"strlstrh");
    d(&mut b, 56);
    b.extend_from_slice(b"vidsMJPG");
    d(&mut b, 0); // flags
    d(&mut b, 0); // priority, language
    d(&mut b, 0); // initial frames
    d(&mut b, 1); // scale
    d(&mut b, FPS); // rate
    d(&mut b, 0); // start
    d(&mut b, frames);
    d(&mut b, biggest);
    d(&mut b, u32::MAX); // quality: the default
    d(&mut b, 0); // sample size
    b.extend_from_slice(&0u16.to_le_bytes());
    b.extend_from_slice(&0u16.to_le_bytes());
    b.extend_from_slice(&(w as u16).to_le_bytes());
    b.extend_from_slice(&(h as u16).to_le_bytes());
    b.extend_from_slice(b"strf");
    d(&mut b, 40);
    d(&mut b, 40);
    d(&mut b, w);
    d(&mut b, h);
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&24u16.to_le_bytes());
    b.extend_from_slice(b"MJPG");
    d(&mut b, w * h * 3);
    b.extend_from_slice(&[0; 16]);
    b.extend_from_slice(b"LIST");
    d(&mut b, movi);
    b.extend_from_slice(b"movi");
    debug_assert_eq!(b.len(), AVI_HEAD);
    b
}

/// A QuickTime movie's `ftyp` box.
const MOV_FTYP: usize = 20;

fn mov_head() -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&20u32.to_be_bytes());
    b.extend_from_slice(b"ftypqt  ");
    b.extend_from_slice(&0x2005_0300u32.to_be_bytes());
    b.extend_from_slice(b"qt  ");
    // The pictures' box, its size filled in at the end.
    b.extend_from_slice(&0u32.to_be_bytes());
    b.extend_from_slice(b"mdat");
    b
}

/// A box: its size, its name, then what's in it.
fn mbox(name: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(body.len() + 8);
    b.extend_from_slice(&(body.len() as u32 + 8).to_be_bytes());
    b.extend_from_slice(name);
    b.extend_from_slice(body);
    b
}

const MATRIX: [u32; 9] = [0x10000, 0, 0, 0, 0x10000, 0, 0, 0, 0x4000_0000];

fn mov_moov(w: u32, h: u32, index: &[(u64, u32)]) -> Vec<u8> {
    let n = index.len() as u32;
    let scale = FPS * 100;
    let length = n * 100;
    let mut be = Vec::new();
    let d = |b: &mut Vec<u8>, v: u32| b.extend_from_slice(&v.to_be_bytes());
    let s = |b: &mut Vec<u8>, v: u16| b.extend_from_slice(&v.to_be_bytes());
    // mvhd
    let mut mvhd = Vec::new();
    d(&mut mvhd, 0);
    d(&mut mvhd, 0);
    d(&mut mvhd, 0);
    d(&mut mvhd, scale);
    d(&mut mvhd, length);
    d(&mut mvhd, 0x10000);
    s(&mut mvhd, 0x100);
    mvhd.extend_from_slice(&[0; 10]);
    MATRIX.iter().for_each(|&m| d(&mut mvhd, m));
    mvhd.extend_from_slice(&[0; 24]);
    d(&mut mvhd, 2);
    // tkhd
    let mut tkhd = Vec::new();
    d(&mut tkhd, 3); // enabled, in the movie
    d(&mut tkhd, 0);
    d(&mut tkhd, 0);
    d(&mut tkhd, 1);
    d(&mut tkhd, 0);
    d(&mut tkhd, length);
    tkhd.extend_from_slice(&[0; 8]);
    s(&mut tkhd, 0);
    s(&mut tkhd, 0);
    s(&mut tkhd, 0);
    s(&mut tkhd, 0);
    MATRIX.iter().for_each(|&m| d(&mut tkhd, m));
    d(&mut tkhd, w << 16);
    d(&mut tkhd, h << 16);
    // mdhd
    let mut mdhd = Vec::new();
    d(&mut mdhd, 0);
    d(&mut mdhd, 0);
    d(&mut mdhd, 0);
    d(&mut mdhd, scale);
    d(&mut mdhd, length);
    s(&mut mdhd, 0);
    s(&mut mdhd, 0);
    let hdlr = |kind: &[u8; 4], sub: &[u8; 4]| {
        let mut b = Vec::new();
        d(&mut b, 0);
        b.extend_from_slice(kind);
        b.extend_from_slice(sub);
        b.extend_from_slice(&[0; 12]);
        b.push(0);
        mbox(b"hdlr", &b)
    };
    let mut vmhd = Vec::new();
    d(&mut vmhd, 1);
    vmhd.extend_from_slice(&[0; 8]);
    let mut dref = Vec::new();
    d(&mut dref, 0);
    d(&mut dref, 1);
    let mut alis = Vec::new();
    d(&mut alis, 1); // in this file
    dref.extend(mbox(b"alis", &alis));
    // stsd: one JPEG picture kind.
    let mut jpeg = Vec::new();
    jpeg.extend_from_slice(&[0; 6]);
    s(&mut jpeg, 1);
    s(&mut jpeg, 0);
    s(&mut jpeg, 0);
    jpeg.extend_from_slice(b"appl");
    d(&mut jpeg, 0);
    d(&mut jpeg, 0x200);
    s(&mut jpeg, w as u16);
    s(&mut jpeg, h as u16);
    d(&mut jpeg, 72 << 16);
    d(&mut jpeg, 72 << 16);
    d(&mut jpeg, 0);
    s(&mut jpeg, 1);
    let mut name = [0u8; 32];
    let label = b"Photo - JPEG";
    name[0] = label.len() as u8;
    name[1..=label.len()].copy_from_slice(label);
    jpeg.extend_from_slice(&name);
    s(&mut jpeg, 24);
    s(&mut jpeg, 0xffff);
    let mut stsd = Vec::new();
    d(&mut stsd, 0);
    d(&mut stsd, 1);
    stsd.extend(mbox(b"jpeg", &jpeg));
    let mut stts = Vec::new();
    d(&mut stts, 0);
    d(&mut stts, 1);
    d(&mut stts, n);
    d(&mut stts, 100);
    let mut stsc = Vec::new();
    d(&mut stsc, 0);
    d(&mut stsc, 1);
    d(&mut stsc, 1);
    d(&mut stsc, 1);
    d(&mut stsc, 1);
    let mut stsz = Vec::new();
    d(&mut stsz, 0);
    d(&mut stsz, 0);
    d(&mut stsz, n);
    index.iter().for_each(|&(_, len)| d(&mut stsz, len));
    let mut stco = Vec::new();
    d(&mut stco, 0);
    d(&mut stco, n);
    index.iter().for_each(|&(at, _)| d(&mut stco, at as u32));
    let stbl = [mbox(b"stsd", &stsd), mbox(b"stts", &stts), mbox(b"stsc", &stsc), mbox(b"stsz", &stsz), mbox(b"stco", &stco)].concat();
    let minf = [mbox(b"vmhd", &vmhd), hdlr(b"dhlr", b"alis"), mbox(b"dinf", &mbox(b"dref", &dref)), mbox(b"stbl", &stbl)].concat();
    let mdia = [mbox(b"mdhd", &mdhd), hdlr(b"mhlr", b"vide"), mbox(b"minf", &minf)].concat();
    let trak = [mbox(b"tkhd", &tkhd), mbox(b"mdia", &mdia)].concat();
    be.extend(mbox(b"mvhd", &mvhd));
    be.extend(mbox(b"trak", &trak));
    mbox(b"moov", &be)
}

/// Where clips go: `Clips` beside the saves.
pub fn dir() -> PathBuf {
    mm2_settings::user_dir().join("Clips")
}

/// A free name for a clip made now.
pub fn new_path(kind: Kind) -> std::io::Result<PathBuf> {
    let dir = dir();
    std::fs::create_dir_all(&dir)?;
    Ok((1..)
        .map(|n| dir.join(format!("OpenMCM2 clip {n:03}.{}", kind.ext())))
        .find(|p| !p.exists())
        .expect("a free name"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(n: u8) -> Frame {
        let (width, height) = (64, 48);
        let pixels = (0..width * height).flat_map(|i| [(i % 64) as u8 * 4, n * 20, 128, 255]).collect();
        Frame { width, height, bgra: false, pixels }
    }

    fn make(kind: Kind) -> Vec<u8> {
        let path = std::env::temp_dir().join(format!("mm2-clip-test-{}.{}", std::process::id(), kind.ext()));
        let mut c = Clip::start(&path, kind).unwrap();
        for n in 0..5 {
            assert!(c.push(picture(n)));
        }
        assert_eq!(c.finish().unwrap(), 5);
        let bytes = std::fs::read(&path).unwrap();
        // To look at in a player: MM2_CLIP_TEST_KEEP=folder.
        if let Some(keep) = std::env::var_os("MM2_CLIP_TEST_KEEP") {
            std::fs::copy(&path, Path::new(&keep).join(format!("clip-test.{}", kind.ext()))).unwrap();
        }
        let _ = std::fs::remove_file(&path);
        bytes
    }

    #[test]
    fn an_avi_says_its_sizes() {
        let b = make(Kind::Avi);
        assert_eq!(&b[..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize, b.len() - 8);
        assert_eq!(&b[AVI_HEAD - 4..AVI_HEAD], b"movi");
        // Five pictures in the head and the index.
        assert_eq!(u32::from_le_bytes(b[48..52].try_into().unwrap()), 5);
        let idx = b.windows(4).rposition(|w| w == b"idx1").unwrap();
        assert_eq!(u32::from_le_bytes(b[idx + 4..idx + 8].try_into().unwrap()), 5 * 16);
        // The first entry points at the first picture's chunk.
        let at = u32::from_le_bytes(b[idx + 16..idx + 20].try_into().unwrap()) as usize;
        assert_eq!(&b[AVI_HEAD - 4 + at..AVI_HEAD + at], b"00dc");
        assert_eq!(&b[AVI_HEAD + at + 4..AVI_HEAD + at + 6], &[0xff, 0xd8]);
    }

    #[test]
    fn a_quicktime_movie_points_at_its_pictures() {
        let b = make(Kind::Mov);
        assert_eq!(&b[4..8], b"ftyp");
        let mdat = u32::from_be_bytes(b[20..24].try_into().unwrap()) as usize;
        assert_eq!(&b[24..28], b"mdat");
        assert_eq!(&b[20 + mdat + 4..20 + mdat + 8], b"moov");
        assert_eq!(u32::from_be_bytes(b[20 + mdat..24 + mdat].try_into().unwrap()) as usize, b.len() - 20 - mdat);
        let stco = b.windows(4).rposition(|w| w == b"stco").unwrap();
        let first = u32::from_be_bytes(b[stco + 12..stco + 16].try_into().unwrap()) as usize;
        assert_eq!(first, 28);
        assert_eq!(&b[first..first + 2], &[0xff, 0xd8]);
    }
}
