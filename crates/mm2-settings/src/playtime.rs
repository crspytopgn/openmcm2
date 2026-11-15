//! The playtime tracker: how long the player has played, kept beside the
//! saves in `playtime.txt` as `key<TAB>seconds` lines.
//!
//! The keys: `total` (the game open), `races` (time in races), `count`
//! (races started), `event:<n>` (the game's event numbers, as in
//! [`crate::laps`]), `track:<name>` and `bike:<cc>`.
//!
//! The launcher shows it, and, unless turned off, sends what is new to the
//! leaderboard server as anonymous totals: only these keys and numbers,
//! never a name. `playtime-sent.txt` is what has been sent.

use std::collections::BTreeMap;
use std::path::Path;

pub const FILE: &str = "playtime.txt";
pub const SENT: &str = "playtime-sent.txt";

pub type Times = BTreeMap<String, u64>;

pub fn parse(text: &str) -> Times {
    let mut t = Times::new();
    for line in text.lines() {
        if let Some((k, v)) = line.split_once('\t') {
            let k = crate::laps::clean(k);
            if let (false, Ok(v)) = (k.is_empty(), v.trim().parse::<u64>()) {
                *t.entry(k).or_default() += v;
            }
        }
    }
    t
}

pub fn text(t: &Times) -> String {
    t.iter().filter(|(_, &v)| v > 0).map(|(k, v)| format!("{k}\t{v}\n")).collect()
}

/// `old` with `more` added.
pub fn add(old: &str, more: &[(String, u64)]) -> String {
    let mut t = parse(old);
    for (k, v) in more {
        *t.entry(crate::laps::clean(k)).or_default() += v;
    }
    text(&t)
}

/// The keys a race adds to: the races, the event, the track (its file
/// name without the extension) and the bike's engine size.
pub fn race_keys(event: u32, track: &Path, cc: u32) -> Vec<String> {
    let track = track.file_stem().map_or_else(String::new, |s| s.to_string_lossy().to_lowercase());
    vec!["races".into(), format!("event:{event}"), format!("track:{track}"), format!("bike:{cc}")]
}

pub fn read(dir: &Path, name: &str) -> Times {
    parse(&std::fs::read_to_string(dir.join(name)).unwrap_or_default())
}

/// What `now` has that `sent` hasn't yet.
pub fn new_since(now: &Times, sent: &Times) -> Times {
    now.iter()
        .filter_map(|(k, &v)| {
            let d = v.saturating_sub(sent.get(k).copied().unwrap_or(0));
            (d > 0).then(|| (k.clone(), d))
        })
        .collect()
}

/// `seconds` as a person reads it: `3 h 12 min`, `12 min`, `40 s`.
pub fn shown(seconds: u64) -> String {
    let (h, m) = (seconds / 3600, seconds / 60 % 60);
    match (h, m) {
        (0, 0) => format!("{seconds} s"),
        (0, m) => format!("{m} min"),
        (h, m) => format!("{h} h {m} min"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_add_up() {
        let a = add("", &[("total".into(), 60), ("races".into(), 30)]);
        let b = add(&a, &[("total".into(), 15), ("track:farm".into(), 30)]);
        let t = parse(&b);
        assert_eq!(t["total"], 75);
        assert_eq!(t["track:farm"], 30);
        let sent = parse("total\t60\n");
        let d = new_since(&t, &sent);
        assert_eq!(d["total"], 15);
        assert_eq!(d["races"], 30);
        assert_eq!(shown(3600 * 3 + 12 * 60 + 5), "3 h 12 min");
        assert_eq!(shown(40), "40 s");
        assert_eq!(race_keys(2, Path::new("A/Farm.ENV"), 250)[2], "track:farm");
    }
}
