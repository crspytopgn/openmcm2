//! Dynamic resolution for the enhanced drawing (`dynamic`, on with
//! `quality=auto`): while racing, the scene is drawn a little smaller when
//! frames start coming late, and sharper again once they keep time, so the
//! frame rate holds on a weak computer. On a fast one with a big screen
//! (where the preset draws the scene smaller than the screen) it goes past
//! the preset's size, up to the screen's own, for as long as frames keep
//! time. Only the size the 3D scene is drawn at changes (then stretched
//! over the screen as before); the menus and the race display stay sharp,
//! and the riding and the clock are untouched.
//!
//! The frame time is all there is to go on (a GPU's own timers are not
//! there everywhere), so with VSync a frame that keeps time says nothing
//! of how much time it had to spare. A step up is tried after a quiet
//! spell; if it comes in late it is undone within half a second, and the
//! next try waits twice as long.

use std::sync::atomic::{AtomicU32, Ordering};

/// The shares of the preset's scene size (each way) the scene can be
/// drawn at, sharpest first; [`PRESET`] is the preset's own. 0.6 each way
/// is 36% of the pixels: below that the picture goes soft faster than the
/// time it saves. 1.5 each way takes a 1440p scene to a 4K screen's own.
pub const STEPS: [f32; 9] = [1.5, 1.32, 1.15, 1.0, 0.92, 0.84, 0.76, 0.68, 0.6];
/// The preset's own size in [`STEPS`].
pub const PRESET: usize = 3;

/// The share now in use, as f32 bits (1.0 until the governor says).
static SCALE: AtomicU32 = AtomicU32::new(0x3f80_0000);
/// The most the scene can grow by before it is the screen's own size,
/// as f32 bits (the drawing says, each frame).
static ROOM: AtomicU32 = AtomicU32::new(0x3f80_0000);

/// The share of the preset's scene size each way to draw at now.
pub fn scale() -> f32 {
    f32::from_bits(SCALE.load(Ordering::Relaxed))
}

fn set_scale(k: f32) {
    SCALE.store(k.to_bits(), Ordering::Relaxed);
}

/// Says how much bigger, each way, the screen is than the preset's scene.
pub fn set_room(k: f32) {
    ROOM.store(k.max(1.0).to_bits(), Ordering::Relaxed);
}

fn room() -> f32 {
    f32::from_bits(ROOM.load(Ordering::Relaxed))
}

/// Back to the preset's size (out of a race).
pub fn reset() {
    set_scale(1.0);
}

/// The scene's size for the preset's `size` at the share in use, no
/// bigger than the `screen`.
pub fn scaled(size: (u32, u32), screen: (u32, u32)) -> (u32, u32) {
    let k = scale();
    if k == 1.0 {
        return size;
    }
    let w = ((size.0 as f32 * k).round() as u32).clamp(1, screen.0.max(size.0));
    let h = ((size.1 as f32 * k).round() as u32).clamp(1, screen.1.max(size.1));
    (w, h)
}

/// Late by this share of the frame's time, on average, steps down.
const LATE: f32 = 1.12;
/// On time to within this share, on average, may step up.
const ON_TIME: f32 = 1.03;
/// How quickly the average follows the frames: about the last 20.
const FOLLOW: f32 = 0.05;
/// Least time between two changes, s, so each one is seen before the next.
const SETTLE: f32 = 1.5;
/// How soon a step up that comes in late is undone, s.
const UNDO: f32 = 0.5;
/// The first wait before a step up, s, and the longest it grows to.
const FIRST_WAIT: f32 = 6.0;
const LONGEST_WAIT: f32 = 120.0;
/// A step up kept this long has held, s.
const TRIAL: f32 = 4.0;

#[derive(Debug, Clone)]
pub struct Governor {
    /// Which of [`STEPS`] is in use.
    step: usize,
    /// The frame time over the time wanted, averaged.
    late: f32,
    /// Since the last change, s.
    since: f32,
    /// How long to keep time before stepping up, s.
    wait: f32,
    /// Stepped up and not yet held for [`TRIAL`].
    trying: bool,
}

impl Default for Governor {
    fn default() -> Self {
        Governor {
            step: PRESET,
            late: 1.0,
            since: 0.0,
            wait: FIRST_WAIT,
            trying: false,
        }
    }
}

impl Governor {
    /// After a frame that took `real` seconds when `wanted` was the time a
    /// frame should take (the screen's beat with VSync, else a 60th of a
    /// second or the frame cap), with `room` to grow before the scene is
    /// the screen's own size. Returns the share to draw at.
    pub fn frame(&mut self, real: f32, wanted: f32, room: f32) -> f32 {
        // No growing past the screen (a share within 2% of it is as good).
        while self.step < PRESET && STEPS[self.step] > room * 1.02 {
            self.step += 1;
        }
        // A load or a pause in the window says nothing of the drawing.
        if !(real > 0.0 && real < 0.25) || wanted <= 0.0 {
            return STEPS[self.step];
        }
        self.since += real;
        self.late += (real / wanted - self.late) * FOLLOW;
        if self.trying && self.since > TRIAL {
            // The step up held: the next may come as soon as the first.
            self.trying = false;
            self.wait = FIRST_WAIT;
        }
        let settle = if self.trying { UNDO } else { SETTLE };
        let can_grow = self.step > 0 && (self.step > PRESET || STEPS[self.step - 1] <= room * 1.02);
        if self.since >= settle && self.late > LATE && self.step + 1 < STEPS.len() {
            if self.trying {
                self.wait = (self.wait * 2.0).min(LONGEST_WAIT);
                self.trying = false;
            }
            self.step += 1;
            self.changed();
        } else if self.since >= self.wait && self.late < ON_TIME && can_grow {
            self.step -= 1;
            self.trying = true;
            self.changed();
        }
        STEPS[self.step]
    }

    fn changed(&mut self) {
        self.since = 0.0;
        // Starts again from on time: the frames before were another size.
        self.late = 1.0;
    }

    /// Feeds a frame and puts the share in use for the drawing.
    pub fn apply(&mut self, real: f32, wanted: f32) {
        let k = self.frame(real, wanted, room());
        if k != scale() {
            set_scale(k);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BEAT: f32 = 1.0 / 60.0;

    /// Frames that take as long as `cost` × the pixels drawn would,
    /// rounded up to whole beats as VSync shows them; returns the share
    /// at the end and the frames that came late.
    fn run(g: &mut Governor, seconds: f32, cost: f32, room: f32) -> (f32, u32) {
        let mut t = 0.0;
        let mut k = STEPS[g.step];
        let mut late = 0;
        while t < seconds {
            let work = cost * k * k;
            let real = (work / BEAT).ceil().max(1.0) * BEAT;
            late += (real > BEAT * 1.5) as u32;
            k = g.frame(real, BEAT, room);
            t += real;
        }
        (k, late)
    }

    #[test]
    fn keeps_the_preset_size_when_the_screen_is_no_bigger() {
        let mut g = Governor::default();
        assert_eq!(run(&mut g, 60.0, BEAT * 0.3, 1.0).0, 1.0);
    }

    #[test]
    fn grows_to_the_screen_when_there_is_time_to_spare() {
        let mut g = Governor::default();
        // A 1440p scene on a 4K screen, drawn in a third of a beat.
        assert_eq!(run(&mut g, 60.0, BEAT * 0.3, 1.5).0, 1.5);
        // A smaller screen stops it short.
        let mut g = Governor::default();
        assert_eq!(run(&mut g, 60.0, BEAT * 0.3, 1.2).0, 1.15);
    }

    #[test]
    fn steps_down_till_frames_keep_time_and_stays_there() {
        let mut g = Governor::default();
        // Half again too much to draw at full size: 0.76 each way is 58%.
        let (k, _) = run(&mut g, 20.0, BEAT * 1.5, 1.0);
        assert!((0.68..=0.84).contains(&k), "{k}");
        // It tries a step up now and then, but spends most time keeping up.
        let (_, late) = run(&mut g, 120.0, BEAT * 1.5, 1.0);
        assert!(late < 60, "{late} frames late in two minutes");
    }

    #[test]
    fn a_fast_card_on_a_big_screen_settles_where_it_keeps_time() {
        let mut g = Governor::default();
        // Room for 1.5, time for about 1.2 each way.
        let (k, _) = run(&mut g, 60.0, BEAT * 0.68, 1.5);
        assert!((1.0..=1.15).contains(&k), "{k}");
        let (_, late) = run(&mut g, 120.0, BEAT * 0.68, 1.5);
        assert!(late < 60, "{late} frames late in two minutes");
    }

    #[test]
    fn comes_back_up_when_the_load_goes() {
        let mut g = Governor::default();
        run(&mut g, 20.0, BEAT * 2.0, 1.0);
        assert!(STEPS[g.step] < 1.0);
        assert_eq!(run(&mut g, 120.0, BEAT * 0.5, 1.0).0, 1.0);
    }

    #[test]
    fn hitches_are_ignored() {
        let mut g = Governor::default();
        for _ in 0..20 {
            g.frame(0.5, BEAT, 1.0);
            for _ in 0..100 {
                g.frame(BEAT, BEAT, 1.0);
            }
        }
        assert_eq!(STEPS[g.step], 1.0);
    }
}
