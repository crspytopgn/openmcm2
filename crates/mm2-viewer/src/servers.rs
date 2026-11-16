//! The server browser (`mm2_settings::races`): a host's race told to the
//! leaderboard server while it is open, and the open races fetched for the
//! Host or Join box's list. Both are on unless the player turns them off
//! (Extras, `--no-list-races`) or keeps everything on their computer (no
//! leaderboard server).

use crate::online::{self, Reply};
use mm2_settings::races::{self, OpenRace};
use web_time::{Duration, Instant};

/// How often the Host or Join box asks for the list again.
const LOOK_EVERY: Duration = Duration::from_secs(5);

/// A random name for a hosted race, only the host knows: the server takes
/// word of the race only with it.
fn secret() -> String {
    #[cfg(not(target_arch = "wasm32"))]
    let parts = {
        use std::hash::{BuildHasher, Hasher};
        let mut a = std::collections::hash_map::RandomState::new().build_hasher();
        a.write_u128(web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
        let b = std::collections::hash_map::RandomState::new().build_hasher().finish();
        [a.finish(), b ^ std::process::id() as u64]
    };
    #[cfg(target_arch = "wasm32")]
    let parts = [0, 1].map(|_| (js_sys::Math::random() * 2f64.powi(52)) as u64 ^ ((js_sys::Math::random() * 4096.0) as u64) << 52);
    format!("{:016x}{:016x}", parts[0], parts[1])
}

/// A hosted race in the server browser.
pub struct Listing {
    id: String,
    url: String,
    sent: Option<Instant>,
    reply: Option<Reply>,
    /// What the server last said ("ok", "closed", "full"), to tell the
    /// host once when it changes.
    said: String,
}

impl Listing {
    /// A listing, if the player lists their races and has a server.
    pub fn new() -> Option<Listing> {
        // The tests host sessions of their own, not for the list.
        if cfg!(test) || !crate::tweaks::more().list_races {
            return None;
        }
        Some(Listing { id: secret(), url: online::address()?, sent: None, reply: None, said: String::new() })
    }

    /// Tells the server of the race when it's time, `place` being where
    /// players reach it (`("pc", port)` or `("web", code)`; none yet while
    /// a browser's code is on its way). A line for the lobby's chat when
    /// the server's answer is news.
    pub fn tick(&mut self, place: Option<(&str, String)>, race: &OpenRace) -> Option<String> {
        let mut news = None;
        if let Some(r) = self.reply.as_ref().and_then(online::take) {
            self.reply = None;
            let said = match r {
                Ok(t) => t.trim().to_string(),
                Err(e) => {
                    eprintln!("the server browser: {e}");
                    String::new()
                }
            };
            if !said.is_empty() && said != self.said {
                news = match said.as_str() {
                    "ok" => Some("Your race is in the server browser.".to_string()),
                    "closed" => Some(format!(
                        "Not in the server browser: players outside your network can't reach this computer. Forward TCP port {} on your router to list it.",
                        crate::net::PORT
                    )),
                    "full" => Some("Not in the server browser: it's full just now.".to_string()),
                    _ => None,
                };
                self.said = said;
            }
        }
        let due = self.sent.is_none_or(|t| t.elapsed() > Duration::from_secs(races::EVERY));
        if let (true, None, Some((kind, at))) = (due, &self.reply, place) {
            self.sent = Some(Instant::now());
            let body = races::listing(&self.id, (kind, &at), race);
            self.reply = Some(online::post_v4(format!("{}/race", self.url), body));
        }
        news
    }

    /// The race is over: off the list now rather than in 90 seconds.
    pub fn end(&self) {
        if self.sent.is_some() {
            online::post_v4(format!("{}/race/end", self.url), format!("id\t{}\n", self.id));
        }
    }
}

/// The open races, fetched while the Host or Join box is up.
#[derive(Default)]
pub struct Browser {
    /// Another server than the player's (the tests').
    server: Option<String>,
    asked: Option<Instant>,
    reply: Option<Reply>,
    pub races: Vec<OpenRace>,
}

impl Browser {
    /// Asks again when it's time and takes the answer. Whether the list
    /// changed.
    pub fn tick(&mut self) -> bool {
        let Some(url) = self.server.clone().or_else(online::address) else {
            return false;
        };
        let mut changed = false;
        if let Some(r) = self.reply.as_ref().and_then(online::take) {
            self.reply = None;
            match r {
                Ok(text) => {
                    let races = races::parse(&text);
                    changed = races != self.races;
                    self.races = races;
                }
                Err(e) => eprintln!("the server browser: {e}"),
            }
        }
        if self.reply.is_none() && self.asked.is_none_or(|t| t.elapsed() > LOOK_EVERY) {
            self.asked = Some(Instant::now());
            let kind = if cfg!(target_arch = "wasm32") { "web" } else { "pc" };
            self.reply = Some(online::get(format!("{url}/races?kind={kind}")));
        }
        changed
    }
}

/// A race's line in the Host or Join box's list: its name, then where and
/// how many.
pub fn line(r: &OpenRace) -> String {
    let place = if r.track.is_empty() { races::event_name(r.event).to_string() } else { r.track.clone() };
    let racing = if r.racing { ", racing" } else { "" };
    format!("{}  ({place}, {}/{}{racing})", r.name, r.players, crate::net::MAX_PLAYERS)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Against a server run with `npx wrangler dev` in tools/leaderboard:
    /// `MM2_RACES_TEST=http://127.0.0.1:8787 cargo test -p mm2-viewer
    /// servers -- --ignored`.
    #[test]
    #[ignore]
    fn a_race_is_listed_and_found() {
        let url = std::env::var("MM2_RACES_TEST").expect("MM2_RACES_TEST");
        let listener = std::net::TcpListener::bind(("0.0.0.0", 28_229)).unwrap();
        let mut l = Listing { id: secret(), url: url.clone(), sent: None, reply: None, said: String::new() };
        let race = OpenRace {
            address: String::new(),
            name: "Test race".into(),
            event: 3,
            track: "Seattle".into(),
            players: 2,
            racing: false,
            version: "test".into(),
        };
        let mut news = None;
        for _ in 0..100 {
            news = news.or(l.tick(Some(("pc", "28229".into())), &race));
            if news.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert_eq!(news.as_deref(), Some("Your race is in the server browser."));
        let mut b = Browser { server: Some(url), ..Default::default() };
        let mut found = false;
        for _ in 0..100 {
            b.tick();
            if b.races.iter().any(|r| r.name == "Test race" && r.address.ends_with(":28229")) {
                found = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(found, "listed: {:?}", b.races);
        assert_eq!(line(&b.races.iter().find(|r| r.name == "Test race").unwrap()), "Test race  (Seattle, 2/8)");
        l.end();
        std::thread::sleep(Duration::from_millis(1500));
        drop(listener);
    }
}
