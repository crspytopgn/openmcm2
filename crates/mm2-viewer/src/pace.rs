//! Even frame times with VSync on. The clock read at the start of each
//! frame wobbles by a millisecond or two (when the program got to run,
//! not when the picture is shown), while the screen shows frames at its
//! own fixed beat. Moving the world on by the wobbly time makes things
//! that move across the screen, the other riders most of all, judder a
//! little even at a steady 60 fps. A frame time close to a whole number
//! of the screen's beats is taken as exactly that, and what that leaves
//! over is handed back a little at a time, so the game's clock keeps
//! with the real one.

#[derive(Debug, Default, Clone, Copy)]
pub struct Pace {
    /// Real time not yet given to the game, s (either way).
    owed: f32,
}

/// How near a whole number of beats a frame must be to be evened, of a
/// beat.
const NEAR: f32 = 0.2;
/// The share of what is owed handed back each frame.
const BACK: f32 = 0.05;

impl Pace {
    /// The time to move the game on by for a frame that took `real`
    /// seconds, on a screen showing a frame every `beat` seconds (`None`
    /// when not waiting for the screen, or it doesn't say).
    pub fn even(&mut self, real: f32, beat: Option<f32>) -> f32 {
        let Some(beat) = beat.filter(|b| *b > 0.002) else {
            self.owed = 0.0;
            return real;
        };
        let beats = (real / beat).round();
        if beats < 1.0 || (real - beats * beat).abs() > NEAR * beat {
            // A hitch or a change of pace: as it came.
            self.owed = 0.0;
            return real;
        }
        self.owed += real - beats * beat;
        let back = (self.owed * BACK).clamp(-0.1 * beat, 0.1 * beat);
        self.owed -= back;
        beats * beat + back
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wobbly_frames_come_out_even_and_keep_time() {
        let beat = 1.0 / 60.0;
        let mut pace = Pace::default();
        let (mut real, mut game) = (0.0f64, 0.0f64);
        let mut worst = 0.0f32;
        for i in 0..6000 {
            // ±1.5 ms of wobble, and a screen a little slower than it says.
            let wobble = ((i * 7919) % 31) as f32 / 30.0 * 0.003 - 0.0015;
            let t = beat * 1.001 + wobble;
            let dt = pace.even(t, Some(beat));
            if i > 100 {
                worst = worst.max((dt - beat).abs());
            }
            real += t as f64;
            game += dt as f64;
        }
        assert!(worst < 0.0005, "frames still uneven by {worst}");
        assert!((real - game).abs() < 0.01, "clock drifted {}", real - game);
    }

    #[test]
    fn hitches_and_no_vsync_pass_through() {
        let mut pace = Pace::default();
        assert_eq!(pace.even(0.045, Some(1.0 / 60.0)), 0.045);
        assert_eq!(pace.even(0.013, None), 0.013);
        let two = pace.even(0.0335, Some(1.0 / 60.0));
        assert!((two - 2.0 / 60.0).abs() < 1e-4);
    }
}
