//! Sharing tracks with other players, straight from the builder: Share
//! (in Save / Export) sends the track's `.course` to the leaderboard
//! server, and Shared tracks (in Open) lists everyone's, to download,
//! ride and rate. Only the track's recipe goes up (a few KB): a copy of
//! another track names that track's file and the player who downloads it
//! needs that track, and pictures and models of the player's own stay on
//! their computer (`mm2_course::share`).

use super::panel::{self, Input, Painter};
use super::{load_file, Builder, Popup, ACCENT};
use crate::job::Job;
use crate::online::{self, Reply};
use mm2_course::{share, Course, Mode};
use mm2_settings::shared::{self, Mine, Row, Sort};
use std::path::PathBuf;

/// What the builder is doing with shared tracks.
#[derive(Default)]
pub(super) struct Sharing {
    mine: Option<Mine>,
    rows: Vec<Row>,
    /// The list shown: one kind of race (none: all), sorted, which page.
    mode: Option<Mode>,
    sort: Sort,
    page: u32,
    listing: Option<Reply>,
    list_error: Option<String>,
    /// The track being made ready (checked for the game's own files),
    /// then sent: its file stem.
    ready: Option<Job<Result<String, String>>>,
    upload: Option<(Reply, String)>,
    removing: Option<(Reply, String)>,
    download: Option<(Reply, Row)>,
    rating: Option<(Reply, String, u8)>,
}

impl Sharing {
    fn busy(&self) -> bool {
        self.ready.is_some() || self.upload.is_some() || self.removing.is_some()
    }
}

fn mine_path() -> PathBuf {
    crate::profile::user_dir().join(shared::FILE)
}

fn random_seed() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        (js_sys::Math::random() * 2f64.powi(52)) as u64 ^ ((js_sys::Math::random() * 4096.0) as u64) << 52
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        shared::seed()
    }
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// The player's name to put on a shared track: the last profile played.
fn player_name() -> String {
    crate::profile::Store::open().settings().get("MRUProfile").unwrap_or("").to_string()
}

impl Builder {
    fn mine(&mut self) -> &mut Mine {
        if self.sharing.mine.is_none() {
            let mut m = Mine::parse(&crate::fs::read_to_string(mine_path()).unwrap_or_default());
            if m.ensure_voter(random_seed()) {
                let _ = crate::fs::write(mine_path(), m.text());
            }
            self.sharing.mine = Some(m);
        }
        self.sharing.mine.as_mut().unwrap()
    }

    fn save_mine(&mut self) {
        let text = self.mine().text();
        let _ = crate::fs::write(mine_path(), text);
    }

    /// Opens the list of shared tracks.
    pub(super) fn open_shared(&mut self) {
        self.sharing.mode = Some(self.course.mode);
        self.sharing.page = 0;
        self.fetch_shared();
        self.wants_tracks = self.tracks.is_empty();
        self.popup = Some(Popup::Shared { scroll: 0 });
    }

    fn fetch_shared(&mut self) {
        let Some(server) = online::address() else {
            self.sharing.list_error = Some("Sharing uses the leaderboard server, and its address is cleared (Extras, Lap leaderboards).".into());
            return;
        };
        let mode = self.sharing.mode.map_or("", |m| m.name());
        self.sharing.list_error = None;
        self.sharing.listing = Some(online::get(shared::list_url(&server, mode, self.sharing.sort, "", self.sharing.page)));
    }

    /// Makes the track ready to share on another thread (a check that no
    /// file of the game's own is packed in), then sends it.
    fn share_now(&mut self) {
        if self.sharing.busy() {
            return;
        }
        let Some(_) = online::address() else {
            self.say("Sharing needs the leaderboard server (Extras)");
            return;
        };
        if self.course.author.trim().is_empty() {
            self.course.author = player_name();
        }
        self.save();
        let course = self.course.clone();
        self.sharing.ready = Some(Job::spawn(move || ready_to_share(&course)));
        self.say("Getting the track ready to share...");
    }

    fn stop_sharing(&mut self) {
        let stem = self.course.stem();
        let Some((id, key)) = self.mine().shared.get(&stem).cloned() else { return };
        let Some(server) = online::address() else { return };
        self.sharing.removing = Some((online::post(format!("{server}/shared/remove"), format!("{id}\t{key}\n")), stem));
    }

    fn get_shared(&mut self, row: &Row) {
        let Some(server) = online::address() else { return };
        let voter = self.mine().voter.clone();
        self.sharing.download = Some((online::get_big(shared::track_url(&server, &row.id, &voter)), row.clone()));
        self.say(&format!("Downloading {}...", row.name));
    }

    fn rate(&mut self, id: &str, stars: u8) {
        let Some(server) = online::address() else { return };
        let voter = self.mine().voter.clone();
        self.sharing.rating = Some((online::post(format!("{server}/shared/rate"), format!("{id}\t{voter}\t{stars}\n")), id.to_string(), stars));
    }

    fn report(&mut self, id: &str) {
        let Some(server) = online::address() else { return };
        let voter = self.mine().voter.clone();
        let _ = online::post(format!("{server}/shared/report"), format!("{id}\t{voter}\n"));
        self.say("Thanks, it's been reported");
    }

    /// The track a copy was made from, among the tracks there are here.
    fn find_base(&self, file: &str) -> Option<PathBuf> {
        self.tracks.iter().map(|t| &t.path).find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(file))).cloned()
    }

    /// Answers from the server, looked at each frame.
    pub(super) fn poll_sharing(&mut self) {
        if let Some(r) = self.sharing.listing.as_ref().and_then(online::take) {
            self.sharing.listing = None;
            match r {
                Ok(text) => self.sharing.rows = shared::parse_rows(&text),
                Err(e) => {
                    eprintln!("shared tracks: {e}");
                    self.sharing.list_error = Some("Couldn't reach the server. Check the internet and try again.".into());
                }
            }
        }
        if self.sharing.ready.as_ref().is_some_and(|j| j.is_finished()) {
            match self.sharing.ready.take().unwrap().join() {
                Ok(Ok(json)) => {
                    let stem = self.course.stem();
                    let again = self.mine().shared.get(&stem).cloned();
                    let (voter, server) = (self.mine().voter.clone(), online::address().unwrap_or_default());
                    let url = shared::share_url(&server, &voter, again.as_ref().map(|(i, k)| (i.as_str(), k.as_str())));
                    self.sharing.upload = Some((online::post(url, json), stem));
                    self.say("Sharing...");
                }
                Ok(Err(e)) => self.say(&e),
                Err(_) => self.say("Couldn't get the track ready to share"),
            }
        }
        if let Some(r) = self.sharing.upload.as_ref().and_then(|(r, _)| online::take(r)) {
            let (_, stem) = self.sharing.upload.take().unwrap();
            match r.map_err(|e| {
                eprintln!("sharing: {e}");
                "Couldn't reach the server. Try again in a moment".to_string()
            })
            .and_then(|t| shared::parse_shared(&t))
            {
                Ok((id, key)) => {
                    self.mine().shared.insert(stem, (id, key));
                    self.save_mine();
                    self.say("Shared! Players find it under Open, Shared tracks");
                }
                Err(e) => self.say(&e),
            }
        }
        if let Some(r) = self.sharing.removing.as_ref().and_then(|(r, _)| online::take(r)) {
            let (_, stem) = self.sharing.removing.take().unwrap();
            match r.as_deref().map(str::trim) {
                Ok("ok") | Ok("gone") => {
                    self.mine().shared.remove(&stem);
                    self.save_mine();
                    self.say("No longer shared");
                }
                _ => self.say("Couldn't reach the server. Try again in a moment"),
            }
        }
        if let Some(r) = self.sharing.rating.as_ref().and_then(|(r, _, _)| online::take(r)) {
            let (_, id, stars) = self.sharing.rating.take().unwrap();
            if let Ok(t) = r {
                let f: Vec<&str> = t.trim().split('\t').collect();
                if let ["ok", s, v] = f.as_slice() {
                    if let Some(row) = self.sharing.rows.iter_mut().find(|r| r.id == id) {
                        row.stars = s.parse().unwrap_or(row.stars);
                        row.votes = v.parse().unwrap_or(row.votes);
                    }
                    self.mine().votes.insert(id, stars);
                    self.save_mine();
                    self.say(&format!("Rated {stars} of 5, thanks"));
                }
            } else {
                self.say("Couldn't reach the server");
            }
        }
        if let Some(r) = self.sharing.download.as_ref().and_then(|(r, _)| online::take(r)) {
            let (_, row) = self.sharing.download.take().unwrap();
            match r.map_err(|_| "Couldn't download it. Try again in a moment".to_string()).and_then(|t| self.save_shared(&row, &t)) {
                Ok(()) => {}
                Err(e) => self.say(&e),
            }
        }
    }

    /// Keeps a downloaded track among the player's own, opens it and puts
    /// it in the game's track lists.
    fn save_shared(&mut self, row: &Row, json: &str) -> Result<(), String> {
        let mut c = Course::from_json(json).map_err(|_| "That track couldn't be read".to_string())?;
        if let Some(b) = &mut c.base {
            let file = share::file_name(&b.file).to_string();
            let Some(path) = self.find_base(&file) else {
                return Err(format!("It's a copy of {file}, which isn't in your game"));
            };
            b.file = path.to_string_lossy().to_string();
        }
        // Its own file name, unless one of the player's tracks has it (a
        // track downloaded again replaces itself)...
        let again = self.mine().got.get(&row.id).cloned();
        // ... nor any track in the game's list for its race (never put in
        // place of a track that's there).
        let stems = |files: Vec<PathBuf>| files.into_iter().filter_map(|f| f.file_stem().map(|s| s.to_string_lossy().to_ascii_uppercase())).collect::<Vec<_>>();
        let folder = match &self.game {
            Some(g) => g.join("TERAFORM"),
            None => self.dir.join("TERAFORM"),
        }
        .join(c.mode.folder());
        let envs: Vec<PathBuf> = crate::fs::read_dir(&folder).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("env"))).collect();
        let mut taken = stems(self.course_files());
        taken.extend(stems(envs));
        taken.retain(|s| Some(s) != again.as_ref());
        c.name = shared::free_name(&c.name, &taken).ok_or("No room for another track of that name")?;
        let stem = c.stem();
        let path = self.dir.join(format!("{stem}.course"));
        let _ = crate::fs::create_dir_all(&self.dir);
        crate::fs::write(&path, c.to_json()).map_err(|e| format!("Couldn't save it: {e}"))?;
        self.mine().got.insert(row.id.clone(), stem);
        self.save_mine();
        let opened = load_file(&path).ok_or("That track couldn't be opened")?;
        self.open_copy(opened);
        self.unsaved = false;
        self.popup = None;
        // Into the game's track lists too, to race it from the menus.
        self.bake_track(false);
        self.say(&format!("{} by {} is yours to ride. It's in the track list too", row.name, if row.author.is_empty() { "a player" } else { &row.author }));
        Ok(())
    }

    /// The Share pop-up: what goes up, and the button that sends it.
    pub(super) fn share_popup(&mut self, p: &mut Painter, input: &mut Input, w: f32) -> bool {
        let (pw, ph) = (380.0, 236.0);
        let (px, py) = ((w - pw) / 2.0, 90.0);
        p.fill([0.0, 0.0, w, 480.0], [0.0, 0.0, 0.0, 0.35]);
        p.panel(input, [px, py, pw, ph]);
        p.text([px + 14.0, py + 12.0], "SHARE WITH OTHER PLAYERS", 14.0, true, [ACCENT[0], ACCENT[1], ACCENT[2], 1.0]);
        let soft = [0.85, 0.85, 0.8, 1.0];
        let stem = self.course.stem();
        let shared_before = self.mine().shared.contains_key(&stem);
        let author = if self.course.author.trim().is_empty() { player_name() } else { self.course.author.clone() };
        let mut say = format!(
            "\"{}\" goes up for everyone with OpenMCM2 to download, ride and rate, in the game, the launcher and the browser version, by {}.",
            self.course.name,
            if author.is_empty() { "you (no name)".to_string() } else { author.clone() }
        );
        if let Some(b) = &self.course.base {
            say += &format!(" It's a copy of {}: only your changes go up, and players need that track to ride it.", share::file_name(&b.file));
        }
        say += " Only its recipe goes up (the line, jumps, shaping, paint and plants, a few KB): nothing of the game's own.";
        if let Ok((_, left)) = share::for_sharing(&self.course) {
            if !left.is_empty() {
                say += &format!(" {} stay on your computer: players get the track without them.", capital(&left.join(", ")));
            }
        } else if self.course.land.heightmap.is_some() {
            say += " Its hills come from a height map picture of yours, so it can't be shared.";
        }
        let mut y = py + 34.0;
        for line in panel::wrap(p, &say, pw - 28.0, 11.0) {
            p.text([px + 14.0, y], &line, 11.0, false, soft);
            y += 13.0;
        }
        let mut keep = true;
        let busy = self.sharing.busy();
        let label = if busy {
            "Sharing..."
        } else if shared_before {
            "Share this version"
        } else {
            "Share it"
        };
        let tip = if shared_before {
            "You shared this track before: this puts the track as it is now in its place. Its downloads and ratings stay."
        } else {
            "Sends the track to the shared tracks list now. You can share a new version or stop sharing it later from here."
        };
        if p.button(input, "share-go", [px + 14.0, py + ph - 92.0, pw - 28.0, 28.0], label, tip, true) && !busy {
            self.share_now();
        }
        if shared_before {
            let removing = self.sharing.removing.is_some();
            if p.button(input, "share-stop", [px + 14.0, py + ph - 58.0, 160.0, 22.0], if removing { "Removing..." } else { "Stop sharing" }, "Takes this track off the shared list. Players who downloaded it keep their copy.", false) && !removing {
                self.stop_sharing();
            }
        }
        if p.button(input, "share-close", [px + pw - 84.0, py + ph - 30.0, 70.0, 20.0], "Close", "Back to the track.", false) {
            keep = false;
        }
        keep
    }

    /// The Shared tracks pop-up: everyone's tracks, to download and rate.
    pub(super) fn shared_popup(&mut self, p: &mut Painter, input: &mut Input, w: f32, scroll: &mut usize) -> bool {
        let (pw, ph) = (560.0, 380.0);
        let (px, py) = ((w - pw) / 2.0, 50.0);
        let accent = [ACCENT[0], ACCENT[1], ACCENT[2], 1.0];
        let soft = [0.85, 0.85, 0.8, 1.0];
        let dim = [0.6, 0.6, 0.55, 1.0];
        p.fill([0.0, 0.0, w, 480.0], [0.0, 0.0, 0.0, 0.35]);
        p.panel(input, [px, py, pw, ph]);
        p.text([px + 14.0, py + 12.0], "SHARED TRACKS", 14.0, true, accent);
        p.text([px + 140.0, py + 14.0], "Tracks other players made in the Course Builder", 11.0, false, dim);
        let mut keep = true;
        let mut fetch = false;
        // The kind of race, and how they're sorted.
        let modes: Vec<Option<Mode>> = std::iter::once(None).chain(Mode::ALL.iter().copied().map(Some)).collect();
        let bw = 70.0;
        for (k, m) in modes.iter().enumerate() {
            let label = m.map_or("All", |m| m.name());
            if p.button(input, &format!("sh-mode{k}"), [px + 14.0 + k as f32 * (bw + 2.0), py + 34.0, bw, 18.0], label, "Show this kind of race's shared tracks.", self.sharing.mode == *m) {
                self.sharing.mode = *m;
                self.sharing.page = 0;
                fetch = true;
            }
        }
        for (k, s) in Sort::ALL.iter().enumerate() {
            let sw = 98.0;
            let r = [px + 14.0 + k as f32 * (sw + 2.0), py + 56.0, sw, 18.0];
            let tip = match s {
                Sort::New => "The tracks shared or changed most recently first.",
                Sort::Top => "The best rated first (a track with only a vote or two counts for a little less).",
                Sort::Most => "The most downloaded first.",
            };
            if p.button(input, &format!("sh-sort{k}"), r, s.label(), tip, self.sharing.sort == *s) {
                self.sharing.sort = *s;
                self.sharing.page = 0;
                fetch = true;
            }
        }
        if p.button(input, "sh-refresh", [px + pw - 84.0, py + 56.0, 70.0, 18.0], "Refresh", "Ask the server for the list again.", false) {
            fetch = true;
        }
        // The list.
        let top = py + 82.0;
        let rows = 11usize;
        let row_h = 24.0;
        let have: Vec<String> = self.tracks.iter().filter_map(|t| t.path.file_name().map(|n| n.to_string_lossy().to_ascii_uppercase())).collect();
        if input.pointer.0 >= px && input.pointer.0 < px + pw && input.pointer.1 > top && input.pointer.1 < top + rows as f32 * row_h {
            if input.wheel < 0.0 {
                *scroll = (*scroll + 1).min(self.sharing.rows.len().saturating_sub(rows));
            } else if input.wheel > 0.0 {
                *scroll = scroll.saturating_sub(1);
            }
        }
        if let Some(e) = &self.sharing.list_error {
            for (k, line) in panel::wrap(p, e, pw - 28.0, 11.0).iter().enumerate() {
                p.text([px + 14.0, top + 6.0 + k as f32 * 13.0], line, 11.0, false, soft);
            }
        } else if self.sharing.listing.is_some() && self.sharing.rows.is_empty() {
            p.text([px + 14.0, top + 6.0], "Asking the server...", 11.0, false, soft);
        } else if self.sharing.rows.is_empty() {
            p.text([px + 14.0, top + 6.0], "None shared yet. Be the first: Save / Export, Share with other players.", 11.0, false, soft);
        }
        let mine_ids: Vec<String> = self.mine().shared.values().map(|(id, _)| id.clone()).collect();
        let got: Vec<String> = self.mine().got.keys().cloned().collect();
        let voted = self.mine().votes.clone();
        let list = self.sharing.rows.clone();
        let downloading = self.sharing.download.as_ref().map(|(_, r)| r.id.clone());
        for (k, row) in list.iter().skip(*scroll).take(rows).enumerate() {
            let y = top + k as f32 * row_h;
            let x = px + 14.0;
            let name = if row.author.is_empty() { row.name.clone() } else { format!("{}  by {}", row.name, row.author) };
            let mut shown = name.clone();
            while shown.chars().count() > 4 && p.text_width(&shown, 11.0, false) > 210.0 {
                shown.pop();
            }
            if shown != name {
                shown.push_str("...");
            }
            p.text([x, y + 2.0], &shown, 11.0, true, soft);
            let info = format!("{}  {} downloads  {}", row.mode, row.downloads, row.rating());
            p.text([x, y + 13.0], &info, 9.0, false, dim);
            // Get, or why it can't be had.
            let needs = (!row.base.is_empty() && !have.contains(&row.base.to_ascii_uppercase())).then(|| row.base.clone());
            let label = if downloading.as_deref() == Some(row.id.as_str()) {
                "Getting..."
            } else if needs.is_some() {
                "Needs track"
            } else if got.contains(&row.id) {
                "Get again"
            } else {
                "Get"
            };
            let tip = match &needs {
                Some(f) => format!("A copy of {f}, which isn't in your game. Add that track to ride this one."),
                None if row.base.is_empty() => "Downloads the track into your tracks and the game's track list, and opens it here to ride or change.".to_string(),
                None => format!("A copy of {}, which you have. Downloads the player's changes into your tracks and the track list, and opens it here.", row.base),
            };
            let gx = px + 240.0;
            if p.button(input, &format!("sh-get{}", row.id), [gx, y + 1.0, 74.0, 20.0], label, &tip, false) && needs.is_none() && downloading.is_none() {
                self.get_shared(row);
            }
            // Stars, once downloaded (one vote each, changeable).
            let mine = mine_ids.contains(&row.id);
            if got.contains(&row.id) && !mine {
                let given = voted.get(&row.id).copied().unwrap_or(0);
                for s in 1..=5u8 {
                    let r = [gx + 80.0 + (s - 1) as f32 * 22.0, y + 1.0, 20.0, 20.0];
                    let tip = format!("Rate this track {s} of 5. You can change your rating any time.");
                    if p.button(input, &format!("sh-star{}-{s}", row.id), r, &s.to_string(), &tip, s <= given) {
                        self.rate(&row.id, s);
                    }
                }
                if p.button(input, &format!("sh-rep{}", row.id), [gx + 194.0, y + 1.0, 52.0, 20.0], "Report", "Tell us this track has something it shouldn't: the game's own files, or something rude. It's taken off the list once a few players report it.", false) {
                    self.report(&row.id);
                }
            } else if mine {
                p.text([gx + 82.0, y + 6.0], "Shared by you", 10.0, false, dim);
            } else {
                p.text([gx + 82.0, y + 6.0], "Get it to rate it", 10.0, false, dim);
            }
        }
        // Pages.
        let by = py + ph - 30.0;
        if self.sharing.page > 0 && p.button(input, "sh-prev", [px + 14.0, by, 70.0, 20.0], "< Back", "The page before.", false) {
            self.sharing.page -= 1;
            fetch = true;
        }
        if self.sharing.rows.len() >= 60 && p.button(input, "sh-next", [px + 88.0, by, 70.0, 20.0], "More >", "The next page of tracks.", false) {
            self.sharing.page += 1;
            fetch = true;
        }
        if self.sharing.rows.len() > rows {
            p.text([px + 166.0, by + 5.0], &format!("{}-{} of {} (wheel scrolls)", *scroll + 1, (*scroll + rows).min(self.sharing.rows.len()), self.sharing.rows.len()), 10.0, false, dim);
        }
        if p.button(input, "sh-close", [px + pw - 84.0, by, 70.0, 20.0], "Close", "Back to the track.", false) {
            keep = false;
        }
        if fetch {
            *scroll = 0;
            self.fetch_shared();
        }
        keep
    }
}

/// The track's recipe as it goes up: the builder's own fields alone, the
/// player's own pictures and models left on their computer (and said).
fn ready_to_share(course: &Course) -> Result<String, String> {
    let (out, left) = share::for_sharing(course)?;
    let json = out.to_json_compact();
    if json.len() > share::MOST_BYTES {
        return Err(format!("The track's recipe is {} KB, more than the {} KB that can be shared", json.len() / 1024, share::MOST_BYTES / 1024));
    }
    if !left.is_empty() {
        eprintln!("sharing without {}", left.join(", "));
    }
    Ok(json)
}
