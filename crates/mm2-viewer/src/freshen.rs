//! Freshened engine sounds: the player's own `RES/ENGINE.RES` samples
//! given a touch more life as they load, so OpenMCM2 ships the recipe and
//! never the recordings. Michael's chain (his `rebuild44.py`), per sample:
//!
//! - a 1.16% higher pitch (played that much faster, as his formant-kept
//!   shift is too small to tell apart);
//! - the class's EQ, a few peaking bands;
//! - +9 dB into two soft clippers (tanh, then atan), for a fuller sound;
//! - the level put back at the original's loudness + 1.5 dB, peaks under
//!   0.95;
//! - samples that loop (idle, torque and power band) are made seamless,
//!   their start cross-faded with what follows their end; the others fade
//!   out over their last 5 ms.
//!
//! Off unless turned on (`--fresh-sounds`).

use mm2_formats::wav::Sound;

/// His pitch: rubberband `pitch=1.011619`.
const PITCH: f64 = 1.011619;

/// Whether the engine samples are freshened.
pub fn on() -> bool {
    crate::tweaks::more().fresh_sounds
}

/// A peaking band: centre Hz, Q, gain dB (ffmpeg's `equalizer`, `t=q`).
type Band = (f64, f64, f64);

fn eq(class: &str) -> &'static [Band] {
    match class {
        "125" => &[(150.0, 1.0, 1.5), (1400.0, 1.0, 2.5), (3200.0, 1.2, 5.0), (5000.0, 1.5, 2.0)],
        "250" => &[(180.0, 1.0, 3.0), (1200.0, 1.0, 2.0), (2800.0, 1.2, 4.0)],
        "400" => &[(110.0, 1.0, 4.0), (250.0, 1.0, 2.0), (1100.0, 1.0, 2.0), (2600.0, 1.2, 2.5)],
        _ => &[],
    }
}

/// The samples his chain loops seamlessly, by name.
fn loops(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["idle", "lowtorque", "midtorque", "hightorque", "power"].iter().any(|k| n.contains(k))
}

/// An RBJ peaking biquad, run in place.
fn peaking(x: &mut [f64], rate: f64, (f, q, gain): Band) {
    if f >= rate * 0.49 {
        return;
    }
    let a = 10f64.powf(gain / 40.0);
    let w = std::f64::consts::TAU * f / rate;
    let alpha = w.sin() / (2.0 * q);
    let (b0, b1, b2) = (1.0 + alpha * a, -2.0 * w.cos(), 1.0 - alpha * a);
    let (a0, a1, a2) = (1.0 + alpha / a, -2.0 * w.cos(), 1.0 - alpha / a);
    let (b0, b1, b2, a1, a2) = (b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0);
    let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
    for s in x.iter_mut() {
        let y = b0 * *s + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        x2 = x1;
        x1 = *s;
        y2 = y1;
        y1 = y;
        *s = y;
    }
}

fn chain(x: &mut [f64], rate: f64, class: &str) {
    for &band in eq(class) {
        peaking(x, rate, band);
    }
    let boost = 10f64.powf(9.0 / 20.0);
    for s in x.iter_mut() {
        *s = std::f64::consts::FRAC_2_PI * (*s * boost).tanh().atan();
    }
}

fn rms(x: &[f64]) -> f64 {
    (x.iter().map(|s| s * s).sum::<f64>() / x.len().max(1) as f64).sqrt()
}

/// `sound` (named `name` in ENGINE.RES, for engine class `class`: 125,
/// 250 or 400) freshened.
pub fn engine(sound: &Sound, name: &str, class: &str) -> Sound {
    let ch = sound.channels.max(1) as usize;
    let n = sound.frames();
    if n < 64 {
        return sound.clone();
    }
    // Mono, as all the game's engine samples are; a stereo one is mixed down.
    let x: Vec<f64> = (0..n).map(|i| (0..ch).map(|c| sound.samples[i * ch + c] as f64).sum::<f64>() / ch as f64).collect();
    let rate = sound.rate as f64;
    let mut out = if loops(name) {
        // Three times round, so the filters have settled by the middle
        // one; its start cross-faded with what follows its end.
        let mut y: Vec<f64> = x.iter().chain(&x).chain(&x).copied().collect();
        chain(&mut y, rate, class);
        let f = (rate * 0.01).round().min((n / 8) as f64) as usize;
        let mut mid = y[n..2 * n].to_vec();
        for i in 0..f {
            let w = i as f64 / (f - 1).max(1) as f64;
            mid[i] = y[2 * n + i] * (1.0 - w) + mid[i] * w;
        }
        mid
    } else {
        let mut y = x.clone();
        chain(&mut y, rate, class);
        let f = (rate * 0.005).round().min((n / 10) as f64) as usize;
        for i in 0..f {
            let w = 1.0 - i as f64 / f.max(1) as f64;
            y[n - f + i] *= w * w;
        }
        y
    };
    let gain = 10f64.powf(1.5 / 20.0) * rms(&x) / rms(&out).max(1e-9);
    let peak = out.iter().fold(0.0f64, |m, s| m.max(s.abs())) * gain;
    let gain = if peak > 0.95 { gain * 0.95 / peak } else { gain };
    for s in &mut out {
        *s *= gain;
    }
    Sound {
        rate: (rate * PITCH).round() as u32,
        channels: 1,
        samples: out.into_iter().map(|s| s as f32).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(n: usize) -> Sound {
        Sound {
            rate: 22050,
            channels: 1,
            samples: (0..n).map(|i| 0.3 * (i as f32 * 0.07).sin() + 0.1 * (i as f32 * 0.6).sin()).collect(),
        }
    }

    #[test]
    fn keeps_length_and_lifts_level_a_little() {
        let s = tone(20000);
        let f = engine(&s, "16_125PowerBand01.wav", "125");
        assert_eq!(f.frames(), s.frames());
        assert_eq!(f.rate, 22306);
        let r = |s: &Sound| rms(&s.samples.iter().map(|&v| v as f64).collect::<Vec<_>>());
        let db = 20.0 * (r(&f) / r(&s)).log10();
        assert!((db - 1.5).abs() < 0.1, "{db}");
        assert!(f.samples.iter().all(|v| v.abs() <= 0.951));
    }

    #[test]
    fn loops_join_up() {
        let s = tone(20000);
        let f = engine(&s, "16_250IdleLoop07.wav", "250");
        // The step from its end round to its start is about as big as
        // the steps within it: no click.
        let steps = f.samples.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0f32, f32::max);
        let wrap = (f.samples[0] - f.samples[f.samples.len() - 1]).abs();
        assert!(wrap <= steps * 1.25, "{wrap} {steps}");
        // A one-shot fades out to nothing.
        let d = engine(&s, "16_250Decel01.wav", "250");
        assert!(d.samples.last().unwrap().abs() < 1e-3);
    }
}
