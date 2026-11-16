//! The server browser: the open races the leaderboard server keeps
//! (`tools/leaderboard/worker.js`). A host tells of its race every
//! [`EVERY`] seconds (`POST /race`) and once more as it closes
//! (`POST /race/end`); anyone can ask for the list (`GET /races`).
//!
//! A computer's race is reached at its address (the server sees it, and
//! lists it only if the race port can be reached from outside); a
//! browser's by its race code. The two can't race each other, so each
//! asks only for its own kind.

/// Seconds between a host's word of its race; the server drops a race not
/// heard of for 90.
pub const EVERY: u64 = 30;

/// A race in the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenRace {
    /// `ip:port`, or a browser's race code.
    pub address: String,
    pub name: String,
    /// The game's event number (`event_name`).
    pub event: u32,
    /// The track's name as the host's game shows it.
    pub track: String,
    pub players: u32,
    /// Racing now, rather than waiting in the lobby.
    pub racing: bool,
    pub version: String,
}

/// The server's `GET /races` answer.
pub fn parse(text: &str) -> Vec<OpenRace> {
    text.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.trim_end_matches('\r').split('\t').collect();
            if f.len() < 6 || f[0].is_empty() {
                return None;
            }
            Some(OpenRace {
                address: f[0].to_string(),
                name: crate::laps::clean(f[1]),
                event: f[2].parse().unwrap_or(0),
                track: crate::laps::clean(f[3]),
                players: f[4].parse().unwrap_or(1),
                racing: f[5] == "1",
                version: f.get(6).map(|v| crate::laps::clean(v)).unwrap_or_default(),
            })
        })
        .collect()
}

/// What a host sends of its race: `key<TAB>value` lines. `id` is the
/// host's own secret name for the race (letters and digits, 16 or more);
/// `place` is `("web", code)` or `("pc", port)`.
pub fn listing(id: &str, place: (&str, &str), race: &OpenRace) -> String {
    let (kind, at) = place;
    let at_key = if kind == "web" { "code" } else { "port" };
    let field = |s: &str| s.replace(['\t', '\n', '\r'], " ");
    format!(
        "id\t{id}\nkind\t{kind}\n{at_key}\t{at}\nname\t{}\nevent\t{}\ntrack\t{}\nplayers\t{}\nracing\t{}\nversion\t{}\n",
        field(&race.name),
        race.event,
        field(&race.track),
        race.players,
        if race.racing { 1 } else { 0 },
        field(&race.version),
    )
}

/// The game's event numbers' names.
pub fn event_name(event: u32) -> &'static str {
    match event {
        0 => "Stunt Quarry",
        1 => "Baja",
        2 => "Nationals",
        3 => "Supercross",
        4 => "Moto-Tag",
        5 => "Enduro",
        _ => "Race",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_reads_back() {
        let text = "1.2.3.4:28220\tMike's\t3\tSeattle\t2\t0\t0.5.3\nABCDEF\tFun\t4\t\t1\t1\t\nbad line\n";
        let races = parse(text);
        assert_eq!(races.len(), 2);
        assert_eq!(races[0].address, "1.2.3.4:28220");
        assert_eq!(races[0].track, "Seattle");
        assert!(!races[0].racing && races[1].racing);
        let body = listing("abcdefabcdef12345", ("web", "ABCDEF"), &races[1]);
        assert!(body.contains("code\tABCDEF\n") && body.contains("racing\t1\n"));
    }
}
