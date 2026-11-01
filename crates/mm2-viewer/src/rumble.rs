//! Pad vibration from the player's riding: a thump on landing, as hard
//! as the bike comes down, a knock over a sharp bump, a jolt as a crash
//! starts and a rumble while the thrown rider tumbles. Not the game's:
//! its DirectInput force feedback was for joysticks that had it, and is
//! not studied here. The profile's Force Feedback box turns it on and its
//! slider sets the strength.

use glam::Vec3;

/// A landing falling slower than this, ft/s, is felt not at all…
const SOFT: f32 = 5.0;
/// …and one this fast at full strength (a level landing crashes the
/// game's bike from 44).
const HARD: f32 = 32.0;
/// A rise in speed upwards in one frame on the ground, ft/s, that
/// counts as a bump, and the rise felt at full strength.
const BUMP: f32 = 4.0;
const BUMP_FULL: f32 = 14.0;
/// The tumbling rider's rumble, heavy and light motor.
const TUMBLE: [f32; 2] = [0.3, 0.5];

/// What the vibration needs of the player's bike each frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct Feel {
    pub airborne: bool,
    pub crashed: bool,
    /// The thrown rider touching the ground.
    pub tumbling: bool,
    pub velocity: Vec3,
}

/// One kick: both motors' strengths, fading to nothing over its length.
struct Pulse {
    strength: [f32; 2],
    left: f32,
    length: f32,
}

#[derive(Default)]
pub struct Rumble {
    last: Option<Feel>,
    /// How fast the bike was falling on its last frame in the air.
    fall: f32,
    pulses: Vec<Pulse>,
}

impl Rumble {
    /// The heavy and light motors' strengths for this frame, 0 to 1.
    pub fn update(&mut self, now: Feel, dt: f32) -> [f32; 2] {
        if let Some(was) = self.last {
            if now.airborne {
                self.fall = -now.velocity.y;
            } else if was.airborne {
                // Touching down: the bike's speed now is after the hit, so
                // the fall is the last frame in the air's.
                let s = ((self.fall - SOFT) / (HARD - SOFT)).clamp(0.0, 1.0);
                if s > 0.0 {
                    let wreck = if now.crashed { 0.5 } else { 1.0 };
                    self.kick([(0.2 + 0.8 * s) * wreck, (0.3 + 0.7 * s) * wreck], 0.18 + 0.27 * s);
                }
            } else if !now.crashed {
                let rise = now.velocity.y - was.velocity.y;
                let s = ((rise - BUMP) / (BUMP_FULL - BUMP)).clamp(0.0, 1.0);
                if s > 0.0 {
                    self.kick([0.25 + 0.45 * s, 0.35 + 0.45 * s], 0.11);
                }
            }
            if now.crashed && !was.crashed {
                self.kick([1.0, 1.0], 0.6);
            }
        }
        self.last = Some(now);

        let mut out = if now.tumbling { TUMBLE } else { [0.0; 2] };
        for p in &mut self.pulses {
            let fade = p.left / p.length;
            for (o, s) in out.iter_mut().zip(p.strength) {
                *o = o.max(s * fade);
            }
            p.left -= dt;
        }
        self.pulses.retain(|p| p.left > 0.0);
        out
    }

    /// Starts again, for a new race or a restart.
    pub fn reset(&mut self) {
        *self = Rumble::default();
    }

    fn kick(&mut self, strength: [f32; 2], length: f32) {
        self.pulses.push(Pulse { strength, left: length, length });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn air(vy: f32) -> Feel {
        Feel { airborne: true, velocity: Vec3::new(0.0, vy, 30.0), ..Default::default() }
    }

    fn ground(vy: f32) -> Feel {
        Feel { velocity: Vec3::new(0.0, vy, 30.0), ..Default::default() }
    }

    #[test]
    fn landings_thump_as_hard_as_they_come_down() {
        let land = |fall: f32| {
            let mut r = Rumble::default();
            r.update(ground(0.0), DT);
            r.update(air(-fall), DT);
            r.update(ground(0.0), DT)
        };
        assert_eq!(land(5.0), [0.0; 2], "a hop is not felt");
        let (soft, hard) = (land(15.0), land(35.0));
        assert!(soft[0] > 0.0 && hard[0] > soft[0] && hard[1] > soft[1]);
        assert!(land(80.0)[0] <= 1.0);
    }

    #[test]
    fn a_kick_fades_out() {
        let mut r = Rumble::default();
        r.update(ground(0.0), DT);
        r.update(air(-40.0), DT);
        let first = r.update(ground(0.0), DT);
        let next = r.update(ground(0.0), DT);
        assert!(next[0] < first[0]);
        for _ in 0..60 {
            r.update(ground(0.0), DT);
        }
        assert_eq!(r.update(ground(0.0), DT), [0.0; 2]);
    }

    #[test]
    fn crashes_jolt_and_tumbling_rumbles() {
        let mut r = Rumble::default();
        r.update(ground(0.0), DT);
        let crash = Feel { crashed: true, ..ground(0.0) };
        assert_eq!(r.update(crash, DT), [1.0, 1.0]);
        for _ in 0..60 {
            r.update(crash, DT);
        }
        assert_eq!(r.update(crash, DT), [0.0; 2]);
        assert_eq!(r.update(Feel { tumbling: true, ..crash }, DT), TUMBLE);
    }

    #[test]
    fn riding_along_is_still() {
        let mut r = Rumble::default();
        for i in 0..120 {
            // Gentle ground: up and down at a few ft/s.
            let vy = (i as f32 * 0.1).sin() * 4.0;
            assert_eq!(r.update(ground(vy), DT), [0.0; 2]);
        }
    }
}
