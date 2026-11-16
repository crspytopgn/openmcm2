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
    request(url, None, false)
}

pub fn post(url: String, body: String) -> Reply {
    request(url, Some(body), false)
}

/// A post over IPv4 only, on a computer: the server browser lists a race
/// at the address its word came from, and the game hosts on IPv4.
pub fn post_v4(url: String, body: String) -> Reply {
    request(url, Some(body), true)
}

#[cfg(not(target_arch = "wasm32"))]
fn request(url: String, body: Option<String>, v4: bool) -> Reply {
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
            .and_then(|mut r| r.body_mut().with_config().limit(1 << 20).read_to_string().map_err(|e| e.to_string()));
        *out.lock().unwrap() = Some(text);
    });
    reply
}

#[cfg(target_arch = "wasm32")]
fn request(url: String, body: Option<String>, _v4: bool) -> Reply {
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
        if !waiting.is_empty() {
            let reply = post(format!("{url}/laps"), laps::text(&waiting));
            after(reply, dir.join(laps::PENDING), move |path| {
                // Laps the game added while these were on their way stay.
                let now = laps::parse_all(&crate::fs::read_to_string(path).unwrap_or_default());
                let left: Vec<_> = now.into_iter().filter(|l| !waiting.contains(l)).collect();
                let _ = crate::fs::write(path, laps::text(&left));
            });
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
            after(reply, dir.join(playtime::SENT), move |path| {
                let _ = crate::fs::write(path, playtime::text(&now));
            });
        }
    }
}

/// Runs `then` once `reply` has come back well, on the sending thread, or
/// in a browser when the answer arrives.
fn after(reply: Reply, path: PathBuf, then: impl FnOnce(&Path) + Send + 'static) {
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(move || loop {
        if let Some(r) = take(&reply) {
            match r {
                Ok(_) => then(&path),
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
                    Ok(_) => then(&path),
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
            Some((laps::clean(f.first()?), f.get(1)?.trim().parse().ok()?, f.get(2).and_then(|c| c.trim().parse().ok()).unwrap_or(0)))
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
    }
}
