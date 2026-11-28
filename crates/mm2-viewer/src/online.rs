//! The leaderboard server (`tools/leaderboard/`): sending the player's new
//! best laps and playtime after a race, and fetching the boards for the
//! High Scores screen. On a thread with `ureq`, or in a browser with
//! `fetch`; either way the answer lands in a [`Reply`] the caller looks at
//! each frame.

use mm2_settings::{laps, playtime};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// An answer still on its way (`None`), or the server's text.
pub type Reply = Arc<Mutex<Option<Result<String, String>>>>;

/// The server's address, or none when the player keeps everything on
/// their computer.
pub fn address() -> Option<String> {
    let url = crate::tweaks::more().leaderboard_url.trim().trim_end_matches('/').to_string();
    (!url.is_empty()).then_some(url)
}

pub fn get(url: String) -> Reply {
    request(url, None, false, 1 << 20)
}

/// A get whose answer can be a few MB (a shared track).
pub fn get_big(url: String) -> Reply {
    request(url, None, false, mm2_settings::shared::MOST_BYTES as u64 + 4096)
}

pub fn post(url: String, body: String) -> Reply {
    request(url, Some(body), false, 1 << 20)
}

/// A post over IPv4 only, on a computer: the server browser lists a race
/// at the address its word came from, and the game hosts on IPv4.
pub fn post_v4(url: String, body: String) -> Reply {
    request(url, Some(body), true, 1 << 20)
}

#[cfg(not(target_arch = "wasm32"))]
fn request(url: String, body: Option<String>, v4: bool, limit: u64) -> Reply {
    let reply: Reply = Arc::default();
    let out = reply.clone();
    std::thread::spawn(move || {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(20)))
            .user_agent(concat!("OpenMCM2/", env!("CARGO_PKG_VERSION")))
            .ip_family(if v4 { ureq::config::IpFamily::Ipv4Only } else { ureq::config::IpFamily::Any })
            .build()
            .into();
        let sent = match body {
            Some(b) => agent.post(&url).content_type("text/plain; charset=utf-8").send(b),
            None => agent.get(&url).call(),
        };
        let text = sent
            .map_err(|e| e.to_string())
            .and_then(|mut r| r.body_mut().with_config().limit(limit).read_to_string().map_err(|e| e.to_string()));
        *out.lock().unwrap() = Some(text);
    });
    reply
}

#[cfg(target_arch = "wasm32")]
fn request(url: String, body: Option<String>, _v4: bool, _limit: u64) -> Reply {
    use wasm_bindgen::JsCast;
    let reply: Reply = Arc::default();
    let out = reply.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let text = async {
            let init = web_sys::RequestInit::new();
            if let Some(b) = &body {
                // A plain text body needs no preflight from another site.
                init.set_method("POST");
                init.set_body(&wasm_bindgen::JsValue::from_str(b));
            }
            let window = web_sys::window().ok_or("no window")?;
            let got = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str_and_init(&url, &init))
                .await
                .map_err(|e| format!("{e:?}"))?;
            let response: web_sys::Response = got.dyn_into().map_err(|_| "not a response")?;
            if !response.ok() {
                return Err(format!("status {}", response.status()));
            }
            let text = wasm_bindgen_futures::JsFuture::from(response.text().map_err(|e| format!("{e:?}"))?)
                .await
                .map_err(|e| format!("{e:?}"))?;
            Ok(text.as_string().unwrap_or_default())
        }
        .await;
        *out.lock().unwrap() = Some(text.map_err(|e: String| e));
    });
    reply
}

/// Takes the answer if it has come.
pub fn take(reply: &Reply) -> Option<Result<String, String>> {
    reply.lock().ok()?.take()
}

/// Sends the laps waiting in the user folder `dir` and the playtime not
/// sent yet, as the player's settings allow, and forgets them once the
/// server has them.
pub fn send_saved(dir: &Path) {
    let Some(url) = address() else { return };
    let more = crate::tweaks::more();
    if more.leaderboards {
        let waiting = laps::parse_all(&crate::fs::read_to_string(dir.join(laps::PENDING)).unwrap_or_default());
        // Best laps' ghosts go up with the leaderboards, so there are
        // ghosts to race for those who turn the online ghosts extra on.
        let ghosts = Some((url.clone(), dir.to_path_buf()));
        if !waiting.is_empty() {
            let reply = post(format!("{url}/laps"), laps::text(&waiting));
            after(reply, dir.join(laps::PENDING), move |path, _| {
                // Laps the game added while these were on their way stay.
                let now = laps::parse_all(&crate::fs::read_to_string(path).unwrap_or_default());
                let left: Vec<_> = now.into_iter().filter(|l| !waiting.contains(l)).collect();
                let _ = crate::fs::write(path, laps::text(&left));
                // A ghost goes once the server has its lap.
                if let Some((url, dir)) = ghosts {
                    send_ghosts(&url, &dir);
                }
            });
        } else if let Some((url, dir)) = ghosts {
            send_ghosts(&url, &dir);
        }
    }
    if more.share_playtime {
        let read = |n: &str| playtime::parse(&crate::fs::read_to_string(dir.join(n)).unwrap_or_default());
        let (now, sent) = (read(playtime::FILE), read(playtime::SENT));
        let mut new = playtime::new_since(&now, &sent);
        if !new.is_empty() {
            // A first send counts one more player, with nothing to say who.
            if sent.is_empty() {
                new.insert("players".into(), 1);
            }
            let reply = post(format!("{url}/playtime"), playtime::text(&new));
            after(reply, dir.join(playtime::SENT), move |path, _| {
                let _ = crate::fs::write(path, playtime::text(&now));
            });
        }
    }
}

/// Sends the playtime not sent yet with the browser's beacon, which goes
/// even as the tab closes, and counts it as sent straight away.
#[cfg(target_arch = "wasm32")]
pub fn send_playtime_now(dir: &Path) {
    let Some(url) = address() else { return };
    if !crate::tweaks::more().share_playtime {
        return;
    }
    let read = |n: &str| playtime::parse(&crate::fs::read_to_string(dir.join(n)).unwrap_or_default());
    let (now, sent) = (read(playtime::FILE), read(playtime::SENT));
    let mut new = playtime::new_since(&now, &sent);
    if new.is_empty() {
        return;
    }
    if sent.is_empty() {
        new.insert("players".into(), 1);
    }
    let Some(navigator) = web_sys::window().map(|w| w.navigator()) else { return };
    if navigator.send_beacon_with_opt_str(&format!("{url}/playtime"), Some(&playtime::text(&new))).unwrap_or(false) {
        let _ = crate::fs::write(&dir.join(playtime::SENT), playtime::text(&now));
    }
}

/// Runs `then` with the answer once `reply` has come back well, on the sending thread, or
/// in a browser when the answer arrives.
fn after(reply: Reply, path: PathBuf, then: impl FnOnce(&Path, &str) + Send + 'static) {
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(move || loop {
        if let Some(r) = take(&reply) {
            match r {
                Ok(t) => then(&path, &t),
                Err(e) => eprintln!("the leaderboard server: {e}"),
            }
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    });
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(async move {
        loop {
            if let Some(r) = take(&reply) {
                match r {
                    Ok(t) => then(&path, &t),
                    Err(e) => web_sys::console::log_1(&format!("the leaderboard server: {e}").into()),
                }
                return;
            }
            // Yield to the browser until the fetch is done.
            let wait = js_sys::Promise::new(&mut |done, _| {
                let _ = web_sys::window().map(|w| w.set_timeout_with_callback_and_timeout_and_arguments_0(&done, 100));
            });
            let _ = wasm_bindgen_futures::JsFuture::from(wait).await;
        }
    });
}

/// The online ghosts waiting to go up, in the user folder: one file for
/// each board, its lap's line then the tape in base64 on the next, as
/// `POST /ghost` takes it.
pub const GHOSTS_PENDING: &str = "ghosts-pending";

/// Keeps a new best lap's ghost to send with the laps.
pub fn keep_ghost(dir: &Path, lap: &laps::Lap, tape: &mm2_formats::vcr::Tape) -> std::io::Result<()> {
    let folder = dir.join(GHOSTS_PENDING);
    crate::fs::create_dir_all(&folder)?;
    let name: String = lap.board.key().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    crate::fs::write(&folder.join(format!("{name}.txt")), ghost_body(lap, tape))
}

/// What `POST /ghost` takes: the lap's line, then its tape in base64.
pub fn ghost_body(lap: &laps::Lap, tape: &mm2_formats::vcr::Tape) -> String {
    format!("{}\n{}\n", lap.line(), base64(&tape.write()))
}

/// Sends the ghosts waiting, each forgotten once the server has answered
/// (it keeps only the fastest lap's), unless it asks for it again later
/// ("wait": the lap isn't on its board yet).
fn send_ghosts(url: &str, dir: &Path) {
    let Ok(files) = crate::fs::read_dir(&dir.join(GHOSTS_PENDING)) else { return };
    for path in files.filter_map(|e| e.ok().map(|e| e.path())) {
        let Ok(body) = crate::fs::read_to_string(&path) else { continue };
        let reply = post(format!("{url}/ghost"), body);
        after(reply, path, move |path, answer| {
            if !answer.starts_with("wait") {
                let _ = crate::fs::remove_file(path);
            }
        });
    }
}

/// One ghost on the server: the fastest lap of a track and course that
/// went up with its ghost.
#[derive(Debug, Clone, PartialEq)]
pub struct GhostInfo {
    pub track: String,
    pub event: u32,
    pub course: u32,
    pub class: u32,
    pub rider: String,
    pub ms: u32,
    pub title: String,
}

impl GhostInfo {
    /// The `GET /ghost` address of its tape.
    pub fn address(&self, url: &str) -> String {
        format!("{url}/ghost?track={}&event={}&course={}", escape(&self.track), self.event, self.course)
    }
}

/// The `GET /ghosts` answer: lines of
/// `track<TAB>event<TAB>course<TAB>class<TAB>rider<TAB>ms<TAB>title`.
pub fn parse_ghosts(text: &str) -> Vec<GhostInfo> {
    text.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            Some(GhostInfo {
                track: laps::clean(f.first()?),
                event: f.get(1)?.trim().parse().ok()?,
                course: f.get(2)?.trim().parse().ok()?,
                class: f.get(3)?.trim().parse().ok()?,
                rider: laps::clean(f.get(4)?),
                ms: f.get(5)?.trim().parse().ok()?,
                title: f.get(6).map_or_else(String::new, |t| laps::clean_title(t)),
            })
        })
        .filter(|g| !g.track.is_empty())
        .collect()
}

/// The `GET /ghost` answer: the tape, in base64.
pub fn parse_ghost(text: &str) -> Option<mm2_formats::vcr::Tape> {
    mm2_formats::vcr::Tape::parse(&unbase64(text.trim())?).ok()
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            if k <= c.len() {
                out.push(B64[(n >> (18 - 6 * k) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub fn unbase64(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut n, mut bits) = (0u32, 0);
    for b in text.bytes() {
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b'\n' | b'\r' => continue,
            _ => return None,
        };
        n = n << 6 | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((n >> bits) as u8);
            n &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// One board on the server: its track, course and class, how many riders
/// are on it, and the track's name.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardInfo {
    pub track: String,
    pub course: u32,
    pub riders: u32,
    pub title: String,
}

/// The `GET /boards?event=` answer: every track with laps in the event, one
/// line each (`track<TAB>course<TAB>riders<TAB>title`), best known first.
pub fn parse_boards(text: &str) -> Vec<BoardInfo> {
    text.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            Some(BoardInfo {
                track: laps::clean(f.first()?),
                course: f.get(1)?.trim().parse().ok()?,
                riders: f.get(2)?.trim().parse().ok()?,
                title: f.get(3).map_or_else(String::new, |t| laps::clean_title(t)),
            })
        })
        .collect()
}

/// The `GET /board?track=&event=&course=` answer with every class: rows
/// of rider, time (ms) and the bike's class, fastest first.
pub fn parse_board(text: &str) -> Vec<(String, u32, u32)> {
    let mut rows: Vec<(String, u32, u32)> = text
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            Some((mm2_settings::clean::mask(&laps::clean(f.first()?)), f.get(1)?.trim().parse().ok()?, f.get(2).and_then(|c| c.trim().parse().ok()).unwrap_or(0)))
        })
        .collect();
    rows.sort_by_key(|r| r.1);
    rows
}

/// A query part, its few special characters escaped.
pub fn escape(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_read() {
        let b = parse_boards("farm\t0\t12\tMunchberry Farms\nsx01\t0\t3\n\nbad\n");
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].title, "Munchberry Farms");
        assert_eq!(b[1].riders, 3);
        assert_eq!(parse_board("B\t90000\t250\nA\t83456\t125\n"), vec![("A".into(), 83_456, 125), ("B".into(), 90_000, 250)]);
        assert_eq!(escape("rat's hole"), "rat%27s%20hole");
        let g = parse_ghosts("farm\t2\t0\t250\tMike\t83456\tMunchberry Farms\n\tbad\n");
        assert_eq!(g.len(), 1);
        assert_eq!((g[0].event, g[0].class, g[0].ms, g[0].title.as_str()), (2, 250, 83_456, "Munchberry Farms"));
        assert_eq!(g[0].address("https://x"), "https://x/ghost?track=farm&event=2&course=0");
    }

    #[test]
    fn base64_goes_both_ways() {
        for n in 0..12 {
            let bytes: Vec<u8> = (0..n).map(|i| (i * 37 + 5) as u8).collect();
            assert_eq!(unbase64(&base64(&bytes)).unwrap(), bytes);
        }
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(unbase64("TWE=").unwrap(), b"Ma");
    }
}
