//! The demo's other tracks, rebuilt (an extra, `--demo-tracks`, off by
//! default). The MM2 demo keeps, for each track it doesn't let you ride,
//! the scene, the route and the menu pictures but not the ground; the
//! unpacked demo has them under `DEMO/TERAFORM` (`mm2_formats::demo`).
//! With the extra on they are listed with each event's tracks, and each
//! is rebuilt the first time it is ridden (`mm2_course::rebuild`): the
//! original's route and scenery on new ground, painted from the Free
//! Pack's ground pictures, into the game's own `TERAFORM` folder. In a
//! browser the game folder lasts one visit, so a built track is kept among
//! the saves (`/user/DemoTracks`) and put back from there.

use mm2_course::rebuild;
use std::path::{Path, PathBuf};

/// Where the demo's ground-less tracks are, in the game folder.
const STUBS: &str = mm2_formats::demo::STUBS;

/// The file beside a rebuilt track saying which making of the rebuild it
/// is, so a fix reaches tracks built before it.
fn marker(path: &Path) -> PathBuf {
    path.with_extension("rebuilt")
}

/// The demo's track behind `path` (`<game>/TERAFORM/<event>/<NAME>.ENV`),
/// when there is one.
fn stub_for(path: &Path) -> Option<PathBuf> {
    let file = path.file_name()?;
    let event = path.parent()?;
    let teraform = event.parent()?;
    let game = teraform.parent()?;
    let dir = game.join(STUBS).join(teraform.file_name()?).join(event.file_name()?);
    let stub = dir.join(file);
    crate::fs::is_file(&stub).then_some(stub)
}

/// Whether the extra is on, with the demo as the game.
pub fn on() -> bool {
    crate::demo::active() && crate::tweaks::more().demo_tracks
}

/// A demo unpacked before its ground-less tracks were kept (OpenMCM2
/// 0.7.0): they are taken from the installer, which the launcher keeps
/// beside the game folder, once.
pub fn unpack_stubs(game: &Path) {
    static DONE: std::sync::Once = std::sync::Once::new();
    DONE.call_once(|| {
        if crate::fs::is_dir(&game.join(STUBS)) {
            return;
        }
        let Some(exe) = game.parent().map(|p| p.join("MCM2PCG.exe")).filter(|p| crate::fs::is_file(p)) else { return };
        let Ok(bytes) = crate::fs::read(&exe) else { return };
        match mm2_formats::demo::unpack(&bytes, |_, _| {}) {
            Ok(files) => {
                for (path, data) in files.iter().filter(|(p, _)| p.starts_with(&format!("{STUBS}/"))) {
                    let out = game.join(path);
                    if let Some(dir) = out.parent() {
                        let _ = crate::fs::create_dir_all(dir);
                    }
                    let _ = crate::fs::write(&out, data);
                }
            }
            Err(e) => eprintln!("the demo's other tracks: {e}"),
        }
    });
}

/// A rebuilt track the event list shows: where it goes once built, the
/// demo's own file, and its races' names.
pub struct Listed {
    pub path: PathBuf,
    pub stub: PathBuf,
    pub names: Vec<String>,
}

/// The demo's tracks of an event folder (`<game>/TERAFORM/SX`) that can be
/// rebuilt, with the extra on.
pub fn listed(folder: &Path) -> Vec<Listed> {
    if !on() {
        return Vec::new();
    }
    let dir = (|| {
        let event = folder.file_name()?;
        let teraform = folder.parent()?;
        Some(teraform.parent()?.join(STUBS).join(teraform.file_name()?).join(event))
    })();
    let Some(dir) = dir else { return Vec::new() };
    if let Some(game) = dir.parent().and_then(Path::parent).and_then(Path::parent) {
        unpack_stubs(game);
    }
    let mut files: Vec<PathBuf> = crate::fs::read_dir(&dir)
        .map(|d| {
            d.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("env")))
                .collect()
        })
        .unwrap_or_default();
    files.sort_by_key(|p| p.file_name().map(|n| n.to_ascii_uppercase()));
    let mut out = Vec::new();
    for stub in files {
        let Ok(bytes) = crate::fs::read(&stub) else { continue };
        let name = stub.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if !rebuild::can_rebuild(&bytes, &name) {
            continue;
        }
        let names = rebuild::names(&bytes, &name);
        out.push(Listed { path: folder.join(name.to_ascii_uppercase()), stub, names });
    }
    out
}

/// Builds the rebuilt track `path` names if it isn't there yet (or was
/// built by an older making). Anything else is left alone.
pub fn ensure(path: &Path) -> Result<(), String> {
    // Only listed with the extra on, so asked for only then.
    let Some(stub) = stub_for(path) else { return Ok(()) };
    let current = |p: &Path| crate::fs::is_file(p) && crate::fs::read(marker(p)).is_ok_and(|m| m == rebuild::MADE.as_bytes());
    if current(path) {
        return Ok(());
    }
    let file = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    #[cfg(target_arch = "wasm32")]
    let kept = PathBuf::from("/user/DemoTracks").join(path.parent().and_then(|p| p.file_name()).unwrap_or_default()).join(&file);
    #[cfg(target_arch = "wasm32")]
    if current(&kept) {
        if let Ok(bytes) = crate::fs::read(&kept) {
            let _ = crate::fs::write(path, bytes);
            let _ = crate::fs::write(marker(path), rebuild::MADE.as_bytes());
        }
        if current(path) {
            return Ok(());
        }
    }
    let started = web_time::Instant::now();
    let bytes = crate::fs::read(&stub).map_err(|e| format!("{}: {e}", stub.display()))?;
    let env = rebuild::rebuild_env(&bytes, &file, &crate::fine_ground::builder_pictures())?;
    if let Some(dir) = path.parent() {
        let _ = crate::fs::create_dir_all(dir);
    }
    crate::fs::write(path, &env).map_err(|e| format!("{}: {e}", path.display()))?;
    let _ = crate::fs::write(marker(path), rebuild::MADE.as_bytes());
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(dir) = kept.parent() {
            let _ = crate::fs::create_dir_all(dir);
        }
        let _ = crate::fs::write(&kept, &env);
        let _ = crate::fs::write(marker(&kept), rebuild::MADE.as_bytes());
    }
    eprintln!("rebuilt the demo's track {file} in {:.1} s", started.elapsed().as_secs_f32());
    Ok(())
}

/// The menu picture `preview` of a rebuilt track not built yet, from the
/// demo's own file, without its "Track unavailable" line.
pub fn preview(path: &Path, preview: &str) -> Option<Vec<u8>> {
    let stub = stub_for(path)?;
    let bytes = crate::read_archive(&stub).ok()?;
    let ar = mm2_formats::rs2::Archive::parse(&bytes).ok()?;
    let e = ar.entries().iter().find(|e| e.name.eq_ignore_ascii_case(preview))?;
    let data = ar.data(e);
    Some(rebuild::cleaned_picture(data).unwrap_or_else(|| data.to_vec()))
}
