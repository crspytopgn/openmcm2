//! What the vehicle the player drives sounds like ([`crate::drive`]): an
//! engine loop for its kind (a small petrol car, a V8, a diesel van, a big
//! truck, a tractor, a propeller, a helicopter's rotor, a UFO's hum),
//! pitched up with its revs and louder on the gas, and its tyres
//! squealing when a car slides. The loops are cut from public-domain
//! recordings (`tools/make_vehicle_sounds.py`).

use crate::audio::{Audio, Voice};
use crate::drive::{Drive, Kind};
use crate::sound::Listener;
use mm2_formats::wav::Sound;
use std::sync::Arc;

/// The loops, by name: built into the desktop game, but in a browser
/// read from the page's store (`web/index.html` fetches them while the
/// menus are up, so the browser build doesn't carry their 1.3 MB).
const NAMES: [&str; 9] = ["car", "v8", "diesel", "truck", "tractor", "heli", "prop", "ufo", "skid"];

#[cfg(not(target_arch = "wasm32"))]
fn loop_bytes(name: &str) -> Option<std::borrow::Cow<'static, [u8]>> {
    use std::borrow::Cow;
    Some(Cow::Borrowed(match name {
        "car" => include_bytes!("vehicle_sounds/car.wav"),
        "v8" => include_bytes!("vehicle_sounds/v8.wav"),
        "diesel" => include_bytes!("vehicle_sounds/diesel.wav"),
        "truck" => include_bytes!("vehicle_sounds/truck.wav"),
        "tractor" => include_bytes!("vehicle_sounds/tractor.wav"),
        "heli" => include_bytes!("vehicle_sounds/heli.wav"),
        "prop" => include_bytes!("vehicle_sounds/prop.wav"),
        "ufo" => include_bytes!("vehicle_sounds/ufo.wav"),
        "skid" => include_bytes!("vehicle_sounds/skid.wav"),
        _ => return None,
    }))
}

#[cfg(target_arch = "wasm32")]
fn loop_bytes(name: &str) -> Option<std::borrow::Cow<'static, [u8]>> {
    crate::fs::read(format!("/assets/vehicle_sounds/{name}.wav")).ok().map(std::borrow::Cow::Owned)
}

/// The loops that are there to play: a loop the page hasn't fetched yet
/// is left out, and looked for again the next time a vehicle is driven,
/// so driving is silent only until they arrive.
fn load_loops() -> Vec<(&'static str, Arc<Sound>)> {
    NAMES
        .iter()
        .filter_map(|name| {
            let bytes = loop_bytes(name)?;
            Some((*name, Arc::new(mm2_formats::wav::parse(&bytes).ok()?)))
        })
        .collect()
}

/// Heard this far round, ft: full up to the first, nothing past the second.
const RANGE: (f32, f32) = (40.0, 1500.0);

/// Which loop a model drives with, and its pitch at rest.
pub fn voice(model: &str, kind: Kind) -> (&'static str, f32) {
    let m = model.to_ascii_lowercase();
    let is = |p: &str| m.starts_with(p);
    match kind {
        Kind::Plane if is("cargo") || is("dc") || is("beech") => ("prop", 0.8),
        Kind::Plane => ("prop", 1.0),
        Kind::Hover if is("ufo") => ("ufo", 1.0),
        Kind::Hover if is("blimp") || is("zeppelin") => ("prop", 0.6),
        Kind::Hover => ("heli", 1.0),
        Kind::Rail => ("truck", 0.75),
        Kind::Car if is("300zx") || is("chevelle") || is("cop") => ("v8", 1.0),
        Kind::Car if is("mx3") || is("sts") => ("car", 1.05),
        Kind::Car if is("jeep") || is("4runner") || is("famtrk") => ("diesel", 1.0),
        Kind::Car if is("airfield_tractor") => ("tractor", 1.2),
        Kind::Car if is("tractor") || is("combine") => ("tractor", 1.0),
        Kind::Car if is("dumptruck") || is("scraper") => ("truck", 0.85),
        Kind::Car if is("greyhound") || is("schoolbus") || is("fedup") || is("firetruck") || is("semi") || is("water") || is("company") => ("truck", 1.0),
        Kind::Car => ("car", 1.0),
    }
}

/// The driven vehicle's engine and tyres.
pub struct DriveSound {
    loops: Vec<(&'static str, Arc<Sound>)>,
    engine: Option<(Voice, &'static str)>,
    skid: Option<Voice>,
    /// The engine's revs, 0 (idle) to 1, smoothed.
    revs: f32,
}

impl DriveSound {
    pub fn new() -> DriveSound {
        DriveSound { loops: load_loops(), engine: None, skid: None, revs: 0.0 }
    }

    fn sound(&self, name: &str) -> Option<&Arc<Sound>> {
        self.loops.iter().find(|l| l.0 == name).map(|l| &l.1)
    }

    /// Follows `drive` (none: quiet) with the gas at `throttle`, heard
    /// from `l` at `gain`.
    pub fn update(&mut self, audio: &Audio, gain: f32, drive: Option<&Drive>, throttle: f32, l: &Listener, dt: f32) {
        let Some(d) = drive.filter(|_| gain > 0.0) else {
            self.stop(audio);
            return;
        };
        // The page fetches the loops while the menus are up: if they
        // weren't there when this was built, look again now.
        if self.loops.is_empty() {
            self.loops = load_loops();
        }
        let (name, base) = d.voice;
        if self.engine.is_some_and(|(v, n)| n != name || !audio.playing(v)) {
            if let Some((v, _)) = self.engine.take() {
                audio.stop(v);
            }
        }
        if self.engine.is_none() {
            if let Some(s) = self.sound(name).cloned() {
                self.engine = Some((audio.play(&s, 0.0, true), name));
            }
        }
        let throttle = throttle.clamp(0.0, 1.0);
        let pace = (d.speed.abs() / d.top_speed().max(1.0)).clamp(0.0, 1.2);
        // A car or truck runs up through four gears; a craft's engine
        // follows the gas.
        let want = match d.kind {
            Kind::Car => {
                let x = (pace * 4.0).min(3.999);
                let gear = x.floor();
                0.15 + 0.55 * (x - gear) * (0.6 + 0.4 * throttle) + 0.08 * gear + 0.15 * throttle
            }
            Kind::Rail => 0.2 + 0.6 * pace,
            Kind::Plane | Kind::Hover => 0.25 + 0.55 * throttle + 0.25 * pace,
        };
        self.revs += (want - self.revs) * (1.0 - (-6.0 * dt).exp());
        let (volume, pan, doppler) = l.hear(d.position, d.velocity(), RANGE);
        if let Some((v, _)) = self.engine {
            let pitch = base * (0.8 + 0.75 * self.revs);
            audio.place(v, volume * gain * (0.6 + 0.4 * throttle.max(self.revs)), pan, pitch * doppler);
        }
        // A car's tyres squeal as it slides.
        let slide = if d.kind == Kind::Car && d.on_ground() { ((d.slip() - 8.0) / 20.0).clamp(0.0, 1.0) } else { 0.0 };
        if slide > 0.02 {
            if self.skid.is_none_or(|v| !audio.playing(v)) {
                if let Some(s) = self.sound("skid").cloned() {
                    self.skid = Some(audio.play(&s, 0.0, true));
                }
            }
            if let Some(v) = self.skid {
                audio.place(v, volume * gain * slide * 0.8, pan, doppler);
            }
        } else if let Some(v) = self.skid.take() {
            audio.stop(v);
        }
    }

    pub fn stop(&mut self, audio: &Audio) {
        if let Some((v, _)) = self.engine.take() {
            audio.stop(v);
        }
        if let Some(v) = self.skid.take() {
            audio.stop(v);
        }
        self.revs = 0.0;
    }
}
