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
}

impl Source {
    /// Works out what `path` is.
    pub fn of(path: &Path) -> Result<Source, String> {
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
