//! The Motocross Madness 2 demo as the game folder (`mm2_formats::demo`):
//! its own two tracks, menus, bikes and sounds with the original look, and
//! the Free Pack beside it (`--fill-from`) for everything it lacks. Every
//! event, bike and outfit the demo has is open: nothing is locked.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

static ON: AtomicBool = AtomicBool::new(false);

/// Notes whether `game` is an unpacked demo (it has the demo's marker).
pub fn note(game: &Path) {
    ON.store(crate::fs::is_file(game.join(mm2_formats::demo::MARKER)), Ordering::Relaxed);
}

/// Whether the game folder is the demo.
pub fn active() -> bool {
    ON.load(Ordering::Relaxed)
}

/// The names (lower case) in the game's `RES/BIKE.RES`, for the bikes and
/// outfits the demo really has: its lists name the whole game's.
pub fn bike_files(game: &Path) -> Option<std::collections::HashSet<String>> {
    let bytes = crate::read_archive(&game.join("RES").join("BIKE.RES")).ok()?;
    let ar = mm2_formats::rs2::Archive::parse(&bytes).ok()?;
    Some(ar.entries().iter().map(|e| e.name.to_ascii_lowercase()).collect())
}

/// Where the browser page puts the demo's installer it downloaded.
#[cfg(target_arch = "wasm32")]
const INSTALLER: &str = "/demo/MCM2PCG.exe";

/// In a browser: the demo's installer the page kept, unpacked into the game
/// folder (`/game`) for this visit, when there is one and no game.
#[cfg(target_arch = "wasm32")]
pub fn unpack_in_browser() {
    if !crate::fs::is_file(INSTALLER) || crate::fs::is_file("/game/UI/MAINMENU.DAT") {
        return;
    }
    crate::web::stage("demo");
    let Ok(exe) = crate::fs::read(INSTALLER) else {
        return;
    };
    match mm2_formats::demo::unpack(&exe, |_, _| {}) {
        Ok(files) => {
            for (path, data) in files {
                crate::fs::put_game(format!("/game/{path}"), &data);
            }
        }
        Err(e) => eprintln!("the demo: {e}"),
    }
}
