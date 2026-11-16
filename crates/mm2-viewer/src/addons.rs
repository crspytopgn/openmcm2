//! Add-on resource packs: extra archives in `RES/ADDONS` that fill in
//! what a player-made track needs (art in `GLOBAL.RES`, trees in
//! `ECO.RES`, models in `MODELS.RES`, …) without the game's own archives
//! being replaced.
//!
//! An add-on joins the game archive its name ends with:
//! `RES/ADDONS/Rats Hole-ECO.RES` and `RES/ADDONS/ECO/anything.res` both
//! join `ECO.RES`. An add-on only adds names the game's archive doesn't
//! have; it never replaces one of the game's own files, so the stock
//! tracks look as they always did. Between add-ons, the first by name
//! wins.

use crate::fs;
use mm2_formats::faoe;
use mm2_formats::rs2;
use std::path::{Path, PathBuf};

/// The add-on archives for the game archive `base` (`ECO.RES`) in the
/// `RES` folder `res`, in name order.
pub fn packs_for(res: &Path, base: &str) -> Vec<PathBuf> {
    let Some(dir) = find_ci(res, "ADDONS") else { return Vec::new() };
    let stem = base.rsplit_once('.').map_or(base, |(s, _)| s).to_ascii_uppercase();
    let mut found = Vec::new();
    let Ok(list) = fs::read_dir(&dir) else { return found };
    for e in list.flatten() {
        let path = e.path();
        let name = e.file_name().to_string_lossy().to_ascii_uppercase();
        if fs::is_dir(&path) {
            if name == stem {
                if let Ok(inner) = fs::read_dir(&path) {
                    found.extend(inner.flatten().map(|e| e.path()).filter(|p| is_res(p) && fs::is_file(p)));
                }
            }
        } else if is_res(&path) && joins(&name, &stem) {
            found.push(path);
        }
    }
    found.sort_by_key(|p| p.to_string_lossy().to_ascii_lowercase());
    found
}

fn is_res(p: &Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("res"))
}

/// Whether the add-on file `name` (upper case) joins the archive `stem`:
/// `ECO.RES` itself, or a name ending in `-ECO.RES`, `_ECO.RES` or
/// ` ECO.RES`.
fn joins(name: &str, stem: &str) -> bool {
    let Some(head) = name.strip_suffix(".RES") else { return false };
    let Some(before) = head.strip_suffix(stem) else { return false };
    before.is_empty() || before.ends_with(['-', '_', ' ', '.'])
}

fn find_ci(dir: &Path, name: &str) -> Option<PathBuf> {
    let exact = dir.join(name);
    if fs::is_dir(&exact) {
        return Some(exact);
    }
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name) && fs::is_dir(e.path()))
        .map(|e| e.path())
}

/// An archive's plain RS2 bytes, from either container.
pub fn plain(raw: Vec<u8>, name: &str) -> Option<Vec<u8>> {
    if raw.starts_with(rs2::MAGIC) {
        return Some(raw);
    }
    faoe::decode(&raw, name).ok().filter(|b| b.starts_with(rs2::MAGIC))
}

/// The game archive's plain bytes `base`, with the add-ons' new names
/// joined after its own. With no add-ons, `base` comes back unchanged.
pub fn join(base: Vec<u8>, packs: &[(String, Vec<u8>)]) -> Vec<u8> {
    if packs.is_empty() {
        return base;
    }
    let Ok(own) = rs2::Archive::parse(&base) else { return base };
    let mut names: Vec<String> = own.entries().iter().map(|e| e.name.to_ascii_lowercase()).collect();
    let mut files: Vec<(&str, &[u8])> = own.entries().iter().map(|e| (e.name.as_str(), own.data(e))).collect();
    let parsed: Vec<(&String, rs2::Archive)> = packs
        .iter()
        .filter_map(|(n, b)| match rs2::Archive::parse(b) {
            Ok(a) => Some((n, a)),
            Err(e) => {
                eprintln!("add-on {n}: {e}, left out");
                None
            }
        })
        .collect();
    let mut added = 0;
    for (_, a) in &parsed {
        for e in a.entries() {
            let key = e.name.to_ascii_lowercase();
            if !names.contains(&key) {
                names.push(key);
                files.push((e.name.as_str(), a.data(e)));
                added += 1;
            }
        }
    }
    if added == 0 {
        return base;
    }
    rs2::build(&files)
}

/// Reads the add-on archives for `path` (a game archive such as
/// `RES/ECO.RES`) and joins them to its plain bytes `base`.
pub fn with_addons(path: &Path, base: Vec<u8>) -> Vec<u8> {
    let (Some(res), Some(name)) = (path.parent(), path.file_name().and_then(|n| n.to_str())) else {
        return base;
    };
    let mut packs: Vec<(String, Vec<u8>)> = packs_for(res, name)
        .into_iter()
        .filter_map(|p| {
            let n = p.file_name()?.to_string_lossy().into_owned();
            let raw = fs::read(&p).ok()?;
            match plain(raw, &n) {
                Some(b) => Some((n, b)),
                None => {
                    eprintln!("add-on {}: not an RS2 or FAOE archive, left out", p.display());
                    None
                }
            }
        })
        .collect();
    // The paints and outfits in the game's BIKES and RIDERS folders join
    // the bikes' archive under names of their own, as the game always
    // reads them.
    if name.eq_ignore_ascii_case("BIKE.RES") {
        if let Some(game) = res.parent() {
            let mut files = Vec::new();
            for folder in ["BIKES", "RIDERS"] {
                for (file, _) in crate::roster::own_files(game, folder) {
                    let dir = fs::read_dir(game)
                        .ok()
                        .and_then(|l| l.flatten().find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(folder)))
                        .map(|e| e.path());
                    if let Some(data) = dir.and_then(|d| fs::read(d.join(&file)).ok()) {
                        files.push((crate::roster::own_key(folder, &file), data));
                    }
                }
            }
            if !files.is_empty() {
                let list: Vec<(&str, &[u8])> = files.iter().map(|(n, d)| (n.as_str(), d.as_slice())).collect();
                packs.push(("own skins".into(), rs2::build(&list)));
            }
        }
    }
    join(base, &packs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_join_their_archive() {
        assert!(joins("ECO.RES", "ECO"));
        assert!(joins("RATS HOLE-ECO.RES", "ECO"));
        assert!(joins("MY_TRACK_GLOBAL.RES", "GLOBAL"));
        assert!(!joins("DECO.RES", "ECO"));
        assert!(!joins("ECO.TXT", "ECO"));
    }

    #[test]
    fn an_addon_adds_but_never_replaces() {
        let base = rs2::build(&[("tree.mdl", b"stock"), ("rock.mdl", b"rock")]);
        let a = rs2::build(&[("TREE.mdl", b"other"), ("palm.mdl", b"palm")]);
        let b = rs2::build(&[("palm.mdl", b"second"), ("cactus.mdl", b"cactus")]);
        let out = join(base.clone(), &[("a".into(), a), ("b".into(), b), ("bad".into(), b"junk".to_vec())]);
        let arc = rs2::Archive::parse(&out).unwrap();
        assert_eq!(arc.get("tree.mdl"), Some(&b"stock"[..]));
        assert_eq!(arc.get("palm.mdl"), Some(&b"palm"[..]));
        assert_eq!(arc.get("cactus.mdl"), Some(&b"cactus"[..]));
        assert_eq!(arc.entries().len(), 4);
        assert_eq!(join(base.clone(), &[]), base);
    }

    #[test]
    fn finds_addons_on_disk() {
        let dir = std::env::temp_dir().join(format!("mm2-addons-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Addons/eco")).unwrap();
        let pack = rs2::build(&[("palm.mdl", b"palm")]);
        std::fs::write(dir.join("Addons/Zed-ECO.RES"), &pack).unwrap();
        std::fs::write(dir.join("Addons/eco/a.res"), &pack).unwrap();
        std::fs::write(dir.join("Addons/Zed-GLOBAL.RES"), &pack).unwrap();
        std::fs::write(dir.join("Addons/notes.txt"), b"x").unwrap();
        let found = packs_for(&dir, "ECO.RES");
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found[0].ends_with("eco/a.res"));
        let base = rs2::build(&[("tree.mdl", b"t")]);
        let out = with_addons(&dir.join("ECO.RES"), base);
        assert_eq!(rs2::Archive::parse(&out).unwrap().get("palm.mdl"), Some(&b"palm"[..]));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
