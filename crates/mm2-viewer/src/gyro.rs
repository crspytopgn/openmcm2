//! Gyro steering, an extra (off unless Extras sets a full-lock angle):
//! tip the pad left or right like a handlebar and the bike steers that
//! way. Not the game's: it read no motion sensors.
//!
//! gilrs can't read a pad's gyro, so SDL (built in, only its pad parts)
//! reads the motion sensors of the pads that have them: the Steam Deck,
//! the Steam Controller, DualSense, DualShock 4, Switch Pro and Joy-Cons.
//! Only their tilt is taken from SDL; the buttons and sticks still come
//! through gilrs as before, so nothing else changes. SDL starts only
//! when the extra is on.
//!
//! The tilt is the pad's sideways lean from level, the same whether it
//! is held flat or upright like a Steam Deck: the gravity the
//! accelerometer feels, steadied by the gyro so it follows the hands
//! at once and doesn't wander.

use std::ffi::{CStr, CString};

use sdl3_sys::everything::*;
use web_time::Instant;

/// Tilt within this of level steers nothing, radians (1.5°), so a pad
/// held a little off level rides straight.
const DEAD: f32 = 0.026;
/// How long the accelerometer takes to pull the gyro's reckoning back to
/// the true tilt, seconds: short enough that the gyro's drift never
/// shows, long enough that a jolt of the hands doesn't steer.
const SETTLE: f32 = 0.35;
/// How often new pads are looked for, seconds.
const RESCAN: f32 = 1.0;

/// The pad's sideways tilt, followed from its sensors.
#[derive(Debug, Default, Clone, Copy)]
pub struct Tilt {
    /// Radians, plus with the right side down.
    angle: Option<f32>,
}

impl Tilt {
    /// Takes this frame's readings, SDL's way round (x to the pad's right,
    /// y up out of its face, z towards the player; the accelerometer in
    /// m/s², pushing up against gravity at rest; the gyro in rad/s,
    /// anticlockwise looking down each axis), and gives the tilt.
    pub fn update(&mut self, accel: [f32; 3], gyro: [f32; 3], dt: f32) -> f32 {
        let len = (accel[0] * accel[0] + accel[1] * accel[1] + accel[2] * accel[2]).sqrt();
        if len < 1.0 {
            // No reading yet.
            return self.angle.unwrap_or(0.0);
        }
        let up = [accel[0] / len, accel[1] / len, accel[2] / len];
        // Right side down tips "up" towards the pad's left.
        let felt = (-up[0]).clamp(-1.0, 1.0).asin();
        let angle = match self.angle {
            None => felt,
            Some(was) => {
                // Up, fixed in the world, turns the other way in the pad:
                // its change is up × ω; the tilt's is minus its x part
                // over cos(tilt).
                let dx = up[1] * gyro[2] - up[2] * gyro[1];
                let turned = was - dx / felt.cos().max(0.2) * dt;
                let k = (-dt / SETTLE).exp();
                turned * k + felt * (1.0 - k)
            }
        };
        self.angle = Some(angle);
        angle
    }

    pub fn reset(&mut self) {
        self.angle = None;
    }
}

/// The steering for a tilt: nothing near level, full lock at `full`
/// radians, in straight proportion between.
pub fn steer(angle: f32, full: f32) -> f32 {
    let full = full.max(DEAD * 2.0);
    let a = angle.abs();
    if a <= DEAD {
        return 0.0;
    }
    angle.signum() * ((a - DEAD) / (full - DEAD)).min(1.0)
}

struct Pad {
    id: SDL_JoystickID,
    handle: *mut SDL_Gamepad,
    has_accel: bool,
}

/// SDL's pads that have a gyro.
pub struct Gyro {
    pads: Vec<Pad>,
    tilt: Tilt,
    /// The pad whose tilt is followed.
    using: Option<SDL_JoystickID>,
    scanned: Option<Instant>,
    last: Option<Instant>,
}

fn set_hint(name: &CStr, value: &str) {
    let v = CString::new(value).unwrap();
    // SAFETY: both are NUL-terminated strings that live through the call.
    unsafe {
        SDL_SetHintWithPriority(name.as_ptr(), v.as_ptr(), SDL_HINT_OVERRIDE);
    }
}

fn error() -> String {
    // SAFETY: SDL_GetError always gives a NUL-terminated string.
    unsafe { CStr::from_ptr(SDL_GetError()).to_string_lossy().into_owned() }
}

impl Gyro {
    /// Starts SDL's pad reading, or None (with a line in the log) where
    /// it can't.
    pub fn new() -> Option<Gyro> {
        // Steam hides the Steam Deck's and Steam Controller's own device
        // from SDL games behind its virtual pad, which has no gyro. Only
        // the tilt is read here, so the real one is wanted.
        set_hint(c"SDL_GAMECONTROLLER_IGNORE_DEVICES", "");
        set_hint(c"SDL_JOYSTICK_HIDAPI_STEAM", "1");
        set_hint(c"SDL_JOYSTICK_HIDAPI_STEAMDECK", "1");
        // SDL has no window of its own here, so it never has the focus.
        set_hint(c"SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1");
        // Pads with no gyro are left to gilrs alone.
        for driver in [c"SDL_JOYSTICK_HIDAPI_XBOX", c"SDL_JOYSTICK_HIDAPI_GAMECUBE", c"SDL_JOYSTICK_HIDAPI_LUNA", c"SDL_JOYSTICK_HIDAPI_STADIA", c"SDL_JOYSTICK_HIDAPI_SHIELD"] {
            set_hint(driver, "0");
        }
        // The light bar and player light stay as they are.
        set_hint(c"SDL_JOYSTICK_HIDAPI_PS5_PLAYER_LED", "0");
        // SAFETY: SDL_Init is called once, from the main thread.
        if !unsafe { SDL_Init(SDL_INIT_GAMEPAD) } {
            eprintln!("gyro: SDL didn't start: {}", error());
            return None;
        }
        eprintln!("gyro: on, looking for pads with motion sensors");
        Some(Gyro { pads: Vec::new(), tilt: Tilt::default(), using: None, scanned: None, last: None })
    }

    /// Opens pads that have come since the last look, and lets go of
    /// those that have gone.
    fn scan(&mut self) {
        self.pads.retain(|p| {
            // SAFETY: the handle is one SDL_OpenGamepad gave and not yet closed.
            let on = unsafe { SDL_GamepadConnected(p.handle) };
            if !on {
                unsafe { SDL_CloseGamepad(p.handle) };
            }
            on
        });
        let mut count = 0;
        // SAFETY: SDL gives an array of `count` ids, freed below.
        let ids = unsafe { SDL_GetGamepads(&mut count) };
        if ids.is_null() {
            return;
        }
        let list: Vec<SDL_JoystickID> = (0..count.max(0) as usize).map(|i| unsafe { *ids.add(i) }).collect();
        unsafe { SDL_free(ids.cast()) };
        for id in list {
            if self.pads.iter().any(|p| p.id == id) {
                continue;
            }
            // SAFETY: an id SDL just listed.
            let handle = unsafe { SDL_OpenGamepad(id) };
            if handle.is_null() {
                continue;
            }
            let name = unsafe { CStr::from_ptr(SDL_GetGamepadName(handle)).to_string_lossy().into_owned() };
            if !unsafe { SDL_GamepadHasSensor(handle, SDL_SENSOR_GYRO) } {
                eprintln!("gyro: {name} has no gyro");
                unsafe { SDL_CloseGamepad(handle) };
                // Kept out of the next looks by a closed entry.
                self.pads.push(Pad { id, handle: std::ptr::null_mut(), has_accel: false });
                continue;
            }
            let has_accel = unsafe { SDL_GamepadHasSensor(handle, SDL_SENSOR_ACCEL) };
            unsafe {
                SDL_SetGamepadSensorEnabled(handle, SDL_SENSOR_GYRO, true);
                if has_accel {
                    SDL_SetGamepadSensorEnabled(handle, SDL_SENSOR_ACCEL, true);
                }
            }
            eprintln!("gyro: reading {name}{}", if has_accel { "" } else { " (no accelerometer, so no steering)" });
            self.pads.push(Pad { id, handle, has_accel });
        }
    }

    /// This frame's steering, -1 (full left) to 1, for full lock at
    /// `full_lock` degrees of tilt.
    pub fn steer(&mut self, full_lock: f32) -> f32 {
        let now = Instant::now();
        let dt = self.last.map_or(0.0, |t| now.duration_since(t).as_secs_f32()).min(0.1);
        self.last = Some(now);
        if self.scanned.is_none_or(|t| now.duration_since(t).as_secs_f32() > RESCAN) {
            // Closed entries for pads with no gyro are kept only so as not
            // to reopen them; SDL_GamepadConnected can't take a null.
            let gone: Vec<SDL_JoystickID> = self.pads.iter().filter(|p| p.handle.is_null()).map(|p| p.id).collect();
            self.pads.retain(|p| !p.handle.is_null());
            self.scan();
            for id in gone {
                if !self.pads.iter().any(|p| p.id == id) {
                    self.pads.push(Pad { id, handle: std::ptr::null_mut(), has_accel: false });
                }
            }
            self.scanned = Some(now);
        }
        // SAFETY: pumps SDL's pad state, from the main thread.
        unsafe { SDL_UpdateGamepads() };
        let Some(pad) = self.pads.iter().find(|p| p.has_accel && !p.handle.is_null()) else {
            self.using = None;
            return 0.0;
        };
        if self.using != Some(pad.id) {
            self.using = Some(pad.id);
            self.tilt.reset();
        }
        let (mut accel, mut gyro) = ([0.0f32; 3], [0.0f32; 3]);
        // SAFETY: each reads 3 floats into a 3-float array.
        unsafe {
            SDL_GetGamepadSensorData(pad.handle, SDL_SENSOR_ACCEL, accel.as_mut_ptr(), 3);
            SDL_GetGamepadSensorData(pad.handle, SDL_SENSOR_GYRO, gyro.as_mut_ptr(), 3);
        }
        let angle = self.tilt.update(accel, gyro, dt);
        steer(angle, full_lock.to_radians())
    }
}

impl Drop for Gyro {
    fn drop(&mut self) {
        for p in &self.pads {
            if !p.handle.is_null() {
                // SAFETY: opened and not yet closed.
                unsafe { SDL_CloseGamepad(p.handle) };
            }
        }
        unsafe { SDL_Quit() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: f32 = 9.81;
    const DT: f32 = 1.0 / 60.0;

    /// The accelerometer of a pad held flat, rolled `deg` right side down.
    fn flat(deg: f32) -> [f32; 3] {
        let r = deg.to_radians();
        [-G * r.sin(), G * r.cos(), 0.0]
    }

    /// The same for a pad held upright (face towards the player, like a
    /// Steam Deck), turned `deg` clockwise like a wheel.
    fn upright(deg: f32) -> [f32; 3] {
        let r = deg.to_radians();
        // Up lies along the pad's face, its top; turning it clockwise
        // tips up towards the pad's left.
        [-G * r.sin(), 0.0, -G * r.cos()]
    }

    #[test]
    fn tilt_steers_the_same_held_flat_or_upright() {
        for deg in [-30.0f32, -10.0, 0.0, 10.0, 30.0] {
            let a = Tilt::default().update(flat(deg), [0.0; 3], DT);
            let b = Tilt::default().update(upright(deg), [0.0; 3], DT);
            assert!((a.to_degrees() - deg).abs() < 0.01, "{a} {deg}");
            assert!((b.to_degrees() - deg).abs() < 0.01, "{b} {deg}");
        }
    }

    #[test]
    fn right_side_down_steers_right() {
        let a = Tilt::default().update(flat(20.0), [0.0; 3], DT);
        assert!(steer(a, 30f32.to_radians()) > 0.5);
        assert_eq!(steer(0.01, 0.5), 0.0, "near level rides straight");
        assert_eq!(steer(1.0, 0.5), 1.0);
        assert_eq!(steer(-1.0, 0.5), -1.0);
    }

    #[test]
    fn the_gyro_turns_the_tilt_before_gravity_catches_up() {
        // Held flat, rolling right side down at 1 rad/s: about the z
        // axis (towards the player), clockwise looking down it, so minus.
        let mut t = Tilt::default();
        t.update(flat(0.0), [0.0; 3], DT);
        let a = t.update(flat(0.0), [0.0, 0.0, -1.0], DT);
        assert!(a > 0.0, "{a}");
        // Upright, turning like a wheel is about the axis out of its
        // face, y; clockwise is minus again.
        let mut t = Tilt::default();
        t.update(upright(0.0), [0.0; 3], DT);
        let b = t.update(upright(0.0), [0.0, -1.0, 0.0], DT);
        assert!(b > 0.0, "{b}");
    }

    #[test]
    fn a_steady_pad_settles_on_its_true_tilt() {
        let mut t = Tilt::default();
        t.update(flat(0.0), [0.0; 3], DT);
        // A gyro that reads a little turn though the pad is still.
        let mut a = 0.0;
        for _ in 0..600 {
            a = t.update(flat(15.0), [0.0, 0.0, 0.02], DT);
        }
        assert!((a.to_degrees() - 15.0).abs() < 1.0, "{}", a.to_degrees());
    }

    /// SDL starts and reads nothing with no pad (`--ignored`: it opens
    /// the machine's pads).
    #[test]
    #[ignore]
    fn starts_with_no_pad() {
        let mut g = Gyro::new().expect("SDL starts");
        for _ in 0..3 {
            assert_eq!(g.steer(30.0), 0.0);
        }
    }
}
