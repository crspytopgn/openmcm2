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

/// What the event lists add to a locked track's name.
pub const FULL_GAME: &str = "(Full Game)";

/// The full game's tracks of an event the demo lists but doesn't let you
/// ride, as the demo's own event lists did: its ground-less copies under
/// `DEMO/TERAFORM`, each with its menu picture saying "Track
/// unavailable". Start on one says which tracks the trial has
/// ([`locked_note`]). None with the rebuilt tracks extra on: those are
/// listed to ride instead (`demo_tracks`).
pub fn locked(game: &Path, event: crate::frontend::EventKind) -> Vec<crate::frontend::TrackEntry> {
    use crate::frontend::{EventKind, TrackEntry};
    if !active() || crate::demo_tracks::on() || event == EventKind::Tag {
        return Vec::new();
    }
    crate::demo_tracks::unpack_stubs(game);
    let dir = game.join(mm2_formats::demo::STUBS).join(event.folder());
    let mut files: Vec<std::path::PathBuf> = crate::fs::read_dir(&dir)
        .map(|d| {
            d.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("env")))
                .collect()
        })
        .unwrap_or_default();
    files.sort_by_key(|p| p.file_name().map(|n| n.to_ascii_uppercase()));
    files
        .into_iter()
        .filter_map(|path| {
            let file = path.file_name()?.to_string_lossy().to_string();
            let stem = path.file_stem()?.to_string_lossy().to_string();
            let bytes = crate::fs::read(&path).ok()?;
            let name = mm2_course::rebuild::names(&bytes, &file).into_iter().next()?;
            let name = name.trim_end_matches(mm2_course::rebuild::LABEL).trim_end().to_string();
            let preview = if event.courses() { format!("{stem}01.tga") } else { format!("{stem}.tga") };
            Some(TrackEntry { path, course: 0, name, preview })
        })
        .collect()
}

/// Whether `path` is one of the demo's tracks it doesn't let you ride.
pub fn is_locked(path: &Path) -> bool {
    let mut parts = path.components().rev().map(|c| c.as_os_str().to_string_lossy().to_ascii_uppercase());
    // <game>/DEMO/TERAFORM/<event>/<NAME>.ENV
    let (_, _, teraform, demo) = (parts.next(), parts.next(), parts.next(), parts.next());
    active() && teraform.as_deref() == Some("TERAFORM") && demo.as_deref() == Some(mm2_formats::demo::STUBS)
}

/// The box Start on a locked track shows (the demo said "You must choose
/// one of the following trial version tracks"): its title and words.
pub fn locked_note() -> (&'static str, &'static str) {
    (
        "Trial Version",
        "This track comes with the full game. The trial version has Donner Pass (Stunts) and \
         A Voodoo Basin (Nationals), plus our open source tracks in every event. For more, load \
         your Motocross Madness 2 files or get the starter set of player-made tracks.",
    )
}

/// The main menu's event buttons' tips with the demo as the game: what
/// each event has to ride in the trial version.
pub fn event_tips() -> [(&'static str, &'static str); 6] {
    const NOT_IN: &str = "Not in the trial version: our open source tracks, or load your MM2 files";
    [
        ("ButStuntQuarry", "Donner Pass from the trial version, and the full game's quarries"),
        ("ButNationals", "A Voodoo Basin from the trial version, and the full game's tracks"),
        ("ButBaja", NOT_IN),
        ("ButEnduro", NOT_IN),
        ("ButSupercross", NOT_IN),
        ("ButProCircuit", "A career on the trial version's tracks and our open source ones"),
    ]
}
