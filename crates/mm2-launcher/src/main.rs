//! The OpenMCM2 launcher: sets up the game data from the player's own
//! copy of Motocross Madness 2, picks the settings, and starts
//! `mm2-viewer` with them.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod game;
mod iso;
mod settings;

use eframe::egui;
use game::Source;
use settings::{Quality, Settings, UPGRADES};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::Ordering;
use std::sync::Arc;
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
        let mut l = Launcher { saved: settings.clone(), settings, import: None, running: None, note: None };
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

    fn play(&mut self) {
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
        match cmd.spawn() {
            Ok(child) => {
                self.running = Some(child);
                self.note = None;
            }
            Err(e) => self.note = Some((format!("Couldn't start the game: {e}"), true)),
        }
    }

    fn poll(&mut self, ctx: &egui::Context) {
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
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll(&ctx);
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
                self.play();
            }
            ui.add_space(8.0);
        });
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(4.0);
                ui.heading(egui::RichText::new("OpenMCM2").size(28.0).strong());
                ui.label("Motocross Madness 2, rebuilt to run natively.");
                ui.add_space(10.0);
                self.game_ui(ui, busy);
                self.display_ui(ui);
                self.graphics_ui(ui);
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
