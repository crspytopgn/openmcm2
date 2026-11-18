//! "Freshen up" for the original's engine samples (the Freshen engine
//! sounds extra): the same chain as the engine sound page's switch, baked
//! into each sample as it loads from the player's own `ENGINE.RES`.
//!
//! The sample is raised to 44.1 kHz, then: a low shelf at 220 Hz (+8 dB)
//! and a little bump at 320 Hz for body, a touch of presence at 2.5 kHz,
//! a soft-clipped "air" band above 9 kHz from what is above 3 kHz, a very
//! short room, and a limiter. The samples loop, so every stage runs round
//! the sample's end back into its start: the loop seam stays seamless.

use mm2_formats::wav::Sound;

/// The rate the chain runs at.
const RATE: u32 = 44_100;
/// The low shelf's lift, dB (the page's Fullness at its default).
const FULLNESS: f32 = 8.0;

/// The freshened copy of `s`.
pub fn freshen(s: &Sound) -> Sound {
    let channels = s.channels.max(1) as usize;
    let frames = s.samples.len() / channels;
    if frames == 0 {
        return s.clone();
    }
    let factor = if s.rate < 32_000 { (RATE as f32 / s.rate.max(1) as f32).round().max(1.0) as usize } else { 1 };
    let rate = s.rate * factor as u32;
    let mut out = vec![0.0; frames * factor * channels];
    for c in 0..channels {
        let x: Vec<f32> = (0..frames).map(|i| s.samples[i * channels + c]).collect();
        let y = chain(&upsample(&x, factor), rate as f32);
        for (i, v) in y.into_iter().enumerate() {
            out[i * channels + c] = v;
        }
    }
    Sound { rate, channels: s.channels, samples: out }
}

/// `x` at `factor` times its rate, by windowed-sinc interpolation round
/// the loop.
fn upsample(x: &[f32], factor: usize) -> Vec<f32> {
    if factor == 1 {
        return x.to_vec();
    }
    const TAPS: isize = 16;
    let n = x.len() as isize;
    // Each in-between phase's taps, worked out once.
    let kernels: Vec<Vec<f32>> = (1..factor)
        .map(|p| {
            let frac = p as f32 / factor as f32;
            (-TAPS + 1..=TAPS)
                .map(|k| {
                    let t = k as f32 - frac;
                    let sinc = (std::f32::consts::PI * t).sin() / (std::f32::consts::PI * t);
                    sinc * (0.5 + 0.5 * (std::f32::consts::PI * t / TAPS as f32).cos())
                })
                .collect()
        })
        .collect();
    let mut y = Vec::with_capacity(x.len() * factor);
    for i in 0..n {
        y.push(x[i as usize]);
        for kernel in &kernels {
            let mut acc = 0.0;
            for (k, w) in (-TAPS + 1..=TAPS).zip(kernel) {
                acc += x[(i + k).rem_euclid(n) as usize] * w;
            }
            y.push(acc);
        }
    }
    y
}

/// A biquad from the RBJ cookbook, as Web Audio's filters are.
#[derive(Clone, Copy)]
struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
    z: [f32; 2],
}

impl Biquad {
    fn new(b: [f32; 3], a: [f32; 3]) -> Biquad {
        Biquad { b: [b[0] / a[0], b[1] / a[0], b[2] / a[0]], a: [a[1] / a[0], a[2] / a[0]], z: [0.0; 2] }
    }

    fn low_shelf(f: f32, db: f32, rate: f32) -> Biquad {
        let a = 10f32.powf(db / 40.0);
        let w = std::f32::consts::TAU * f / rate;
        let (sin, cos) = w.sin_cos();
        let alpha = sin / 2.0 * std::f32::consts::SQRT_2;
        let k = 2.0 * a.sqrt() * alpha;
        Biquad::new(
            [a * ((a + 1.0) - (a - 1.0) * cos + k), 2.0 * a * ((a - 1.0) - (a + 1.0) * cos), a * ((a + 1.0) - (a - 1.0) * cos - k)],
            [(a + 1.0) + (a - 1.0) * cos + k, -2.0 * ((a - 1.0) + (a + 1.0) * cos), (a + 1.0) + (a - 1.0) * cos - k],
        )
    }

    fn peaking(f: f32, q: f32, db: f32, rate: f32) -> Biquad {
        let a = 10f32.powf(db / 40.0);
        let w = std::f32::consts::TAU * f / rate;
        let (sin, cos) = w.sin_cos();
        let alpha = sin / (2.0 * q);
        Biquad::new([1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a], [1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a])
    }

    /// Web Audio's high-pass, whose Q is in dB.
    fn high_pass(f: f32, q_db: f32, rate: f32) -> Biquad {
        let w = std::f32::consts::TAU * f.min(rate * 0.45) / rate;
        let (sin, cos) = w.sin_cos();
        let alpha = sin / (2.0 * 10f32.powf(q_db / 20.0));
        Biquad::new([(1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0], [1.0 + alpha, -2.0 * cos, 1.0 - alpha])
    }

    fn run(&mut self, x: f32) -> f32 {
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

/// A short room: four damped combs into two all-passes, after 8 ms, its
/// echoes dying away by e every 60 ms like the page's noise burst.
struct Room {
    pre: Vec<f32>,
    combs: Vec<(Vec<f32>, f32)>,
    passes: Vec<(Vec<f32>, f32)>,
    at: usize,
    gain: f32,
}

impl Room {
    fn new(rate: f32) -> Room {
        let len = |ms: f32| ((ms * rate / 1000.0) as usize).max(1);
        let combs: Vec<(Vec<f32>, f32)> = [29.7, 37.1, 41.1, 43.7]
            .iter()
            .map(|&ms| (vec![0.0; len(ms)], (-(ms / 1000.0) / 0.06f32).exp()))
            .collect();
        // White noise in comes out at about its own loudness.
        let power: f32 = combs.iter().map(|(_, g)| 1.0 / (1.0 - g * g)).sum();
        Room {
            pre: vec![0.0; len(8.0)],
            combs,
            passes: [5.0, 1.7].iter().map(|&ms| (vec![0.0; len(ms)], 0.7)).collect(),
            at: 0,
            gain: 1.0 / power.sqrt(),
        }
    }

    fn run(&mut self, x: f32) -> f32 {
        let at = self.at;
        self.at = at.wrapping_add(1);
        let p = at % self.pre.len();
        let x = std::mem::replace(&mut self.pre[p], x);
        let mut y = 0.0;
        for (line, g) in &mut self.combs {
            let i = at % line.len();
            let out = line[i];
            line[i] = x + out * *g;
            y += out;
        }
        y *= self.gain;
        for (line, g) in &mut self.passes {
            let i = at % line.len();
            let d = line[i];
            let v = y + d * *g;
            line[i] = v;
            y = d - *g * v;
        }
        y
    }
}

/// The page's compressor (Web Audio's: -3 dB, a 3 dB knee, 20:1, 2 ms
/// attack, 120 ms release, 6 ms look-ahead, its own make-up gain).
struct Limiter {
    attack: f32,
    release: f32,
    env: f32,
    makeup: f32,
}

const THRESHOLD: f32 = -3.0;
const KNEE: f32 = 3.0;
const RATIO: f32 = 20.0;

impl Limiter {
    fn new(rate: f32) -> Limiter {
        let coef = |ms: f32| (-1.0 / (ms / 1000.0 * rate)).exp();
        Limiter {
            attack: coef(2.0),
            release: coef(120.0),
            env: -120.0,
            // Web Audio lifts the output by 0.6 of what full scale loses.
            makeup: -0.6 * (curve(0.0) - 0.0),
        }
    }

    /// The gain, dB, for a level `x` dB (`look` ahead of the output).
    fn gain(&mut self, look: f32) -> f32 {
        let level = 20.0 * look.abs().max(1e-6).log10();
        let c = if level > self.env { self.attack } else { self.release };
        self.env = level + c * (self.env - level);
        curve(self.env) - self.env + self.makeup
    }
}

/// The compressor's output level, dB, for an input level, dB.
fn curve(x: f32) -> f32 {
    let over = x - THRESHOLD;
    if 2.0 * over < -KNEE {
        x
    } else if 2.0 * over <= KNEE {
        x + (1.0 / RATIO - 1.0) * (over + KNEE / 2.0).powi(2) / (2.0 * KNEE)
    } else {
        THRESHOLD + over / RATIO
    }
}

/// The whole chain over one channel at `rate`, round the loop.
fn chain(x: &[f32], rate: f32) -> Vec<f32> {
    let n = x.len();
    let mut eq = [
        Biquad::low_shelf(220.0, FULLNESS, rate),
        Biquad::peaking(320.0, 0.9, 0.35 * FULLNESS, rate),
        Biquad::peaking(2500.0, 0.8, 1.5, rate),
    ];
    let mut air_in = Biquad::high_pass(3000.0, 0.707, rate);
    let mut air_out = Biquad::high_pass(9000.0, 0.7, rate);
    let mut room = Room::new(rate);
    // Web Audio's convolver evens out the burst's loudness: about 0.155 of
    // the input, times the page's 0.18.
    const ROOM: f32 = 0.155 * 0.18;
    // Half a second round the loop first, so the filters and the room are
    // already ringing as they will be when the loop comes round.
    let warm = (rate * 0.5) as usize;
    // The page's bus was 0.7; a little lower keeps the original 125's
    // samples as loud as they were on average (each within 3 dB), so the
    // engines sit against the crowd and the effects as in the game.
    const LEVEL: f32 = 0.56;
    let mut wet = Vec::with_capacity(n);
    for i in 0..warm + n {
        let v = x[(i + n * (warm / n + 1) - warm) % n];
        let e = eq.iter_mut().fold(v, |v, f| f.run(v));
        let air = air_out.run((4.0 * 3.0 * air_in.run(e)).tanh()) * 0.12;
        let r = room.run(e) * ROOM;
        if i >= warm {
            wet.push((e + air + r) * LEVEL);
        }
    }
    // The limiter, looking 6 ms ahead, also once round first.
    let look = ((rate * 0.006) as usize).max(1);
    let mut lim = Limiter::new(rate);
    let mut out = Vec::with_capacity(n);
    for i in 0..warm + n {
        let j = (i + n * (warm / n + 1) - warm) % n;
        let g = lim.gain(wet[(j + look) % n]);
        if i >= warm {
            out.push((wet[j] * 10f32.powf(g / 20.0)).clamp(-1.0, 1.0));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: u32, secs: f32, f: f32, amp: f32) -> Sound {
        let n = (rate as f32 * secs) as usize;
        Sound { rate, channels: 1, samples: (0..n).map(|i| amp * (std::f32::consts::TAU * f * i as f32 / rate as f32).sin()).collect() }
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    #[test]
    fn lifts_the_low_end_and_doubles_the_rate() {
        // 100 Hz fits a whole number of times in the loop.
        let low = freshen(&tone(22_050, 0.5, 100.0, 0.1));
        assert_eq!(low.rate, 44_100);
        assert_eq!(low.samples.len(), 2 * 11_025);
        let lift = 20.0 * (rms(&low.samples) / 0.1 * std::f32::consts::SQRT_2).log10();
        // +8 dB of shelf, a bump, less the bus's 0.56 (-5 dB).
        assert!((2.0..7.0).contains(&lift), "{lift}");
        let mid = freshen(&tone(22_050, 0.5, 1000.0, 0.1));
        assert!(rms(&mid.samples) < rms(&low.samples));
    }

    #[test]
    fn loud_samples_are_held_under_full_scale() {
        let s = freshen(&tone(22_050, 0.5, 100.0, 0.95));
        let peak = s.samples.iter().fold(0f32, |m, v| m.max(v.abs()));
        assert!(peak < 1.0, "{peak}");
        assert!(peak > 0.6, "{peak}");
    }

    #[test]
    fn the_loop_seam_stays_smooth() {
        let s = freshen(&tone(22_050, 0.5, 100.0, 0.3));
        let x = &s.samples;
        let jump = (x[0] - x[x.len() - 1]).abs();
        let step = x.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0f32, f32::max);
        assert!(jump <= step * 1.1, "{jump} {step}");
    }

    /// Freshens every WAV in `FRESHEN_IN` into `FRESHEN_OUT`, to listen to.
    #[test]
    #[ignore]
    fn freshen_a_folder() {
        let (Ok(from), Ok(to)) = (std::env::var("FRESHEN_IN"), std::env::var("FRESHEN_OUT")) else { return };
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let Ok(s) = mm2_formats::wav::parse(&std::fs::read(e.path()).unwrap()) else { continue };
            let f = freshen(&s);
            let data: Vec<u8> = f.samples.iter().flat_map(|v| ((v * 32767.0) as i16).to_le_bytes()).collect();
            let mut w = Vec::new();
            let block = 2 * f.channels as u32;
            w.extend(b"RIFF");
            w.extend((36 + data.len() as u32).to_le_bytes());
            w.extend(b"WAVEfmt ");
            w.extend(16u32.to_le_bytes());
            w.extend(1u16.to_le_bytes());
            w.extend(f.channels.to_le_bytes());
            w.extend(f.rate.to_le_bytes());
            w.extend((f.rate * block).to_le_bytes());
            w.extend((block as u16).to_le_bytes());
            w.extend(16u16.to_le_bytes());
            w.extend(b"data");
            w.extend((data.len() as u32).to_le_bytes());
            w.extend(data);
            std::fs::write(std::path::Path::new(&to).join(e.file_name()), w).unwrap();
        }
    }
}
