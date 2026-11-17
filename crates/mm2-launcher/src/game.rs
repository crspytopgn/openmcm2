//! Finding the game data in the player's own copy of Motocross Madness 2
//! and copying it where the port reads it. Nothing is ever downloaded:
//! the data comes only from a CD, CD image or install the player has.

use crate::iso::Image;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Files the port can't start without, from the game folder.
pub const NEEDED: [&str; 6] =
    ["UI/MAINMENU.DAT", "UI/GLOBAL.DAT", "RES/ENGINE.RES", "UILANG.DLL", "LANG.DLL", "TERAFORM"];

/// The files from [`NEEDED`] missing from the game folder `dir`.
pub fn missing(dir: &Path) -> Vec<&'static str> {
    NEEDED.iter().copied().filter(|f| find_ci(dir, f).is_none()).collect()
}

/// `rel` under `dir`, matching each part's case loosely (a CD copied on
/// a case-sensitive disk can be in any case).
pub fn find_ci(dir: &Path, rel: &str) -> Option<PathBuf> {
    let mut here = dir.to_path_buf();
    for part in rel.split('/') {
        let exact = here.join(part);
        here = if exact.exists() {
            exact
        } else {
            std::fs::read_dir(&here)
                .ok()?
                .flatten()
                .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(part))?
                .path()
        };
    }
    Some(here)
}

/// Where the game data is in what the player picked.
pub enum Source {
    /// A folder that is the game itself (an install): used where it is.
    Installed(PathBuf),
    /// A CD or a copy of one: its `GAME` folder is copied.
    CdFolder(PathBuf),
    /// A CD image: its `GAME` folder is copied out.
    Image(PathBuf),
    /// A zip (an abandonware download, a backup): an install or a CD
    /// folder in it is copied out, or a CD image in it is.
    Zip(PathBuf),
}

/// The disc image files a zip may hold.
const IMAGES: [&str; 5] = [".iso", ".mdf", ".bin", ".img", ".nrg"];

impl Source {
    /// Works out what `path` is.
    pub fn of(path: &Path) -> Result<Source, String> {
        if path.is_file() && path.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
            let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
            let zip = zip::ZipArchive::new(file).map_err(|e| format!("{}: {e}", path.display()))?;
            let names: Vec<String> = zip.file_names().map(|n| n.map(|n| n.to_ascii_lowercase()).unwrap_or_default()).collect();
            if names.iter().any(|n| n.ends_with("ui/mainmenu.dat") || IMAGES.iter().any(|i| n.ends_with(i))) {
                return Ok(Source::Zip(path.to_path_buf()));
            }
            return Err("this zip has no Motocross Madness 2 in it (no UI/MAINMENU.DAT and no CD image)".into());
        }
        if path.is_file() {
            let mut img = Image::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
            return match img.find("GAME/UI/MAINMENU.DAT") {
                Ok(Some(_)) => Ok(Source::Image(path.to_path_buf())),
                _ => Err("this CD image isn't a Motocross Madness 2 CD (no GAME/UI/MAINMENU.DAT)".into()),
            };
        }
        if find_ci(path, "UI/MAINMENU.DAT").is_some() {
            return Ok(Source::Installed(path.to_path_buf()));
        }
        if let Some(game) = find_ci(path, "GAME").filter(|g| find_ci(g, "UI/MAINMENU.DAT").is_some()) {
            return Ok(Source::CdFolder(game));
        }
        Err("no Motocross Madness 2 game data here: pick the CD, a CD image (.iso, .bin, .cue, .img) or the folder the game is installed in".into())
    }
}

/// How a copy is going, shared with the window.
#[derive(Default)]
pub struct Progress {
    pub done: AtomicU64,
    pub total: AtomicU64,
    pub cancel: AtomicBool,
    /// The end: the game folder, or why it failed.
    pub result: Mutex<Option<Result<PathBuf, String>>>,
}

/// Copies the game from `source` into `dest` (replacing what's there) on
/// a thread of its own.
pub fn import(source: Source, dest: PathBuf) -> Arc<Progress> {
    let progress = Arc::new(Progress::default());
    let p = progress.clone();
    std::thread::spawn(move || {
        let r = match source {
            Source::Installed(dir) => Ok(dir),
            Source::CdFolder(game) => copy_folder(&game, &dest, &p).map(|_| dest.clone()),
            Source::Image(img) => extract(&img, &dest, &p).map(|_| dest.clone()),
            Source::Zip(zip) => unzip(&zip, &dest, &p).map(|_| dest.clone()),
        };
        let r = r.map_err(|e| e.to_string()).and_then(|dir| {
            let gone = missing(&dir);
            if gone.is_empty() {
                Ok(dir)
            } else {
                Err(format!("the game data is incomplete, missing {}", gone.join(", ")))
            }
        });
        *p.result.lock().unwrap() = Some(r);
    });
    progress
}

fn cancelled() -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, "cancelled")
}

/// A fresh, empty `dest`, its parents made.
fn fresh(dest: &Path) -> io::Result<()> {
    if dest.exists() {
        std::fs::remove_dir_all(dest)?;
    }
    std::fs::create_dir_all(dest)
}

fn extract(path: &Path, dest: &Path, p: &Progress) -> io::Result<()> {
    let mut img = Image::open(path)?;
    let game = img.find("GAME")?.ok_or_else(|| io::Error::other("no GAME folder on the CD"))?;
    let files = img.walk(&game)?;
    p.total.store(files.iter().filter(|(_, e)| !e.dir).map(|(_, e)| e.size).sum(), Ordering::Relaxed);
    let tmp = dest.with_extension("part");
    fresh(&tmp)?;
    for (rel, e) in &files {
        if p.cancel.load(Ordering::Relaxed) {
            return Err(cancelled());
        }
        let out = tmp.join(rel);
        if e.dir {
            std::fs::create_dir_all(&out)?;
        } else {
            std::fs::write(&out, img.read_file(e)?)?;
            p.done.fetch_add(e.size, Ordering::Relaxed);
        }
    }
    finish(&tmp, dest)
}

/// Copies the game out of a zip: the folder holding `UI/MAINMENU.DAT`,
/// or else the biggest CD image in it, unpacked beside `dest` first.
fn unzip(path: &Path, dest: &Path, p: &Progress) -> io::Result<()> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path)?).map_err(io::Error::other)?;
    let names: Vec<String> = zip.file_names().map(|n| n.map(|n| n.into_owned()).unwrap_or_default()).collect();
    let root = names.iter().find_map(|n| {
        let l = n.to_ascii_lowercase().replace('\\', "/");
        l.strip_suffix("ui/mainmenu.dat").map(|r| n[..r.len()].to_string())
    });
    if let Some(root) = root {
        return unzip_under(&mut zip, &names, &root, dest, p);
    }
    // A CD image: unpacked to a file of its own, then read as any image.
    let image = (0..zip.len())
        .filter(|&i| IMAGES.iter().any(|e| names[i].to_ascii_lowercase().ends_with(e)))
        .max_by_key(|&i| zip.by_index(i).map(|f| f.size()).unwrap_or(0))
        .ok_or_else(|| io::Error::other("no game folder or CD image in the zip"))?;
    let mut f = zip.by_index(image).map_err(io::Error::other)?;
    p.total.store(f.size(), Ordering::Relaxed);
    let unpacked = dest.with_extension("image");
    {
        let mut out = std::fs::File::create(&unpacked)?;
        let mut buf = vec![0u8; 1 << 20];
        loop {
            if p.cancel.load(Ordering::Relaxed) {
                drop(out);
                let _ = std::fs::remove_file(&unpacked);
                return Err(cancelled());
            }
            let n = io::Read::read(&mut f, &mut buf)?;
            if n == 0 {
                break;
            }
            io::Write::write_all(&mut out, &buf[..n])?;
            p.done.fetch_add(n as u64, Ordering::Relaxed);
        }
    }
    p.done.store(0, Ordering::Relaxed);
    let r = extract(&unpacked, dest, p);
    let _ = std::fs::remove_file(&unpacked);
    r
}

/// Copies the zip's files under `root` into `dest`.
fn unzip_under(zip: &mut zip::ZipArchive<std::fs::File>, names: &[String], root: &str, dest: &Path, p: &Progress) -> io::Result<()> {
    let files: Vec<usize> = (0..zip.len()).filter(|&i| names[i].starts_with(root) && !names[i].ends_with('/')).collect();
    let mut total = 0;
    for &i in &files {
        total += zip.by_index(i).map_err(io::Error::other)?.size();
    }
    p.total.store(total, Ordering::Relaxed);
    let tmp = dest.with_extension("part");
    fresh(&tmp)?;
    for i in files {
        if p.cancel.load(Ordering::Relaxed) {
            return Err(cancelled());
        }
        let mut f = zip.by_index(i).map_err(io::Error::other)?;
        let rel = names[i][root.len()..].replace('\\', "/");
        // Only plain paths inside the folder.
        if rel.split('/').any(|part| part == ".." || part.is_empty()) {
            continue;
        }
        let out = tmp.join(&rel);
        if let Some(dir) = out.parent() {
            std::fs::create_dir_all(dir)?;
        }
        io::copy(&mut f, &mut std::fs::File::create(&out)?)?;
        p.done.fetch_add(f.size(), Ordering::Relaxed);
    }
    finish(&tmp, dest)
}

/// Whether `dir` is a free pack: its `stockmap.toml` is there.
pub fn is_pack(dir: &Path) -> bool {
    dir.join("stockmap.toml").is_file()
}

/// Sets up the free pack from `path`: a pack folder is used where it is;
/// a zip is unpacked into `dest` (the folder holding `stockmap.toml`), on
/// a thread of its own.
pub fn import_pack(path: PathBuf, dest: PathBuf) -> Arc<Progress> {
    let progress = Arc::new(Progress::default());
    let p = progress.clone();
    std::thread::spawn(move || {
        let r = if path.is_dir() {
            if is_pack(&path) {
                Ok(path)
            } else {
                Err("that folder isn't the Free Pack (it has no stockmap.toml)".to_string())
            }
        } else {
            unzip_pack(&path, &dest, &p).map(|_| dest.clone()).map_err(|e| e.to_string())
        };
        *p.result.lock().unwrap() = Some(r);
    });
    progress
}

fn unzip_pack(path: &Path, dest: &Path, p: &Progress) -> io::Result<()> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path)?).map_err(io::Error::other)?;
    let names: Vec<String> = zip.file_names().map(|n| n.map(|n| n.into_owned()).unwrap_or_default()).collect();
    // The shallowest stockmap.toml: the pack's own folder.
    const MAP: &str = "stockmap.toml";
    let root = names
        .iter()
        .filter(|n| n.to_ascii_lowercase().ends_with(MAP))
        .min_by_key(|n| n.len())
        .map(|n| n[..n.len() - MAP.len()].to_string())
        .ok_or_else(|| io::Error::other("this zip isn't the Free Pack (it has no stockmap.toml)"))?;
    unzip_under(&mut zip, &names, &root, dest, p)
}

fn copy_folder(from: &Path, dest: &Path, p: &Progress) -> io::Result<()> {
    let mut files = Vec::new();
    let mut todo = vec![PathBuf::new()];
    while let Some(rel) = todo.pop() {
        for e in std::fs::read_dir(from.join(&rel))? {
            let e = e?;
            let r = rel.join(e.file_name());
            if e.file_type()?.is_dir() {
                todo.push(r.clone());
                files.push((r, None));
            } else {
                files.push((r, Some(e.metadata()?.len())));
            }
        }
    }
    files.sort();
    p.total.store(files.iter().filter_map(|f| f.1).sum(), Ordering::Relaxed);
    let tmp = dest.with_extension("part");
    fresh(&tmp)?;
    for (rel, size) in &files {
        if p.cancel.load(Ordering::Relaxed) {
            return Err(cancelled());
        }
        match size {
            None => std::fs::create_dir_all(tmp.join(rel))?,
            Some(n) => {
                std::fs::copy(from.join(rel), tmp.join(rel))?;
                // Files off a CD are read-only; the game writes saves here.
                let mut perm = std::fs::metadata(tmp.join(rel))?.permissions();
                #[allow(clippy::permissions_set_readonly_false)]
                perm.set_readonly(false);
                std::fs::set_permissions(tmp.join(rel), perm)?;
                p.done.fetch_add(*n, Ordering::Relaxed);
            }
        }
    }
    finish(&tmp, dest)
}

/// Puts the finished copy in place of any older one.
fn finish(tmp: &Path, dest: &Path) -> io::Result<()> {
    if dest.exists() {
        std::fs::remove_dir_all(dest)?;
    }
    std::fs::rename(tmp, dest)
}
