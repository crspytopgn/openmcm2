//! Getting the Motocross Madness 2 demo for a player without the game:
//! Microsoft's free 2000 demo, downloaded from the Internet Archive when
//! the player asks (never shipped with OpenMCM2), checked, and unpacked
//! into a game folder (`mm2_formats::demo`). With the Free Pack beside it,
//! the game fills in everything the demo lacks.

use crate::freepack::{self, Part, Progress};
use mm2_formats::demo;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// Downloads the demo (or uses `exe`, one the player already has) and
/// unpacks it into `dest`, replacing what's there, on a thread of its own.
/// The result is the game folder.
pub fn install(exe: Option<PathBuf>, dest: PathBuf) -> Arc<Progress> {
    let progress = Arc::new(Progress::default());
    let p = progress.clone();
    std::thread::spawn(move || {
        let r = run(exe, &dest, &p);
        *p.result.lock().unwrap() = Some(r.map(|_| dest));
    });
    progress
}

fn run(exe: Option<PathBuf>, dest: &Path, p: &Progress) -> Result<(), String> {
    let file = match exe {
        Some(f) => f,
        None => {
            let to = dest.with_file_name("MCM2PCG.exe");
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
            }
            let part = Part {
                name: "demo".into(),
                file: "MCM2PCG.exe".into(),
                size: demo::SIZE,
                sha256: demo::SHA256.into(),
            };
            p.total.store(demo::SIZE, Ordering::Relaxed);
            let stage = "Downloading the Motocross Madness 2 demo from the Internet Archive";
            *p.stage.lock().unwrap() = format!("{stage}…");
            freepack::download(demo::URL, &part, &to, p, stage)?;
            to
        }
    };
    *p.stage.lock().unwrap() = "Unpacking the demo…".into();
    let bytes = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    p.total.store(1, Ordering::Relaxed);
    p.done.store(0, Ordering::Relaxed);
    let files = demo::unpack(&bytes, |done, of| {
        p.total.store(of.max(1) as u64, Ordering::Relaxed);
        p.done.store(done as u64, Ordering::Relaxed);
    })
    .map_err(|e| e.to_string())?;
    if p.cancel.load(Ordering::Relaxed) {
        return Err("cancelled".into());
    }
    let tmp = dest.with_extension("part");
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    }
    for (rel, data) in &files {
        let out = tmp.join(rel);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&out, data).map_err(|e| format!("{}: {e}", out.display()))?;
    }
    // The game keeps its profiles and saves here, as in a real install.
    std::fs::create_dir_all(tmp.join("UI").join("PROFILE")).map_err(|e| e.to_string())?;
    if dest.exists() {
        std::fs::remove_dir_all(dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    }
    std::fs::rename(&tmp, dest).map_err(|e| format!("{}: {e}", dest.display()))
}

/// Whether `path` is the demo's installer, so the CD picker can take it.
pub fn is_installer(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe"))
        && std::fs::metadata(path).is_ok_and(|m| m.len() == demo::SIZE)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With `MM2_DEMO_DOWNLOAD=1`: the real download and unpack, into a
    /// folder of its own.
    #[test]
    fn downloads_and_unpacks() {
        if std::env::var_os("MM2_DEMO_DOWNLOAD").is_none() {
            return;
        }
        let dir = std::env::temp_dir().join("openmcm2-demo-test");
        let _ = std::fs::remove_dir_all(&dir);
        let game = dir.join("GAME");
        let p = install(None, game.clone());
        let r = loop {
            if let Some(r) = p.result.lock().unwrap().take() {
                break r;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        };
        assert_eq!(r.unwrap(), game);
        assert!(mm2_settings::is_demo(&game));
        assert!(game.join("TERAFORM/NATIONAL/NAT04.ENV").is_file());
        assert!(crate::game::missing(&game).is_empty());
    }
}
