//! The Motocross Madness 2 demo, the one-file installer Microsoft gave
//! away in 2000 (`MCM2PCG.exe`, kept by the Internet Archive). It holds a
//! Microsoft cabinet with a cut-down game folder: the menus, two whole
//! tracks (the National `NAT04` and the Stunt Quarry `QUARRY01`) and,
//! for every other track, only its scene and course, without the ground.
//!
//! [`unpack`] turns the installer into a game folder the port can read,
//! with the names in upper case like the CD's. The tracks that have no
//! ground go under [`STUBS`] rather than `TERAFORM`, out of the game's
//! track lists; the port can rebuild them (`mm2_course::rebuild`). Nothing of the demo is shipped with the port: the
//! launcher and the browser page fetch it from [`URL`] when the player
//! asks, and check it against [`SIZE`] and [`SHA256`].

use crate::{faoe, rs2, Error, Result};
use std::io::{Cursor, Read};

/// Where the demo is kept.
pub const URL: &str = "https://archive.org/download/MotocrossMadness2Demo/MCM2PCG.exe";
/// The same file, with the headers a web page needs to read it.
pub const BROWSER_URL: &str = "https://cors.archive.org/cors/MotocrossMadness2Demo/MCM2PCG.exe";
/// The installer's size in bytes.
pub const SIZE: u64 = 42_954_810;
/// Its SHA-256, in lower-case hex.
pub const SHA256: &str = "383eae1ccac90b478d89864c0a6142407fab6b151c7b84d7f7db52121ac4125a";

/// Where an unpacked demo keeps the tracks it has no ground for, as
/// `DEMO/TERAFORM/<event>/<NAME>.ENV`.
pub const STUBS: &str = "DEMO";

/// The file an unpacked demo folder has, which tells the game that it is
/// the demo (`DEMO.TXT`).
pub const MARKER: &str = "DEMO.TXT";

/// What goes in [`MARKER`].
const MARKER_TEXT: &str = "The Motocross Madness 2 demo, unpacked by OpenMCM2 from MCM2PCG.exe.\r\n\
Its tracks, menus and art are Microsoft's. The rest comes from the Free Pack.\r\n";

/// The parts of the installer's cabinet that are the game: the rest is
/// the installer itself, its help and its uninstaller.
fn wanted(path: &str) -> bool {
    const DIRS: [&str; 3] = ["RES/", "TERAFORM/", "UI/"];
    const FILES: [&str; 7] =
        ["MCM2.EXE", "LANG.DLL", "UILANG.DLL", "BLADE.DLL", "BLANK.CUR", "PARTBLT.TGA", "MCM2HELP.HLP"];
    DIRS.iter().any(|d| path.starts_with(d)) || FILES.contains(&path)
}

/// Whether the installer looks like the demo: a Windows program with a
/// cabinet inside. Cheap; [`unpack`] does the real check.
pub fn looks_like(bytes: &[u8]) -> bool {
    bytes.starts_with(b"MZ") && cabinets(bytes).next().is_some()
}

/// Where cabinets may start in `bytes`: each `MSCF` whose header's size
/// fits in what follows.
fn cabinets(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    (0..bytes.len().saturating_sub(36)).filter_map(move |i| {
        if &bytes[i..i + 4] != b"MSCF" || bytes[i + 4..i + 8] != [0; 4] {
            return None;
        }
        let size = u32::from_le_bytes(bytes[i + 8..i + 12].try_into().unwrap()) as usize;
        (size >= 36 && i + size <= bytes.len()).then(|| &bytes[i..i + size])
    })
}

/// Whether a track archive has its ground (`.trn`): the demo's locked
/// tracks only have the scene, the course and the preview pictures.
fn has_ground(env: &[u8], name: &str) -> bool {
    let Ok(plain) = faoe::decode(env, name) else {
        return false;
    };
    rs2::Archive::parse(&plain)
        .is_ok_and(|ar| ar.entries().iter().any(|e| e.name.to_ascii_lowercase().ends_with(".trn")))
}

/// The game folder in the demo installer `exe`: each file's path (upper
/// case, `/` between folders) and bytes, [`MARKER`] among them. `progress`
/// hears how many of the cabinet's files are done, out of how many.
pub fn unpack(exe: &[u8], mut progress: impl FnMut(usize, usize)) -> Result<Vec<(String, Vec<u8>)>> {
    const NOT_DEMO: Error = Error::Corrupt("this isn't the Motocross Madness 2 demo (MCM2PCG.exe)");
    for cab in cabinets(exe) {
        let Ok(mut cabinet) = cab::Cabinet::new(Cursor::new(cab)) else {
            continue;
        };
        let names: Vec<String> = cabinet
            .folder_entries()
            .flat_map(|f| f.file_entries().map(|e| e.name().to_string()).collect::<Vec<_>>())
            .collect();
        if !names.iter().any(|n| n.replace('\\', "/").eq_ignore_ascii_case("Res/Engine.res")) {
            continue;
        }
        let mut out = Vec::new();
        for (i, name) in names.iter().enumerate() {
            progress(i, names.len());
            let path = name.replace('\\', "/").to_ascii_uppercase();
            if !wanted(&path) {
                continue;
            }
            let mut bytes = Vec::new();
            cabinet
                .read_file(name)
                .and_then(|mut r| r.read_to_end(&mut bytes))
                .map_err(|_| Error::Corrupt("the demo's cabinet is damaged"))?;
            let path = if path.ends_with(".ENV") && !has_ground(&bytes, path.rsplit('/').next().unwrap_or(&path)) {
                format!("{STUBS}/{path}")
            } else {
                path
            };
            out.push((path, bytes));
        }
        progress(names.len(), names.len());
        out.push((MARKER.to_string(), MARKER_TEXT.as_bytes().to_vec()));
        return Ok(out);
    }
    Err(NOT_DEMO)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_the_game() {
        assert!(wanted("RES/ENGINE.RES"));
        assert!(wanted("TERAFORM/NATIONAL/NAT04.ENV"));
        assert!(wanted("UILANG.DLL"));
        assert!(!wanted("SETUPENU.DLL"));
        assert!(!wanted("UNINSTAL.EXE"));
        assert!(!wanted("README.TXT"));
    }

    #[test]
    fn is_not_fooled() {
        assert!(!looks_like(b"MZ not a cabinet in sight"));
        assert!(unpack(b"MZ nothing", |_, _| {}).is_err());
    }

    /// With `MM2_DEMO` naming a copy of `MCM2PCG.exe`: the two whole
    /// tracks come out, the ones without ground go aside.
    #[test]
    fn unpacks_the_demo() {
        let Some(path) = std::env::var_os("MM2_DEMO") else {
            return;
        };
        let exe = std::fs::read(path).unwrap();
        let files = unpack(&exe, |_, _| {}).unwrap();
        let has = |p: &str| files.iter().any(|(n, _)| n == p);
        assert!(has("UI/MAINMENU.DAT") && has("RES/ENGINE.RES") && has(MARKER));
        assert!(has("TERAFORM/NATIONAL/NAT04.ENV") && has("TERAFORM/QUARRIES/QUARRY01.ENV"));
        assert!(!has("TERAFORM/SX/SX01.ENV") && has("DEMO/TERAFORM/SX/SX01.ENV"));
        assert!(!files.iter().any(|(n, _)| n.starts_with("SETUP")));
    }
}
