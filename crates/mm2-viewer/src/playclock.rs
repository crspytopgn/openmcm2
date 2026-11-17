//! The playtime clock: when the time played was last written down, and
//! the race on now with the keys it adds to (see `mm2_settings::playtime`).
//! It lives outside the game's state so that a browser tab can write the
//! time down and send it as the tab is hidden or closed.

use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;
use web_time::Instant;

struct Race {
    began: Instant,
    keys: Vec<String>,
    /// Whether this race is in the count yet.
    counted: bool,
}

struct Clock {
    mark: Instant,
    race: Option<Race>,
}

static CLOCK: Mutex<Option<Clock>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Clock) -> R) -> R {
    let mut clock = CLOCK.lock().unwrap_or_else(|e| e.into_inner());
    f(clock.get_or_insert_with(|| Clock { mark: Instant::now(), race: None }))
}

/// Starts the clock: the game is open from now.
pub fn start() {
    with(|_| ());
}

/// A race began, adding to `keys` (none for a replay).
pub fn race_began(keys: Option<Vec<String>>) {
    with(|c| c.race = keys.map(|keys| Race { began: Instant::now(), keys, counted: false }));
}

/// Whole seconds since `from`, moving `from` on by them; the part of a
/// second left over counts next time.
fn whole_since(from: &mut Instant, now: Instant) -> u64 {
    let gone = now.duration_since(*from);
    let secs = gone.as_secs();
    *from = now - (gone - Duration::from_secs(secs));
    secs
}

/// The time played since last time: the game open, and the race (to its
/// end with `race_over`, or so far while it goes on).
fn take(race_over: bool) -> Vec<(String, u64)> {
    with(|c| {
        let now = Instant::now();
        let mut more = vec![("total".to_string(), whole_since(&mut c.mark, now))];
        if let Some(race) = &mut c.race {
            let secs = whole_since(&mut race.began, now);
            more.extend(race.keys.iter().map(|k| (k.clone(), secs)));
            if !race.counted {
                race.counted = true;
                more.push(("count".into(), 1));
            }
        }
        if race_over {
            c.race = None;
        }
        more
    })
}

/// Writes down the time played since last time in the user folder `dir`.
pub fn note(dir: &Path, race_over: bool) {
    let more = take(race_over);
    let path = dir.join(mm2_settings::playtime::FILE);
    let old = crate::fs::read_to_string(&path).unwrap_or_default();
    let _ = crate::fs::create_dir_all(dir);
    if let Err(e) = crate::fs::write(&path, mm2_settings::playtime::add(&old, &more)) {
        eprintln!("playtime: {e}");
    }
}

/// The user folder the playtime is kept in, beside the saves.
pub fn dir() -> std::path::PathBuf {
    crate::profile::Store::open().dir
}

/// The tab is being hidden or closed: writes the time down and sends what
/// the server hasn't had yet straight away, as the page may not run again.
/// It is counted as sent at once, so a send the closing tab loses is not
/// sent twice.
#[cfg(target_arch = "wasm32")]
pub fn flush() {
    let dir = dir();
    note(&dir, false);
    crate::online::send_playtime_now(&dir);
}
