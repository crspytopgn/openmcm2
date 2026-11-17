//! The Free Pack's own tracks. They are not downloaded: each is a recipe
//! (`mm2_course::free::SET`, a seed and a style) that is built on the
//! player's machine the first time it is ridden or copied, from the
//! recipe and the pack's shared ground pictures (`ground/*.png`).
//!
//! They are listed with an event's tracks whenever the free pack is in
//! use, and built into the game's `TERAFORM/<event>` folder (without the
//! game, the Course Builder's `Courses/TERAFORM`, where its own tracks go).
//! In a browser the game folder lasts one visit, so a built track is kept
//! among the saves (`/user/FreeTracks`) and put back from there.

use mm2_course::free::{self, Def, Kind};
use std::path::{Path, PathBuf};

/// The `TERAFORM` folder the free tracks are built into.
pub fn home(game: Option<&Path>) -> PathBuf {
    match game {
        Some(g) => g.join("TERAFORM"),
        None => crate::profile::user_dir().join("Courses").join("TERAFORM"),
    }
}

/// The free tracks of an event folder (`NATIONAL`, `SX` ...) and where
/// each one goes, when the free pack is in use.
pub fn listed(teraform: &Path, folder: &str) -> Vec<(PathBuf, &'static Def)> {
    if crate::freepack::root().is_none() {
        return Vec::new();
    }
    free::SET
        .iter()
        .filter(|d| d.kind.folder().eq_ignore_ascii_case(folder))
        .map(|d| (teraform.join(d.kind.folder()).join(format!("{}.ENV", d.file)), d))
        .collect()
}

/// The courses a free Baja or enduro track has, by name.
pub fn courses(def: &Def) -> Vec<String> {
    match def.kind {
        Kind::Baja | Kind::Enduro => free::race_names(def),
        _ => Vec::new(),
    }
}

/// Builds the free track `path` names if it isn't there yet. Anything
/// else is left alone.
pub fn ensure(path: &Path) -> Result<(), String> {
    let Some(def) = path.file_stem().and_then(|s| s.to_str()).and_then(free::find) else {
        return Ok(());
    };
    let Some(dir) = path.parent() else { return Ok(()) };
    // Built, and by this making of the recipes (an older one's is built
    // again, so a fix to a track reaches it).
    let current = |dir: &Path| {
        crate::fs::is_file(&dir.join(format!("{}.ENV", def.file)))
            && crate::fs::read(dir.join(format!("{}.made", def.file))).is_ok_and(|m| m == free::MADE.as_bytes())
    };
    if current(dir) {
        return Ok(());
    }
    #[cfg(target_arch = "wasm32")]
    let kept = PathBuf::from("/user/FreeTracks").join(def.kind.folder());
    #[cfg(target_arch = "wasm32")]
    if current(&kept) {
        for name in [format!("{}.ENV", def.file), format!("{}.lights", def.file), format!("{}.made", def.file)] {
            if let Ok(bytes) = crate::fs::read(kept.join(&name)) {
                let _ = crate::fs::write(dir.join(&name), bytes);
            }
        }
        if current(dir) {
            return Ok(());
        }
    }
    let ground = crate::freepack::root().map(|r| r.join("ground"));
    let read = |name: &str| crate::fs::read(ground.as_ref()?.join(name)).ok();
    let started = web_time::Instant::now();
    let made = free::make(def, &read).map_err(|e| format!("{}: {e}", def.file))?;
    let _ = crate::fs::create_dir_all(dir);
    for (name, bytes) in &made {
        crate::fs::write(dir.join(name), bytes).map_err(|e| format!("{}: {e}", dir.join(name).display()))?;
        #[cfg(target_arch = "wasm32")]
        let _ = crate::fs::write(kept.join(name), bytes);
    }
    eprintln!("built the free track {} ({}) in {:.1} s", def.file, def.name, started.elapsed().as_secs_f32());
    Ok(())
}
