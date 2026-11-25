//! Shared Course Builder tracks (the leaderboard server's `/shared`): the
//! list it gives, and what this player has shared, downloaded and rated,
//! kept in `shared-tracks.txt` in the user folder. The game's builder and
//! the launcher's Tracks page both use it.

use std::collections::BTreeMap;
use std::path::Path;

/// The file in the user folder.
pub const FILE: &str = "shared-tracks.txt";

/// The biggest track the server takes, bytes of `.course` JSON.
pub const MOST_BYTES: usize = 6 * 1024 * 1024;

/// One shared track, as the server lists it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Row {
    pub id: String,
    pub name: String,
    pub author: String,
    /// `Supercross`, `National`, `Baja` or `Enduro`.
    pub mode: String,
    /// For a copy of another track, that track's file (the player needs
    /// it to ride the copy); empty for a track made from scratch.
    pub base: String,
    pub downloads: u32,
    /// The average rating, times ten (35 is three and a half stars).
    pub stars: u32,
    pub votes: u32,
    /// When it was shared and last changed, Unix seconds.
    pub created: u64,
    pub bytes: u64,
    pub updated: u64,
}

impl Row {
    /// The rating as stars, such as "4.5 (12)", or "no votes".
    pub fn rating(&self) -> String {
        if self.votes == 0 {
            "no votes yet".into()
        } else {
            format!("{}.{} stars ({})", self.stars / 10, self.stars % 10, self.votes)
        }
    }
}

/// The server's list, one track a line; lines it can't read are skipped.
pub fn parse_rows(text: &str) -> Vec<Row> {
    text.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.trim_end_matches('\r').split('\t').collect();
            if f.len() < 11 || f[0].is_empty() {
                return None;
            }
            let n = |i: usize| f[i].trim().parse().unwrap_or(0);
            Some(Row {
                id: f[0].into(),
                name: f[1].into(),
                author: f[2].into(),
                mode: f[3].into(),
                base: f[4].into(),
                downloads: n(5) as u32,
                stars: n(6) as u32,
                votes: n(7) as u32,
                created: n(8),
                bytes: n(9),
                updated: n(10),
            })
        })
        .collect()
}

/// How the list is sorted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    #[default]
    New,
    Top,
    Most,
}

impl Sort {
    pub const ALL: [Sort; 3] = [Sort::New, Sort::Top, Sort::Most];

    pub fn label(self) -> &'static str {
        match self {
            Sort::New => "Newest",
            Sort::Top => "Top rated",
            Sort::Most => "Most downloaded",
        }
    }

    fn word(self) -> &'static str {
        match self {
            Sort::New => "new",
            Sort::Top => "top",
            Sort::Most => "most",
        }
    }
}

/// The list's address on the server at `server`: one kind of race (or
/// all, for an empty `mode`), sorted, names or authors with `search`.
pub fn list_url(server: &str, mode: &str, sort: Sort, search: &str, page: u32) -> String {
    format!("{server}/shared?mode={}&sort={}&q={}&page={page}", encode(mode), sort.word(), encode(search.trim()))
}

pub fn track_url(server: &str, id: &str, voter: &str) -> String {
    format!("{server}/shared/track?id={}&voter={}", encode(id), encode(voter))
}

/// Where a track is shared to: a new one, or a new version of one this
/// player shared before (`again`: its id and key).
pub fn share_url(server: &str, voter: &str, again: Option<(&str, &str)>) -> String {
    match again {
        Some((id, key)) => format!("{server}/shared?voter={}&id={}&key={}", encode(voter), encode(id), encode(key)),
        None => format!("{server}/shared?voter={}", encode(voter)),
    }
}

/// The server's answer to a share: the track's id and its key.
pub fn parse_shared(answer: &str) -> Result<(String, String), String> {
    let f: Vec<&str> = answer.trim().split('\t').collect();
    match f.as_slice() {
        ["ok", id, key] if !id.is_empty() && !key.is_empty() => Ok((id.to_string(), key.to_string())),
        _ => Err(match answer.trim() {
            "slow down" => "You've shared a lot of tracks today: try again tomorrow".into(),
            "full" => "The server is full just now".into(),
            "not a track" => "The server couldn't read the track".into(),
            "too big" => "The track is too big to share".into(),
            "not yours" => "That track was shared by someone else".into(),
            other => format!("The server said: {other}"),
        }),
    }
}

/// A track name as the builder files it: letters and digits, at most 8,
/// in capitals (`Course::stem`).
pub fn stem(name: &str) -> String {
    let s: String = name.chars().filter(|c| c.is_ascii_alphanumeric()).take(8).collect();
    if s.is_empty() {
        "MYTRACK".into()
    } else {
        s.to_ascii_uppercase()
    }
}

/// `name`, or a numbered short form of it ("Copper 2") whose stem none of
/// `taken` (stems) has.
pub fn free_name(name: &str, taken: &[String]) -> Option<String> {
    let has = |n: &str| taken.iter().any(|t| t.eq_ignore_ascii_case(&stem(n)));
    if !has(name) {
        return Some(name.to_string());
    }
    let words = name.split_whitespace().collect::<Vec<_>>().join(" ");
    (2..100u32).map(|k| {
        let room = 8 - k.to_string().len();
        let mut short = String::new();
        let mut letters = 0;
        for ch in words.chars() {
            if ch.is_ascii_alphanumeric() {
                if letters == room {
                    break;
                }
                letters += 1;
            }
            short.push(ch);
        }
        format!("{} {k}", short.trim_end())
    })
    .find(|n| !has(n))
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// What this player has done with shared tracks.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mine {
    /// A random name for this player (no account, nothing about them),
    /// so a rating or a download counts once.
    pub voter: String,
    /// The tracks shared from here, by the `.course` file's stem: the
    /// track's id and the key that changes or removes it.
    pub shared: BTreeMap<String, (String, String)>,
    /// The tracks downloaded, by id: the stem they were saved as.
    pub got: BTreeMap<String, String>,
    /// The stars given, by id.
    pub votes: BTreeMap<String, u8>,
}

impl Mine {
    pub fn parse(text: &str) -> Mine {
        let mut m = Mine::default();
        for l in text.lines() {
            let f: Vec<&str> = l.trim_end_matches('\r').split('\t').collect();
            match f.as_slice() {
                ["voter", v] => m.voter = v.to_string(),
                ["shared", stem, id, key] => {
                    m.shared.insert(stem.to_string(), (id.to_string(), key.to_string()));
                }
                ["got", id, stem] => {
                    m.got.insert(id.to_string(), stem.to_string());
                }
                ["vote", id, stars] => {
                    if let Ok(s) = stars.parse() {
                        m.votes.insert(id.to_string(), s);
                    }
                }
                _ => {}
            }
        }
        m
    }

    pub fn text(&self) -> String {
        let mut out = format!("voter\t{}\n", self.voter);
        for (stem, (id, key)) in &self.shared {
            out += &format!("shared\t{stem}\t{id}\t{key}\n");
        }
        for (id, stem) in &self.got {
            out += &format!("got\t{id}\t{stem}\n");
        }
        for (id, s) in &self.votes {
            out += &format!("vote\t{id}\t{s}\n");
        }
        out
    }

    /// Gives this player a voter name if it has none, from `seed` (some
    /// random bits the caller has).
    pub fn ensure_voter(&mut self, seed: u64) -> bool {
        if self.voter.len() >= 12 {
            return false;
        }
        const ABC: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
        let mut x = seed ^ 0x9E37_79B9_7F4A_7C15;
        self.voter = (0..20)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                ABC[(x % ABC.len() as u64) as usize] as char
            })
            .collect();
        true
    }

    /// The id of the shared track this player downloaded as `stem`.
    pub fn got_as(&self, stem: &str) -> Option<&str> {
        self.got.iter().find(|(_, s)| s.eq_ignore_ascii_case(stem)).map(|(id, _)| id.as_str())
    }

    pub fn read_from(dir: &Path) -> Mine {
        Mine::parse(&std::fs::read_to_string(dir.join(FILE)).unwrap_or_default())
    }
}

/// Some random bits from the clock and the process, for [`Mine::ensure_voter`]
/// off a browser.
pub fn seed() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    h.write_u32(std::process::id());
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_list() {
        let rows = parse_rows("ab12cd34\tCopper Hill\tMike\tNational\t\t12\t45\t2\t1700000000\t300000\t1700000100\nbad line\n");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Copper Hill");
        assert_eq!((rows[0].downloads, rows[0].stars, rows[0].votes), (12, 45, 2));
        assert_eq!(rows[0].rating(), "4.5 stars (2)");
    }

    #[test]
    fn keeps_what_i_did() {
        let mut m = Mine::default();
        assert!(m.ensure_voter(42));
        assert_eq!(m.voter.len(), 20);
        assert!(!m.ensure_voter(43));
        m.shared.insert("MYNAT1".into(), ("abc".into(), "k1".into()));
        m.got.insert("xyz".into(), "HILL".into());
        m.votes.insert("xyz".into(), 4);
        assert_eq!(Mine::parse(&m.text()), m);
        assert_eq!(m.got_as("hill"), Some("xyz"));
    }

    #[test]
    fn names_a_download_apart_from_my_tracks() {
        assert_eq!(free_name("Copper Hill", &[]).unwrap(), "Copper Hill");
        let n = free_name("Copper Hill", &["COPPERHI".into()]).unwrap();
        assert_eq!((n.as_str(), stem(&n).as_str()), ("Copper H 2", "COPPERH2"));
        assert_eq!(free_name("Copper Hill", &["COPPERHI".into(), "COPPERH2".into()]).unwrap(), "Copper H 3");
    }

    #[test]
    fn addresses() {
        assert_eq!(list_url("https://s", "National", Sort::Top, " big air ", 1), "https://s/shared?mode=National&sort=top&q=big%20air&page=1");
        assert_eq!(parse_shared("ok\tab\tkk\n").unwrap(), ("ab".into(), "kk".into()));
        assert!(parse_shared("slow down\n").unwrap_err().contains("tomorrow"));
    }
}
