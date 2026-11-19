//! Joysticks: game pads (Xbox, PlayStation and the like) and flight
//! sticks, read the way the game's DirectInput joystick reads them
//! (`PCJoystickDevice`, 0x4c6ec0 to 0x4c8aa0).
//!
//! The game takes a `DIJOYSTATE` each frame (0x4c7ee0): six axes (X, Y,
//! Z, RX, RY, RZ) and 32 buttons. The control map (`control.ctl`) names
//! them as DirectInput numbers them: buttons from 0, axes by the codes
//! −3 X, −5 Y, −7 Z, −9 RX, −11 RY, −13 RZ. Here a pad with a known
//! layout (an Xbox or PlayStation pad, through the SDL mappings) gives
//! its buttons in the order DirectInput lists an Xbox pad's (A, B, X, Y,
//! the shoulders, Back, Start, the stick clicks); any other stick gives
//! its own buttons and axes in their HID order, as DirectInput would.
//!
//! Every pad and stick plugged in is read together; the one last touched
//! names the buttons on the controls page.
//!
//! An axis that rests at one end rather than in the middle (a trigger, a
//! pedal, or a device that only looks like a stick) would hold the bike
//! leaned or steered for good, so the first moment a pad is seen its
//! axes are taken at rest: one resting past nine tenths reads from its
//! rest instead, 0 there and 1 at its other end. DirectInput's
//! calibration does as much for the game. One resting part of the way
//! out (a trigger read half way, a motion sensor, a worn stick) reads
//! nothing until it has been seen back in the middle, so no pad leans
//! or steers the bike on its own. Only a pad that has been used counts
//! besides the one in use (a Steam Deck has its own controls and
//! Steam's copy of them, and other devices may look like pads).
//!
//! Axis values here are DirectInput's way round, −1 to 1 with Y growing
//! towards the rider (stick pulled back).

use std::collections::{HashMap, HashSet};
use std::time::Duration;
use web_time::Instant;

use gilrs::ff::{BaseEffect, BaseEffectType, Effect, EffectBuilder};
use gilrs::{Axis, Button, Gamepad, GamepadId, Gilrs, MappingSource};

/// The axes found resting at an end: each one's rest, −1 or 1, by pad
/// and native axis code; 0 for one found part of the way out, which
/// reads nothing until seen in the middle.
type Rest = HashMap<(GamepadId, u32), f32>;

/// How near the middle an axis must be seen to count as centred.
const CENTRED: f32 = 0.25;

/// How long after a pad is first seen its axes count as at rest.
const SETTLE: Duration = Duration::from_millis(1500);

/// The share of a stick's travel the pad in use must be pushed for the
/// others to be passed over (stick drift is less).
const IN_USE: f32 = 0.15;

/// A known pad's buttons in DirectInput's order for an Xbox pad, then
/// the D-pad's four (which DirectInput gave as a hat) as 10 to 13, so
/// they can be bound too.
const PAD_BUTTONS: [Button; 14] = [
    Button::South,
    Button::East,
    Button::West,
    Button::North,
    Button::LeftTrigger,
    Button::RightTrigger,
    Button::Select,
    Button::Start,
    Button::LeftThumb,
    Button::RightThumb,
    Button::DPadUp,
    Button::DPadDown,
    Button::DPadLeft,
    Button::DPadRight,
];

/// The D-pad's buttons, by DirectInput's number here.
pub const DPAD_UP: i32 = 10;
pub const DPAD_DOWN: i32 = 11;
pub const DPAD_LEFT: i32 = 12;
pub const DPAD_RIGHT: i32 = 13;

/// The six axis codes, X to RZ.
pub const AXES: [i32; 6] = [-3, -5, -7, -9, -11, -13];

/// How far a stick goes before it counts as pushed, as the game's
/// gamepad reading has it (0x4c74e0: below 16384 or above 49152 of
/// 0 to 65535) and for binding an axis on the controls page.
pub const PUSHED: f32 = 0.5;

/// The steering axis's dead centre (0x449e20 makes the axis with 0.04).
pub const STEER_DEAD: f32 = 0.04;

/// The lean axis's dead centre. The game's has none (0x449e20); this
/// keeps a stick that drifts a little from leaning the rider.
pub const LEAN_DEAD: f32 = 0.12;

/// The analog throttle: nothing below 0.2 (0x52e3e0), and the gas counts
/// as open past 0.33 (0x493270).
pub const THROTTLE_FLOOR: f32 = 0.2;
pub const THROTTLE_ON: f32 = 0.33;

/// Something the joystick did this frame, for the controls page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Press {
    /// A button went down (DirectInput's number, from 0).
    Button(i32),
    /// An axis went over half way (its code, −3 X and so on).
    Axis(i32),
}

pub struct Pads {
    gilrs: Gilrs,
    /// The pad last touched: the one that drives the bike.
    active: Option<GamepadId>,
    /// Each axis's side last frame on the active pad, to see one cross
    /// half way.
    pushed: [bool; 7],
    /// The buttons down last frame, a bit each.
    held: u32,
    /// When each pad was first seen, to take its axes at rest.
    first: HashMap<GamepadId, Instant>,
    /// The axes resting at an end, and those seen in the middle while
    /// the pad settles.
    rest: Rest,
    centred: HashSet<(GamepadId, u32)>,
    /// The pads a button or stick has been used on.
    touched: HashSet<GamepadId>,
    /// The pads whose axes at rest have been written to the log.
    logged: HashSet<GamepadId>,
    /// The pad vibrating: its heavy and light motors, each a steady
    /// effect turned up and down, and the strengths last asked.
    motors: Option<(GamepadId, [Effect; 2], [f32; 2])>,
}

impl Pads {
    pub fn new() -> Option<Pads> {
        let gilrs = Gilrs::new()
            .map_err(|err| eprintln!("no gamepads: {err}"))
            .ok()?;
        for (_, pad) in gilrs.gamepads() {
            eprintln!("joystick: {} ({})", pad.name(), kind_name(&pad));
        }
        Some(Pads {
            gilrs,
            active: None,
            pushed: [false; 7],
            held: 0,
            first: HashMap::new(),
            rest: HashMap::new(),
            centred: HashSet::new(),
            touched: HashSet::new(),
            logged: HashSet::new(),
            motors: None,
        })
    }

    /// Takes the joysticks' events; the pad a button or a stick moves on
    /// becomes the one in use. Returns the active pad's new presses.
    pub fn poll(&mut self) -> Vec<Press> {
        use gilrs::EventType::*;
        while let Some(ev) = self.gilrs.next_event() {
            match ev.event {
                ButtonPressed(..) => self.take(ev.id),
                AxisChanged(_, v, code)
                    if v.abs() > PUSHED && !self.rest.contains_key(&(ev.id, code.into_u32())) =>
                {
                    self.take(ev.id)
                }
                Connected => {
                    let pad = self.gilrs.gamepad(ev.id);
                    eprintln!("joystick: {} ({})", pad.name(), kind_name(&pad));
                }
                Disconnected => {
                    if self.active == Some(ev.id) {
                        self.active = None;
                    }
                    self.first.remove(&ev.id);
                    self.rest.retain(|&(id, _), _| id != ev.id);
                    self.centred.retain(|&(id, _)| id != ev.id);
                    self.touched.remove(&ev.id);
                }
                _ => {}
            }
        }
        self.settle();
        // Every pad and stick plugged in counts together, so a stick and
        // a separate throttle (or a wheel and its pedals) work as one.
        if self.gilrs.gamepads().next().is_none() {
            return Vec::new();
        }
        let held = (0..32).filter(|&n| self.button(n)).fold(0u32, |m, n| m | 1 << n);
        let mut pushed = [false; 7];
        for (i, &code) in AXES.iter().enumerate() {
            // Z only as a throttle lever (below): a game pad's Z is
            // its triggers, which rest half way.
            pushed[i] = i != 2 && self.axis(code).is_some_and(|v| v.abs() > PUSHED);
        }
        pushed[6] = self.gilrs.gamepads().any(|(_, p)| throttle(&p).is_some_and(|v| v > PUSHED));
        let fresh = held & !self.held;
        self.held = held;
        let mut out = Vec::new();
        for (i, &now) in pushed.iter().enumerate() {
            if now && !self.pushed[i] {
                out.push(Press::Axis(if i == 6 { -7 } else { AXES[i] }));
            }
        }
        self.pushed = pushed;
        out.extend((0..32).filter(|n| fresh & 1 << n != 0).map(Press::Button));
        let info = self.info();
        if crate::screens::keys::pad() != info {
            crate::screens::keys::set_pad(info);
        }
        out
    }

    /// What the pad does to the menus' pointer: the left stick's push
    /// (x, y, screen-down positive), and A and B held. (The D-pad jumps
    /// between the buttons instead: its presses.) A stick read as it
    /// comes moves the pointer with its hat too.
    pub fn pointer(&self) -> ([f32; 2], bool, bool) {
        let Some(pad) = self.pad() else {
            return ([0.0; 2], false, false);
        };
        let mut v = [
            axis(&pad, -3, &self.rest).unwrap_or(0.0),
            axis(&pad, -5, &self.rest).unwrap_or(0.0),
        ];
        if !known(&pad) {
            for (b, i, s) in [
                (Button::DPadLeft, 0, -1.0),
                (Button::DPadRight, 0, 1.0),
                (Button::DPadUp, 1, -1.0),
                (Button::DPadDown, 1, 1.0),
            ] {
                if pad.is_pressed(b) {
                    v[i] = s;
                }
            }
        }
        (v, button(&pad, 0), button(&pad, 1))
    }

    /// Whether DirectInput's button `n` is down on any pad or stick.
    pub fn button(&self, n: i32) -> bool {
        self.gilrs.gamepads().any(|(_, p)| button(&p, n))
    }

    /// An axis (−3 X to −13 RZ): the pad in use's, unless it sits in
    /// the middle, when the one pushed furthest over every pad and stick
    /// (a stick and a separate throttle, say, work as one).
    pub fn axis(&self, code: i32) -> Option<f32> {
        let own = self.pad().and_then(|p| axis(&p, code, &self.rest));
        if own.is_some_and(|v| v.abs() >= IN_USE) {
            return own;
        }
        self.gilrs
            .gamepads()
            .filter(|(id, _)| self.touched.contains(id))
            .filter_map(|(_, p)| axis(&p, code, &self.rest))
            .chain(own)
            .reduce(|a, b| if b.abs() > a.abs() { b } else { a })
    }

    /// Accelerate on the Z axis: a stick's throttle lever (a separate
    /// throttle counts), else a pad's Z, as `throttle_axis`.
    pub fn throttle_axis(&self, code: i32) -> Option<f32> {
        let lever = self.gilrs.gamepads().find(|(_, p)| throttle(p).is_some());
        match lever {
            Some((_, p)) => throttle_axis(&p, code, &self.rest),
            None => self.pad().and_then(|p| throttle_axis(&p, code, &self.rest)),
        }
    }

    /// A pad's trigger, the furthest pulled over every pad, 0 to 1.
    pub fn trigger(&self, b: Button) -> f32 {
        self.gilrs
            .gamepads()
            .map(|(_, p)| p.button_data(b).map_or(0.0, |d| d.value()))
            .fold(0.0, f32::max)
    }

    /// The pad in use as the controls page names it.
    pub fn info(&self) -> Option<crate::screens::keys::PadInfo> {
        use crate::screens::keys::{PadFamily, PadInfo};
        let pad = self.pad()?;
        let family = known(&pad).then(|| match pad.vendor_id() {
            Some(0x054c) => PadFamily::PlayStation,
            Some(0x057e) => PadFamily::Nintendo,
            _ => PadFamily::Xbox,
        });
        Some(PadInfo {
            family,
            name: plain_name(pad.name()),
            device: self.device(),
            rumble: pad.is_ff_supported(),
        })
    }

    /// Runs the pad in use's heavy and light motors at these strengths,
    /// 0 to 1 (the others stop).
    pub fn rumble(&mut self, strength: [f32; 2]) {
        let id = self.pad().filter(|p| p.is_ff_supported()).map(|p| p.id());
        if self.motors.as_ref().map(|m| m.0) != id {
            self.motors = id.and_then(|id| Some((id, motors(&mut self.gilrs, id)?, [0.0; 2])));
            if let Some((id, ..)) = &self.motors {
                eprintln!("vibration: on {}", self.gilrs.gamepad(*id).name());
            }
        }
        let Some((_, effects, was)) = &mut self.motors else { return };
        for ((effect, was), s) in effects.iter().zip(was.iter_mut()).zip(strength) {
            // Each change is a message to gilrs's effect thread.
            if (s - *was).abs() > 0.02 || (s == 0.0 && *was != 0.0) {
                let _ = effect.set_gain(s);
                *was = s;
            }
        }
    }

    /// Takes each pad's axes at rest in its first moments: one past nine
    /// tenths and not seen in the middle rests at that end; one part of
    /// the way out reads nothing until it comes back to the middle.
    fn settle(&mut self) {
        let now = Instant::now();
        for (id, pad) in self.gilrs.gamepads() {
            let since = *self.first.entry(id).or_insert(now);
            let settling = now.duration_since(since) <= SETTLE;
            if !settling && self.logged.insert(id) {
                // For a player's log: what the pad is and where it rests.
                let axes: Vec<String> = pad
                    .state()
                    .axes()
                    .map(|(code, data)| format!("{:#x}={:+.2}", code.into_u32() & 0xffff, data.value()))
                    .collect();
                eprintln!(
                    "joystick: {} ({:04x}:{:04x}, {:?}) axes at rest: {}",
                    pad.name(),
                    pad.vendor_id().unwrap_or(0),
                    pad.product_id().unwrap_or(0),
                    pad.mapping_source(),
                    if axes.is_empty() { "none moved".to_string() } else { axes.join(" ") },
                );
            }
            for (code, data) in pad.state().axes() {
                let key = (id, code.into_u32());
                let v = data.value();
                if !settling {
                    // A doubtful axis is trusted once seen in the middle.
                    if self.rest.get(&key) == Some(&0.0) && v.abs() < CENTRED {
                        eprintln!("joystick: {} axis {:#x} centred", pad.name(), key.1);
                        self.rest.remove(&key);
                    }
                    continue;
                }
                if v.abs() < CENTRED {
                    self.centred.insert(key);
                    self.rest.remove(&key);
                } else if !self.centred.contains(&key) && !self.rest.contains_key(&key) {
                    let r = if v.abs() > 0.9 { v.signum() } else { 0.0 };
                    eprintln!("joystick: {} axis {:#x} rests at {v:+.2}", pad.name(), key.1);
                    self.rest.insert(key, r);
                }
            }
        }
    }

    fn take(&mut self, id: GamepadId) {
        self.touched.insert(id);
        if self.active != Some(id) {
            let pad = self.gilrs.gamepad(id);
            eprintln!("joystick in use: {} ({})", pad.name(), kind_name(&pad));
            self.active = Some(id);
        }
    }

    /// The pad in use: the last one touched, else the first game pad
    /// connected, else the first stick.
    pub fn pad(&self) -> Option<Gamepad<'_>> {
        if let Some(p) = self.active.and_then(|id| self.gilrs.connected_gamepad(id)) {
            return Some(p);
        }
        let mut all = self.gilrs.gamepads().map(|(_, p)| p);
        let first = self.gilrs.gamepads().next().map(|(_, p)| p);
        all.find(|p| known(p)).or(first)
    }

    /// The device the controls page and `control.ctl` file the pad
    /// under (strings 460 to 467), as 0x44a1b0 picks it from what
    /// DirectInput says: a game pad is 3 (Standard Game Pad), any other
    /// stick 1 (2 Axis, 4+ Button Joystick). With none it is 6, the
    /// keyboard.
    pub fn device(&self) -> usize {
        match self.pad() {
            Some(p) if known(&p) => 3,
            Some(_) => 1,
            None => crate::input::KEYBOARD,
        }
    }
}

/// A pad's heavy and light motors, each running for good at no strength.
fn motors(gilrs: &mut Gilrs, id: GamepadId) -> Option<[Effect; 2]> {
    let mut motor = |kind| {
        let effect = EffectBuilder::new()
            .add_effect(BaseEffect { kind, ..Default::default() })
            .gamepads(&[id])
            .gain(0.0)
            .finish(gilrs)
            .map_err(|err| eprintln!("no vibration: {err}"))
            .ok()?;
        effect.play().ok()?;
        Some(effect)
    };
    Some([
        motor(BaseEffectType::Strong { magnitude: u16::MAX })?,
        motor(BaseEffectType::Weak { magnitude: u16::MAX })?,
    ])
}

/// A pad's name as people know it. Browsers add their own tags: Chrome
/// "Xbox Wireless Controller (STANDARD GAMEPAD Vendor: 045e Product:
/// 0b13)", Firefox "045e-0b13-Xbox Wireless Controller".
fn plain_name(name: &str) -> String {
    let mut n = name.trim();
    if let Some(at) = n.rfind(" (") {
        let tag = &n[at..];
        if tag.contains("Vendor:") || tag.contains("STANDARD GAMEPAD") || tag.contains("XInput") {
            n = n[..at].trim_end();
        }
    }
    let b = n.as_bytes();
    if b.len() > 10
        && b[4] == b'-'
        && b[9] == b'-'
        && b[..4].iter().chain(&b[5..9]).all(u8::is_ascii_hexdigit)
    {
        n = &n[10..];
    }
    n.trim().to_string()
}

/// Whether gilrs knows the pad's layout (an Xbox, PlayStation, Switch or
/// other pad in the SDL list), rather than a stick read as it comes.
fn known(pad: &Gamepad) -> bool {
    pad.mapping_source() != MappingSource::None
}

fn kind_name(pad: &Gamepad) -> &'static str {
    if known(pad) {
        "game pad"
    } else {
        "joystick"
    }
}

/// Whether DirectInput's button `n` is down.
pub fn button(pad: &Gamepad, n: i32) -> bool {
    let Ok(n) = usize::try_from(n) else {
        return false;
    };
    if known(pad) {
        return PAD_BUTTONS.get(n).is_some_and(|&b| pad.is_pressed(b));
    }
    pad.state()
        .buttons()
        .any(|(code, data)| data.is_pressed() && raw_button(code.into_u32()) == Some(n))
}

/// An axis (−3 X to −13 RZ), DirectInput's way round; `None` when the
/// stick has no such axis.
pub fn axis(pad: &Gamepad, code: i32, rest: &Rest) -> Option<f32> {
    let i = AXES.iter().position(|&c| c == code)?;
    if known(pad) {
        // DirectInput gives an Xbox pad's triggers as one Z axis, the
        // left trigger one way and the right the other.
        if i == 2 {
            let t = |b| pad.button_data(b).map_or(0.0, |d| d.value());
            return Some(t(Button::LeftTrigger2) - t(Button::RightTrigger2));
        }
        let (a, sign) = [
            (Axis::LeftStickX, 1.0),
            (Axis::LeftStickY, -1.0),
            (Axis::LeftZ, 1.0),
            (Axis::RightStickX, 1.0),
            (Axis::RightStickY, -1.0),
            (Axis::RightZ, 1.0),
        ][i];
        let code = pad.axis_code(a)?;
        let v = pad.state().axis_data(code)?.value();
        return Some(from_rest(pad, code.into_u32(), v, rest) * sign);
    }
    raw_axis(pad, i, Some(rest))
}

/// An axis's value from its rest: as it is if it rests in the middle,
/// else 0 at rest to ±1 at its other end.
fn from_rest(pad: &Gamepad, code: u32, v: f32, rest: &Rest) -> f32 {
    match rest.get(&(pad.id(), code)) {
        Some(&r) if r == 0.0 => 0.0,
        Some(r) => (v - r) * 0.5,
        None => v,
    }
}

/// A stick's throttle lever, 0 pulled back to 1 pushed forward: its Z
/// axis, which the game reads the other way round (0x4c7ee0 takes
/// range − Z), or a slider when it has no Z, as many sticks report
/// their throttle.
pub fn throttle(pad: &Gamepad) -> Option<f32> {
    if known(pad) {
        return None;
    }
    let z = raw_axis(pad, 2, None).or_else(|| raw_axis(pad, 6, None))?;
    Some((1.0 - z) * 0.5)
}

/// Accelerate bound to the Z axis (−7, or its other half −6): how far
/// open, 0 to 1. The game makes this axis 0 to 1 over Z's range
/// (0x449e20), Z read as range − Z, with nothing below 0.2. (Bound to RZ
/// the game's axis is stuck at 1; here RZ reads like Z.)
pub fn throttle_axis(pad: &Gamepad, code: i32, rest: &Rest) -> Option<f32> {
    let v = match code {
        -7 | -6 => throttle(pad).or_else(|| axis(pad, -7, rest).map(|z| (1.0 - z) * 0.5))?,
        -13 | -12 => (1.0 - axis(pad, -13, rest)?) * 0.5,
        _ => return None,
    };
    Some(if v <= THROTTLE_FLOOR { 0.0 } else { v })
}

/// A stick's own axis by DirectInput's place (0 X to 5 RZ, 6 the
/// slider), the way round DirectInput gives it; read from its rest when
/// given the rests (not for a throttle lever, which rests at an end).
fn raw_axis(pad: &Gamepad, i: usize, rest: Option<&Rest>) -> Option<f32> {
    let (code, data) = pad
        .state()
        .axes()
        .find(|(code, _)| raw_axis_place(code.into_u32()) == Some(i))?;
    // gilrs turns the axes it takes for Y round so that up is positive;
    // DirectInput (and the HID report) has down positive.
    let flipped = [Axis::LeftStickY, Axis::RightStickY]
        .into_iter()
        .any(|a| pad.axis_code(a) == Some(code));
    let v = match rest {
        Some(rest) => from_rest(pad, code.into_u32(), data.value(), rest),
        None => data.value(),
    };
    Some(if flipped { -v } else { v })
}

/// The DirectInput button number of a stick's native button code.
fn raw_button(code: u32) -> Option<usize> {
    let (kind, n) = (code >> 16, (code & 0xffff) as usize);
    if cfg!(target_os = "macos") {
        // HID: the button page, usages from 1.
        (kind == 0x09 && n >= 1).then(|| n - 1)
    } else if cfg!(target_os = "linux") {
        // evdev: BTN_JOYSTICK on, then the pad block, then BTN_TRIGGER_HAPPY.
        match (kind, n) {
            (1, 0x120..=0x12f) => Some(n - 0x120),
            (1, 0x130..=0x13f) => Some(n - 0x130),
            (1, 0x2c0..=0x2e7) => Some(n - 0x2c0 + 16),
            _ => None,
        }
    } else {
        // Windows.Gaming.Input: kind 0 buttons, by index.
        (kind == 0).then_some(n)
    }
}

/// DirectInput's place (0 X to 5 RZ, 6 a slider or throttle) of a
/// stick's native axis code.
fn raw_axis_place(code: u32) -> Option<usize> {
    let (kind, n) = (code >> 16, code & 0xffff);
    if cfg!(target_os = "macos") {
        match (kind, n) {
            // Generic desktop X, Y, Z, Rx, Ry, Rz, then the slider and dial.
            (0x01, 0x30..=0x35) => Some((n - 0x30) as usize),
            (0x01, 0x36 | 0x37) => Some(6),
            // Simulation controls: the throttle.
            (0x02, 0xbb) => Some(6),
            _ => None,
        }
    } else if cfg!(target_os = "linux") {
        // evdev ABS_X to ABS_RZ, then ABS_THROTTLE.
        match (kind, n) {
            (3, 0..=5) => Some(n as usize),
            (3, 6) => Some(6),
            _ => None,
        }
    } else {
        // Windows.Gaming.Input: kind 1 axes, by index.
        match (kind, n) {
            (1, 0..=5) => Some(n as usize),
            (1, 6) => Some(6),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_codes() {
        if cfg!(target_os = "linux") {
            assert_eq!(raw_button((1 << 16) | 0x120), Some(0));
            assert_eq!(raw_button((1 << 16) | 0x12b), Some(11));
            assert_eq!(raw_axis_place((3 << 16) | 2), Some(2));
            assert_eq!(raw_axis_place((3 << 16) | 0x10), None);
        }
    }

    #[test]
    fn browser_names() {
        assert_eq!(
            plain_name("Xbox Wireless Controller (STANDARD GAMEPAD Vendor: 045e Product: 0b13)"),
            "Xbox Wireless Controller"
        );
        assert_eq!(plain_name("054c-05c4-Wireless Controller"), "Wireless Controller");
        assert_eq!(plain_name("Xbox 360 Controller (XInput STANDARD GAMEPAD)"), "Xbox 360 Controller");
        assert_eq!(plain_name("Logitech Extreme 3D (rev 2)"), "Logitech Extreme 3D (rev 2)");
    }
}
