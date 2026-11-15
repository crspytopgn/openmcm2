//! The lap leaderboards' side in the launcher (the extra): the game keeps
//! the player's best laps (`mm2_settings::laps`), and the launcher sends
//! the new ones to the leaderboard server and fetches its boards.
//!
//! The server (`tools/leaderboard/`) takes laps as lines
//! (`POST <address>/laps`) and gives a board's best laps as
//! `rider<TAB>ms` lines (`GET <address>/board?track=&event=&course=&class=`).

use mm2_settings::laps::{self, Board, Lap};
use std::path::Path;
use std::time::Duration;

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .user_agent(concat!("OpenMCM2-launcher/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn base(url: &str) -> &str {
    url.trim().trim_end_matches('/')
}

/// Sends the laps waiting in `dir` to the server at `url`, and forgets
/// them once it has them. Returns how many went.
pub fn send_pending(url: &str, dir: &Path) -> Result<usize, String> {
    let (path, waiting) = laps::pending(dir);
    if waiting.is_empty() {
        return Ok(0);
    }
    agent()
        .post(format!("{}/laps", base(url)))
        .content_type("text/plain; charset=utf-8")
        .send(laps::text(&waiting))
        .map_err(|e| format!("the leaderboard server: {e}"))?;
    // Laps the game added while these were on their way stay.
    let now = laps::read(&path);
    let left: Vec<Lap> = now.into_iter().filter(|l| !waiting.contains(l)).collect();
    let written = if left.is_empty() { std::fs::remove_file(&path) } else { std::fs::write(&path, laps::text(&left)) };
    written.map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(waiting.len())
}

/// The best laps on one board, fastest first.
pub fn fetch(url: &str, board: &Board) -> Result<Vec<(String, u32)>, String> {
    let text = agent()
        .get(format!("{}/board", base(url)))
        .query("track", &board.track)
        .query("event", board.event.to_string())
        .query("course", board.course.to_string())
        .query("class", board.class.to_string())
        .call()
        .map_err(|e| format!("the leaderboard server: {e}"))?
        .body_mut()
        .with_config()
        .limit(1 << 20)
        .read_to_string()
        .map_err(|e| format!("the leaderboard server: {e}"))?;
    Ok(parse_board(&text))
}

fn parse_board(text: &str) -> Vec<(String, u32)> {
    let mut rows: Vec<(String, u32)> = text
        .lines()
        .filter_map(|l| {
            let (rider, ms) = l.split_once('\t')?;
            Some((laps::clean(rider), ms.trim().parse().ok()?))
        })
        .collect();
    rows.sort_by_key(|r| r.1);
    rows
}

/// A lap time as the game shows one: `1:23.45`.
pub fn time(ms: u32) -> String {
    format!("{}:{:02}.{:02}", ms / 60_000, ms / 1000 % 60, ms % 1000 / 10)
}

/// A board's name for a person: `farm, Nationals, course 1, 250cc`.
pub fn name(b: &Board) -> String {
    let event = match b.event {
        1 => "Baja",
        2 => "Nationals",
        3 => "Supercross",
        5 => "Enduro",
        _ => "Race",
    };
    format!("{}, {event}, course {}, {}cc", b.track, b.course + 1, b.class)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boards_read_and_times_show() {
        assert_eq!(parse_board("B\t90000\nA\t83456\njunk\n"), vec![("A".into(), 83_456), ("B".into(), 90_000)]);
        assert_eq!(time(83_456), "1:23.45");
        assert_eq!(time(5_000), "0:05.00");
    }
}
