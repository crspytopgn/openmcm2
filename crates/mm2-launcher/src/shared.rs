//! The Tracks page's shared Course Builder tracks: tracks players made in
//! the Course Builder and shared from it, listed by the leaderboard
//! server. Getting one downloads its `.course` into the builder's folder
//! (so it opens in the builder too) and has the game bake it into each
//! track folder (`mm2-viewer --bake-course`), so it's in the track lists
//! with either graphics.

use crate::game::find_ci;
use crate::look;
use crate::settings::user_dir;
use eframe::egui;
use mm2_settings::shared::{self, Mine, Row, Sort};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

type Slot<T> = Arc<Mutex<Option<Result<T, String>>>>;

const KINDS: [(&str, &str, &str); 4] = [("Supercross", "Supercross", "SX"), ("National", "Nationals", "NATIONAL"), ("Baja", "Baja", "BAJA"), ("Enduro", "Enduro", "ENDURO")];

#[derive(Default)]
pub struct SharedTracks {
    mine: Option<Mine>,
    /// The kind of race shown (an index into KINDS), or all.
    kind: Option<usize>,
    sort: Sort,
    search: String,
    page: u32,
    rows: Option<Vec<Row>>,
    fetching: Option<Slot<Vec<Row>>>,
    error: Option<String>,
    /// Downloads by id, and how the finished ones went.
    getting: std::collections::HashMap<String, Slot<String>>,
    failed: std::collections::HashMap<String, String>,
    done: std::collections::HashMap<String, String>,
    rating: Option<Slot<(String, u8, u32, u32)>>,
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(40)))
        .user_agent(concat!("OpenMCM2-launcher/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn get_text(url: &str, limit: u64) -> Result<String, String> {
    let mut r = agent().get(url).call().map_err(|e| e.to_string())?;
    r.body_mut().with_config().limit(limit).read_to_string().map_err(|e| e.to_string())
}

fn post_text(url: &str, body: String) -> Result<String, String> {
    let mut r = agent().post(url).content_type("text/plain; charset=utf-8").send(body).map_err(|e| e.to_string())?;
    r.body_mut().with_config().limit(1 << 16).read_to_string().map_err(|e| e.to_string())
}

fn mine_path() -> PathBuf {
    user_dir().join(shared::FILE)
}

impl SharedTracks {
    fn mine(&mut self) -> &mut Mine {
        if self.mine.is_none() {
            let mut m = Mine::read_from(&user_dir());
            if m.ensure_voter(shared::seed()) {
                let _ = std::fs::create_dir_all(user_dir());
                let _ = std::fs::write(mine_path(), m.text());
            }
            self.mine = Some(m);
        }
        self.mine.as_mut().unwrap()
    }

    fn fetch(&mut self, server: &str) {
        let mode = self.kind.map_or("", |k| KINDS[k].0);
        let url = shared::list_url(server, mode, self.sort, &self.search, self.page);
        let slot: Slot<Vec<Row>> = Arc::default();
        let s = slot.clone();
        std::thread::spawn(move || *s.lock().unwrap() = Some(get_text(&url, 1 << 20).map(|t| shared::parse_rows(&t))));
        self.fetching = Some(slot);
        self.error = None;
    }
}

/// Downloads a shared track, keeps its `.course` among the builder's
/// tracks and bakes it into each of `homes` (the game folder and the
/// Free Pack's). Answers the name it was saved as.
fn get_track(server: &str, row: &Row, voter: &str, again: Option<String>, homes: &[PathBuf], viewer: &Path, args: &[String]) -> Result<String, String> {
    let json = get_text(&shared::track_url(server, &row.id, voter), shared::MOST_BYTES as u64 + 4096)?;
    let mut course: serde_json::Value = serde_json::from_str(&json).map_err(|_| "the server sent something that isn't a track".to_string())?;
    let folder = KINDS.iter().find(|k| k.0 == row.mode).map(|k| k.2).ok_or("an unknown kind of race")?;
    // A copy names the track it was made from: the one in the game here.
    if let Some(base) = course.get_mut("base").filter(|b| b.is_object()) {
        let file = base.get("file").and_then(|f| f.as_str()).unwrap_or("").rsplit(['/', '\\']).next().unwrap_or("").to_string();
        let found = homes.iter().find_map(|h| find_ci(h, &format!("TERAFORM/{folder}/{file}")));
        let Some(path) = found else {
            return Err(format!("it's a copy of {file}, which isn't in your game"));
        };
        base["file"] = serde_json::Value::String(path.to_string_lossy().to_string());
    }
    // Its own short name, apart from the player's tracks and the game's.
    let courses = user_dir().join("Courses");
    let stems_in = |dir: &Path, ext: &str| -> Vec<String> {
        std::fs::read_dir(dir)
            .map(|r| r.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case(ext))).filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().to_ascii_uppercase())).collect())
            .unwrap_or_default()
    };
    let mut taken = stems_in(&courses, "course");
    for h in homes {
        if let Some(d) = find_ci(h, &format!("TERAFORM/{folder}")) {
            taken.extend(stems_in(&d, "env"));
        }
    }
    taken.retain(|s| Some(s) != again.as_ref());
    let name = course.get("name").and_then(|n| n.as_str()).unwrap_or(&row.name).to_string();
    let name = shared::free_name(&name, &taken).ok_or("no room for another track of that name")?;
    course["name"] = serde_json::Value::String(name.clone());
    let stem = shared::stem(&name);
    std::fs::create_dir_all(&courses).map_err(|e| e.to_string())?;
    let file = courses.join(format!("{stem}.course"));
    std::fs::write(&file, serde_json::to_string_pretty(&course).unwrap_or_default()).map_err(|e| e.to_string())?;
    for h in homes {
        let dir = find_ci(h, &format!("TERAFORM/{folder}")).unwrap_or_else(|| h.join("TERAFORM").join(folder));
        let out = dir.join(format!("{stem}.ENV"));
        let mut cmd = Command::new(viewer);
        cmd.args(args).arg("--bake-course").arg(&file).arg(&out).stdout(Stdio::null()).stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        match cmd.output() {
            Ok(o) if o.status.success() => {}
            Ok(o) => return Err(String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("the game stopped").to_string()),
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(stem)
}

/// The card: browse, get and rate players' Course Builder tracks.
pub fn card(ui: &mut egui::Ui, t: &mut SharedTracks, server: &str, homes: &[PathBuf], viewer: Option<PathBuf>, args: Vec<String>) {
    use look::icon;
    look::card(ui, icon::ICON_EDIT_ROAD, "Shared Course Builder tracks", |ui| {
        ui.label(look::weak("Tracks players made in the Course Builder and shared from it (Save / Export, Share with other players). Get one and it's in the game's track list for its event, and in your Course Builder to change."));
        if server.is_empty() {
            look::status(ui, false, "These come from the leaderboard server, and its address is cleared (Extras, Lap leaderboards).");
            return;
        }
        let mut fetch = t.rows.is_none() && t.fetching.is_none() && t.error.is_none();
        let kinds: Vec<_> = std::iter::once(look::Choice::new(None, "All")).chain(KINDS.iter().enumerate().map(|(i, k)| look::Choice::new(Some(i), k.1))).collect();
        if let Some(k) = look::segmented(ui, t.kind, &kinds) {
            t.kind = k;
            t.page = 0;
            fetch = true;
        }
        let sorts: Vec<_> = Sort::ALL.iter().map(|s| look::Choice::new(*s, s.label())).collect();
        if let Some(s) = look::segmented(ui, t.sort, &sorts) {
            t.sort = s;
            t.page = 0;
            fetch = true;
        }
        ui.horizontal(|ui| {
            let edit = ui.add(egui::TextEdit::singleline(&mut t.search).hint_text("Search by name or maker").desired_width(ui.available_width() - 110.0));
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button(look::with(icon::ICON_SEARCH, "Search")).on_hover_text("Only the shared tracks whose name or maker has these words.").clicked() || enter {
                t.page = 0;
                fetch = true;
            }
        });
        if let Some(r) = t.fetching.as_ref().and_then(|s| s.lock().unwrap().take()) {
            t.fetching = None;
            match r {
                Ok(rows) => t.rows = Some(rows),
                Err(e) => {
                    eprintln!("shared tracks: {e}");
                    t.error = Some("Couldn't reach the server. Check the internet and try again.".into());
                }
            }
        }
        if let Some(r) = t.rating.as_ref().and_then(|s| s.lock().unwrap().take()) {
            t.rating = None;
            if let Ok((id, stars, avg, votes)) = r {
                t.mine().votes.insert(id.clone(), stars);
                let text = t.mine().text();
                let _ = std::fs::write(mine_path(), text);
                if let Some(row) = t.rows.iter_mut().flatten().find(|x| x.id == id) {
                    row.stars = avg;
                    row.votes = votes;
                }
            }
        }
        let finished: Vec<(String, Result<String, String>)> = t.getting.iter().filter_map(|(id, s)| s.lock().unwrap().take().map(|r| (id.clone(), r))).collect();
        for (id, r) in finished {
            t.getting.remove(&id);
            match r {
                Ok(stem) => {
                    t.mine().got.insert(id.clone(), stem.clone());
                    let text = t.mine().text();
                    let _ = std::fs::write(mine_path(), text);
                    t.done.insert(id, stem);
                }
                Err(e) => {
                    t.failed.insert(id, e);
                }
            }
        }
        if t.fetching.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Getting the list…");
            });
        }
        if let Some(e) = t.error.clone() {
            look::status(ui, false, &e);
            if ui.button("Try again").clicked() {
                fetch = true;
            }
        }
        let rows = t.rows.clone().unwrap_or_default();
        if rows.is_empty() && t.fetching.is_none() && t.error.is_none() {
            ui.label(look::weak("None shared yet. Make one in the Course Builder and be the first."));
        }
        let voter = t.mine().voter.clone();
        let mine_ids: Vec<String> = t.mine().shared.values().map(|(id, _)| id.clone()).collect();
        for row in &rows {
            egui::Frame::new().fill(look::CARD).stroke(egui::Stroke::new(1.0, look::LINE)).corner_radius(10).inner_margin(10).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.label(egui::RichText::new(&row.name).family(look::bold_family()).size(15.0));
                        let kind = KINDS.iter().find(|k| k.0 == row.mode).map_or(row.mode.as_str(), |k| k.1);
                        let by = if row.author.is_empty() { String::new() } else { format!(" · by {}", row.author) };
                        let copy = if row.base.is_empty() { String::new() } else { format!(" · a copy of {}", row.base) };
                        ui.label(look::weak(format!("{kind}{by}{copy} · {} downloads · {}", row.downloads, row.rating())));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let got = t.mine().got.contains_key(&row.id) || t.done.contains_key(&row.id);
                        if t.getting.contains_key(&row.id) {
                            ui.spinner();
                            ui.label("Getting it…");
                        } else {
                            let can = viewer.is_some() && !homes.is_empty();
                            let label = if got { "Get again" } else { "Get" };
                            let b = egui::Button::new(egui::RichText::new(look::with(icon::ICON_DOWNLOAD, label)).color(look::ON_ACCENT)).fill(look::ACCENT);
                            let hover = if row.base.is_empty() {
                                "Downloads the track and builds it into the game's track list for its event (a few seconds). It's in your Course Builder too.".to_string()
                            } else {
                                format!("A copy of {}: only the maker's changes come down, laid on your own copy of that track, which you need.", row.base)
                            };
                            if ui.add_enabled(can, b).on_hover_text(hover).clicked() {
                                let slot: Slot<String> = Arc::default();
                                let s = slot.clone();
                                let (server, row2, voter, again, homes, viewer, args) = (server.to_string(), row.clone(), voter.clone(), t.mine().got.get(&row.id).cloned(), homes.to_vec(), viewer.clone().unwrap(), args.clone());
                                let ctx = ui.ctx().clone();
                                std::thread::spawn(move || {
                                    *s.lock().unwrap() = Some(get_track(&server, &row2, &voter, again, &homes, &viewer, &args));
                                    ctx.request_repaint();
                                });
                                t.failed.remove(&row.id);
                                t.getting.insert(row.id.clone(), slot);
                            }
                            if got && !mine_ids.contains(&row.id) {
                                let given = t.mine().votes.get(&row.id).copied().unwrap_or(0);
                                if ui.small_button("Report").on_hover_text("Tell us this track has something it shouldn't: the game's own files, or something rude. It's taken off the list once a few players report it.").clicked() {
                                    let (url, body) = (format!("{server}/shared/report"), format!("{}\t{voter}\n", row.id));
                                    std::thread::spawn(move || drop(post_text(&url, body)));
                                    t.done.insert(row.id.clone(), "reported".into());
                                }
                                let mut pick = given;
                                egui::ComboBox::from_id_salt(format!("rate-{}", row.id))
                                    .selected_text(if given == 0 { "Rate it".to_string() } else { format!("{given} of 5") })
                                    .width(70.0)
                                    .show_ui(ui, |ui| {
                                        for s in (1..=5u8).rev() {
                                            ui.selectable_value(&mut pick, s, format!("{s} of 5"));
                                        }
                                    })
                                    .response
                                    .on_hover_text("How good is it? One vote each; you can change yours.");
                                if pick != given && t.rating.is_none() {
                                    let slot: Slot<(String, u8, u32, u32)> = Arc::default();
                                    let s = slot.clone();
                                    let (url, body, id) = (format!("{server}/shared/rate"), format!("{}\t{voter}\t{pick}\n", row.id), row.id.clone());
                                    std::thread::spawn(move || {
                                        let r = post_text(&url, body).and_then(|a| {
                                            let f: Vec<&str> = a.trim().split('\t').collect();
                                            match f.as_slice() {
                                                ["ok", s, v] => Ok((id, pick, s.parse().unwrap_or(0), v.parse().unwrap_or(0))),
                                                _ => Err(a),
                                            }
                                        });
                                        *s.lock().unwrap() = Some(r);
                                    });
                                    t.rating = Some(slot);
                                }
                            } else if mine_ids.contains(&row.id) {
                                ui.label(look::weak("Shared by you"));
                            }
                        }
                    });
                });
                if let Some(e) = t.failed.get(&row.id) {
                    look::status(ui, false, &format!("Couldn't get it: {e}"));
                } else if let Some(stem) = t.done.get(&row.id).filter(|s| *s != "reported") {
                    look::status(ui, true, &format!("Added as {stem}: it's in the track list for its event."));
                }
            });
            ui.add_space(4.0);
        }
        ui.horizontal(|ui| {
            if ui.add_enabled(t.page > 0 && t.fetching.is_none(), egui::Button::new("Previous")).clicked() {
                t.page -= 1;
                fetch = true;
            }
            if ui.add_enabled(rows.len() >= 60 && t.fetching.is_none(), egui::Button::new("Next")).clicked() {
                t.page += 1;
                fetch = true;
            }
            if ui.button(look::with(icon::ICON_REFRESH, "Refresh")).on_hover_text("Ask the server for the list again.").clicked() {
                fetch = true;
            }
        });
        if fetch && t.fetching.is_none() {
            t.fetch(server);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Gets every National and Supercross track a test server shares into
    /// a copy of a game folder: `MM2_SHARED_SERVER` (such as a local
    /// `wrangler dev`), `MM2_TEST_GAME` and `MM2_TEST_VIEWER`; run with
    /// `--ignored`.
    #[test]
    #[ignore]
    fn live_get() {
        let var = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
        let (server, game, viewer) = (var("MM2_SHARED_SERVER"), PathBuf::from(var("MM2_TEST_GAME")), PathBuf::from(var("MM2_TEST_VIEWER")));
        let rows = shared::parse_rows(&get_text(&shared::list_url(&server, "", Sort::New, "", 0), 1 << 20).unwrap());
        assert!(!rows.is_empty(), "share a track first");
        for row in rows {
            let stem = get_track(&server, &row, "launchertest1234", None, std::slice::from_ref(&game), &viewer, &[]).unwrap();
            let folder = KINDS.iter().find(|k| k.0 == row.mode).unwrap().2;
            let env = find_ci(&game, &format!("TERAFORM/{folder}/{stem}.ENV")).expect("baked");
            println!("{} ({}) -> {} {} bytes", row.name, row.base, env.display(), env.metadata().unwrap().len());
        }
    }
}
