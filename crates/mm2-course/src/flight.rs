//! Flight paths: a plane, helicopter or blimp flying round a loop over
//! the track.
//!
//! Each flight goes into the track as the game's own animated model: an
//! `.mcf` naming the craft's model and one looping motion, the `.mot`
//! holding where the craft is on each frame (its root, in track
//! coordinates) and its propellers or rotors turning, and an
//! `[AnimationN]` in the scene with the craft's engine sound. The game's
//! models and sounds are used, so a flight shows in the original game as
//! it does here; with the free pack its stand-in flies the same path.

use crate::{Course, Craft, Flight};

/// A propeller or rotor: its model frame (1 based), the axis it turns
/// about, and where it sits on its parent.
struct Rotor {
    frame: u8,
    axis: u8,
    at: [f32; 3],
}

const X: u8 = 0;
const Y: u8 = 1;
const Z: u8 = 2;

fn rotors(craft: Craft) -> &'static [Rotor] {
    match craft {
        Craft::Biplane => &[Rotor { frame: 2, axis: Z, at: [0.0, 0.0, 7.427] }],
        Craft::Helicopter => &[Rotor { frame: 2, axis: Y, at: [0.0, 11.642, 0.0] }, Rotor { frame: 3, axis: X, at: [-0.824, 7.645, -22.885] }],
        Craft::CargoPlane => &[Rotor { frame: 3, axis: Z, at: [-24.754, 1.355, 18.279] }, Rotor { frame: 4, axis: Z, at: [24.754, 1.355, 18.279] }],
        Craft::Blimp => &[],
    }
}

/// The craft's engine sound in `AUDIO.RES`, and how far it carries
/// (most, and full loudness within), ft.
fn sound(craft: Craft) -> (&'static str, f32, f32) {
    match craft {
        Craft::Biplane => ("airplane02", 700.0, 120.0),
        Craft::CargoPlane => ("airplane03", 1000.0, 200.0),
        Craft::Helicopter => ("Heli01", 600.0, 110.0),
        Craft::Blimp => ("zepplin", 450.0, 90.0),
    }
}

/// Frames a second of the motion.
fn fps(craft: Craft) -> f32 {
    match craft {
        Craft::Blimp => 6.0,
        _ => 15.0,
    }
}

/// The longest loop a motion holds, s.
const LONGEST: f32 = 900.0;

/// The points a flight goes round: its own, or with fewer than three, a
/// circle round them.
pub fn loop_points(f: &Flight) -> Vec<[f32; 3]> {
    if f.points.len() >= 3 {
        return f.points.clone();
    }
    let Some(first) = f.points.first() else { return Vec::new() };
    let last = f.points.last().unwrap();
    let mid = [(first[0] + last[0]) / 2.0, (first[1] + last[1]) / 2.0, (first[2] + last[2]) / 2.0];
    let half = ((last[0] - first[0]).powi(2) + (last[2] - first[2]).powi(2)).sqrt() / 2.0;
    let r = half.max(match f.craft {
        Craft::Helicopter => 160.0,
        Craft::Blimp => 300.0,
        _ => 450.0,
    });
    let turn = (last[2] - first[2]).atan2(last[0] - first[0]);
    (0..8)
        .map(|k| {
            let a = turn + k as f32 * std::f32::consts::TAU / 8.0;
            // Wider along the two points than across.
            let across = if f.points.len() == 2 { 0.55 } else { 1.0 };
            let (s, c) = a.sin_cos();
            let (x, z) = (c * r, s * r * across);
            let (ts, tc) = turn.sin_cos();
            [mid[0] + x * tc - z * ts, mid[1], mid[2] + x * ts + z * tc]
        })
        .collect()
}

/// The flight's loop every couple of feet, in track coordinates, never
/// lower than a little over the ground.
pub fn path(f: &Flight, ground: &dyn Fn(f32, f32) -> f32) -> Vec<[f32; 3]> {
    let pts = loop_points(f);
    let n = pts.len();
    if n < 2 {
        return Vec::new();
    }
    let low = match f.craft {
        Craft::Helicopter => 25.0,
        Craft::Blimp => 60.0,
        _ => 45.0,
    };
    // Each point's height over the ground made a height in the world.
    let abs: Vec<[f32; 3]> = pts.iter().map(|p| [p[0], ground(p[0], p[2]) + p[1].max(low), p[2]]).collect();
    let mut out = Vec::new();
    for i in 0..n {
        let (a, b, c, d) = (abs[(i + n - 1) % n], abs[i], abs[(i + 1) % n], abs[(i + 2) % n]);
        let len = ((c[0] - b[0]).powi(2) + (c[1] - b[1]).powi(2) + (c[2] - b[2]).powi(2)).sqrt();
        let steps = (len / 2.0).ceil().max(1.0) as usize;
        for k in 0..steps {
            let t = k as f32 / steps as f32;
            let (t2, t3) = (t * t, t * t * t);
            let f = |a: f32, b: f32, c: f32, d: f32| 0.5 * (2.0 * b + (-a + c) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (-a + 3.0 * b - 3.0 * c + d) * t3);
            let mut p = [f(a[0], b[0], c[0], d[0]), f(a[1], b[1], c[1], d[1]), f(a[2], b[2], c[2], d[2])];
            p[1] = p[1].max(ground(p[0], p[2]) + low * 0.6);
            out.push(p);
        }
    }
    out
}

/// Where a craft is along its loop and which way it faces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub at: [f32; 3],
    pub look: [f32; 3],
    pub up: [f32; 3],
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
    [v[0] / l, v[1] / l, v[2] / l]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// A loop measured for travel along it.
pub struct Route {
    pts: Vec<[f32; 3]>,
    s: Vec<f32>,
    pub length: f32,
}

impl Route {
    pub fn new(pts: Vec<[f32; 3]>) -> Route {
        let mut s = Vec::with_capacity(pts.len() + 1);
        let mut total = 0.0;
        for i in 0..pts.len() {
            s.push(total);
            let d = sub(pts[(i + 1) % pts.len()], pts[i]);
            total += dot(d, d).sqrt();
        }
        Route { pts, s, length: total }
    }

    fn at(&self, s: f32) -> [f32; 3] {
        let n = self.pts.len();
        if n == 0 {
            return [0.0; 3];
        }
        let s = s.rem_euclid(self.length.max(1e-3));
        let i = self.s.partition_point(|&v| v <= s).saturating_sub(1);
        let (a, b) = (self.pts[i], self.pts[(i + 1) % n]);
        let seg = (self.s.get(i + 1).copied().unwrap_or(self.length) - self.s[i]).max(1e-4);
        let t = ((s - self.s[i]) / seg).clamp(0.0, 1.0);
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
    }

    /// The craft `s` ft round the loop, flying at `speed` ft/s.
    pub fn pose(&self, craft: Craft, s: f32, speed: f32) -> Pose {
        let at = self.at(s);
        let (back, ahead) = (self.at(s - 12.0), self.at(s + 12.0));
        let tangent = norm(sub(ahead, back));
        // Which way and how hard it turns: the change of direction over
        // the stretch, as a sideways pull, ft/s².
        let (b2, a2) = (norm(sub(at, self.at(s - 40.0))), norm(sub(self.at(s + 40.0), at)));
        let pull = [(a2[0] - b2[0]) / 40.0 * speed * speed, 0.0, (a2[2] - b2[2]) / 40.0 * speed * speed];
        // No steeper than a 50 degree bank.
        let most = 32.0 * 1.19;
        let len = (pull[0] * pull[0] + pull[2] * pull[2]).sqrt();
        let pull = if len > most { [pull[0] / len * most, 0.0, pull[2] / len * most] } else { pull };
        let flat = norm([tangent[0], 0.0, tangent[2]]);
        let (look, lean) = match craft {
            Craft::Biplane | Craft::CargoPlane => (tangent, 1.0),
            // Nose a little down as it flies; a gentle lean.
            Craft::Helicopter => (norm([flat[0], -0.12, flat[2]]), 0.5),
            Craft::Blimp => (flat, 0.08),
        };
        let up = [pull[0] * lean, 32.0, pull[2] * lean];
        let up = norm(sub(up, {
            let d = dot(up, look);
            [look[0] * d, look[1] * d, look[2] * d]
        }));
        Pose { at, look, up }
    }
}

/// The motion (`.mot`) of a flight round `route`, and its frames and
/// frames a second.
pub fn motion(f: &Flight, route: &Route) -> (Vec<u8>, u32, f32) {
    let fps = fps(f.craft);
    let speed = f.speed.clamp(5.0, 400.0);
    let seconds = (route.length / speed).clamp(1.0, LONGEST);
    let frames = (seconds * fps).round().max(2.0) as u32;
    let rotors = rotors(f.craft);
    let keys = 1 + rotors.len() as u32;
    let mut out = Vec::with_capacity(8 + (frames * keys) as usize * 37 + 256);
    out.extend_from_slice(&frames.to_le_bytes());
    out.extend_from_slice(&keys.to_le_bytes());
    let key = |out: &mut Vec<u8>, frame: u8, look: [f32; 3], up: [f32; 3], at: [f32; 3]| {
        out.push(frame);
        for v in look.iter().chain(&up).chain(&at) {
            out.extend_from_slice(&v.to_le_bytes());
        }
    };
    // Each step round evenly, so the loop closes on itself.
    let real = route.length / (frames as f32 / fps);
    for i in 0..frames {
        let s = route.length * i as f32 / frames as f32;
        let p = route.pose(f.craft, s, real);
        key(&mut out, 1, p.look, p.up, p.at);
        // The blades: turned a little under half a turn each frame, which
        // the eye takes for a spin.
        let a = (i as f32 * 100.0).to_radians();
        let (sn, cs) = a.sin_cos();
        for r in rotors {
            let (look, up) = match r.axis {
                X => ([0.0, sn, cs], [0.0, cs, -sn]),
                Y => ([sn, 0.0, cs], [0.0, 1.0, 0.0]),
                _ => ([0.0, 0.0, 1.0], [sn, cs, 0.0]),
            };
            key(&mut out, r.frame, look, up, r.at);
        }
    }
    // The 256 bytes the game's files end with.
    out.extend_from_slice(&[0u8; 256]);
    (out, frames, fps)
}

/// The files a course's flights add to its track (`.mcf` and `.mot`
/// each), and their `[AnimationN]` sections numbered from `first`.
pub fn files(course: &Course, stem: &str, ground: &dyn Fn(f32, f32) -> f32, first: usize) -> (Vec<(String, Vec<u8>)>, Vec<String>) {
    let mut files = Vec::new();
    let mut sections = Vec::new();
    for (k, f) in course.flights.iter().enumerate() {
        let route = Route::new(path(f, ground));
        if route.length < 10.0 {
            continue;
        }
        let (mot, frames, fps) = motion(f, &route);
        let name = format!("{stem}_fly{}", k + 1);
        let mcf = format!(
            "[General info]\r\nNumberOfMotions=1\r\nContentDirectory=\r\nSLTFile={}\r\n[MOTION 1]\r\nNAME=Fly\r\nVUE={name}.mot\r\nFPS={fps:.0}\r\nFrameCount={frames}\r\nLooping=yes\r\n",
            f.craft.model()
        );
        files.push((format!("{name}.mcf"), mcf.into_bytes()));
        files.push((format!("{name}.mot"), mot));
        let (wav, far, near) = sound(f.craft);
        sections.push(format!(
            "[Animation{}]\r\nMCF={name}.mcf\r\nOffset=0.0, 0.0, 0.0\r\nUseLighting=T\r\nUseBlendedKeys=T\r\nNumberOfMotions=1\r\nMotionToPlay1=Fly\r\nNumberInSequence=1\r\nMotionSequence=1\r\nAutomaticSequence=T\r\nSoundResourceFile={wav}\r\nSoundMaxDistance={far:.1}\r\nSoundMinDistance={near:.1}\r\nSoundLooping=T\r\n\r\n",
            first + sections.len()
        ));
    }
    (files, sections)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mm2_formats::anim::{Mcf, Motion};

    #[test]
    fn a_flight_is_a_looping_motion_the_readers_take() {
        let mut c = Course::new(crate::Mode::National);
        c.flights.push(Flight { craft: Craft::Helicopter, points: vec![[1000.0, 120.0, 1000.0], [2500.0, 200.0, 1200.0], [1800.0, 150.0, 2600.0]], speed: 70.0 });
        c.flights.push(Flight { craft: Craft::Blimp, points: vec![[1900.0, 260.0, 1900.0]], speed: 25.0 });
        let (files, sections) = files(&c, "TEST", &|_, _| 50.0, 1);
        assert_eq!(files.len(), 4);
        assert_eq!(sections.len(), 2);
        let mcf = Mcf::parse(&files[0].1).unwrap();
        assert_eq!(mcf.model, "HeliNoCam.slt");
        assert!(mcf.motions[0].looping);
        let m = Motion::parse(&files[1].1).unwrap();
        assert_eq!(m.frames.len() as u32, mcf.motions[0].frame_count);
        assert_eq!(m.frames[0].len(), 3);
        // Up in the air, and back round to where it began.
        let (a, b) = (m.frames[0][0].position, m.frames.last().unwrap()[0].position);
        assert!(a[1] > 100.0);
        assert!(((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt() < 30.0);
        assert!(sections[1].starts_with("[Animation2]") && sections[1].contains("MCF=TEST_fly2.mcf"));
    }
}
