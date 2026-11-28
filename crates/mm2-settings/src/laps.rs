//! Lap leaderboards (on unless turned off): the player's best laps, kept
//! beside the saves and shared with other players through the leaderboard
//! server (`tools/leaderboard/`).
//!
//! The game writes each lap that counts into two files in the user's data
//! folder: `laps.txt`, the best lap for each board, and
//! `laps-pending.txt`, the laps not yet sent. The game sends the pending
//! ones after each race, the launcher when it starts.
//!
//! A board is one track, event, course and bike class. A line in either
//! file is the board, the rider, the time in milliseconds and the track's
//! name as the game shows it, split by tabs:
//! `farm\t2\t0\t250\tMichael\t83456\tMunchberry Farms`.

use std::path::{Path, PathBuf};

/// The best laps file.
pub const BEST: &str = "laps.txt";
/// The laps not yet sent.
pub const PENDING: &str = "laps-pending.txt";
/// A lap with more than this ridden off the course (seconds, not counting
/// time down in a crash) doesn't count, in Nationals and Supercross. The
/// race tracker's course edges sit inside the ridden track at some bends:
/// the computer's own line on Voodoo Basin is "off" 2.4 to 4 s a clean
/// lap, so the limit is 6 s, the game's own put-back time (0x492ac0),
/// which leaves room for a wider line than the computer's.
pub const MOST_OFF_COURSE: f32 = 6.0;

/// Whether a lap in `event` must keep to the course ([`MOST_OFF_COURSE`]):
/// Nationals and Supercross. Baja and Enduro go between checkpoints.
pub fn keeps_on_course(event: u32) -> bool {
    matches!(event, 2 | 3)
}
/// Laps shorter than this (ms) can't be real.
pub const SHORTEST: u32 = 5_000;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Board {
    /// The track's file name without its extension, in lower case.
    pub track: String,
    /// The event (the game's numbers: 1 Baja, 2 Nationals, 3 Supercross,
    /// 5 Enduro).
    pub event: u32,
    pub course: u32,
    /// The bike's engine size, cc.
    pub class: u32,
}

impl Board {
    pub fn new(track: &Path, event: u32, course: u32, class: u32) -> Board {
        let track = track.file_stem().map_or_else(String::new, |s| s.to_string_lossy().to_lowercase());
        Board { track: clean(&track), event, course, class }
    }

    /// The board's part of a line, as the server takes it in a query too.
    pub fn key(&self) -> String {
        format!("{}\t{}\t{}\t{}", self.track, self.event, self.course, self.class)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lap {
    pub board: Board,
    pub rider: String,
    pub ms: u32,
    /// The track's name in the game's lists (may be empty).
    pub title: String,
}

impl Lap {
    pub fn line(&self) -> String {
        format!("{}\t{}\t{}\t{}", self.board.key(), clean(&self.rider), self.ms, clean_title(&self.title))
    }

    pub fn parse(line: &str) -> Option<Lap> {
        let f: Vec<&str> = line.trim_end_matches(['\r', '\n']).split('\t').collect();
        let (fields, title) = match f.as_slice() {
            [a @ .., t] if a.len() == 6 => (a, *t),
            a => (a, ""),
        };
        let [track, event, course, class, rider, ms] = fields else { return None };
        Some(Lap {
            board: Board {
                track: clean(track),
                event: event.parse().ok()?,
                course: course.parse().ok()?,
                class: class.parse().ok()?,
            },
            rider: clean(rider),
            ms: ms.parse().ok().filter(|&ms| ms >= SHORTEST)?,
            title: clean_title(title),
        })
    }
}

/// A name or track as it may go in a line: no tabs or line breaks, and at
/// most 24 letters.
pub fn clean(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(24).collect::<String>().trim().to_string()
}

/// A track's name for a line: like [`clean`], up to 40 letters.
pub fn clean_title(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(40).collect::<String>().trim().to_string()
}

pub fn parse_all(text: &str) -> Vec<Lap> {
    text.lines().filter_map(Lap::parse).collect()
}

pub fn text(laps: &[Lap]) -> String {
    laps.iter().map(|l| l.line() + "\n").collect()
}

pub fn read(path: &Path) -> Vec<Lap> {
    std::fs::read_to_string(path).map_or_else(|_| Vec::new(), |t| parse_all(&t))
}

/// The best lap of each board and rider, fastest first.
pub fn best_of(laps: impl IntoIterator<Item = Lap>) -> Vec<Lap> {
    let mut best: Vec<Lap> = Vec::new();
    for lap in laps {
        match best.iter_mut().find(|b| b.board == lap.board && b.rider.eq_ignore_ascii_case(&lap.rider)) {
            Some(b) if b.ms <= lap.ms => {}
            Some(b) => *b = lap,
            None => best.push(lap),
        }
    }
    best.sort_by(|a, b| a.board.cmp(&b.board).then(a.ms.cmp(&b.ms)));
    best
}

/// The two files' new text with `laps` entered, from their old text, or
/// none if no lap was a new best.
pub fn enter(best: &str, pending: &str, laps: &[Lap]) -> Option<(String, String)> {
    let old = parse_all(best);
    let new: Vec<Lap> = best_of(laps.iter().cloned())
        .into_iter()
        .filter(|l| !old.iter().any(|o| o.board == l.board && o.rider.eq_ignore_ascii_case(&l.rider) && o.ms <= l.ms))
        .collect();
    if new.is_empty() {
        return None;
    }
    let best = text(&best_of(old.into_iter().chain(new.iter().cloned())));
    let pending = text(&best_of(parse_all(pending).into_iter().chain(new)));
    Some((best, pending))
}

/// Keeps the laps in `dir`: a new best goes in the best laps and waits to
/// be sent. Returns whether any lap was a new best.
pub fn record(dir: &Path, laps: &[Lap]) -> std::io::Result<bool> {
    let old = |n: &str| std::fs::read_to_string(dir.join(n)).unwrap_or_default();
    let Some((best, pending)) = enter(&old(BEST), &old(PENDING), laps) else { return Ok(false) };
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join(BEST), best)?;
    std::fs::write(dir.join(PENDING), pending)?;
    Ok(true)
}

/// The laps waiting to be sent.
pub fn pending(dir: &Path) -> (PathBuf, Vec<Lap>) {
    let path = dir.join(PENDING);
    let laps = read(&path);
    (path, laps)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lap(track: &str, rider: &str, ms: u32) -> Lap {
        Lap { board: Board::new(Path::new(track), 2, 0, 250), rider: rider.into(), ms, title: String::new() }
    }

    #[test]
    fn lines_round_trip() {
        let l = lap("TRACKS/Farm.ENV", "Mi\tke", 83_456);
        assert_eq!(l.board.track, "farm");
        assert_eq!(Lap::parse(&l.line()), Some(Lap { rider: "Mike".into(), ..l.clone() }));
        assert_eq!(Lap::parse("farm\t2\t0\t250\tA\t10"), None, "too short");
        assert_eq!(Lap::parse("junk"), None);
        let named = Lap::parse("farm\t2\t0\t250\tA\t60000\tMunchberry Farms").unwrap();
        assert_eq!(named.title, "Munchberry Farms");
        assert_eq!(Lap::parse(&named.line()), Some(named));
    }

    #[test]
    fn only_new_bests_are_kept_and_sent() {
        let dir = std::env::temp_dir().join(format!("mm2-laps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(record(&dir, &[lap("farm", "A", 90_000), lap("farm", "A", 80_000), lap("sx01", "A", 50_000)]).unwrap());
        assert!(!record(&dir, &[lap("farm", "a", 85_000)]).unwrap(), "slower than the best");
        assert!(record(&dir, &[lap("farm", "B", 99_000)]).unwrap());
        let best = read(&dir.join(BEST));
        assert_eq!(best.len(), 3);
        assert_eq!(best.iter().find(|l| l.rider == "A" && l.board.track == "farm").unwrap().ms, 80_000);
        assert_eq!(pending(&dir).1.len(), 3);
        assert!(keeps_on_course(2) && keeps_on_course(3) && !keeps_on_course(1) && !keeps_on_course(5));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
