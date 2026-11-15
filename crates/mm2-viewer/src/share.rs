//! Lobby downloads (an extra, off unless turned on): in a network lobby,
//! a player who hasn't the host's track, or another rider's own bike
//! paint or outfit (the own skins extra's `BIKES` and `RIDERS` files),
//! fetches it from whoever has it, through the host.
//!
//! A download is asked of its owner, who sends it in parts a few at a
//! time each frame so the lobby keeps running; the file is written beside
//! its place as `.part` and put there once whole. Nothing already there
//! is replaced, and only what is in use is given: the host's race track,
//! a rider's own skins.

use crate::frontend::EventKind;
use crate::net::{self, FileNote, Msg, Player, FILE_BIKE, FILE_PART, FILE_RIDER, FILE_TRACK};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

/// The biggest file given or taken (the biggest tracks are about 40 MB).
const MOST: u32 = 96 << 20;
/// Parts sent each frame.
const PARTS_A_FRAME: usize = 12;

/// A file being sent: to whom, and how far.
struct Out {
    note: FileNote,
    data: Vec<u8>,
    offset: usize,
}

/// A file coming in.
struct In {
    data: Vec<u8>,
    total: u32,
}

#[derive(Default)]
pub struct Share {
    sending: VecDeque<Out>,
    coming: HashMap<(u8, String), In>,
    /// What has been asked for already, so it is asked once.
    asked: HashSet<(u8, String)>,
}

/// Whether the extra is on.
pub fn on() -> bool {
    crate::tweaks::more().lobby_downloads
}

/// The name a track goes by in a download: its event's number and stem.
pub fn track_name(event: EventKind, stem: &str) -> String {
    format!("{}:{}", event as u8, stem)
}

/// A skin's file name from a ride's texture key (`own-bikes-red.tga`), if
/// it is one of the player's own.
pub fn own_skin(key: &str) -> Option<(u8, String)> {
    if let Some(f) = key.strip_prefix("own-bikes-") {
        return Some((FILE_BIKE, f.to_string()));
    }
    key.strip_prefix("own-riders-").map(|f| (FILE_RIDER, f.to_string()))
}

/// A name from the network made safe to be a file name: no folders, no
/// odd characters.
fn safe(name: &str) -> Option<String> {
    let name = name.rsplit(['/', '\\']).next()?.trim();
    let ok = !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('.')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || " _-.()&'+!".contains(c));
    ok.then(|| name.to_string())
}

/// Where a file lives in the game folder `game`: the event's track folder
/// with `.ENV`, or `BIKES`/`RIDERS`, found whatever their case.
fn place(game: &Path, kind: u8, name: &str) -> Option<PathBuf> {
    let folder_of = |dir: &Path, want: &str| -> PathBuf {
        crate::fs::read_dir(dir)
            .ok()
            .and_then(|l| l.flatten().find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(want)))
            .map(|e| e.path())
            .unwrap_or_else(|| dir.join(want))
    };
    match kind {
        FILE_TRACK => {
            let (event, stem) = name.split_once(':')?;
            let event = EventKind::from_number(event.parse().ok()?)?;
            let stem = safe(stem)?;
            let mut dir = game.to_path_buf();
            for part in event.folder().split('/') {
                dir = folder_of(&dir, part);
            }
            // A file of that name in any case.
            let found = crate::fs::read_dir(&dir).ok().and_then(|l| {
                l.flatten()
                    .map(|e| e.path())
                    .find(|p| p.file_stem().is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case(&stem)) && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("env")))
            });
            Some(found.unwrap_or_else(|| dir.join(format!("{stem}.ENV"))))
        }
        FILE_BIKE | FILE_RIDER => {
            let file = safe(name)?;
            if !file.to_ascii_lowercase().ends_with(".tga") {
                return None;
            }
            let dir = folder_of(game, if kind == FILE_BIKE { "BIKES" } else { "RIDERS" });
            let found = crate::fs::read_dir(&dir)
                .ok()
                .and_then(|l| l.flatten().map(|e| e.path()).find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(&file))));
            Some(found.unwrap_or_else(|| dir.join(file)))
        }
        _ => None,
    }
}

fn what(kind: u8, name: &str) -> String {
    match kind {
        FILE_TRACK => format!("the track {}", name.split_once(':').map_or(name, |n| n.1)),
        FILE_BIKE => format!("the bike paint {name}"),
        _ => format!("the outfit {name}"),
    }
}

impl Share {
    /// Asks for whatever this player hasn't of the host's track (`event`,
    /// `stem`) and the other riders' own skins. Returns lines for the chat.
    pub fn want(&mut self, game: &Path, track: Option<(EventKind, &str)>, players: &[Player], me: u8) -> Vec<String> {
        let mut said = Vec::new();
        let mut wants: Vec<(u8, u8, String)> = Vec::new();
        if let Some((event, stem)) = track.filter(|(e, s)| *e != EventKind::Tag && !s.is_empty()) {
            if me != 0 {
                wants.push((0, FILE_TRACK, track_name(event, stem)));
            }
        }
        for p in players.iter().filter(|p| p.id != me) {
            for key in [&p.ride.texture, &p.ride.outfit] {
                if let Some((kind, file)) = own_skin(key) {
                    wants.push((p.id, kind, file));
                }
            }
        }
        for (owner, kind, name) in wants {
            let Some(path) = place(game, kind, &name) else { continue };
            if crate::fs::is_file(&path) || !self.asked.insert((kind, name.clone())) {
                continue;
            }
            said.push(format!("Getting {} from the other player…", what(kind, &name)));
            net::with(|s| s.send_file(Msg::FileAsk(FileNote { to: owner, from: 0, kind, name })));
        }
        said
    }

    /// A download's message for this player. Returns a line for the chat
    /// when one finishes or can't be had.
    pub fn heard(&mut self, game: &Path, m: Msg, track: Option<(EventKind, &str)>, mine: (&str, &str)) -> Option<String> {
        match m {
            Msg::FileAsk(n) => {
                // Only what is in use: the host's race track, my own skins.
                let ok = match n.kind {
                    FILE_TRACK => track.is_some_and(|(e, s)| track_name(e, s).eq_ignore_ascii_case(&n.name)),
                    FILE_BIKE => own_skin(mine.0).is_some_and(|(_, f)| f.eq_ignore_ascii_case(&n.name)),
                    FILE_RIDER => own_skin(mine.1).is_some_and(|(_, f)| f.eq_ignore_ascii_case(&n.name)),
                    _ => false,
                };
                let data = ok
                    .then(|| place(game, n.kind, &n.name))
                    .flatten()
                    .and_then(|p| crate::fs::read(p).ok())
                    .filter(|d| d.len() as u32 <= MOST);
                let back = FileNote { to: n.from, from: 0, kind: n.kind, name: n.name };
                match data {
                    Some(data) => self.sending.push_back(Out { note: back, data, offset: 0 }),
                    None => net::with(|s| s.send_file(Msg::FileNone(back))).unwrap_or(()),
                }
                None
            }
            Msg::FileNone(n) => Some(format!("Couldn't get {}.", what(n.kind, &n.name))),
            Msg::FilePart { note, offset, total, data } => {
                let key = (note.kind, note.name.clone());
                if !self.asked.contains(&key) || total > MOST {
                    return None;
                }
                let got = self.coming.entry(key.clone()).or_insert_with(|| In { data: Vec::new(), total });
                if offset as usize != got.data.len() || got.total != total {
                    self.coming.remove(&key);
                    return Some(format!("Couldn't get {}: it came out of order.", what(note.kind, &note.name)));
                }
                got.data.extend_from_slice(&data);
                if got.data.len() < total as usize {
                    return None;
                }
                let got = self.coming.remove(&key)?;
                Some(match self.keep(game, note.kind, &note.name, &got.data) {
                    Ok(()) => format!("Got {}.", what(note.kind, &note.name)),
                    Err(e) => format!("Couldn't keep {}: {e}", what(note.kind, &note.name)),
                })
            }
            _ => None,
        }
    }

    /// Writes a finished download into the game folder, never over a file
    /// already there.
    fn keep(&self, game: &Path, kind: u8, name: &str, data: &[u8]) -> Result<(), String> {
        let path = place(game, kind, name).ok_or("its name isn't one a file can have")?;
        if crate::fs::is_file(&path) {
            return Err("there's already a file of that name".into());
        }
        if kind == FILE_TRACK && !data.starts_with(b"RS2") && mm2_formats::faoe::decode(data, "x.env").is_err() {
            return Err("it isn't a track".into());
        }
        if let Some(dir) = path.parent() {
            crate::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let part = path.with_extension("part");
        crate::fs::write(&part, data).map_err(|e| e.to_string())?;
        crate::fs::copy(&part, &path).map_err(|e| e.to_string())?;
        let _ = crate::fs::remove_file(&part);
        Ok(())
    }

    /// Sends a few parts of what is being given, each frame.
    pub fn pump(&mut self) {
        for _ in 0..PARTS_A_FRAME {
            let Some(out) = self.sending.front_mut() else { return };
            let end = (out.offset + FILE_PART).min(out.data.len());
            let m = Msg::FilePart {
                note: out.note.clone(),
                offset: out.offset as u32,
                total: out.data.len() as u32,
                data: out.data[out.offset..end].to_vec(),
            };
            net::with(|s| s.send_file(m));
            out.offset = end;
            if out.offset >= out.data.len() {
                self.sending.pop_front();
            }
        }
    }

    /// Whether anything is still coming in.
    pub fn busy(&self) -> bool {
        !self.coming.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_from_the_network_stay_in_their_folder() {
        assert_eq!(safe("../../etc/passwd").as_deref(), Some("passwd"));
        assert_eq!(safe("RED one.tga").as_deref(), Some("RED one.tga"));
        assert_eq!(safe(".hidden"), None);
        assert_eq!(safe("a\\b:c"), None);
        assert_eq!(own_skin("own-bikes-red.tga"), Some((FILE_BIKE, "red.tga".into())));
        assert_eq!(own_skin("HondaCR250_08.tga"), None);
        let game = Path::new("/g");
        assert!(place(game, FILE_BIKE, "x.exe").is_none());
        let p = place(game, FILE_TRACK, &track_name(EventKind::Supercross, "MYSX")).unwrap();
        assert!(p.ends_with("TERAFORM/SX/MYSX.ENV"), "{}", p.display());
    }

    #[test]
    fn a_track_comes_in_parts_and_is_kept() {
        let game = std::env::temp_dir().join(format!("mm2-share-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&game);
        std::fs::create_dir_all(game.join("TERAFORM/SX")).unwrap();
        let mut share = Share::default();
        let said = share.want(&game, Some((EventKind::Supercross, "MYSX")), &[], 1);
        assert_eq!(said.len(), 1, "{said:?}");
        // Asked once only.
        assert!(share.want(&game, Some((EventKind::Supercross, "MYSX")), &[], 1).is_empty());
        let data: Vec<u8> = b"RS2\0".iter().copied().chain((0..100_000).map(|i| i as u8)).collect();
        let name = track_name(EventKind::Supercross, "MYSX");
        let mut last = None;
        for (i, part) in data.chunks(FILE_PART).enumerate() {
            let m = Msg::FilePart {
                note: FileNote { to: 1, from: 0, kind: FILE_TRACK, name: name.clone() },
                offset: (i * FILE_PART) as u32,
                total: data.len() as u32,
                data: part.to_vec(),
            };
            last = share.heard(&game, m, None, ("", ""));
        }
        assert_eq!(last.as_deref(), Some("Got the track MYSX."));
        assert_eq!(std::fs::read(game.join("TERAFORM/SX/MYSX.ENV")).unwrap(), data);
        assert!(!share.busy());
        std::fs::remove_dir_all(&game).unwrap();
    }
}
