//! Keys standing in for a stick, as the game's key axes do (0x48b050,
//! bound by 0x449cc0 → 0x48af10): a fresh press starts the axis from the
//! centre, holding moves it 0.018 every 0.01 s (1.8 a second, so full lock
//! takes 0.56 s), and letting go snaps it back to the centre.

/// What one held key adds (0x449cc0's 0x3c9374bc) and how often (0x3c23d70a).
const STEP: f32 = 0.018;
const EVERY: f32 = 0.01;
/// The frame time a fresh press loses: the game's own 50 Hz step.
const FIRST_FRAME: f32 = 0.02;

#[derive(Debug, Clone, Copy, Default)]
pub struct KeyAxis {
    pub value: f32,
    /// Time saved up toward the next step for each key, as the game's
    /// +0x14 and +0x18.
    minus: f32,
    plus: f32,
    held: (bool, bool),
}

impl KeyAxis {
    /// Moves the axis on by `dt` with the keys for −1 and +1.
    pub fn update(&mut self, minus: bool, plus: bool, dt: f32) -> f32 {
        if (minus && !self.held.0) || (plus && !self.held.1) {
            // A fresh press starts from the centre.
            self.value = 0.0;
        }
        if !minus {
            self.minus = 0.0;
        }
        if !plus {
            self.plus = 0.0;
        }
        let fresh = (minus && !self.held.0, plus && !self.held.1);
        for (held, fresh, clock, sign) in [
            (minus, fresh.0, &mut self.minus, -1.0),
            (plus, fresh.1, &mut self.plus, 1.0),
        ] {
            // The frame a key goes down only starts its clock: in the
            // game that frame's time is lost. Ours loses one 50 Hz frame
            // (the game's step) whatever the frame rate, so the bars move
            // the same at 30 fps as at 240.
            if held {
                *clock += if fresh { dt - FIRST_FRAME } else { dt };
                while *clock > EVERY {
                    self.value = (self.value + sign * STEP).clamp(-1.0, 1.0);
                    *clock -= EVERY;
                }
            }
        }
        if !minus && !plus {
            self.value = 0.0;
        }
        self.held = (minus, plus);
        self.value
    }
}

/// The actions of the controls page (strings 402 to 415) that riding reads.
pub const STEER_LEFT: usize = 0;
pub const STEER_RIGHT: usize = 1;
pub const LEAN_FORWARD: usize = 2;
pub const LEAN_BACK: usize = 3;
pub const ACCELERATE: usize = 4;
pub const BRAKE: usize = 5;
pub const STUNTS_LOW: usize = 6;
pub const STUNTS_HIGH: usize = 7;
pub const KAHUNA: usize = 8;
pub const TAUNT: usize = 9;
pub const THRILL_CAM: usize = 10;
pub const REVERSE_CAM: usize = 11;
pub const CLUTCH: usize = 12;
pub const BACK_UP: usize = 13;

/// The keyboard device's place in `control.ctl` (string 466, "Keyboard").
pub const KEYBOARD: usize = 6;

/// The keyboard's scan code for each action, as `control.ctl` stores them.
#[derive(Debug, Clone)]
pub struct KeyMap(pub [Option<u32>; crate::profile::ACTIONS]);

impl KeyMap {
    /// `UI/PRESET.CTL`'s keyboard: the arrows steer, accelerate and brake,
    /// W and S lean, Z and X the stunts, Enter the Kahuna Dumpster and
    /// Space the taunt.
    pub fn factory() -> KeyMap {
        let mut m = [None; crate::profile::ACTIONS];
        for (a, code) in [203, 205, 17, 31, 200, 208, 44, 45, 28, 57, 42, 56, 29, 14]
            .into_iter()
            .enumerate()
        {
            m[a] = Some(code);
        }
        KeyMap(m)
    }

    /// The keyboard's keys in a profile's map; actions it leaves unset
    /// or gives to another device have no key.
    pub fn from_profile(keys: &crate::profile::Controls) -> KeyMap {
        let mut m = [None; crate::profile::ACTIONS];
        for (a, slot) in m.iter_mut().enumerate() {
            if let (0, code) = keys.get(KEYBOARD, a) {
                *slot = u32::try_from(code).ok();
            }
        }
        KeyMap(m)
    }

    /// Whether `action`'s key is among these scan codes.
    pub fn held_code(&self, action: usize, codes: &std::collections::HashSet<u32>) -> bool {
        self.0[action].is_some_and(|c| codes.contains(&c))
    }

    pub fn held(
        &self,
        action: usize,
        keys: &std::collections::HashSet<winit::keyboard::KeyCode>,
    ) -> bool {
        self.0[action].is_some_and(|code| {
            keys.iter()
                .any(|&k| crate::screens::keys::scan_code(k) == Some(code))
        })
    }
}

/// A timed key script, as the comparison runs record them (`inputs.txt`):
/// `wait MS`, `down SC` and `up SC` with hex scan codes; other lines
/// (`@F`, `mark`, `frames`) are notes for the capture and are skipped.
#[derive(Debug, Clone, Default)]
pub struct Script {
    /// Seconds from the start, pressed, scan code.
    events: Vec<(f32, bool, u32)>,
}

impl Script {
    pub fn parse(text: &str) -> Script {
        let mut at = 0.0;
        let mut events = Vec::new();
        for line in text.lines() {
            let mut words = line.split_whitespace();
            let (Some(word), arg) = (words.next(), words.next()) else {
                continue;
            };
            let code = || {
                let a = arg?;
                u32::from_str_radix(a.trim_start_matches("0x").trim_start_matches("0X"), 16).ok()
            };
            match word {
                "wait" => at += arg.and_then(|a| a.parse::<f32>().ok()).unwrap_or(0.0) / 1000.0,
                "down" | "up" => {
                    if let Some(c) = code() {
                        events.push((at, word == "down", c));
                    }
                }
                _ => {}
            }
        }
        // The recordings' harness lets every key go when its script
        // ends (the tapes coast from there), so a script does too.
        let mut held = Vec::new();
        for &(_, down, c) in &events {
            held.retain(|&h| h != c);
            if down {
                held.push(c);
            }
        }
        events.extend(held.into_iter().map(|c| (at, false, c)));
        Script { events }
    }

    /// The scan codes held `t` seconds in.
    pub fn held(&self, t: f32) -> std::collections::HashSet<u32> {
        let mut held = std::collections::HashSet::new();
        for &(at, down, code) in &self.events {
            if at > t {
                break;
            }
            if down {
                held.insert(code);
            } else {
                held.remove(&code);
            }
        }
        held
    }

    /// When the last key event happens.
    pub fn length(&self) -> f32 {
        self.events.last().map_or(0.0, |e| e.0)
    }
}

/// The factory joystick map (`UI/PRESET.CTL` `[Controller1]`): the X
/// axis steers, the Y axis leans, buttons 1 and 2 are the gas and brake
/// and 3 and 4 the stunts; the rest are keys.
pub fn factory_pad() -> [(i32, i32); crate::profile::ACTIONS] {
    [
        (2, -3),
        (2, -3),
        (2, -5),
        (2, -5),
        (2, 0),
        (2, 1),
        (2, 2),
        (2, 3),
        (0, 28),
        (0, 57),
        (0, 42),
        (0, 56),
        (0, 29),
        (0, 14),
    ]
}

/// A game pad's own buttons for the actions the factory map leaves on
/// keys, which a pad hasn't got: the stick clicks Kahuna and Thrill
/// Cam, the shoulders Reverse Cam and the clutch, the D-pad's left the
/// taunt and its down backing up. (DirectInput's numbers, the D-pad's
/// as [`crate::pad`] gives them.)
pub const PAD_EXTRAS: [(usize, i32); 6] = [
    (KAHUNA, 8),
    (THRILL_CAM, 9),
    (REVERSE_CAM, 4),
    (CLUTCH, 5),
    (TAUNT, 12),
    (BACK_UP, 11),
];

/// A game pad's map with [`PAD_EXTRAS`] filled in where it has no pad
/// button of its own, each only if the map doesn't use that button.
pub fn with_pad_extras(mut map: [(i32, i32); crate::profile::ACTIONS]) -> [(i32, i32); crate::profile::ACTIONS] {
    for (action, button) in PAD_EXTRAS {
        let used = map.iter().any(|&(k, c)| k == 2 && c == button);
        if map[action].0 != 2 && !used {
            map[action] = (2, button);
        }
    }
    map
}

/// A stick's steering is cubed (0x52e3e0, for joystick devices): fine
/// near the centre, full lock at the edge. The steering curve extra
/// (`curve`, % of the game's) eases it towards a straight line: the
/// power runs from 3 at 100% to 1 at 0%.
pub fn stick_steer(x: f32, curve: u32) -> f32 {
    if curve >= 100 {
        return x * x * x;
    }
    let power = 1.0 + 2.0 * curve as f32 / 100.0;
    x.signum() * x.abs().powf(power)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steering_curve() {
        assert_eq!(stick_steer(0.5, 100), 0.125);
        assert!((stick_steer(-0.5, 0) + 0.5).abs() < 1e-6);
        assert!((stick_steer(0.5, 50) - 0.25).abs() < 1e-6);
        assert_eq!(stick_steer(1.0, 30), 1.0);
    }

    #[test]
    fn pad_extras_fill_only_free_buttons() {
        let mut map = factory_pad();
        // The player put the low stunts on LB themselves.
        map[STUNTS_LOW] = (2, 4);
        let m = with_pad_extras(map);
        assert_eq!(m[KAHUNA], (2, 8));
        assert_eq!(m[CLUTCH], (2, 5));
        assert_eq!(m[REVERSE_CAM], (0, 56), "LB is taken, so Reverse Cam stays on its key");
        assert_eq!(m[STEER_LEFT], (2, -3));
        assert_eq!(m[ACCELERATE], (2, 0));
    }

    #[test]
    fn reads_a_script() {
        let s = Script::parse("down 0xc8\n@F\nwait 500\nmark x\ndown 0xcd\nwait 250\nup 0xc8\n");
        assert!(s.held(0.0).contains(&0xc8));
        assert_eq!(s.held(0.6).len(), 2);
        // Everything is let go when the script ends.
        assert!(s.held(0.8).is_empty());
        assert_eq!(s.length(), 0.75);
    }

    #[test]
    fn ramps_the_same_at_any_frame_rate() {
        let at = |fps: f32| {
            let mut a = KeyAxis::default();
            let mut v = 0.0;
            for _ in 0..(fps * 0.3).round() as usize {
                v = a.update(false, true, 1.0 / fps);
            }
            v
        };
        let fifty = at(50.0);
        for fps in [30.0, 60.0, 144.0, 240.0] {
            assert!((at(fps) - fifty).abs() <= 0.018 + 1e-5, "{fps}: {} vs {fifty}", at(fps));
        }
    }

    #[test]
    fn ramps_and_snaps_back() {
        let mut a = KeyAxis::default();
        let mut v = 0.0;
        for _ in 0..15 {
            v = a.update(false, true, 1.0 / 60.0);
        }
        // A quarter second in: about 0.45, not full lock.
        assert!((0.4..0.5).contains(&v), "{v}");
        for _ in 0..60 {
            v = a.update(false, true, 1.0 / 60.0);
        }
        assert_eq!(v, 1.0);
        assert_eq!(a.update(false, false, 1.0 / 60.0), 0.0);
        // Turning the other way starts again from the centre.
        a.update(false, true, 0.5);
        assert!(a.update(true, true, 1.0 / 60.0) < 0.1);
    }
}
