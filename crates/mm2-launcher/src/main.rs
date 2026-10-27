//! The OpenMCM2 launcher: sets up the game data from the player's own
//! copy of Motocross Madness 2, picks the settings, and starts
//! `mm2-viewer` with them. It can also update OpenMCM2 itself from its
//! GitHub releases (see [`update`]).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod game;
mod iso;
mod settings;
mod update;

use eframe::egui;
use game::Source;
use settings::{Quality, Settings, UPGRADES};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const SIZES: [(u32, u32); 7] =
    [(800, 600), (1024, 768), (1280, 720), (1280, 960), (1600, 900), (1920, 1080), (2560, 1440)];
const TIMES: [(&str, &str); 6] =
    [("", "Track's own"), ("dawn", "Dawn"), ("morning", "Morning"), ("noon", "Noon"), ("afternoon", "Afternoon"), ("dusk", "Dusk")];
const WEATHERS: [(&str, &str); 3] = [("clear", "Clear"), ("overcast", "Overcast"), ("rain", "Rain")];
const ACCENT: egui::Color32 = egui::Color32::from_rgb(232, 120, 32);

fn main() -> eframe::Result {
    // `--import PATH [DEST]`: sets up the game data without the window.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--import") {
        std::process::exit(import_only(&args[1..]));
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("OpenMCM2")
            .with_inner_size([560.0, 760.0])
            .with_min_inner_size([480.0, 480.0]),
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
        let mut style = (*cc.egui_ctx.global_style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.selection.bg_fill = ACCENT;
        style.visuals.hyperlink_color = ACCENT;
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        cc.egui_ctx.set_global_style(style);
        let settings = Settings::load();
        let install = match update::current() {
            Some(_) => update::Install::here(),
            None => Err("this copy was built from source, so it doesn't update itself".into()),
        };
        if let Ok(i) = &install {
            i.tidy();
        }
        let mut l = Launcher {
            saved: settings.clone(),
            settings,
            import: None,
            running: None,
            note: None,
            install,
            update: Update::Idle,
        };
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
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                Ok(None) => ctx.request_repaint_after(Duration::from_millis(500)),
                Err(_) => self.running = None,
            }
        }
    }
}

fn section(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style()).inner_margin(12.0).corner_radius(6.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(egui::RichText::new(title).strong().size(16.0).color(ACCENT));
        ui.add_space(4.0);
        body(ui);
    });
    ui.add_space(8.0);
}

impl eframe::App for Launcher {
    // Runs while minimized too, so the launcher sees the game end.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let busy = self.import.is_some();
        egui::Panel::bottom("play").show(ui, |ui| {
            ui.add_space(8.0);
            if let Some((text, bad)) = &self.note {
                let colour = if *bad { egui::Color32::from_rgb(240, 90, 80) } else { egui::Color32::from_rgb(120, 200, 120) };
                ui.label(egui::RichText::new(text).color(colour));
            }
            let ready = self.settings.game.is_some() && !busy && self.running.is_none();
            let label = if self.running.is_some() { "Running…" } else { "PLAY" };
            let button = egui::Button::new(egui::RichText::new(label).size(22.0).strong().color(egui::Color32::WHITE))
                .fill(if ready { ACCENT } else { egui::Color32::from_gray(60) })
                .min_size(egui::vec2(ui.available_width(), 48.0));
            let clicked = ui.vertical_centered_justified(|ui| ui.add_enabled(ready, button).clicked()).inner;
            if clicked {
                let _ = self.settings.save();
                self.saved = self.settings.clone();
                self.play(&ctx);
            }
            ui.add_space(8.0);
        });
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(4.0);
                ui.heading(egui::RichText::new("OpenMCM2").size(28.0).strong());
                ui.label("Motocross Madness 2, rebuilt to run natively.");
                ui.add_space(10.0);
                self.update_banner(ui, busy);
                self.game_ui(ui, busy);
                self.display_ui(ui);
                self.graphics_ui(ui);
                self.updates_ui(ui);
                ui.collapsing("Command line", |ui| {
                    let line = std::iter::once("mm2-viewer".to_string())
                        .chain(self.settings.args().into_iter().map(|a| if a.contains(' ') { format!("\"{a}\"") } else { a }))
                        .collect::<Vec<_>>()
                        .join(" ");
                    ui.add(egui::Label::new(egui::RichText::new(line).monospace()).wrap());
                });
            });
        });
        if self.settings != self.saved && !busy {
            let _ = self.settings.save();
            self.saved = self.settings.clone();
        }
    }
}

impl Launcher {
    fn game_ui(&mut self, ui: &mut egui::Ui, busy: bool) {
        section(ui, "Game data", |ui| {
            ui.label(
                "OpenMCM2 doesn't include the game. Point it at your own Motocross Madness 2: \
                 the CD, a CD image of it (.iso, .bin/.cue, .img), or the folder it's installed in.",
            );
            ui.add_space(4.0);
            match &self.settings.game {
                Some(g) => ui.label(egui::RichText::new(format!("Ready: {}", g.display())).color(egui::Color32::from_rgb(120, 200, 120))),
                None => ui.label(egui::RichText::new("Not set up yet").color(egui::Color32::from_rgb(240, 90, 80))),
            };
            if let Some(p) = &self.import {
                let total = p.total.load(Ordering::Relaxed).max(1);
                let done = p.done.load(Ordering::Relaxed);
                ui.horizontal(|ui| {
                    ui.add(egui::ProgressBar::new(done as f32 / total as f32).show_percentage().desired_width(ui.available_width() - 80.0));
                    if ui.button("Cancel").clicked() {
                        p.cancel.store(true, Ordering::Relaxed);
                    }
                });
                ui.label(format!("Copying game data… {} of {} MB", done >> 20, total >> 20));
            }
            ui.horizontal(|ui| {
                ui.add_enabled_ui(!busy, |ui| {
                    if ui.button("Choose CD image…").clicked() {
                        if let Some(f) = rfd::FileDialog::new()
                            .set_title("Your Motocross Madness 2 CD image")
                            .add_filter("CD image", &["iso", "bin", "cue", "img", "mdf", "nrg"])
                            .add_filter("All files", &["*"])
                            .pick_file()
                        {
                            self.pick(f);
                        }
                    }
                    if ui.button("Choose CD or install folder…").clicked() {
                        if let Some(d) = rfd::FileDialog::new().set_title("Your Motocross Madness 2 CD or install folder").pick_folder() {
                            self.pick(d);
                        }
                    }
                });
            });
        });
    }

    fn display_ui(&mut self, ui: &mut egui::Ui) {
        let s = &mut self.settings;
        section(ui, "Display", |ui| {
            ui.checkbox(&mut s.fullscreen, "Full screen (Alt+Enter switches in game)");
            ui.add_enabled_ui(!s.fullscreen, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Window size");
                    egui::ComboBox::from_id_salt("size")
                        .selected_text(format!("{} × {}", s.window.0, s.window.1))
                        .show_ui(ui, |ui| {
                            for size in SIZES {
                                ui.selectable_value(&mut s.window, size, format!("{} × {}", size.0, size.1));
                            }
                        });
                });
            });
            ui.checkbox(&mut s.vsync, "VSync");
            ui.checkbox(&mut s.show_fps, "Show frame rate");
        });
    }

    fn graphics_ui(&mut self, ui: &mut egui::Ui) {
        let s = &mut self.settings;
        section(ui, "Graphics", |ui| {
            ui.horizontal(|ui| {
                ui.radio_value(&mut s.enhanced, false, "Original look");
                ui.radio_value(&mut s.enhanced, true, "Enhanced look");
            });
            ui.label(egui::RichText::new("F3 switches between the two while racing. The riding is the same either way.").weak());
            if s.enhanced {
                ui.add_space(4.0);
                egui::Grid::new("enhanced").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                    ui.label("Quality");
                    let before = s.quality;
                    egui::ComboBox::from_id_salt("quality").selected_text(title(s.quality.name())).show_ui(ui, |ui| {
                        for q in Quality::ALL {
                            ui.selectable_value(&mut s.quality, q, title(q.name()));
                        }
                    });
                    if s.quality != before {
                        for (u, on) in UPGRADES.iter().zip(s.upgrades.iter_mut()) {
                            *on = s.quality.has(u.0);
                        }
                    }
                    ui.end_row();
                    ui.label("Time of day");
                    pick(ui, "time", &mut s.time, &TIMES);
                    ui.end_row();
                    ui.label("Weather");
                    pick(ui, "weather", &mut s.weather, &WEATHERS);
                    ui.end_row();
                    ui.label("Ground detail");
                    ui.add(egui::Slider::new(&mut s.detail, 0.0..=1.0).fixed_decimals(2));
                    ui.end_row();
                });
                ui.collapsing("Individual effects", |ui| {
                    egui::Grid::new("upgrades").num_columns(2).show(ui, |ui| {
                        for (i, (u, on)) in UPGRADES.iter().zip(s.upgrades.iter_mut()).enumerate() {
                            ui.checkbox(on, u.1);
                            if i % 2 == 1 {
                                ui.end_row();
                            }
                        }
                    });
                });
            }
            ui.add_space(6.0);
            ui.separator();
            ui.checkbox(&mut s.use_hires, "High-res textures");
            ui.add_enabled_ui(s.use_hires, |ui| {
                ui.horizontal(|ui| {
                    let text = s.hires.as_ref().map_or("No folder chosen".to_string(), |h| h.display().to_string());
                    ui.add(egui::Label::new(egui::RichText::new(text).weak()).truncate());
                });
                ui.horizontal(|ui| {
                    if ui.button("Choose texture folder…").clicked() {
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
}

impl Launcher {
    /// The notice at the top when a newer release is out, and the
    /// download's progress once the player says yes.
    fn update_banner(&mut self, ui: &mut egui::Ui, busy: bool) {
        let mut act = None;
        match &self.update {
            Update::Found(r) => {
                section(ui, "Update available", |ui| {
                    ui.label(format!(
                        "OpenMCM2 {} is out. You have {}. Updating replaces only the program; your game data and settings stay as they are.",
                        r.tag,
                        update::current().unwrap_or("?")
                    ));
                    ui.horizontal(|ui| {
                        let can = !busy && self.running.is_none();
                        if ui.add_enabled(can, egui::Button::new("Update and restart")).clicked() {
                            act = Some(true);
                        }
                        ui.hyperlink_to("What's new", &r.page);
                        if ui.button("Not now").clicked() {
                            act = Some(false);
                        }
                    });
                    if self.running.is_some() {
                        ui.label(egui::RichText::new("Close the game first.").weak());
                    }
                });
            }
            Update::Installing(r, p) => {
                section(ui, &format!("Updating to {}", r.tag), |ui| {
                    let total = p.total.load(Ordering::Relaxed).max(1);
                    let done = p.done.load(Ordering::Relaxed);
                    ui.horizontal(|ui| {
                        ui.add(egui::ProgressBar::new(done as f32 / total as f32).show_percentage().desired_width(ui.available_width() - 80.0));
                        if ui.button("Cancel").clicked() {
                            p.cancel.store(true, Ordering::Relaxed);
                        }
                    });
                    ui.label(p.stage.lock().unwrap().clone());
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

    fn updates_ui(&mut self, ui: &mut egui::Ui) {
        let mut check = false;
        let mut install = None;
        section(ui, "Updates", |ui| {
            match update::current() {
                Some(v) => ui.label(format!("This is OpenMCM2 {v}.")),
                None => ui.label("This copy of OpenMCM2 was built from source."),
            };
            if let Err(e) = &self.install {
                ui.label(egui::RichText::new(format!("It can't update itself: {e}.")).weak());
                return;
            }
            ui.checkbox(&mut self.settings.check_updates, "Check for updates when the launcher opens");
            ui.horizontal(|ui| {
                let idle = !matches!(self.update, Update::Checking(_) | Update::Installing(..));
                if ui.add_enabled(idle, egui::Button::new("Check now")).clicked() {
                    check = true;
                }
                match &self.update {
                    Update::Checking(_) => {
                        ui.spinner();
                        ui.label("Checking…");
                    }
                    Update::UpToDate => {
                        ui.label(egui::RichText::new("You have the latest version.").color(egui::Color32::from_rgb(120, 200, 120)));
                    }
                    Update::Found(r) => {
                        ui.label(format!("{} is out.", r.tag));
                    }
                    Update::Dismissed(r) => {
                        ui.label(format!("{} is out.", r.tag));
                        if ui.button("Update and restart").clicked() {
                            install = Some(r.clone());
                        }
                    }
                    Update::Failed(e) => {
                        ui.add(egui::Label::new(egui::RichText::new(format!("Update: {e}")).color(egui::Color32::from_rgb(240, 90, 80))).wrap());
                    }
                    _ => {}
                }
            });
        });
        if check {
            self.check_updates();
        }
        if let Some(r) = install {
            self.update = Update::Found(r);
        }
    }
}

fn title(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

fn pick(ui: &mut egui::Ui, id: &str, value: &mut String, choices: &[(&str, &str)]) {
    let shown = choices.iter().find(|c| c.0 == value).map_or(value.as_str(), |c| c.1).to_string();
    egui::ComboBox::from_id_salt(id).selected_text(shown).show_ui(ui, |ui| {
        for (key, label) in choices {
            if ui.selectable_label(value == key, *label).clicked() {
                *value = key.to_string();
            }
        }
    });
}
