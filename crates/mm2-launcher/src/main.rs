//! The OpenMCM2 launcher: sets up the game data from the player's own
//! copy of Motocross Madness 2, picks the settings, and starts
//! `mm2-viewer` with them. It can also update OpenMCM2 itself from its
//! GitHub releases (see [`update`]), and add tracks players have made
//! (see [`tracks`]).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod boards;
mod game;
mod iso;
mod look;
mod settings;
mod shortcut;
mod tracks;
mod update;

use eframe::egui;
use game::Source;
use settings::{fits, fitting, shape_of, Quality, Settings, ANTIALIASING, SHAPES, UPGRADES};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The window icon and the logo at the top (drawn from `assets/icon/*.svg`).
/// Where the Updates page's Donate button goes.
const SPONSOR_URL: &str = "https://github.com/sponsors/crspytopgn";

const ICON_PNG: &[u8] = include_bytes!("../../../assets/icon/icon-256.png");
const LOGO_PNG: &[u8] = include_bytes!("../../../assets/icon/logo-dark.png");

fn main() -> eframe::Result {
    // `--import PATH [DEST]`: sets up the game data without the window.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--import") {
        std::process::exit(import_only(&args[1..]));
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("OpenMCM2")
            .with_inner_size([1000.0, 760.0])
            .with_min_inner_size([760.0, 520.0])
            .with_icon(eframe::icon_data::from_png_bytes(ICON_PNG).expect("built-in icon")),
        ..Default::default()
    };
    eframe::run_native("OpenMCM2", options, Box::new(|cc| Ok(Box::new(Launcher::new(cc)))))
}

fn import_only(args: &[String]) -> i32 {
    let Some(path) = args.first() else {
        eprintln!("usage: mm2-launcher --import <CD image or folder> [destination]");
        return 2;
    };
    let dest = args.get(1).map_or_else(|| settings::user_dir().join("GAME"), PathBuf::from);
    let source = match Source::of(path.as_ref()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let p = game::import(source, dest);
    loop {
        if let Some(r) = p.result.lock().unwrap().take() {
            return match r {
                Ok(dir) => {
                    println!("game data ready in {}", dir.display());
                    let mut s = Settings::load();
                    s.game = Some(dir);
                    let _ = s.save();
                    0
                }
                Err(e) => {
                    eprintln!("{e}");
                    1
                }
            };
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

struct Launcher {
    settings: Settings,
    saved: Settings,
    import: Option<Arc<game::Progress>>,
    running: Option<Child>,
    /// The last thing to tell the player, and whether it's bad news.
    note: Option<(String, bool)>,
    /// Where this copy of OpenMCM2 is installed, if it can update itself.
    install: Result<update::Install, String>,
    update: Update,
    tracks: Tracks,
    boards: Boards,
    /// Loaded at the first frame.
    logo: Option<egui::TextureHandle>,
    page: Page,
}

/// What a background job gives back, once it has.
type Slot<T> = Arc<Mutex<Option<Result<T, String>>>>;

/// The lap leaderboards' sending and the board shown.
#[derive(Default)]
struct Boards {
    sending: Option<Slot<usize>>,
    fetching: Option<Slot<Vec<(String, u32)>>>,
    shown: Option<(mm2_settings::laps::Board, Option<Result<Vec<(String, u32)>, String>>)>,
    note: Option<String>,
    /// Everyone's playtime from the server, once fetched.
    everyone: Option<Result<mm2_settings::playtime::Times, String>>,
    fetching_everyone: Option<Slot<mm2_settings::playtime::Times>>,
    /// The server browser's open races, once asked for.
    races: Option<Result<Vec<mm2_settings::races::OpenRace>, String>>,
    fetching_races: Option<Slot<Vec<mm2_settings::races::OpenRace>>>,
}

impl Boards {
    /// Sends the laps the game kept, if the extra is on with a server.
    /// Sends the playtime too, unless that is turned off, and fetches
    /// everyone's.
    fn send(&mut self, s: &Settings) {
        let url = s.more.leaderboard_url.trim().to_string();
        if url.is_empty() || self.sending.is_some() {
            return;
        }
        let (laps, play) = (s.more.leaderboards, s.more.share_playtime);
        let slot: Slot<usize> = Arc::default();
        let out = slot.clone();
        let everyone: Slot<mm2_settings::playtime::Times> = Arc::default();
        let all = everyone.clone();
        std::thread::spawn(move || {
            let dir = settings::user_dir();
            if play {
                if let Err(e) = boards::send_playtime(&url, &dir) {
                    eprintln!("playtime: {e}");
                }
            }
            *all.lock().unwrap() = Some(boards::fetch_playtime(&url));
            *out.lock().unwrap() = Some(if laps { boards::send_pending(&url, &dir) } else { Ok(0) });
        });
        self.sending = Some(slot);
        self.fetching_everyone = Some(everyone);
    }

    fn fetch_races(&mut self, url: &str) {
        let slot: Slot<Vec<mm2_settings::races::OpenRace>> = Arc::default();
        let (out, url) = (slot.clone(), url.to_string());
        std::thread::spawn(move || *out.lock().unwrap() = Some(boards::fetch_races(&url)));
        self.fetching_races = Some(slot);
    }

    fn show(&mut self, url: &str, board: mm2_settings::laps::Board) {
        let slot: Slot<Vec<(String, u32)>> = Arc::default();
        let (out, url, b) = (slot.clone(), url.to_string(), board.clone());
        std::thread::spawn(move || *out.lock().unwrap() = Some(boards::fetch(&url, &b)));
        self.fetching = Some(slot);
        self.shown = Some((board, None));
    }

    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(s) = &self.sending {
            let got = s.lock().unwrap().take();
            match got {
                Some(Ok(0)) => self.sending = None,
                Some(Ok(n)) => {
                    self.note = Some(format!("{n} new best lap{} sent to the leaderboard server.", if n == 1 { "" } else { "s" }));
                    self.sending = None;
                }
                Some(Err(e)) => {
                    self.note = Some(format!("Laps not sent yet ({e}). They'll go next time."));
                    self.sending = None;
                }
                None => ctx.request_repaint_after(Duration::from_millis(250)),
            }
        }
        if let Some(f) = &self.fetching_races {
            let got = f.lock().unwrap().take();
            match got {
                Some(r) => {
                    self.races = Some(r);
                    self.fetching_races = None;
                }
                None => ctx.request_repaint_after(Duration::from_millis(250)),
            }
        }
        let everyone = self.fetching_everyone.as_ref().and_then(|f| f.lock().unwrap().take());
        if let Some(r) = everyone {
            self.everyone = Some(r);
            self.fetching_everyone = None;
        }
        if let Some(f) = &self.fetching {
            let got = f.lock().unwrap().take();
            match got {
                Some(r) => {
                    if let Some((_, rows)) = &mut self.shown {
                        *rows = Some(r);
                    }
                    self.fetching = None;
                }
                None => ctx.request_repaint_after(Duration::from_millis(250)),
            }
        }
    }
}

/// A listing page being fetched, until it arrives.
type Fetch = Arc<Mutex<Option<Result<tracks::Listing, String>>>>;

/// The Tracks page's browser.
#[derive(Default)]
struct Tracks {
    site: usize,
    /// The category shown, or `None` while showing a search.
    kind: Option<usize>,
    search: String,
    page: u32,
    listing: Option<tracks::Listing>,
    fetching: Option<Fetch>,
    error: Option<String>,
    added: tracks::Added,
    /// Downloads by address, and how the finished ones went.
    jobs: std::collections::HashMap<String, Arc<tracks::Job>>,
    failed: std::collections::HashMap<String, String>,
}

impl Tracks {
    fn fetch(&mut self, page: u32) {
        let site = &tracks::SITES[self.site];
        let query = match self.kind {
            Some(k) => site.kinds[k].category.to_string(),
            None => self.search.trim().to_string(),
        };
        let slot = Arc::new(Mutex::new(None));
        let s = slot.clone();
        std::thread::spawn(move || *s.lock().unwrap() = Some(tracks::list(site, &query, page)));
        self.page = page;
        self.error = None;
        self.fetching = Some(slot);
    }

    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(f) = &self.fetching {
            let got = f.lock().unwrap().take();
            match got {
                Some(Ok(l)) => {
                    self.listing = Some(l);
                    self.fetching = None;
                }
                Some(Err(e)) => {
                    self.error = Some(e);
                    self.fetching = None;
                }
                None => ctx.request_repaint_after(Duration::from_millis(100)),
            }
        }
        let mut done = Vec::new();
        for (url, job) in &self.jobs {
            if let Some(r) = job.result.lock().unwrap().take() {
                done.push((url.clone(), r));
            }
        }
        for (url, r) in done {
            self.jobs.remove(&url);
            match r {
                Ok(_) => {
                    self.failed.remove(&url);
                }
                Err(e) => {
                    self.failed.insert(url, e);
                }
            }
            self.added = tracks::Added::load();
        }
        if !self.jobs.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}

/// What a check for updates found, once it's done.
type Checked = Arc<Mutex<Option<Result<Option<update::Release>, String>>>>;

/// Where the self-update has got to.
enum Update {
    Idle,
    Checking(Checked),
    UpToDate,
    Found(update::Release),
    /// A newer release the player said "not now" to.
    Dismissed(update::Release),
    Installing(update::Release, Arc<update::Progress>),
    Failed(String),
}

impl Launcher {
    fn new(cc: &eframe::CreationContext) -> Launcher {
        look::apply(&cc.egui_ctx);
        let install = match update::current() {
            Some(_) => update::Install::here(),
            None => Err("this copy was built from source, so it doesn't update itself".into()),
        };
        if let Ok(i) = &install {
            i.tidy();
        }
        let mut l = Launcher::with(Settings::load(), install);
        l.boards.send(&l.settings);
        if l.settings.check_updates && l.install.is_ok() {
            l.check_updates();
        }
        // A game copied before but since moved or deleted.
        if let Some(g) = l.settings.game.clone() {
            if !game::missing(&g).is_empty() {
                l.note = Some((format!("The game data at {} is gone or incomplete. Set it up again.", g.display()), true));
                l.settings.game = None;
            }
        }
        l
    }

    /// The launcher with `settings`, before anything is checked.
    fn with(settings: Settings, install: Result<update::Install, String>) -> Launcher {
        Launcher {
            saved: settings.clone(),
            settings,
            import: None,
            running: None,
            note: None,
            install,
            update: Update::Idle,
            tracks: Tracks { added: tracks::Added::load(), ..Default::default() },
            boards: Boards::default(),
            logo: None,
            page: Page::Play,
        }
    }

    fn pick(&mut self, path: PathBuf) {
        match Source::of(&path) {
            Ok(source) => {
                self.note = None;
                self.import = Some(game::import(source, settings::user_dir().join("GAME")));
            }
            Err(e) => self.note = Some((e, true)),
        }
    }

    fn play(&mut self, ctx: &egui::Context) {
        let Some(viewer) = settings::viewer() else {
            self.note = Some(("Can't find mm2-viewer next to the launcher.".into(), true));
            return;
        };
        let log = settings::user_dir().join("last-run.log");
        let err = std::fs::File::create(&log).map(Stdio::from).unwrap_or(Stdio::null());
        let mut cmd = Command::new(&viewer);
        cmd.args(self.settings.args()).stdout(Stdio::null()).stderr(err);
        if let Some(g) = &self.settings.game {
            cmd.current_dir(g);
        }
        // The viewer is a console program; without this Windows opens a
        // console window for it behind the game.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        match cmd.spawn() {
            Ok(child) => {
                self.running = Some(child);
                self.note = None;
                // Out of the way while the game runs, so it can't show
                // through behind a full-screen game; back when it ends.
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            Err(e) => self.note = Some((format!("Couldn't start the game: {e}"), true)),
        }
    }

    /// A desktop shortcut that starts the game straight away, without the
    /// launcher, with the settings last saved here.
    fn make_shortcut(&mut self) {
        let Some(viewer) = settings::viewer() else {
            self.note = Some(("Can't find mm2-viewer next to the launcher.".into(), true));
            return;
        };
        self.note = Some(match shortcut::make(&viewer) {
            Ok(at) => (
                format!("Shortcut made: {}. It always starts with the settings here.", at.display()),
                false,
            ),
            Err(e) => (e, true),
        });
    }

    fn check_updates(&mut self) {
        let slot = Arc::new(Mutex::new(None));
        let s = slot.clone();
        std::thread::spawn(move || *s.lock().unwrap() = Some(update::check()));
        self.update = Update::Checking(slot);
    }

    fn install_update(&mut self, release: update::Release) {
        if let Ok(install) = self.install.clone() {
            self.update = Update::Installing(release.clone(), update::start(release, install));
        }
    }

    fn poll_update(&mut self, ctx: &egui::Context) {
        let checked = match &self.update {
            Update::Checking(slot) => slot.lock().unwrap().take(),
            _ => None,
        };
        let installed = match &self.update {
            Update::Installing(_, p) => p.result.lock().unwrap().take(),
            _ => None,
        };
        let waiting = matches!(self.update, Update::Checking(_) | Update::Installing(..));
        if let Some(r) = checked {
            self.update = match r {
                Ok(Some(r)) => Update::Found(r),
                Ok(None) => Update::UpToDate,
                Err(e) => Update::Failed(e),
            };
        } else if let Some(r) = installed {
            match r {
                Ok(_) => {
                    let _ = self.settings.save();
                    let restarted = self.install.as_ref().map_err(|e| e.clone()).and_then(|i| update::restart(i).map_err(|e| e.to_string()));
                    match restarted {
                        Ok(()) => std::process::exit(0),
                        Err(e) => {
                            self.update = Update::Failed(format!("updated, but couldn't restart ({e}); open OpenMCM2 again"))
                        }
                    }
                }
                Err(e) => self.update = Update::Failed(format!("{e}. Nothing was changed.")),
            }
        } else if waiting {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn poll(&mut self, ctx: &egui::Context) {
        self.poll_update(ctx);
        self.tracks.poll(ctx);
        self.boards.poll(ctx);
        if let Some(p) = self.import.clone() {
            let r = p.result.lock().unwrap().take();
            if let Some(r) = r {
                match r {
                    Ok(dir) => {
                        // A high-res pack beside the game, as the repo keeps one.
                        if self.settings.hires.is_none() {
                            let near = dir.parent().map(|d| d.join("textures-hires"));
                            self.settings.hires = near.filter(|d| d.is_dir());
                        }
                        self.note = Some((format!("Game data ready in {}", dir.display()), false));
                        self.settings.game = Some(dir);
                    }
                    Err(e) => self.note = Some((e, true)),
                }
                self.import = None;
            } else {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        }
        if let Some(child) = &mut self.running {
            match child.try_wait() {
                Ok(Some(status)) => {
                    if !status.success() {
                        let log = settings::user_dir().join("last-run.log");
                        let tail = std::fs::read_to_string(&log).unwrap_or_default();
                        let last = tail.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").to_string();
                        self.note = Some((format!("The game stopped with an error: {last}\n(full log: {})", log.display()), true));
                    }
                    self.running = None;
                    // Laps from the race just run.
                    self.boards.send(&self.settings);
                    // The game's Tweaks box, or its event page's time and
                    // weather, may have changed them.
                    let game = Settings::load();
                    if game != self.saved {
                        self.settings = game.clone();
                        self.saved = game;
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                Ok(None) => ctx.request_repaint_after(Duration::from_millis(500)),
                Err(_) => self.running = None,
            }
        }
    }
}

/// The launcher's pages, picked in the sidebar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Page {
    Play,
    Graphics,
    Extras,
    Tracks,
    Updates,
}

impl eframe::App for Launcher {
    // Runs while minimized too, so the launcher sees the game end.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
        if self.settings != self.saved && self.import.is_none() {
            let _ = self.settings.save();
            self.saved = self.settings.clone();
        }
    }
}

impl Launcher {
    /// The whole window: the sidebar with the pages and the Play button,
    /// and the page picked.
    fn draw(&mut self, ui: &mut egui::Ui) {
        if self.logo.is_none() {
            let png = eframe::icon_data::from_png_bytes(LOGO_PNG).expect("built-in logo");
            let image = egui::ColorImage::from_rgba_unmultiplied([png.width as usize, png.height as usize], &png.rgba);
            self.logo = Some(ui.ctx().load_texture("logo", image, egui::TextureOptions::LINEAR));
        }
        let busy = self.import.is_some();
        egui::Panel::left("nav")
            .exact_size(236.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(look::SIDEBAR).inner_margin(16).stroke(egui::Stroke::new(1.0, look::LINE)))
            .show(ui, |ui| self.sidebar(ui, busy));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(look::BG).inner_margin(egui::Margin { left: 28, right: 20, top: 18, bottom: 8 }))
            .show(ui, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ui.set_max_width(ui.available_width().min(820.0));
                    match self.page {
                        Page::Play => self.play_page(ui, busy),
                        Page::Graphics => self.graphics_page(ui),
                        Page::Extras => self.extras_page(ui),
                        Page::Tracks => self.tracks_page(ui, busy),
                        Page::Updates => self.updates_page(ui),
                    }
                    ui.add_space(12.0);
                });
            });
    }

    fn sidebar(&mut self, ui: &mut egui::Ui, busy: bool) {
        if let Some(logo) = &self.logo {
            let w = ui.available_width();
            let size = logo.size_vec2() * (w / logo.size_vec2().x).min(0.5);
            ui.add(egui::Image::new(logo).fit_to_exact_size(size)).on_hover_text("OpenMCM2");
        }
        ui.add_space(18.0);
        let update = matches!(self.update, Update::Found(_));
        for (page, i, name) in [
            (Page::Play, look::icon::ICON_SPORTS_MOTORSPORTS, "Play"),
            (Page::Graphics, look::icon::ICON_TUNE, "Graphics"),
            (Page::Extras, look::icon::ICON_EXTENSION, "Extras"),
            (Page::Tracks, look::icon::ICON_ROUTE, "Tracks"),
            (Page::Updates, look::icon::ICON_UPDATE, if update { "Updates  •" } else { "Updates" }),
        ] {
            if look::nav(ui, self.page == page, i, name) {
                self.page = page;
            }
        }
        // From the bottom up: the version, the shortcut, Play, and the
        // last thing to tell the player.
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.add_space(2.0);
            let version = update::current().map_or("Built from source".to_string(), |v| format!("OpenMCM2 {v}"));
            ui.label(look::weak(version));
            ui.add_space(6.0);
            let can = self.settings.game.is_some() && !busy;
            if ui
                .add_enabled(can, egui::Button::new(look::with(look::icon::ICON_SHORTCUT, "Desktop shortcut")).min_size(egui::vec2(ui.available_width(), 32.0)))
                .on_hover_text("Starts the game straight away, skipping the launcher, with the settings here. Change them here and the shortcut follows.")
                .clicked()
            {
                self.make_shortcut();
            }
            ui.add_space(4.0);
            let ready = self.settings.game.is_some() && !busy && self.running.is_none();
            let label = if self.running.is_some() { "Running…".to_string() } else { look::with(look::icon::ICON_PLAY_CIRCLE, "PLAY") };
            if look::big_button(ui, &label, 54.0, ready).clicked() {
                let _ = self.settings.save();
                self.saved = self.settings.clone();
                let ctx = ui.ctx().clone();
                self.play(&ctx);
            }
            if self.settings.game.is_none() && !busy {
                ui.label(look::weak("Set up the game data first."));
            }
            if let Some((text, bad)) = &self.note {
                ui.add_space(4.0);
                ui.add(egui::Label::new(egui::RichText::new(text).color(if *bad { look::BAD } else { look::GOOD }).size(13.0)).wrap());
            }
        });
    }

    // ---- Play ----

    fn play_page(&mut self, ui: &mut egui::Ui, busy: bool) {
        look::page_title(ui, "Ready to ride", "Motocross Madness 2, rebuilt to run natively.");
        self.update_banner(ui, busy);
        self.game_card(ui, busy);
        self.summary_card(ui);
        self.playtime_card(ui);
        self.display_card(ui);
    }

    /// The playtime tracker: the player's own, and everyone's.
    fn playtime_card(&mut self, ui: &mut egui::Ui) {
        use look::icon;
        use mm2_settings::playtime::{self, shown};
        let mine = playtime::read(&settings::user_dir(), playtime::FILE);
        look::card(ui, icon::ICON_SCHEDULE, "Playtime", |ui| {
            let get = |k: &str| mine.get(k).copied().unwrap_or(0);
            if get("total") == 0 {
                ui.label(look::weak("Nothing yet. The time you play goes here."));
            } else {
                ui.label(format!(
                    "{} played, {} of it racing in {} race{}.",
                    shown(get("total")),
                    shown(get("races")),
                    get("count"),
                    if get("count") == 1 { "" } else { "s" }
                ));
                // The most played of each kind.
                let top = |prefix: &str, name: &dyn Fn(&str) -> String| -> String {
                    let mut list: Vec<(&String, &u64)> = mine.iter().filter(|(k, _)| k.starts_with(prefix)).collect();
                    list.sort_by(|a, b| b.1.cmp(a.1));
                    list.iter().take(3).map(|(k, v)| format!("{} {}", name(&k[prefix.len()..]), shown(**v))).collect::<Vec<_>>().join(", ")
                };
                let event = |k: &str| boards::event_name(k.parse().unwrap_or(99)).to_string();
                let track = |k: &str| k.to_string();
                let bike = |k: &str| format!("{k}cc");
                let kinds: [(&str, &str, &dyn Fn(&str) -> String); 3] = [("Events", "event:", &event), ("Tracks", "track:", &track), ("Bikes", "bike:", &bike)];
                egui::Grid::new("playtime").num_columns(2).show(ui, |ui| {
                    for (label, prefix, name) in kinds {
                        let text = top(prefix, name);
                        if !text.is_empty() {
                            ui.label(egui::RichText::new(label).family(look::bold_family()));
                            ui.label(text);
                            ui.end_row();
                        }
                    }
                });
            }
            if let Some(Ok(all)) = &self.boards.everyone {
                let get = |k: &str| all.get(k).copied().unwrap_or(0);
                if get("total") > 0 {
                    ui.label(look::weak(format!(
                        "Everyone: {} played by {} player{}, in {} races.",
                        shown(get("total")),
                        get("players"),
                        if get("players") == 1 { "" } else { "s" },
                        get("count")
                    )));
                }
            }
            let m = &mut self.settings.more;
            look::switch_tile(
                ui,
                &mut m.share_playtime,
                "Add my time to everyone's",
                "Sends how long you play each event, track and bike class to the OpenMCM2 server, added to everyone's. No name or anything else that says who you are.",
                ui.available_width(),
            );
        });
    }

    fn game_card(&mut self, ui: &mut egui::Ui, busy: bool) {
        look::card(ui, look::icon::ICON_DISC_FULL, "Game data", |ui| {
            match &self.settings.game {
                Some(g) => look::status(ui, true, &format!("Ready: {}", g.display())),
                None => look::status(ui, false, "Not set up yet"),
            }
            ui.label(look::weak(
                "OpenMCM2 doesn't include the game. Point it at your own Motocross Madness 2: the CD, a CD image of it (.iso, .bin/.cue, .mdf/.mds, .img), a .zip of either, or the folder it's installed in.",
            ));
            if let Some(p) = &self.import {
                let total = p.total.load(Ordering::Relaxed).max(1);
                let done = p.done.load(Ordering::Relaxed);
                ui.horizontal(|ui| {
                    ui.add(egui::ProgressBar::new(done as f32 / total as f32).show_percentage().desired_width(ui.available_width() - 90.0));
                    if ui.button("Cancel").clicked() {
                        p.cancel.store(true, Ordering::Relaxed);
                    }
                });
                ui.label(look::weak(format!("Copying game data… {} of {} MB", done >> 20, total >> 20)));
            }
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                ui.add_enabled_ui(!busy, |ui| {
                    if ui.button(look::with(look::icon::ICON_DISC_FULL, "Choose CD image or zip…")).clicked() {
                        if let Some(f) = rfd::FileDialog::new()
                            .set_title("Your Motocross Madness 2 CD image or zip")
                            .add_filter("CD image or zip", &["iso", "bin", "cue", "img", "mdf", "mds", "nrg", "zip"])
                            .add_filter("All files", &["*"])
                            .pick_file()
                        {
                            self.pick(f);
                        }
                    }
                    if ui.button(look::with(look::icon::ICON_DRIVE_FOLDER_UPLOAD, "Choose CD or install folder…")).clicked() {
                        if let Some(d) = rfd::FileDialog::new().set_title("Your Motocross Madness 2 CD or install folder").pick_folder() {
                            self.pick(d);
                        }
                    }
                });
            });
        });
    }

    /// The graphics as set, at a glance, and a way to change them.
    fn summary_card(&mut self, ui: &mut egui::Ui) {
        let s = &self.settings;
        let mut go = false;
        look::card(ui, look::icon::ICON_PALETTE, "Look", |ui| {
            let mut parts: Vec<String> = Vec::new();
            if s.enhanced && s.auto {
                parts.push(match auto_last() {
                    Some((q, _)) => format!("Auto preset (last: {q})"),
                    None => "Auto preset".into(),
                });
            } else if s.enhanced {
                let preset = preset_of(s);
                parts.push(preset.map_or("Custom preset".to_string(), |q| format!("{} preset", q.label())));
                let traced = UPGRADES.iter().zip(&s.upgrades).any(|(u, on)| u.0 == "raytracing" && *on);
                if traced && preset != Some(Quality::RayTraced) {
                    parts.push("Ray tracing".into());
                }
                if let Some(a) = ANTIALIASING.iter().find(|a| a.0 == s.antialiasing) {
                    parts.push(if a.0 == "taa" { "TAA".into() } else { a.1.to_string() });
                }
                parts.push(if s.resolution == "auto" { "Auto resolution".into() } else { format!("{} resolution", s.resolution) });
            } else {
                parts.push("The original look, as the game drew it".into());
            }
            ui.horizontal_wrapped(|ui| {
                for p in &parts {
                    look::chip(ui, p);
                }
            });
            ui.add_space(4.0);
            go = ui.button(look::with(look::icon::ICON_TUNE, "Change graphics")).clicked();
        });
        if go {
            self.page = Page::Graphics;
        }
    }

    /// How the game starts: full screen (at the screen's own resolution or
    /// one picked) or a window of a picked shape and size. The game's
    /// Options, Graphics, Display Resolution changes the same settings.
    fn display_card(&mut self, ui: &mut egui::Ui) {
        use look::Choice;
        // The screen the launcher is on, in points (as windows are sized)
        // and in pixels (as the screen's resolutions are).
        let (points, pixels) = ui.ctx().input(|i| {
            let v = i.viewport();
            let round = |m: egui::Vec2| (m.x.round() as u32, m.y.round() as u32);
            (v.monitor_size.map(round), v.monitor_size.map(|m| round(m * v.native_pixels_per_point.unwrap_or(1.0))))
        });
        let s = &mut self.settings;
        look::card(ui, look::icon::ICON_MONITOR, "Display", |ui| {
            let modes = [
                Choice::new(true, "Full screen").icon(look::icon::ICON_FULLSCREEN).hover(
                    "The game fills the whole screen. Recommended: the smoothest picture.",
                ),
                Choice::new(false, "In a window").icon(look::icon::ICON_WEB_ASSET).hover(
                    "The game in a window of the size picked below, with the rest of the screen showing round it.",
                ),
            ];
            if let Some(full) = look::segmented(ui, s.fullscreen, &modes) {
                s.fullscreen = full;
            }
            ui.add_space(6.0);
            if s.fullscreen {
                let named = look::label(
                    ui,
                    "Resolution",
                    "How many pixels the full screen has. Your screen's own is the sharpest and switches fastest; a lower one can run faster on an older computer.",
                );
                // The sizes the screen is big enough for, by shape.
                let own = match pixels {
                    Some(m) => format!("Your screen's own ({})", size_text(m)),
                    None => "Your screen's own".into(),
                };
                let shown = if s.screen == (0, 0) { own.clone() } else { size_text(s.screen) };
                egui::ComboBox::from_id_salt("screen")
                    .selected_text(shown)
                    .width(300.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut s.screen, (0, 0), own)
                            .on_hover_text("The screen stays as it is, and the game draws at its full sharpness.");
                        for (name, about, sizes) in SHAPES.iter() {
                            let sizes: Vec<_> = sizes.iter().filter(|z| pixels.is_none_or(|m| z.0 <= m.0 && z.1 <= m.1) && Some((z.0, z.1)) != pixels).collect();
                            if sizes.is_empty() {
                                continue;
                            }
                            ui.label(look::weak(*name));
                            for z in sizes {
                                ui.selectable_value(&mut s.screen, (z.0, z.1), size_text((z.0, z.1)))
                                    .on_hover_text(format!("{about} The screen switches to {} × {} while you play, if it can.", z.0, z.1));
                            }
                        }
                    })
                    .response
                    .labelled_by(named)
                    .on_hover_text("How many pixels the full screen has while you play.");
                let note = match (s.screen, pixels) {
                    ((0, 0), Some(m)) if m.0 * 5 > m.1 * 12 => {
                        "Super ultrawide: the race is drawn 21:9 in the middle, with black bars at the sides.".to_string()
                    }
                    ((0, 0), Some(m)) => format!("{}: {}", SHAPES[shape_of(m)].0, SHAPES[shape_of(m)].1),
                    ((0, 0), None) => "The screen as it is.".into(),
                    (z, _) => format!(
                        "The screen switches to {} × {} while you play, and back after. If it can't show that, the game uses the screen's own.",
                        z.0, z.1
                    ),
                };
                ui.add(egui::Label::new(look::weak(note)).wrap());
            } else {
                // A window as big as the screen is full screen done worse:
                // a size that doesn't fit gives way to the biggest that does.
                if !fits(s.window, points) {
                    s.window = fitting(SHAPES[shape_of(s.window)].2, points)
                        .or_else(|| SHAPES.iter().rev().find_map(|z| fitting(z.2, points)))
                        .unwrap_or((1024, 768));
                }
                let shape = shape_of(s.window);
                look::label(ui, "Shape", "The window's shape. The race shows more to the sides on a wider one.");
                let shapes: Vec<_> = SHAPES
                    .iter()
                    .enumerate()
                    .map(|(k, (name, about, sizes))| {
                        let hover = match fitting(sizes, points) {
                            Some(_) => about.to_string(),
                            None => format!("{about} Too big for a window on your screen: use Full screen."),
                        };
                        Choice::new(k, *name).hover(hover)
                    })
                    .collect();
                if let Some(k) = look::segmented(ui, shape, &shapes) {
                    if let (true, Some(size)) = (k != shape, fitting(SHAPES[k].2, points)) {
                        s.window = size;
                    }
                }
                ui.add_space(6.0);
                let named = look::label(ui, "Size", "The window's size inside, without its title bar.");
                egui::ComboBox::from_id_salt("size")
                    .selected_text(size_text(s.window))
                    .width(300.0)
                    .show_ui(ui, |ui| {
                        for &(w, h, _) in SHAPES[shape].2.iter().filter(|z| fits((z.0, z.1), points)) {
                            ui.selectable_value(&mut s.window, (w, h), size_text((w, h)))
                                .on_hover_text(format!("A window {w} wide and {h} high."));
                        }
                    })
                    .response
                    .labelled_by(named)
                    .on_hover_text("Only the sizes that fit on your screen with room for the title bar.");
                if points != pixels {
                    ui.add(egui::Label::new(look::weak(
                        "Sizes are in your screen's points, as your system counts them; the game still draws at full sharpness.",
                    )).wrap());
                }
            }
            ui.add_space(4.0);
            ui.add(egui::Label::new(look::weak(
                "Alt+Enter switches between full screen and a window while playing. The game's Options, Graphics, Display Resolution changes these too.",
            )).wrap());
        });
    }

    // ---- Extras ----

    /// The extras the original game didn't have, all off unless turned
    /// on. Most of them come from Daniel's fork (github.com/dwolp/MCM2).
    fn extras_page(&mut self, ui: &mut egui::Ui) {
        use look::icon;
        look::page_title(
            ui,
            "Extras",
            "Things the original game didn't have. They all start off, so the game plays as it did. Most of them are also in the game: Options, then Tweaks... at the bottom.",
        );
        let s = &mut self.settings;
        let original = s.is_original();
        if ui
            .add_enabled(!original, egui::Button::new(look::with(icon::ICON_HISTORY, "Back to the original")))
            .on_hover_text("Turns every extra here off, and the difficulty table back to the game's.")
            .clicked()
        {
            s.original();
        }
        ui.add_space(6.0);
        let tile = |ui: &mut egui::Ui, on: &mut bool, name: &str, about: &str| {
            let w = ui.available_width();
            look::switch_tile(ui, on, name, about, w);
        };
        let slider = |ui: &mut egui::Ui, name: &str, about: &str, value: &mut u32, range: std::ops::RangeInclusive<u32>, step: f64, suffix: &str| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(name).family(look::bold_family()));
                ui.add(egui::Slider::new(value, range).step_by(step).suffix(suffix)).on_hover_text(about);
            });
            ui.label(look::weak(about));
        };
        look::card(ui, icon::ICON_SPORTS_MOTORSPORTS, "Riding", |ui| {
            tile(ui, &mut s.quick_get_up, "Quick get-up", "After a crash, G or a pad's Back puts you straight back on the bike. Not in network races.");
            tile(ui, &mut s.forgiving_landings, "Forgiving landings", "Fewer crashes from landing a little crooked. The original game is strict.");
            slider(ui, "Air steering", "How much steering in the air bends your flight. 0% is the original game.", &mut s.air_steer, 0..=mm2_settings::AIR_MOST, 10.0, "%");
            slider(ui, "Air braking", "How much braking in the air shortens your flight. 0% is the original game.", &mut s.air_brake, 0..=mm2_settings::AIR_MOST, 10.0, "%");
            tile(ui, &mut s.clean_starts, "Cleaner starts", "The computer riders keep their order across the gate off the start, so fewer pile into each other.");
            tile(ui, &mut s.one_lap, "One-lap races", "National and Supercross races can be 1 lap long (the original starts at 2).");
            ui.label(look::weak("The difficulty table is in the game: Options, Tweaks..., Difficulty..."));
        });
        look::card(ui, icon::ICON_SPEED, "Race display and camera", |ui| {
            tile(ui, &mut s.sharp_display, "Modernised HUD", "Crisp see-through race panel, radar and speedometer at your screen's resolution.");
            tile(ui, &mut s.kmh, "Speedometer in km/h", "The speed in kilometres an hour instead of miles an hour.");
            tile(ui, &mut s.lap_gap, "Live gap to your best lap", "Under the race panel: how far ahead or behind your best lap you are.");
            tile(ui, &mut s.centred_display, "Race display nearer the middle", "On screens wider than 16:9, the race display where it sits on a 16:9 screen.");
            tile(ui, &mut s.steady_camera, "Steadier camera", "The chase camera rises and dips with the bike more gently.");
            slider(ui, "Field of view", "How wide the riding cameras see. 85° is the original game's.", &mut s.fov, 60..=120, 1.0, "°");
            tile(ui, &mut s.more.helmet_cam, "Helmet Cam", "A camera from the rider's helmet. In a race press F1 to switch it on or off (fn+F1 on most Mac keyboards), or reach it after User Cam with the camera key (C or Numpad 5).");
        });
        look::card(ui, icon::ICON_SPORTS_ESPORTS, "Controllers, sound and screen", |ui| {
            tile(ui, &mut s.vibration, "Controller vibration", "The pad thumps on landings, knocks over bumps and jolts in crashes.");
            ui.add_enabled_ui(s.vibration, |ui| {
                slider(ui, "Vibration strength", "How strongly the pad shakes.", &mut s.vibration_strength, 0..=100, 1.0, "%");
            });
            slider(ui, "Stick dead zone", "How far a stick moves before it steers. 4% is the original game's.", &mut s.more.pad_dead_zone, 0..=50, 1.0, "%");
            slider(ui, "Stick sensitivity", "How far a stick steers for how far it is pushed. 100% is the original game's.", &mut s.more.pad_sensitivity, 25..=200, 5.0, "%");
            slider(ui, "Stick smoothing", "Evens out a twitchy stick's steering. 0% is the original game's.", &mut s.more.pad_smoothing, 0..=90, 5.0, "%");
            tile(ui, &mut s.mute_in_background, "Mute in the background", "The game goes quiet while its window isn't the one in front.");
            tile(ui, &mut s.screenshot_key, "Screenshot key (Ctrl+P)", "Ctrl+P in a race saves a picture to the Screenshots folder beside your saves.");
            tile(ui, &mut s.vsync, "VSync", "A picture each time the screen refreshes, with no tearing.");
            tile(ui, &mut s.show_fps, "Show frame rate", "The frames a second at the bottom left (F2 too).");
            slider(ui, "Frame cap", "The most pictures a second, to keep a laptop cool. 0 is no cap.", &mut s.more.fps_cap, 0..=240, 10.0, " fps");
            tile(ui, &mut s.more.fxaa, "Smoother edges", "Softens jagged edges over the race picture, in either look.");
            slider(ui, "Sharpening", "Brings out detail in the race picture, in either look. 0% is the original game's.", &mut s.more.sharpen, 0..=100, 5.0, "%");
        });
        look::card(ui, icon::ICON_TWO_WHEELER, "Tracks, bikes and riders", |ui| {
            tile(ui, &mut s.more.all_gear, "All gear in every event", "The bikes and outfits the Pro Circuit gives out, in single races too.");
            tile(ui, &mut s.more.track_filter, "Track filter", "All tracks, the base game's or only the ones you added, on the event page.");
            slider(ui, "Riders on the gate", "Up to 30 riders in a single race. 0 is the original game's count.", &mut s.more.riders, 0..=mm2_settings::RIDERS_MOST, 1.0, "");
            ui.label(look::weak("Extra art for added tracks goes in the game's RES/ADDONS folder; it never replaces the game's own."));
        });
        self.leaderboard_card(ui);
        self.network_card(ui);
        ui.label(look::weak("Thanks to Daniel (github.com/dwolp) for many of these, the new launcher, the ray tracing and night races, and the fixes that came with them."));
    }

    /// Network races: lobby downloads, the server browser and its list.
    fn network_card(&mut self, ui: &mut egui::Ui) {
        use look::icon;
        look::card(ui, icon::ICON_LAN, "Network races", |ui| {
            let m = &mut self.settings.more;
            let w = ui.available_width();
            look::switch_tile(
                ui,
                &mut m.lobby_downloads,
                "Lobby downloads",
                "On unless you turn it off. In a network race, the host's track and other riders' own paints and outfits come over if you don't have them, and yours go to them. Everyone needs 0.5.0 or newer.",
                w,
            );
            look::switch_tile(
                ui,
                &mut m.list_races,
                "List my races in the server browser",
                "On unless you turn it off. A race you host shows in everyone's Host or Join list (Multiplayer) with its name, track and riders, so players anywhere can join. The list shows your computer's internet address. It only shows if TCP port 28220 reaches your computer (forwarded on your router); the lobby's chat says if it doesn't. Turn this off to keep your races to people you give the address to.",
                w,
            );
            let url = m.leaderboard_url.trim().to_string();
            if url.is_empty() {
                ui.label(look::weak("The server browser uses the leaderboard server, and its address is cleared (Lap leaderboards)."));
                return;
            }
            ui.horizontal(|ui| {
                let asking = self.boards.fetching_races.is_some();
                if ui
                    .add_enabled(!asking, egui::Button::new(if asking { "Looking…" } else { "Show open races" }))
                    .on_hover_text("The races open in the server browser now. Join one from Multiplayer in the game.")
                    .clicked()
                {
                    self.boards.fetch_races(&url);
                }
            });
            match &self.boards.races {
                None => {}
                Some(Err(e)) => {
                    ui.label(look::weak(e));
                }
                Some(Ok(races)) if races.is_empty() => {
                    ui.label(look::weak("No open races just now. Host one from Multiplayer in the game."));
                }
                Some(Ok(races)) => {
                    egui::Grid::new("open races").striped(true).num_columns(4).show(ui, |ui| {
                        for r in races.iter().take(50) {
                            ui.label(&r.name);
                            ui.label(if r.track.is_empty() { mm2_settings::races::event_name(r.event).to_string() } else { r.track.clone() });
                            ui.label(format!("{}/8", r.players));
                            ui.label(if r.racing { "Racing" } else { "In the lobby" });
                            ui.end_row();
                        }
                    });
                    ui.label(look::weak("Join one from Multiplayer in the game: it's in the Host or Join list."));
                }
            }
        });
    }

    fn leaderboard_card(&mut self, ui: &mut egui::Ui) {
        use look::icon;
        let dir = settings::user_dir();
        look::card(ui, icon::ICON_EMOJI_EVENTS, "Lap leaderboards", |ui| {
            let m = &mut self.settings.more;
            look::switch_tile(
                ui,
                &mut m.leaderboards,
                "Lap leaderboards",
                "On unless you turn it off. Sends your best lap in each race from lap 2 on, with the riding extras off, under your rider name. In Nationals and Supercross, a lap with more than 2 seconds off the course doesn't count. In the game: High Scores, Online.",
                ui.available_width(),
            );
            if !m.leaderboards {
                return;
            }
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Leaderboard server").family(look::bold_family()));
                ui.add(egui::TextEdit::singleline(&mut m.leaderboard_url).hint_text("https://…").desired_width(360.0));
            });
            ui.label(look::weak("Your best laps go on the OpenMCM2 leaderboard under your rider name, and Board shows everyone's. Clear the address to keep them on this computer only."));
            let url = m.leaderboard_url.trim().to_string();
            if let Some(note) = &self.boards.note {
                ui.label(look::weak(note));
            }
            let mine = mm2_settings::laps::read(&dir.join(mm2_settings::laps::BEST));
            if mine.is_empty() {
                ui.label(look::weak("No laps yet. Race a lap race and finish a lap after the first."));
                return;
            }
            egui::Grid::new("my laps").striped(true).num_columns(4).show(ui, |ui| {
                for lap in &mine {
                    ui.label(boards::name(&lap.board));
                    ui.label(&lap.rider);
                    ui.label(boards::time(lap.ms));
                    if ui.add_enabled(!url.is_empty(), egui::Button::new("Board")).on_hover_text("The best laps on the server for this one.").clicked() {
                        self.boards.show(&url, lap.board.clone());
                    }
                    ui.end_row();
                }
            });
            if let Some((board, rows)) = &self.boards.shown {
                ui.add_space(6.0);
                ui.label(egui::RichText::new(boards::name(board)).family(look::bold_family()));
                match rows {
                    None => {
                        ui.label(look::weak("Fetching…"));
                    }
                    Some(Err(e)) => {
                        ui.label(look::weak(e));
                    }
                    Some(Ok(rows)) if rows.is_empty() => {
                        ui.label(look::weak("No laps on this board yet."));
                    }
                    Some(Ok(rows)) => {
                        egui::Grid::new("board").striped(true).num_columns(3).show(ui, |ui| {
                            for (i, (rider, ms)) in rows.iter().enumerate().take(50) {
                                ui.label(format!("{}.", i + 1));
                                ui.label(rider);
                                ui.label(boards::time(*ms));
                                ui.end_row();
                            }
                        });
                    }
                }
            }
        });
    }

    // ---- Graphics ----

    fn graphics_page(&mut self, ui: &mut egui::Ui) {
        use look::{icon, Choice};
        look::page_title(ui, "Graphics", "How the game looks. F3 switches between the original look and the enhanced one while racing.");
        let s = &mut self.settings;
        look::card(ui, icon::ICON_PALETTE, "Look", |ui| {
            let choices = [
                Choice::new(false, "Original").icon(icon::ICON_HISTORY).hover("As the game drew it in 2000."),
                Choice::new(true, "Enhanced").icon(icon::ICON_AUTO_AWESOME).hover("Per-pixel light, real shadows, haze, dust and more. Each effect can be turned on or off on its own."),
            ];
            if let Some(v) = look::segmented(ui, s.enhanced, &choices) {
                s.enhanced = v;
            }
            ui.label(look::weak("The riding is the same either way."));
        });
        if s.enhanced {
            look::card(ui, icon::ICON_SPEED, "Presets", |ui| {
                // The preset the settings are (another one, if changed to
                // just what it sets, which then stands for them), or none:
                // Custom.
                let shown = preset_of(s);
                if let Some(q) = shown {
                    s.quality = q;
                }
                // Auto stands first, as its own pick (`Some(None)`).
                const AUTO: Option<Option<Quality>> = Some(None);
                let current = if s.auto { AUTO } else { shown.map(Some) };
                let mut choices: Vec<_> = std::iter::once(Choice::new(AUTO, "Auto").icon(icon::ICON_AUTO_MODE).hover(AUTO_TEXT))
                    .chain(Quality::ALL.iter().map(|&q| Choice::new(Some(Some(q)), q.label()).hover(preset_text(q))))
                    .collect();
                if shown.is_none() && !s.auto {
                    choices.push(
                        Choice::new(None, "Custom")
                            .icon(icon::ICON_TUNE)
                            .hover(format!("Your own mix, changed from the {} preset. Pick a preset to go back to its settings.", s.quality.label())),
                    );
                }
                let picked = look::segmented(ui, current, &choices);
                if picked == Some(AUTO) {
                    s.auto = true;
                }
                if let Some(Some(Some(q))) = picked {
                    // A preset sets the effects, the anti-aliasing and the
                    // resolution; any can be changed after.
                    s.auto = false;
                    s.quality = q;
                    for (u, on) in UPGRADES.iter().zip(s.upgrades.iter_mut()) {
                        *on = q.has(u.0);
                    }
                    s.antialiasing = q.antialiasing().into();
                    s.resolution = "auto".into();
                }
                let about = match preset_of(s) {
                    _ if s.auto => match auto_last() {
                        Some((q, card)) => format!("{AUTO_TEXT} Last time it picked {q} for your {card}."),
                        None => format!("{AUTO_TEXT} It says what it picked here after the game's first start."),
                    },
                    Some(q) => format!("{} Change any of the settings below and it becomes Custom.", preset_text(q)),
                    None => format!(
                        "Custom: your own mix of the settings below, changed from the {} preset. Pick a preset to go back to its settings.",
                        s.quality.label()
                    ),
                };
                ui.add(egui::Label::new(look::weak(about)).wrap());
            });
            // Under Auto, changing what it picks means picking yourself.
            let before = (s.antialiasing.clone(), s.resolution.clone(), s.upgrades.clone());
            look::card(ui, icon::ICON_IMAGE, "Picture", |ui| {
                look::label(
                    ui,
                    "Anti-aliasing",
                    "MSAA smooths the edges of models and ground. TAA blends each frame with the ones before, so billboards, leaves and fences hold still too, and sharpens a scene drawn below 100%.",
                );
                let current = ANTIALIASING.iter().find(|a| a.0 == s.antialiasing).map_or("msaa4", |a| a.0);
                let choices: Vec<_> = ANTIALIASING
                    .iter()
                    .map(|a| Choice::new(a.0, if a.0 == "taa" { "TAA" } else { a.1 }).hover(match a.0 {
                        "off" => "None: the fastest, the roughest edges.",
                        "taa" => "Temporal: steadier and about as fast as MSAA 2×; a little softer.",
                        _ => "Each step up costs frame rate, most with ray tracing.",
                    }))
                    .collect();
                if let Some(a) = look::segmented(ui, current, &choices) {
                    s.antialiasing = a.into();
                }
                ui.add_space(6.0);
                let named = look::label(
                    ui,
                    "Scene resolution",
                    "How big the 3D scene is drawn, as a share of the screen; the menus and race display stay sharp. The biggest cost to frame rate, ray tracing most of all.",
                );
                let mut auto = s.resolution == "auto";
                let mut pct = s.resolution.trim_end_matches('%').parse::<u32>().unwrap_or(100);
                ui.horizontal(|ui| {
                    if look::switch(ui, &mut auto).on_hover_text("The preset's own scene size. Turn it off to pick a share of the screen.").changed() {
                        s.resolution = if auto { "auto".into() } else { format!("{pct}%") };
                    }
                    ui.label(egui::RichText::new("Auto").family(look::bold_family()));
                    ui.label(look::weak(format!("the preset's own: {}", s.quality.most_pixels())));
                });
                ui.add_enabled_ui(!auto, |ui| {
                    if ui
                        .add(egui::Slider::new(&mut pct, 50..=100).suffix("%").trailing_fill(true))
                        .labelled_by(named)
                        .on_hover_text("How big the 3D scene is drawn, as a share of the screen. Lower is faster.")
                        .changed()
                    {
                        s.resolution = format!("{pct}%");
                    }
                });
                ui.add_space(6.0);
                let named = look::label(ui, "Ground detail", "How strongly the grain of the photo ground shows.");
                let mut detail = (s.detail * 100.0).round() as u32;
                if ui
                    .add(egui::Slider::new(&mut detail, 0..=100).suffix("%").trailing_fill(true))
                    .labelled_by(named)
                    .on_hover_text("How strongly the grain of the photo ground shows.")
                    .changed()
                {
                    s.detail = detail as f32 / 100.0;
                }
            });
            look::card(ui, icon::ICON_AUTO_FIX_HIGH, "Effects", |ui| {
                ui.label(look::weak("Each on its own. The preset sets them; change any and it becomes Custom."));
                ui.add_space(4.0);
                // Ray tracing takes the place of the sun's shadows, so it
                // needs those on.
                let shadows = UPGRADES.iter().zip(&s.upgrades).any(|(u, on)| u.0 == "shadows" && *on);
                let width = ((ui.available_width() - 8.0) / 2.0).floor();
                // Ray tracing first, the biggest change; then as listed.
                let order: Vec<usize> = (0..UPGRADES.len()).filter(|&i| UPGRADES[i].0 == "raytracing").chain((0..UPGRADES.len()).filter(|&i| UPGRADES[i].0 != "raytracing")).collect();
                for pair in order.chunks(2) {
                    ui.horizontal_top(|ui| {
                        for &i in pair {
                            let u = UPGRADES[i];
                            let enabled = u.0 != "raytracing" || shadows;
                            ui.add_enabled_ui(enabled, |ui| {
                                let r = look::switch_tile(ui, &mut s.upgrades[i], u.1, effect_about(u.0), width);
                                if u.0 == "raytracing" {
                                    r.on_hover_text("Needs Sun shadows on, and a graphics card that can trace rays (NVIDIA RTX, AMD RX 6000 or newer); on others the usual look shows.");
                                }
                            });
                        }
                    });
                }
            });
            if s.auto && before != (s.antialiasing.clone(), s.resolution.clone(), s.upgrades.clone()) {
                s.auto = false;
            }
        }
        if !cfg!(target_os = "macos") {
            look::card(ui, icon::ICON_MEMORY, "Graphics card", |ui| {
                look::switch_tile(
                    ui,
                    &mut s.opengl,
                    "Use OpenGL",
                    "For older graphics cards: draws with OpenGL instead of Vulkan or DirectX. Try it if the game won't start or shows a black window. The enhanced look needs Vulkan or DirectX.",
                    ui.available_width(),
                );
            });
        }
        look::card(ui, icon::ICON_TEXTURE, "Textures", |ui| {
            ui.horizontal(|ui| {
                look::switch(ui, &mut s.use_hires)
                    .on_hover_text("Draws the game with a pack of sharper textures from a folder you choose, in place of the game's own.");
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(egui::RichText::new("High-res textures").family(look::bold_family()));
                    let text = s.hires.as_ref().map_or("No folder chosen".to_string(), |h| h.display().to_string());
                    ui.add(egui::Label::new(look::weak(text)).truncate());
                });
            });
            ui.add_enabled_ui(s.use_hires, |ui| {
                ui.horizontal(|ui| {
                    if ui.button(look::with(look::icon::ICON_DRIVE_FOLDER_UPLOAD, "Choose texture folder…")).clicked() {
                        if let Some(d) = rfd::FileDialog::new().set_title("High-res texture folder").pick_folder() {
                            s.hires = Some(d);
                        }
                    }
                    if s.hires.is_some() && ui.button("Clear").clicked() {
                        s.hires = None;
                    }
                });
            });
        });
    }

    // ---- Updates ----

    /// The notice when a newer release is out, and the download's progress
    /// once the player says yes.
    fn update_banner(&mut self, ui: &mut egui::Ui, busy: bool) {
        let mut act = None;
        match &self.update {
            Update::Found(r) => {
                look::card(ui, look::icon::ICON_DOWNLOAD, "Update available", |ui| {
                    ui.add(egui::Label::new(format!(
                        "OpenMCM2 {} is out. You have {}. Updating replaces only the program; your game data and settings stay as they are.",
                        r.tag,
                        update::current().unwrap_or("?")
                    )).wrap());
                    ui.horizontal(|ui| {
                        let can = !busy && self.running.is_none();
                        let b = egui::Button::new(egui::RichText::new("Update and restart").color(egui::Color32::WHITE)).fill(look::ACCENT);
                        if ui.add_enabled(can, b).clicked() {
                            act = Some(true);
                        }
                        ui.hyperlink_to("What's new", &r.page);
                        if ui.button("Not now").clicked() {
                            act = Some(false);
                        }
                    });
                    if self.running.is_some() {
                        ui.label(look::weak("Close the game first."));
                    }
                });
            }
            Update::Installing(r, p) => {
                look::card(ui, look::icon::ICON_DOWNLOADING, &format!("Updating to {}", r.tag), |ui| {
                    let total = p.total.load(Ordering::Relaxed).max(1);
                    let done = p.done.load(Ordering::Relaxed);
                    ui.horizontal(|ui| {
                        ui.add(egui::ProgressBar::new(done as f32 / total as f32).show_percentage().desired_width(ui.available_width() - 90.0));
                        if ui.button("Cancel").clicked() {
                            p.cancel.store(true, Ordering::Relaxed);
                        }
                    });
                    ui.label(look::weak(p.stage.lock().unwrap().clone()));
                });
            }
            _ => {}
        }
        match (act, &self.update) {
            (Some(true), Update::Found(r)) => self.install_update(r.clone()),
            (Some(false), Update::Found(r)) => self.update = Update::Dismissed(r.clone()),
            _ => {}
        }
    }

    fn updates_page(&mut self, ui: &mut egui::Ui) {
        look::page_title(ui, "Updates", "OpenMCM2 can update itself from its releases on GitHub.");
        let busy = self.import.is_some();
        self.update_banner(ui, busy);
        let mut check = false;
        let mut install = None;
        look::card(ui, look::icon::ICON_UPDATE, "This copy", |ui| {
            match update::current() {
                Some(v) => ui.label(format!("This is OpenMCM2 {v}.")),
                None => ui.label("This copy of OpenMCM2 was built from source."),
            };
            if let Err(e) = &self.install {
                ui.label(look::weak(format!("It can't update itself: {e}.")));
                return;
            }
            ui.horizontal(|ui| {
                look::switch(ui, &mut self.settings.check_updates);
                ui.label("Check for updates when the launcher opens");
            });
            let can = self.import.is_none() && self.running.is_none();
            ui.horizontal(|ui| {
                let idle = !matches!(self.update, Update::Checking(_) | Update::Installing(..));
                if ui.add_enabled(idle, egui::Button::new(look::with(look::icon::ICON_REFRESH, "Check now"))).clicked() {
                    check = true;
                }
                match &self.update {
                    Update::Checking(_) => {
                        ui.spinner();
                        ui.label("Checking…");
                    }
                    Update::UpToDate => look::status(ui, true, "You have the latest version."),
                    Update::Found(r) | Update::Dismissed(r) => {
                        ui.label(format!("{} is out.", r.tag));
                        if ui.add_enabled(can, egui::Button::new("Update and restart")).clicked() {
                            install = Some(r.clone());
                        }
                    }
                    Update::Failed(e) => look::status(ui, false, &format!("Update: {e}")),
                    _ => {}
                }
            });
        });
        look::card(ui, look::icon::ICON_FAVORITE, "Enjoying OpenMCM2?", |ui| {
            ui.add(egui::Label::new("Want more updates? OpenMCM2 is free and always will be. If you enjoy it, a small donation helps keep the updates coming.").wrap());
            let b = egui::Button::new(egui::RichText::new(look::with(look::icon::ICON_FAVORITE, "Donate on GitHub")).color(egui::Color32::WHITE)).fill(look::ACCENT);
            if ui.add(b).on_hover_text(SPONSOR_URL).clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(SPONSOR_URL));
            }
        });
        look::card(ui, look::icon::ICON_INFO, "Command line", |ui| {
            ui.label(look::weak("What the Play button starts, for a shortcut of your own or to report a problem."));
            let line = std::iter::once("mm2-viewer".to_string())
                .chain(self.settings.args().into_iter().map(|a| if a.contains(' ') { format!("\"{a}\"") } else { a }))
                .collect::<Vec<_>>()
                .join(" ");
            egui::Frame::new().fill(look::SUNKEN).corner_radius(8).inner_margin(10).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add(egui::Label::new(egui::RichText::new(line).monospace()).wrap());
            });
        });
        if check {
            self.check_updates();
        }
        if let Some(r) = install {
            self.install_update(r);
        }
    }

    // ---- Tracks ----

    fn tracks_page(&mut self, ui: &mut egui::Ui, busy: bool) {
        let site = &tracks::SITES[self.tracks.site];
        look::page_title(ui, "Tracks", "Thousands of tracks made by players, added to the game's own.");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.label(look::weak("Each one is downloaded from "));
            ui.hyperlink_to(site.name, site.home);
            ui.label(look::weak(" only when you pick it, and belongs to the player who made it. Added tracks show up in the game's track list for their event."));
        });
        ui.add_space(8.0);
        let Some(game) = self.settings.game.clone() else {
            look::card(ui, look::icon::ICON_DISC_FULL, "Set up the game data first", |ui| {
                ui.label(look::weak("Tracks are added to your copy of the game: choose it on the Play page."));
            });
            return;
        };
        if busy {
            ui.label(look::weak("Waiting for the game data to finish copying…"));
            return;
        }
        let t = &mut self.tracks;
        if t.listing.is_none() && t.fetching.is_none() && t.error.is_none() {
            t.kind = Some(0);
            t.fetch(1);
        }
        let mut fetch = None;
        let mut add = None;
        let mut remove = None;
        look::card(ui, look::icon::ICON_SEARCH, &format!("Browse {}", site.name), |ui| {
            let kinds: Vec<_> = site.kinds.iter().enumerate().map(|(i, k)| look::Choice::new(Some(i), k.label)).collect();
            if let Some(k) = look::segmented(ui, t.kind, &kinds) {
                t.kind = k;
                fetch = Some(1);
            }
            ui.horizontal(|ui| {
                let edit = ui.add(egui::TextEdit::singleline(&mut t.search).hint_text("Search by name").desired_width(ui.available_width() - 110.0));
                let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (ui.button(look::with(look::icon::ICON_SEARCH, "Search")).clicked() || enter) && !t.search.trim().is_empty() {
                    t.kind = None;
                    fetch = Some(1);
                }
            });
            let n = t.added.count(&game);
            if n > 0 {
                ui.label(look::weak(format!("{n} added so far.")));
            }
        });
        if t.fetching.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(format!("Asking {}…", site.name));
            });
        }
        if let Some(e) = &t.error {
            look::status(ui, false, e);
        }
        if let Some(listing) = &t.listing {
            if listing.tracks.is_empty() && t.fetching.is_none() {
                ui.label("No tracks found.");
            }
            for track in &listing.tracks {
                egui::Frame::new().fill(look::CARD).stroke(egui::Stroke::new(1.0, look::LINE)).corner_radius(10).inner_margin(12).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.label(egui::RichText::new(&track.name).family(look::bold_family()).size(15.0));
                            ui.horizontal(|ui| {
                                let kind = site.kinds.iter().find(|k| k.folder == track.folder).map_or("", |k| k.label);
                                let by = if track.author.is_empty() { kind.to_string() } else { format!("{kind} · by {}", track.author) };
                                ui.label(look::weak(by));
                                if let Some(p) = &track.picture {
                                    ui.hyperlink_to("picture", p);
                                }
                            });
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if let Some(job) = t.jobs.get(&track.url) {
                                if ui.button("Cancel").clicked() {
                                    job.cancel.store(true, Ordering::Relaxed);
                                }
                                let total = job.total.load(Ordering::Relaxed).max(1);
                                let done = job.done.load(Ordering::Relaxed);
                                ui.add(egui::ProgressBar::new(done as f32 / total as f32).desired_width(110.0));
                            } else if t.added.has(&game, &track.url) {
                                if ui.button(look::with(look::icon::ICON_DELETE, "Remove")).clicked() {
                                    remove = Some(track.url.clone());
                                }
                                ui.label(egui::RichText::new(look::with(look::icon::ICON_CHECK_CIRCLE, "Added")).color(look::GOOD));
                            } else if track.can_add() {
                                let b = egui::Button::new(egui::RichText::new(look::with(look::icon::ICON_DOWNLOAD, "Get")).color(egui::Color32::WHITE)).fill(look::ACCENT);
                                if ui.add(b).clicked() {
                                    add = Some(track.clone());
                                }
                            } else {
                                ui.hyperlink_to("Download page", site.home).on_hover_text("This one is a .rar archive, which the launcher can't unpack.");
                            }
                        });
                    });
                    if let Some(e) = t.failed.get(&track.url) {
                        look::status(ui, false, &format!("Couldn't add it: {e}"));
                    }
                });
                ui.add_space(6.0);
            }
            let pages = listing.pages.max(1);
            ui.horizontal(|ui| {
                if ui.add_enabled(t.page > 1 && t.fetching.is_none(), egui::Button::new("Previous")).clicked() {
                    fetch = Some(t.page - 1);
                }
                ui.label(format!("Page {} of {pages}", t.page.max(1)));
                if ui.add_enabled(t.page < pages && t.fetching.is_none(), egui::Button::new("Next")).clicked() {
                    fetch = Some(t.page + 1);
                }
            });
        }
        if let Some(page) = fetch {
            t.fetch(page);
        }
        if let Some(track) = add {
            t.failed.remove(&track.url);
            t.jobs.insert(track.url.clone(), tracks::add(track, game.clone()));
        }
        if let Some(url) = remove {
            if let Err(e) = t.added.remove(&game, &url) {
                t.failed.insert(url, e);
            }
        }
    }
}

/// Whether the effects, anti-aliasing and scene resolution in `s` are just
/// what preset `q` sets.
fn is_preset(s: &Settings, q: Quality) -> bool {
    s.antialiasing == q.antialiasing() && s.resolution == "auto" && UPGRADES.iter().zip(&s.upgrades).all(|(u, on)| *on == q.has(u.0))
}

/// The preset `s` is: the one picked, or another it was changed to match;
/// none when it is the player's own mix (Custom).
fn preset_of(s: &Settings) -> Option<Quality> {
    if is_preset(s, s.quality) {
        return Some(s.quality);
    }
    Quality::ALL.into_iter().find(|&q| is_preset(s, q))
}

/// What the Auto preset does, for its hover text.
const AUTO_TEXT: &str = "Auto: each time the game starts it looks at your graphics card and screen and picks Low, Medium or High to keep the racing smooth. Change any setting below to choose yourself.";

/// What Auto picked when the game last started, and for which card, as
/// the game left it in `auto-graphics.txt`.
fn auto_last() -> Option<(String, String)> {
    let text = std::fs::read_to_string(settings::user_dir().join("auto-graphics.txt")).ok()?;
    let get = |k: &str| text.lines().find_map(|l| l.strip_prefix(k).map(str::trim).map(String::from));
    let q = get("quality=")?;
    let q = Quality::ALL.into_iter().find(|x| x.name() == q).map_or(q, |x| x.label().to_string());
    Some((q, get("card=").unwrap_or_else(|| "graphics card".into())))
}

/// A line on what each effect ([`UPGRADES`]) does, for its switch.
fn effect_about(name: &str) -> &'static str {
    match name {
        "lighting" => "Light worked out for every pixel, with bumps and highlights.",
        "shadows" => "Every object casts a shadow from the sun.",
        "distance" => "The whole track drawn, fading into haze.",
        "bloom" => "Bright lights and the sun glow softly.",
        "ao" => "Corners and contacts darkened, as ambient light is.",
        "gtao" => "A truer ambient occlusion, from the horizon round each pixel: deeper in creases and corners, cleaner round edges. Needs Ambient occlusion on.",
        "contact" => "A thin, soft shadow right where tyres, boots and props touch the ground, finer than the sun's shadows can draw.",
        "ssr" => "In the rain, puddles and wet ground mirror the bikes, riders and trees on screen. Off when ray tracing draws the reflections.",
        "skylight" => "The sky's light worked out over the land when a track loads: a little dimmer in dips, under banks and between hills.",
        "ground" => "The ground re-made from photo materials.",
        "grass" => "Grass growing on the grassy ground near you.",
        "smooth" => "The hills round you smoothed, not faceted.",
        "dust" => "Clouds of dust behind the bikes, casting shadows.",
        "mud" => "Mud and spray caking bikes and riders.",
        "bikes" => "Shiny paint and metal, and smoother riders.",
        "shafts" => "Beams of sunlight past hills, trees and riders.",
        "cinematic" => "Replays filmed as through a lens.",
        "ruts" => "Tyre ruts that stay for the race.",
        "scenery" => "Props along the course, a paddock and a crowd.",
        "grade" => "A bolder look: colour, contrast, a vignette.",
        "raytracing" => "Truer shadows, light shafts and reflections, traced as rays.",
        _ => "",
    }
}

/// "1920 × 1080 (Full HD 1080p)", or just the size when it has no name.
fn size_text((w, h): (u32, u32)) -> String {
    let name = SHAPES.iter().flat_map(|s| s.2).find(|z| (z.0, z.1) == (w, h)).map_or("", |z| z.2);
    match name {
        "" => format!("{w} × {h}"),
        _ => format!("{w} × {h} ({name})"),
    }
}

/// What a quality preset sets, for its hover text.
fn preset_text(q: Quality) -> String {
    let effects = match q {
        Quality::Low => "without ambient occlusion, light shafts, smooth hills, replay blur or 3D grass",
        Quality::Medium => "without light shafts or smooth hills, with TAA rebuilding the scene at the screen's sharpness",
        Quality::RayTraced => "High's effects with ray tracing (needs a graphics card that can trace rays)",
        Quality::High => "every effect that's on unless changed",
        _ => "every effect but ray tracing, with lasting ruts",
    };
    let msaa = ANTIALIASING.iter().find(|a| a.0 == q.antialiasing()).map_or("", |a| a.1);
    format!("{}: {effects}; anti-aliasing {msaa}; scene resolution {}.", q.label(), q.most_pixels())
}

#[cfg(test)]
mod shots {
    use super::*;

    /// Draws each page into the folder `MM2_SHOTS`, to look at:
    /// `MM2_SHOTS=dir cargo test -p mm2-launcher shots -- --ignored`.
    #[test]
    #[ignore]
    fn shots() {
        let dir = std::env::var("MM2_SHOTS").expect("MM2_SHOTS: where to draw the pages");
        for (name, page, enhanced, height) in [
            ("play", Page::Play, true, 1300.0),
            ("graphics", Page::Graphics, true, 1900.0),
            ("graphics_original", Page::Graphics, false, 760.0),
            ("tracks", Page::Tracks, true, 760.0),
            ("updates", Page::Updates, true, 760.0),
            ("extras", Page::Extras, true, 3100.0),
            ("graphics_small", Page::Graphics, true, 520.0),
            ("graphics_custom", Page::Graphics, true, 520.0),
            ("graphics_auto", Page::Graphics, true, 520.0),
            ("play_full", Page::Play, true, 1300.0),
        ] {
            let width = if name.ends_with("small") { 760.0 } else { 1000.0 };
            let custom = name.ends_with("custom");
            let mut s = Settings::default();
            s.enhanced = enhanced;
            s.game = Some("C:/Games/Motocross Madness 2/GAME".into());
            s.quality = Quality::RayTraced;
            s.auto = name == "graphics_auto";
            s.fullscreen = name == "play_full";
            s.antialiasing = "taa".into();
            s.resolution = "75%".into();
            s.time = "night".into();
            for (u, on) in UPGRADES.iter().zip(s.upgrades.iter_mut()) {
                *on = Quality::RayTraced.has(u.0);
            }
            if page == Page::Extras {
                s.more.leaderboards = true;
                s.more.leaderboard_url = "https://laps.example.org".into();
                s.more.helmet_cam = true;
            }
            if custom {
                // Changed from the preset: Custom.
                s.antialiasing = "msaa4".into();
            }
            let mut l = Launcher::with(s, Err("built from source".into()));
            l.page = page;
            // No listing fetched from the net.
            l.tracks.error = Some("(not fetched in this picture)".into());
            // The harness draws in a frame with an 8 px margin round it:
            // 16 px more each way, cut off after, is the window's own.
            let mut h = egui_kittest::Harness::builder()
                .with_size([width + 16.0, height + 16.0])
                // As on a Windows set to light mode, where the launcher
                // must look the same.
                .with_theme(egui::Theme::Light)
                .with_context_setup(|ctx| look::apply(ctx))
                .build_ui_state(|ui, l: &mut Launcher| l.draw(ui), l);
            h.run();
            let image = h.render().expect("render");
            let image = image::imageops::crop_imm(&image, 8, 8, width as u32, height as u32).to_image();
            image.save(format!("{dir}/{name}.png")).expect("save");
        }
    }
}
