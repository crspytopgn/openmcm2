//! `mm2-viewer`: ride or fly around a Motocross Madness 2 track.
//!
//! ```text
//! mm2-viewer <GAME/TERAFORM/.../TRACK.ENV>
//! mm2-viewer                      // with the launcher's saved settings, else the
//!                                 // game's menus (GAME found above here)
//! mm2-viewer <GAME folder>
//! mm2-viewer <TRACK.ENV> [--laps 3] [--riders 4]
//! mm2-viewer <TRACK.ENV> --screenshot out.png [--size 1280x720]
//!     [--camera x,y,z,yaw,pitch]   // game coordinates and degrees
//!     [--time seconds]             // how far into the animations
//!     [--ride seconds [--steer -1..1] [--front]]  // race first; the computer rides
//!                                       // for you unless you steer
//! mm2-viewer <TRACK.ENV> --hires <dir>   // with upscaled textures
//! mm2-viewer ... --enhanced[=no-bloom,no-ao,...]  // modern graphics (F3 compares)
//! mm2-viewer ... --enhanced=raytracing     // ray-traced shadows and reflections, where the GPU can
//! mm2-viewer ... --fullscreen [--no-vsync]  // or Alt+Enter while playing
//! mm2-viewer ... --opengl                  // draw with OpenGL, for GPUs without Vulkan
//! mm2-viewer ... --show-fps                // or F2: the frame rate in the title and terminal
//! mm2-viewer ... --centred-display         // ultrawide: the race display in a 16:9 middle
//! mm2-viewer ... --modern-hud             // crisp see-through speedometer, panel and radar
//! mm2-viewer ... --vibration=N|off         // pad vibration strength 0 to 100 (else the profile's)
//! mm2-viewer ... --air-steer[=N]           // steering in the air bends the flight, N% of the fork's (0-200)
//! mm2-viewer ... --air-brake[=N]           // braking in the air shortens the flight, N% of the fork's (0-200)
//! mm2-viewer ... --quick-get-up            // G or a pad's Back puts you back on after a crash
//! mm2-viewer ... --clean-starts            // computer riders keep their order across the gate
//! mm2-viewer ... --one-lap                 // National and Supercross races may be 1 lap
//! mm2-viewer ... --kmh                     // the speedometer in km/h
//! mm2-viewer ... --mute-in-background      // quiet while the window is in the background
//! mm2-viewer ... --screenshot-key          // Ctrl+P saves a screenshot
//! mm2-viewer ... --photo-mode              // F in a race: hold it still, fly the camera, Enter saves a picture
//! mm2-viewer ... --lap-gap                 // live gap to the best lap under the race panel
//! mm2-viewer ... --steady-camera           // the chase camera turns and rises more gently
//! mm2-viewer ... --fov=N                   // the Chase Cam's field of view (the game's 85)
//! mm2-viewer ... --difficulty=N,..         // the Difficulty box's 12 steps (1-10), row by row, Easy to Hard
//! ```
//!
//! Four bikes and riders from `RES/BIKE.RES` race from the start line in
//! the track's `.tdf`: the player on one, the computer on the others,
//! following the course. The window title shows the lap, place and times.
//!
//! Riding: W/Up throttle, S/Down brake, A/D or Left/Right to steer; in
//! the air, throttle and brake tip the bike back and forward (the game's
//! gas and brake gyro options).
//! Xbox and PlayStation pads and flight sticks work too (see `pad`). R restarts the race, P lets
//! the computer ride for you, Tab switches to the free camera. Free camera: hold the right mouse button and move to
//! look, WASD to fly, Q/E or Space/Ctrl to go down/up, Shift to go faster.
//! Esc quits.

// No console window on Windows (it showed behind the game); the launcher
// keeps stderr in last-run.log.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bench;
mod gpu_free;
mod free_text;
mod demo;
mod demo_tracks;
mod drive;
mod drive_sound;
mod help;
mod addons;
mod ibl;
mod pace;
mod dynres;
mod bars;
mod builder;
mod eco;
mod enhance;
mod rt;
mod flood;
mod night;
#[cfg(test)]
mod shader_tests;
mod gpu_time;
mod grass;
mod ruts;
mod dust;
mod rain;
mod smooth;
mod scenery;
mod crowd;
mod cull;
mod materials;
mod fine_ground;
mod graphics;
mod online;
mod playclock;
mod polish;
mod post;
mod finish;
mod podium;
mod audio;
mod camera;
mod countdown;
mod cues;
mod reverb;
mod freepack;
mod freshen;
mod free_tracks;
mod menu_link;
mod frontend;
#[cfg(target_arch = "wasm32")]
mod web;
mod fs;
mod job;
mod gates;
mod ground;
mod hires;
mod hud;
mod input;
#[cfg(feature = "gamepad")]
mod pad;
mod morph;
mod skin;
mod movie;
mod net;
mod objects;
mod particles;
mod popup;
mod preview;
mod procircuit;
mod profile;
mod race;
mod renderer;
mod replay;
mod ride;
mod riding;
mod roster;
mod rumble;
mod lapgap;
mod tweaks;
mod water;
mod display;
mod screens;
mod servers;
#[cfg(not(target_arch = "wasm32"))]
mod serve;
mod share;
mod sky;
mod skylight;
mod sound;
mod sprites;
mod tag;
mod traffic;
mod ui;
mod update_note;
#[cfg(windows)]
mod winwatch;

use camera::Camera;
use glam::{Mat4, Vec3, Vec4};
use ground::TrackGround;
use hires::HiRes;
use mm2_formats::trn::Terrain;
use mm2_formats::{faoe, rs2, scn, tdf, trn, Container};
use mm2_physics::spec::{self, Spec};
use objects::ObjectMesh;
use race::Race;
use renderer::TerrainRenderer;
use ride::Controls;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use web_time::Instant;
use tdf::TrackDef;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

type BoxError = Box<dyn std::error::Error>;

fn main() -> Result<(), BoxError> {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();
    let args: Vec<String> = crate::args().skip(1).collect();
    // The demo the browser page downloaded: its game folder, unpacked.
    #[cfg(target_arch = "wasm32")]
    demo::unpack_in_browser();
    // Started with nothing (a shortcut, a double-click): as the launcher
    // starts it, with its saved settings.
    if args.is_empty() && play_with_launcher_settings() {
        return Ok(());
    }
    // Older graphics cards with no Vulkan driver (or a broken one) draw
    // through OpenGL instead. Without this the GPU's best API is used, and
    // OpenGL only when nothing else works.
    if args.iter().any(|a| a == "--opengl") {
        std::env::set_var("WGPU_BACKEND", "gl");
    }
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
    };
    // "Free graphics": the free pack's art and sound (`--freepack DIR`, or
    // `--freepack=DIR` from a browser page's address).
    if let Some(dir) = flag("--freepack").map(String::as_str).or_else(|| args.iter().find_map(|a| a.strip_prefix("--freepack="))) {
        freepack::set_root(PathBuf::from(dir));
    }
    // The pack beside a cut-down game (the demo), filling in what it lacks.
    if let Some(dir) = flag("--fill-from").map(String::as_str).or_else(|| args.iter().find_map(|a| a.strip_prefix("--fill-from="))) {
        freepack::set_fill(PathBuf::from(dir));
    }
    // The menus' look: the new one (`--new-menus`, or `--new-menus=DIR`
    // for the pack's menus over the original graphics) or the original's.
    if let Some(arg) = args.iter().find(|a| *a == "--new-menus" || a.starts_with("--new-menus=")) {
        let dir = arg.strip_prefix("--new-menus=").filter(|d| !d.is_empty()).map(PathBuf::from);
        freepack::set_menus(true, dir);
    } else if args.iter().any(|a| a == "--original-menus") {
        freepack::set_menus(false, None);
    }
    let args: Vec<String> = {
        // The packs' folders aren't the game's.
        let at = args.iter().position(|a| a == "--freepack");
        let fill = args.iter().position(|a| a == "--fill-from");
        args.iter()
            .enumerate()
            .filter(|(i, _)| at.is_none_or(|at| *i != at + 1) && fill.is_none_or(|f| *i != f + 1))
            .map(|(_, a)| a.clone())
            .collect()
    };
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
    };
    // An always-open free roam server, with no window (`serve.rs`).
    #[cfg(not(target_arch = "wasm32"))]
    if args.iter().any(|a| a == "--serve") {
        let options = serve::Options::from_args(|f| flag(f).cloned())?;
        return serve::run(options);
    }
    // The enhanced graphics, all of them or less those named after `=`.
    if let Some(arg) = args.iter().find(|a| *a == "--enhanced" || a.starts_with("--enhanced=")) {
        let list = arg.strip_prefix("--enhanced").unwrap().trim_start_matches('=');
        graphics::enable_list(list)?;
        eprintln!("enhanced graphics: {:?} (F3 switches to the original look)", graphics::enhanced().unwrap());
    }
    if let Some(dir) = flag("--hires") {
        let pack = HiRes::open(Path::new(dir)).map_err(|e| format!("{dir}: {e}"))?;
        eprintln!("{dir}: {} high-res textures", pack.len());
        hires::set_pack(pack);
    }
    // A track to ride straight away, or the game folder for the menus.
    let path = args.first().filter(|a| !a.starts_with("--")).map(Path::new);
    let is_track =
        path.is_some_and(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("env")));
    // A track ridden straight away: the game whose bikes and sounds it
    // takes (`--game DIR`, for a free track, which lives in the pack's
    // folder), else the one the track is in.
    if is_track {
        let game = flag("--game").map(PathBuf::from).or_else(|| {
            path?.ancestors().find(|d| fs::is_file(d.join("RES").join("MODELS.RES"))).map(Path::to_path_buf)
        });
        if let Some(game) = game {
            freepack::set_game(&game);
        }
    }
    // `--course-builder` with no menus to go behind it (the free pack on
    // its own): the builder by itself, riding straight from it.
    // With the free pack and no game folder to show menus from, the
    // builder is what opens: the pack has no menus of its own yet.
    // With no game files there are no original menus to pick.
    let game_menus = path.is_none() && find_game().is_some_and(|g| fs::is_file(g.join("UI").join("MAINMENU.DAT")));
    if freepack::chosen() && path.is_none() && !freepack::new_menus() && !game_menus {
        freepack::set_menus(true, None);
    }
    let free_only = freepack::chosen() && path.is_none() && !freepack::has_menus() && !game_menus;
    if !is_track && (free_only || args.iter().any(|a| a == "--course-builder")) {
        let has_menus = |g: &Path| fs::is_file(g.join("UI").join("MAINMENU.DAT"));
        let given = path.map(Path::to_path_buf);
        if free_only || (!given.as_deref().is_some_and(has_menus) && (given.is_some() || find_game().is_none())) {
            // No game folder: tracks are baked into the user's own folder.
            let game = given;
            let i = args.iter().position(|a| a == "--course-builder");
            let file = i.and_then(|i| args.get(i + 1)).filter(|a| !a.starts_with("--")).map(PathBuf::from);
            let mut app = App::new(None, None, None);
            app.builder = Some(builder::Builder::new(game, file.as_deref()));
            return run(app);
        }
    }
    if !is_track {
        // The free graphics with no game files: the pack's own menus, with
        // the pack's folder standing in for the game's.
        let free_menus = (freepack::chosen() && path.is_none() && freepack::has_menus()).then(freepack::root).flatten();
        let Some(game) = free_menus.or_else(|| {
            path.map(Path::to_path_buf)
                .or_else(find_game)
                .filter(|g| fs::is_file(g.join("UI").join("MAINMENU.DAT")))
        }) else {
            eprintln!(
                "usage: mm2-viewer [GAME folder] | <TRACK.ENV> [--screenshot out.png] [--size WxH] [--camera x,y,z,yaw,pitch]"
            );
            std::process::exit(2);
        };
        demo::note(&game);
        freepack::set_game(&game);
        if let Some(out) = flag("--screenshot") {
            let clicks = flag("--clicks").map(String::as_str).unwrap_or("");
            return pollster::block_on(menu_screenshot(&game, clicks, Path::new(out), None));
        }
        let mut front =
            frontend::Frontend::new(&game).ok_or("the game's UI/MAINMENU.DAT cannot be read")?;
        update_note::check();
        let builder_at = args.iter().position(|a| a == "--course-builder");
        // `--course-builder [file]`: straight into the Course Builder, with
        // the main menu behind it rather than the intro.
        if builder_at.is_some() {
            front.main_menu();
        } else {
            front.intro();
        }
        let mut app = App::new(None, None, Some(front));
        if let Some(i) = builder_at {
            let file = args.get(i + 1).filter(|a| !a.starts_with("--")).map(PathBuf::from);
            app.open_builder(file.as_deref());
        }
        return run(app);
    }
    let path = path.unwrap();
    // A tape to show at `--at` seconds instead of a race.
    let mut tape = flag("--replay").and_then(|p| replay::Replay::open(Path::new(p)));
    let riders = flag("--riders")
        .and_then(|n| n.parse().ok())
        .or(tape.as_ref().map(|t| t.tape.riders.len()))
        .unwrap_or(RIDERS)
        .clamp(1, mm2_settings::RIDERS_MOST as usize);
    if let Some(v) = flag("--spawn") {
        let n: Vec<f32> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        if let [x, z, h] = n[..] {
            let _ = SPAWN.set((Vec3::new(x, 0.0, z), Vec3::new(h.sin(), 0.0, h.cos())));
        }
    }
    // `--course N`: a Baja or Enduro track's Nth course, else the tape's.
    let course = flag("--course")
        .and_then(|n| n.parse().ok())
        .or(tape.as_ref().map(|t| t.tape.course()))
        .unwrap_or(0);
    let ghost = args.iter().any(|a| a == "--ghost").then_some(1);
    // `--random-gates COUNT:RACE` lays a Baja track's gates out at random.
    let random = flag("--random-gates").and_then(|v| {
        let (n, seed) = v.split_once(':').unwrap_or((v, "0"));
        Some(gates::RandomGates { count: n.parse().ok()?, seed: seed.parse().ok()? })
    });
    let mut track = load_track(path, course, riders, Vec::new(), PLAYER, ghost, random)?;
    if let Some(out) = flag("--screenshot") {
        let (w, h) = flag("--size")
            .and_then(|s| s.split_once('x'))
            .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
            .unwrap_or((1280, 720));
        let mut camera = match flag("--camera") {
            Some(c) => parse_camera(c).ok_or("--camera wants x,y,z,yaw,pitch")?,
            None => renderer::start_camera(&track.terrain),
        };
        let mut time: f32 = flag("--time").and_then(|t| t.parse().ok()).unwrap_or(0.0);
        let laps = flag("--laps")
            .and_then(|n| n.parse().ok())
            .unwrap_or(race::DEFAULT_LAPS);
        // Moto-Tag for this many minutes (`--tag M`), with the ball
        // (`--ball`).
        let tag_flags = flag("--tag")
            .and_then(|m| m.parse::<f32>().ok())
            .map(|m| (m, args.iter().any(|a| a == "--ball")));
        // Start the computer riders from states on a tape of the game's AI
        // (`--ai-check TAPE.csv --from T1,T2,.. --for S --ai-csv OUT`) and
        // ride on with the tape's frame times, to compare.
        if let Some(tape) = flag("--ai-check") {
            let starts: Vec<f32> = flag("--from")
                .map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect())
                .unwrap_or_default();
            let seconds = flag("--for").and_then(|t| t.parse().ok()).unwrap_or(2.0);
            let level = flag("--level").and_then(|l| l.parse::<usize>().ok()).unwrap_or(2);
            let out = flag("--ai-csv").cloned().unwrap_or_else(|| "ai_check.csv".into());
            // `--lanes a,b,..`: each rider's lane, ft, in place of the
            // random one.
            let lanes: Vec<f32> = flag("--lanes")
                .map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect())
                .unwrap_or_default();
            return ai_check(&track, tape, &starts, seconds, level, &lanes, &out);
        }
        // A network race between two copies on this machine or network:
        // `--net host` waits for one player, `--net join:ADDR` joins.
        if let Some(how) = flag("--net") {
            let seconds: f32 = flag("--ride").and_then(|t| t.parse().ok()).unwrap_or(10.0);
            let roam = args.iter().any(|a| a == "--roam");
            return net_test(&track, how, seconds, tag_flags, roam);
        }
        // Race for a while before taking the picture.
        let mut race = None;
        let mut ride_dirt = None;
        if let Some(seconds) = flag("--ride").and_then(|t| t.parse::<f32>().ok()) {
            // `--slot N` starts the player in gate slot N (the original's
            // practice uses the second).
            let slot = flag("--slot").and_then(|t| t.parse().ok()).unwrap_or(PLAYER);
            let mut r = make_race_as(&track, laps, slot).ok_or("this track has no bike to ride")?;
            // `--view N`: presses the camera button N times first.
            for _ in 0..flag("--view").and_then(|t| t.parse::<usize>().ok()).unwrap_or(0) {
                r.next_view();
            }
            // Scripted: the game's own steps, frame by frame; `--live-steps`
            // steps as live play does instead, to try frame rates.
            let live = args.iter().any(|a| a == "--live-steps");
            r.fixed_step = live;
            r.ai_frame = if live { race::AI_FRAME } else { 0.0 };
            let mut dirt = particles::Particles::new();
            let mut ride_tyres = Vec::new();
            // A Stunt Quarry session of this many minutes.
            if let Some(m) = flag("--stunt").and_then(|m| m.parse::<f32>().ok()) {
                r.stunt_event = true;
                r.minutes = Some(m);
            }
            if let Some((m, ball)) = tag_flags {
                r.tag = Some(tag::Tag::new(r.racers.len(), ball, false));
                r.minutes = Some(m);
            }
            // The computer riders' level, 1 to 3 (`--level N`).
            if let Some(l) = flag("--level").and_then(|l| l.parse::<usize>().ok()) {
                r.level = l.clamp(1, 3) - 1;
            }
            // Nationals stunt money, to see the units add up.
            r.purse = Some(mm2_physics::career::Tables::shipped().stunt_purse(2));
            // The computer rides for the player unless told how to steer.
            let steer = flag("--steer").and_then(|t| t.parse().ok());
            r.autopilot = steer.is_none() && r.course.is_some();
            let controls = Controls {
                throttle: 1.0,
                steer: steer.unwrap_or(0.0),
                ..Default::default()
            };
            // Hold the gas back until this race time (`--gas-from 3`).
            let gas_from = flag("--gas-from").and_then(|t| t.parse::<f32>().ok());
            // Or play a key script (`--inputs FILE`) from the gate drop, or
            // from race time `--inputs-at`, through the factory keyboard.
            let script = match flag("--inputs") {
                Some(f) => Some(input::Script::parse(&crate::fs::read_to_string(f)?)),
                None => None,
            };
            // `--practice`: the original's practice has no gate, so the
            // bike is free from the start and `--inputs-at` counts from it.
            let practice = args.iter().any(|a| a == "--practice");
            if practice {
                r.time = r.lead;
                r.practice = true;
            }
            let script_at = flag("--inputs-at").and_then(|t| t.parse::<f32>().ok()).unwrap_or(if practice { 0.0 } else { r.lead })
                + if practice { r.lead } else { 0.0 };
            let key_map = input::KeyMap::factory();
            let (mut steer_keys, mut lean_keys) = (input::KeyAxis::default(), input::KeyAxis::default());
            // `MM2_STATE` is put in once, at the first frame from its time.
            let mut state_done = false;
            // The player's state each frame as CSV (`--csv FILE`), in the
            // comparison runs' columns, timed from the script's start.
            let mut csv = flag("--csv").map(|_| {
                String::from("t,x,y,z,roll,pitch,heading,vx,vy,vz,speed,gear,rpm,crashed,steer,turn,traction,sx,sy,sz,front_down,rear_down,front_squeeze,rear_squeeze,bars,lean,phase,mix,slow,motion\n")
            });
            // Every rider's state each frame (`--ai-csv FILE`), in the
            // columns of the original's AI tapes, timed from the gate drop.
            let mut ai_csv = flag("--ai-csv").map(|_| {
                String::from("t,bike,x,y,z,vx,vy,vz,speed,heading,airborne,crashed,trick,laps,on_course\n")
            });
            let mut chase = (Vec3::ZERO, Vec3::Z);
            let mut air = 0;
            // A ghost race: the second rider is the ghost of the best lap.
            let mut ghost = args.iter().any(|a| a == "--ghost").then(|| {
                r.ghost = Some(1);
                r.lead = 0.0;
                let tape = mm2_formats::vcr::Tape::new(
                    Vec::new(),
                    vec![mm2_formats::vcr::Rider::default()],
                );
                // A saved ghost to ride against (`--ghost-file FILE`).
                let saved = flag("--ghost-file").and_then(|f| mm2_formats::vcr::Tape::parse(&crate::fs::read(f).ok()?).ok());
                replay::Ghost::new(1, tape, saved, 0.0)
            });
            // The player's laps taped as the online ghosts tape them, the
            // fastest written to `--lap-ghost FILE`.
            let mut lap_tapes = flag("--lap-ghost").map(|_| {
                let mut h = mm2_formats::vcr::Tape::new(Vec::new(), vec![mm2_formats::vcr::Rider::default()]);
                h.set_track(path.file_stem().and_then(|s| s.to_str()).unwrap_or_default());
                let sx = path.parent().and_then(|p| p.file_name()).is_some_and(|n| n.eq_ignore_ascii_case("SX"));
                h.set_setting(mm2_formats::vcr::setting::EVENT, if sx { 3 } else { 2 });
                h.set_setting(mm2_formats::vcr::setting::MODE, frontend::GHOST);
                replay::LapTapes::new(h)
            });
            // The ride on tape, written to `--record`.
            let mut rec = flag("--record").map(|_| {
                replay::Recording::new(replay::Heading {
                    track: path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or_default()
                        .to_string(),
                    course: 0,
                    event: 2,
                    mode: 0,
                    laps,
                    random_gates: None,
                    riders: (0..r.racers.len())
                        .map(|i| mm2_formats::vcr::Rider {
                            id: i as u32,
                            computer: i != PLAYER,
                            name: format!("Rider {i}"),
                            bike_model: GRID[i % GRID.len()].0.into(),
                            rider_skin: GRID[i % GRID.len()].1.into(),
                            ..Default::default()
                        })
                        .collect(),
                })
            });
            // Tap trick button 6 twice a second.
            let tricks = args.iter().any(|a| a == "--tricks");
            // Or hold the Taunting key, or the Kahuna Dumpster's, in the air.
            let taunt = args.iter().any(|a| a == "--taunt");
            let kahuna = args.iter().any(|a| a == "--kahuna");
            // The engine sounds alone, as heard from the bike, written
            // as a 22.05 kHz WAV (`--engine-wav FILE`).
            let mut engine_wav = flag("--engine-wav").and_then(|_| {
                let audio = audio::Audio::offline(22050.0);
                let res = sound_res(track.res.as_deref())?;
                let mut sounds = sound::RaceSounds::load(
                    &res,
                    &audio,
                    &[match flag("--engine-class").map(String::as_str) {
                        Some("125") => sound::class(125, false),
                        Some("400") => sound::class(450, true),
                        Some("500") => sound::class(500, false),
                        Some("600") => sound::class(600, true),
                        _ => sound::class(250, false),
                    }],
                    0,
                    frontend::EventKind::Nationals,
                    false,
                    sound::Options::default(),
                );
                sounds.engines_only = true;
                Some((audio, sounds, Vec::<f32>::new(), 0.0f64))
            });
            let mut drive_sound = drive_sound::DriveSound::new();
            // Each frame lasts 1/60 s unless `--fps N` says otherwise. At
            // more than 50 fps the original runs one bike step per frame,
            // as long as that frame.
            let fps = flag("--fps").and_then(|f| f.parse::<f32>().ok()).map_or(1.0 / 60.0, |f| 1.0 / f);
            // Or the frames of a tape of the game (`--frames FILE`: the
            // clock at the end of each frame, counted like `--inputs-at`), so
            // each step is as long as the game's was. A gap over 0.025 s is
            // two frames.
            let shift = if practice { r.lead } else { 0.0 };
            let frames: Vec<f32> = flag("--frames")
                .and_then(|p| crate::fs::read_to_string(p).ok())
                .map(|s| s.split_whitespace().filter_map(|x| x.parse::<f32>().ok()).map(|t| t + shift).collect())
                .unwrap_or_default();
            let mut film = match flag("--film") {
                dir if dir.is_some() || std::env::var("MM2_WRECK_LOG").is_ok() || std::env::var("MM2_FILM_BENCH").is_ok() => {
                    let size = flag("--size")
                        .and_then(|s| s.split_once('x'))
                        .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
                        .unwrap_or((640, 360));
                    let from = flag("--film-from").and_then(|t| t.parse().ok()).unwrap_or(0.0);
                    Some(Film::new(&track, dir.map(PathBuf::from), from, size)?)
                }
                _ => None,
            };
            let mut next = 0;
            let mut elapsed = 0.0;
            let mut k = 0usize;
            let mut driving: Option<drive::Drive> = None;
            while elapsed < seconds {
                while next < frames.len() && frames[next] <= r.time + 1e-4 {
                    next += 1;
                }
                let frame = match frames.get(next) {
                    Some(&t) if t - r.time > 0.025 => (t - r.time) / 2.0,
                    Some(&t) => t - r.time,
                    None => fps,
                };
                elapsed += frame;
                k += 1;
                let flying = r.racers[r.player].bike.airborne();
                r.buttons = [tricks && flying && k % 30 < 15, false];
                r.actions = [kahuna && flying, taunt && flying];
                // Stop for the picture halfway through a trick.
                let me = &r.racers[r.player];
                if tricks && me.stunt.busy() && me.trick_clock > 0.5 {
                    break;
                }
                // Or with the head turned furthest, or the rider let go.
                if me.taunt.is_some_and(|(_, p)| (p - 0.5).abs() > 0.25) {
                    break;
                }
                if let race::Bike::Player(b) = &me.bike {
                    if kahuna && b.crash.is_some_and(|(_, left)| left < 1.2) {
                        break;
                    }
                }
                let mut c = match gas_from {
                    Some(t) if r.time < t => Controls { throttle: 0.0, ..controls },
                    _ => controls,
                };
                if let Some(s) = &script {
                    // The keys held at the frame's end: a tape's clock is read
                    // with the frame's input, so a key logged before a frame's
                    // time already counts in that frame (a keyed
                    // ride: the bars move in the first frame after the key).
                    let codes = s.held(r.time + frame - script_at);
                    let held = |a| key_map.held_code(a, &codes);
                    let dt = frame;
                    c = Controls {
                        throttle: held(input::ACCELERATE) as i32 as f32,
                        brake: held(input::BRAKE) as i32 as f32,
                        steer: steer_keys.update(held(input::STEER_LEFT), held(input::STEER_RIGHT), dt),
                        lean: lean_keys.update(held(input::LEAN_BACK), held(input::LEAN_FORWARD), dt),
                    };
                    r.buttons = [held(input::STUNTS_LOW), held(input::STUNTS_HIGH)];
                    r.actions = [held(input::KAHUNA), held(input::TAUNT)];
                }
                // A state from the running game to go on from (`MM2_STATE`:
                // t|com|velocity|right|up|forward|world spin|squeeze front,rear).
                if let Ok(state) = std::env::var("MM2_STATE") {
                    let f: Vec<Vec<f32>> = state
                        .split('|')
                        .map(|p| p.split(',').filter_map(|x| x.trim().parse().ok()).collect())
                        .collect();
                    let v = |i: usize| Vec3::new(f[i][0], f[i][1], f[i][2]);
                    let now = r.time - script_at;
                    if now >= f[0][0] && !state_done {
                        state_done = true;
                        if let race::Bike::Player(p) = &mut r.racers[r.player].bike {
                            // Going on from the game's state, uncrashed.
                            p.crash = None;
                            p.wreck = None;
                            p.set_state(v(1), v(2), glam::Mat3::from_cols(v(3), v(4), v(5)), v(6), [f[7][0], f[7][1]]);
                        }
                    }
                }
                track.run_traffic(frame);
                // `--drive-at T` (`--drive-craft T` for the nearest craft):
                // into a vehicle T s into the script, and drive it with it.
                for (flag_name, craft) in [("--drive-at", false), ("--drive-craft", true)] {
                    let at = flag(flag_name).and_then(|t| t.parse::<f32>().ok());
                    if driving.is_none() && at.is_some_and(|t| r.time - script_at >= t) {
                        driving = pick_vehicle(&r, &mut track, craft);
                        eprintln!("drive: {:?}", driving.as_ref().map(|d| (&d.name, d.kind)));
                        if let Some(drive::Seat::Air(o)) = driving.as_ref().map(|d| d.seat) {
                            pin_craft(track.objects.animated.get_mut(o), driving.as_ref());
                        }
                    }
                }
                let gas = c.throttle;
                let c = match &mut driving {
                    Some(d) => {
                        d.update(c, &track.terrain, &track.world, &mut track.traffic, frame);
                        if let drive::Seat::Air(o) = d.seat {
                            pin_craft(track.objects.animated.get_mut(o), Some(d));
                        }
                        Controls { brake: 1.0, ..Controls::default() }
                    }
                    None => c,
                };
                let stepped = web_time::Instant::now();
                chase = r.update(&track.ground(), c, frame);
                if let Some(f) = &mut film {
                    f.step = stepped.elapsed().as_secs_f64();
                }
                if let Some(d) = &mut driving {
                    chase = d.camera(&track.terrain, frame);
                    if k % 30 == 0 {
                        eprintln!("drive t {:.1} at {:.0?} speed {:.0} up {:.2?} fwd {:.2?} springs {:.2?}", r.time - script_at, d.position, d.speed, d.up, d.forward, d.springs);
                    }
                }
                if let Some(f) = &mut film {
                    f.drive = driving.clone();
                    let clock = r.time - script_at;
                    f.frame(&mut r, &track, chase, clock)?;
                }
                if let Some((audio, sounds, out, owed)) = &mut engine_wav {
                    let bike = &r.racers[r.player].bike;
                    let listener = match &driving {
                        // Heard from the chase camera, going with it.
                        Some(d) => sound::Listener { velocity: d.velocity(), ..listener(&r, chase.0, chase.1) },
                        None => sound::Listener {
                            eye: bike.position(),
                            right: Vec3::X,
                            velocity: bike.velocity(),
                        },
                    };
                    sounds.hush = driving.is_some().then_some(r.player);
                    sounds.update(audio, &r, &listener, frame);
                    drive_sound.update(audio, 1.0, driving.as_ref(), gas, &listener, frame);
                    *owed += 22050.0 / 60.0;
                    let n = *owed as usize;
                    *owed -= n as f64;
                    out.extend(audio.render(n).chunks(2).map(|f| (f[0] + f[1]) / 2.0));
                }
                throw_dirt(&r, &track.ground(), &mut dirt, frame);
                splash_water(&r, &track.water, &mut dirt, frame);
                ride_tyres.push((tyre_places(&r, &track.ground()), frame));
                // A film shows the dust and dirt too, fed as it goes.
                if let Some(f) = &mut film {
                    f.renderer.tyres(ride_tyres.last().map_or(&[][..], |t| &t.0[..]), frame);
                    throw_dirt(&r, &track.ground(), &mut f.renderer.particles, frame);
                }
                if let Some(out) = &mut ai_csv {
                    use std::fmt::Write as _;
                    for (i, rc) in r.racers.iter().enumerate() {
                        let b = &rc.bike;
                        let (p, v, f) = (b.position(), b.velocity(), b.forward());
                        let _ = writeln!(
                            out,
                            "{:.4},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.4},{},{},{},{},{}",
                            r.clock(), i, p.x, p.y, p.z, v.x, v.y, v.z, v.length(), f.x.atan2(f.z),
                            b.airborne() as u8, b.crashed() as u8,
                            rc.stunt.trick.map_or(-1, |t| t as i32), rc.laps, rc.on_course() as u8
                        );
                    }
                }
                if let (Some(out), race::Bike::Player(p)) = (&mut csv, &r.racers[r.player].bike) {
                    use std::fmt::Write as _;
                    let (pos, v, m) = (p.position, p.velocity, p.rotation());
                    let f = m.z_axis;
                    let _ = writeln!(
                        out,
                        "{:.4},{:.3},{:.3},{:.3},{:.4},{:.4},{:.4},{:.3},{:.3},{:.3},{:.3},{},{:.0},{},{:.4},{:.3},{:.3},{:.3},{:.3},{:.3},{},{},{:.3},{:.3}",
                        r.time - script_at,
                        pos.x, pos.y, pos.z,
                        -m.x_axis.y.clamp(-1.0, 1.0).asin(),
                        f.y.clamp(-1.0, 1.0).asin(),
                        f.x.atan2(f.z),
                        v.x, v.y, v.z, v.length(),
                        p.drivetrain.gear, p.drivetrain.rpm, p.crash.is_some() as u8,
                        p.steer, p.turn_rate, p.traction,
                        p.spin.x, p.spin.y, p.spin.z,
                        p.tyres_down()[0] as u8, p.tyres_down()[1] as u8, p.squeezes()[0], p.squeezes()[1]
                    );
                    // The rider's pose as the original's tapes have it:
                    // f_504, f_508, f_65c, f_654, f_660 and the first row.
                    out.pop();
                    let mix = &r.racers[r.player].mixes.0;
                    let (phase, w, g, name) = match mix.as_slice() {
                        [a, b, ..] => (a.1, b.2 / (a.2 + b.2).max(1e-6), a.2 + b.2, a.0),
                        [a] => (a.1, 0.0, 1.0, a.0),
                        [] => (0.0, 0.0, 0.0, ""),
                    };
                    let _ = writeln!(out, ",{:.3},{:.3},{:.3},{:.3},{:.3},{}", -p.rider.0, p.rider.1, phase, w, g, name);
                }
                if std::env::var("MM2_TRACE").is_ok() && (k % 6 == 0 || std::env::var("MM2_TRACE").as_deref() == Ok("all")) {
                    let b = &r.racers[r.player].bike;
                    let g = track.ground();
                    eprintln!(
                        "t {:.1} pos {:.0?} vel {:.0?} look,spin,lean {:.2?} crashed {} wall {:?}",
                        k as f32 / 60.0,
                        b.position(),
                        b.velocity(),
                        match b { race::Bike::Player(p) => (p.forward(), p.spin, p.lean), _ => (Vec3::Z, Vec3::ZERO, 0.0) },
                        b.crashed(),
                        mm2_physics::Ground::solid(&g, b.position(), 1.5).map(|h| h.depth)
                    );
                    if let race::Bike::Player(p) = b {
                        let at = p.position;
                        eprintln!("    ground {:.2?} tyres {:?}", mm2_physics::Ground::terrain(&g, at.x, at.z), p.tyres_down());
                    }
                }
                if let Some(rec) = &mut rec {
                    rec.tick(&r, frame);
                }
                if let Some(g) = &mut ghost {
                    g.tick(&mut r, &track.terrain, frame);
                }
                if let Some(t) = &mut lap_tapes {
                    t.tick(&r, frame);
                }
                air += r.racers[r.player].bike.airborne() as usize;
            }
            if let (Some((_, _, out, _)), Some(path)) = (&engine_wav, flag("--engine-wav")) {
                let mut wav = Vec::new();
                let bytes = out.len() as u32 * 2;
                wav.extend(b"RIFF");
                wav.extend((36 + bytes).to_le_bytes());
                wav.extend(b"WAVEfmt ");
                wav.extend(16u32.to_le_bytes());
                wav.extend([1u8, 0, 1, 0]);
                wav.extend(22050u32.to_le_bytes());
                wav.extend(44100u32.to_le_bytes());
                wav.extend([2u8, 0, 16, 0]);
                wav.extend(b"data");
                wav.extend(bytes.to_le_bytes());
                for s in out {
                    wav.extend(((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
                }
                crate::fs::write(path, wav)?;
            }
            if let Some(g) = &ghost {
                let best = g.best().map(|t| t.length);
                let shown = !r.racers[1].bike.place().x_axis.abs().max_element().eq(&0.0);
                eprintln!("ghost: best lap {best:?}, showing {shown}");
            }
            if let (Some(mut t), Some(out)) = (lap_tapes, flag("--lap-ghost")) {
                let me = &r.racers[r.player];
                let fastest = me.lap_times.iter().enumerate().skip(1).min_by(|a, b| a.1.total_cmp(b.1)).map(|(i, _)| i);
                match fastest.and_then(|i| t.take(i)) {
                    Some(tape) => {
                        crate::fs::write(out, tape.write())?;
                        eprintln!("lap ghost: lap {:?} of {:.2}s, {} records, to {out}", fastest, tape.length, tape.records.len());
                    }
                    None => eprintln!("lap ghost: no lap taped"),
                }
            }
            if let (Some(text), Some(out)) = (ai_csv, flag("--ai-csv")) {
                crate::fs::write(out, text)?;
            }
            if let (Some(text), Some(out)) = (csv, flag("--csv")) {
                crate::fs::write(out, text)?;
            }
            if let (Some(rec), Some(out)) = (rec, flag("--record")) {
                crate::fs::write(out, rec.finish().write())?;
                eprintln!("wrote the tape to {out}");
            }
            {
                let me = &r.racers[r.player];
                eprintln!("laps {:?}, off the course {:?}", me.lap_times, me.lap_off);
            }
            let b = &r.racers[r.player].bike;
            let at = b.position();
            eprintln!(
                "rode {seconds}s: at {:.0},{:.0},{:.0}, {:.0} units/s, {:.1}s in the air{}, computer crashes {}",
                at.x,
                at.y,
                at.z,
                b.speed(),
                air as f32 / 60.0,
                if b.crashed() {
                    ", crashed"
                } else if b.airborne() {
                    ", flying"
                } else {
                    ""
                },
                race::AI_CRASHES.load(std::sync::atomic::Ordering::Relaxed),
            );
            eprintln!("{}", r.status());
            if let Some(b) = r.tag.as_ref().and_then(|t| t.ball) {
                let p = b.position;
                eprintln!("ball at {:.1},{:.1},{:.1}", p.x, p.y, p.z);
            }
            let me = &r.racers[r.player];
            eprintln!(
                "stunts: {:.0} points, best {:.0}, now {:?}",
                me.stunt_points, me.best_stunt, me.stunt.trick
            );
            for (i, racer) in r.racers.iter().enumerate() {
                let laps: Vec<_> = racer.lap_times.iter().map(|&t| race::time(t)).collect();
                let p = racer.bike.position();
                eprintln!(
                    "  rider {i} at {:.0},{:.0}: lap {} {:?}, gates {}, stunts {:.0}, units {:.2}/{:.2}/{:.2}{}",
                    p.x,
                    p.z,
                    racer.laps,
                    laps,
                    racer.timing.gates,
                    racer.stunt_points,
                    racer.repair,
                    racer.medical,
                    racer.stunt_units,
                    if racer.bike.crashed() {
                        ", crashed"
                    } else {
                        ""
                    }
                );
            }
            if flag("--camera").is_none() {
                camera =
                    Camera::looking_at(renderer::from_game(chase.0), renderer::from_game(chase.1));
                camera.roll = r.camera_roll();
            }
            // A close look at the player's bike from in front.
            // The whole start from up behind it (`--grid-view`).
            if args.iter().any(|a| a == "--grid-view") {
                let up = Vec3::Y;
                let left = up.cross(b.forward());
                let eye = at - b.forward() * 45.0 + up * 30.0 + left * 25.0;
                camera = Camera::looking_at(renderer::from_game(eye), renderer::from_game(at + b.forward() * 30.0 + left * 25.0));
            }
            // And from the side (`--side`, the right).
            if args.iter().any(|a| a == "--side") {
                let up = Vec3::Y;
                let eye = at + b.forward().cross(up) * 8.0 + up * 3.0;
                camera = Camera::looking_at(renderer::from_game(eye), renderer::from_game(at + up * 2.5));
            }
            if args.iter().any(|a| a == "--front") {
                let up = Vec3::Y;
                let eye = at + b.forward() * 7.0 + up * 3.5 + b.forward().cross(up) * 2.0;
                camera = Camera::looking_at(
                    renderer::from_game(eye),
                    renderer::from_game(at + up * 2.5),
                );
            }
            // From behind another rider (`--watch i`).
            if let Some(w) = flag("--watch").and_then(|i| i.parse::<usize>().ok()) {
                let b = &r.racers[w.min(r.racers.len() - 1)].bike;
                let (at, f) = (b.position(), b.forward());
                let eye = at - f * 16.0 + Vec3::Y * 7.0;
                camera = Camera::looking_at(
                    renderer::from_game(eye),
                    renderer::from_game(at + Vec3::Y * 3.0),
                );
            }
            // The podium `--podium T` s in, for the track's kind of event
            // (`--roll` the stage's coin, 0 to 1).
            if let (Some(t), Some(stand)) = (flag("--podium").and_then(|t| t.parse::<f32>().ok()), &track.podium) {
                use frontend::EventKind::*;
                let folder = path.to_string_lossy().to_ascii_uppercase();
                let event = [("/SX/", Supercross), ("NATIONAL", Nationals), ("TAG", Tag), ("BAJA", Baja), ("ENDURO", Enduro)]
                    .into_iter()
                    .find(|(name, _)| folder.contains(name))
                    .map_or(Stunt, |(_, e)| e);
                let roll = flag("--roll").and_then(|t| t.parse::<f32>().ok()).unwrap_or(0.7);
                let p = podium_for(&track, stand, event, roll);
                r.podium = Some((p, r.time));
                r.time += t;
                let (eye, target) = p.camera(t);
                camera = Camera::looking_at(renderer::from_game(eye), renderer::from_game(target));
            }
            time = r.time;
            // A stunt score as if just scored: base,multiplier,seconds ago.
            if let Some(p) = flag("--popup") {
                let v: Vec<f32> = p.split(',').filter_map(|v| v.parse().ok()).collect();
                if let [base, m, age] = v[..] {
                    r.popup = Some((r.time - age, base, m));
                }
            }
            // The Race Info page after this race, instead of the track.
            if let (true, Some((ui, _))) = (args.iter().any(|a| a == "--results"), track.hud()) {
                let game = track.res.as_deref().and_then(Path::parent).unwrap();
                let lines = results(&r, &ui);
                return pollster::block_on(menu_screenshot(game, "", Path::new(out), Some(lines)));
            }
            race = Some(r);
            ride_dirt = Some((dirt, ride_tyres));
        }
        if let (Some(rp), None) = (&mut tape, &race) {
            let mut r = make_race(&track, laps).ok_or("this track has no bike to ride")?;
            rp.time = flag("--at")
                .and_then(|t| t.parse().ok())
                .unwrap_or(rp.start());
            rp.follow = flag("--follow").and_then(|t| t.parse().ok()).unwrap_or(0);
            for _ in 0..flag("--view").and_then(|t| t.parse::<usize>().ok()).unwrap_or(0) {
                r.next_view();
            }
            // From a little before, so the chase camera has settled.
            let at = rp.time;
            rp.time = (at - 1.0).max(rp.start());
            r.time = rp.time;
            rp.show(&mut r, &track.terrain);
            while rp.time < at {
                rp.time = (rp.time + 1.0 / 60.0).min(at);
                chase_at(&mut camera, rp.show(&mut r, &track.terrain));
            }
            camera.fov = r.fov();
            time = r.time;
            race = Some(r);
        }
        return pollster::block_on(screenshot(
            &track,
            race.as_ref(),
            ride_dirt,
            tape.as_mut(),
            &camera,
            time,
            Path::new(out),
            w,
            h,
        ));
    }
    let laps = flag("--laps")
        .and_then(|n| n.parse().ok())
        .unwrap_or(race::DEFAULT_LAPS);
    let race = make_race(&track, laps);
    run(App::new(Some(track), race, None))
}

/// The command line; in a browser, the page address's options, so that
/// `index.html?enhanced` is `--enhanced`.
fn args() -> std::vec::IntoIter<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let query = web_sys::window().and_then(|w| w.location().search().ok()).unwrap_or_default();
        let options = query.trim_start_matches('?').split('&').filter(|o| !o.is_empty());
        let args: Vec<String> = std::iter::once("mm2-viewer".to_string())
            .chain(options.map(|o| format!("--{}", js_sys::decode_uri_component(o).map(String::from).unwrap_or(o.to_string()))))
            .collect();
        args.into_iter()
    }
    #[cfg(not(target_arch = "wasm32"))]
    std::env::args().collect::<Vec<_>>().into_iter()
}

#[cfg(not(target_arch = "wasm32"))]
fn run(mut app: App) -> Result<(), BoxError> {
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}

/// In a browser the page's own frame loop runs the game, after `main`
/// has returned.
#[cfg(target_arch = "wasm32")]
fn run(app: App) -> Result<(), BoxError> {
    use winit::platform::web::EventLoopExtWebSys;
    EventLoop::new()?.spawn_app(app);
    Ok(())
}

/// The size the game draws its picture at in a window of `w`×`h`: the
/// window's own, or with `--max-pixels=N` (the browser's light mode for
/// phones) no more than N pixels, the same shape, which the browser then
/// stretches over the screen. A phone's sharp screen otherwise has its
/// graphics chip fill three million pixels or more every frame.
fn drawn_size(w: u32, h: u32) -> (u32, u32) {
    let (w, h) = most_drawn(w, h);
    // In a browser, while racing: dynres::Picture's share of it.
    let k = dynres::picture();
    if k >= 1.0 {
        return (w, h);
    }
    (((w as f32 * k).round() as u32).max(1), ((h as f32 * k).round() as u32).max(1))
}

/// The most the picture is drawn at in a window of `w`×`h` (`--max-pixels`).
fn most_drawn(w: u32, h: u32) -> (u32, u32) {
    static MOST: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();
    let most = *MOST.get_or_init(|| {
        crate::args()
            .find_map(|a| a.strip_prefix("--max-pixels=").and_then(|n| n.parse::<u64>().ok()))
            .filter(|&n| n >= 320 * 240)
    });
    fit_pixels(w.max(1), h.max(1), most)
}

fn fit_pixels(w: u32, h: u32, most: Option<u64>) -> (u32, u32) {
    match most {
        Some(most) if w as u64 * h as u64 > most => {
            let k = (most as f64 / (w as f64 * h as f64)).sqrt();
            (((w as f64 * k) as u32).max(1), ((h as f64 * k) as u32).max(1))
        }
        _ => (w, h),
    }
}

/// At the screen's refresh, or as fast as the machine can with the VSync
/// tweak off (`--no-vsync`). Either way the bike steps once a frame, as
/// the game does.
fn present_mode() -> wgpu::PresentMode {
    match tweaks::get().vsync {
        true => wgpu::PresentMode::AutoVsync,
        false => wgpu::PresentMode::AutoNoVsync,
    }
}

/// The full screen asked for: the screen's mode `--screen WxH` names
/// (its fastest refresh, most colours), or the screen as it is when it
/// has no such mode or none was named.
fn full_screen(event_loop: &ActiveEventLoop) -> winit::window::Fullscreen {
    let args: Vec<String> = crate::args().collect();
    let asked = args
        .iter()
        .position(|a| a == "--screen")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| mm2_settings::parse_size(v))
        .filter(|z| z.0 > 0 && z.1 > 0);
    let Some((w, h)) = asked else {
        return winit::window::Fullscreen::Borderless(None);
    };
    let monitor = event_loop.primary_monitor().or_else(|| event_loop.available_monitors().next());
    let mode = monitor.into_iter().flat_map(|m| m.video_modes()).filter(|m| m.size() == winit::dpi::PhysicalSize::new(w, h)).max_by_key(|m| (m.refresh_rate_millihertz(), m.bit_depth()));
    match mode {
        Some(m) => {
            eprintln!("display: full screen at {w}x{h}, {:.0} Hz", m.refresh_rate_millihertz() as f64 / 1000.0);
            winit::window::Fullscreen::Exclusive(m)
        }
        None => {
            eprintln!("display: the screen has no {w}x{h} mode; full screen at its own");
            winit::window::Fullscreen::Borderless(None)
        }
    }
}

/// `--window WxH`: the window's size, 1024 × 768 unless given.
fn window_size() -> winit::dpi::LogicalSize<f64> {
    let args: Vec<String> = crate::args().collect();
    args.iter()
        .position(|a| a == "--window")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.split_once('x'))
        .and_then(|(w, h)| Some(winit::dpi::LogicalSize::new(w.parse().ok()?, h.parse().ok()?)))
        .unwrap_or(winit::dpi::LogicalSize::new(1024.0, 768.0))
}

/// The game folder: `GAME` in or above the folder the port starts in, or
/// the copy the launcher made in the user folder.
/// Starts the game again as the launcher does, with the settings it saved,
/// from the game folder and with the error output in `last-run.log`.
/// With none saved, or the game folder gone, and no game to be found
/// here either, opens the launcher beside the game to set it up. Whether
/// either started.
fn play_with_launcher_settings() -> bool {
    use std::process::{Command, Stdio};
    let Ok(me) = std::env::current_exe() else {
        return false;
    };
    let settings = mm2_settings::Settings::load();
    if let Some(game) = settings.game.as_ref().filter(|g| g.join("UI").join("MAINMENU.DAT").is_file()) {
        let log = crate::fs::File::create(mm2_settings::user_dir().join("last-run.log"));
        return Command::new(&me)
            .args(settings.args())
            .current_dir(game)
            .stdout(Stdio::null())
            .stderr(log.map(Stdio::from).unwrap_or(Stdio::null()))
            .spawn()
            .is_ok();
    }
    if find_game().is_some() {
        return false;
    }
    let launcher = me.with_file_name(format!("mm2-launcher{}", std::env::consts::EXE_SUFFIX));
    launcher.is_file() && Command::new(launcher).spawn().is_ok()
}

fn find_game() -> Option<std::path::PathBuf> {
    // In a browser, the copy the player picked on the page.
    if cfg!(target_arch = "wasm32") {
        return Some(std::path::PathBuf::from("/game"));
    }
    let here = std::env::current_dir().ok()?;
    here.ancestors()
        .flat_map(|d| [d.join("GAME"), d.to_path_buf()])
        .chain([profile::user_dir().join("GAME")])
        .find(|g| fs::is_file(g.join("UI").join("MAINMENU.DAT")))
}

/// A track's terrain and the objects its scene places.
pub struct Track {
    terrain: Terrain,
    /// The terrain textures to draw: the game's, or a high-res pack's.
    terrain_textures: Vec<hires::Image>,
    /// The solid objects, for the player's bike.
    world: mm2_physics::collision::World,
    /// The player's bike.
    spec: Spec,
    objects: ObjectMesh,
    start: Start,
    course: Option<TrackDef>,
    /// The bikes on the start line, as animated objects; each one's rider
    /// comes next.
    grid: Vec<usize>,
    /// The start gate's animated object.
    gate: Option<usize>,
    /// The Free Pack's start gate (a free supercross track's), which drops
    /// with the race as the original's does.
    free_gate: Option<usize>,
    /// The flag girl and the 30-second man.
    people: Finish,
    /// The podium and its riders, for after the race.
    podium: Option<Stand>,
    /// The game's `RES` folder, for the race's sounds.
    res: Option<std::path::PathBuf>,
    /// The track's name, shown as the race begins.
    name: String,
    /// A stadium track (TERAFORM/SX).
    supercross: bool,
    /// The lamps the track's `.lights` file lists, which light it at
    /// night (crate::night).
    night_lights: Option<Vec<night::Lamp>>,
    /// A desert track (TERAFORM/BAJA), where the enhanced drawing's rain
    /// is overcast instead.
    desert: bool,
    /// The checkpoint gates of a Baja or Enduro course, in order.
    gates: Vec<mm2_physics::race::Line>,
    /// Their arches, as animated objects, and where they stand.
    arches: Vec<(usize, Mat4)>,
    /// The player's trick motions.
    tricks: Vec<Option<race::TrickMotion>>,
    /// The stunt score's models.
    popup: Option<popup::Popup>,
    /// Moto-Tag's IT sign, beach ball and the player's own sign, as
    /// animated objects.
    tag: Option<[usize; 3]>,
    /// Visual Cues: the arrow and the runway lights.
    cues: Option<cues::Cues>,
    /// The 3D countdown numbers.
    countdown: Option<countdown::Countdown>,
    /// The sky box.
    sky: Option<(mm2_formats::cub::Cube, hires::SkyImages)>,
    /// The fog's colour, which the screen is cleared to.
    fog: Option<[f32; 3]>,
    /// The scene's fog Visibility and Haziness, when it has fog: only a
    /// scene with a sky box does (0x4e2d70).
    haze: Option<(f32, f32)>,
    /// The way the scene's directional light travels, which the player's
    /// shadow falls along.
    sun: Vec3,
    /// The scene's lights.
    lights: renderer::SceneLights,
    /// The terrain's detail texture (0x50b140).
    detail: Option<mm2_formats::tga::Image>,
    /// A free track's fine ground (fine_ground.rs).
    ground_map: Option<fine_ground::GroundMap>,
    /// The dirt and dust sheet (`ParticleTextureMapName`).
    particle_sheet: Option<mm2_formats::tga::Image>,
    /// The scene's scripted vehicles.
    traffic: traffic::Traffic,
    /// How each terrain surface type grips.
    zones: [mm2_formats::scn::Zone; 8],
    /// The scene has a crowd (`[Sounds] CrowdPresent`).
    crowd: bool,
    /// The enhanced drawing's spectators (game coordinates) and which way
    /// each looks.
    spectators: Vec<(Vec3, Vec3)>,
    /// Rivers' water (a builder track's `.wtr`), game coordinates.
    water: Vec<mm2_course::nature::Water>,
    /// The scene's ambient sounds and what each hangs on.
    sounds: Vec<(scn::SoundSpec, sound::Site)>,
    /// The craft in its sky that can be flown (crate::drive).
    aircraft: Vec<drive::Craft>,
    /// Free mode: a Free Pack track, or any track with the free graphics
    /// (where the out-of-bounds whistle plays).
    free: bool,
}

impl Track {
    /// The floor the course runs over, for the lights at night.
    fn floor(&self) -> night::Floor {
        self.course
            .as_ref()
            .and_then(|c| night::Floor::round_course(c.nodes.iter().flat_map(|n| n.sections.iter()).flat_map(|s| [s.center, s.edges[0], s.edges[1]]), &self.terrain))
            .unwrap_or_else(|| night::Floor::guess(&self.terrain))
    }

    /// Where a scene sound's site is at `time` on the race clock.
    fn site(&self, site: sound::Site, time: f32) -> Option<Vec3> {
        match site {
            sound::Site::Fixed(p) => Some(p),
            sound::Site::Object(o, frame) => self.objects.animated.get(o).map(|m| m.frame_position(frame, time)),
            sound::Site::Car(c) => self.traffic.cars.get(c).map(|c| c.vehicle.position),
        }
    }

    /// The in-race display, when the game's files are found.
    fn hud(&self) -> Option<(ui::Ui, hud::Hud)> {
        let game = self.res.as_deref()?.parent()?;
        let mut hud = hud::Hud::default();
        hud.say(&self.name);
        Some((ui::Ui::new(game), hud))
    }

    /// Drives the scene's vehicles on by `dt`.
    fn run_traffic(&mut self, dt: f32) {
        self.traffic.update(dt, &self.terrain, &mut self.world);
    }

    fn ground(&self) -> TrackGround<'_> {
        TrackGround {
            terrain: &self.terrain,
            world: &self.world,
            zones: &self.zones,
            edge: !self.supercross,
        }
    }
}

/// A race on `track` with the bikes on its start line.
fn make_race(track: &Track, laps: u32) -> Option<Race> {
    make_race_as(track, laps, PLAYER)
}

/// A race with the player on bike `player`.
/// See `--ai-check`.
fn ai_check(
    track: &Track,
    tape: &str,
    starts: &[f32],
    seconds: f32,
    level: usize,
    lanes: &[f32],
    out: &str,
) -> Result<(), BoxError> {
    use std::fmt::Write as _;
    let text = crate::fs::read_to_string(tape)?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap_or("").split(',').collect();
    let col = |n: &str| head.iter().position(|h| *h == n).unwrap_or(0);
    let (ct, cb) = (col("t"), col("bike"));
    let cols = ["x", "y", "z", "vx", "vy", "vz", "roll", "pitch", "heading", "spin_x", "spin_y", "spin_z"].map(col);
    // Rows by bike: time and the twelve values.
    let mut rows: Vec<Vec<(f32, [f32; 12])>> = Vec::new();
    for line in lines {
        let f: Vec<&str> = line.split(',').collect();
        let (Ok(t), Ok(b)) = (f[ct].parse::<f32>(), f[cb].parse::<usize>()) else { continue };
        let v = cols.map(|c| f[c].parse::<f32>().unwrap_or(0.0));
        if rows.len() <= b {
            rows.resize(b + 1, Vec::new());
        }
        rows[b].push((t, v));
    }
    let mut text = String::from("start,t,bike,x,y,z,speed,crashed,airborne,trick,vx,vy,vz,fy\n");
    for &start in starts {
        let mut r = make_race_as(track, race::DEFAULT_LAPS, PLAYER).ok_or("no race")?;
        // Scripted: the game's own steps, frame by frame.
        r.fixed_step = false;
        r.ai_frame = 0.0;
        r.level = level.clamp(1, 3) - 1;
        r.time = r.lead + start;
        let k0 = rows[1].iter().position(|(t, _)| *t >= start).ok_or("start past the tape")?;
        for (i, racer) in r.racers.iter_mut().enumerate() {
            let Some(&(_, v)) = rows.get(i).and_then(|b| b.get(k0)) else { continue };
            let m = replay::rotation([v[6], v[7], v[8]]);
            racer.set_computer(
                Vec3::new(v[0], v[1], v[2]),
                Vec3::new(v[3], v[4], v[5]),
                m.z_axis,
                m.y_axis,
                Vec3::new(v[9], v[10], v[11]),
            );
            if let (Some(&lane), race::Bike::Computer(c)) = (lanes.get(i), &mut racer.bike) {
                c.rider.lane = lane;
            }
        }
        let mut k = k0;
        while k + 1 < rows[1].len() && rows[1][k + 1].0 <= start + seconds {
            let dt = rows[1][k + 1].0 - rows[1][k].0;
            k += 1;
            if dt <= 0.0 {
                continue;
            }
            r.update(&track.ground(), Controls::default(), dt);
            for (i, racer) in r.racers.iter().enumerate() {
                let b = &racer.bike;
                let p = b.position();
                let _ = writeln!(
                    text,
                    "{start},{:.4},{i},{:.3},{:.3},{:.3},{:.3},{},{},{},{:.3},{:.3},{:.3},{:.4}",
                    rows[1][k].0,
                    p.x,
                    p.y,
                    p.z,
                    b.velocity().length(),
                    b.crashed() as u8,
                    b.airborne() as u8,
                    racer.stunt.busy() as u8,
                    b.velocity().x,
                    b.velocity().y,
                    b.velocity().z,
                    b.place().z_axis.y
                );
            }
        }
    }
    crate::fs::write(out, text)?;
    Ok(())
}

fn make_race_as(track: &Track, laps: u32, player: usize) -> Option<Race> {
    let mut race = make_race_on(track, laps, player)?;
    race.tricks = track.tricks.clone();
    Some(race)
}

fn make_race_on(track: &Track, laps: u32, player: usize) -> Option<Race> {
    let grid: Vec<_> = track
        .grid
        .iter()
        .map(|&o| (o, track.objects.animated[o].position()))
        .collect();
    // A `--spawn` heading replaces the start line's.
    let direction = SPAWN.get().map_or(track.start.direction, |&(_, way)| way);
    Race::new(
        &track.ground(),
        &grid,
        player,
        direction,
        track.course.as_ref(),
        laps,
        track.spec.clone(),
        track.gates.clone(),
    )
    .map(|mut race| {
        race.supercross = track.supercross;
        // With the 30-second man out, the countdown is his 8.2 s walk.
        if track.people.man.is_some() {
            race.lead = finish::MAN_LEAD;
        }
        race
    })
}

/// Moves the bikes and riders, and holds the gate until it drops.
/// The camera's lens on the 640×480 screen, in pixels (our 60° view).
const FOCAL: f32 = 415.69;

/// Where each bike's tyres touch the ground, game coordinates (none for a
/// tyre in the air), for the enhanced drawing's ruts.
/// A computer rider's tyres, front and back, where its bike is drawn
/// (between its frames, `race::Computer::drawn_place`) rather than where
/// its last frame left it, so the dust and the tracks come off the bike
/// on the screen and not up to a frame ahead of it.
fn drawn_ends(place: Mat4) -> [Vec3; 2] {
    let (at, look) = (place.w_axis.truncate(), place.z_axis.truncate().normalize_or(Vec3::Z));
    [at + look * 2.5, at - look * 2.5]
}

fn tyre_places(race: &Race, ground: &TrackGround) -> Vec<([Option<Vec3>; 2], dust::Kick)> {
    // Whether the ground here raises dust, as the game's own trail asks.
    let dusty = |at: Option<(Vec3, bool)>| at.is_some_and(|(at, on)| if on { ground.zones[7].dust } else { ground.zone(at).dust });
    // And whether it is dirt that flies off a spinning tyre.
    let dirt = |at: Option<Vec3>| at.is_some_and(|at| ground.zone(at).dirt);
    race.racers
        .iter()
        .map(|r| match &r.bike {
            race::Bike::Player(b) => {
                let at = b.tyre_contacts().map(|t| t.filter(|_| b.crash.is_none()));
                let kick = dust::Kick {
                    dusty: dusty(at[1]) || dusty(at[0]),
                    spin: b.roost().1,
                    dirt: at[1].is_some_and(|(p, on)| !on && dirt(Some(p))),
                };
                (at.map(|t| t.map(|(at, _)| at)), kick)
            }
            race::Bike::Computer(c) => match &c.wreck {
                Some(_) => ([None; 2], dust::Kick::default()),
                None => {
                    let rd = &c.rider;
                    let ends = drawn_ends(r.bike.place());
                    let at = [0, 1].map(|k| rd.down[k].then_some(ends[k]));
                    let kick = dust::Kick {
                        dusty: at.iter().flatten().any(|&p| dusty(Some((p, false)))),
                        spin: 0.0,
                        dirt: dirt(at[1]),
                    };
                    (at, kick)
                }
            },
            race::Bike::Taped(_) => {
                let b = &r.bike;
                let down = !b.airborne() && !b.crashed();
                let at = [down.then(|| b.position() + b.forward() * 2.5), down.then(|| b.position() - b.forward() * 2.5)];
                let kick = dust::Kick {
                    dusty: at.iter().flatten().any(|&p| dusty(Some((p, false)))),
                    spin: 0.0,
                    dirt: dirt(at[1]),
                };
                (at, kick)
            }
        })
        .collect()
}

/// The bikes feed their dust, dirt, exhaust and spark emitters, and what
/// is in flight flies on (see `particles`).
/// One raced frame of the live ride log: the race clock, the frame, the
/// actions held (bit per action: 0 left, 1 right, 2 lean forward, 3 lean
/// back, 4 gas, 5 brake, ...) and the player's state.
fn log_ride(out: &mut impl std::io::Write, r: &Race, acts: &[bool], dt: f32) {
    let race::Bike::Player(p) = &r.racers[r.player].bike else { return };
    let keys = acts.iter().enumerate().fold(0u32, |k, (i, &a)| k | (a as u32) << i);
    let (pos, v, m) = (p.position, p.velocity, p.rotation());
    let f = m.z_axis;
    let _ = writeln!(
        out,
        "{:.4},{:.4},{},{:.3},{:.3},{:.3},{:.4},{:.4},{:.4},{:.3},{:.3},{:.3},{:.3},{},{:.0},{},{:.3},{:.3},{},{},{:.3},{:.3}",
        r.time, dt, keys, pos.x, pos.y, pos.z,
        -m.x_axis.y.clamp(-1.0, 1.0).asin(), f.y.clamp(-1.0, 1.0).asin(), f.x.atan2(f.z),
        v.x, v.y, v.z, v.length(), p.drivetrain.gear, p.drivetrain.rpm, p.crash.is_some() as u8,
        -p.rider.0, p.rider.1, p.tyres_down()[0] as u8, p.tyres_down()[1] as u8,
        p.squeezes()[0], p.squeezes()[1]
    );
}

fn throw_dirt(race: &Race, ground: &TrackGround, dirt: &mut particles::Particles, dt: f32) {
    let tyre = |at: Vec3, on_object: bool| {
        let zone = if on_object { ground.zones[7] } else { ground.zone(at) };
        particles::Tyre {
            at,
            dust: zone.dust,
            dirt: zone.dirt,
        }
    };
    // The exhaust's place on the bike (0x4087e0).
    let exhaust = |place: Mat4| place.transform_point3(Vec3::new(0.0, 2.5, -2.75));
    // A bike on the full model: the player's, or a computer rider's once
    // crashed.
    let full = |b: &mm2_physics::Bike| {
        let (traction, spin, throw) = b.roost();
        let crashed = b.crash.is_some();
        particles::Feed {
            tyres: b.tyre_contacts().map(|t| t.filter(|_| !crashed).map(|(at, on)| tyre(at, on))),
            traction,
            spin,
            throw,
            right: b.rotation().x_axis,
            bars: b.steer,
            running: b.speed() >= 0.2,
            exhaust: exhaust(b.place()),
            gas: b.drivetrain.throttle,
            gas_ok: !crashed && b.velocity.length() < 22.0,
            hit: b.hit,
            slide: b.wreck.as_ref().filter(|w| w.touching).map(|w| w.position),
            moved: false,
        }
    };
    for (i, r) in race.racers.iter().enumerate() {
        let feed = match &r.bike {
            race::Bike::Player(b) => full(b),
            race::Bike::Computer(c) => match &c.wreck {
                Some(b) => full(b),
                None => {
                    // The computer's bikes keep their tyres' traction at 1
                    // and spin at 0: dust, but no chunks.
                    let rd = &c.rider;
                    let ends = drawn_ends(r.bike.place());
                    particles::Feed {
                        tyres: [0, 1].map(|k| rd.down[k].then(|| tyre(ends[k], false))),
                        traction: 1.0,
                        right: rd.up.cross(rd.look).normalize_or(Vec3::X),
                        running: rd.forward_speed() >= 0.2,
                        exhaust: exhaust(r.bike.place()),
                        // The computer's throttle opens as its speed rises.
                        gas: rd.speed,
                        gas_ok: rd.speed < 22.0,
                        ..Default::default()
                    }
                }
            },
            race::Bike::Taped(_) => {
                let down = !r.bike.airborne() && !r.bike.crashed();
                let at = r.bike.position() - r.bike.forward() * 2.5;
                particles::Feed {
                    tyres: [None, down.then(|| tyre(at, false))],
                    traction: 1.0,
                    running: r.bike.speed() >= 0.2,
                    exhaust: exhaust(r.bike.place()),
                    ..Default::default()
                }
            }
        };
        dirt.bike(i, &feed, dt);
    }
    dirt.update(dt);
}

/// Bikes running through a river throw water up (`water`, the track's
/// rivers); returns where each splashes and how hard, for its sound.
fn splash_water(race: &Race, water: &[mm2_course::nature::Water], dirt: &mut particles::Particles, dt: f32) -> Vec<(Vec3, f32)> {
    let mut out = Vec::new();
    if water.is_empty() {
        return out;
    }
    for r in &race.racers {
        let at = r.bike.position();
        let Some((level, _)) = crate::water::level_at(water, at.x, at.z) else { continue };
        let speed = r.bike.speed();
        // In the water, not over it in the air.
        if at.y > level + 3.5 || speed < 3.0 {
            continue;
        }
        let amount = (speed / 45.0).min(1.0);
        dirt.splash(Vec3::new(at.x, level, at.z), r.bike.velocity(), amount, dt);
        out.push((Vec3::new(at.x, level, at.z), amount));
    }
    out
}

/// The thrown rider's collision shapes (`Rider.col`, loaded by 0x4337c0
/// from 0x535830): each part's frame in Rider.mcf and its corners there.
static RIDER_COL: std::sync::OnceLock<Vec<(usize, Vec<Vec3>)>> = std::sync::OnceLock::new();

/// Hands each thrown rider's pelvis and collision shapes, as the crash
/// motion poses them, to the wreck physics: the body is the pelvis, and
/// the shapes are its ground contacts.
fn feel_wrecks(race: &mut Race, renderer: &TerrainRenderer) {
    let Some(parts) = RIDER_COL.get() else {
        return;
    };
    for r in &mut race.racers {
        let Some((place, _)) = r.bike.wreck() else {
            continue;
        };
        let Some(pelvis) = renderer.object_frame_index(r.object + 1, "Pelvis") else {
            continue;
        };
        let frames: Vec<usize> = parts.iter().map(|p| p.0).chain([pelvis]).collect();
        let posed = renderer.object_frames(r.object + 1, &frames, race.time);
        let Some(Some(at)) = posed.last().copied() else {
            continue;
        };
        let to_body = at.inverse();
        let points: Vec<Vec3> = posed
            .iter()
            .zip(parts)
            .filter_map(|(m, (_, corners))| m.map(|m| (to_body * m, corners)))
            .flat_map(|(m, corners)| corners.iter().map(move |c| m.transform_point3(*c)))
            .collect();
        r.bike.set_wreck_pose(place.inverse() * at, points);
    }
}

/// Where a bike's headlight is: how far ahead of the bike's middle and how
/// high, ft.
const HEADLIGHT_AHEAD: f32 = 2.6;
const HEADLIGHT_UP: f32 = 3.2;

fn show(race: &Race, track: &Track, renderer: &mut TerrainRenderer, eye: Vec3, view: Option<cues::Eye>) {
    // The player's shadow: the bike and rider, seen from the sun.
    {
        let me = &race.racers[race.player];
        renderer.set_shadow([me.object, me.object + 1], me.bike.position() + Vec3::Y * 1.5, track.sun);
    }
    // Each bike's headlight: up front, about at the bars.
    let lamps: Vec<(Vec3, Vec3)> = race
        .racers
        .iter()
        .enumerate()
        .map(|(i, r)| {
            if race.away(i) {
                return (Vec3::new(0.0, -1.0e5, 0.0), Vec3::Y);
            }
            let ahead = r.bike.forward().normalize_or(Vec3::Z);
            let place = r.bike.place();
            let up = place.transform_vector3(Vec3::Y).normalize_or(Vec3::Y);
            // Aimed a little down, at the track ahead, from where the bike
            // is drawn.
            (place.w_axis.truncate() + ahead * HEADLIGHT_AHEAD + up * HEADLIGHT_UP, (ahead - up * 0.12).normalize())
        })
        .collect();
    renderer.set_headlights(&lamps);
    for (i, r) in race.racers.iter().enumerate() {
        // A ghost race's ghost is see-through.
        let ghost = race.ghost == Some(i);
        renderer.set_object_ghost(r.object, ghost);
        renderer.set_object_ghost(r.object + 1, ghost);
        // Seen from the player's own eyes, the player's rider is left out
        // of the picture (his shadow stays), so no arm or helmet fills it.
        renderer.set_object_unseen(r.object + 1, i == race.player && race.first_person());
        renderer.show_mud(i, [r.object, r.object + 1]);
        // Crossed up in the air, the cross-up motions turn the bike and
        // the rider themselves.
        let place = if r.pose.crossed() {
            r.bike.place()
        } else {
            r.bike.place() * Mat4::from_quat(r.bike.drawn_turn())
        };
        renderer.set_object_place(r.object, place);
        // The wheels turn with the bike's speed (0x518c40).
        for (frame, (angle, _)) in ["TireFront", "TireRear"].into_iter().zip(r.wheels) {
            renderer.set_spin(r.object, frame, angle);
        }
        // The fork slides up its stroke and the swingarm swings up as the
        // shocks take the bumps (0x4ff100, 0x4ff950 move the frames).
        let (fork, swing) = r.bike.suspension_frames();
        renderer.set_slide(r.object, "Shocks", fork);
        renderer.set_spin(r.object, "SwingArm", swing);
        // In a crash the rider is thrown off and plays the fall.
        let wreck = r.bike.wreck();
        let (rider, motion) = wreck.unwrap_or((place, RIDING));
        renderer.set_object_place(r.object + 1, rider);
        match r.trick_motion(&race.tricks).filter(|_| wreck.is_none()) {
            Some((trick, clock)) => {
                let start = race.time - clock;
                renderer.set_motion(r.object + 1, &trick.rider, start);
                renderer.set_motion(r.object, &trick.bike, start);
            }
            None if wreck.is_some() => {
                // A bail starts at once (0x537890); a fall and the landing
                // after it fade in over 0.5 s (0x537980, 0x535d50).
                use mm2_physics::wreck::{FALLS, HITS};
                let span = if FALLS.contains(&motion) || HITS.contains(&motion) { 0.5 } else { 0.0 };
                renderer.fade_motion(r.object + 1, motion, race.time, span);
                renderer.set_motion(r.object, BIKE_RIDING, 0.0);
            }
            None => {
                // Riding: the turn and lean motions mixed by the steer,
                // the rider's weight and the speed (0x48fe50).
                let (rider, bike) = &r.mixes;
                renderer.set_motion(r.object + 1, RIDING, 0.0);
                renderer.set_blend(r.object + 1, rider);
                renderer.set_motion(r.object, BIKE_RIDING, 0.0);
                renderer.set_blend(r.object, bike);
            }
        }
        // An empty place in a free roam session: nobody there to draw.
        if race.away(i) {
            let gone = Mat4::from_cols(Vec4::ZERO, Vec4::ZERO, Vec4::ZERO, Vec4::new(0.0, -1.0e5, 0.0, 1.0));
            renderer.set_object_place(r.object, gone);
            renderer.set_object_place(r.object + 1, gone);
        }
    }
    for gate in [track.gate, track.free_gate].into_iter().flatten() {
        renderer.set_motion_start(gate, race.lead);
    }
    let hidden = |at: Mat4| Mat4::from_cols(Vec4::ZERO, Vec4::ZERO, Vec4::ZERO, at.w_axis);
    // The flag girl waves the flag she has out; the other copy is hidden.
    if let Some((girls, place)) = track.people.girl {
        let g = race.flag_girl;
        for (k, object) in girls.into_iter().enumerate() {
            let out = !race.practice && g.white == (k == 1);
            renderer.set_motion(object, g.motion, g.start);
            renderer.set_motion_start(object, g.start);
            renderer.set_object_place(object, if out { place } else { hidden(place) });
        }
    }
    // The 30-second man walks in 15 ft before the player's bike during
    // the countdown (0x421f90, 0x419fe3) and is gone when the gate drops.
    if let Some(man) = track.people.man {
        let me = &race.racers[race.player].bike;
        let forward = me.forward();
        let mut at = me.position() + forward * finish::MAN_AHEAD;
        at.y = ride::ground(&track.terrain, at.x, at.z);
        let place = objects::orientation((-forward).into(), [0.0, 1.0, 0.0], Vec3::ONE, at.into());
        let out = !race.practice && !race.gate_down();
        renderer.set_object_place(man, if out { place } else { hidden(place) });
        renderer.set_motion_start(man, race.lead - finish::MAN_LEAD);
    }
    // The first three on the podium after the race; the rest are hidden.
    if let Some(stand) = &track.podium {
        let shown = race.podium.as_ref().map(|(p, start)| (p, *start, race.standings()));
        for (k, &model) in stand.models.iter().enumerate() {
            let place = shown.as_ref().filter(|(p, ..)| p.model() == k).map(|(p, ..)| p.place());
            renderer.set_object_place(model, place.unwrap_or(hidden(Mat4::IDENTITY)));
        }
        for (i, &object) in stand.winners.iter().enumerate() {
            let on = shown.as_ref().and_then(|(p, start, order)| {
                let count = order.len().min(3);
                let k = order.iter().take(count).position(|&r| r == i)?;
                Some((p.rider(k, count), *start))
            });
            match on {
                Some(((at, motion), start)) => {
                    renderer.set_motion(object, &motion, start);
                    renderer.set_motion_start(object, start);
                    renderer.set_object_place(object, at);
                }
                None => renderer.set_object_place(object, hidden(Mat4::IDENTITY)),
            }
        }
    }
    // The player's last stunt score.
    if let Some(popup) = &track.popup {
        let me = &race.racers[race.player];
        let score = race
            .popup
            .map(|(at, base, m)| (race.time - at, base, m))
            .filter(|_| !me.bike.crashed());
        let places = popup.places(score, me.bike.position(), me.bike.forward(), eye, FOCAL);
        for (object, place) in places {
            renderer.set_object_place(object, place);
        }
    }
    // Moto-Tag's signs and ball, hidden outside a tag game; in any other
    // race with more than one rider the player's own sign stands over
    // their bike (0x418cd0, 0x41e0b0) when Player Indicator is on.
    if let Some(models) = track.tag {
        // The signs over the riders cast no shadow; the ball does.
        renderer.set_shadowless(&[models[0], models[2]]);
        let hidden = Mat4::from_cols(Vec4::ZERO, Vec4::ZERO, Vec4::ZERO, Vec4::W);
        let mut places = match &race.tag {
            Some(t) => {
                let riders = race.tag_riders();
                let me = race.names.len().gt(&1).then_some(race.player);
                t.places(&riders, me, race.time)
            }
            None if race.racers.len() > 1 => {
                let riders = race.tag_riders();
                [hidden, hidden, tag::own_sign(&riders[race.player], race.time)]
            }
            None => [hidden; 3],
        };
        if !race.player_indicator || race.ghost.is_some() || race.first_person() {
            places[2] = hidden;
        }
        for (object, place) in models.into_iter().zip(places) {
            renderer.set_object_place(object, place);
        }
    }
    if let Some(cues) = &track.cues {
        place_cues(race, track, cues, renderer, view);
    }
    if let Some(c) = &track.countdown {
        place_countdown(race, track, c, renderer, view);
    }
    for (object, place) in track.traffic.places() {
        renderer.set_object_place(object, place);
    }
    // Only the gate the player rides for and the one after stand
    // (0x4052c0); the others are hidden.
    let count = track.arches.len();
    let next = race.racers[race.player].timing.gate;
    for (k, &(object, place)) in track.arches.iter().enumerate() {
        let shown = k == next || k == (next + 1) % count;
        renderer.set_object_place(
            object,
            if shown {
                place
            } else {
                Mat4::from_cols(Vec4::ZERO, Vec4::ZERO, Vec4::ZERO, place.w_axis)
            },
        );
    }
}

/// The vehicle the player at the bike gets in: the nearest of the track's
/// cars, trucks and trains within reach, or else (or with `craft_only`)
/// the nearest craft in the sky, wherever it is.
fn pick_vehicle(r: &Race, track: &mut Track, craft_only: bool) -> Option<drive::Drive> {
    let at = r.racers[r.player].bike.position();
    let time = r.time;
    match drive::nearest_car(&track.traffic, at).filter(|_| !craft_only) {
        Some(i) => {
            let model = track.traffic.cars[i].model.clone();
            drive::Drive::car(&mut track.traffic, i, &model)
        }
        None => {
            let objects = &track.objects.animated;
            drive::nearest_craft(&track.aircraft, at, |o| objects.get(o).map(|m| m.root_at(time).w_axis.truncate()))
                .and_then(|c| Some(drive::Drive::craft(c, objects.get(c.object)?, time)))
        }
    }
}

/// Holds a craft where the player flies it.
fn pin_craft(model: Option<&mut objects::AnimatedModel>, d: Option<&drive::Drive>) {
    if let (Some(a), Some(d)) = (model, d) {
        a.pin = Some(d.key);
        a.set_place(d.place());
    }
}

/// The player in a vehicle (crate::drive): the rider has got off the bike
/// and into it, a craft goes where it is flown, and its shadow is the
/// player's.
fn drive_show(d: &drive::Drive, race: &Race, track: &Track, renderer: &mut TerrainRenderer) {
    let me = &race.racers[race.player];
    let hidden = Mat4::from_cols(Vec4::ZERO, Vec4::ZERO, Vec4::ZERO, me.bike.position().extend(1.0));
    renderer.set_object_place(me.object + 1, hidden);
    let object = match d.seat {
        drive::Seat::Air(o) => {
            pin_craft(renderer.animated_mut(o), Some(d));
            Some(o)
        }
        drive::Seat::Car(c) => track.traffic.cars.get(c).and_then(|c| c.object),
    };
    if let Some(o) = object {
        renderer.set_shadow([o, o], d.position + Vec3::Y * 2.0, track.sun);
    }
    // A parked craft's propellers and rotors turn while it is flown (one
    // in its own flight turns them in its motion).
    if let (drive::Seat::Car(_), Some(o)) = (d.seat, object) {
        for (name, axis, angle) in d.blades() {
            renderer.set_turn(o, name, axis, angle);
        }
    }
}

/// The 3D countdown numbers (0x48cf70): in a race, in the live race's
/// own view, before the gate drops when the 30-second man is not out, and
/// while the player is off the course.
fn place_countdown(race: &Race, track: &Track, c: &countdown::Countdown, renderer: &mut TerrainRenderer, view: Option<cues::Eye>) {
    let hide = |place: Mat4| Mat4::from_cols(Vec4::ZERO, Vec4::ZERO, Vec4::ZERO, place.w_axis);
    let me = &race.racers[race.player];
    let left = track.people.man.is_none().then_some(race.lead - race.time);
    let shown = view
        .filter(|_| !race.practice && race.ghost.is_none() && race.watched.is_none())
        .and_then(|v| {
            // Only the lap races' tracker (0x4eb170) counts the time off
            // the course.
            let off = if race.gates.is_empty() { me.off_course() } else { 0.0 };
            countdown::number(left, off).map(|k| (k, v))
        });
    for (k, &object) in c.numbers.iter().enumerate() {
        let place = match shown {
            Some((n, v)) if n == k => c.place(k, v),
            _ => hide(Mat4::IDENTITY),
        };
        renderer.set_object_place(object, place);
    }
}

/// The Visual Cue arrow and the runway lights (0x48c0a0, 0x48b700): only
/// in the live race's own view, with Visual Cues on and the player still
/// racing.
fn place_cues(race: &Race, track: &Track, cues: &cues::Cues, renderer: &mut TerrainRenderer, view: Option<cues::Eye>) {
    let hide = |place: Mat4| Mat4::from_cols(Vec4::ZERO, Vec4::ZERO, Vec4::ZERO, place.w_axis);
    let me = &race.racers[race.player];
    let racing = race.visual_cues && race.ghost != Some(race.player) && !me.timing.finished && race.podium.is_none();
    let ground = |x: f32, z: f32| {
        let h = |x: f32, z: f32| ride::ground(&track.terrain, x, z);
        let normal = Vec3::new(h(x - 1.0, z) - h(x + 1.0, z), 2.0, h(x, z - 1.0) - h(x, z + 1.0)).normalize();
        (h(x, z), normal)
    };
    // Off the course in a lap race, the place to get back to.
    let rejoin = race.course.as_ref().and_then(|c| me.rejoin(c)).filter(|_| race.gates.is_empty());
    let aim = if let Some(t) = &race.tag {
        // IT, unless that is the player; with the ball, the ball.
        match (&t.ball, t.it) {
            (Some(b), _) => Some(b.position),
            (None, Some(it)) if it != race.player => Some(race.racers[it].bike.position()),
            _ => None,
        }
    } else if let Some(gate) = race.gates.get(me.timing.gate) {
        // The gate's own place, which stands 2 ft above the ground
        // (0x48c0a0 aims at the waypoint the course read, 0x405330).
        Some(gate.centre + Vec3::Y * gates::RAISE)
    } else {
        rejoin.map(|(at, _)| {
            let mut at = at;
            at.y = ground(at.x, at.z).0;
            at
        })
    };
    let arrow = match (view, aim) {
        (Some(view), Some(aim)) if racing => cues.arrow(view, aim),
        _ => hide(Mat4::IDENTITY),
    };
    renderer.set_object_place(cues.arrow, arrow);
    let lights = rejoin.filter(|_| racing && view.is_some() && race.tag.is_none());
    for (k, &object) in cues.lights.iter().enumerate() {
        let place = lights.map(|(at, way)| cues.lights(at, way, race.time, ground)[k]);
        renderer.set_object_place(
            object,
            match place {
                Some((place, true)) => place,
                Some((place, false)) => hide(place),
                None => hide(Mat4::IDENTITY),
            },
        );
    }
}

/// The flag girl, with the checkered flag and with the white one, and the
/// 30-second man, as animated objects.
#[derive(Debug, Clone, Copy, Default)]
struct Finish {
    girl: Option<([usize; 2], Mat4)>,
    man: Option<usize>,
}

/// The two podium models and each grid rider's `Winner.mcf`, as objects, and
/// the scene's own podium spot and direction.
#[derive(Debug, Clone, Default)]
struct Stand {
    models: [usize; 2],
    winners: Vec<usize>,
    site: Option<(Vec3, Vec3)>,
}

/// Moto-Tag's models in `RES/MODELS.RES`: the IT sign, the ball (0x4ad900)
/// and the sign over the player's own bike (0x419fe3's `youind.slt`).
const TAG_MODELS: [&str; 3] = ["Itind.slt", "BBHighLod.slt", "YouInd.slt"];

/// A ghost race's ghost bike and rider (`Ghost.slt`, `GhostRider.slt`).
const GHOST: (&str, &str) = ("Ghost.mcf", "GhostRider.mcf");

/// The rider's motion on the bike, and the bike's.
const RIDING: &str = "LeanR02";
const BIKE_RIDING: &str = "BLeanR02";

/// The player's bike from the game's `UI/UIRES.RES`: the Honda CR250,
/// a 250 (`bikes.pb`) with the first preset of its class (`presets.pb`).
fn player_spec(res: &Path, cc: u32) -> Spec {
    let presets = res
        .parent()
        .map(|game| game.join("UI").join("UIRES.RES"))
        .and_then(|p| read_archive(&p).ok())
        .and_then(|bytes| {
            let archive = rs2::Archive::parse(&bytes).ok()?;
            let text = String::from_utf8_lossy(archive.get("presets.pb")?).into_owned();
            Spec::from_presets(&text, spec::category(cc), Some(0))
        });
    // Without the file (the Free Pack alone), the same numbers built in.
    presets.unwrap_or_else(|| Spec::stock(spec::category(cc)))
}

/// Where a race starts, in game coordinates.
#[derive(Debug, Clone, Copy)]
pub struct Start {
    position: Vec3,
    /// Level and of unit length.
    direction: Vec3,
    /// Where the grid's middle stands from `position`: ft behind it and
    /// ft left of it. None goes by the start line.
    grid: Option<(f32, f32)>,
}

/// Loads a track in one go (see `load_track_steps`).
fn load_track(
    path: &Path,
    course: usize,
    riders: usize,
    rides: Vec<Option<roster::Ride>>,
    me: usize,
    ghost: Option<usize>,
    random_gates: Option<gates::RandomGates>,
) -> Result<Track, BoxError> {
    job::block_on(load_track_steps(path, course, riders, rides, me, ghost, random_gates))
}

/// Reads a track's `.ENV` archive, and the models it uses from the game's
/// `RES` folder when that can be found above it.
/// `rides` are each racer's bike, outfit and number from the menus, the
/// player's first; any left out take the default grid.
async fn load_track_steps(
    path: &Path,
    course: usize,
    riders: usize,
    rides: Vec<Option<roster::Ride>>,
    me: usize,
    ghost: Option<usize>,
    random_gates: Option<gates::RandomGates>,
) -> Result<Track, BoxError> {
    let ride = rides.get(me).cloned().flatten();
    // Building a Free Pack track takes a phone a while: the page says so.
    load_stage(if free_tracks::to_build(path) { "build" } else { "track" }).await;
    // A Free Pack track is built from its recipe the first time.
    free_tracks::ensure(path)?;
    // A demo track rebuilt the first time (an extra).
    demo_tracks::ensure(path)?;
    let track_bytes = read_archive(path)?;
    let archive = rs2::Archive::parse(&track_bytes)?;
    let find = |ext: &str| {
        archive
            .entries()
            .iter()
            .find(|e| e.name.to_ascii_lowercase().ends_with(ext))
    };
    let entry = find(".trn").ok_or("no terrain (.trn) in this archive")?;
    let terrain = trn::parse(archive.data(entry))?;
    let ground_map = find(".gmt").and_then(|e| fine_ground::GroundMap::parse(archive.data(e)));
    load_stage("ground").await;
    let used_before = hires::pack().map_or(0, HiRes::used);
    let terrain_textures = terrain_textures(&terrain, &file_name(path), hires::pack());
    load_step().await;

    let scene = find(".scn")
        .map(|e| scn::Scene::parse(archive.data(e)))
        .transpose()?;
    let zones = scene.as_ref().map_or_else(Default::default, |s| s.zones);
    load_step().await;
    let (mut start, course, stem) = race_start(&archive, scene.as_ref(), &terrain, course);
    // A Baja or Enduro course's gates, from the `.wpt` beside its `.tdf`.
    let waypoints = stem
        .and_then(|stem| archive.get(&format!("{stem}.wpt")))
        .map(|b| gates::Waypoints::parse(&String::from_utf8_lossy(b)))
        .unwrap_or_default();
    // Or, with the Random Gate box ticked, gates laid out at random over
    // the terrain (0x405330), and no course line for anyone to follow.
    let random = random_gates.and_then(|r| {
        // The solid objects (those with a shape in `COLSEG.RES`) a gate's
        // 15 ft box must miss; ours count each as a 10 ft box round its
        // place, for its own size.
        let solid = solid_models(path);
        let solids: Vec<[f32; 2]> = scene
            .as_ref()
            .map(|s| {
                s.models
                    .iter()
                    .filter(|m| solid.contains(&model_stem(&m.model)))
                    .map(|m| [m.position[0], m.position[2]])
                    .collect()
            })
            .unwrap_or_default();
        let reach = gates::CLEAR + 10.0;
        let near = |x: f32, z: f32| solids.iter().any(|p| (p[0] - x).abs() < reach && (p[1] - z).abs() < reach);
        let w = gates::Waypoints::random(
            r,
            (terrain.size - 1) as f32 * terrain.scale,
            terrain.scale,
            |x, z| ride::normal(&terrain, Vec3::new(x, 0.0, z)),
            |x, z| ride::ground(&terrain, x, z),
            near,
        );
        eprintln!("{} random gates from race #{}", w.points.len(), r.seed);
        (w.points.len() >= 2).then_some(w)
    });
    let random_course = random.is_some();
    let (waypoints, course) = match random {
        Some(w) => (w, None),
        None => (waypoints, course),
    };
    let gate_list = waypoints.gates(|x, z| ride::ground(&terrain, x, z));
    load_step().await;
    // A course with no start line starts at its last gate, the finish
    // arch: the original's lone rider stands 7.96 ft through it and
    // 0.78 ft right of its middle (all six Baja and Enduro tapes).
    // So does one made of gates alone.
    let gates_only = course.is_none() && !gate_list.is_empty();
    if random_course || gates_only || course.as_ref().is_some_and(|c| c.start.is_none()) {
        if let Some(last) = gate_list.last() {
            start = Start {
                position: Vec3::new(
                    last.position.x,
                    if random_course || gates_only { ride::ground(&terrain, last.position.x, last.position.z) } else { start.position.y },
                    last.position.z,
                ),
                direction: last.look,
                grid: Some((-7.96, -0.78)),
            };
        }
    }
    let name = scene.as_ref().map(|s| s.name.clone()).unwrap_or_default();
    let (mut grid, mut gate, mut free_gate, mut arch_objects) = (Vec::new(), None, None, Vec::new());
    let mut people = Finish::default();
    let mut podium_objects = None;
    let mut tricks = Vec::new();
    let mut popup_models = None;
    let mut tag_models = None;
    let mut cue_models = None;
    let mut countdown_models = None;
    let mut world = mm2_physics::collision::World::default();
    let mut traffic = traffic::Traffic::default();
    let crowd = scene.as_ref().is_some_and(|s| s.crowd);
    let mut scene_sounds = Vec::new();
    let res = path
        .ancestors()
        .map(|dir| dir.join("RES"))
        .find(|res| fs::is_file(res.join("MODELS.RES")))
        // Without the original's files, the free pack stands in for them.
        .or_else(freepack::root);
    // The bikes and riders, their numbers and motions: the game's own on
    // every track (a free track's folder is the pack's), unless the free
    // graphics were picked.
    let bikes_res = freepack::original_res()
        .filter(|r| fs::is_file(r.join("BIKE.RES")))
        .or_else(|| res.clone());
    let cc = ride.as_ref().map_or(250, |r| r.bike.cc);
    let spec = bikes_res
        .as_deref()
        .map(|res| player_spec(res, cc))
        .unwrap_or_default();
    let sky = load_sky(res.as_deref(), scene.as_ref());
    load_step().await;
    let fog = scene.as_ref().and_then(|s| s.fog_color);
    let haze = scene
        .as_ref()
        .filter(|s| s.cube_file.is_some())
        .map(|s| (s.fog_visibility, s.fog_haziness));
    let detail = res.as_deref().and_then(|r| detail_texture(r, scene.as_ref()));
    let particle_sheet = res.as_deref().and_then(|r| {
        let name = scene.as_ref().and_then(|s| s.particle_texture.as_deref());
        name.and_then(|n| global_texture(r, n)).or_else(|| dirt_sheet(r))
    });
    let mut scene_lights = scene
        .as_ref()
        .map_or_else(|| renderer::SceneLights::from_scene(&[]), |s| renderer::SceneLights::from_scene(&s.lights));
    // Free and Course Builder tracks: ground with no light painted in.
    if find(".scn").is_some_and(|e| archive.data(e).windows(13).any(|w| w.eq_ignore_ascii_case(b"LightGround=1"))) {
        scene_lights.light_ground = 1.0;
    }
    let sun = scene
        .as_ref()
        .and_then(|s| {
            s.lights
                .iter()
                .find(|l| l.kind == scn::LightKind::Directional)
                .and_then(|l| l.direction)
        })
        .map(Vec3::from)
        .filter(|d| d.y < -1e-3)
        .unwrap_or(Vec3::new(1.0, -10.0, 0.0))
        .normalize();
    // The enhanced drawing's spectators: where each stands, looking which way.
    let mut spectators = Vec::new();
    let objects = match (scene, res.clone()) {
        (Some(scene), Some(res)) => {
            // Track makers pack their own models, textures and motions
            // into the track's file; they are found there first.
            let mut model_files = vec![read_archive(&res.join("MODELS.RES"))?, track_bytes.clone()];
            let mut texture_files = vec![
                (file_name(path).to_ascii_uppercase(), track_bytes.clone()),
                ("OBJECTS.RES".to_string(), read_archive(&res.join("OBJECTS.RES"))?),
            ];
            load_step().await;
            // Bikes and riders keep their models, motions and skins together.
            let mut animations = scene.animations.clone();
            // The Free Pack's start gate stands in the scene as a still
            // model; it goes with the animations to drop as the original's
            // `StartGate.mcf` does (freepack::gate_drop).
            animations.extend(scene.models.iter().filter(|m| freepack::is_gate(&m.model)).map(|m| scn::Animation {
                model: freepack::GATE_MCF.into(),
                offset: m.position,
                look: m.look,
                up: m.up,
                scale: m.scale,
                motion: None,
                lighting: m.lighting,
            }));
            let mut skins = Vec::new();
            let mut grid_sources = Vec::new();
            match read_archive(&bikes_res.as_deref().unwrap_or(&res).join("BIKE.RES")) {
                Ok(bytes) => {
                    let bike = ride.as_ref().map_or(GRID[0].0, |r| r.bike.model.as_str());
                    tricks = rs2::Archive::parse(&bytes)
                        .map(|ar| trick_motions(&ar, bike))
                        .unwrap_or_default();
                    // Without the game's motions (the Free Pack alone), the
                    // pack's riders do tricks of their own.
                    if tricks.iter().all(Option::is_none) {
                        tricks = freepack::own_tricks();
                    }
                    // The player's collision points, from the bike's `.col`.
                    if let Ok(ar) = rs2::Archive::parse(&bytes) {
                        let stem = bike.rsplit_once('.').map_or(bike, |(s, _)| s);
                        let col = ar
                            .get(&format!("{stem}.col"))
                            .or_else(|| ar.get("honda.col"))
                            .and_then(|d| mm2_formats::col::parse(d).ok());
                        match col {
                            Some(col) => race::set_hull(mm2_physics::bike::Hull::new(&col)),
                            // The pack's own bikes have none, and without the
                            // game's files there's no `honda.col` to borrow:
                            // the one every bike of the original shares, so a
                            // free bike rides as its MM2 counterpart.
                            None => race::set_hull(mm2_physics::bike::Hull::shared()),
                        }
                        if let Some(col) = ar.get("Rider.col").and_then(|d| mm2_formats::col::parse(d).ok()) {
                            let parts = col
                                .parts
                                .iter()
                                .filter_map(|p| {
                                    let corners = p.body.vertices.iter().map(|v| Vec3::from(*v)).collect();
                                    Some((p.frame? as usize, corners))
                                })
                                .collect();
                            let _ = RIDER_COL.set(parts);
                        }
                        if let Some(frames) = cross_up(&ar, bike) {
                            race::set_cross_up(frames);
                        }
                    }
                    // Without the original's bikes, the computer's riders
                    // ride the free pack's.
                    // (`MM2_FREE_GRID` gives them the pack's anyway.)
                    let originals = rs2::Archive::parse(&bytes).is_ok_and(|ar| ar.get(GRID[0].0).is_some())
                        && std::env::var_os("MM2_FREE_GRID").is_none();
                    model_files.push(bytes.clone());
                    texture_files.push(("BIKE.RES".to_string(), bytes));
                    grid_sources = start_grid(
                        &mut animations,
                        &mut skins,
                        &start,
                        &terrain,
                        riders,
                        &rides,
                        ghost,
                        originals,
                    );
                }
                Err(err) => eprintln!("{err}"),
            }
            // An outdoor track's trees and bushes, from `ECO.RES`.
            let mut statics = scene.models.clone();
            statics.retain(|m| !freepack::is_gate(&m.model));
            load_step().await;
            // A lap race's finish arch and flag girl (0x418cd0). Custom
            // tracks hide them with a finish line that has no direction:
            // the game turns them to face along it (0x500bb0), and with
            // nothing to face along they shrink to nothing.
            let finish_line = course
                .as_ref()
                .and_then(|c| c.finish)
                .filter(|l| l.direction[0] != 0.0 || l.direction[2] != 0.0)
                .filter(|_| gate_list.is_empty());
            // Without the game, the pack's arch (and its flag girl) stand in.
            let arch_width = rs2::Archive::parse(&model_files[0])
                .ok()
                .and_then(|ar| mm2_formats::slb::parse(ar.get("MetalArch.slb")?).ok())
                .or_else(|| freepack::model_shape("metalarch"))
                .map(|m| {
                    let x = m.lods.iter().take(1).flat_map(|l| &l.surfaces);
                    2.0 * x.flat_map(|s| &s.positions).fold(0.0f32, |w, p| w.max(p[0].abs()))
                });
            let mut girl_sources = None;
            let mut girl_place = Mat4::IDENTITY;
            if let (Some(line), Some(width)) = (finish_line, arch_width) {
                let (arch, girl_box, girl) =
                    finish::layout(&line, width, |x, z| ride::ground(&terrain, x, z));
                statics.push(arch);
                statics.push(girl_box);
                skins.resize(animations.len(), None);
                girl_sources = Some([animations.len(), animations.len() + 1]);
                girl_place = objects::orientation(girl.look, girl.up, Vec3::ONE, girl.offset);
                animations.push(girl.clone());
                animations.push(girl);
                skins.push(None);
                skins.push(Some(format!("{}{}", objects::SECOND, finish::WHITE)));
            }
            let man_source = animations
                .iter()
                .position(|a| a.model.eq_ignore_ascii_case(finish::MAN));
            load_step().await;
            // Each rider on the podium, in their own outfit (0x45e490).
            skins.resize(animations.len(), None);
            let winner_sources: Vec<usize> = grid_sources
                .iter()
                .map(|&bike| {
                    let outfit = skins[bike + 1].clone();
                    // A rider of the pack's stands up there as itself.
                    let model = freepack::winner_model(&animations[bike + 1].model)
                        .unwrap_or_else(|| podium::WINNER.into());
                    animations.push(scn::Animation {
                        model,
                        offset: [0.0; 3],
                        look: [0.0, 0.0, 1.0],
                        up: [0.0, 1.0, 0.0],
                        scale: [1.0; 3],
                        motion: Some("Podium4_01".into()),
                        lighting: Default::default(),
                    });
                    skins.push(outfit);
                    animations.len() - 1
                })
                .collect();
            let mut trunks = Vec::new();
            // The eco-system's trees among the statics, and each one's far
            // billboard.
            let mut trees = 0..0;
            let mut billboards = Vec::new();
            if let Some(eco) = find(".esb").and_then(|e| {
                mm2_formats::esb::parse(archive.data(e))
                    .map_err(|err| eprintln!("{}: {err}", e.name))
                    .ok()
            }) {
                match read_archive(&res.join("ECO.RES")) {
                    Ok(bytes) => {
                        let (mut placed, mut solid, mut far) =
                            eco::place(&eco, &rs2::Archive::parse(&bytes)?, &terrain);
                        // Random gates take out the trees within 150 ft
                        // of each gate (0x460c50).
                        if random_course {
                            let r = gates::TREES_CLEARED;
                            let clear = |p: Vec3| {
                                !gate_list.iter().any(|g| (g.position.x - p.x).abs() < r && (g.position.z - p.z).abs() < r)
                            };
                            let keep: Vec<bool> = placed.iter().map(|p| clear(Vec3::from(p.position))).collect();
                            let mut k = keep.iter();
                            placed.retain(|_| *k.next().unwrap());
                            let mut k = keep.iter();
                            far.retain(|_| *k.next().unwrap());
                            solid.retain(|t| clear(t.base));
                        }
                        // Without the species' models (the free pack has
                        // none yet) there are no trees to bump into.
                        if placed.is_empty() {
                            solid.clear();
                        }
                        eprintln!("{} trees and bushes, {} trunks", placed.len(), solid.len());
                        trees = statics.len()..statics.len() + placed.len();
                        billboards = far;
                        statics.extend(placed);
                        trunks = solid;
                        model_files.push(bytes.clone());
                        texture_files.push(("ECO.RES".to_string(), bytes));
                    }
                    Err(err) => eprintln!("{err}"),
                }
            }
            for name in &scene.resource_files {
                let name = name.to_ascii_uppercase();
                match read_archive(&res.join(&name)) {
                    Ok(bytes) => texture_files.push((name, bytes)),
                    Err(err) => eprintln!("{err}"),
                }
            }
            let textures = texture_files
                .iter()
                .map(|(name, b)| Ok((name.as_str(), rs2::Archive::parse(b)?)))
                .collect::<Result<Vec<_>, mm2_formats::Error>>()?;
            let models = model_files
                .iter()
                .map(|b| rs2::Archive::parse(b))
                .collect::<Result<Vec<_>, _>>()?;
            let mut arches = waypoints.placements(&gate_list);
            // Without the game's own arch (the Free Pack's art), the pack's
            // checkpoint arches take turns.
            let own_arch = models.iter().any(|m| m.get("waypnt.slb").is_some());
            if !own_arch && waypoints.model.eq_ignore_ascii_case("waypnt.slt") {
                let seed = file_name(path).bytes().fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b.to_ascii_uppercase() as u32));
                let last = arches.len().saturating_sub(1);
                for (i, a) in arches.iter_mut().enumerate().take(last) {
                    a.model = format!("{}.slt", freepack::waypoint_variant(i, seed));
                }
            }
            let procvue = read_archive(&res.join("PROCVUE.RES")).unwrap_or_default();
            let procvue = rs2::Archive::parse(&procvue).ok();
            let mut vehicles = traffic::load(&scene, procvue.as_ref());
            // "Drive the scenery" on a free track, which has no traffic of
            // its own: a lot of the pack's cars and machines parked by the
            // start, and its aircraft out on open ground, all to drive.
            let free_track = file_name(path).to_ascii_uppercase().starts_with("FP");
            if tweaks::more().drive_vehicles && free_track && vehicles.is_empty() && scene.stadium.is_none() {
                let gates: Vec<Vec3> = gate_list.iter().map(|g| g.position).collect();
                let parked = scenery::parking(&terrain, course.as_ref(), &gates, (start.position, start.direction), &statics, &drive::FLEET);
                eprintln!("{} parked vehicles to drive", parked.len());
                for place in parked {
                    let (at, look) = (Vec3::from(place.position), Vec3::from(place.look));
                    let path = mm2_physics::procedural::Path { keys: vec![(0.0, at), (15.0, at + look)], fps: 15.0 };
                    let mut vehicle = mm2_physics::procedural::Vehicle::new(path, 0.0, Vec3::ZERO);
                    vehicle.position = at;
                    vehicle.heading = look;
                    vehicle.up = Vec3::Y;
                    vehicles.push((traffic::PARKED, vehicle, place));
                }
            }
            let mut movable = arches.clone();
            movable.extend(popup::Popup::placements());
            movable.extend(TAG_MODELS.map(|m| mm2_formats::scn::Placement {
                model: m.into(),
                position: [0.0; 3],
                look: [0.0, 0.0, 1.0],
                up: [0.0, 1.0, 0.0],
                scale: [0.0; 3],
                lighting: Default::default(),
            }));
            movable.extend(podium::MODELS.map(|m| mm2_formats::scn::Placement {
                model: m.into(),
                position: [0.0; 3],
                look: [0.0, 0.0, 1.0],
                up: [0.0, 1.0, 0.0],
                scale: [0.0; 3],
                lighting: Default::default(),
            }));
            movable.extend(vehicles.iter().map(|(_, _, model)| model.clone()));
            movable.extend(cues::MODELS.map(|m| mm2_formats::scn::Placement {
                model: m.into(),
                position: [0.0; 3],
                look: [0.0, 0.0, 1.0],
                up: [0.0, 1.0, 0.0],
                scale: [0.0; 3],
                lighting: Default::default(),
            }));
            movable.extend(countdown::MODELS.map(|m| mm2_formats::scn::Placement {
                model: m.into(),
                position: [0.0; 3],
                look: [0.0, 0.0, 1.0],
                up: [0.0, 1.0, 0.0],
                scale: [0.0; 3],
                lighting: Default::default(),
            }));
            // The enhanced drawing's trackside dressing, outdoors.
            let desert = path.parent().and_then(|d| d.file_name()).is_some_and(|d| d.eq_ignore_ascii_case("BAJA"));
            let extras;
            (extras, spectators) = match crate::graphics::enhanced() {
                Some(e) if e.scenery && scene.stadium.is_none() => {
                    let gates: Vec<Vec3> = gate_list.iter().map(|g| g.position).collect();
                    let placed = scenery::place(&terrain, course.as_ref(), &gates, (start.position, start.direction), &statics, desert);
                    eprintln!("{} trackside props", placed.0.len());
                    placed
                }
                _ => (Vec::new(), Vec::new()),
            };
            page_stage("scenery").await;
            let mut objects = ObjectMesh::build(
                &statics,
                (trees, &billboards),
                &extras,
                &movable,
                &animations,
                &skins,
                &models,
                &textures,
                hires::pack(),
            );
            objects.surfaced = materials::surfaced(&file_name(path));
            match read_archive(&res.join("COLSEG.RES")) {
                Ok(bytes) => {
                    let colseg = rs2::Archive::parse(&bytes)?;
                    world = ground::solids(
                        &scene.models,
                        &colseg,
                        &models,
                        scene.stadium.as_deref(),
                    );
                    // A stadium's walls hold the bikes in.
                    if let (Some(seg), Some(frame)) = (
                        scene.stadium.as_deref().and_then(|n| colseg.get(n)),
                        &scene.stadium_frame,
                    ) {
                        let walls = ground::stadium_walls(&String::from_utf8_lossy(seg), frame);
                        if crate::graphics::enhanced().is_some_and(|e| e.scenery) {
                            let outline = ground::stadium_outline(&String::from_utf8_lossy(seg), frame);
                            spectators = scenery::stadium_crowd(&outline, frame.bottom, |p| objects.surface_below(p, frame.bottom + 150.0));
                        }
                        eprintln!("{} stadium walls", walls.len());
                        world.solids.extend(walls);
                    }
                    world.trunks = mm2_physics::collision::Trunks::new(trunks);
                    traffic.first = world.solids.len();
                    // All but the vehicles' stay put.
                    world.still = traffic.first;
                    for (source, vehicle, model) in &vehicles {
                        let first = world.solids.len();
                        world.solids.extend(traffic::solids(&model.model, &colseg));
                        traffic.cars.push(traffic::Car {
                            vehicle: vehicle.clone(),
                            solids: first..world.solids.len(),
                            object: None,
                            source: *source,
                            // A parked one stays put till it is driven.
                            driven: *source == traffic::PARKED,
                            model: model.model.clone(),
                        });
                    }
                    eprintln!("{} scripted vehicles", traffic.cars.len());
                    eprintln!("{} solid objects", world.solids.len());
                }
                Err(err) => eprintln!("{err}"),
            }
            let object = |source: usize| objects.sources.iter().position(|&s| s == source);
            grid = grid_sources.iter().filter_map(|&s| object(s)).collect();
            arch_objects = arches
                .iter()
                .enumerate()
                .filter_map(|(j, p)| {
                    let place = objects::orientation(p.look, p.up, Vec3::from(p.scale), p.position);
                    Some((object(animations.len() + j)?, place))
                })
                .collect();
            let first = animations.len() + arches.len();
            let score: Vec<_> = (first..first + popup::Popup::placements().len())
                .map(object)
                .collect();
            popup_models = popup::Popup::new(&score);
            let first = first + popup::Popup::placements().len();
            let found: Option<Vec<usize>> = (first..first + TAG_MODELS.len()).map(object).collect();
            tag_models = found.and_then(|f| f.try_into().ok());
            let first = first + TAG_MODELS.len();
            podium_objects = object(first).zip(object(first + 1)).map(|(a, b)| Stand {
                models: [a, b],
                winners: winner_sources.iter().filter_map(|&w| object(w)).collect(),
                site: scene.podium.map(|(p, d)| (Vec3::from(p), Vec3::from(d))),
            });
            let first = first + podium::MODELS.len();
            for (j, car) in traffic.cars.iter_mut().enumerate() {
                car.object = object(first + j);
            }
            let first = first + vehicles.len();
            let found: Option<Vec<usize>> = (first..first + cues::MODELS.len()).map(object).collect();
            cue_models = found.map(|f| cues::Cues {
                arrow: f[0],
                lights: [f[1], f[2], f[3], f[4], f[5]],
                size: model_size(&models, "viscue.slb").unwrap_or(1.0),
            });
            let first = first + cues::MODELS.len();
            let found: Option<Vec<usize>> = (first..first + countdown::MODELS.len()).map(object).collect();
            countdown_models = found.map(|f| countdown::Countdown {
                numbers: [f[0], f[1], f[2], f[3], f[4]],
                sizes: countdown::MODELS.map(|m| model_size(&models, &m.replace(".slt", ".slb")).unwrap_or(1.0)),
            });
            load_step().await;
            // The scene's ambient sounds, on what they hang on.
            for sound in &scene.sounds {
                let site = match &sound.site {
                    scn::SoundSite::Fixed(p) => Some(sound::Site::Fixed(Vec3::from(*p))),
                    scn::SoundSite::Model(i) => {
                        scene.models.get(*i).map(|m| sound::Site::Fixed(Vec3::from(m.position)))
                    }
                    scn::SoundSite::Animation(i, frame) => object(*i).map(|o| {
                        let f = frame.as_deref().and_then(|f| objects.animated[o].frame_named(f));
                        sound::Site::Object(o, f)
                    }),
                    scn::SoundSite::Procedural(i) => traffic
                        .cars
                        .iter()
                        .position(|c| c.source == *i)
                        .map(sound::Site::Car),
                };
                if let Some(site) = site {
                    scene_sounds.push((sound.spec.clone(), site));
                }
            }
            gate = animations
                .iter()
                .position(|a| a.model.eq_ignore_ascii_case("StartGate.mcf"))
                .and_then(object);
            free_gate = animations.iter().position(|a| a.model == freepack::GATE_MCF).and_then(object);
            people = Finish {
                girl: girl_sources.and_then(|[a, b]| Some(([object(a)?, object(b)?], girl_place))),
                man: man_source.and_then(object),
            };
            eprintln!(
                "{}: placed {} of {} objects ({} animated)",
                scene.name,
                objects.placed,
                scene.models.len() + animations.len(),
                objects.animated.len()
            );
            if !objects.missing_models.is_empty() {
                eprintln!("models not found: {}", objects.missing_models.join(", "));
            }
            objects
        }
        (_, None) => {
            eprintln!("no RES/MODELS.RES above the track, so no objects");
            ObjectMesh::default()
        }
        _ => ObjectMesh::default(),
    };
    if let Some(h) = hires::pack() {
        eprintln!("using {} high-res textures", h.used() - used_before);
    }
    load_step().await;
    let water = archive
        .entries()
        .iter()
        .find(|e| e.name.to_ascii_lowercase().ends_with(".wtr"))
        .map_or_else(Vec::new, |e| mm2_course::nature::read_water(&String::from_utf8_lossy(archive.data(e))));
    // The rivers sound along their banks.
    for r in &water {
        let mut since = f32::MAX;
        for w in r.points.windows(2) {
            since += Vec3::from(w[1]).distance(Vec3::from(w[0]));
            if since >= 140.0 {
                since = 0.0;
                let spec = scn::SoundSpec {
                    file: sound::RIVER.into(),
                    min_distance: 20.0 + r.width * 0.5,
                    max_distance: 150.0 + r.width,
                    looping: true,
                    one_shot: false,
                    one_shot_distance: 20.0,
                    trigger_percent: 100.0,
                    force_2d: false,
                };
                scene_sounds.push((spec, sound::Site::Fixed(Vec3::from(w[0]))));
            }
        }
    }
    let aircraft = drive::aircraft(objects.animated.iter().map(|a| a.model.as_str()));
    // What's left is handing it all to the graphics card (`finish_loading`).
    page_stage("gpu").await;
    Ok(Track {
        terrain,
        terrain_textures,
        world,
        spec,
        objects,
        start,
        course,
        grid,
        gate,
        free_gate,
        people,
        podium: podium_objects,
        res,
        name,
        supercross: path.parent().and_then(|d| d.file_name()).is_some_and(|d| d.eq_ignore_ascii_case("SX")),
        night_lights: night::read_lights(path),
        desert: path.parent().and_then(|d| d.file_name()).is_some_and(|d| d.eq_ignore_ascii_case("BAJA")),
        gates: gate_list.iter().map(gates::Gate::line).collect(),
        arches: arch_objects,
        tricks,
        popup: popup_models,
        tag: tag_models,
        cues: cue_models,
        countdown: countdown_models,
        sky,
        fog,
        haze,
        sun,
        lights: scene_lights,
        detail,
        ground_map,
        particle_sheet,
        traffic,
        zones,
        crowd,
        spectators,
        water,
        sounds: scene_sounds,
        aircraft,
        free: freepack::chosen() || file_name(path).to_ascii_uppercase().starts_with("FP"),
    })
}

/// Twice a model's largest half extent, its most detailed LOD's (0x5036e0,
/// as an `ArcadeObject` sizes itself on the screen).
fn model_size(models: &[rs2::Archive], name: &str) -> Option<f32> {
    let data = models.iter().find_map(|m| m.get(name))?;
    let model = mm2_formats::slb::parse(data).ok()?;
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for p in model.lods.first()?.surfaces.iter().flat_map(|s| &s.positions) {
        lo = lo.min(Vec3::from(*p));
        hi = hi.max(Vec3::from(*p));
    }
    (hi.x >= lo.x).then(|| (hi - lo).max_element())
}

/// The scene's sky box from `RES/SKIES.RES`.
/// The dirt particles' sheet, `DirtPart.TGA` in `RES/GLOBAL.RES`.
/// The scene's detail texture from `RES/GLOBAL.RES`: the one it names, or
/// `GrayNoise.tga`. A named one the archive lacks (the ski lodge's
/// `whitenoise.tga`) leaves the terrain without.
fn detail_texture(res: &std::path::Path, scene: Option<&scn::Scene>) -> Option<mm2_formats::tga::Image> {
    let name = scene.and_then(|s| s.detail_texture.as_deref()).unwrap_or("GrayNoise.tga");
    global_texture(res, name)
}

/// A texture in `RES/GLOBAL.RES`.
/// Without the game's archive (the Free Pack alone), the pack's own
/// picture of that name in its `global` folder.
fn global_texture(res: &std::path::Path, name: &str) -> Option<mm2_formats::tga::Image> {
    let path = res.join("GLOBAL.RES");
    if !fs::is_file(&path) {
        return freepack::global_image(name);
    }
    let bytes = read_archive(&path).ok()?;
    let archive = mm2_formats::rs2::Archive::parse(&bytes).ok()?;
    let data = archive
        .entries()
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case(name))
        .map(|e| archive.data(e))?;
    mm2_formats::texture::parse_overlay(data).ok()
}

fn dirt_sheet(res: &std::path::Path) -> Option<mm2_formats::tga::Image> {
    global_texture(res, "DirtPart.TGA")
}

fn load_sky(
    res: Option<&Path>,
    scene: Option<&scn::Scene>,
) -> Option<(mm2_formats::cub::Cube, hires::SkyImages)> {
    let name = scene?.cube_file.as_deref()?;
    let skies = res.and_then(|res| read_archive(&res.join("SKIES.RES")).ok());
    let skies = skies.as_deref().and_then(|s| rs2::Archive::parse(s).ok());
    let Some(data) = skies.as_ref().and_then(|s| s.get(name)) else {
        // Not in the game's files: the free pack's.
        return freepack::sky(name);
    };
    match mm2_formats::cub::Cube::parse(data) {
        Ok(cube) => {
            let images = hires::sky_images(&cube, name, hires::pack());
            Some((cube, images))
        }
        Err(e) => {
            eprintln!("sky {name}: {e}");
            None
        }
    }
}

/// The bike model's root frame on each frame of its cross-up motion
/// (`CrosBik1`), as turns of the bike.
fn cross_up(bike_res: &rs2::Archive, bike: &str) -> Option<Vec<glam::Quat>> {
    // The free pack's bikes play the original bike's motions.
    let bike = freepack::animated(bike).map_or(bike, |(template, _)| template);
    let mcf = mm2_formats::anim::Mcf::parse(bike_res.get(bike)?).ok()?;
    let info = mcf
        .motions
        .iter()
        .find(|m| m.name.eq_ignore_ascii_case("CrosBik1"))?;
    let motion = mm2_formats::anim::Motion::parse(bike_res.get(&info.file)?).ok()?;
    motion
        .frames
        .iter()
        .map(|keys| {
            let k = keys.iter().find(|k| k.frame == 0)?;
            let look = glam::Vec3::from(k.look).normalize_or(glam::Vec3::Z);
            let right = glam::Vec3::from(k.up).cross(look).normalize_or(glam::Vec3::X);
            let up = look.cross(right);
            Some(glam::Quat::from_mat3(&glam::Mat3::from_cols(right, up, look)))
        })
        .collect()
}

/// The rider's and the bike's motions for each trick, from `Rider.mcf` and
/// the bike's `.mcf` (the bike's are named `b<trick>`).
fn trick_motions(bike_res: &rs2::Archive, bike: &str) -> Vec<Option<race::TrickMotion>> {
    let bike = freepack::animated(bike).map_or(bike, |(template, _)| template);
    let mcf = |name: &str| {
        bike_res
            .get(name)
            .and_then(|b| mm2_formats::anim::Mcf::parse(b).ok())
    };
    let (Some(rider), Some(bike)) = (mcf("Rider.mcf"), mcf(bike)) else {
        return Vec::new();
    };
    let names = mm2_physics::stunt::TRICKS
        .iter()
        .map(|t| t.to_string())
        .chain(mm2_physics::stunt::TRICKS.iter().map(|t| format!("{t}Alt")));
    names
        .map(|name| {
            let r = rider
                .motions
                .iter()
                .find(|m| m.name.eq_ignore_ascii_case(&name))?;
            let b = format!("b{name}");
            let b = bike
                .motions
                .iter()
                .find(|m| m.name.eq_ignore_ascii_case(&b));
            Some(race::TrickMotion {
                rider: r.name.clone(),
                bike: b.map_or(String::new(), |b| b.name.clone()),
                fps: r.fps,
                frames: r.frame_count as i32,
            })
        })
        .collect()
}

/// The start line from the track definition (`.tdf`), or the scene's
/// default start when there is none.
/// A model's name without its extension, in lower case.
fn model_stem(name: &str) -> String {
    name.rsplit_once('.').map_or(name, |(s, _)| s).trim().to_ascii_lowercase()
}

/// The models that have a collision shape in the game's `RES/COLSEG.RES`.
fn solid_models(track: &Path) -> std::collections::HashSet<String> {
    track
        .ancestors()
        .map(|dir| dir.join("RES").join("COLSEG.RES"))
        .find(|p| fs::is_file(p))
        .and_then(|p| read_archive(&p).ok())
        .and_then(|b| {
            let ar = rs2::Archive::parse(&b).ok()?;
            Some(
                ar.entries()
                    .iter()
                    .filter(|e| e.name.to_ascii_lowercase().ends_with(".col"))
                    .map(|e| model_stem(&e.name))
                    .collect(),
            )
        })
        .unwrap_or_default()
}

fn race_start(
    archive: &rs2::Archive,
    scene: Option<&scn::Scene>,
    terrain: &Terrain,
    which: usize,
) -> (Start, Option<TrackDef>, Option<String>) {
    // Tracks with several courses (Baja, enduro) number them.
    let mut defs: Vec<_> = archive
        .entries()
        .iter()
        .filter(|e| e.name.to_ascii_lowercase().ends_with(".tdf"))
        .collect();
    defs.sort_by_key(|e| e.name.to_ascii_lowercase());
    let def = defs.get(which).or(defs.first());
    // A Baja or Enduro track made with only its gates (`.wpt`, no `.tdf`,
    // as many players' tracks are): the course is its gates alone.
    let mut gates: Vec<_> = archive.entries().iter().filter(|e| e.name.to_ascii_lowercase().ends_with(".wpt")).collect();
    gates.sort_by_key(|e| e.name.to_ascii_lowercase());
    let stem = def
        .or_else(|| gates.get(which).or(gates.first()))
        .map(|e| e.name[..e.name.len() - 4].to_string());
    let course = def.and_then(|e| {
        tdf::parse(archive.data(e))
            .map_err(|err| eprintln!("{}: {err}", e.name))
            .ok()
    });
    let line = course.as_ref().and_then(TrackDef::start_line);
    let (mut position, direction) = match (line, scene) {
        (Some(line), _) => (Vec3::from(line.position), Vec3::from(line.direction)),
        (None, Some(scene)) => (
            Vec3::from(scene.start_position),
            Vec3::from(scene.start_direction),
        ),
        (None, None) => {
            let mid = (terrain.size / 2) as f32 * terrain.scale;
            (Vec3::new(mid, 0.0, mid), Vec3::Z)
        }
    };
    position.y = ride::ground(terrain, position.x, position.z);
    let direction = Vec3::new(direction.x, 0.0, direction.z).normalize_or(Vec3::Z);
    (
        Start {
            position,
            direction,
            grid: None,
        },
        course,
        stem,
    )
}

/// Bikes, their riders' outfits from `RES/BIKE.RES`.
/// The player rides the first; the computer's riders take the rest in turn.
const GRID: [(&str, &str); 8] = [
    ("honda.mcf", "FOX02_360Red_08.tga"),
    ("yamaha.mcf", "FOX02_360Navy_08.tga"),
    ("ktm.mcf", "FOX02_360Orange_08.tga"),
    ("suzuki.mcf", "FOX02_360Yellow_08.tga"),
    ("honda.mcf", "FOX02_360Green_08.tga"),
    ("yamaha.mcf", "FOX02_360Purple_08.tga"),
    ("ktm.mcf", "FOX02_360Black_08.tga"),
    ("suzuki.mcf", "FOX02_360Patriot_08.tga"),
];
/// The player's name until there are profiles (the game takes it from
/// the one chosen on the main menu).
const PLAYER_NAME: &str = "Player";

/// A racer's name: the one the menus gave, else the player's stand-in or
/// a computer rider's from the game's list (`LANG.DLL` 5000 to 5035;
/// which rider gets which name is our pick).
fn rider_name(race: &Race, ui: &ui::Ui, rider: usize) -> String {
    match race.names.get(rider).filter(|n| !n.is_empty()) {
        Some(name) => name.clone(),
        None if rider == race.player => PLAYER_NAME.to_string(),
        None => ui.lang(5000 + (rider as u32 * 7 + 3) % 36),
    }
}

/// The Race Info page's lines for a race, in finishing order (`0x4521b0`).
fn results(race: &Race, ui: &ui::Ui) -> frontend::RaceResults {
    let unset = "-:--.--".to_string();
    let lines = race
        .standings()
        .into_iter()
        .enumerate()
        .map(|(place, i)| {
            let r = &race.racers[i];
            let name = rider_name(race, ui, i);
            let t = &r.timing;
            if race.stunt_event {
                // The Stunt Quarry lists names and scores (result +0x2c).
                return [
                    name,
                    String::new(),
                    format!("{:.0}", r.stunt_points),
                    String::new(),
                ];
            }
            let best = if t.best_lap > 0.0 {
                hud::clock_text(t.best_lap)
            } else {
                unset.clone()
            };
            let total = if t.finished {
                hud::clock_text(t.race_time)
            } else {
                ui.lang(4080)
            };
            [
                name,
                (place + 1).to_string(),
                format!("{best} ({total})"),
                String::new(),
            ]
        })
        .collect();
    let mut placings = vec![frontend::Placing::default(); race.racers.len()];
    for (place, i) in race.standings().into_iter().enumerate() {
        let r = &race.racers[i];
        placings[i] = frontend::Placing {
            place: place + 1,
            repair: r.repair,
            medical: r.medical,
            stunt: r.stunt_units,
        };
    }
    frontend::RaceResults {
        lines,
        stunt: race.stunt_event,
        placings,
        replay: None,
    }
}

/// The number on each rider's plates. The game takes the player's from
/// the profile (EditPlateNumber) and the computer riders' from their
/// records; these are our picks until those are read.
fn plate_number(rider: usize) -> u32 {
    // Past the game's 11 (the more riders extra), numbers of their own.
    match rider {
        0..=10 => [7, 22, 4, 13, 31, 2, 18, 9, 45, 11, 26][rider],
        _ => [33, 15, 27, 5, 41, 19, 8, 36, 24, 12, 48, 3, 29, 17, 38, 6, 21, 44, 14][(rider - 11) % 19],
    }
}

/// Which of them the player rides.
const PLAYER: usize = 0;
/// Riders when the command line starts a race: the player and three.
const RIDERS: usize = 4;
/// Bikes side by side in a row at the start.
const ROW: usize = 8;
/// The start grid's spacing, and how far left of the line's middle it
/// sits, ft.
const SLOT_WIDTH: f32 = 7.75;
const GRID_OFFSET: f32 = 3.03;
/// `--spawn x,z,heading`: where the first bike stands instead, to replay
/// a tape from exactly its start.
static SPAWN: std::sync::OnceLock<(Vec3, Vec3)> = std::sync::OnceLock::new();

/// Moves the start gate, and whatever the scene placed with it (the
/// 30-second board man), onto the start line as the game does, and lines up
/// bikes and riders behind it. Returns the bikes' indices in `animations`;
/// `skins` gets a rider outfit for each animation.
fn start_grid(
    animations: &mut Vec<scn::Animation>,
    skins: &mut Vec<Option<String>>,
    start: &Start,
    terrain: &Terrain,
    riders: usize,
    rides: &[Option<roster::Ride>],
    ghost: Option<usize>,
    originals: bool,
) -> Vec<usize> {
    let gate = animations
        .iter()
        .find(|a| a.model.eq_ignore_ascii_case("StartGate.mcf"))
        .map(|g| g.offset);
    let (d, up) = (start.direction, Vec3::Y);
    let right = up.cross(d);
    if let Some(gate) = gate {
        for a in animations.iter_mut().filter(|a| a.offset == gate) {
            a.offset = start.position.into();
            a.look = d.into();
            a.up = up.into();
        }
    }
    // Behind the line: the original's bikes stand 4.7 ft back (its tapes,
    // gate or not); clear of the bars where there is no line to go by.
    let (back, left) = start
        .grid
        .unwrap_or((if gate.is_some() { 4.6 } else { 2.0 }, GRID_OFFSET));
    skins.resize(animations.len(), None);
    // Without the stock grid's bikes (the demo has two of the game's), the
    // computer's riders ride the game's own bikes it does have, on every
    // track, before the free pack's.
    let own_grid = match (originals, freepack::original_res()) {
        (false, Some(res)) if std::env::var_os("MM2_FREE_GRID").is_none() => {
            let game = res.parent();
            let mut own = game.and_then(roster::Roster::originals).unwrap_or_default();
            // Only the ones whose models and paints are really there.
            if let Some(have) = game.and_then(demo::bike_files) {
                let has = |f: &str| have.contains(&f.to_ascii_lowercase());
                own.bikes.retain(|b| has(&b.model) && (b.texture.is_empty() || has(&b.texture)));
                own.outfits.retain(|o| has(&o.file));
            }
            own
        }
        _ => roster::Roster::default(),
    };
    let own_grid = (!own_grid.bikes.is_empty() && !own_grid.outfits.is_empty()).then_some(own_grid);
    let mut bikes = Vec::new();
    for i in 0..riders.max(1) {
        let (mut bike, mut outfit) = (
            GRID[i % GRID.len()].0.to_string(),
            GRID[i % GRID.len()].1.to_string(),
        );
        let mut skin = Some(format!("{}{}", objects::PLATE, plate_number(i)));
        let (free_bikes, free_outfits) = (freepack::bikes(), freepack::outfits());
        if let Some(own) = &own_grid {
            let b = &own.bikes[i % own.bikes.len()];
            bike = b.model.clone();
            skin = Some(match b.texture.as_str() {
                "" => format!("{}{}", objects::PLATE, plate_number(i)),
                paint => format!("{}{}:{paint}", objects::PLATE, plate_number(i)),
            });
            outfit = own.outfits[(i + 1) % own.outfits.len()].file.clone();
        } else if !originals && !free_bikes.is_empty() && !free_outfits.is_empty() {
            // Spread over the shapes and colours: no two alike side by side.
            // (`MM2_FREE_GRID=n` starts the turn at the pack's bike n.)
            let first: usize = std::env::var("MM2_FREE_GRID").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
            let b = &free_bikes[(first + i * 7) % free_bikes.len()];
            bike = format!("{}.mcf", b.shape);
            skin = Some(format!("{}{}:{}", objects::PLATE, plate_number(i), b.livery));
            outfit = free_outfits[(i + 1) % free_outfits.len()].outfit.clone();
        }
        let mut rider = "Rider.mcf".to_string();
        if let Some(Some(r)) = rides.get(i) {
            bike = r.bike.model.clone();
            outfit = r.outfit.clone();
            // No paint (a replay of a computer rider on the default grid)
            // keeps the model's own; an empty one would put the number
            // plate's texture on the whole bike.
            skin = Some(match r.bike.texture.as_str() {
                "" => format!("{}{}", objects::PLATE, r.plate),
                paint => format!("{}{}:{paint}", objects::PLATE, r.plate),
            });
        }
        // A ghost race's ghost rides its own models (0x41a770).
        let ghostly = ghost == Some(i);
        if ghostly {
            (bike, rider, skin) = (GHOST.0.into(), GHOST.1.into(), None);
        }
        // Rows of up to eight, the player on the right: the original's
        // tapes put them 7.75 ft apart, centred 3 ft left of the line's
        // middle (five riders on SX01; a lone practice rider on every
        // supercross track).
        let (row, slot) = (i / ROW, i % ROW);
        let in_row = (riders - row * ROW).min(ROW);
        let across = (in_row as f32 - 1.0) / 2.0 - slot as f32;
        let mut at = start.position - d * (back + row as f32 * 8.0)
            + right * (across * SLOT_WIDTH - left);
        let mut look = d;
        if let (0, Some(&(spot, way))) = (i, SPAWN.get()) {
            (at, look) = (spot, way);
        }
        at.y = ride::ground(terrain, at.x, at.z);
        let place = |model: &str, motion: &str| scn::Animation {
            model: model.to_string(),
            offset: at.into(),
            look: look.into(),
            up: up.into(),
            scale: [1.0; 3],
            motion: Some(motion.to_string()),
            lighting: Default::default(),
        };
        bikes.push(animations.len());
        animations.push(place(&bike, BIKE_RIDING));
        skins.push(skin);
        // An outfit of the free pack's goes on its own rider.
        if let Some(model) = freepack::outfit_model(&outfit).filter(|_| !ghostly) {
            rider = model;
        }
        animations.push(place(&rider, RIDING));
        skins.push((!ghostly).then_some(outfit));
    }
    let p = start.position;
    eprintln!("start line at {:.0},{:.0},{:.0}", p.x, p.y, p.z);
    bikes
}

/// Two copies racing over the network, headless, for testing: each rides
/// on autopilot for `seconds` of real time and says where the other's
/// bike is. With `roam`, a free roam session: the host starts alone and
/// the other drops in when it comes and readies.
fn net_test(
    track: &Track,
    how: &str,
    seconds: f32,
    tag: Option<(f32, bool)>,
    roam: bool,
) -> Result<(), BoxError> {
    let ball = tag.is_some_and(|t| t.1);
    let me = |name: &str| net::Player {
        name: name.into(),
        ready: true,
        ..Default::default()
    };
    let session = if how == "host" {
        net::Session::host("test", me("Host"))?
    } else {
        let addr = how.strip_prefix("join:").ok_or("--net host or join:ADDR (or race code)")?;
        net::Session::join(addr.parse()?, me("Guest"))?
    };
    net::set(Some(session));
    let started = Instant::now();
    let mut told = String::new();
    // A host waits a little for its race code, to say it.
    while how == "host" && started.elapsed().as_secs() < 15 && net::with(|s| s.code()).unwrap_or_default().is_empty() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let n = loop {
        let code = net::with(|s| s.code()).unwrap_or_default();
        if code != told {
            println!("race code {code}");
            told = code;
        }
        if started.elapsed().as_secs() > 40 {
            return Err("nobody came".into());
        }
        let heard = net::with(|s| s.poll()).unwrap_or_default();
        let (host, players, me) = net::with(|s| (s.host, s.players.clone(), s.me)).unwrap();
        if heard.contains(&net::Heard::Players) && !host {
            net::with(|s| s.say(net::Msg::Ready(true)));
        }
        if host && roam {
            let mut places = players.clone();
            places.resize(net::MAX_PLAYERS, net::Player { id: net::EMPTY, ..Default::default() });
            let game = net::GameInfo { mode: net::ROAM_MODE, event: 2, ..Default::default() };
            net::with(|s| s.say(net::Msg::Start { game, players: places }));
            break frontend::NetRace {
                me: 0,
                humans: net::MAX_PLAYERS,
                host: true,
                tag_ball: false,
                tag_stunts: false,
                roam: true,
            };
        }
        if host && players.len() >= 2 {
            net::with(|s| {
                s.say(net::Msg::Start {
                    game: net::GameInfo::default(),
                    players: players.clone(),
                })
            });
            break frontend::NetRace {
                me: 0,
                humans: players.len(),
                host: true,
                tag_ball: ball,
                tag_stunts: false,
                roam: false,
            };
        }
        if let Some(net::Heard::Start { players, .. }) = heard
            .into_iter()
            .find(|h| matches!(h, net::Heard::Start { .. }))
        {
            let at = players.iter().position(|p| p.id == me).unwrap_or(0);
            eprintln!("dropped in at place {at} of {}", players.len());
            break frontend::NetRace {
                me: at,
                humans: players.len(),
                host: false,
                tag_ball: ball,
                tag_stunts: false,
                roam,
            };
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    let mut r = make_race_as(track, race::ENDLESS, n.me).ok_or("no bikes")?;
    // Scripted: the game's own steps, frame by frame.
    r.fixed_step = false;
    r.ai_frame = 0.0;
    r.autopilot = true;
    if let Some((minutes, ball)) = tag {
        let mut t = tag::Tag::new(r.racers.len(), ball, false);
        t.judge = n.host;
        t.local = (0..r.racers.len()).map(|i| owned(n, i)).collect();
        r.tag = Some(t);
        r.minutes = Some(minutes);
    }
    let mut remote: Vec<replay::Remote> = (0..r.racers.len()).map(|_| Default::default()).collect();
    for (i, racer) in r.racers.iter_mut().enumerate() {
        if !owned(n, i) {
            racer.bike = replay::taped_like(&racer.bike);
        }
    }
    let mut wait = 0.0;
    let step = 1.0 / 60.0;
    let start = Instant::now();
    let mut k = 0u32;
    while start.elapsed().as_secs_f32() < seconds {
        r.update(&track.ground(), Controls::default(), step);
        let _ = net_race(n, &mut r, &mut remote, &mut wait, &track.terrain, step);
        k += 1;
        let ahead = k as f32 * step - start.elapsed().as_secs_f32();
        if ahead > 0.0 {
            std::thread::sleep(std::time::Duration::from_secs_f32(ahead));
        }
    }
    for (i, racer) in r.racers.iter().enumerate() {
        let p = racer.bike.position();
        let whose = if owned(n, i) { "mine" } else if r.away(i) { "empty" } else { "theirs" };
        let name = r.names.get(i).cloned().unwrap_or_default();
        eprintln!("racer {i} ({whose}) {name} at {:.1},{:.1},{:.1}", p.x, p.y, p.z);
    }
    if let Some(t) = &r.tag {
        let times: Vec<String> = (0..r.racers.len())
            .map(|i| race::time(t.time_it(i)))
            .collect();
        eprintln!("tag: IT {:?}, time as IT {times:?}; {}", t.it, r.status());
    }
    net::set(None);
    Ok(())
}

/// How long a free roam rider can go unheard before their place counts as
/// empty, s.
const ROAM_GONE: f32 = 2.0;

/// Whether this machine moves racer `i` in a network race: its player's,
/// and the host's computer riders on the host.
fn owned(n: frontend::NetRace, i: usize) -> bool {
    i == n.me || (n.host && i >= n.humans)
}

/// A network race's traffic each frame: the other machines' bikes in, and
/// ours out twenty times a second, as the game's whole-bike packets.
fn net_race(
    n: frontend::NetRace,
    race: &mut Race,
    remote: &mut [replay::Remote],
    wait: &mut f32,
    terrain: &Terrain,
    dt: f32,
) -> Vec<String> {
    let mut chat = Vec::new();
    for h in net::with(|s| s.poll()).unwrap_or_default() {
        if let net::Heard::Chat(line) = &h {
            chat.push(line.clone());
        }
        // Free roam: who rides in each place now, for their names.
        if let (true, net::Heard::Start { players, .. }) = (n.roam, &h) {
            for (i, p) in players.iter().enumerate() {
                if i != n.me && i < n.humans {
                    if let Some(name) = race.names.get_mut(i) {
                        *name = p.name.clone();
                    }
                }
            }
        }
        if let net::Heard::Bike {
            racer, kind, data, ..
        } = &h
        {
            tag_heard(n, race, *racer, *kind, data);
            // A jump's score, from the machine that rides the racer.
            if let (mm2_formats::vcr::STUNT, Some(score), false) =
                (*kind, stunt_score(data), owned(n, *racer as usize))
            {
                if let Some(r) = race.racers.get_mut(*racer as usize) {
                    r.heard_stunt(score);
                }
            }
        }
        if let net::Heard::Bike {
            racer,
            time,
            kind: mm2_formats::vcr::FULL,
            data,
        } = h
        {
            let i = racer as usize;
            if let (Some(st), Some(rm), false) = (
                mm2_formats::vcr::State::full(&data),
                remote.get_mut(i),
                owned(n, i),
            ) {
                rm.push(time, st, race.time);
            }
        }
    }
    let at = race.time - replay::NET_DELAY;
    if n.roam {
        // A place is empty while nobody there has been heard of for a
        // couple of seconds: not come yet, or gone.
        race.empty = (0..race.racers.len())
            .map(|i| !owned(n, i) && remote.get(i).and_then(|r| r.heard).is_none_or(|t| race.time - t > ROAM_GONE))
            .collect();
    }
    for (i, rm) in remote.iter().enumerate() {
        if owned(n, i) {
            continue;
        }
        if let (Some(st), Some(r)) = (rm.at(at), race.racers.get_mut(i)) {
            let t = replay::Taped::from_state(&st, terrain);
            if t.trick != r.stunt.trick {
                r.trick_clock = 0.0;
            } else {
                r.trick_clock += dt;
            }
            r.stunt.trick = t.trick;
            r.bike = race::Bike::Taped(Box::new(t));
        }
    }
    tag_say(race);
    // Each jump our riders score goes to the others as the game's stunt
    // packet (type 10), so every scoreboard shows the same points.
    let mut scores = Vec::new();
    for (i, r) in race.racers.iter_mut().enumerate() {
        for score in r.unsent.drain(..) {
            if owned(n, i) {
                scores.push(net::Msg::Bike {
                    racer: i as u8,
                    time: race.time,
                    kind: mm2_formats::vcr::STUNT,
                    data: stunt_packet(i, score),
                });
            }
        }
    }
    if !scores.is_empty() {
        net::with(|s| {
            for m in scores {
                s.say(m);
            }
        });
    }
    *wait -= dt;
    if *wait > 0.0 {
        return chat;
    }
    *wait = replay::NET_SEND;
    let order = race.standings();
    let clock = (race.time * 1000.0) as u32;
    // In a free roam room bikes go packed small; elsewhere as the game's
    // full packet, which every version knows.
    let small = net::with(|s| s.room).unwrap_or(false);
    // In free roam, how far the nearest other rider is: a bike nobody is
    // near is sent less often (see `due`).
    let nearest = |i: usize| -> Option<f32> {
        let me = race.racers[i].bike.position();
        (0..race.racers.len())
            .filter(|&k| !owned(n, k) && !race.empty.get(k).copied().unwrap_or(false))
            .map(|k| (race.racers[k].bike.position() - me).length())
            .min_by(f32::total_cmp)
    };
    let msgs: Vec<net::Msg> = (0..race.racers.len())
        .filter(|&i| owned(n, i))
        .filter_map(|i| {
            let place = order.iter().position(|&k| k == i).unwrap_or(0) as u8 + 1;
            let st = replay::state_of(&race.racers[i], clock, place, i as u8, race.clock());
            let every = if n.roam { send_every(nearest(i)) } else { replay::NET_SEND };
            if !due(i, &st, race.time, every) {
                return None;
            }
            Some(if small {
                net::Msg::Pos { racer: i as u8, time: race.time, data: st.write_small() }
            } else {
                net::Msg::Bike {
                    racer: i as u8,
                    time: race.time,
                    kind: mm2_formats::vcr::FULL,
                    data: st.write_full(),
                }
            })
        })
        .collect();
    net::with(|s| {
        for m in msgs {
            s.say(m);
        }
    });
    chat
}

/// The game's stunt packet (type 10, as on its tapes): the bike in byte
/// 1 and the jump's score, a whole number, in bytes 4..8.
fn stunt_packet(bike: usize, score: f32) -> Vec<u8> {
    let mut d = vec![0u8; 8];
    d[1] = bike as u8;
    d[4..8].copy_from_slice(&(score.round() as u32).to_le_bytes());
    d
}

fn stunt_score(data: &[u8]) -> Option<f32> {
    let b: [u8; 4] = data.get(4..8)?.try_into().ok()?;
    Some(u32::from_le_bytes(b) as f32)
}

/// How often a free roam bike is sent, by how far the nearest other
/// rider is: 20 a second near anyone, fewer when they're far off and the
/// bike is a speck on their screen (their game carries it on by its
/// speed between), and twice a second with nobody else riding.
fn send_every(nearest: Option<f32>) -> f32 {
    match nearest {
        None => 0.5,
        Some(d) if d < NEAR => replay::NET_SEND,
        Some(d) if d < FAR => 0.1,
        Some(_) => 0.25,
    }
}

/// Feet within which a rider gets every update, and beyond which a few.
const NEAR: f32 = 500.0;
const FAR: f32 = 1500.0;

/// Whether racer `i`'s bike is due to be sent, `every` seconds apart; a
/// bike standing still (in the start, waiting at the side) only every
/// [`STILL_EVERY`] seconds, which saves most of the traffic of a free
/// roam session where riders stop to chat.
fn due(i: usize, st: &mm2_formats::vcr::State, now: f32, every: f32) -> bool {
    use std::cell::RefCell;
    thread_local! {
        static LAST: RefCell<Vec<Option<(f32, mm2_formats::vcr::State)>>> = const { RefCell::new(Vec::new()) };
    }
    LAST.with(|l| {
        let mut l = l.borrow_mut();
        if l.len() <= i {
            l.resize(i + 1, None);
        }
        let still = |a: &mm2_formats::vcr::State| {
            let moved: f32 = (0..3).map(|k| (a.position[k] - st.position[k]).powi(2)).sum::<f32>().sqrt();
            let speed: f32 = st.velocity.iter().map(|v| v * v).sum::<f32>().sqrt();
            moved < 0.1 && speed < 1.0 && a.pose == st.pose && a.flags == st.flags
        };
        // A little slack: sends come on the 20-a-second beat.
        let due = match &l[i] {
            Some((t, a)) => {
                let wait = if still(a) { STILL_EVERY.max(every) } else { every };
                now - t >= wait - 0.01 || now < *t
            }
            None => true,
        };
        if due {
            l[i] = Some((now, *st));
        }
        due
    })
}

/// How often a bike standing still is sent.
const STILL_EVERY: f32 = 0.25;

/// Moto-Tag's packets: who is IT, from the host (0x87), and a player's
/// claim to the host (0x87 + 0x8c); one byte, the racer or 0xff for none.
const TAG_IT: i32 = 0x87;
const TAG_CLAIM: i32 = 0x87 + 0x8c;

fn tag_heard(n: frontend::NetRace, race: &mut Race, racer: u8, kind: i32, data: &[u8]) {
    let who = |b: Option<&u8>| b.copied().filter(|&b| b != 0xff).map(usize::from);
    let time = race.time;
    let mut points: Vec<f32> = race.racers.iter().map(|r| r.stunt_points).collect();
    let Some(t) = &mut race.tag else { return };
    match kind {
        TAG_IT if !n.host => t.pass(who(data.first()), time, &mut points),
        TAG_CLAIM if n.host => {
            t.claim(racer as usize, who(data.first()), time, &mut points);
        }
        _ => return,
    }
    for (r, p) in race.racers.iter_mut().zip(points) {
        r.stunt_points = p;
    }
}

/// Sends the host's IT changes, or this player's claims.
fn tag_say(race: &mut Race) {
    let Some(t) = &mut race.tag else { return };
    let byte = |w: Option<usize>| vec![w.map_or(0xff, |w| w as u8)];
    let mut msgs: Vec<net::Msg> = t
        .changes
        .drain(..)
        .map(|w| net::Msg::Bike {
            racer: w.map_or(0xff, |w| w as u8),
            time: race.time,
            kind: TAG_IT,
            data: byte(w),
        })
        .collect();
    msgs.extend(t.claims.drain(..).map(|(who, w)| net::Msg::Bike {
        racer: who as u8,
        time: race.time,
        kind: TAG_CLAIM,
        data: byte(w),
    }));
    if msgs.is_empty() {
        return;
    }
    net::with(|s| {
        for m in msgs {
            s.say(m);
        }
    });
}

/// The race's sounds are heard from the camera at `eye`, looking at
/// `target`, moving with the player's bike (0x4e84a0).
/// Where a race's sounds come from. A free track's folder is the pack's,
/// so it has no `RES` of the original's; unless the free graphics were
/// picked, the game's own `RES` (the full game's or the demo's) gives the
/// bikes their original sounds there too, and the pack only fills in.
fn sound_res(track_res: Option<&Path>) -> Option<PathBuf> {
    freepack::original_res().or_else(|| track_res.map(Path::to_path_buf))
}

fn listener(r: &Race, eye: Vec3, target: Vec3) -> sound::Listener {
    let f = target - eye;
    sound::Listener {
        eye,
        right: Vec3::new(f.z, 0.0, -f.x).normalize_or_zero(),
        velocity: r.racers[r.player].bike.velocity(),
    }
}

/// Builds the header saved at the start of a race recording: which
/// track and event, and every rider's bike and outfit.
fn heading(setup: &frontend::Setup, race: &Race) -> replay::Heading {
    let riders = (0..race.racers.len())
        .map(|i| {
            let ride = if i == PLAYER {
                setup.ride.clone()
            } else {
                setup.rides.get(i).cloned().flatten()
            };
            let (model, skin, outfit, cc) = match &ride {
                Some(r) => (
                    r.bike.model.clone(),
                    r.bike.texture.clone(),
                    r.outfit.clone(),
                    r.bike.cc,
                ),
                None => {
                    let g = GRID[i % GRID.len()];
                    (g.0.to_string(), String::new(), g.1.to_string(), 250)
                }
            };
            mm2_formats::vcr::Rider {
                cc,
                flags: (i == PLAYER) as u32 * 7,
                id: i as u32,
                plate: ride.as_ref().map_or(plate_number(i), |r| r.plate),
                computer: i != PLAYER,
                name: setup.names.get(i).cloned().unwrap_or_default(),
                bike_model: model,
                rider_model: "rider.mcf".into(),
                bike_skin: skin,
                rider_skin: outfit,
            }
        })
        .collect();
    replay::Heading {
        track: setup
            .track
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string(),
        course: setup.course,
        event: setup.event as u32,
        mode: setup.mode,
        laps: setup.laps,
        random_gates: setup.random_gates.map(|r| (r.count, r.seed)),
        riders,
    }
}

/// The race a tape was made in, to load its track and riders.
fn replay_setup(game: &Path, tape: &mm2_formats::vcr::Tape) -> Option<frontend::Setup> {
    use mm2_formats::vcr::setting;
    let event = frontend::EventKind::from_number(tape.setting(setting::EVENT))?;
    let stem = tape.track();
    let course = tape.course();
    let entries = frontend::list_tracks(&game.join(event.folder()), event);
    let entry = entries
        .iter()
        .filter(|e| {
            e.path
                .file_stem()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.eq_ignore_ascii_case(&stem))
        })
        .find(|e| e.course == course || !event.courses())?
        .clone();
    let ride = |r: &mm2_formats::vcr::Rider, i: usize| {
        // "Res\suzuki.mcf": the model's name in `RES/BIKE.RES`.
        let model = r
            .bike_model
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or("")
            .to_string();
        (!model.is_empty()).then(|| roster::Ride {
            bike: roster::BikeChoice {
                name: String::new(),
                model,
                ui_model: String::new(),
                texture: r.bike_skin.clone(),
                cc: r.cc,
                four_stroke: false,
                own: false,
            },
            outfit: r.rider_skin.clone(),
            plate: plate_number(i),
        })
    };
    let rides: Vec<_> = tape
        .riders
        .iter()
        .enumerate()
        .map(|(i, r)| ride(r, i))
        .collect();
    Some(frontend::Setup {
        event,
        track: entry.path,
        course: entry.course,
        laps: tape.setting(setting::LAPS).max(1),
        opponents: tape.riders.len().saturating_sub(1) as u32,
        mode: tape.setting(setting::MODE),
        name: entry.name,
        ride: rides.first().cloned().flatten(),
        level: 1,
        names: tape.riders.iter().map(|r| r.name.clone()).collect(),
        rides,
        ghost: None,
        net: None,
        random_gates: tape
            .random_gates()
            .filter(|_| event == frontend::EventKind::Baja)
            .map(|(count, seed)| gates::RandomGates { count, seed }),
    })
}

/// Reads an RS2 archive, decoding FAOE if needed.
/// A track's terrain textures, each replaced by its high-res image when
/// `hires` has one. Textures the tiles use all share the largest size, so
/// if only some were replaced the rest are enlarged to match.
fn terrain_textures(terrain: &Terrain, track: &str, hires: Option<&HiRes>) -> Vec<hires::Image> {
    let mut images: Vec<_> = terrain
        .textures
        .iter()
        .enumerate()
        .map(|(i, tex)| hires::image(hires, &hires::terrain_key(track, i), tex))
        .collect();
    let original = terrain.textures.iter().map(|t| t.size).max().unwrap_or(1);
    let size = images.iter().map(|t| t.size).max().unwrap_or(1);
    for (image, tex) in images.iter_mut().zip(&terrain.textures) {
        if tex.size == original {
            image.grow_to(size);
        }
    }
    images
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn read_archive(path: &Path) -> Result<Vec<u8>, BoxError> {
    // The free pack has none of the original's archives: each one it
    // lacks reads as empty, and its stand-ins fill in.
    if freepack::root().is_some_and(|root| path.starts_with(&root)) && !fs::is_file(path) {
        return Ok(rs2::build(&[]));
    }
    let raw = crate::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    let plain = match Container::detect(&raw) {
        Container::Faoe => faoe::decode(&raw, name)?,
        Container::Rs2 => raw,
        Container::Unknown => return Err(format!("{}: not an RS2 or FAOE archive", path.display()).into()),
    };
    // Player-made packs in `RES/ADDONS` add what custom tracks need.
    Ok(addons::with_addons(path, plain))
}

async fn gpu(
    compatible_surface: Option<&wgpu::Surface<'_>>,
) -> Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue), BoxError> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    gpu_from(&instance, compatible_surface).await
}

async fn gpu_from(
    instance: &wgpu::Instance,
    compatible_surface: Option<&wgpu::Surface<'_>>,
) -> Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue), BoxError> {
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface,
            ..Default::default()
        })
        .await?;
    // `quality=auto`: the preset for this card and screen.
    graphics::pick_auto(&adapter.get_info());
    graphics::fit_screen();
    // The enhanced drawing reads depth textures, which OpenGL cannot.
    if adapter.get_info().backend == wgpu::Backend::Gl && graphics::enhanced().is_some() {
        eprintln!("enhanced graphics need Vulkan, Metal or DirectX 12: drawing the original look on OpenGL");
        graphics::not_drawable();
    }
    // Anti-aliasing with 2 or 8 samples, where the GPU has them.
    let specific = adapter.features() & wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
    // Ray queries for the ray-traced shadows, only when they are asked
    // for and the GPU has them: wgpu still calls them experimental.
    let rays = graphics::wants_ray_tracing() && adapter.features().contains(wgpu::Features::EXPERIMENTAL_RAY_QUERY);
    graphics::set_ray_queries(rays);
    if graphics::wants_ray_tracing() && !rays {
        eprintln!("ray tracing: this GPU cannot trace rays, so the shadow maps are drawn and nothing is reflected");
    }
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("mm2"),
            required_features: specific
                | if rays { wgpu::Features::EXPERIMENTAL_RAY_QUERY } else { wgpu::Features::empty() }
                // MM2_GPU_TIMES: each part of the frame timed (gpu_time).
                | if gpu_time::wanted() { adapter.features() & gpu_time::features() } else { wgpu::Features::empty() },
            required_limits: adapter.limits(),
            // SAFETY: only ray queries are used of the experimental
            // features, and only on a GPU that says it has them.
            experimental_features: if rays { unsafe { wgpu::ExperimentalFeatures::enabled() } } else { wgpu::ExperimentalFeatures::disabled() },
            ..Default::default()
        })
        .await?;
    // A lost device draws nothing again: every frame after is skipped and
    // the picture freezes. In a browser the page starts the game again
    // (Safari loses it when short of graphics memory); natively it is said
    // in the log.
    device.set_device_lost_callback(|reason, why| {
        if reason == wgpu::DeviceLostReason::Destroyed {
            return;
        }
        eprintln!("the graphics device was lost: {why}");
        #[cfg(target_arch = "wasm32")]
        web::gpu_lost(&why);
    });
    if !specific.is_empty() {
        let counts = |f| adapter.get_texture_format_features(f).flags.supported_sample_counts();
        let depth = counts(renderer::DEPTH_FORMAT);
        graphics::set_sample_counts(counts(graphics::HDR_FORMAT).into_iter().filter(|c| depth.contains(c)).collect());
    }
    Ok((adapter, device, queue))
}

/// Renders one frame offscreen and saves it as a PNG.
fn parse_camera(s: &str) -> Option<Camera> {
    let v: Vec<f32> = s
        .split(',')
        .map(|p| p.trim().parse().ok())
        .collect::<Option<_>>()?;
    let [x, y, z, yaw, pitch] = v[..] else {
        return None;
    };
    Some(Camera {
        position: renderer::from_game(Vec3::new(x, y, z)),
        yaw: yaw.to_radians(),
        pitch: pitch.to_radians(),
        fov: camera::FOV,
        roll: 0.0,
    })
}

/// The podium for `event` on `track`: the scene's own spot, a supercross
/// track's start gate, or the terrain's middle (0x45e490).
fn podium_for(track: &Track, stand: &Stand, event: frontend::EventKind, roll: f32) -> podium::Podium {
    use frontend::EventKind::*;
    let gate = (event == Supercross && track.gate.is_some())
        .then_some((track.start.position, track.start.direction));
    // The middle 256-sample block, rounded down: with `n` blocks a side,
    // block n/2 of an odd count, (n-1)/2 of an even one.
    let n = (track.terrain.size - 1) as f32 / 256.0;
    let k = if n as i32 % 2 == 1 { n } else { n - 1.0 } * 0.5;
    let half = (k as i32) as f32 * 256.0 * track.terrain.scale;
    podium::Podium::new(
        stand.site,
        gate,
        Vec3::new(half, 0.0, half),
        |x, z| ride::ground(&track.terrain, x, z),
        matches!(event, Nationals | Supercross | Tag),
        roll,
    )
}

/// A number from 0 to 1 that differs each time.
fn rand_unit() -> f32 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    (nanos % 1000) as f32 / 1000.0
}

/// Points `camera` from a chase camera's eye at its target.
fn chase_at(camera: &mut Camera, (eye, target): (Vec3, Vec3)) {
    *camera = Camera::looking_at(renderer::from_game(eye), renderer::from_game(target));
}

#[allow(clippy::too_many_arguments)]
async fn screenshot(
    track: &Track,
    race: Option<&Race>,
    dirt: Option<(particles::Particles, Vec<(Vec<([Option<Vec3>; 2], dust::Kick)>, f32)>)>,
    replay: Option<&mut replay::Replay>,
    camera: &Camera,
    time: f32,
    out: &Path,
    width: u32,
    height: u32,
) -> Result<(), BoxError> {
    let (adapter, device, queue) = gpu(None).await?;
    let info = adapter.get_info();
    let rt = adapter.features().contains(wgpu::Features::EXPERIMENTAL_RAY_QUERY);
    eprintln!("rendering on {} ({:?}, ray queries: {rt})", info.name, info.backend);
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut renderer = track_renderer(track, &device, &queue, format);
    if let Some((d, tyres)) = dirt {
        renderer.particles.adopt(d);
        for (frame, dt) in &tyres {
            renderer.tyres(frame, *dt);
        }
    }
    if let Some(r) = race {
        let eye = renderer::from_game(camera.position);
        let view = cues::Eye {
            at: eye,
            target: eye + renderer::from_game(camera.forward()),
            fov: camera.fov,
            aspect: width as f32 / height.max(1) as f32,
        };
        show(r, track, &mut renderer, eye, Some(view));
    }
    // A replay is filmed as through a lens, in focus on the rider followed.
    renderer.cinematic = race
        .filter(|_| replay.is_some())
        .map(|r| renderer::from_game(r.racers[r.player].bike.position()).distance(camera.position));
    let target = offscreen(&device, format, width, height);
    let window_view = target.create_view(&Default::default());
    // Wider than 21:9, the race goes between black bars as on screen.
    let mut bars = bars::Bars::default();
    let between_bars = race.and_then(|_| bars.target(&device, format, (width, height)));
    let (view, (width, height)) = match &between_bars {
        Some((view, size)) => (view, *size),
        None => (&window_view, (width, height)),
    };
    // MM2_BENCH=N: draws the frame N times more first, and says how long
    // a frame takes to record and to finish.
    if let Some(n) = std::env::var("MM2_BENCH").ok().and_then(|n| n.parse::<u32>().ok()) {
        let draw = renderer.render(&device, &queue, view, (width, height), camera, time);
        queue.submit([draw]);
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        let (mut record, start) = (0.0, web_time::Instant::now());
        for _ in 0..n {
            let t = web_time::Instant::now();
            let draw = renderer.render(&device, &queue, view, (width, height), camera, time);
            record += t.elapsed().as_secs_f64();
            queue.submit([draw]);
            let _ = device.poll(wgpu::PollType::wait_indefinitely());
        }
        let n = n.max(1) as f64;
        eprintln!("bench: {:.2} ms a frame ({:.3} ms recording)", start.elapsed().as_secs_f64() * 1000.0 / n, record * 1000.0 / n);
        renderer.report_gpu_times(&device);
    }
    // The picture extras, as on screen.
    let mut polish = (between_bars.is_none() && polish::wanted()).then(|| polish::Polish::new(&device, format));
    let polish_view = polish.as_mut().map(|p| p.target(&device, format, (width, height)));
    let draw = renderer.render(&device, &queue, polish_view.as_ref().unwrap_or(view), (width, height), camera, time);
    let draw = match polish.as_ref().and_then(|p| p.finish(&device, &queue, view)) {
        Some(finish) => {
            queue.submit([draw]);
            finish
        }
        None => draw,
    };
    // The in-race display over the race.
    let mut encoder = device.create_command_encoder(&Default::default());
    if let (Some(race), Some((mut ui, mut hud))) = (race, track.hud()) {
        let mut sprites = sprites::Sprites::new(&device, format);
        sprites.stretch = true;
        sprites.scale = (width as f32 / 640.0).min(height as f32 / 480.0);
        sprites.widescreen = replay.is_none();
        let mut gfx = ui::Gfx {
            sprites: &mut sprites,
            device: &device,
            queue: &queue,
        };
        match replay {
            Some(rp) => {
                // Twice: the slider's thumb finds its place once drawn.
                rp.draw(&mut ui, &mut gfx, 0.0);
                gfx.sprites.discard();
                rp.draw(&mut ui, &mut gfx, 0.0);
            }
            None => {
                let aspect = width as f32 / height.max(1) as f32;
                hud.view_proj =
                    Mat4::perspective_rh(camera.fovy(aspect), aspect, 1.0, 20_000.0) * camera.view();
                hud.heads = race.racers.iter().map(|r| renderer.object_frame(r.object + 1, "Head", time)).collect();
                hud.look(&track.terrain, renderer::from_game(camera.position));
                hud.draw(&mut ui, &mut gfx, race, 1.0 / 60.0)
            }
        }
        sprites.render(&device, &mut encoder, view, (width, height), false);
    }
    if between_bars.is_some() {
        bars.finish(&mut encoder, &target);
    }
    queue.submit([draw]);
    save_png(&device, &queue, encoder.finish(), &target, out)
}

/// A renderer for `track` with its sky, lights, detail, fog and dirt.
fn track_renderer(track: &Track, device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> TerrainRenderer {
    let mut renderer = TerrainRenderer::new(
        device,
        queue,
        &track.terrain,
        &track.terrain_textures,
        &track.objects,
        format,
    );
    if let Some((cube, images)) = &track.sky {
        renderer.set_sky(device, queue, cube, images, format);
    }
    renderer.set_lights(track.lights);
    renderer.set_desert(track.desert);
    renderer.set_stadium(track.supercross);
    renderer.set_night(track.night_lights.clone(), track.floor());
    renderer.set_crowd(device, &track.spectators);
    renderer.set_water(device, &track.water);
    if let Some(detail) = &track.detail {
        renderer.set_detail(device, queue, detail);
    }
    if let Some(map) = &track.ground_map {
        renderer.set_ground_map(device, queue, map);
    }
    if let Some(fog) = track.fog {
        renderer.set_fog(fog);
    }
    if let Some((visibility, haziness)) = track.haze {
        renderer.set_visibility(profile::GraphicsOptions::default().terrain, visibility, haziness);
    }
    if let Some(sheet) = &track.particle_sheet {
        renderer.set_dirt(device, queue, sheet, format);
    }
    renderer
}

/// A scripted ride filmed (`--film DIR`): the models posed each frame as
/// live play poses them, so a thrown rider feels its motion, and from
/// `--film-from` s on the chase view saved 15 times a second as
/// DIR/NNNN.png (`--size`). `MM2_WRECK_LOG=FILE` also writes the player's
/// thrown rider each frame: t, stage, motion, the pelvis (x y z, then its
/// x, y and z axes).
struct Film {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: TerrainRenderer,
    dir: Option<PathBuf>,
    from: f32,
    size: (u32, u32),
    next: f32,
    shot: usize,
    log: Option<(PathBuf, String)>,
    /// The vehicle the player drives, if any.
    drive: Option<drive::Drive>,
    /// `MM2_FILM_BENCH`: each frame drawn and timed instead of saved
    /// (crate::bench), for finding where frames come late.
    bench: Option<bench::Bench>,
    /// The game's own step before each frame, s (for the bench).
    pub step: f64,
}

impl Film {
    fn new(track: &Track, dir: Option<PathBuf>, from: f32, size: (u32, u32)) -> Result<Self, BoxError> {
        let (_, device, queue) = pollster::block_on(gpu(None))?;
        let renderer = track_renderer(track, &device, &queue, wgpu::TextureFormat::Rgba8UnormSrgb);
        let log = std::env::var("MM2_WRECK_LOG")
            .ok()
            .map(|f| (PathBuf::from(f), String::from("t,stage,motion,x,y,z,xx,xy,xz,yx,yy,yz,zx,zy,zz\n")));
        let bench = bench::Bench::wanted();
        Ok(Film { device, queue, renderer, dir, from, size, next: from, shot: 0, log, drive: None, bench, step: 0.0 })
    }

    /// Poses the race at `clock` s into the ride, and films it.
    fn frame(&mut self, race: &mut Race, track: &Track, chase: (Vec3, Vec3), clock: f32) -> Result<(), BoxError> {
        // MM2_FILM_WATCH=i: filmed from behind rider i, as `--watch` is;
        // MM2_FILM_SIDE=1: from the player's right, as `--side` is.
        let chase = match (std::env::var("MM2_FILM_WATCH").ok().and_then(|w| w.parse::<usize>().ok()), std::env::var_os("MM2_FILM_SIDE")) {
            (Some(w), _) => {
                let b = &race.racers[w.min(race.racers.len() - 1)].bike;
                let (at, f) = (b.position(), b.forward());
                (at - f * 16.0 + Vec3::Y * 7.0, at + Vec3::Y * 3.0)
            }
            (None, Some(_)) => {
                let b = &race.racers[race.player].bike;
                let at = b.position();
                (at + b.forward().cross(Vec3::Y) * 8.0 + Vec3::Y * 3.0, at + Vec3::Y * 2.5)
            }
            _ => chase,
        };
        let eye = renderer::from_game(chase.0);
        let shown = web_time::Instant::now();
        show(race, track, &mut self.renderer, eye, None);
        feel_wrecks(race, &self.renderer);
        let shown = shown.elapsed().as_secs_f64();
        if let Some(b) = &mut self.bench {
            let (w, h) = self.size;
            let mut camera = Camera::looking_at(eye, renderer::from_game(chase.1));
            camera.fov = race.fov();
            b.frame(&self.device, &self.queue, &mut self.renderer, (w, h), &camera, race.time, clock, self.step, shown);
            return Ok(());
        }
        if let Some(d) = &self.drive {
            drive_show(d, race, track, &mut self.renderer);
        }
        if let (Some((_, text)), race::Bike::Player(b)) = (&mut self.log, &race.racers[race.player].bike) {
            if let Some(w) = &b.wreck {
                use std::fmt::Write as _;
                let m = w.body();
                let _ = writeln!(
                    text,
                    "{clock:.4},{:?},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3}",
                    w.stage, w.motion, m.w_axis.x, m.w_axis.y, m.w_axis.z,
                    m.x_axis.x, m.x_axis.y, m.x_axis.z, m.y_axis.x, m.y_axis.y, m.y_axis.z, m.z_axis.x, m.z_axis.y, m.z_axis.z
                );
            }
        }
        let Some(dir) = &self.dir else { return Ok(()) };
        if clock + 1e-4 < self.next {
            return Ok(());
        }
        // MM2_FILM_FPS: frames a second (15 by default).
        self.next += 1.0 / std::env::var("MM2_FILM_FPS").ok().and_then(|f| f.parse::<f32>().ok()).unwrap_or(15.0);
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let (w, h) = self.size;
        let target = offscreen(&self.device, format, w, h);
        let view = target.create_view(&Default::default());
        let mut camera = Camera::looking_at(eye, renderer::from_game(chase.1));
        camera.fov = race.fov();
        let draw = self.renderer.render(&self.device, &self.queue, &view, (w, h), &camera, race.time);
        let encoder = self.device.create_command_encoder(&Default::default());
        self.queue.submit([draw]);
        let out = dir.join(format!("{:04}.png", self.shot));
        self.shot += 1;
        save_png(&self.device, &self.queue, encoder.finish(), &target, &out)
    }
}

impl Drop for Film {
    fn drop(&mut self) {
        if let Some(b) = &self.bench {
            b.report(&self.device, &mut self.renderer);
        }
        if let Some((path, text)) = &self.log {
            let _ = crate::fs::write(path, text);
        }
    }
}

/// The menus as they look after clicking at each `x,y` of `clicks`
/// (or typing `type:text`, pressing `key:z` or `esc`, or finishing the
/// race just started in a place, `race:1`)
/// (separated by `;`), a second apart.
async fn menu_screenshot(
    game: &Path,
    clicks: &str,
    out: &Path,
    results: Option<frontend::RaceResults>,
) -> Result<(), BoxError> {
    let (_, device, queue) = gpu(None).await?;
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut sprites = sprites::Sprites::new(&device, format);
    // MM2_SHOT_SCALE renders larger, as a window that size would.
    let scale: f32 = std::env::var("MM2_SHOT_SCALE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);
    sprites.scale = scale;
    let mut front =
        frontend::Frontend::new(game).ok_or("the game's UI/MAINMENU.DAT cannot be read")?;
    update_note::from_env();
    let audio = audio::Audio::new();
    if let Some(r) = results {
        front.race_over(r);
    }
    let mut now = 0.0;
    let mut started: Option<frontend::Setup> = None;
    for click in clicks.split(';').filter(|c| !c.trim().is_empty()) {
        // The race just started, over with the player in this place and
        // the computer riders behind or ahead in turn.
        if let Some(place) = click.strip_prefix("race:") {
            if let Some(setup) = started.take() {
                let riders = setup.opponents as usize + 1;
                let mine: usize = place.trim().parse().unwrap_or(1);
                let mut places = (1..=riders).filter(|&p| p != mine);
                let placings = (0..riders)
                    .map(|i| frontend::Placing {
                        place: if i == 0 {
                            mine
                        } else {
                            places.next().unwrap_or(riders)
                        },
                        ..Default::default()
                    })
                    .collect::<Vec<frontend::Placing>>();
                let mut lines: Vec<frontend::ResultLine> = placings
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let name = if i == 0 {
                            "You".to_string()
                        } else {
                            format!("Rider {i}")
                        };
                        let time = format!("1:{:02}.00", 20 + p.place);
                        [name, p.place.to_string(), time, String::new()]
                    })
                    .collect();
                lines.sort_by_key(|l| l[1].parse::<usize>().unwrap_or(0));
                front.race_over(frontend::RaceResults {
                    lines,
                    placings,
                    ..Default::default()
                });
            }
            continue;
        }
        // Typing, a key, or Esc, between the clicks.
        if let Some(text) = click.strip_prefix("type:") {
            text.chars().for_each(|c| front.type_char(c));
            front.type_char('\r');
            continue;
        }
        // Time passing, with the screen drawn (movies, animations).
        if let Some(t) = click
            .strip_prefix("wait:")
            .and_then(|t| t.trim().parse::<f32>().ok())
        {
            let mut gfx = ui::Gfx {
                sprites: &mut sprites,
                device: &device,
                queue: &queue,
            };
            // In steps, so what counts frame time (the clips' wait) sees it.
            let mut left = t;
            while left > 0.0 {
                let step = left.min(0.05);
                front.draw(&mut gfx, now, step);
                gfx.sprites.discard();
                now += step;
                left -= step;
            }
            continue;
        }
        if click.trim() == "intro" {
            front.intro();
            continue;
        }
        if click.trim() == "esc" {
            front.escape(now);
            continue;
        }
        if let Some(key) = click.strip_prefix("key:") {
            let code = match key {
                "z" => KeyCode::KeyZ,
                "space" => KeyCode::Space,
                "up" => KeyCode::ArrowUp,
                _ => KeyCode::KeyA,
            };
            front.key(&audio, code);
            continue;
        }
        // `wheel:N` turns the wheel N lines (up is positive).
        if let Some(n) = click.strip_prefix("wheel:").and_then(|n| n.trim().parse::<i32>().ok()) {
            front.wheel(n);
            continue;
        }
        // `drag:x,y,x2,y2` presses at x,y, moves to x2,y2 and lets go.
        if let Some(d) = click.strip_prefix("drag:") {
            let v: Vec<f32> = d.split(',').filter_map(|t| t.trim().parse().ok()).collect();
            let [x, y, x2, y2] = v[..] else { return Err("--clicks drag wants x,y,x2,y2".into()) };
            let mut gfx = ui::Gfx {
                sprites: &mut sprites,
                device: &device,
                queue: &queue,
            };
            front.draw(&mut gfx, now, 0.0);
            sprites.discard();
            front.mouse_move(&audio, x, y);
            front.mouse_down(x, y);
            front.mouse_move(&audio, (x + x2) / 2.0, (y + y2) / 2.0);
            front.mouse_move(&audio, x2, y2);
            front.mouse_up(&audio, x2, y2, now);
            now += 1.0;
            continue;
        }
        // `move:x,y` rests the pointer there without clicking.
        let (hover, click) = match click.strip_prefix("move:") {
            Some(c) => (true, c),
            None => (false, click),
        };
        let (x, y) = click
            .split_once(',')
            .and_then(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?)))
            .ok_or("--clicks wants x,y;x,y")?;
        // Draw once so lists know where their lines are.
        let mut gfx = ui::Gfx {
            sprites: &mut sprites,
            device: &device,
            queue: &queue,
        };
        front.draw(&mut gfx, now, 0.0);
        sprites.discard();
        front.mouse_move(&audio, x, y);
        if hover {
            continue;
        }
        front.mouse_down(x, y);
        if let frontend::Next::Race(setup) = front.mouse_up(&audio, x, y, now) {
            eprintln!("race: {setup:?}");
            started = Some(setup);
        }
        now += 1.0;
    }
    let (w, h) = ((640.0 * scale) as u32, (480.0 * scale) as u32);
    let target = offscreen(&device, format, w, h);
    let mut gfx = ui::Gfx {
        sprites: &mut sprites,
        device: &device,
        queue: &queue,
    };
    front.draw(&mut gfx, now, 0.5);    let mut encoder = device.create_command_encoder(&Default::default());
    let view = target.create_view(&Default::default());
    sprites.render(&device, &mut encoder, &view, (w, h), true);
    let mut commands = vec![encoder.finish()];
    // The Bike & Rider page's models, part way round.
    if let Some((ride, _)) = front.preview() {
        let res = read_archive(&game.join("RES").join("BIKE.RES"))?;
        let res = rs2::Archive::parse(&res)?;
        if let Some(mut p) = preview::Preview::new(&device, &queue, &res, &ride, format) {
            p.window = front.preview_window();
            commands.push(p.render(&device, &queue, &view, (w, h), 2.0, 1.0, true));
        }
    }
    let last = commands.pop().unwrap();
    queue.submit(commands);
    save_png(&device, &queue, last, &target, out)
}

fn offscreen(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("screenshot"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

/// A frame not drawn, in the log at most once a second.
fn skipped(why: &str) {
    static LAST: std::sync::Mutex<Option<Instant>> = std::sync::Mutex::new(None);
    let mut last = LAST.lock().unwrap();
    if last.is_none_or(|t| t.elapsed().as_secs_f32() > 1.0) {
        eprintln!("frame skipped: {why}");
        *last = Some(Instant::now());
    }
}

/// Saves the window's picture `frame` to the saves folder's Screenshots,
/// numbered after the last there (the screenshot key, an extra).
fn save_screenshot(device: &wgpu::Device, queue: &wgpu::Queue, frame: &wgpu::Texture) -> Result<PathBuf, BoxError> {
    if !frame.usage().contains(wgpu::TextureUsages::COPY_SRC) {
        return Err("this window's picture can't be read back".into());
    }
    let dir = mm2_settings::user_dir().join("Screenshots");
    crate::fs::create_dir_all(&dir)?;
    let out = (1..)
        .map(|n| dir.join(format!("OpenMCM2 {n:04}.png")))
        .find(|p| !p.exists())
        .expect("a free name");
    let none = device.create_command_encoder(&Default::default()).finish();
    save_png(device, queue, none, frame, &out)?;
    Ok(out)
}

/// Runs `draw` and saves `target` as a PNG.
fn save_png(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    draw: wgpu::CommandBuffer,
    target: &wgpu::Texture,
    out: &Path,
) -> Result<(), BoxError> {
    let (width, height) = (target.width(), target.height());
    let row = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (row * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([draw, encoder.finish()]);
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.expect("map readback"));
    device.poll(wgpu::PollType::wait_indefinitely())?;
    let data = readback.slice(..).get_mapped_range()?;
    let mut pixels: Vec<u8> = data
        .chunks(row as usize)
        .flat_map(|r| &r[..(width * 4) as usize])
        .copied()
        .collect();
    // A window's picture (the screenshot key) may be blue first, and its
    // alpha isn't meant to be seen.
    if matches!(target.format(), wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb) {
        pixels.chunks_exact_mut(4).for_each(|p| {
            p.swap(0, 2);
            p[3] = 255;
        });
    }
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(crate::fs::File::create(out)?),
        width,
        height,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.write_header()?.write_image_data(&pixels)?;
    eprintln!("wrote {}", out.display());
    Ok(())
}

/// The loading bar's steps so far: the game's bar is 20 steps long (0x451e30)
/// and steps one at each stage of the race's loading (0x49cb00, called from
/// 0x45d930, 0x418cd0 and 0x41a770), 11 of them in a race, so it shows a
/// little over half full when the race begins.
static LOAD_STEPS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
const LOAD_BAR_STEPS: u32 = 20;

/// Photo mode's line at the bottom when it starts, and how long a line
/// stays there.
const PHOTO_HELP: &str = "Photo mode: Enter or A saves a picture, F, Esc or B goes back to the race";
const PHOTO_NOTE_SECS: f32 = 5.0;
/// Photo mode's camera: its speed, with Shift (or RB) its speed, and how
/// fast it turns (radians a second).
const PHOTO_SPEEDS: (f32, f32, f32) = (25.0, 150.0, 1.2);

/// A step of a track's loading: the bar moves on, and in a browser the
/// page gets to draw (the loading runs between its frames, a step a frame).
async fn load_step() {
    LOAD_STEPS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    #[cfg(target_arch = "wasm32")]
    job::next_frame().await;
}

/// A step that the page names on its loading note (`web/index.html`
/// trackStage), as some take a while on a phone.
async fn load_stage(stage: &str) {
    #[cfg(target_arch = "wasm32")]
    web::track_stage(stage);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = stage;
    load_step().await;
}

/// The same in a browser alone, without a step of the game's bar.
async fn page_stage(stage: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        web::track_stage(stage);
        job::next_frame().await;
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = stage;
}

/// A track loading on another thread while the loading screen shows.
struct Loading {
    setup: frontend::Setup,
    job: job::Job<Result<Track, String>>,
}

struct App {
    track: Option<Track>,
    state: Option<State>,
    keys: HashSet<KeyCode>,
    /// The steering and lean keys as the game's key axes.
    steer_keys: input::KeyAxis,
    lean_keys: input::KeyAxis,
    /// The keyboard's keys for the 14 actions, from the profile.
    key_map: input::KeyMap,
    /// The profile's joystick map (its current device, or the first
    /// joystick when that is the keyboard), and its Analog Axes switch.
    pad_map: [(i32, i32); profile::ACTIONS],
    analog: bool,
    /// Each raced frame's keys, controls and the player's state
    /// (`MM2_RIDE_LOG=FILE`), to set a live ride beside a tape of the
    /// game's.
    ride_log: Option<std::io::BufWriter<crate::fs::File>>,
    /// A chat line being typed in a network race (`/`, 0x4203e0).
    chat: Option<String>,
    /// Ctrl+S has turned the race's sounds off.
    sound_off: bool,
    /// Ctrl+V's arena reverb, heard only with EAX on the Sound page; it
    /// starts on in Supercross and Moto-Tag (0x4e81a0's +0x54).
    reverb: bool,
    /// F4 Visual Cues; a new profile starts with them on (+0x6a8), and the
    /// profile file does not keep them.
    visual_cues: bool,
    looking: bool,
    last: Instant,
    /// Evens the frame times to the screen's beat (`pace`).
    pace: pace::Pace,
    /// Dynamic resolution's governor (`dynres`), while racing.
    dynres: dynres::Governor,
    /// In a browser, the whole picture's size while racing (`dynres::Picture`).
    picture: dynres::Picture,
    /// The screen's beat, s, and when it was last asked for.
    beat: (Option<Instant>, Option<f32>),
    /// F2 (or the Show frame rate tweak): the frame rate, at the bottom
    /// of the screen and in the terminal, each second: since when, the
    /// frames and the slowest.
    fps: Option<(Instant, u32, f32)>,
    /// The frame rate shown, from the last whole second.
    fps_shown: Option<u32>,
    start: Instant,
    race: Option<Race>,
    /// The player out of the saddle, at the wheel of one of the track's
    /// vehicles (crate::drive).
    drive: Option<drive::Drive>,
    /// The driven vehicle's engine and tyres.
    drive_sound: drive_sound::DriveSound,
    /// The camera follows the bike rather than flying free.
    follow: bool,
    /// The window title last set, to change it only when the race status
    /// changes.
    title: String,
    /// The menus, when the game started from them.
    front: Option<frontend::Frontend>,
    loading: Option<Loading>,
    audio: audio::Audio,
    sounds: Option<sound::RaceSounds>,
    /// The in-race display, with the game's text and pictures.
    hud: Option<(ui::Ui, hud::Hud)>,
    /// The last frame's length, seconds.
    frame: f32,
    /// The pointer on the 640×480 screen.
    pointer: (f32, f32),
    /// The pad's A and B last frame, to click and go back in the menus.
    pad_click: (bool, bool),
    /// The stick's smoothed steering and when it was last read (the pad
    /// smoothing extra).
    pad_smooth: (f32, Option<Instant>),
    /// Whether the log has been told of the pad leaning the rider.
    pad_lean_logged: bool,
    /// The race going on tape, and a tape playing (with the track it
    /// waits for while that loads).
    recording: Option<replay::Recording>,
    ghost: Option<replay::Ghost>,
    replay: Option<replay::Replay>,
    tape: Option<replay::Replay>,
    /// The race's event, for its podium.
    event: Option<frontend::EventKind>,
    /// Where the race's result goes in the track records, unless it is
    /// practice, a ghost race, a replay or over the network.
    records: Option<screens::records::Entry>,
    /// The lap leaderboard this race's laps go on (the extra), and the
    /// player's rider name for it.
    lap_board: Option<(mm2_settings::laps::Board, String)>,
    /// The player's laps on tape, for the online ghosts (the extra).
    lap_tapes: Option<replay::LapTapes>,
    /// A network race: whose bikes are whose, the other machines' bikes
    /// as they arrive, and the wait to send ours again.
    net: Option<frontend::NetRace>,
    remote: Vec<replay::Remote>,
    send_wait: f32,
    /// The profile's JoyDirectionFlipped setting: the stick's Y axis
    /// the other way round (0x4c6ec0).
    joy_flipped: bool,
    #[cfg(feature = "gamepad")]
    pads: Option<pad::Pads>,
    #[cfg(not(feature = "gamepad"))]
    pads: Option<()>,
    /// The pad's vibration from the player's riding, and this frame's
    /// strengths of its heavy and light motors.
    rumble: rumble::Rumble,
    shake: [f32; 2],
    /// The window out of focus or covered: the pad keeps still.
    quiet: bool,
    /// The screenshot key was pressed: save the next frame.
    shoot: bool,
    /// Photo mode (an extra): the race held still, the display hidden and
    /// the camera flying free; whether it followed the bike before, and
    /// the line shown at the bottom until when.
    photo: bool,
    photo_follow: bool,
    photo_note: Option<(String, Instant)>,
    /// The Course Builder (an extra), open over the menus.
    builder: Option<builder::Builder>,
    /// The Course Builder was asked for while the page was still reading
    /// files in (the browser): it opens on this once they are.
    builder_waiting: Option<Option<PathBuf>>,
    /// Leave the game at the end of this frame (the builder on its own
    /// was closed).
    quit: bool,
}

/// The menu button the D-pad jumps to from `at`, in direction `dir`:
/// the nearest ahead, a step off to the side counting double.
#[cfg_attr(not(feature = "gamepad"), allow(dead_code))]
fn pad_jump(at: (f32, f32), dir: (f32, f32), targets: &[(f32, f32)]) -> Option<(f32, f32)> {
    targets
        .iter()
        .copied()
        .filter_map(|t| {
            let (dx, dy) = (t.0 - at.0, t.1 - at.1);
            let ahead = dx * dir.0 + dy * dir.1;
            let side = (dx * dir.1 - dy * dir.0).abs();
            (ahead > 4.0).then_some((ahead + 2.0 * side, t))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, t)| t)
}

/// The device whose map the race uses when the profile's own is the
/// keyboard: the joystick's kind, or the first joystick's map.
#[cfg(feature = "gamepad")]
fn pad_device(pads: &Option<pad::Pads>) -> usize {
    match pads {
        Some(p) if p.pad().is_some() => p.device(),
        _ => 1,
    }
}

#[cfg(not(feature = "gamepad"))]
fn pad_device(_pads: &Option<()>) -> usize {
    1
}

struct State {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    /// The track's renderer, while racing.
    renderer: Option<TerrainRenderer>,
    sprites: sprites::Sprites,
    /// The race between black bars on a window wider than 21:9.
    bars: bars::Bars,
    /// The picture extras' pass (edge smoothing, sharpening), made when
    /// first wanted.
    polish: Option<polish::Polish>,
    camera: Camera,
    /// The Bike & Rider page's turning models, and `RES/BIKE.RES` for them.
    preview: Option<preview::Preview>,
    bike_res: Option<Vec<u8>>,
}

impl State {
    /// Makes the surface anew at the window's size; never while a frame
    /// is out.
    fn fit_surface(&mut self) {
        let size = self.window.inner_size();
        (self.config.width, self.config.height) = drawn_size(size.width, size.height);
        self.surface.configure(&self.device, &self.config);
    }
}

impl App {
    fn new(
        track: Option<Track>,
        race: Option<Race>,
        mut front: Option<frontend::Frontend>,
    ) -> Self {
        playclock::start();
        let audio = audio::Audio::new();
        if let Some(f) = &mut front {
            f.set_audio(&audio);
        }
        let sounds = track
            .as_ref()
            .and_then(|t| t.res.as_deref())
            .map(|res| {
                sound::RaceSounds::load(
                    res,
                    &audio,
                    &[sound::class(250, false)],
                    0,
                    frontend::EventKind::Nationals,
                    false,
                    sound::Options::default(),
                )
            });
        let hud = track.as_ref().and_then(Track::hud);
        App {
            track,
            state: None,
            keys: HashSet::new(),
            steer_keys: input::KeyAxis::default(),
            lean_keys: input::KeyAxis::default(),
            key_map: input::KeyMap::factory(),
            pad_map: input::factory_pad(),
            analog: true,
            ride_log: std::env::var("MM2_RIDE_LOG").ok().and_then(|p| crate::fs::File::create(p).ok()).map(|f| {
                use std::io::Write as _;
                let mut w = std::io::BufWriter::new(f);
                let _ = writeln!(w, "t,dt,keys,x,y,z,roll,pitch,heading,vx,vy,vz,speed,gear,rpm,crashed,bars,lean,front_down,rear_down,front_squeeze,rear_squeeze");
                w
            }),
            chat: None,
            sound_off: false,
            reverb: false,
            visual_cues: true,
            looking: false,
            last: Instant::now(),
            pace: pace::Pace::default(),
            dynres: dynres::Governor::default(),
            picture: dynres::Picture::default(),
            beat: (None, None),
            fps: tweaks::get().show_fps.then(|| (Instant::now(), 0, 0.0)),
            fps_shown: None,
            start: Instant::now(),
            follow: race.is_some(),
            race,
            drive: None,
            drive_sound: drive_sound::DriveSound::new(),
            title: String::new(),
            front,
            loading: None,
            audio,
            hud,
            frame: 0.0,
            sounds,
            pointer: (0.0, 0.0),
            pad_click: (false, false),
            pad_smooth: (0.0, None),
            pad_lean_logged: false,
            recording: None,
            ghost: None,
            replay: None,
            tape: None,
            event: None,
            records: None,
            lap_board: None,
            lap_tapes: None,
            net: None,
            remote: Vec::new(),
            send_wait: 0.0,
            joy_flipped: false,
            #[cfg(feature = "gamepad")]
            pads: pad::Pads::new(),
            #[cfg(not(feature = "gamepad"))]
            pads: None,
            rumble: rumble::Rumble::default(),
            shake: [0.0; 2],
            quiet: false,
            shoot: false,
            photo: false,
            photo_follow: false,
            photo_note: None,
            builder: None,
            builder_waiting: None,
            quit: false,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn init(&mut self, event_loop: &ActiveEventLoop) -> Result<State, BoxError> {
        let (window, instance, surface) = Self::open_window(event_loop)?;
        let gpu = pollster::block_on(gpu_from(&instance, Some(&surface)))?;
        self.make_state(window, surface, gpu)
    }

    /// The window, and the GPU's way to draw in it.
    fn open_window(
        event_loop: &ActiveEventLoop,
    ) -> Result<(Arc<Window>, wgpu::Instance, wgpu::Surface<'static>), BoxError> {
        let attrs = Window::default_attributes()
            .with_title("Motocross Madness 2")
            .with_inner_size(window_size())
            .with_window_icon(window_icon());
        // `--fullscreen`: the whole screen, as Alt+Enter gives; with
        // `--screen WxH`, at that resolution if the screen has it.
        let attrs = if crate::args().any(|a| a == "--fullscreen") {
            attrs.with_fullscreen(Some(full_screen(event_loop)))
        } else {
            attrs
        };
        // In a browser, the page's canvas, sized by the page.
        #[cfg(target_arch = "wasm32")]
        let attrs = {
            use wasm_bindgen::JsCast;
            use winit::platform::web::WindowAttributesExtWebSys;
            let canvas = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.get_element_by_id("mm2"))
                .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok());
            let mut attrs = attrs.with_canvas(canvas);
            attrs.inner_size = None;
            attrs
        };
        let window = Arc::new(event_loop.create_window(attrs)?);
        if let Some(m) = window.current_monitor().or_else(|| event_loop.primary_monitor()) {
            graphics::set_screen((m.size().width, m.size().height));
        }
        // The game's own is drawn instead, so never two.
        if ui::draws_pointer() {
            window.set_cursor_visible(false);
        }
        #[cfg(windows)]
        {
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(RawWindowHandle::Win32(h)) = window.window_handle().map(|h| h.as_raw()) {
                winwatch::start(h.hwnd.get());
            }
        }
        let display = Box::new(event_loop.owned_display_handle());
        let instance = wgpu::Instance::new(
            wgpu::InstanceDescriptor::new_with_display_handle_from_env(display),
        );
        let surface = instance.create_surface(window.clone())?;
        Ok((window, instance, surface))
    }

    fn make_state(
        &mut self,
        window: Arc<Window>,
        surface: wgpu::Surface<'static>,
        (adapter, device, queue): (wgpu::Adapter, wgpu::Device, wgpu::Queue),
    ) -> Result<State, BoxError> {
        let size = window.inner_size();
        let (w, h) = drawn_size(size.width, size.height);
        let mut config = surface
            .get_default_config(&adapter, w, h)
            .ok_or("this GPU cannot draw to the window")?;
        let caps = surface.get_capabilities(&adapter);
        config.format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(config.format);
        // A browser's canvas has no sRGB format: the game draws through an
        // sRGB view of it instead, or every colour comes out too dark.
        if !config.format.is_srgb() {
            config.view_formats = vec![config.format.add_srgb_suffix()];
        }
        config.present_mode = present_mode();
        // The race is copied in between black bars on a window wider than 21:9.
        if caps.usages.contains(wgpu::TextureUsages::COPY_DST) {
            config.usage |= wgpu::TextureUsages::COPY_DST;
        }
        // The screenshot key reads the window's picture back.
        if caps.usages.contains(wgpu::TextureUsages::COPY_SRC) {
            config.usage |= wgpu::TextureUsages::COPY_SRC;
        }
        surface.configure(&device, &config);
        let sprites = sprites::Sprites::new(&device, config.format.add_srgb_suffix());
        let mut state = State {
            window,
            surface,
            device,
            queue,
            config,
            renderer: None,
            sprites,
            bars: bars::Bars::default(),
            polish: None,
            camera: Camera::looking_at(Vec3::ZERO, Vec3::Z),
            preview: None,
            bike_res: None,
        };
        if let Some(track) = &self.track {
            Self::show_track(&mut state, track, self.visibility());
        }
        Ok(state)
    }

    /// The Visibility detail, 0 to 9: the Advanced page's slider, else the
    /// overall graphic quality (global +0x1ee4).
    fn visibility(&self) -> u32 {
        let default = profile::GraphicsOptions::default().terrain;
        self.front.as_ref().map_or(default, |f| {
            let u = f.user();
            u.settings.number("SldVisibility", u.profile.graphics.terrain as i64).clamp(0, 9) as u32
        })
    }

    fn show_track(state: &mut State, track: &Track, quality: u32) {
        let mut renderer = TerrainRenderer::new(
            &state.device,
            &state.queue,
            &track.terrain,
            &track.terrain_textures,
            &track.objects,
            state.config.format.add_srgb_suffix(),
        );
        if let Some((cube, images)) = &track.sky {
            renderer.set_sky(
                &state.device,
                &state.queue,
                cube,
                images,
                state.config.format.add_srgb_suffix(),
            );
        }
        renderer.set_lights(track.lights);
        renderer.set_desert(track.desert);
    renderer.set_stadium(track.supercross);
        renderer.set_night(track.night_lights.clone(), track.floor());
        renderer.set_crowd(&state.device, &track.spectators);
        renderer.set_water(&state.device, &track.water);
        if let Some(detail) = &track.detail {
            renderer.set_detail(&state.device, &state.queue, detail);
        }
        if let Some(map) = &track.ground_map {
            renderer.set_ground_map(&state.device, &state.queue, map);
        }
        if let Some(fog) = track.fog {
            renderer.set_fog(fog);
        }
        if let Some((visibility, haziness)) = track.haze {
            renderer.set_visibility(quality, visibility, haziness);
        }
        if let Some(sheet) = &track.particle_sheet {
            renderer.set_dirt(&state.device, &state.queue, &sheet, state.config.format.add_srgb_suffix());
        }
        state.renderer = Some(renderer);
        state.camera = renderer::start_camera(&track.terrain);
    }

    /// Starts loading the chosen race's track.
    fn load(&mut self, setup: frontend::Setup) {
        let (path, course, riders) = (
            setup.track.clone(),
            setup.course,
            setup.opponents as usize + 1,
        );
        // A ghost race's ghost is a second bike like the player's.
        let ghost = setup.mode == frontend::GHOST;
        let riders = if ghost { 2 } else { riders };
        let me = setup.net.map_or(PLAYER, |n| n.me);
        let mut rides = setup.rides.clone();
        rides.resize(riders.max(1), None);
        rides[PLAYER] = setup.ride.clone();
        if ghost {
            rides[1] = setup.ride.clone();
        }
        LOAD_STEPS.store(0, std::sync::atomic::Ordering::Relaxed);
        let random = setup.random_gates;
        let job = job::Job::spawn_steps(move || async move {
            load_track_steps(&path, course, riders, rides, me, ghost.then_some(1), random)
                .await
                .map_err(|e| e.to_string())
        });
        self.loading = Some(Loading {
            setup,
            job,
        });
        #[cfg(target_arch = "wasm32")]
        web::track_loading(true);
    }

    /// Once the track has loaded, the race begins.
    fn finish_loading(&mut self) {
        if !self.loading.as_ref().is_some_and(|l| l.job.is_finished()) {
            return;
        }
        let Loading { setup, job, .. } = self.loading.take().unwrap();
        #[cfg(target_arch = "wasm32")]
        web::track_loading(false);
        match job.join() {
            Ok(Ok(mut track)) => {
                track.name = setup.name.clone();
                // Practice has no end.
                let practice = setup.mode == frontend::PRACTICE || setup.mode == frontend::GHOST;
                let stunt = setup.event == frontend::EventKind::Stunt;
                let tag = setup.event == frontend::EventKind::Tag;
                let laps = if practice || stunt || tag {
                    race::ENDLESS
                } else {
                    setup.laps
                };
                self.net = setup.net;
                self.event = Some(setup.event);
                // A race on random gates goes in no records (0x49d180).
                let counts = !practice && setup.net.is_none() && self.tape.is_none() && setup.random_gates.is_none();
                self.records = counts.then(|| screens::records::Entry {
                    event: setup.event as u32,
                    track: setup.track.clone(),
                    course: setup.course,
                    title: setup.name.clone(),
                    length: setup.laps,
                });
                // Laps go on the leaderboards in the lap races, only as
                // the original rides: with the riding extras off.
                let t = tweaks::get();
                let fair = t.air_steer == 0 && t.air_brake == 0 && !t.quick_get_up && !t.forgiving_landings;
                let lap_event = matches!(setup.event, frontend::EventKind::Baja | frontend::EventKind::Nationals | frontend::EventKind::Supercross | frontend::EventKind::Enduro);
                playclock::race_began(self.tape.is_none().then(|| {
                    let cc = setup.ride.as_ref().map_or(250, |r| r.bike.cc);
                    mm2_settings::playtime::race_keys(setup.event as u32, &setup.track, cc)
                }));
                self.lap_board = (counts && fair && lap_event && tweaks::more().leaderboards).then(|| {
                    let cc = setup.ride.as_ref().map_or(250, |r| r.bike.cc);
                    (mm2_settings::laps::Board::new(&setup.track, setup.event as u32, setup.course as u32, cc), setup.name.clone())
                });
                self.race = make_race_as(&track, laps, setup.net.map_or(PLAYER, |n| n.me));
                // The online ghosts: the player's laps on tape, so the
                // best can go up with its lap.
                self.lap_tapes = match (&self.race, &self.lap_board) {
                    (Some(r), Some(_)) if tweaks::more().online_ghosts && crate::online::address().is_some() => {
                        let mut h = heading(&setup, r);
                        h.riders = h.riders.into_iter().nth(r.player).into_iter().collect();
                        h.mode = frontend::GHOST;
                        Some(replay::LapTapes::new(replay::header(h)))
                    }
                    _ => None,
                };
                self.replay = self.tape.take();
                self.remote.clear();
                if let (Some(n), Some(r)) = (self.net, &mut self.race) {
                    self.remote = (0..r.racers.len())
                        .map(|_| replay::Remote::default())
                        .collect();
                    // The other machines' bikes wait at the start for them.
                    for (i, racer) in r.racers.iter_mut().enumerate() {
                        if !owned(n, i) {
                            racer.bike = replay::taped_like(&racer.bike);
                        }
                    }
                }
                let ghost = setup.mode == frontend::GHOST;
                self.recording = match (&self.race, &self.replay) {
                    (Some(r), None) if !ghost => Some(replay::Recording::new(heading(&setup, r))),
                    _ => None,
                };
                self.ghost = None;
                if let (Some(r), true) = (&mut self.race, ghost) {
                    r.ghost = Some(1);
                    let mut h = heading(&setup, r);
                    h.riders.truncate(1);
                    let saved = setup.ghost.as_ref().and_then(|p| {
                        let bytes = crate::fs::read(p).ok()?;
                        mm2_formats::vcr::Tape::parse(&bytes).ok()
                    });
                    self.ghost = Some(replay::Ghost::new(1, replay::header(h), saved, 0.0));
                }
                if let Some(r) = &mut self.race {
                    r.practice = practice;
                    // Practice and ghost races have no countdown (race
                    // +0x15c stays 0, 0x421f90).
                    if practice {
                        r.lead = 0.0;
                    }
                    r.stunt_event = stunt;
                    r.minutes = ((stunt || tag) && !practice).then_some(setup.laps as f32);
                    if tag {
                        let n = setup.net;
                        let mut t = tag::Tag::new(
                            r.racers.len(),
                            n.is_some_and(|n| n.tag_ball),
                            n.is_some_and(|n| n.tag_stunts),
                        );
                        if let Some(n) = n {
                            t.judge = n.host;
                            t.local = (0..r.racers.len()).map(|i| owned(n, i)).collect();
                        }
                        r.tag = Some(t);
                    }
                    r.level = setup.level;
                    r.names = setup.names.clone();
                    // The profile's keys and gyro options.
                    if let Some(f) = &self.front {
                        let p = &f.user().profile;
                        r.wreck_reset = p.game.wreck_reset || setup.net.is_some();
                        r.quick_get_up = setup.net.is_none() && tweaks::get().quick_get_up;
                        r.player_indicator = p.game.player_indicator;
                        r.visual_cues = self.visual_cues;
                        let map = input::KeyMap::from_profile(&p.keys);
                        self.key_map = if map.0.iter().any(Option::is_some) {
                            map
                        } else {
                            input::KeyMap::factory()
                        };
                        // The joystick's map: the one the controls page
                        // shows if a joystick's, else the pad plugged in.
                        let joy = |d: usize| p.keys.keys.get(d).filter(|m| m.iter().any(|&(k, _)| k == 2));
                        let shown = p.controls.device as usize;
                        self.joy_flipped = f.user().settings.number("JoyDirectionFlipped", 0) != 0;
                        self.pad_map = match (shown < input::KEYBOARD).then(|| joy(shown)).flatten() {
                            Some(m) => *m,
                            None => joy(pad_device(&self.pads)).copied().unwrap_or_else(input::factory_pad),
                        };
                        // A game pad gets buttons for the actions left on keys.
                        if pad_device(&self.pads) == 3 {
                            self.pad_map = input::with_pad_extras(self.pad_map);
                        }
                        // For a player's log: the map the race reads.
                        eprintln!(
                            "joystick map: device {} (shown {shown}), analog {}, flipped {}: {:?}",
                            pad_device(&self.pads),
                            p.controls.analog_axes,
                            self.joy_flipped,
                            self.pad_map
                        );
                        self.analog = p.controls.analog_axes;
                        if let race::Bike::Player(b) = &mut r.racers[r.player].bike {
                            b.gas_gyro = p.controls.gas_gyro;
                            b.brake_gyro = p.controls.brake_gyro;
                            // The sliders (0x496a50).
                            let c = &p.controls;
                            b.sliders = mm2_physics::bike::Sliders::from_values(
                                c.steering,
                                c.cross_up,
                                c.pitch,
                                c.press_brakes,
                                c.release_brakes,
                            );
                            // The garage's curve, once a bike has put one
                            // there; else the engine's own (0x4d7090).
                            let g = &p.garage;
                            if g.ride_rpm[1] > 0 && g.ride_rpm[1] > g.ride_rpm[0] {
                                let curve = Spec {
                                    rpm_low: g.ride_rpm[0] as f32,
                                    rpm_high: g.ride_rpm[1] as f32,
                                    power: g.ride_power.map(|v| v as f32),
                                    ..b.spec.clone()
                                };
                                b.drivetrain = mm2_physics::drivetrain::Drivetrain::new(&curve, 1.0, 1.0);
                            }
                            b.drivetrain.open_rate = mm2_physics::bike::slider(c.press_throttle);
                            b.drivetrain.close_rate = mm2_physics::bike::slider(c.release_throttle);
                        }
                    }
                    // The shipped Pro Circuit tables' stunt money for the event.
                    r.purse = (setup.event != frontend::EventKind::Stunt).then(|| {
                        mm2_physics::career::Tables::shipped().stunt_purse(setup.event as usize)
                    });
                }
                // Each racer's engine class, from their bike (the default
                // grid rides 250s).
                let player = self.race.as_ref().map_or(PLAYER, |r| r.player);
                let classes: Vec<usize> = (0..self.race.as_ref().map_or(1, |r| r.racers.len()))
                    .map(|i| {
                        let ride = if i == player { setup.ride.as_ref() } else { setup.rides.get(i).and_then(Option::as_ref) };
                        ride.map_or(sound::class(250, false), |r| sound::class(r.bike.cc, r.bike.four_stroke))
                    })
                    .collect();
                let options = self
                    .front
                    .as_ref()
                    .map_or_else(Default::default, |f| sound::Options::from_profile(&f.user().profile.sound));
                self.sound_off = false;
                self.reverb = matches!(setup.event, frontend::EventKind::Supercross | frontend::EventKind::Tag);
                self.apply_reverb();
                self.sounds = sound_res(track.res.as_deref()).map(|res| {
                    let mut s = sound::RaceSounds::load(&res, &self.audio, &classes, player, setup.event, track.crowd, options);
                    s.set_scene(track.sounds.iter().cloned());
                    s.whistle = track.free && tweaks::more().whistle;
                    s
                });
                self.follow = self.race.is_some();
                let quality = self.visibility();
                if let Some(state) = &mut self.state {
                    Self::show_track(state, &track, quality);
                }
                self.hud = track.hud();
                if let (Some((_, hud)), Some(f)) = (&mut self.hud, &self.front) {
                    hud.show = hud::Show::from_profile(&f.user().profile);
                }
                self.track = Some(track);
                self.start = Instant::now();
            }
            Ok(Err(e)) => {
                eprintln!("{}: {e}", setup.track.display());
                #[cfg(target_arch = "wasm32")]
                web::track_failed(&setup.name, &format!("{}: {e}", setup.track.display()));
                self.back_to_menu();
            }
            Err(_) => self.back_to_menu(),
        }
    }

    /// Whether a race played from the menus is over by itself (0x45feb0,
    /// run each frame): a lap or gate race the frame after the player
    /// crosses the line for the last time, once 0x497740 has said the
    /// place; a Stunt Quarry or tag session when its minutes are up.
    /// Practice and ghost races never end, and a network race is left to
    /// Esc here.
    fn race_done(&self) -> bool {
        let Some(r) = &self.race else { return false };
        if self.front.is_none() || self.replay.is_some() || self.net.is_some() || r.practice || r.podium.is_some() {
            return false;
        }
        match r.time_left() {
            Some(left) => left <= 0.0,
            None => r.racers[r.player].finished.is_some(),
        }
    }

    /// Ends the race: with the podium first when the player made the top
    /// three of a finished race and the Podiums option is on (0x45f610,
    /// 0x45e490); Esc again leaves it.
    fn end_race(&mut self) {
        self.leave_vehicle(false);
        let (Some(r), Some(track), Some(f)) = (&mut self.race, &self.track, &self.front) else {
            return self.back_to_menu();
        };
        let me = &r.racers[r.player];
        let over = me.finished.is_some() || r.time_left().is_some_and(|t| t <= 0.0);
        let top = self.net.is_some() || r.position() <= 3;
        let wanted = f.user().profile.game.podiums && self.replay.is_none();
        let event = self.event.unwrap_or(frontend::EventKind::Nationals);
        match &track.podium {
            Some(stand) if r.podium.is_none() && wanted && over && top && !r.practice => {
                r.podium = Some((podium_for(track, stand, event, rand_unit()), r.time));
                if let Some(s) = &mut self.sounds {
                    s.podium(&self.audio);
                }
                self.follow = true;
            }
            _ => self.back_to_menu(),
        }
    }

    /// Leaves the race: for the Race Info page with its results, or the
    /// main menu when the track did not load.
    /// Writes down the time played since last time: the game open, and
    /// the race just left.
    fn note_playtime(&mut self) {
        playclock::note(&self.play_dir(), true);
    }

    /// The folder the playtime is kept in: the user folder, with the
    /// saves, menus or none (the free graphics' Course Builder on its own).
    fn play_dir(&self) -> PathBuf {
        playclock::dir()
    }

    fn back_to_menu(&mut self) {
        self.leave_vehicle(false);
        self.note_playtime();
        self.net = None;
        self.remote.clear();
        // The player's best lap (stunt) and whole race go in the track's
        // records.
        if let (Some(entry), Some(race), Some((ui, _)), Some(f)) = (self.records.take(), &self.race, &self.hud, &self.front) {
            let me = &race.racers[race.player];
            let result = if race.stunt_event {
                screens::records::Outcome {
                    best: Some(me.best_stunt),
                    whole: race.time_left().is_some_and(|t| t <= 0.0).then_some(me.stunt_points),
                }
            } else {
                screens::records::Outcome {
                    best: Some(me.timing.best_lap),
                    whole: me.timing.finished.then_some(me.timing.race_time),
                }
            };
            let name = rider_name(race, ui, race.player);
            if let Err(e) = screens::records::enter(&f.user().store.dir, &entry, &name, result) {
                eprintln!("the track records: {e}");
            }
        }
        // The laps that count go on the lap leaderboards: from the second
        // (the first starts from the gate), and in Nationals and
        // Supercross not with much of it off the course. Baja and Enduro
        // are ridden between checkpoints, away from the course's line.
        if let (Some((board, title)), Some(race), Some((ui, _)), Some(f)) = (self.lap_board.take(), &self.race, &self.hud, &self.front) {
            let me = &race.racers[race.player];
            let kept_on = mm2_settings::laps::keeps_on_course(board.event);
            let rider = rider_name(race, ui, race.player);
            let indexed: Vec<_> = me
                .lap_times
                .iter()
                .enumerate()
                .skip(1)
                .filter(|&(i, _)| !kept_on || me.lap_off.get(i).is_none_or(|&off| off <= mm2_settings::laps::MOST_OFF_COURSE))
                .map(|(i, &t)| (i, mm2_settings::laps::Lap { board: board.clone(), rider: rider.clone(), ms: (t * 1000.0).round() as u32, title: title.clone() }))
                .filter(|(_, l)| l.ms >= mm2_settings::laps::SHORTEST)
                .collect();
            let laps: Vec<_> = indexed.iter().map(|(_, l)| l.clone()).collect();
            if !laps.is_empty() {
                let dir = &f.user().store.dir;
                let old = |n: &str| crate::fs::read_to_string(dir.join(n)).unwrap_or_default();
                if let Some((best, pending)) = mm2_settings::laps::enter(&old(mm2_settings::laps::BEST), &old(mm2_settings::laps::PENDING), &laps) {
                    let _ = crate::fs::create_dir_all(dir);
                    for (n, t) in [(mm2_settings::laps::BEST, best.clone()), (mm2_settings::laps::PENDING, pending)] {
                        if let Err(e) = crate::fs::write(dir.join(n), t) {
                            eprintln!("the lap leaderboards: {e}");
                        }
                    }
                    // The online ghosts: a new best lap goes up with its
                    // ghost, if it was taped.
                    let fastest = indexed.iter().min_by_key(|(_, l)| l.ms);
                    if let (Some((i, lap)), Some(tapes)) = (fastest, &mut self.lap_tapes) {
                        let is_best = mm2_settings::laps::parse_all(&best).contains(lap);
                        if let (true, Some(tape)) = (is_best, tapes.take(*i)) {
                            if let Err(e) = crate::online::keep_ghost(dir, lap, &tape) {
                                eprintln!("the online ghosts: {e}");
                            }
                        }
                    }
                }
            }
        }
        self.lap_tapes = None;
        // New laps and playtime go to the leaderboard server now (the
        // browser has no launcher to send them).
        crate::online::send_saved(&self.play_dir());
        let mut results = match (&self.race, &self.hud) {
            (Some(race), Some((ui, _))) => Some(results(race, ui)),
            _ => None,
        };
        // The race's tape is kept for the View Replay button.
        if let (Some(rec), Some(r), Some(f)) = (self.recording.take(), &mut results, &self.front) {
            let path = replay::tape_path(&f.user().store.dir);
            let written = path
                .parent()
                .map_or(Ok(()), crate::fs::create_dir_all)
                .and_then(|_| crate::fs::write(&path, rec.finish().write()));
            match written {
                Ok(()) => r.replay = Some(path),
                Err(e) => eprintln!("{}: {e}", path.display()),
            }
        }
        // A ghost race offers to keep a lap that beat the ghost it started
        // with, once the results are up.
        let new_ghost = self
            .ghost
            .take()
            .filter(|g| g.improved)
            .and_then(|g| g.best().cloned());
        let replayed = self.replay.take().is_some() || self.tape.take().is_some();
        self.race = None;
        self.track = None;
        self.follow = false;
        if let Some(s) = &mut self.sounds {
            s.stop(&self.audio);
        }
        self.sounds = None;
        self.hud = None;
        self.audio.stop_all();
        self.audio.set_reverb(None);
        if let Some(state) = &mut self.state {
            state.renderer = None;
            state.window.set_title("Motocross Madness 2");
        }
        self.title.clear();
        // A ride from the Course Builder goes back to it.
        if let Some(b) = &mut self.builder {
            b.back();
            self.keys.clear();
            return;
        }
        if let Some(f) = &mut self.front {
            match results {
                _ if replayed => f.replay_over(),
                Some(r) => f.race_over(r),
                None => f.main_menu(),
            }
            if let Some(lap) = new_ghost {
                f.ask_ghost(lap);
            }
        }
    }

    /// Plays a tape: its track loads with its riders, then the tape runs.
    fn start_replay(&mut self, path: &Path) {
        let Some(tape) = replay::Replay::open(path) else {
            if let Some(f) = &mut self.front {
                f.replay_over();
            }
            return;
        };
        let game = self
            .front
            .as_ref()
            .and_then(|f| f.game())
            .map(Path::to_path_buf);
        let Some(setup) = game.and_then(|g| replay_setup(&g, &tape.tape)) else {
            eprintln!("{}: the tape's track is not here", path.display());
            if let Some(f) = &mut self.front {
                f.replay_over();
            }
            return;
        };
        if let Some(f) = &mut self.front {
            f.loading(setup.event);
        }
        self.tape = Some(tape);
        self.load(setup);
    }

    /// The menus are showing (not a race, nor a track loading).
    /// What the box being typed in holds: a chat line, or one of the
    /// menus' boxes.
    #[cfg(target_arch = "wasm32")]
    fn typing_text(&mut self) -> Option<String> {
        if let Some(line) = &self.chat {
            return Some(line.clone());
        }
        if self.in_builder() {
            return self.builder.as_ref().and_then(|b| b.typing_text());
        }
        if !self.in_menus() {
            return None;
        }
        self.front.as_mut().and_then(|f| f.typing_text())
    }

    fn in_menus(&self) -> bool {
        self.front.is_some() && self.track.is_none() && self.loading.is_none() && self.builder.is_none()
    }

    /// The Course Builder is showing (not riding its track).
    fn in_builder(&self) -> bool {
        self.builder.is_some() && self.track.is_none() && self.loading.is_none()
    }

    /// Opens the Course Builder on `file`, or on the last track made.
    fn open_builder(&mut self, file: Option<&Path>) {
        // It draws the scenery and ground, which may not be read in yet.
        #[cfg(target_arch = "wasm32")]
        if !crate::fs::all_read() {
            self.builder_waiting = Some(file.map(Path::to_path_buf));
            web::track_loading(true);
            return;
        }
        let game = self.front.as_ref().and_then(|f| f.game()).map(Path::to_path_buf);
        self.keys.clear();
        self.builder = Some(builder::Builder::new(game, file));
    }

    /// The builder's input; true when it took the event.
    fn builder_event(&mut self, event: &WindowEvent) -> bool {
        let Some(b) = &mut self.builder else { return false };
        match event {
            // Away from the window (Steam's overlay, another app), no key,
            // button or drag stays held: one whose release went elsewhere
            // would keep turning or sliding the view.
            WindowEvent::Focused(false) => {
                b.back();
                return false;
            }
            WindowEvent::CursorMoved { position, .. } => b.mouse_move(position.x, position.y),
            WindowEvent::MouseInput { state, button, .. } => b.mouse_button(*button, *state == ElementState::Pressed),
            WindowEvent::MouseWheel { delta, .. } => b.wheel(match delta {
                winit::event::MouseScrollDelta::LineDelta(_, y) => *y,
                winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
            }),
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else { return true };
                match event.state {
                    ElementState::Pressed => self.keys.insert(code),
                    ElementState::Released => self.keys.remove(&code),
                };
                let alt = self.keys.contains(&KeyCode::AltLeft) || self.keys.contains(&KeyCode::AltRight);
                // Alt+Enter and F2 are the game's still.
                if (alt && matches!(code, KeyCode::Enter | KeyCode::NumpadEnter)) || code == KeyCode::F2 {
                    return false;
                }
                if !event.repeat || event.state == ElementState::Pressed {
                    b.key(code, event.state == ElementState::Pressed, event.text.as_deref());
                }
            }
            _ => return false,
        }
        true
    }

    /// The builder's frame: its camera and what it asks.
    fn builder_tick(&mut self, dt: f32) {
        if builder::take_open() && self.in_menus() {
            self.open_builder(None);
        }
        if !self.in_builder() {
            return;
        }
        let now = self.start.elapsed().as_secs_f32();
        let Some(b) = &mut self.builder else { return };
        // The tracks to copy, listed by the menus' own track lists.
        if std::mem::take(&mut b.wants_tracks) {
            if let Some(f) = &mut self.front {
                use frontend::EventKind as E;
                for (event, mode) in [(E::Supercross, mm2_course::Mode::Supercross), (E::Nationals, mm2_course::Mode::National), (E::Baja, mm2_course::Mode::Baja), (E::Enduro, mm2_course::Mode::Enduro)] {
                    for (name, path, course) in f.track_list(event) {
                        b.tracks.push(builder::GameTrack { mode, name, path, course });
                    }
                }
                eprintln!("course builder: {} tracks to copy", b.tracks.len());
            } else {
                b.tracks = builder::tracks_in_folder();
            }
        }
        b.update(dt, now);
        match b.take_ask() {
            builder::Ask::Stay => {}
            builder::Ask::Exit => {
                let rode = self.builder.take().is_some_and(|b| b.rode);
                self.keys.clear();
                if self.front.is_none() {
                    self.quit = true;
                }
                if let Some(f) = &mut self.front {
                    f.forget_tracks();
                    // Back to the screen it opened from, unless a ride's
                    // loading screen took its place.
                    if rode {
                        f.main_menu();
                    }
                }
            }
            builder::Ask::Ride { env, mode, name } => {
                use frontend::EventKind;
                let event = match mode {
                    mm2_course::Mode::Supercross => EventKind::Supercross,
                    mm2_course::Mode::National => EventKind::Nationals,
                    mm2_course::Mode::Baja => EventKind::Baja,
                    mm2_course::Mode::Enduro => EventKind::Enduro,
                };
                let setup = match &mut self.front {
                    Some(f) => f.practice(event, env, name),
                    // The builder on its own: a practice lap on the
                    // track's own bike.
                    None => frontend::Setup {
                        event,
                        track: env,
                        course: 0,
                        laps: 99,
                        opponents: 0,
                        mode: frontend::PRACTICE,
                        name,
                        ride: None,
                        level: 1,
                        names: Vec::new(),
                        rides: Vec::new(),
                        ghost: None,
                        net: None,
                        random_gates: None,
                    },
                };
                self.keys.clear();
                self.load(setup);
            }
        }
    }

    /// Photo mode on (F, or both sticks clicked on a pad; an extra): the
    /// race holds still with its sound off and its display hidden, and
    /// the camera flies free from where it is. Not in a network race,
    /// where the others can't wait, nor on the podium.
    fn enter_photo(&mut self) {
        let can = tweaks::more().photo_mode
            && !self.photo
            && self.net.is_none()
            && self.loading.is_none()
            && !self.in_menus()
            && self.race.as_ref().is_some_and(|r| r.podium.is_none());
        if !can {
            return;
        }
        eprintln!("photo mode: on");
        self.photo = true;
        self.photo_follow = self.follow;
        self.follow = false;
        self.audio.set_muted(true);
        self.photo_note = Some((PHOTO_HELP.to_string(), Instant::now()));
    }

    /// Back to the race as it was left.
    fn leave_photo(&mut self) {
        if !std::mem::take(&mut self.photo) {
            return;
        }
        eprintln!("photo mode: off");
        self.follow = self.photo_follow && self.race.is_some();
        self.audio.set_muted(self.quiet && tweaks::get().mute_in_background);
        self.photo_note = None;
    }

    /// A pad in photo mode: the left stick flies (RB faster), the right
    /// stick looks round, the right trigger rises and the left sinks, A
    /// saves a picture, and B, Start or both sticks again go back.
    #[cfg(feature = "gamepad")]
    fn photo_pad(&mut self, presses: &[pad::Press], [stick, right]: [[f32; 2]; 2], rise: f32, fast: bool, dt: f32) {
        let pressed = |b: i32| presses.contains(&pad::Press::Button(b));
        if pressed(1) || pressed(7) {
            self.leave_photo();
            return;
        }
        if pressed(0) {
            self.shoot = true;
        }
        let Some(state) = &mut self.state else { return };
        let dead = |v: f32| if v.abs() < 0.15 { 0.0 } else { (v.abs() - 0.15) / 0.85 * v.signum() };
        let cam = &mut state.camera;
        let speed = if fast { PHOTO_SPEEDS.1 } else { PHOTO_SPEEDS.0 };
        let way = cam.forward() * -dead(stick[1]) + cam.right() * dead(stick[0]) + Vec3::Y * dead(rise);
        cam.position += way.clamp_length_max(1.0) * speed * dt;
        cam.look(dead(right[0]) * PHOTO_SPEEDS.2 * dt, dead(right[1]) * PHOTO_SPEEDS.2 * dt);
    }

    fn fly(&mut self, dt: f32) {
        let Some(state) = &mut self.state else { return };
        let key = |k| self.keys.contains(&k);
        let cam = &mut state.camera;
        let mut dir = Vec3::ZERO;
        if key(KeyCode::KeyW) {
            dir += cam.forward()
        }
        if key(KeyCode::KeyS) {
            dir -= cam.forward()
        }
        if key(KeyCode::KeyD) {
            dir += cam.right()
        }
        if key(KeyCode::KeyA) {
            dir -= cam.right()
        }
        if key(KeyCode::KeyE) || key(KeyCode::Space) {
            dir += Vec3::Y
        }
        if key(KeyCode::KeyQ) || key(KeyCode::ControlLeft) {
            dir -= Vec3::Y
        }
        // Photo mode's camera goes at a walk, to frame a picture close up.
        let (slow, fast, turn) = if self.photo { PHOTO_SPEEDS } else { (200.0, 900.0, 1.5) };
        let speed = if key(KeyCode::ShiftLeft) || key(KeyCode::ShiftRight) { fast } else { slow };
        cam.position += dir.normalize_or_zero() * speed * dt;
        let turn = turn * dt;
        if key(KeyCode::ArrowLeft) {
            cam.look(-turn, 0.0)
        }
        if key(KeyCode::ArrowRight) {
            cam.look(turn, 0.0)
        }
        if key(KeyCode::ArrowUp) {
            cam.look(0.0, -turn)
        }
        if key(KeyCode::ArrowDown) {
            cam.look(0.0, turn)
        }
    }

    /// What the keyboard and the first gamepad ask of the bike.
    /// Ctrl+S: the race's sounds off, or on again (0x4e9250).
    fn flip_sound(&mut self) {
        self.sound_off = !self.sound_off;
        let gain = self
            .front
            .as_ref()
            .map_or_else(sound::Options::default, |f| sound::Options::from_profile(&f.user().profile.sound))
            .gain;
        if let Some(s) = &mut self.sounds {
            s.set_on(&self.audio, !self.sound_off, gain);
        }
        if let Some((ui, hud)) = &mut self.hud {
            hud.say_switch(ui, 5161, !self.sound_off);
        }
    }

    /// The listener's EAX environment: the arena while the reverb is on,
    /// and only with EAX ticked (the game needs an EAX card for any).
    fn apply_reverb(&self) {
        let eax = self.front.as_ref().is_some_and(|f| f.user().profile.sound.eax);
        self.audio.set_reverb((eax && self.reverb).then_some(reverb::ARENA));
    }

    /// Ctrl+V: the reverb on or off, said (0x4e9250, string 0x14c3).
    fn flip_reverb(&mut self) {
        let eax = self.front.as_ref().is_some_and(|f| f.user().profile.sound.eax);
        if !eax || self.sounds.is_none() {
            return;
        }
        self.reverb = !self.reverb;
        self.apply_reverb();
        if let Some((ui, hud)) = &mut self.hud {
            hud.say_switch(ui, 0x14c3, self.reverb);
        }
    }

    /// F4: the Visual Cues on or off (0x4e54c0, string 0x13b6).
    fn flip_cues(&mut self) {
        self.visual_cues = !self.visual_cues;
        if let Some(r) = &mut self.race {
            r.visual_cues = self.visual_cues;
        }
        if let Some((ui, hud)) = &mut self.hud {
            hud.say_switch(ui, 0x13b6, self.visual_cues);
        }
    }

    /// F11 or F12: the overall graphic quality down or up a step, 0 to 9,
    /// which sets all four details, redraws the fog and the draw distance
    /// and says "Graphic Quality : n" (0x4e54c0).
    fn step_quality(&mut self, by: i64) {
        let Some(f) = &mut self.front else { return };
        let user = f.user_mut();
        let q = (user.profile.graphics.terrain as i64 + by).clamp(0, 9) as u32;
        user.profile.graphics.terrain = q;
        for key in screens::options::DETAILS {
            user.settings.set(key, &q.to_string());
        }
        user.save_settings();
        if let Err(e) = user.store.save(&user.profile) {
            eprintln!("could not save the profile: {e}");
        }
        if let (Some(state), Some(track)) = (&mut self.state, &self.track) {
            if let (Some(renderer), Some((visibility, haziness))) = (&mut state.renderer, track.haze) {
                renderer.set_visibility(q, visibility, haziness);
            }
        }
        if let Some((ui, hud)) = &mut self.hud {
            hud.say(&format!("{} : {}", ui.lang(0x14c4), q + 1));
        }
    }

    /// The fork's quick get-up, riding (not watching a replay or flying).
    /// V: into the nearest of the track's vehicles, or the nearest craft
    /// in its sky, or out again onto the bike (Extras, Drive the scenery;
    /// in Practice and free roam).
    fn toggle_drive(&mut self) {
        if self.drive.is_some() {
            self.leave_vehicle(true);
            return;
        }
        if !self.follow || self.replay.is_some() || !tweaks::more().drive_vehicles {
            return;
        }
        let practice = self.front.is_none() || self.race.as_ref().is_some_and(|r| r.practice);
        let (Some(r), Some(track)) = (&mut self.race, &mut self.track) else { return };
        let say = |hud: &mut Option<(ui::Ui, hud::Hud)>, text: &str| {
            if let Some((_, hud)) = hud {
                hud.say(text);
            }
        };
        if !practice {
            say(&mut self.hud, "Vehicles are for Practice and free roam.");
            return;
        }
        let d = pick_vehicle(r, track, false);
        let Some(d) = d else {
            say(&mut self.hud, "No vehicle near enough to get in, and nothing flying here.");
            return;
        };
        if let drive::Seat::Air(o) = d.seat {
            pin_craft(track.objects.animated.get_mut(o), Some(&d));
            pin_craft(self.state.as_mut().and_then(|s| s.renderer.as_mut()).and_then(|r| r.animated_mut(o)), Some(&d));
        }
        say(&mut self.hud, &d.hint());
        eprintln!("drive: in the {} ({:?})", d.name, d.kind);
        self.drive = Some(d);
    }

    /// Out of the vehicle the player is in, if any; with `onto_bike` the
    /// bike waits beside it.
    fn leave_vehicle(&mut self, onto_bike: bool) {
        let Some(d) = self.drive.take() else { return };
        let Some(track) = &mut self.track else { return };
        d.leave(&mut track.traffic);
        if let drive::Seat::Air(o) = d.seat {
            for a in [track.objects.animated.get_mut(o), self.state.as_mut().and_then(|s| s.renderer.as_mut()).and_then(|r| r.animated_mut(o))].into_iter().flatten() {
                a.pin = None;
                a.set_place(d.home);
            }
        }
        if !onto_bike {
            return;
        }
        let (at, way) = d.way_out(&track.terrain);
        if let Some(r) = &mut self.race {
            let me = r.player;
            if let race::Bike::Player(b) = &mut r.racers[me].bike {
                b.reset(&track.ground(), at, way);
            }
        }
        if let Some((_, hud)) = &mut self.hud {
            hud.say("Back on the bike.");
        }
    }

    fn get_back_on(&mut self) {
        if !self.follow || self.replay.is_some() {
            return;
        }
        if let (Some(r), Some(t)) = (&mut self.race, &self.track) {
            r.get_back_on(&t.ground());
        }
    }

    fn ctrl(&self) -> bool {
        self.keys.contains(&KeyCode::ControlLeft) || self.keys.contains(&KeyCode::ControlRight)
    }

    /// Ctrl+G or Ctrl+B: the gas or brake gyro on or off, said and kept.
    fn flip_gyro(&mut self, gas: bool) {
        let Some(r) = &mut self.race else { return };
        let me = r.player;
        let race::Bike::Player(b) = &mut r.racers[me].bike else {
            return;
        };
        let on = if gas {
            b.gas_gyro = !b.gas_gyro;
            b.gas_gyro
        } else {
            b.brake_gyro = !b.brake_gyro;
            b.brake_gyro
        };
        if let Some((ui, hud)) = &mut self.hud {
            hud.say_switch(ui, if gas { 5133 } else { 5165 }, on);
        }
        if let Some(f) = &mut self.front {
            let user = f.user_mut();
            if gas {
                user.profile.controls.gas_gyro = on;
            } else {
                user.profile.controls.brake_gyro = on;
            }
            if let Err(e) = user.store.save(&user.profile) {
                eprintln!("could not save the profile: {e}");
            }
        }
    }

    /// `/` opens the chat line unless the chat box is always off; Shift+/
    /// steps its mode: normal, always on, always off ("ChatBox : Normal").
    fn chat_key(&mut self, shift: bool) {
        let Some(f) = &mut self.front else { return };
        let user = f.user_mut();
        if !shift {
            if user.profile.game.chat_mode != 2 {
                self.chat = Some(String::new());
                self.keys.clear();
            }
            return;
        }
        let mode = (user.profile.game.chat_mode + 1) % 3;
        user.profile.game.chat_mode = mode;
        if let Err(e) = user.store.save(&user.profile) {
            eprintln!("could not save the profile: {e}");
        }
        if let Some((ui, hud)) = &mut self.hud {
            let what = [5131, 5130, 5132][mode as usize];
            hud.say(&format!("{} : {}", ui.lang(5160), ui.lang(what)));
        }
    }

    /// A key while the chat line is open: Enter sends it, Escape drops it.
    fn type_chat(&mut self, event: &winit::event::KeyEvent) {
        let Some(line) = &mut self.chat else { return };
        match event.physical_key {
            PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) => {
                let line = self.chat.take().unwrap_or_default();
                if !line.trim().is_empty() {
                    self.send_chat(&line);
                }
                return;
            }
            PhysicalKey::Code(KeyCode::Escape) => {
                self.chat = None;
                return;
            }
            PhysicalKey::Code(KeyCode::Backspace) => {
                line.pop();
            }
            _ => {
                if let Some(text) = &event.text {
                    for c in text.chars().filter(|c| !c.is_control()) {
                        if line.chars().count() < 73 {
                            line.push(c);
                        }
                    }
                }
            }
        }
        let shown = format!("> {line}_");
        if let Some((_, hud)) = &mut self.hud {
            hud.say(&shown);
        }
    }

    /// Sends a chat line as "Name: line" and shows it here too.
    fn send_chat(&mut self, line: &str) {
        let name = self.front.as_ref().map_or_else(String::new, |f| f.user().profile.name.clone());
        let text = format!("{name}: {line}");
        net::with(|s| s.say(net::Msg::Chat(text.clone())));
        if let Some((_, hud)) = &mut self.hud {
            hud.say(&text);
        }
    }

    /// F5 to F10 flip the race panel, radar, speedometer, sky, dirt and
    /// shadows, and say so.
    fn flip_switch(&mut self, key: KeyCode) {
        let Some((ui, hud)) = &mut self.hud else {
            return;
        };
        let s = &mut hud.show;
        let (flag, what) = match key {
            KeyCode::F5 => (&mut s.stats, 5141),
            KeyCode::F6 => (&mut s.radar, 5185),
            KeyCode::F7 => (&mut s.gauges, 5142),
            KeyCode::F8 => (&mut s.sky, 5137),
            KeyCode::F9 => (&mut s.particles, 5135),
            _ => (&mut s.shadows, 5136),
        };
        *flag = !*flag;
        let on = *flag;
        hud.say_switch(ui, what, on);
        self.keep_switches();
    }

    /// The profile keeps the race's switches.
    fn keep_switches(&mut self) {
        let (Some((_, hud)), Some(f)) = (&self.hud, &mut self.front) else {
            return;
        };
        let user = f.user_mut();
        hud.show.store(&mut user.profile);
        if let Err(e) = user.store.save(&user.profile) {
            eprintln!("could not save the profile: {e}");
        }
    }

    fn controls(&mut self, dt: f32) -> Controls {
        let acts = self.actions_held();
        let held = |a: usize| acts[a];
        let keyboard = Controls {
            throttle: held(input::ACCELERATE) as i32 as f32,
            brake: held(input::BRAKE) as i32 as f32,
            steer: self
                .steer_keys
                .update(held(input::STEER_LEFT), held(input::STEER_RIGHT), dt),
            lean: self
                .lean_keys
                .update(held(input::LEAN_BACK), held(input::LEAN_FORWARD), dt),
        };
        self.gamepad(keyboard)
    }

    #[cfg(not(feature = "gamepad"))]
    fn gamepad(&mut self, c: Controls) -> Controls {
        c
    }

    /// The trick buttons (the game's buttons 6 and 7): the profile's keys
    /// (Z and X from the factory), or the first gamepad's A and B (our
    /// picks until the controls page sets them).
    fn trick_buttons(&self) -> [bool; 2] {
        let b = [
            self.key_map.held(input::STUNTS_LOW, &self.keys) || self.pad_held(input::STUNTS_LOW),
            self.key_map.held(input::STUNTS_HIGH, &self.keys) || self.pad_held(input::STUNTS_HIGH),
        ];
        b
    }

    /// Each action held this frame, on the keys or the gamepad.
    fn actions_held(&self) -> [bool; profile::ACTIONS] {
        std::array::from_fn(|a| self.key_map.held(a, &self.keys) || self.pad_held(a))
    }

    /// The joystick in use, as the profile maps it (0x449e20 binds the
    /// map, 0x52e3e0 reads it): an action on a button is held with it;
    /// the steering axis has a 0.04 dead centre and is cubed, fine near
    /// the middle and full lock at the edge; the lean axis has a small dead
    /// centre. Accelerate on the Z axis (a flight stick's throttle
    /// lever) opens the gas past a third of its travel, the way the
    /// game's analog throttle does (0.2 floor, 0.33 to count). Analog
    /// Axes off, a stick works the bars and lean like the keys. A game
    /// pad's triggers also work the gas and brake, past a third.
    #[cfg(feature = "gamepad")]
    fn gamepad(&mut self, mut c: Controls) -> Controls {
        let map = self.pad_map;
        let flip = if self.joy_flipped { -1.0 } else { 1.0 };
        let Some(pad) = self.pads.as_ref() else {
            return c;
        };
        let trigger = |b| pad.trigger(b);
        if trigger(gilrs::Button::RightTrigger2) > pad::THROTTLE_ON {
            c.throttle = 1.0;
        }
        if trigger(gilrs::Button::LeftTrigger2) > pad::THROTTLE_ON {
            c.brake = 1.0;
        }
        if let (2, code) = map[input::ACCELERATE] {
            if let Some(v) = pad.throttle_axis(code) {
                if v > pad::THROTTLE_ON {
                    c.throttle = 1.0;
                }
            }
        }
        // An action's pull, 0 to 1; `side` is the axis's half it reads.
        let pull = |action: usize, side: f32, dead: f32| -> f32 {
            match map[action] {
                (2, code) => match pad.axis(code) {
                    Some(v) if code < 0 => {
                        let v = if code == -5 { v * flip } else { v };
                        let v = if v.abs() < dead { 0.0 } else { v };
                        (v * side).max(0.0)
                    }
                    _ => 0.0,
                },
                _ => 0.0,
            }
        };
        // The pad tuning extra: its dead zone, sensitivity and smoothing,
        // the game's own (0.04, as it reads, unsmoothed) unless changed.
        let tune = crate::tweaks::more();
        let dead = if tune.pad_dead_zone == mm2_settings::PAD_DEAD_ZONE {
            pad::STEER_DEAD
        } else {
            tune.pad_dead_zone as f32 / 100.0
        };
        let mut steer = pull(input::STEER_RIGHT, 1.0, dead) - pull(input::STEER_LEFT, -1.0, dead);
        if tune.pad_dead_zone != mm2_settings::PAD_DEAD_ZONE && steer != 0.0 {
            // A bigger dead zone still reaches full lock at the edge.
            steer = steer.signum() * ((steer.abs() - dead) / (1.0 - dead).max(0.05)).clamp(0.0, 1.0);
        }
        if tune.pad_sensitivity != 100 {
            steer = (steer * tune.pad_sensitivity as f32 / 100.0).clamp(-1.0, 1.0);
        }
        if tune.pad_smoothing > 0 {
            let now = Instant::now();
            let dt = self.pad_smooth.1.map_or(0.0, |t| now.duration_since(t).as_secs_f32()).min(0.1);
            // Up to a quarter of a second to follow the stick.
            let tau = 0.25 * tune.pad_smoothing as f32 / 100.0;
            let k = 1.0 - (-dt / tau).exp();
            let s = self.pad_smooth.0 + (steer - self.pad_smooth.0) * k;
            self.pad_smooth = (if s.abs() < 1e-3 && steer == 0.0 { 0.0 } else { s }, Some(now));
            steer = self.pad_smooth.0;
        }
        // The lean axis: the game gives it no dead centre, so a stick
        // that drifts a little off the middle (worn sticks, a Steam
        // Deck's) would lean the rider all the time. Here it has a small
        // one, rescaled so the edge is still full lean.
        let lean = pull(input::LEAN_FORWARD, -1.0, pad::LEAN_DEAD) - pull(input::LEAN_BACK, 1.0, pad::LEAN_DEAD);
        let lean = lean.signum() * ((lean.abs() - pad::LEAN_DEAD) / (1.0 - pad::LEAN_DEAD)).max(0.0);
        // For a player's log: the first time the pad leans the rider,
        // and on which pad.
        if lean.abs() > 0.2 && !self.pad_lean_logged {
            self.pad_lean_logged = true;
            let name = pad.info().map_or_else(String::new, |i| i.name);
            eprintln!("joystick: lean {lean:+.2} from {name} (map {:?} {:?})", map[input::LEAN_FORWARD], map[input::LEAN_BACK]);
        }
        if self.analog {
            if steer != 0.0 {
                c.steer = input::stick_steer(steer, crate::tweaks::more().pad_curve);
            }
            if lean != 0.0 {
                c.lean = lean;
            }
        }
        c
    }

    /// Whether the joystick holds `action`'s button, or, with Analog
    /// Axes off, pushes its axis's side over half way, as a key (the
    /// game's game pad reading, 0x4c74e0).
    #[cfg(feature = "gamepad")]
    fn pad_held(&self, action: usize) -> bool {
        let Some(pad) = self.pads.as_ref() else {
            return false;
        };
        match self.pad_map[action] {
            (2, code) if code >= 0 => pad.button(code),
            (2, code) if !self.analog => pad.axis(code).is_some_and(|v| {
                let v = if code == -5 && self.joy_flipped { -v } else { v };
                let side = match action {
                    input::STEER_LEFT | input::LEAN_FORWARD => -1.0,
                    _ => 1.0,
                };
                v * side > pad::PUSHED
            }),
            _ => false,
        }
    }

    /// Takes the joysticks' events each frame. In the menus the pad works
    /// the pointer (the game's menus took only the mouse and keys): the
    /// left stick or D-pad moves it, A clicks and B goes back. On the
    /// controls page, while an action waits, a button or stick pressed
    /// goes to the page to bind instead.
    #[cfg(feature = "gamepad")]
    fn poll_pads(&mut self, event_loop: &ActiveEventLoop, dt: f32) {
        let Some(pads) = &mut self.pads else {
            return;
        };
        let presses = pads.poll();
        let (stick, a, b) = pads.pointer();
        let right = [pads.axis(-9).unwrap_or(0.0), pads.axis(-11).unwrap_or(0.0)];
        let zoom = -pads.axis(-7).unwrap_or(0.0);
        // Both sticks clicked (DirectInput's 8 and 9): photo mode.
        let pressed = |n: i32| presses.contains(&pad::Press::Button(n));
        let both_sticks = (pressed(8) && pads.button(9)) || (pressed(9) && pads.button(8));
        let fast = pads.button(5);
        let (was_a, was_b) = std::mem::replace(&mut self.pad_click, (a, b));
        if self.photo {
            if both_sticks {
                self.leave_photo();
            } else {
                self.photo_pad(&presses, [stick, right], zoom, fast, dt);
            }
            return;
        }
        if both_sticks && !self.in_menus() && !self.in_builder() && self.track.is_some() {
            self.enter_photo();
            if self.photo {
                return;
            }
        }
        // The Course Builder: the left stick or D-pad moves the pointer
        // (pushed on at the edge, the view), A clicks, B backs out, X
        // removes, Y undoes, the shoulders change tool, the right stick
        // turns the view, the triggers zoom, Start rides, Back looks down.
        if self.in_builder() {
            let back = b;
            let (Some(b), Some(s)) = (&mut self.builder, &self.state) else { return };
            let [sx, sy, rx, ry, zoom] = b.pad_axes([stick[0], stick[1], right[0], right[1], zoom]);
            let (stick, right) = ([sx, sy], [rx, ry]);
            let (w, h) = (s.config.width as f64, s.config.height as f64);
            let speed = |v: f32| if v.abs() < builder::PAD_DEAD { 0.0 } else { v as f64 * 600.0 * dt as f64 * h / 480.0 };
            let (dx, dy) = (speed(stick[0]), speed(stick[1]));
            let mut pan = [0.0f32; 2];
            if dx != 0.0 || dy != 0.0 {
                let (x, y) = b.pointer_px();
                let (nx, ny) = ((x + dx).clamp(0.0, w - 1.0), (y + dy).clamp(0.0, h - 1.0));
                // Pushed against an edge: the view moves instead.
                if (x + dx) != nx {
                    pan[0] = stick[0];
                }
                if (y + dy) != ny {
                    pan[1] = stick[1];
                }
                b.mouse_move(nx, ny);
                let _ = s.window.set_cursor_position(winit::dpi::PhysicalPosition::new(nx, ny));
            }
            b.pad_view(right, zoom, pan, dt);
            if a != was_a {
                b.mouse_button(winit::event::MouseButton::Left, a);
            }
            use builder::PadButton as P;
            if back && !was_b {
                b.pad_button(P::Back);
            }
            for (n, what) in [(2, P::Remove), (3, P::Undo), (4, P::Tool(-1)), (5, P::Tool(1)), (6, P::Top), (7, P::Ride)] {
                if presses.contains(&pad::Press::Button(n)) {
                    b.pad_button(what);
                }
            }
            return;
        }
        if !self.in_menus() {
            // Buttons the map leaves free: Back (DirectInput's button 6)
            // the fork's quick get-up, Start (7) out of the race as
            // Escape, the D-pad's up the next camera view, its right
            // the Helmet Cam and its down a vehicle (drive.rs).
            let free = |b: i32| presses.contains(&pad::Press::Button(b)) && !self.pad_map.iter().any(|&(k, c)| k == 2 && c == b);
            let (back, start, view, helmet) = (free(6), free(7), free(pad::DPAD_UP), free(pad::DPAD_RIGHT));
            let vehicle = free(pad::DPAD_DOWN);
            if back {
                self.get_back_on();
            }
            // The D-pad's down: into or out of a vehicle, as V (Extras).
            if vehicle {
                self.toggle_drive();
            }
            if start && self.front.is_some() && self.loading.is_none() {
                self.end_race();
                return;
            }
            if self.follow && (view || helmet) {
                let name = self.race.as_mut().map(|r| if helmet { r.toggle_helmet() } else { r.next_view() });
                if let (Some(name), Some((_, hud))) = (name, &mut self.hud) {
                    hud.say(name);
                }
            }
            return;
        }
        let now = self.start.elapsed().as_secs_f32();
        let Some(f) = &mut self.front else { return };
        if f.wants_pad() {
            for press in presses {
                let input = match press {
                    pad::Press::Button(n) => (2, n),
                    pad::Press::Axis(code) => (2, code),
                };
                let Some(f) = &mut self.front else { return };
                let next = f.pad_input(&self.audio, input);
                if !matches!(next, frontend::Next::Stay) {
                    self.next(next, event_loop);
                    return;
                }
            }
            return;
        }
        // The shoulder buttons (DirectInput's 4 and 5) go along the tabs.
        for (button, step) in [(4, -1), (5, 1)] {
            if presses.contains(&pad::Press::Button(button)) {
                let Some(f) = &mut self.front else { return };
                let next = f.step_tab(&self.audio, step, now);
                if !matches!(next, frontend::Next::Stay) {
                    self.next(next, event_loop);
                    return;
                }
            }
        }
        let Some(f) = &mut self.front else { return };
        // Past a fifth of the stick, up to 500 of the 640×480 screen a second.
        let speed = |v: f32| if v.abs() < 0.2 { 0.0 } else { v * 500.0 * dt };
        let (dx, dy) = (speed(stick[0]), speed(stick[1]));
        let mut to = None;
        if dx != 0.0 || dy != 0.0 {
            let (x, y) = self.pointer;
            to = Some(((x + dx).clamp(0.0, 639.0), (y + dy).clamp(0.0, 479.0)));
        }
        // The D-pad jumps the pointer to the next button that way, which
        // lights up as it would under the mouse.
        for (b, dir) in [
            (pad::DPAD_UP, (0.0, -1.0)),
            (pad::DPAD_DOWN, (0.0, 1.0)),
            (pad::DPAD_LEFT, (-1.0, 0.0)),
            (pad::DPAD_RIGHT, (1.0, 0.0)),
        ] {
            if presses.contains(&pad::Press::Button(b)) {
                if let Some(t) = pad_jump(self.pointer, dir, &f.targets()) {
                    to = Some(t);
                }
            }
        }
        if let Some((x, y)) = to {
            self.pointer = (x, y);
            f.mouse_move(&self.audio, x, y);
            if let Some(s) = &self.state {
                let (w, h) = (s.config.width as f32, s.config.height as f32);
                let scale = (w / sprites::SCREEN.0).min(h / sprites::SCREEN.1);
                let (ox, oy) = ((w - sprites::SCREEN.0 * scale) / 2.0, (h - sprites::SCREEN.1 * scale) / 2.0);
                let at = winit::dpi::PhysicalPosition::new((x * scale + ox) as f64, (y * scale + oy) as f64);
                let _ = s.window.set_cursor_position(at);
            }
        }
        let (x, y) = self.pointer;
        if a && !was_a {
            f.mouse_down(x, y);
        } else if !a && was_a {
            let next = f.mouse_up(&self.audio, x, y, now);
            self.next(next, event_loop);
            return;
        }
        if b && !was_b {
            let next = f.escape(now);
            self.next(next, event_loop);
        }
    }

    #[cfg(not(feature = "gamepad"))]
    fn poll_pads(&mut self, _event_loop: &ActiveEventLoop, _dt: f32) {}

    /// Runs the pad's motors at this frame's vibration, at the profile's
    /// Force Feedback strength: 0.4 times at the slider's least to three
    /// times at its most (half way about 1.1), each step a like share
    /// stronger; the box off stops it.
    #[cfg(feature = "gamepad")]
    fn feel_pad(&mut self) {
        let Some(pads) = &mut self.pads else { return };
        // The launcher's strength, else the profile's.
        let controls = self.front.as_ref().map(|f| &f.user().profile.controls);
        let strength = match screens::options::vibration() {
            Some(set) => set,
            None => controls.map_or(Some(50), |c| c.force_feedback.then_some(c.feedback)),
        };
        let gain = match strength {
            _ if self.quiet => 0.0,
            None => 0.0,
            Some(s) => 0.4 * (3.0f32 / 0.4).powf((s as f32 / 100.0).min(1.0)),
        };
        pads.rumble(self.shake.map(|s| (s * gain).min(1.0)));
    }

    #[cfg(not(feature = "gamepad"))]
    fn feel_pad(&mut self) {}
    #[cfg(not(feature = "gamepad"))]
    fn pad_held(&self, _action: usize) -> bool {
        false
    }

    /// Rides the bike, or flies the camera when not following it.
    fn update(&mut self, dt: f32) {
        self.finish_loading();
        self.shake = [0.0; 2];
        if self.track.is_none() {
            self.rumble.reset();
            self.leave_photo();
            return;
        }
        if self.photo {
            if self.race.is_none() || self.in_menus() || self.net.is_some() {
                self.leave_photo();
            } else {
                // Nothing moves on, not even the race's clock: only the
                // camera. The marker over the player, the Visual Cue
                // arrow and the countdown numbers are left out of the
                // picture.
                self.fly(dt);
                if let (Some(r), Some(track), Some(state)) = (&mut self.race, &self.track, &mut self.state) {
                    if let Some(renderer) = &mut state.renderer {
                        let marker = std::mem::replace(&mut r.player_indicator, false);
                        show(r, track, renderer, renderer::from_game(state.camera.position), None);
                        r.player_indicator = marker;
                    }
                }
                return;
            }
        }
        let podium_over = |r: &Race| r.podium.is_some_and(|(_, at)| r.time + dt - at >= podium::LENGTH);
        if self.race.as_ref().is_some_and(podium_over) {
            self.back_to_menu();
            return;
        }
        if self.race_done() {
            self.end_race();
            return;
        }
        let controls = if self.follow {
            self.controls(dt)
        } else {
            Controls::default()
        };
        let buttons = if self.follow {
            self.trick_buttons()
        } else {
            [false; 2]
        };
        if self.replay.is_none() && self.race.is_some() {
            if let Some(track) = &mut self.track {
                track.run_traffic(dt);
            }
        } else if self.drive.is_some() {
            self.leave_vehicle(false);
        }
        let gas = controls.throttle;
        // At the wheel, the bike waits with its brake on.
        let controls = match (&mut self.drive, &mut self.track) {
            (Some(d), Some(track)) => {
                d.update(controls, &track.terrain, &track.world, &mut track.traffic, dt);
                if let drive::Seat::Air(o) = d.seat {
                    if let Some(a) = track.objects.animated.get_mut(o) {
                        a.set_place(d.place());
                    }
                }
                Controls { brake: 1.0, ..Controls::default() }
            }
            _ => controls,
        };
        let acts = self.actions_held();
        let Some(track) = &self.track else { return };
        let Some(state) = &mut self.state else { return };
        if let (Some(rp), Some(r)) = (&mut self.replay, &mut self.race) {
            rp.advance(dt);
            let (eye, target) = rp.show(r, &track.terrain);
            if let Some(sounds) = &mut self.sounds {
                let l = listener(r, eye, target);
                sounds.update(&self.audio, r, &l, dt);
                sounds.ambient(&self.audio, &l, |site| track.site(site, r.time), dt);
            }
            if let Some(renderer) = &mut state.renderer {
                show(r, track, renderer, eye, None);
            }
            if self.follow {
                state.camera =
                    Camera::looking_at(renderer::from_game(eye), renderer::from_game(target));
                state.camera.fov = r.fov();
                state.camera.roll = r.camera_roll();
                return;
            }
            self.fly(dt);
            return;
        }
        // The podium runs its 7 s and then leaves for the results.
        if let Some((p, start)) = self.race.as_ref().and_then(|r| r.podium) {
            let Some(r) = &mut self.race else { return };
            r.time += dt;
            let (eye, target) = p.camera(r.time - start);
            if let Some(renderer) = &mut state.renderer {
                show(r, track, renderer, eye, None);
            }
            state.camera = Camera::looking_at(renderer::from_game(eye), renderer::from_game(target));
            return;
        }
        match &mut self.race {
            Some(r) => {
                r.buttons = buttons;
                let held = |a: usize| self.follow && acts[a];
                r.cam_keys = ride::CamKeys {
                    thrill: held(input::THRILL_CAM),
                    reverse: held(input::REVERSE_CAM),
                };
                let (clutch, back_up) = (held(input::CLUTCH), held(input::BACK_UP));
                let me = r.player;
                r.racers[me].bike.hands(clutch, back_up);
                r.actions = if self.follow {
                    [
                        acts[input::KAHUNA],
                        acts[input::TAUNT],
                    ]
                } else {
                    [false; 2]
                };
                let (eye, target) = r.update(&track.ground(), controls, dt);
                let (eye, target) = match &mut self.drive {
                    Some(d) => d.camera(&track.terrain, dt),
                    None => (eye, target),
                };
                let mut splashes = Vec::new();
                if let Some(renderer) = &mut state.renderer {
                    throw_dirt(r, &track.ground(), &mut renderer.particles, dt);
                    splashes = splash_water(r, &track.water, &mut renderer.particles, dt);
                    renderer.tyres(&tyre_places(r, &track.ground()), dt);
                }
                if let Some(rec) = &mut self.recording {
                    rec.tick(r, dt);
                }
                if let Some(t) = &mut self.lap_tapes {
                    t.tick(r, dt);
                }
                if let Some(log) = &mut self.ride_log {
                    log_ride(log, r, &acts, dt);
                }
                if let Some(g) = &mut self.ghost {
                    g.tick(r, &track.terrain, dt);
                }
                if let Some(n) = self.net {
                    let chat = net_race(
                        n,
                        r,
                        &mut self.remote,
                        &mut self.send_wait,
                        &track.terrain,
                        dt,
                    );
                    // Chat Box Always Off hides what the others say.
                    let shown = self.front.as_ref().is_none_or(|f| f.user().profile.game.chat_mode != 2);
                    if let (Some((_, hud)), true) = (&mut self.hud, shown) {
                        for line in chat {
                            hud.say(&line);
                        }
                    }
                }
                if let Some(sounds) = &mut self.sounds {
                    let mut l = listener(r, eye, target);
                    // The camera goes with the vehicle the player drives.
                    if let Some(d) = &self.drive {
                        l.velocity = d.velocity();
                    }
                    // The bike is quiet while the player drives something else.
                    sounds.hush = self.drive.is_some().then_some(r.player);
                    sounds.update(&self.audio, r, &l, dt);
                    self.drive_sound.update(&self.audio, sounds.gain(), self.drive.as_ref(), gas, &l, dt);
                    sounds.ambient(&self.audio, &l, |site| track.site(site, r.time), dt);
                    for (at, amount) in &splashes {
                        sounds.splash(&self.audio, &l, *at, *amount, dt);
                    }
                }
                if self.follow {
                    let bike = &r.racers[r.player].bike;
                    let feel = rumble::Feel {
                        airborne: bike.airborne(),
                        crashed: bike.crashed(),
                        tumbling: bike.rider_touching(),
                        velocity: bike.velocity(),
                    };
                    self.shake = self.rumble.update(feel, dt);
                }
                if let Some(renderer) = &mut state.renderer {
                    let view = cues::Eye { at: eye, target, fov: r.fov(), aspect: renderer.aspect() };
                    show(r, track, renderer, eye, Some(view));
                    feel_wrecks(r, renderer);
                    if let Some(d) = &self.drive {
                        drive_show(d, r, track, renderer);
                    }
                }
                let status = r.status();
                if status != self.title {
                    state
                        .window
                        .set_title(&format!("Motocross Madness 2 - {status}"));
                    self.title = status;
                }
                if self.follow {
                    state.camera =
                        Camera::looking_at(renderer::from_game(eye), renderer::from_game(target));
                    state.camera.fov = r.fov();
                    state.camera.roll = r.camera_roll();
                    return;
                }
            }
            None => self.follow = false,
        }
        self.fly(dt);
    }

    fn redraw(&mut self) {
        let now = self.start.elapsed().as_secs_f32();
        let progress = self.loading.as_ref().map_or(1.0, |_| {
            let steps = LOAD_STEPS.load(std::sync::atomic::Ordering::Relaxed).min(LOAD_BAR_STEPS);
            steps as f32 / LOAD_BAR_STEPS as f32
        });
        let Some(s) = &mut self.state else { return };
        display::tick(&s.window);
        // In a browser the picture's size changes while racing
        // (dynres::Picture): made anew at it, before the frame is taken.
        let size = s.window.inner_size();
        let drawn = drawn_size(size.width, size.height);
        if s.config.present_mode != present_mode() || (cfg!(target_arch = "wasm32") && drawn != (s.config.width, s.config.height)) {
            s.config.present_mode = present_mode();
            (s.config.width, s.config.height) = drawn;
            s.surface.configure(&s.device, &s.config);
        }
        // A suboptimal frame (the Steam Deck's full screen gives them) is
        // still drawn and shown, and the surface made anew after: wgpu
        // refuses to configure it while a frame is out.
        let (frame, suboptimal) = match s.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
            wgpu::CurrentSurfaceTexture::Outdated => {
                skipped("the window's surface is out of date");
                s.fit_surface();
                return;
            }
            other => {
                skipped(&format!("{other:?}"));
                return;
            }
        };
        let window_view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(s.config.format.add_srgb_suffix()),
            ..Default::default()
        });
        let window_size = (s.config.width, s.config.height);
        let mut commands = Vec::new();
        let racing = self.track.is_some() && self.loading.is_none();
        // Wider than 21:9, the race is drawn 21:9 and goes between black bars.
        let between_bars = (racing && s.config.usage.contains(wgpu::TextureUsages::COPY_DST))
            .then(|| s.bars.target(&s.device, s.config.format.add_srgb_suffix(), window_size))
            .flatten();
        // The picture extras draw the race into a picture of their own and
        // put it on the window smoothed and sharpened.
        let polishing = racing
            && between_bars.is_none()
            && polish::wanted();
        let polish_view = polishing.then(|| {
            let format = s.config.format;
            let device = &s.device;
            s.polish.get_or_insert_with(|| polish::Polish::new(device, format)).target(device, format, window_size)
        });
        let (view, size) = match (&between_bars, &polish_view) {
            (Some((view, size)), _) => (view, *size),
            (None, Some(view)) => (view, window_size),
            (None, None) => (&window_view, window_size),
        };
        // Text is drawn at the window's own resolution, Retina included.
        s.sprites.scale = (size.0 as f32 / 640.0).min(size.1 as f32 / 480.0);
        if let (true, Some(renderer)) = (racing, &mut s.renderer) {
            // Animations run on the race's clock, so a restart replays the gate.
            let time = match &self.race {
                Some(r) => r.time,
                None => now,
            };
            if let Some((_, hud)) = &mut self.hud {
                renderer.sky_on = hud.show.sky;
                renderer.shadows_on = hud.show.shadows;
                renderer.particles_on = hud.show.particles;
                let aspect = size.0 as f32 / size.1.max(1) as f32;
                hud.view_proj = Mat4::perspective_rh(s.camera.fovy(aspect), aspect, 1.0, 20_000.0)
                    * s.camera.view();
            }
            if let (Some((_, hud)), Some(race), Some(track)) = (&mut self.hud, &self.race, &self.track) {
                hud.heads = race.racers.iter().map(|r| renderer.object_frame(r.object + 1, "Head", time)).collect();
                hud.look(&track.terrain, renderer::from_game(s.camera.position));
            }
            renderer.cinematic = self
                .race
                .as_ref()
                .filter(|_| self.replay.is_some())
                .map(|r| renderer::from_game(r.racers[r.player].bike.position()).distance(s.camera.position));
            commands.push(renderer.render(&s.device, &s.queue, view, size, &s.camera, time));
            // The race's display goes over the polished picture, on the window.
            let view = match (&polish_view, &s.polish) {
                (Some(_), Some(p)) => {
                    if let Some(c) = p.finish(&s.device, &s.queue, &window_view) {
                        commands.push(c);
                    }
                    &window_view
                }
                _ => view,
            };
            if let (Some((ui, hud)), Some(race)) = (&mut self.hud, &self.race) {
                let mut gfx = ui::Gfx {
                    sprites: &mut s.sprites,
                    device: &s.device,
                    queue: &s.queue,
                };
                // A replay shows the VCR player instead, and photo mode
                // nothing but its line at the bottom for a few seconds
                // (never in the picture it saves).
                let note = self.photo_note.as_ref().filter(|(_, at)| at.elapsed().as_secs_f32() < PHOTO_NOTE_SECS);
                match &mut self.replay {
                    _ if self.photo => {
                        if let (Some((text, _)), false) = (note, self.shoot) {
                            ui.bottom_note(&mut gfx, text);
                        }
                    }
                    Some(rp) => rp.draw(ui, &mut gfx, now),
                    None if race.podium.is_some() => {}
                    None => hud.draw(ui, &mut gfx, race, self.frame),
                }
                if let (Some(fps), false) = (self.fps_shown, self.photo) {
                    ui.frame_rate(&mut gfx, fps);
                }
                if self.replay.is_some() && !self.photo && ui::draws_pointer() {
                    ui.pointer(&mut gfx, self.pointer);
                }
                let mut encoder = s.device.create_command_encoder(&Default::default());
                s.sprites.stretch = true;
                s.sprites.widescreen = self.replay.is_none();
                s.sprites
                    .render(&s.device, &mut encoder, view, size, false);
                s.sprites.stretch = false;
                s.sprites.widescreen = false;
                commands.push(encoder.finish());
            }
            if between_bars.is_some() {
                let mut encoder = s.device.create_command_encoder(&Default::default());
                s.bars.finish(&mut encoder, &frame.texture);
                commands.push(encoder.finish());
            }
        } else if let (Some(b), true) = (
            &mut self.builder,
            self.track.is_none() && self.loading.is_none(),
        ) {
            let game = self.front.as_ref().and_then(|f| f.game()).map(Path::to_path_buf).unwrap_or_else(|| b.game_dir());
            let format = s.config.format.add_srgb_suffix();
            commands.extend(b.draw(&s.device, &s.queue, &mut s.sprites, format, view, size, &game));
        } else if let Some(front) = &mut self.front {
            let mut gfx = ui::Gfx {
                sprites: &mut s.sprites,
                device: &s.device,
                queue: &s.queue,
            };
            front.draw(&mut gfx, now, progress);
            if let Some(fps) = self.fps_shown {
                front.ui_mut().frame_rate(&mut gfx, fps);
            }
            if ui::draws_pointer() {
                front.ui_mut().pointer(&mut gfx, self.pointer);
            }
            let mut encoder = s.device.create_command_encoder(&Default::default());
            s.sprites.render(&s.device, &mut encoder, view, size, true);
            commands.push(encoder.finish());
            match front.preview() {
                Some((ride, turning)) => {
                    if s.bike_res.is_none() {
                        s.bike_res = front
                            .game()
                            .and_then(|g| read_archive(&g.join("RES").join("BIKE.RES")).ok());
                    }
                    if s.preview.as_ref().is_none_or(|p| *p.ride() != ride) {
                        s.preview = s
                            .bike_res
                            .as_deref()
                            .and_then(|b| rs2::Archive::parse(b).ok())
                            .and_then(|res| {
                                preview::Preview::new(
                                    &s.device,
                                    &s.queue,
                                    &res,
                                    &ride,
                                    s.config.format.add_srgb_suffix(),
                                )
                            });
                    }
                    if let Some(p) = &mut s.preview {
                        p.window = front.preview_window();
                        commands.push(
                            p.render(&s.device, &s.queue, view, size, now, self.frame, turning),
                        );
                    }
                }
                None => s.preview = None,
            }
        }
        s.queue.submit(commands);
        let shoot = std::mem::take(&mut self.shoot);
        // A browser can't wait for the picture to come back from the
        // GPU: the page saves the canvas, after it is shown.
        #[cfg(target_arch = "wasm32")]
        let web_photo = shoot;
        #[cfg(target_arch = "wasm32")]
        let shoot = false;
        if shoot {
            let said = match save_screenshot(&s.device, &s.queue, &frame.texture) {
                Ok(path) => format!("Screenshot saved: {}", path.file_name().unwrap_or_default().to_string_lossy()),
                Err(e) => {
                    eprintln!("screenshot: {e}");
                    "Couldn't save the screenshot".to_string()
                }
            };
            if self.photo {
                self.photo_note = Some((said, Instant::now()));
            } else if let Some((_, hud)) = &mut self.hud {
                hud.say(&said);
            }
        }
        s.window.pre_present_notify();
        s.queue.present(frame);
        #[cfg(target_arch = "wasm32")]
        if web_photo {
            let said = if web::save_picture() { "Picture saved to your downloads" } else { "Couldn't save the picture" };
            if self.photo {
                self.photo_note = Some((said.to_string(), Instant::now()));
            } else if let Some((_, hud)) = &mut self.hud {
                hud.say(said);
            }
        }
        if suboptimal {
            s.fit_surface();
        }
    }

    /// Acts on what the menus ask.
    fn next(&mut self, next: frontend::Next, event_loop: &ActiveEventLoop) {
        match next {
            frontend::Next::Stay => {}
            frontend::Next::Quit => event_loop.exit(),
            frontend::Next::Race(setup) => self.load(setup),
            frontend::Next::Replay(path) => self.start_replay(&path),
        }
    }
}

impl ApplicationHandler for App {
    fn exiting(&mut self, _: &ActiveEventLoop) {
        self.note_playtime();
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        {
            // A browser hands out the GPU later: once it has,
            // `window_event` finishes setting up.
            if web::ASKED.with(|a| a.get()) {
                return;
            }
            match Self::open_window(event_loop) {
                Ok((window, instance, surface)) => web::ask_gpu(window, instance, surface),
                Err(e) => web::fail(&e.to_string()),
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        match self.init(event_loop) {
            Ok(state) => {
                state.window.request_redraw();
                self.state = Some(state);
            }
            Err(e) => {
                eprintln!("error: {e}");
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        #[cfg(target_arch = "wasm32")]
        if self.state.is_none() {
            match web::take_gpu() {
                Some((window, surface, Ok(gpu))) => match self.make_state(window, surface, gpu) {
                    Ok(state) => {
                        state.window.request_redraw();
                        self.state = Some(state);
                        web::started();
                    }
                    Err(e) => web::fail(&e.to_string()),
                },
                Some((_, _, Err(e))) => web::fail(&e),
                None => {}
            }
        }
        let now = self.start.elapsed().as_secs_f32();
        if self.in_builder() && self.builder_event(&event) {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Focused(on) => {
                eprintln!("{now:.2}s: the game {} focus", if on { "has" } else { "lost" });
                self.quiet = !on;
                // Keys let go while another window had focus (Alt+Tab,
                // another browser tab) send no release here, so they
                // would stay held: the gas or a lean stuck on, or Alt
                // still down so the next Enter goes full screen.
                if !on {
                    self.keys.clear();
                }
                // An extra: the original played on in the background.
                if tweaks::get().mute_in_background {
                    self.audio.set_muted(!on);
                }
                self.feel_pad();
            }
            WindowEvent::Occluded(hidden) => {
                eprintln!("{now:.2}s: the game is {}", if hidden { "covered" } else { "uncovered" });
                self.quiet = hidden;
                self.feel_pad();
            }
            WindowEvent::Resized(size) => {
                if let Some(s) = &mut self.state {
                    (s.config.width, s.config.height) = drawn_size(size.width, size.height);
                    s.surface.configure(&s.device, &s.config);
                }
            }
            // A finger on a touch screen works as the mouse: the menus and
            // the replay player take a tap as a click. The race's own
            // touch buttons are the page's (web/touch.js).
            WindowEvent::Touch(t) => {
                use winit::event::TouchPhase;
                let device_id = t.device_id;
                self.window_event(event_loop, id, WindowEvent::CursorMoved { device_id, position: t.location });
                let state = match t.phase {
                    TouchPhase::Started => ElementState::Pressed,
                    TouchPhase::Ended | TouchPhase::Cancelled => ElementState::Released,
                    TouchPhase::Moved => return,
                };
                self.window_event(event_loop, id, WindowEvent::MouseInput { device_id, state, button: MouseButton::Left });
                // Told now, not at the next frame: the page opens the
                // phone's keyboard as this same tap ends, which is the
                // only time a phone lets it.
                #[cfg(target_arch = "wasm32")]
                web::typing(self.typing_text());
            }
            WindowEvent::CursorMoved { mut position, .. } => {
                if let Some(s) = &self.state {
                    let size = (s.config.width, s.config.height);
                    // The picture may be drawn smaller than the window
                    // (`--max-pixels`) and stretched over it.
                    let window = s.window.inner_size();
                    if window.width > 0 && window.height > 0 {
                        position.x *= size.0 as f64 / window.width as f64;
                        position.y *= size.1 as f64 / window.height as f64;
                    }
                    // The replay player is drawn stretched over the window,
                    // or over the race between black bars.
                    let stretch = self.replay.is_some() && !self.in_menus();
                    let (x, width) = match stretch {
                        true => bars::race_area(size),
                        false => (0, size.0),
                    };
                    self.pointer = sprites::Sprites::to_screen_as((width, size.1), position.x - x as f64, position.y, stretch);
                }
                if self.in_menus() {
                    let (x, y) = self.pointer;
                    if let Some(f) = &mut self.front {
                        f.mouse_move(&self.audio, x, y);
                    }
                }
                if let (Some(rp), Some((ui, _))) = (&mut self.replay, &mut self.hud) {
                    let (x, y) = self.pointer;
                    rp.mouse_move(ui, x, y);
                }
            }
            WindowEvent::MouseWheel { delta, .. } if self.in_menus() => {
                let lines = menu_wheel_lines(delta);
                if let Some(f) = &mut self.front {
                    f.wheel(lines);
                }
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } if self.in_menus() => {
                let (x, y) = self.pointer;
                let Some(f) = &mut self.front else { return };
                if state == ElementState::Pressed {
                    f.mouse_down(x, y);
                } else {
                    let next = f.mouse_up(&self.audio, x, y, now);
                    self.next(next, event_loop);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                // The port's own keys, not the game's: Alt+Enter for full
                // screen, F2 for the frame rate, and with the enhanced
                // graphics F3 to compare them with the original look.
                if let (ElementState::Pressed, false, PhysicalKey::Code(code)) =
                    (event.state, event.repeat, event.physical_key)
                {
                    let alt = self.keys.contains(&KeyCode::AltLeft) || self.keys.contains(&KeyCode::AltRight);
                    if alt && matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
                        if let Some(s) = &self.state {
                            let full = s.window.fullscreen().is_none();
                            s.window.set_fullscreen(full.then_some(winit::window::Fullscreen::Borderless(None)));
                        }
                        return;
                    }
                    if code == KeyCode::F2 {
                        let mut t = tweaks::get();
                        t.show_fps = !t.show_fps;
                        tweaks::set(t);
                        return;
                    }
                    if code == KeyCode::F3 && graphics::enhanced().is_some() {
                        let on = graphics::toggle();
                        eprintln!("graphics: {}", if on { "enhanced" } else { "original" });
                        return;
                    }
                }
                // Typing into the menus' boxes.
                if self.in_menus() && event.state == ElementState::Pressed {
                    if let Some(f) = &mut self.front {
                        // The controls page takes the key itself.
                        if let PhysicalKey::Code(code) = event.physical_key {
                            if code != KeyCode::Escape && !event.repeat {
                                let next = f.key(&self.audio, code);
                                if !matches!(next, frontend::Next::Stay) {
                                    self.next(next, event_loop);
                                    return;
                                }
                            }
                        }
                        let Some(f) = &mut self.front else { return };
                        if event.physical_key == PhysicalKey::Code(KeyCode::Backspace) {
                            f.type_char('\u{8}');
                        } else if let Some(text) = &event.text {
                            text.chars().for_each(|c| f.type_char(c));
                        }
                    }
                }
                // A chat line takes the keys while it is typed.
                if self.chat.is_some() && event.state == ElementState::Pressed {
                    self.type_chat(&event);
                    return;
                }
                if let PhysicalKey::Code(code) = event.physical_key {
                    let photo = self.photo;
                    if code == KeyCode::Escape && event.state == ElementState::Pressed {
                        if photo {
                            self.leave_photo();
                        } else if self.in_menus() {
                            if let Some(f) = &mut self.front {
                                let next = f.escape(now);
                                self.next(next, event_loop);
                            }
                        } else if self.front.is_some() || self.builder.is_some() {
                            if self.loading.is_none() {
                                self.end_race();
                            }
                        } else {
                            event_loop.exit();
                        }
                    }
                    // Photo mode takes the keys: F or Escape goes back,
                    // Enter saves a picture, and the camera's keys fly.
                    if photo && event.state == ElementState::Pressed && !event.repeat {
                        match code {
                            KeyCode::KeyF => self.leave_photo(),
                            KeyCode::KeyP if self.ctrl() => self.shoot = true,
                            KeyCode::Enter | KeyCode::NumpadEnter if !self.keys.contains(&KeyCode::AltLeft) && !self.keys.contains(&KeyCode::AltRight) => self.shoot = true,
                            _ => {}
                        }
                    }
                    if !photo && event.state == ElementState::Pressed && !event.repeat && self.track.is_some()
                    {
                        // Enter or Space ends the podium (0x460190).
                        let podium = self.race.as_ref().is_some_and(|r| r.podium.is_some());
                        if podium && matches!(code, KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space) {
                            self.back_to_menu();
                            return;
                        }
                        match code {
                            KeyCode::Tab => self.follow = !self.follow && self.race.is_some(),
                            KeyCode::KeyR => {
                                self.leave_vehicle(false);
                                if let (Some(r), Some(t)) = (&mut self.race, &self.track) {
                                    r.restart(&t.ground());
                                }
                            }
                            KeyCode::Equal | KeyCode::NumpadAdd => {
                                if let Some((_, hud)) = &mut self.hud {
                                    hud.zoom_by(-1.0);
                                }
                            }
                            KeyCode::Minus | KeyCode::NumpadSubtract => {
                                if let Some((_, hud)) = &mut self.hud {
                                    hud.zoom_by(1.0);
                                }
                            }
                            // Numpad 5 cycles the views (0x467ec0); C too,
                            // for keyboards without a number pad.
                            KeyCode::KeyS if self.ctrl() => self.flip_sound(),
                            KeyCode::KeyV if self.ctrl() => self.flip_reverb(),
                            KeyCode::KeyC if self.ctrl() => {
                                let said = match (&mut self.sounds, &self.audio) {
                                    (Some(s), a) => s.step_crowd(a),
                                    _ => None,
                                };
                                if let (Some(id), Some((ui, hud))) = (said, &mut self.hud) {
                                    hud.say(&ui.lang(id));
                                }
                            }
                            KeyCode::Numpad0 if self.follow => {
                                if let Some(r) = &mut self.race {
                                    r.freeze_camera();
                                }
                            }
                            // F1: the Helmet Cam, straight in and out
                            // (on a Mac keyboard, fn+F1 unless the F keys are
                            // set as standard function keys).
                            KeyCode::F1 if self.follow => {
                                if let Some(name) = self.race.as_mut().map(|r| r.toggle_helmet()) {
                                    if let Some((_, hud)) = &mut self.hud {
                                        hud.say(name);
                                    }
                                }
                            }
                            KeyCode::Numpad5 | KeyCode::KeyC if self.follow => {
                                if let Some(r) = &mut self.race {
                                    let name = r.next_view();
                                    if let Some((_, hud)) = &mut self.hud {
                                        hud.say(name);
                                    }
                                }
                            }
                            // Ctrl+G and Ctrl+B flip the gyros; Ctrl and a
                            // digit sends that chat message, and / opens the
                            // chat (Shift+/ its mode) in a network race;
                            // [ and ], Page Up and Down watch the others
                            // (0x4203e0).
                            KeyCode::KeyG | KeyCode::KeyB if self.ctrl() => self.flip_gyro(code == KeyCode::KeyG),
                            // G alone: the fork's quick get-up.
                            KeyCode::KeyG => self.get_back_on(),
                            // V alone: into or out of a vehicle (Extras).
                            KeyCode::KeyV => self.toggle_drive(),
                            KeyCode::Digit0
                            | KeyCode::Digit1
                            | KeyCode::Digit2
                            | KeyCode::Digit3
                            | KeyCode::Digit4
                            | KeyCode::Digit5
                            | KeyCode::Digit6
                            | KeyCode::Digit7
                            | KeyCode::Digit8
                            | KeyCode::Digit9
                                if self.ctrl() && self.net.is_some() =>
                            {
                                let d = code as usize - KeyCode::Digit0 as usize;
                                let line = self.front.as_ref().and_then(|f| f.user().profile.messages.get(d).cloned());
                                if let Some(line) = line.filter(|l| !l.is_empty()) {
                                    self.send_chat(&line);
                                }
                            }
                            KeyCode::Slash if self.net.is_some() => {
                                let shift = self.keys.contains(&KeyCode::ShiftLeft) || self.keys.contains(&KeyCode::ShiftRight);
                                self.chat_key(shift);
                            }
                            KeyCode::BracketLeft | KeyCode::PageUp | KeyCode::BracketRight | KeyCode::PageDown => {
                                if let Some(r) = &mut self.race {
                                    r.watch_next(matches!(code, KeyCode::BracketRight | KeyCode::PageDown));
                                }
                            }
                            // The race's switches (0x4e54c0).
                            KeyCode::F4 => self.flip_cues(),
                            KeyCode::F11 | KeyCode::F12 => self.step_quality(if code == KeyCode::F12 { 1 } else { -1 }),
                            KeyCode::F5 | KeyCode::F6 | KeyCode::F7 | KeyCode::F8 | KeyCode::F9 | KeyCode::F10 => {
                                self.flip_switch(code);
                            }
                            KeyCode::KeyN if self.ctrl() => {
                                if let Some((ui, hud)) = &mut self.hud {
                                    let (what, on) = hud.show.cycle_labels();
                                    hud.say_switch(ui, what, on);
                                }
                                self.keep_switches();
                            }
                            KeyCode::Home | KeyCode::End => {
                                if let Some(r) = &mut self.race {
                                    r.zoom(if code == KeyCode::Home { -1.0 } else { 1.0 });
                                }
                            }
                            KeyCode::KeyP if self.ctrl() && tweaks::get().screenshot_key => self.shoot = true,
                            // F: photo mode (an extra).
                            KeyCode::KeyF if !self.ctrl() => self.enter_photo(),
                            KeyCode::KeyP => {
                                if let Some(r) = &mut self.race {
                                    r.autopilot = !r.autopilot;
                                }
                            }
                            _ => {}
                        }
                    }
                    match event.state {
                        ElementState::Pressed => self.keys.insert(code),
                        ElementState::Released => self.keys.remove(&code),
                    };
                }
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } if self.replay.is_some() => {
                let (x, y) = self.pointer;
                let (Some(rp), Some((ui, _))) = (&mut self.replay, &mut self.hud) else {
                    return;
                };
                if state == ElementState::Pressed {
                    rp.mouse_down(ui, x, y);
                    return;
                }
                match rp.mouse_up(ui, x, y) {
                    replay::Ask::Stay => {}
                    replay::Ask::Camera => {
                        if let Some(r) = &mut self.race {
                            r.next_view();
                        }
                    }
                    replay::Ask::Exit => self.back_to_menu(),
                    replay::Ask::Save => {
                        let dir = self.front.as_ref().map(|f| f.user().store.dir.clone());
                        let track = self.track.as_ref().map_or("", |t| t.name.as_str());
                        if let Some(dir) = dir {
                            match replay::save(&rp.path, &dir, track) {
                                Ok(p) => eprintln!("saved the replay as {}", p.display()),
                                Err(e) => eprintln!("saving the replay: {e}"),
                            }
                        }
                    }
                }
            }
            WindowEvent::MouseInput {
                button: MouseButton::Right,
                state,
                ..
            } if self.track.is_some() => {
                self.looking = state == ElementState::Pressed;
                if let Some(s) = &self.state {
                    let mode = if self.looking {
                        CursorGrabMode::Locked
                    } else {
                        CursorGrabMode::None
                    };
                    // Some platforms only support Confined; looking still works without a grab.
                    let _ = s
                        .window
                        .set_cursor_grab(mode)
                        .or_else(|_| s.window.set_cursor_grab(CursorGrabMode::Confined));
                    s.window.set_cursor_visible(!self.looking && !ui::draws_pointer());
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(n) = self.front.as_mut().and_then(|f| f.take_next()) {
                    self.next(n, event_loop);
                }
                if self.builder_waiting.is_some() && crate::fs::all_read() {
                    let file = self.builder_waiting.take().flatten();
                    #[cfg(target_arch = "wasm32")]
                    web::track_loading(false);
                    self.open_builder(file.as_deref());
                }
                let now = Instant::now();
                let real = (now - self.last).as_secs_f32().min(0.1);
                self.last = now;
                // Waiting for the screen, the world moves on in whole beats
                // of it, so what moves across it does so evenly.
                // (Asked again each second: the window may move to
                // another screen.)
                if self.beat.0.is_none_or(|at| now - at > std::time::Duration::from_secs(1)) {
                    let hz = self.state.as_ref().and_then(|s| s.window.current_monitor()).and_then(|m| m.refresh_rate_millihertz());
                    self.beat = (Some(now), hz.map(|mhz| 1000.0 / mhz.max(1) as f32));
                }
                let beat = self.beat.1.filter(|_| tweaks::get().vsync);
                let dt = self.pace.even(real, beat);
                self.frame = dt;
                // Dynamic resolution: the time a frame should take is the
                // screen's beat with VSync (60 fps on a faster screen:
                // sharpness first past that), else the frame cap's or 60 fps.
                #[cfg(not(target_arch = "wasm32"))]
                let cap = tweaks::more().fps_cap;
                #[cfg(target_arch = "wasm32")]
                let cap = 0;
                let free = if cap > 0 { 1.0 / cap as f32 } else { 1.0 / 60.0 };
                let wanted = if tweaks::get().vsync { self.beat.1.unwrap_or(0.0).max(1.0 / 60.0) } else { free };
                let racing = self.track.is_some() && self.loading.is_none();
                let governing = racing && graphics::showing().is_some_and(|e| e.dynamic);
                // In a browser, the whole picture too, in either look.
                #[cfg(target_arch = "wasm32")]
                if let Some(s) = &self.state {
                    let size = s.window.inner_size();
                    self.picture.frame(racing, governing, real, wanted, most_drawn(size.width, size.height));
                }
                if governing {
                    self.dynres.apply(real, wanted);
                } else if self.track.is_none() {
                    self.dynres = dynres::Governor::default();
                    dynres::reset();
                }
                self.poll_pads(event_loop, dt);
                self.update(dt);
                self.builder_tick(dt);
                if self.quit {
                    event_loop.exit();
                    return;
                }
                self.feel_pad();
                self.redraw();
                #[cfg(target_arch = "wasm32")]
                web::racing(self.race.is_some() && self.replay.is_none() && !self.in_menus());
                #[cfg(target_arch = "wasm32")]
                web::building(self.in_builder());
                #[cfg(target_arch = "wasm32")]
                web::typing(self.typing_text());
                if tweaks::get().show_fps != self.fps.is_some() {
                    self.fps = self.fps.is_none().then(|| (Instant::now(), 0, 0.0));
                    self.fps_shown = None;
                }
                if let Some((since, frames, slowest)) = &mut self.fps {
                    *frames += 1;
                    *slowest = slowest.max(dt);
                    let span = since.elapsed().as_secs_f32();
                    if span >= 1.0 {
                        let text = format!(
                            "{:.0} fps ({:.1} ms a frame, slowest {:.1} ms)",
                            *frames as f32 / span,
                            span * 1000.0 / *frames as f32,
                            *slowest * 1000.0
                        );
                        eprintln!("{text}");
                        self.fps_shown = Some((*frames as f32 / span).round() as u32);
                        self.fps = Some((Instant::now(), 0, 0.0));
                    }
                }
                // The frame cap extra: a frame no sooner than its share of a
                // second after the last began.
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let cap = tweaks::more().fps_cap;
                    if cap > 0 {
                        let next = now + std::time::Duration::from_secs_f64(1.0 / cap as f64);
                        let left = next.saturating_duration_since(Instant::now());
                        if !left.is_zero() {
                            std::thread::sleep(left);
                        }
                    }
                }
                if let Some(s) = &self.state {
                    s.window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: winit::event::DeviceId, event: DeviceEvent) {
        if let (true, DeviceEvent::MouseMotion { delta }, Some(s)) =
            (self.looking, event, &mut self.state)
        {
            s.camera
                .look(delta.0 as f32 * 0.003, delta.1 as f32 * 0.003);
        }
    }
}

/// The OpenMCM2 tyre for the title bar and taskbar (`assets/icon`).
fn window_icon() -> Option<winit::window::Icon> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(&include_bytes!("../../../assets/icon/icon-256.png")[..]));
    decoder.set_transformations(png::Transformations::normalize_to_color8() | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    winit::window::Icon::from_rgba(buf, info.width, info.height).ok()
}

thread_local! {
    /// The part of a line the menus' wheel has turned and not yet scrolled.
    static MENU_WHEEL: std::cell::Cell<f32> = const { std::cell::Cell::new(0.0) };
}

/// How many lines a turn of the wheel scrolls the menus' lists, keeping
/// the part of a line left over: a Mac's wheel and trackpad send small
/// parts of a line (once rounded to nothing), and a browser sends about
/// 100 pixels a notch (once seven lines).
fn menu_wheel_lines(delta: winit::event::MouseScrollDelta) -> i32 {
    let turn = match delta {
        winit::event::MouseScrollDelta::LineDelta(_, y) => y,
        winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
    };
    MENU_WHEEL.with(|w| {
        // A turn the other way starts afresh.
        let mut total = if w.get() * turn < 0.0 { turn } else { w.get() + turn };
        let lines = total.trunc();
        total -= lines;
        w.set(total);
        lines as i32
    })
}

#[cfg(test)]
mod drawn_size_tests {
    use super::fit_pixels;

    #[test]
    fn a_phone_screen_is_drawn_smaller_in_the_same_shape() {
        assert_eq!(fit_pixels(2532, 1170, Some(1280 * 720)), (1412, 652));
        assert_eq!(fit_pixels(1280, 720, Some(1280 * 720)), (1280, 720));
        assert_eq!(fit_pixels(2532, 1170, None), (2532, 1170));
    }
}
