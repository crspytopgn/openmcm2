//! Race sounds as the game's `Audio.cpp` (RaceSound) plays them
//! (`docs/physics.md` section 15, `docs/sound.md`).
//!
//! The engine note is not pitch-shifted: recorded samples from
//! `RES/ENGINE.RES` (listed in `Audio_16.ini`, or `Audio_08.ini` for 8-bit
//! sound) play whole, one after another, in a small state machine
//! (0x4e8720, 0x4e9aa0). Opening the throttle picks a torque band by speed,
//! which runs on into the power band, which alternates with sputters;
//! closing it plays the decel, which runs on into the idle loop. A change
//! of state rewrites the stream from the write cursor, so it is heard at
//! once. Each bike uses its own engine class's samples; the four bikes
//! nearest the camera have a voice.
//!
//! Every engine, landing, wreck and fall is a DirectSound 3D buffer: the
//! listener is the camera, moving with the followed bike, and each sound
//! falls off as min/distance between its minimum and maximum distance, is
//! silent beyond the maximum (`DSBCAPS_MUTE3DATMAXDISTANCE`, 0x4c1c90) and
//! is Doppler-shifted (0x4e81a0: distance factor 0.3048 m/ft, rolloff and
//! Doppler factor 1). Crashes, landings, the crowd and its reactions come
//! from `RES/AUDIO.RES`.

use crate::audio::{Audio, Voice};
use crate::frontend::EventKind;
use crate::race::Race;
use glam::Vec3;
use mm2_formats::{rs2, scn, wav::Sound};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

/// The engine sample lists of `Audio_16.ini`, in the order the game reads
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum List {
    Idle,
    LowTorque,
    MidTorque,
    HighTorque,
    PowerBand,
    Decel,
    Sputter,
    Rev,
}

const LISTS: [(List, &str); 8] = [
    (List::Idle, "Idle"),
    (List::LowTorque, "LowTorque"),
    (List::MidTorque, "MidTorque"),
    (List::HighTorque, "HighTorque"),
    (List::PowerBand, "PowerBand"),
    (List::Decel, "Decel"),
    (List::Sputter, "Sputter"),
    (List::Rev, "Rev"),
];

/// The engine classes: the original's three ini sections (below 250 cc,
/// two-strokes from 250 cc, the four-strokes; bike +0x738, +0x737), then
/// the free pack's big two-strokes and big four-strokes, which the
/// original plays with its `[250]` and `[400]` samples.
const CLASSES: [&str; 5] = ["125", "250", "400", "500", "600"];
/// The original's section for each class.
const ORIGINAL: [usize; 5] = [0, 1, 2, 1, 2];

/// ft/s to mph, as the game converts (0x55ca00).
const MPH: f32 = 0.68182;
/// Engines heard at once.
const VOICES: usize = 4;
/// Bikes farther than this, side to side or front to back, all count as
/// this far when the voices are handed out (0x4e91d0).
const FAR: f32 = 500.0;
/// In the air this long, the engine note changes (0x555868).
const AIR: f32 = 0.2;
/// The speed of sound in ft/s: DirectSound's 343 m/s over the game's
/// distance factor of 0.3048 m/ft.
const SOUND_SPEED: f32 = 343.0 / 0.3048;
/// Each buffer's minimum and maximum distance, ft (0x4e81a0): the engines,
/// wrecks and falls 50 to 500, the hard landing 10 to 500.
const ENGINE_RANGE: (f32, f32) = (50.0, 500.0);
const LANDING_RANGE: (f32, f32) = (10.0, 500.0);
/// The crowd's reactions wait this long between plays, s (0x555bf8,
/// 0x555800).
const OH_WAIT: f32 = 30.0;
const APPLAUSE_WAIT: f32 = 15.0;
/// A stunt worth this many points rings the bonus (0x555bfc).
const BONUS_POINTS: f32 = 10000.0;
/// The Free Pack's out-of-bounds slide whistle, up as the rider is thrown
/// back off the world's edge and down as they fall (tools/freepack/
/// make_whistle.py).
const WHISTLE_UP: &str = "whistle_up.wav";
const WHISTLE_DOWN: &str = "whistle_down.wav";

/// The engine class of a bike of `cc` (0x4e7160).
pub fn class(cc: u32, four_stroke: bool) -> usize {
    match (cc, four_stroke) {
        (cc, _) if cc < 250 => 0,
        (cc, true) if cc >= 500 => 4,
        (_, true) => 2,
        (cc, false) if cc >= 450 => 3,
        _ => 1,
    }
}

/// The player's sound options that the race reads.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// The whole race's volume, 0 to 1 (0 with In-Game Sound Effects off).
    pub gain: f32,
    /// Waypoint Chime.
    pub chime: bool,
    /// 16-bit sound: `Audio_16.ini`'s samples, else `Audio_08.ini`'s.
    pub sixteen_bit: bool,
}

impl Default for Options {
    /// A new profile's: full volume, the chime, 16-bit sound.
    fn default() -> Self {
        Options::from_profile(&crate::profile::SoundOptions::default())
    }
}

impl Options {
    /// From the player's Sound page: the In-Game Volume slider gives the
    /// primary buffer `volume × 25 − 2500` hundredths of a decibel, and
    /// nothing at 0 (0x4e81a0).
    pub fn from_profile(s: &crate::profile::SoundOptions) -> Options {
        let hundredths = s.game_volume.min(100) as f32 * 25.0 - 2500.0;
        let gain = if !s.effects || hundredths <= -2500.0 {
            0.0
        } else {
            10f32.powf(hundredths / 2000.0)
        };
        Options {
            gain,
            chime: s.waypoint_chime,
            sixteen_bit: s.sixteen_bit,
        }
    }
}

/// Where the race is heard from: the camera, its right-hand side, and the
/// followed bike's velocity (0x4e84a0 gives the listener the camera's
/// place and the followed bike's velocity).
#[derive(Debug, Clone, Copy)]
pub struct Listener {
    pub eye: Vec3,
    pub right: Vec3,
    pub velocity: Vec3,
}

impl Listener {
    /// How a 3D buffer at `at`, moving at `velocity`, with minimum and
    /// maximum distances `range`, is heard: its volume, pan and Doppler
    /// pitch. The volume is min/distance between the two (rolloff 1), and
    /// nothing beyond the maximum: every 3D buffer is made with
    /// `DSBCAPS_MUTE3DATMAXDISTANCE` (0x4c1c90's flags `| 0x20010`). The
    /// pan follows the side the sound is on **[likely]**: DirectSound's
    /// software 3D pans by the sound's direction, and how hard is not in
    /// the game.
    pub fn hear(&self, at: Vec3, velocity: Vec3, range: (f32, f32)) -> (f32, f32, f32) {
        let to = at - self.eye;
        let distance = to.length();
        let volume = if distance > range.1.max(range.0) {
            0.0
        } else {
            range.0 / distance.max(range.0.max(1e-3))
        };
        let toward = to.normalize_or_zero();
        let pan = toward.dot(self.right);
        // Both closing speeds along the line between them.
        let pitch = (SOUND_SPEED + self.velocity.dot(toward))
            / (SOUND_SPEED + velocity.dot(toward)).max(1.0);
        (volume.min(1.0), pan, pitch)
    }
}

/// One engine class (`[125]`, `[250]` or `[400]`).
struct Bank {
    lists: Vec<Vec<Arc<Sound>>>,
    /// The tops of the low, mid and high torque bands, mph.
    bands: [i32; 3],
    /// The free pack's steady loops, played by revs instead of the
    /// original's whole samples (see [`Revs`]).
    loops: Option<Loops>,
    /// Seconds each sample fades into the next over: none for the game's
    /// own samples, which are cut to follow each other, and [`BLEND`] for
    /// the free pack's recordings, cut from longer takes, so the seams
    /// and the jumps between takes aren't heard on your own bike.
    blend: f32,
}

/// How long a free pack sample fades into the next, s.
const BLEND: f32 = 0.12;

/// A free pack class's engine as steady loops, each recorded at a known
/// engine speed, pitched to the revs of the moment: on the gas, off it,
/// and at idle (`engine.ini` in the class's folder). With several loops
/// on and off the gas (`OnLoops = a.wav@3000, b.wav@4500, ...`), the two
/// nearest the revs are heard together, so none is pitched far.
struct Loops {
    /// Each loop and the revs it was recorded at, slowest first.
    on: Vec<(Arc<Sound>, f32)>,
    off: Vec<(Arc<Sound>, f32)>,
    idle: Arc<Sound>,
    idle_rpm: f32,
    /// The engine's idle and its top, rpm.
    low: f32,
    high: f32,
    /// Top speed in each gear, mph.
    gears: Vec<f32>,
    /// From an MX Bikes style `engine.scl` (`Scl = engine.scl`): each on
    /// and off loop's volume at points along the revs (rpm, volume), in
    /// place of crossfading neighbours. The off layer starts at idle, so
    /// the separate idle loop is then silent.
    points: Option<(Vec<Vec<(f32, f32)>>, Vec<Vec<(f32, f32)>>)>,
}

impl Loops {
    /// Whether the class's `engine.ini` asks for its loops over the
    /// whole samples (`RevBlend = 1`).
    fn wanted(dir: &Path) -> bool {
        let Ok(text) = crate::fs::read_to_string(dir.join("engine.ini")) else {
            return false;
        };
        let ini = scn::Ini::parse(&text);
        ini.sections
            .iter()
            .find_map(|s| s.get("RevBlend"))
            .is_some_and(|v| matches!(v.trim(), "1" | "true" | "yes"))
    }

    fn load(dir: &Path) -> Option<Loops> {
        let ini = scn::Ini::parse(&crate::fs::read_to_string(dir.join("engine.ini")).ok()?);
        let get = |key: &str| ini.sections.iter().find_map(|s| s.get(key)).map(|v| v.trim().to_string());
        let num = |key: &str| get(key).and_then(|v| v.parse::<f32>().ok());
        // `OnLoop.wav` is found as `OnLoop.ogg` too, and back.
        let file = |name: &str| {
            let bytes = crate::fs::read(dir.join(name))
                .or_else(|e| mm2_formats::wav::other_name(name).and_then(|n| crate::fs::read(dir.join(n)).ok()).ok_or(e))
                .map_err(|e| eprintln!("{}: {e}", dir.join(name).display()))
                .ok()?;
            mm2_formats::wav::decode(&bytes).ok().map(Arc::new)
        };
        let wav = |key: &str| file(&get(key)?);
        // `OnLoops = a.wav@3000, ...`, or the one `OnLoop` at `OnLoopRpm`.
        let set = |many: &str, one: &str, rpm: &str| -> Option<Vec<(Arc<Sound>, f32)>> {
            let mut list: Vec<(Arc<Sound>, f32)> = get(many)
                .map(|v| {
                    v.split(',')
                        .filter_map(|item| {
                            let (name, at) = item.trim().rsplit_once('@')?;
                            Some((file(name.trim())?, at.trim().parse::<f32>().ok()?))
                        })
                        .collect()
                })
                .unwrap_or_default();
            if list.is_empty() {
                list.push((wav(one)?, num(rpm)?));
            }
            list.sort_by(|a, b| a.1.total_cmp(&b.1));
            Some(list)
        };
        let gears: Vec<f32> = get("GearTopSpeeds")
            .map(|v| v.split(',').filter_map(|g| g.trim().parse().ok()).collect())
            .unwrap_or_else(|| GEARS.to_vec());
        let gears = if gears.is_empty() { vec![75.0] } else { gears };
        if let Some(scl) = get("Scl") {
            let layers = Scl::parse(&crate::fs::read_to_string(dir.join(&scl)).ok()?);
            let mut sets = Vec::new();
            for layer in layers.iter().take(2) {
                let mut sounds = Vec::new();
                let mut points = Vec::new();
                for step in layer {
                    sounds.push((file(&step.wav)?, step.reference));
                    points.push(step.points.clone());
                }
                sets.push((sounds, points));
            }
            let (off, off_points) = sets.pop()?;
            let (on, on_points) = sets.pop()?;
            return Some(Loops {
                idle: off.first()?.0.clone(),
                idle_rpm: off.first()?.1,
                on,
                off,
                low: num("IdleRpm").unwrap_or(1600.0),
                high: num("MaxRpm").unwrap_or(10000.0),
                gears,
                points: Some((on_points, off_points)),
            });
        }
        let gears: Vec<f32> = get("GearTopSpeeds")
            .map(|v| v.split(',').filter_map(|g| g.trim().parse().ok()).collect())
            .unwrap_or_else(|| GEARS.to_vec());
        Some(Loops {
            on: set("OnLoops", "OnLoop", "OnLoopRpm")?,
            off: set("OffLoops", "OffLoop", "OffLoopRpm")?,
            idle: wav("IdleLoop")?,
            idle_rpm: num("IdleLoopRpm")?,
            low: num("IdleRpm").unwrap_or(1600.0),
            high: num("MaxRpm").unwrap_or(10000.0),
            gears: if gears.is_empty() { vec![75.0] } else { gears },
            points: None,
        })
    }
}

/// One loop of an MX Bikes `engine.scl` layer: its file, the revs it was
/// recorded at, and its volume at points along the revs.
#[derive(Debug, Clone, PartialEq)]
struct SclStep {
    wav: String,
    reference: f32,
    points: Vec<(f32, f32)>,
}

/// An MX Bikes engine sound layout (`engine.scl`): `MaxValue`, then
/// `Layer0` (on the gas) and `Layer1` (off it), each a list of samples
/// with `Wav`, `Point`s of `Value` (a share of `MaxValue`) and `Volume`,
/// and `RefPitch` (the revs it was recorded at, also a share).
struct Scl;

impl Scl {
    fn parse(text: &str) -> Vec<Vec<SclStep>> {
        let mut max = 1.0f32;
        let mut layers: Vec<Vec<SclStep>> = Vec::new();
        let mut value = None;
        for line in text.lines() {
            let line = line.split(";;").next().unwrap_or("").trim();
            let (key, v) = match line.split_once('=') {
                Some((k, v)) => (k.trim(), v.trim()),
                None => (line, ""),
            };
            let n = v.parse::<f32>().ok();
            let step = layers.last_mut().and_then(|l| l.last_mut());
            match key {
                "MaxValue" => max = n.unwrap_or(1.0),
                k if k.starts_with("Layer") => layers.push(Vec::new()),
                "Wav" => {
                    if let Some(l) = layers.last_mut() {
                        let name = v.rsplit(['\\', '/']).next().unwrap_or(v).to_string();
                        l.push(SclStep { wav: name, reference: 0.0, points: Vec::new() });
                    }
                }
                "Value" => value = n.map(|x| x * max),
                "Volume" => {
                    if let (Some(s), Some(at), Some(vol)) = (step, value.take(), n) {
                        s.points.push((at, vol));
                    }
                }
                "RefPitch" => {
                    if let (Some(s), Some(r)) = (step, n) {
                        s.reference = r * max;
                    }
                }
                _ => {}
            }
        }
        layers.retain(|l| !l.is_empty());
        layers
    }
}

/// The volume a set of (rpm, volume) points gives at `rpm`: straight
/// lines between them, and silence past either end.
fn trapezoid(points: &[(f32, f32)], rpm: f32) -> f32 {
    match points {
        [] => 0.0,
        [only] => only.1,
        _ => {
            let (first, last) = (points[0], points[points.len() - 1]);
            // Below the first point only a loop that starts at 0 is
            // heard; past the last, it holds at its last volume.
            if rpm < first.0 {
                return if first.0 <= 0.0 { first.1 } else { 0.0 };
            }
            if rpm > last.0 {
                return last.1;
            }
            for w in points.windows(2) {
                let ((a, va), (b, vb)) = (w[0], w[1]);
                if rpm <= b {
                    let t = if b > a { (rpm - a) / (b - a) } else { 1.0 };
                    return va + (vb - va) * t;
                }
            }
            points[points.len() - 1].1
        }
    }
}

/// Top speed in each gear, mph, unless the class's `engine.ini` gives its
/// own: long gears, so each pull to the shift lasts a second or more (the
/// bikes reach 50 mph in about three seconds).
const GEARS: [f32; 4] = [32.0, 50.0, 68.0, 88.0];

/// One bike's revs for the free pack's loops: a five-speed box worked out
/// from the bike's speed, the clutch slipping at a standstill, and free
/// revving in the air and at the gate.
#[derive(Default)]
struct Revs {
    rpm: f32,
    gear: usize,
    /// The road speed the revs follow, mph, smoothed: the bike's own
    /// jumps about over every bump.
    mph: f32,
    /// How much of the on-gas loop is heard, 0 to 1.
    load: f32,
    /// The loops' voices (on, off, idle, as [`Loops::all`] lists them),
    /// each playing only while it is heard.
    voices: Vec<Option<Voice>>,
}

impl Revs {
    fn step(&mut self, loops: &Loops, mph: f32, gas: f32, grounded: bool, dt: f32) {
        let (low, high) = (loops.low, loops.high);
        if self.rpm == 0.0 {
            self.rpm = low;
        }
        self.mph += (mph - self.mph) * (1.0 - (-4.0 * dt).exp());
        let mph = self.mph;
        let target = if grounded {
            // The gear that keeps the revs under the shift point; down a
            // gear only once they fall well below it.
            let top = |g: usize| loops.gears[g.min(loops.gears.len() - 1)];
            while self.gear + 1 < loops.gears.len() && mph > top(self.gear) * 0.97 {
                self.gear += 1;
            }
            while self.gear > 0 && mph < top(self.gear - 1) * 0.6 {
                self.gear -= 1;
            }
            let wheel = high * mph / top(self.gear);
            // Pulling away the clutch slips: the revs hold up with the gas
            // until the wheel catches them.
            let slip = low + (high * 0.7 - low) * gas;
            wheel.max(if self.gear == 0 { slip } else { low })
        } else {
            low + (high * 0.95 - low) * gas
        };
        let target = target.clamp(low, high);
        // Revs climb steadily; on the gas they fall only at a shift (or
        // the clutch going in at the start), which is quick; off it they
        // die away more slowly.
        let rate = if target > self.rpm {
            6.0
        } else if gas > 0.1 {
            16.0
        } else {
            5.0
        };
        self.rpm += (target - self.rpm) * (1.0 - (-rate * dt).exp());
        let load = if gas > 0.1 { 1.0 } else { 0.0 };
        self.load += (load - self.load) * (1.0 - (-14.0 * dt).exp());
    }

    /// Each loop's volume and pitch, in [`Loops::all`]'s order.
    fn mix(&self, loops: &Loops) -> Vec<(f32, f32)> {
        if let Some((on, off)) = &loops.points {
            // As MX Bikes plays them: the on layer by the throttle, the
            // off layer by what's left, each loop at its own volume for
            // the revs and pitched from the revs it was recorded at.
            let mut out = Vec::with_capacity(on.len() + off.len() + 1);
            for (set, points, level) in [(&loops.on, on, self.load), (&loops.off, off, 1.0 - self.load)] {
                for ((_, at), p) in set.iter().zip(points) {
                    out.push((level * trapezoid(p, self.rpm), self.rpm / at.max(1.0)));
                }
            }
            out.push((0.0, 1.0));
            return out;
        }
        let near_idle = ((loops.low * 1.7 - self.rpm) / (loops.low * 0.7)).clamp(0.0, 1.0);
        let off = 1.0 - self.load;
        let mut out = Vec::with_capacity(loops.on.len() + loops.off.len() + 1);
        for (set, level) in [(&loops.on, self.load), (&loops.off, off * (1.0 - near_idle) * 0.7)] {
            let weights = blend(set, self.rpm);
            out.extend(set.iter().zip(weights).map(|((_, at), w)| (level * w, self.rpm / at)));
        }
        out.push((off * near_idle * 0.8, self.rpm / loops.idle_rpm));
        out
    }
}

/// How much of each loop in `set` (slowest first) to hear at `rpm`: the
/// two recorded either side of it, crossfaded at equal power.
fn blend(set: &[(Arc<Sound>, f32)], rpm: f32) -> Vec<f32> {
    let mut w = vec![0.0; set.len()];
    let Some(i) = set.iter().position(|(_, at)| *at >= rpm) else {
        *w.last_mut().unwrap() = 1.0;
        return w;
    };
    if i == 0 {
        w[0] = 1.0;
        return w;
    }
    let (a, b) = (set[i - 1].1, set[i].1);
    // Each loop alone over most of its stretch, crossfaded only across
    // the middle 40% of the way to the next: two takes heard together
    // for long sound doubled, like an echo.
    let t = ((rpm / a).ln() / (b / a).ln());
    let t = ((t - 0.3) / 0.4).clamp(0.0, 1.0);
    w[i - 1] = (t * std::f32::consts::FRAC_PI_2).cos();
    w[i] = (t * std::f32::consts::FRAC_PI_2).sin();
    w
}

impl Loops {
    /// Every loop: on the gas, off it, then idle.
    fn all(&self) -> Vec<&Arc<Sound>> {
        self.on.iter().chain(&self.off).map(|(s, _)| s).chain([&self.idle]).collect()
    }
}

impl Bank {
    /// `divisor` thins every list of more than one sample, to no fewer
    /// than one (0x4e7160, +0x468). `freshen`: through the freshened
    /// engine sounds' chain (`freshen.rs`).
    fn load(res: &Path, sixteen_bit: bool, class: &str, divisor: usize, freshen: bool) -> Option<Bank> {
        let bytes = crate::read_archive(&res.join("ENGINE.RES"))
            .map_err(|e| eprintln!("{e}"))
            .ok()?;
        let ar = rs2::Archive::parse(&bytes).ok()?;
        let file = |name: &str| {
            ar.entries()
                .iter()
                .find(|e| e.name.eq_ignore_ascii_case(name))
                .map(|e| ar.data(e))
        };
        let ini_name = if sixteen_bit {
            "Audio_16.ini"
        } else {
            "Audio_08.ini"
        };
        let ini = scn::Ini::parse(&String::from_utf8_lossy(file(ini_name)?));
        let section = ini.section(class)?;
        let lists = LISTS
            .iter()
            .map(|(_, name)| {
                let mut total: usize = section
                    .get(&format!("Total{name}"))
                    .and_then(|t| t.trim().parse().ok())
                    .unwrap_or(0);
                if total > 1 {
                    total = (total / divisor.max(1)).max(1);
                }
                (1..=total.min(10))
                    .filter_map(|i| section.get(&format!("{name}_{i}")))
                    .filter_map(|f| {
                        let s = mm2_formats::wav::parse(file(f)?).ok()?;
                        Some(if freshen { crate::freshen::engine(&s, f, class) } else { s })
                    })
                    .map(Arc::new)
                    .collect()
            })
            .collect::<Vec<Vec<_>>>();
        // The demo's lists name every class's samples, but it has only the
        // 250's, and only in 8-bit: a class with none is not there.
        if lists.iter().all(Vec::is_empty) {
            return None;
        }
        let band = |key: &str, default: i32| {
            section
                .get(key)
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(default)
        };
        Some(Bank {
            lists,
            bands: [
                band("LowTorqueMaxSpeed", 20),
                band("MidTorqueMaxSpeed", 55),
                band("HighTorqueMaxSpeed", 100),
            ],
            loops: None,
            blend: 0.0,
        })
    }

    /// A class from the free pack's folder: each list's WAVs, named after
    /// the list (`LowTorque01.wav`), in name order.
    fn load_dir(dir: &Path, divisor: usize) -> Option<Bank> {
        let mut names: Vec<(String, std::path::PathBuf)> = crate::fs::read_dir(dir)
            .ok()?
            .filter_map(|e| e.ok())
            .map(|e| (e.file_name().to_string_lossy().to_ascii_lowercase(), e.path()))
            .filter(|(n, _)| mm2_formats::wav::is_sound_name(n))
            .collect();
        names.sort();
        // A sound as both `.ogg` and `.wav` counts once (the Ogg Vorbis).
        let oggs: HashSet<String> = names.iter().filter_map(|(n, _)| n.strip_suffix(".ogg").map(str::to_string)).collect();
        names.retain(|(n, _)| n.strip_suffix(".wav").is_none_or(|stem| !oggs.contains(stem)));
        let lists: Vec<Vec<Arc<Sound>>> = LISTS
            .iter()
            .map(|(_, list)| {
                let prefix = list.to_ascii_lowercase();
                let files: Vec<_> = names
                    .iter()
                    .filter(|(n, _)| {
                        n.strip_prefix(&prefix)
                            .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit() || c == '.'))
                    })
                    .collect();
                let keep = if files.len() > 1 { (files.len() / divisor.max(1)).max(1) } else { files.len() };
                files
                    .into_iter()
                    .take(keep)
                    .filter_map(|(_, p)| crate::fs::read(p).ok())
                    .filter_map(|b| mm2_formats::wav::decode(&b).map_err(|e| eprintln!("{}: {e}", dir.display())).ok())
                    .map(Arc::new)
                    .collect()
            })
            .collect();
        // Whole recorded samples, when the class has them, play as the
        // original's do. The steady loops are only for a class without
        // them: one loop pitched across the whole rev range sounds like a
        // siren close up, on your own bike, though a bike going by hides it.
        let whole = [List::Idle, List::PowerBand, List::Decel]
            .iter()
            .all(|&l| !lists[LISTS.iter().position(|(k, _)| *k == l).unwrap_or(0)].is_empty());
        // Unless the class's `engine.ini` asks for its rev-step loops
        // (`RevBlend = 1`, with `OnLoops`/`OffLoops` recorded every so many
        // revs): then the loops play, blended by revs, on the gas, off it
        // and in the air, and the whole samples are only for an older game.
        let loops = if whole && !Loops::wanted(dir) { None } else { Loops::load(dir) };
        if lists.iter().all(|l| l.is_empty()) && loops.is_none() {
            return None;
        }
        let ini = crate::fs::read_to_string(dir.join("bands.ini")).unwrap_or_default();
        let ini = scn::Ini::parse(&ini);
        let band = |key: &str, default: i32| {
            ini.sections
                .iter()
                .find_map(|s| s.get(key))
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(default)
        };
        Some(Bank {
            lists,
            bands: [
                band("LowTorqueMaxSpeed", 20),
                band("MidTorqueMaxSpeed", 55),
                band("HighTorqueMaxSpeed", 100),
            ],
            loops,
            blend: BLEND,
        })
    }

    fn list(&self, l: List) -> &[Arc<Sound>] {
        let i = LISTS.iter().position(|(k, _)| *k == l).unwrap_or(0);
        &self.lists[i]
    }
}

/// What an engine is playing (the game's states 1–6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum State {
    #[default]
    Idle,
    /// A low, mid or high torque sample.
    Band,
    Power,
    Sputter,
    Decel,
    Rev,
}

impl State {
    /// What follows when the sample ends; `None` loops it.
    fn next(self) -> Option<State> {
        match self {
            State::Idle => None,
            State::Band | State::Sputter => Some(State::Power),
            State::Power => Some(State::Sputter),
            State::Decel | State::Rev => Some(State::Idle),
        }
    }

    /// The list a state plays from (a band's own list is picked by speed).
    fn list(self) -> List {
        match self {
            State::Idle => List::Idle,
            State::Band => List::HighTorque,
            State::Power => List::PowerBand,
            State::Sputter => List::Sputter,
            State::Decel => List::Decel,
            State::Rev => List::Rev,
        }
    }
}

/// Whether the sample queued after `state`'s loops: only the idle does.
fn t_loops(state: State) -> bool {
    state.next() == Some(State::Idle)
}

fn seconds(s: &Sound) -> f32 {
    s.frames() as f32 / s.rate.max(1) as f32
}

/// One bike's engine and crash sounds (a 0x54-byte record at +0x98).
#[derive(Default)]
struct Engine {
    /// The engine class, an index into `CLASSES`.
    class: usize,
    state: State,
    /// The sample playing, and how far into it. Without a voice the
    /// stream stands still (0x4e9aa0 runs only on a buffer).
    sound: Option<Arc<Sound>>,
    clock: f32,
    /// The sample queued to follow it.
    then: Option<Arc<Sound>>,
    voice: Option<Voice>,
    air: f32,
    airborne: bool,
    /// The throttle was let go on the ground, or in the air.
    released: bool,
    released_in_air: bool,
    racing: bool,
    crashed: bool,
    /// The wreck and fall last played for this bike, 1 to 6 (0: none, and
    /// so is the first: the game keeps the index and tests it for 0).
    wreck: usize,
    fall: usize,
    /// The free pack's loops, when its class has them.
    revs: Revs,
}

/// A one-shot 3D sound still playing, where it was started.
struct Placed {
    voice: Voice,
    at: Vec3,
    range: (f32, f32),
}

/// What a scene sound hangs on, found each frame by the caller.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Site {
    /// A fixed place: a static model or a `[StaticSoundN]`.
    Fixed(Vec3),
    /// An animated object and one of its frames (its root when none).
    Object(usize, Option<usize>),
    /// A scripted vehicle, by index into the track's traffic.
    Car(usize),
}

/// A scene's sound emitter (SoundEmitter, 0x402080–0x402a10): it plays
/// while the camera is within 1.2 times its maximum distance, side to side
/// and front to back (0x4024d0), or once, the first time the camera comes
/// within its one-shot distance. Each time it comes in range it rolls its
/// trigger chance once (0x4027b0). It is a 3D buffer with the scene's
/// minimum and maximum distances, moving with what it hangs on (0x402870),
/// unless it is `SoundForce2D` (most are): then a flat buffer whose level
/// follows the distance ([`flat_volume`]).
struct Emitter {
    spec: scn::SoundSpec,
    site: Site,
    sound: Option<Arc<Sound>>,
    voice: Option<Voice>,
    /// Where it was last frame, for its velocity.
    last: Option<Vec3>,
    /// It has rolled since it came in range (+0x1a0 bit 4), and a one-shot
    /// has played (bit 0).
    rolled: bool,
    fired: bool,
}

/// How far out a scene sound keeps playing, in maximum distances
/// (0x5555e8).
const EMITTER_REACH: f32 = 1.2;

/// A `SoundForce2D` emitter's volume (0x402010, set each frame by 0x403100
/// for the emitters in a plain 2D buffer): no 3D, no pan and no Doppler,
/// only a level from its side-to-side and front-to-back distance: full
/// inside its minimum distance, falling in a straight line in decibels to
/// −25 dB at its maximum, and held at −25 dB out to where it stops.
fn flat_volume(distance: f32, (min, max): (f32, f32)) -> f32 {
    let hundredths = if distance >= max {
        -2500.0
    } else if distance <= min {
        0.0
    } else {
        ((distance - min) / (max - min) * -2500.0).trunc()
    };
    10f32.powf(hundredths / 2000.0)
}

pub struct RaceSounds {
    /// By class; only the classes in the race are loaded.
    banks: [Option<Bank>; 5],
    effects: HashMap<String, Arc<Sound>>,
    engines: Vec<Engine>,
    /// The class of any racer not given one: the player's.
    default_class: usize,
    options: Options,
    event: EventKind,
    /// The scene has a crowd (`[Sounds] CrowdPresent`): its loop and its
    /// reactions are loaded.
    crowd_present: bool,
    crowd: Option<Voice>,
    /// The crowd loop's level, hundredths of a dB (Ctrl+C: 0, −500,
    /// −10000).
    crowd_level: f32,
    /// The hard landing is one buffer, placed on the followed bike.
    landing: Option<Voice>,
    /// The six wrecks and six falls are one buffer each, shared by every
    /// bike: one still playing is not started again.
    wrecks: [Option<Voice>; 6],
    falls: [Option<Voice>; 6],
    placed: Vec<Placed>,
    seed: u32,
    /// The player's gates and laps passed, and place, for the chime and
    /// the crowd (+0x1294, +0x1290, +0x129c).
    gates: Option<(u32, u32)>,
    place: Option<usize>,
    /// Seconds since the crowd last said "oh", applauded a pass, and
    /// applauded a stunt (+0x12a4, +0x12a8, +0x12ac).
    since_oh: f32,
    since_pass: f32,
    since_stunt: f32,
    /// The last stunt score seen, for the bonus.
    popup: Option<(f32, f32, f32)>,
    /// When Moto-Tag's IT last changed hands, for its call.
    tagged: Option<f32>,
    /// The scene's ambient sounds.
    emitters: Vec<Emitter>,
    /// Till another splash may sound, s.
    splash_wait: f32,
    /// Only the engines sound: no landings, crashes, chimes or crowd (for
    /// recording the engine note on its own, as the game's engine buffer
    /// is captured to compare with).
    pub engines_only: bool,
    /// A racer whose engine is not heard: the player's, while they drive
    /// something else (crate::drive).
    pub hush: Option<usize>,
    /// The out-of-bounds whistle (Extras, on unless turned off): a free
    /// track's own, none of the game's. Set by the race once loaded.
    pub whistle: bool,
    /// The whistle playing, and whether it is on the way down yet.
    whistling: Option<(Voice, bool)>,
}

impl RaceSounds {
    /// The sounds from the game's `res` folder for a race of `classes`
    /// (each racer's engine class, the player's at `player`), the event,
    /// and whether the scene has a crowd.
    pub fn load(
        res: &Path,
        audio: &Audio,
        classes: &[usize],
        player: usize,
        event: EventKind,
        crowd_present: bool,
        options: Options,
    ) -> RaceSounds {
        // With 16-bit sound and more than one class in the race, the other
        // classes keep half their samples (a quarter with three, when the
        // player's keeps half), the game's way of saving memory (0x4e7160
        // with its fifth argument 6).
        let present: Vec<usize> = (0..CLASSES.len()).filter(|c| classes.contains(c)).collect();
        let mine = classes.get(player).copied().unwrap_or(1);
        let divisor = |c: usize| match (options.sixteen_bit, present.len()) {
            (true, 2) if c != mine => 2,
            (true, 3..) if c != mine => 4,
            (true, 3..) => 2,
            _ => 1,
        };
        // With the Free Pack's sounds (the free graphics' unless the MM2
        // sounds are picked), the pack's own recordings for each class.
        // With the MM2 sounds (the full game's, or the demo's; the MM2
        // graphics' unless the Free Pack's are picked), the original's
        // samples come first, on the free tracks and the free bikes too;
        // the pack's only stand in when the files have no engine at all.
        let pack_first = !crate::freepack::mm2_sounds();
        let freshen = crate::freshen::on();
        let engine_res = res;
        let mut banks: [Option<Bank>; 5] = Default::default();
        for &c in &present {
            // The pack's recordings by the original's section too: its big
            // two-strokes ride on the 250s' sounds and its big four-strokes
            // on the 400s', as in the game.
            let pack = || crate::freepack::engine_dir(CLASSES[ORIGINAL[c]]).and_then(|dir| Bank::load_dir(&dir, divisor(c)));
            let section = |class: &str| {
                Bank::load(engine_res, options.sixteen_bit, class, divisor(c), freshen)
                    .or_else(|| options.sixteen_bit.then(|| Bank::load(engine_res, false, class, divisor(c), freshen)).flatten())
            };
            let original = || section(CLASSES[ORIGINAL[c]]);
            // With the MM2 sounds, a class the files lack rides another
            // class's samples before the pack's: the demo has only the
            // 250's, so every bike sounds like the demo, the Free Pack's
            // bikes too.
            let borrowed = || {
                [1, 2, 0].into_iter().filter(|&k| k != ORIGINAL[c]).find_map(|k| section(CLASSES[k]))
            };
            banks[c] = if pack_first {
                pack().or_else(original)
            } else {
                original().or_else(borrowed).or_else(pack)
            };
            if banks[c].is_none() {
                eprintln!("no [{}] engine sounds", CLASSES[c]);
            }
        }
        // The race effects the same way: the pack's first only with the
        // free graphics; either way, the other fills in what one lacks.
        let mut pack_fx = HashMap::new();
        if let Some(dir) = crate::freepack::fx_dir() {
            for e in crate::fs::read_dir(&dir).into_iter().flatten().flatten() {
                let name = e.file_name().to_string_lossy().to_ascii_lowercase();
                // An Ogg Vorbis `crowdloop.ogg` answers for `crowdloop.wav`,
                // the name the game asks for, and wins over a WAV of it.
                let (name, ogg) = match name.strip_suffix(".ogg") {
                    Some(stem) => (format!("{stem}.wav"), true),
                    None if name.ends_with(".wav") => (name, false),
                    None => continue,
                };
                if !ogg && pack_fx.contains_key(&name) {
                    continue;
                }
                if let Some(s) = crate::fs::read(e.path()).ok().and_then(|b| mm2_formats::wav::decode(&b).ok()) {
                    pack_fx.insert(name, Arc::new(s));
                }
            }
        }
        let mut original_fx = HashMap::new();
        if let Ok(bytes) = crate::read_archive(&res.join("AUDIO.RES")) {
            if let Ok(ar) = rs2::Archive::parse(&bytes) {
                for e in ar.entries() {
                    if let Ok(s) = mm2_formats::wav::parse(ar.data(e)) {
                        original_fx.insert(e.name.to_ascii_lowercase(), Arc::new(s));
                    }
                }
            }
        }
        let (mut effects, rest) = if pack_first { (pack_fx, original_fx) } else { (original_fx, pack_fx) };
        for (name, s) in rest {
            effects.entry(name).or_insert(s);
        }
        // The rivers' own sounds (OpenMCM2's, none of the game's).
        effects.insert(RIVER.into(), Arc::new(river_sound()));
        effects.insert(SPLASH.into(), Arc::new(splash_sound()));
        let engines = classes
            .iter()
            .map(|&class| Engine {
                class,
                ..Default::default()
            })
            .collect();
        let mut sounds = RaceSounds {
            banks,
            effects,
            engines,
            default_class: mine,
            options,
            event,
            crowd_present,
            crowd: None,
            crowd_level: 0.0,
            landing: None,
            wrecks: [None; 6],
            falls: [None; 6],
            placed: Vec::new(),
            seed: 0x2545_f491,
            gates: None,
            place: None,
            since_oh: 0.0,
            since_pass: 0.0,
            since_stunt: 0.0,
            popup: None,
            tagged: None,
            emitters: Vec::new(),
            splash_wait: 0.0,
            engines_only: false,
            hush: None,
            whistle: false,
            whistling: None,
        };
        // The crowd loops from the start, flat (0x4e81a0).
        if crowd_present {
            sounds.crowd = sounds.play(audio, "crowdloop.wav", true);
        }
        sounds
    }

    /// The game's `rand()`, near enough: the picks only need to vary.
    fn random(&mut self, n: usize) -> usize {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        self.seed as usize % n.max(1)
    }

    /// A random sample from one of a class's engine lists.
    fn pick(&mut self, class: usize, list: List) -> Option<Arc<Sound>> {
        let n = self.banks[class].as_ref().map_or(0, |b| b.list(list).len());
        let k = self.random(n);
        self.banks[class]
            .as_ref()
            .and_then(|b| b.list(list).get(k).cloned())
    }

    /// The sound effects' volume now (0 with the sounds off).
    pub fn gain(&self) -> f32 {
        self.options.gain
    }

    /// Ctrl+S: the race's sounds off or on again (0x4e9250); off, every
    /// voice falls silent.
    pub fn set_on(&mut self, audio: &Audio, on: bool, gain: f32) {
        self.options.gain = if on { gain } else { 0.0 };
        if !on {
            audio.stop_all();
            self.crowd = None;
            self.landing = None;
        } else if self.crowd_present && self.crowd.is_none() {
            self.crowd = self.play(audio, "crowdloop.wav", true);
        }
        self.place_crowd(audio);
    }

    /// Ctrl+C in a stadium: the crowd on, reduced (−5 dB) and off, round
    /// again (0x4e9250); the string the game says.
    pub fn step_crowd(&mut self, audio: &Audio) -> Option<u32> {
        if !self.crowd_present {
            return None;
        }
        let (level, said) = match self.crowd_level {
            l if l >= 0.0 => (-500.0, 5312),
            l if l > -10000.0 => (-10000.0, 5314),
            _ => (0.0, 5313),
        };
        self.crowd_level = level;
        self.place_crowd(audio);
        Some(said)
    }

    fn place_crowd(&self, audio: &Audio) {
        if let Some(v) = self.crowd {
            let level = if self.crowd_level <= -10000.0 { 0.0 } else { 10f32.powf(self.crowd_level / 2000.0) };
            audio.place(v, self.options.gain * level, 0.0, 1.0);
        }
    }

    /// Plays one of the race's flat (2D) sounds at full volume.
    fn play(&self, audio: &Audio, name: &str, looping: bool) -> Option<Voice> {
        self.effects
            .get(name)
            .filter(|_| self.options.gain > 0.0 && !self.engines_only)
            .map(|s| audio.play(s, self.options.gain, looping))
    }

    /// Cuts off the out-of-bounds whistle, if it is still going.
    fn stop_whistle(&mut self, audio: &Audio) {
        if let Some((v, _)) = self.whistling.take() {
            audio.stop(v);
        }
    }

    /// One of the crowd's reactions, which only a scene with a crowd has.
    fn cheer(&self, audio: &Audio, name: &str) {
        if self.crowd_present {
            self.play(audio, name, false);
        }
    }

    /// Starts a 3D one-shot at `at`, as heard from `l`.
    fn play_at(
        &mut self,
        audio: &Audio,
        name: &str,
        at: Vec3,
        range: (f32, f32),
        l: &Listener,
    ) -> Option<Voice> {
        let s = self
            .effects
            .get(name)
            .filter(|_| self.options.gain > 0.0 && !self.engines_only)?;
        let (volume, pan, pitch) = l.hear(at, Vec3::ZERO, range);
        let v = audio.play(s, volume * self.options.gain, false);
        audio.place(v, volume * self.options.gain, pan, pitch);
        self.placed.push(Placed {
            voice: v,
            at,
            range,
        });
        Some(v)
    }

    /// A wreck or fall for a bike at `at` (0x4e9050, 0x4e9110): one of the
    /// six not already playing, ten tries at random, or none.
    fn crash_sound(&mut self, audio: &Audio, fall: bool, at: Vec3, l: &Listener) -> usize {
        for _ in 0..10 {
            let k = self.random(6);
            let slot = if fall { self.falls[k] } else { self.wrecks[k] };
            if slot.is_some_and(|v| audio.playing(v)) {
                continue;
            }
            let name = if fall {
                format!("fall{:02}.wav", k + 1)
            } else {
                format!("wreck{:02}.wav", k + 1)
            };
            let v = self.play_at(audio, &name, at, ENGINE_RANGE, l);
            if fall {
                self.falls[k] = v;
            } else {
                self.wrecks[k] = v;
            }
            return k;
        }
        0
    }

    /// Follows the race as heard from `l` (0x4e84a0, 0x4e8720).
    pub fn update(&mut self, audio: &Audio, race: &Race, l: &Listener, dt: f32) {
        let class = self.default_class;
        self.engines.resize_with(race.racers.len(), || Engine {
            class,
            ..Default::default()
        });
        self.since_oh += dt;
        self.since_pass += dt;
        self.since_stunt += dt;
        let player = race.player;

        // The four nearest the camera get a voice, however far (0x4e8720).
        // An empty place in free roam has nobody on it, so no engine.
        let mut near: Vec<(usize, f32)> = race
            .racers
            .iter()
            .enumerate()
            .filter(|&(i, _)| !race.away(i))
            .map(|(i, r)| {
                let to = r.bike.position() - l.eye;
                let far = to.x.abs() > FAR || to.z.abs() > FAR;
                (
                    i,
                    if far {
                        FAR
                    } else {
                        Vec3::new(to.x, 0.0, to.z).length()
                    },
                )
            })
            .collect();
        near.sort_by(|a, b| a.1.total_cmp(&b.1));
        near.truncate(VOICES);

        for i in 0..race.racers.len() {
            let r = &race.racers[i];
            let bike = &r.bike;
            let mut e = std::mem::take(&mut self.engines[i]);
            let was_airborne = e.airborne;
            e.airborne = bike.airborne();
            let heard = near.iter().any(|n| n.0 == i) && self.hush != Some(i);

            // The engine's state machine (0x4e8720), for every bike.
            let gas = !bike.crashed() && r.controls.throttle > 0.1;
            let racing = race.gate_down();
            let mph = bike.velocity().length() * MPH;
            let mut change = None;
            if !bike.airborne() {
                e.air = 0.0;
                e.released_in_air = false;
                // Touching down, the followed bike thumps (LandHard01),
                // unless the last thump still plays.
                if i == player && was_airborne && !self.landing.is_some_and(|v| audio.playing(v)) {
                    self.landing =
                        self.play_at(audio, "landhard01.wav", bike.position(), LANDING_RANGE, l);
                }
                if !gas {
                    e.released = true;
                }
                if !racing {
                    // Revving at the gate: once per blip.
                    if gas && (e.released || e.racing) {
                        e.released = false;
                        change = Some((State::Rev, List::Rev));
                    }
                } else if !gas {
                    if matches!(e.state, State::Band | State::Power | State::Sputter) {
                        change = Some((State::Decel, List::Decel));
                    }
                } else if e.state == State::Rev {
                    change = Some((State::Band, List::HighTorque));
                } else if matches!(e.state, State::Idle | State::Decel) {
                    let bands = self.banks[e.class]
                        .as_ref()
                        .map_or([20, 55, 100], |b| b.bands);
                    let mph = mph as i32;
                    change = Some(if mph < bands[0] {
                        (State::Band, List::LowTorque)
                    } else if mph < bands[1] {
                        (State::Band, List::MidTorque)
                    } else if mph >= bands[2] {
                        (State::Power, List::PowerBand)
                    } else {
                        (State::Band, List::HighTorque)
                    });
                }
            } else {
                e.air += dt;
                if e.air >= AIR {
                    if !gas {
                        e.released_in_air = true;
                    }
                    if !matches!(e.state, State::Decel | State::Rev | State::Idle) {
                        change = Some((State::Decel, List::Decel));
                    }
                    // Blipping the throttle in the air.
                    if e.released_in_air && gas {
                        e.released_in_air = false;
                        change = Some((State::Rev, List::Rev));
                    }
                }
            }
            e.racing = racing;
            if let Some((state, list)) = change {
                e.state = state;
                e.sound = self.pick(e.class, list);
                e.clock = 0.0;
                e.then = state.next().and_then(|n| self.pick(e.class, n.list()));
                if let (Some(v), Some(s)) = (e.voice, &e.sound) {
                    audio.switch(v, s, e.then.is_none());
                    if let Some(t) = &e.then {
                        audio.then(v, t, t_loops(state));
                    }
                }
            }

            // A crashed bike: the crowd's reaction as it goes down, a
            // wreck each time the bike lands, and the rider's grunts while
            // its body touches the ground (0x4e8720's second loop).
            if bike.crashed() {
                // Thrown back off the world's edge, the player hears the
                // launch (0x4e8720, crash 0xc).
                if !e.crashed && i == player && bike.launched() {
                    self.play(audio, "launch.wav", false);
                    // On a free track, a cartoon slide whistle up as the
                    // rider is thrown back in (the Free Pack's own).
                    if self.whistle {
                        self.stop_whistle(audio);
                        self.whistling = self.play(audio, WHISTLE_UP, false).map(|v| (v, false));
                    }
                }
                // ... and down once the throw tops out and they fall.
                if i == player && bike.launched() {
                    if let Some((up, false)) = self.whistling {
                        if bike.velocity().y < 0.0 {
                            audio.stop(up);
                            self.whistling = self.play(audio, WHISTLE_DOWN, false).map(|v| (v, true));
                        }
                    }
                }
                if !e.crashed && i == player && self.event == EventKind::Supercross {
                    let place = race.position();
                    if place == 1 {
                        self.cheer(audio, "boo_01.wav");
                    } else if self.since_oh > OH_WAIT {
                        self.cheer(audio, "oh_01.wav");
                        self.since_oh = 0.0;
                    }
                }
                let free = |slot: Option<Voice>| !slot.is_some_and(|v| audio.playing(v));
                if was_airborne
                    && !bike.airborne()
                    && (e.wreck == 0 || free(self.wrecks[e.wreck - 1]))
                {
                    e.wreck = self.crash_sound(audio, false, bike.position(), l);
                }
                if bike.rider_touching() && (e.fall == 0 || free(self.falls[e.fall - 1])) {
                    e.fall = self.crash_sound(audio, true, bike.position(), l);
                }
            } else {
                e.wreck = 0;
                e.fall = 0;
            }
            e.crashed = bike.crashed();

            if let Some(loops) = self.banks[e.class].as_ref().and_then(|b| b.loops.as_ref()) {
                let gas = if bike.crashed() { 0.0 } else { r.controls.throttle.clamp(0.0, 1.0) };
                // Off the ground only once it's been off a while, as the
                // original's sounds judge it: every bump lifts the bike a
                // moment, and the revs mustn't flare each time.
                let grounded = e.air < AIR && racing;
                let flat = bike.velocity();
                let road = Vec3::new(flat.x, 0.0, flat.z).length() * MPH;
                e.revs.step(loops, road, gas, grounded, dt);
                if !heard || self.options.gain <= 0.0 {
                    for v in e.revs.voices.drain(..).flatten() {
                        audio.stop(v);
                    }
                } else {
                    let sounds = loops.all();
                    e.revs.voices.resize(sounds.len(), None);
                    let (volume, pan, doppler) = l.hear(bike.position(), bike.velocity(), ENGINE_RANGE);
                    let mix = e.revs.mix(loops);
                    // Each bike its own place in the loops and a hair of its
                    // own pitch: bikes side by side at the same revs (off a
                    // jump together) would otherwise play the very same
                    // waves in step, which sounds hollow, like a flanger.
                    let mine = |k: usize| {
                        let h = (i as u32).wrapping_mul(0x9e37_79b9) ^ (k as u32).wrapping_mul(0x85eb_ca6b);
                        let h = (h ^ (h >> 15)).wrapping_mul(0x2c1b_3c6d);
                        (h ^ (h >> 12)) as f32 / u32::MAX as f32
                    };
                    let detune = 1.0 + (mine(usize::MAX) - 0.5) * 0.03;
                    for (k, ((slot, sound), (level, pitch))) in e.revs.voices.iter_mut().zip(sounds).zip(mix).enumerate() {
                        if slot.is_some_and(|v| !audio.playing(v)) {
                            *slot = None;
                        }
                        // Only the loops being heard play.
                        if level < 0.002 {
                            if let Some(v) = slot.take() {
                                audio.stop(v);
                            }
                            continue;
                        }
                        let v = *slot.get_or_insert_with(|| {
                            let length = sound.frames() as f32 / sound.rate.max(1) as f32;
                            audio.play_from(sound, mine(k) * length, 0.0, true)
                        });
                        audio.place(v, volume * level * self.options.gain, pan, doppler * pitch * detune);
                    }
                }
                self.engines[i] = e;
                continue;
            }
            if !heard {
                if let Some(v) = e.voice.take() {
                    audio.stop(v);
                }
                self.engines[i] = e;
                continue;
            }
            // The mixer ran out of queued samples and let the voice go: the
            // game's clock fell behind the sound card's (each frame counts
            // at most 0.1 s, so a hitch, or a browser tab put in the
            // background, loses time the sound card doesn't). Start a new
            // voice where the stream stands, or the engine stays silent
            // for the rest of the race.
            if e.voice.is_some_and(|v| !audio.playing(v)) {
                e.voice = None;
            }
            if e.sound.is_none() {
                e.sound = self.pick(e.class, e.state.list());
                e.then = e.state.next().and_then(|n| self.pick(e.class, n.list()));
                e.clock = 0.0;
            }
            if e.voice.is_none() && self.options.gain > 0.0 {
                // A new voice takes up the stream where it stood.
                if let Some(s) = &e.sound {
                    let v = audio.play_from(s, e.clock, 0.0, e.then.is_none());
                    audio.set_blend(v, self.banks[e.class].as_ref().map_or(0.0, |b| b.blend));
                    if let Some(t) = &e.then {
                        audio.then(v, t, t_loops(e.state));
                    }
                    e.voice = Some(v);
                }
            }
            // Each sample runs on into the next (0x4e9aa0).
            e.clock += dt;
            // A sample fading into the next hands over that much sooner.
            let blend = self.banks[e.class].as_ref().map_or(0.0, |b| b.blend);
            while let (Some(s), Some(next)) = (&e.sound, e.state.next()) {
                let length = (seconds(s) - blend).max(seconds(s) * 0.5);
                if e.clock < length || length <= 0.0 {
                    break;
                }
                e.clock -= length;
                e.state = next;
                e.sound = e.then.take();
                e.then = next.next().and_then(|n| self.pick(e.class, n.list()));
                if let (Some(v), Some(t)) = (e.voice, &e.then) {
                    audio.then(v, t, t_loops(next));
                }
            }
            if e.state == State::Idle {
                if let Some(s) = &e.sound {
                    let length = seconds(s);
                    if length > 0.0 {
                        e.clock %= length;
                    }
                }
            }
            if let Some(v) = e.voice {
                let (volume, pan, pitch) = l.hear(bike.position(), bike.velocity(), ENGINE_RANGE);
                audio.place(v, volume * self.options.gain, pan, pitch);
            }
            self.engines[i] = e;
        }

        // The hard landing follows the bike; the wrecks and falls stay
        // where they started while the camera moves.
        if let Some(v) = self.landing {
            let bike = &race.racers[player].bike;
            let (volume, pan, pitch) = l.hear(bike.position(), Vec3::ZERO, LANDING_RANGE);
            audio.place(v, volume * self.options.gain, pan, pitch);
        }
        self.placed.retain(|p| audio.playing(p.voice));
        for p in &self.placed {
            if Some(p.voice) == self.landing {
                continue;
            }
            let (volume, pan, pitch) = l.hear(p.at, Vec3::ZERO, p.range);
            audio.place(p.voice, volume * self.options.gain, pan, pitch);
        }

        // The player's gates and laps ring the waypoint chime, when it is
        // on.
        let me = &race.racers[player];
        let gates = (me.timing.gates, me.timing.crossings);
        if let Some(was) = self.gates {
            if self.options.chime && (gates.0 > was.0 || gates.1 > was.1) {
                self.play(audio, "waypoint.wav", false);
            }
        }
        self.gates = Some(gates);

        // Passing a rider: applause, always for taking the lead
        // (Applause03), otherwise at most every 15 s (Applause01; the
        // game's pick of Applause02 needs rand() to give 0).
        let place = race.position();
        if let Some(was) = self.place {
            if place < was {
                if place == 1 {
                    self.cheer(audio, "applause03.wav");
                } else if self.since_pass > APPLAUSE_WAIT {
                    self.cheer(audio, "applause01.wav");
                    self.since_pass = 0.0;
                }
            }
        }
        self.place = Some(place);

        // A stunt landed (0x4ea560): the bonus for 10,000 points in the
        // Stunt Quarry or Moto-Tag, and in a stadium the crowd applauds,
        // at most every 15 s.
        if race.popup != self.popup {
            if let Some((_, base, m)) = race.popup {
                if base * m >= BONUS_POINTS
                    && matches!(self.event, EventKind::Stunt | EventKind::Tag)
                {
                    self.play(audio, "bonus.wav", false);
                }
                if self.event == EventKind::Supercross && self.since_stunt > APPLAUSE_WAIT {
                    self.cheer(audio, "applause01.wav");
                    self.since_stunt = 0.0;
                }
            }
            self.popup = race.popup;
        }

        // Moto-Tag calls out each new IT (`It.wav`, 0x4ea5f0).
        let tagged = race.tag.as_ref().and_then(|t| t.passed);
        if tagged != self.tagged && tagged.is_some() {
            self.play(audio, "it.wav", false);
        }
        self.tagged = tagged;
    }

    /// Gives the race the scene's sounds and what each hangs on.
    pub fn set_scene(&mut self, sounds: impl IntoIterator<Item = (scn::SoundSpec, Site)>) {
        self.emitters = sounds
            .into_iter()
            .map(|(spec, site)| Emitter {
                sound: self.effects.get(&spec.file.to_ascii_lowercase()).cloned(),
                spec,
                site,
                voice: None,
                last: None,
                rolled: false,
                fired: false,
            })
            .collect();
    }

    /// Plays the scene's sounds as heard from `l`; `at` says where a site
    /// is now (0x403100 each frame).
    pub fn ambient(
        &mut self,
        audio: &Audio,
        l: &Listener,
        at: impl Fn(Site) -> Option<Vec3>,
        dt: f32,
    ) {
        for k in 0..self.emitters.len() {
            let Some(here) = at(self.emitters[k].site) else {
                continue;
            };
            let e = &mut self.emitters[k];
            let velocity = match e.last {
                Some(last) if dt > 0.0 => (here - last) / dt,
                _ => Vec3::ZERO,
            };
            e.last = Some(here);
            let to = here - l.eye;
            let (dx, dz) = (to.x.abs(), to.z.abs());
            let distance = (dx * dx + dz * dz).sqrt();
            let active = if e.spec.one_shot {
                !e.fired && distance <= e.spec.one_shot_distance
            } else {
                distance < e.spec.max_distance * EMITTER_REACH
            };
            if !active {
                // Out of range: stopped, and it rolls again when back.
                if let Some(v) = e.voice.take() {
                    audio.stop(v);
                }
                e.rolled = false;
                continue;
            }
            if !e.rolled {
                e.rolled = true;
                let roll = self.random(32768) as f32 / 32768.0 * 100.0;
                let e = &mut self.emitters[k];
                if roll <= e.spec.trigger_percent && self.options.gain > 0.0 {
                    if e.spec.one_shot {
                        e.fired = true;
                    }
                    if let Some(s) = &e.sound {
                        e.voice = Some(audio.play(s, 0.0, e.spec.looping));
                    }
                }
            }
            let e = &mut self.emitters[k];
            if let Some(v) = e.voice {
                let range = (e.spec.min_distance, e.spec.max_distance);
                let (volume, pan, pitch) = if e.spec.force_2d {
                    (flat_volume(distance, range), 0.0, 1.0)
                } else {
                    l.hear(here, velocity, range)
                };
                audio.place(v, volume * self.options.gain, pan, pitch);
            }
        }
    }

    /// A bike splashing through a river at `at`, `strength` 0 to 1.
    pub fn splash(&mut self, audio: &Audio, l: &Listener, at: Vec3, strength: f32, dt: f32) {
        self.splash_wait -= dt;
        if self.splash_wait > 0.0 {
            return;
        }
        self.splash_wait = 0.28 - 0.12 * strength.clamp(0.0, 1.0);
        if let Some(v) = self.play_at(audio, SPLASH, at, (30.0, 260.0), l) {
            let (volume, pan, pitch) = l.hear(at, Vec3::ZERO, (30.0, 260.0));
            let k = 0.45 + 0.55 * strength.clamp(0.0, 1.0);
            audio.place(v, volume * k * self.options.gain, pan, pitch * (0.9 + 0.2 * (self.random(100) as f32 / 100.0)));
        }
    }

    /// The podium: the engines stop and the crowd cheers (0x45e490's
    /// `CrowdLoop.wav`).
    pub fn podium(&mut self, audio: &Audio) {
        for e in &mut self.engines {
            if let Some(v) = e.voice.take() {
                audio.stop(v);
            }
        }
        if self.crowd.is_none() {
            self.crowd = self
                .effects
                .get("crowdloop.wav")
                .map(|s| audio.play(s, self.options.gain, true));
        }
    }

    /// Silences everything (the race is over).
    pub fn stop(&mut self, audio: &Audio) {
        for e in &mut self.engines {
            if let Some(v) = e.voice.take() {
                audio.stop(v);
            }
            for v in e.revs.voices.drain(..).flatten() {
                audio.stop(v);
            }
        }
        if let Some(v) = self.crowd.take() {
            audio.stop(v);
        }
        for p in self.placed.drain(..) {
            audio.stop(p.voice);
        }
        for e in &mut self.emitters {
            if let Some(v) = e.voice.take() {
                audio.stop(v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat scene sound: full inside its minimum distance, −12.5 dB half
    /// way to its maximum, −25 dB from there out.
    #[test]
    fn flat_volume_ramps_in_decibels() {
        let r = (10.0, 350.0);
        assert_eq!(flat_volume(5.0, r), 1.0);
        assert!((flat_volume(180.0, r) - 10f32.powf(-1250.0 / 2000.0)).abs() < 1e-4);
        assert!((flat_volume(350.0, r) - 10f32.powf(-1.25)).abs() < 1e-6);
        assert_eq!(flat_volume(400.0, r), flat_volume(350.0, r));
    }

    /// With the game's files beside the engine, every list has samples.
    #[test]
    fn engine_lists_load() {
        let res = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../GAME/RES");
        if !crate::fs::is_file(res.join("ENGINE.RES")) {
            return;
        }
        for sixteen in [true, false] {
            let bank = Bank::load(&res, sixteen, "250", 1, false).expect("the [250] engine");
            for (list, name) in LISTS {
                let samples = bank.list(list);
                assert!(!samples.is_empty(), "{name}");
                assert!(samples.iter().all(|s| s.frames() > 0), "{name}");
            }
            // The 250s change band later than the 125s (55 mph).
            assert_eq!(bank.bands, [20, 65, 100]);
        }
        // Thinned to half, a list of 8 keeps 4 and a list of 1 keeps 1.
        let half = Bank::load(&res, true, "250", 2, false).unwrap();
        assert_eq!(half.list(List::LowTorque).len(), 4);
        assert_eq!(half.list(List::Idle).len(), 1);
    }

    /// A free class with whole recorded samples plays them, even beside
    /// an `engine.ini` of steady loops; the loops are only for a class
    /// without them.
    #[test]
    fn whole_samples_win_over_loops() {
        fn wav() -> Vec<u8> {
            let data = vec![0u8; 400];
            let mut b = Vec::new();
            b.extend(b"RIFF");
            b.extend((36 + data.len() as u32).to_le_bytes());
            b.extend(b"WAVEfmt ");
            b.extend(16u32.to_le_bytes());
            b.extend(1u16.to_le_bytes());
            b.extend(1u16.to_le_bytes());
            b.extend(22050u32.to_le_bytes());
            b.extend(44100u32.to_le_bytes());
            b.extend(2u16.to_le_bytes());
            b.extend(16u16.to_le_bytes());
            b.extend(b"data");
            b.extend((data.len() as u32).to_le_bytes());
            b.extend(data);
            b
        }
        let dir = std::env::temp_dir().join(format!("mm2-whole-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ini = "[Engine]\nOnLoop=OnLoop.wav\nOffLoop=OffLoop.wav\nIdleLoop=IdleLoop.wav\nOnLoopRpm=5000\nOffLoopRpm=5000\nIdleLoopRpm=1500\n";
        std::fs::write(dir.join("engine.ini"), ini).unwrap();
        for name in ["OnLoop", "OffLoop", "IdleLoop"] {
            std::fs::write(dir.join(format!("{name}.wav")), wav()).unwrap();
        }
        // Loops alone: played by revs.
        assert!(Bank::load_dir(&dir, 1).unwrap().loops.is_some());
        for name in ["Idle01", "LowTorque01", "PowerBand01", "Decel01", "Sputter01"] {
            std::fs::write(dir.join(format!("{name}.wav")), wav()).unwrap();
        }
        let bank = Bank::load_dir(&dir, 1).unwrap();
        assert!(bank.loops.is_none());
        assert_eq!(bank.list(List::PowerBand).len(), 1);
        // Asked for, the rev-step loops win, blended by revs.
        let ini = "[Engine]\nRevBlend=1\nOnLoops=OnLoop.wav@3000, PowerBand01.wav@9000\nOffLoop=OffLoop.wav\nIdleLoop=IdleLoop.wav\nOffLoopRpm=5000\nIdleLoopRpm=1500\n";
        std::fs::write(dir.join("engine.ini"), ini).unwrap();
        let bank = Bank::load_dir(&dir, 1).unwrap();
        assert_eq!(bank.loops.as_ref().map(|l| l.on.len()), Some(2));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An MX Bikes `engine.scl` reads as its two layers, each loop with
    /// its file, revs and volume points in rpm.
    #[test]
    fn scl_reads_layers() {
        let text = "MaxValue = 10000.0\r\nAutoPitch = 1\r\nLayer0\r\n\tNumSamples = 1\r\n\tSample0\r\n\t\tWav = ..\\Sound\\a_on.wav\r\n\t\tPoint0\r\n\t\t\tValue = 0.0 ;; 0\r\n\t\t\tVolume = 1.0\r\n\t\tPoint1\r\n\t\t\tValue = 0.5 ;; 5000\r\n\t\t\tVolume = 0.5\r\n\t\tRefPitch = 0.4 ;; 4000\r\nLayer1\r\n\tSample0\r\n\t\tWav = b_off.wav\r\n\t\tPoint0\r\n\t\t\tValue = 0.2\r\n\t\t\tVolume = 0.7\r\n\t\tRefPitch = 0.3\r\n";
        let layers = Scl::parse(text);
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0][0].wav, "a_on.wav");
        assert_eq!(layers[0][0].points, vec![(0.0, 1.0), (5000.0, 0.5)]);
        assert!((layers[0][0].reference - 4000.0).abs() < 0.01);
        assert_eq!(layers[1][0].wav, "b_off.wav");
        // Straight lines between points; silence past a closing zero.
        assert!((trapezoid(&[(0.0, 1.0), (100.0, 1.0), (200.0, 0.0)], 150.0) - 0.5).abs() < 1e-6);
        assert_eq!(trapezoid(&[(0.0, 1.0), (100.0, 1.0), (200.0, 0.0)], 300.0), 0.0);
        assert_eq!(trapezoid(&[(100.0, 0.0), (200.0, 1.0), (300.0, 1.0)], 400.0), 1.0);
        assert_eq!(trapezoid(&[(100.0, 0.0), (200.0, 1.0)], 50.0), 0.0);
    }

    /// The Free Pack's sounds as Ogg Vorbis load as the WAVs did: a list's
    /// samples, an `engine.ini` that still names `.wav` files, and a sound
    /// there as both counts once.
    #[test]
    fn ogg_samples_load() {
        const TONE: &[u8] = include_bytes!("../../mm2-formats/tests/data/tone.ogg");
        let dir = std::env::temp_dir().join(format!("mm2-ogg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ini = "[Engine]\nOnLoop=OnLoop.wav\nOffLoop=OffLoop.wav\nIdleLoop=IdleLoop.wav\nOnLoopRpm=5000\nOffLoopRpm=5000\nIdleLoopRpm=1500\n";
        std::fs::write(dir.join("engine.ini"), ini).unwrap();
        for name in ["OnLoop", "OffLoop", "IdleLoop"] {
            std::fs::write(dir.join(format!("{name}.ogg")), TONE).unwrap();
        }
        let loops = Bank::load_dir(&dir, 1).unwrap().loops.unwrap();
        assert_eq!(loops.idle.frames(), 5513);
        for name in ["Idle01", "PowerBand01", "PowerBand02", "Decel01"] {
            std::fs::write(dir.join(format!("{name}.ogg")), TONE).unwrap();
        }
        std::fs::write(dir.join("PowerBand01.wav"), b"not read").unwrap();
        let bank = Bank::load_dir(&dir, 1).unwrap();
        assert!(bank.loops.is_none());
        assert_eq!(bank.list(List::PowerBand).len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn classes_follow_the_engine() {
        assert_eq!(class(125, false), 0);
        assert_eq!(class(250, false), 1);
        assert_eq!(class(400, false), 1);
        assert_eq!(class(500, false), 3);
        assert_eq!(class(400, true), 2);
        assert_eq!(class(450, true), 2);
        assert_eq!(class(600, true), 4);
        // The original plays the big bikes with its [250] and [400] samples.
        assert_eq!(CLASSES[ORIGINAL[class(500, false)]], "250");
        assert_eq!(CLASSES[ORIGINAL[class(600, true)]], "400");
    }

    #[test]
    fn falls_off_and_shifts_like_directsound() {
        let l = Listener {
            eye: Vec3::ZERO,
            right: Vec3::X,
            velocity: Vec3::ZERO,
        };
        // Full inside the minimum distance, min/d out to the maximum, and
        // silent beyond it.
        assert_eq!(
            l.hear(Vec3::new(0.0, 0.0, 20.0), Vec3::ZERO, ENGINE_RANGE)
                .0,
            1.0
        );
        assert!(
            (l.hear(Vec3::new(0.0, 0.0, 100.0), Vec3::ZERO, ENGINE_RANGE)
                .0
                - 0.5)
                .abs()
                < 1e-6
        );
        assert!(
            (l.hear(Vec3::new(0.0, 0.0, 500.0), Vec3::ZERO, ENGINE_RANGE)
                .0
                - 0.1)
                .abs()
                < 1e-6
        );
        assert_eq!(
            l.hear(Vec3::new(0.0, 0.0, 900.0), Vec3::ZERO, ENGINE_RANGE)
                .0,
            0.0
        );
        // On the right, panned right.
        assert!(
            l.hear(Vec3::new(30.0, 0.0, 0.0), Vec3::ZERO, ENGINE_RANGE)
                .1
                > 0.99
        );
        // Coming at 88 ft/s (60 mph), about 8 % higher; going, lower.
        let at = Vec3::new(0.0, 0.0, 100.0);
        let coming = l.hear(at, Vec3::new(0.0, 0.0, -88.0), ENGINE_RANGE).2;
        let going = l.hear(at, Vec3::new(0.0, 0.0, 88.0), ENGINE_RANGE).2;
        assert!(
            (coming - 1.085).abs() < 0.01 && going < 1.0,
            "{coming} {going}"
        );
        // Riding along with it, no shift.
        let along = Listener {
            velocity: Vec3::new(0.0, 0.0, 88.0),
            ..l
        };
        assert!((along.hear(at, Vec3::new(0.0, 0.0, 88.0), ENGINE_RANGE).2 - 1.0).abs() < 1e-6);
    }

    #[test]
    fn game_volume_in_hundredths_of_a_decibel() {
        let mut s = crate::profile::SoundOptions::default();
        assert!((Options::from_profile(&s).gain - 1.0).abs() < 1e-6);
        s.game_volume = 60;
        // -1000 hundredths: -10 dB.
        assert!((Options::from_profile(&s).gain - 10f32.powf(-0.5)).abs() < 1e-4);
        s.game_volume = 0;
        assert_eq!(Options::from_profile(&s).gain, 0.0);
        s.game_volume = 100;
        s.effects = false;
        assert_eq!(Options::from_profile(&s).gain, 0.0);
    }
}

#[cfg(test)]
mod revs_tests {
    use super::*;

    fn loops() -> Loops {
        let quiet = Arc::new(Sound { rate: 22050, channels: 1, samples: vec![0.0; 100] });
        Loops {
            on: vec![(quiet.clone(), 4000.0), (quiet.clone(), 8000.0)],
            off: vec![(quiet.clone(), 6000.0)],
            idle: quiet,
            idle_rpm: 1600.0,
            low: 1600.0,
            high: 10000.0,
            gears: GEARS.to_vec(),
            points: None,
        }
    }

    #[test]
    fn revs_climb_with_speed_and_drop_at_each_shift() {
        let l = loops();
        let mut r = Revs::default();
        let mut last = (0, 0.0);
        let mut drops = 0;
        for i in 0..600 {
            let mph = i as f32 * 0.12;
            r.step(&l, mph, 1.0, true, 1.0 / 60.0);
            if r.gear > last.0 {
                drops += 1;
            }
            last = (r.gear, r.rpm);
        }
        assert_eq!(r.gear, 3, "{}", r.rpm);
        assert!(drops >= 3);
        assert!(r.rpm > 7000.0 && r.rpm <= 10000.0, "{}", r.rpm);
        // Off the gas at a standstill it settles to idle, on the idle loop.
        for _ in 0..600 {
            r.step(&l, 0.0, 0.0, true, 1.0 / 60.0);
        }
        assert!((r.rpm - 1600.0).abs() < 50.0, "{}", r.rpm);
        let mix = r.mix(&l);
        assert!(mix.last().unwrap().0 > 0.7 && mix[0].0 < 0.01 && mix[1].0 < 0.01);
        // Between two loops, both are heard, at equal power.
        let w = blend(&l.on, 5657.0);
        assert!((w[0] - w[1]).abs() < 0.01 && (w[0] * w[0] + w[1] * w[1] - 1.0).abs() < 0.01);
    }
}

/// The flowing water a river's banks sound with, and a bike's splash:
/// made here, so they need no files.
pub const RIVER: &str = "openmcm2-river.wav";
pub const SPLASH: &str = "openmcm2-splash.wav";

/// A small steady noise source.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
}

/// Six seconds of running water, looping: rumbling low noise with the
/// chuckle of small bubbles over it, its ends faded into each other.
fn river_sound() -> Sound {
    let rate = 22050u32;
    let n = rate as usize * 6;
    let mut noise = Noise(0x1234_5678);
    let (mut low, mut mid, mut band) = (0.0f32, 0.0f32, 0.0f32);
    let mut raw = Vec::with_capacity(n);
    let mut bubbles: Vec<(usize, f32, f32)> = Vec::new();
    for i in 0..n {
        let w = noise.next();
        low += (w - low) * 0.02;
        mid += (w - mid) * 0.25;
        band += (mid - band) * 0.12;
        let t = i as f32 / rate as f32;
        // The flow swells and settles.
        let swell = 0.75 + 0.25 * (t * 0.9).sin() * (t * 0.37 + 1.0).sin();
        let mut v = low * 2.6 * swell + (mid - band) * 0.55 * swell;
        // Now and then a bubble: a quick falling tone.
        if noise.next() > 0.9985 {
            bubbles.push((i, 500.0 + (noise.next() + 1.0) * 700.0, 0.08 + (noise.next() + 1.0) * 0.05));
        }
        bubbles.retain(|&(at, f, a)| {
            let age = (i - at) as f32 / rate as f32;
            if age > 0.06 {
                return false;
            }
            v += (std::f32::consts::TAU * f * (1.0 + age * 6.0) * age).sin() * a * (1.0 - age / 0.06);
            true
        });
        raw.push(v);
    }
    // Loop: the last second faded into the first.
    let fade = rate as usize;
    let mut samples: Vec<f32> = raw[..n - fade].to_vec();
    for k in 0..fade {
        let t = k as f32 / fade as f32;
        samples[k] = raw[k] * t + raw[n - fade + k] * (1.0 - t);
    }
    let peak = samples.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-3);
    Sound { rate, channels: 1, samples: samples.into_iter().map(|v| v / peak * 0.6).collect() }
}

/// A splash: a burst of hiss falling away, with a low slap at its start.
fn splash_sound() -> Sound {
    let rate = 22050u32;
    let n = (rate as f32 * 0.7) as usize;
    let mut noise = Noise(0x9e37_79b9);
    let (mut a, mut b) = (0.0f32, 0.0f32);
    let samples = (0..n)
        .map(|i| {
            let t = i as f32 / rate as f32;
            let w = noise.next();
            a += (w - a) * 0.45;
            b += (a - b) * 0.08;
            let hiss = (a - b) * (-t * 7.0).exp() * (1.0 - (-t * 120.0).exp());
            let slap = (std::f32::consts::TAU * 90.0 * t).sin() * (-t * 30.0).exp() * 0.5;
            (hiss * 1.4 + slap) * 0.8
        })
        .collect();
    Sound { rate, channels: 1, samples }
}
