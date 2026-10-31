//! The player's choices (see [`mm2_settings`]), and where the game is.

pub use mm2_settings::*;
use std::path::PathBuf;

/// `mm2-viewer` beside the launcher (as the release zips and the Mac app
/// put it), or in the cargo target folder when run from a checkout.
pub fn viewer() -> Option<PathBuf> {
    let name = format!("mm2-viewer{}", std::env::consts::EXE_SUFFIX);
    let exe = std::env::current_exe().ok()?;
    let here = exe.parent()?;
    [here.join(&name), here.join("../Resources").join(&name)]
        .into_iter()
        .find(|p| p.is_file())
}
