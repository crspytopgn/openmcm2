//! The replay editor (an extra, `--replay-editor`): over the game's own
//! VCR player, a free camera, slow motion, In and Out marks to loop a
//! moment, camera keys the camera glides between, and clips saved as
//! movies (a desktop's `Clips` folder, a browser's downloads).
//!
//! The VCR player itself is the original's and works as it did; the
//! editor only adds its keys and a line of its own at the top.

use crate::camera::Camera;
use glam::Vec3;
use web_time::Instant;

/// The slow motion steps, as parts of the tape's own speed.
pub const SLOW: [f32; 5] = [1.0, 0.5, 0.25, 0.125, 0.05];
/// The free camera's walk and run, units (feet) a second, and how fast
/// the arrow keys turn it, radians a second.
pub const FLY: (f32, f32, f32) = (60.0, 260.0, 1.4);
/// Frames a second a clip is made at.
pub const CLIP_FPS: u32 = 30;
/// How long a note stays up, seconds.
const NOTE_SECS: f32 = 4.0;

/// Whose view the replay is seen from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// The race's cameras, as the VCR player shows them.
    Race,
    /// Flown with the keys, mouse or pad.
    Free,
    /// Gliding between the camera keys.
    Path,
}

impl View {
    pub fn name(self) -> &'static str {
        match self {
            View::Race => "Race camera",
            View::Free => "Free camera",
            View::Path => "Camera path",
        }
    }
}

/// A camera key: where the camera is at a time along the tape.
#[derive(Debug, Clone, Copy)]
pub struct Key {
    pub time: f32,
    pub camera: Camera,
}

pub struct Editor {
    pub view: View,
    /// Which of [`SLOW`].
    pub slow: usize,
    pub mark_in: Option<f32>,
    pub mark_out: Option<f32>,
    /// Plays In to Out over and over.
    pub looping: bool,
    /// The free camera turns to keep the rider followed in the picture.
    pub aim: bool,
    /// Camera keys, in time order.
    pub keys: Vec<Key>,
    /// The VCR player and the editor's line hidden, for a clean picture.
    pub hidden: bool,
    /// A clip is being made.
    pub recording: bool,
    note: Option<(String, Instant)>,
}

impl Default for Editor {
    fn default() -> Self {
        Editor::new()
    }
}

impl Editor {
    pub fn new() -> Editor {
        Editor {
            view: View::Race,
            slow: 0,
            mark_in: None,
            mark_out: None,
            looping: false,
            aim: false,
            keys: Vec::new(),
            hidden: false,
            recording: false,
            note: Some((HELP.to_string(), Instant::now())),
        }
    }

    pub fn speed(&self) -> f32 {
        SLOW[self.slow.min(SLOW.len() - 1)]
    }

    pub fn say(&mut self, text: impl Into<String>) {
        self.note = Some((text.into(), Instant::now()));
    }

    /// The race camera, the free camera, then the camera path (once it has
    /// two keys to go between), round again.
    pub fn next_view(&mut self) -> View {
        self.view = match self.view {
            View::Race => View::Free,
            View::Free if self.keys.len() >= 2 => View::Path,
            View::Free if self.keys.len() == 1 => {
                self.say("Add a second camera key (K) for the camera path");
                View::Race
            }
            _ => View::Race,
        };
        self.view
    }

    /// Slower (`by` -1) or back towards full speed (+1).
    pub fn step_slow(&mut self, by: i32) {
        self.slow = (self.slow as i32 - by).clamp(0, SLOW.len() as i32 - 1) as usize;
        let said = speed_text(self.speed());
        self.say(if self.slow == 0 { "Full speed".to_string() } else { format!("Slow motion {said}") });
    }

    pub fn set_in(&mut self, time: f32) {
        self.mark_in = Some(time);
        if self.mark_out.is_some_and(|o| o <= time) {
            self.mark_out = None;
        }
        self.say(format!("In at {}", short_time(time)));
    }

    pub fn set_out(&mut self, time: f32) {
        self.mark_out = Some(time);
        if self.mark_in.is_some_and(|i| i >= time) {
            self.mark_in = None;
        }
        self.say(format!("Out at {}", short_time(time)));
    }

    pub fn flip_loop(&mut self) {
        self.looping = !self.looping;
        self.say(if self.looping { "Looping In to Out" } else { "Loop off" });
    }

    pub fn flip_aim(&mut self) {
        self.aim = !self.aim;
        self.say(if self.aim { "Free camera keeps the rider in view" } else { "Free camera looks where you point it" });
    }

    /// Keeps `camera` as the key at `time`, in place of one very close.
    pub fn add_key(&mut self, time: f32, camera: Camera) {
        self.keys.retain(|k| (k.time - time).abs() > 0.05);
        let at = self.keys.partition_point(|k| k.time < time);
        self.keys.insert(at, Key { time, camera });
        self.say(format!("Camera key {} at {}", at + 1, short_time(time)));
    }

    /// Takes away the key nearest `time`.
    pub fn remove_key(&mut self, time: f32) {
        let nearest = (0..self.keys.len()).min_by(|&a, &b| {
            (self.keys[a].time - time).abs().total_cmp(&(self.keys[b].time - time).abs())
        });
        match nearest {
            Some(i) => {
                let k = self.keys.remove(i);
                self.say(format!("Camera key at {} taken away", short_time(k.time)));
                if self.view == View::Path && self.keys.len() < 2 {
                    self.view = View::Free;
                }
            }
            None => self.say("No camera keys yet"),
        }
    }

    /// Where the camera path has the camera at `time`: a smooth curve
    /// through the keys, held at the first before it and the last after.
    pub fn path_at(&self, time: f32) -> Option<Camera> {
        path_at(&self.keys, time)
    }

    /// Where the tape goes after moving on: back to In from Out when
    /// looping.
    pub fn wrap(&self, time: f32, rate: f32) -> Option<f32> {
        let (i, o) = (self.mark_in?, self.mark_out?);
        (self.looping && !self.recording).then_some(())?;
        if rate > 0.0 && time >= o {
            Some(i)
        } else if rate < 0.0 && time <= i {
            Some(o)
        } else {
            None
        }
    }

    /// The editor's line at the top, and a note under it a few seconds.
    pub fn lines(&self) -> Vec<String> {
        let mut top = format!("Replay editor  |  {}", self.view.name());
        if self.view == View::Free && self.aim {
            top += " (aimed at the rider)";
        }
        top += &format!("  |  {}", if self.slow == 0 { "Full speed".to_string() } else { format!("Slow motion {}", speed_text(self.speed())) });
        match (self.mark_in, self.mark_out) {
            (None, None) => {}
            (i, o) => {
                let t = |m: Option<f32>| m.map_or("-".to_string(), short_time);
                top += &format!("  |  In {}  Out {}{}", t(i), t(o), if self.looping { "  Loop" } else { "" });
            }
        }
        if !self.keys.is_empty() {
            top += &format!("  |  {} camera key{}", self.keys.len(), if self.keys.len() == 1 { "" } else { "s" });
        }
        let mut lines = vec![top];
        if let Some((text, at)) = &self.note {
            if at.elapsed().as_secs_f32() < NOTE_SECS {
                lines.push(text.clone());
            }
        }
        lines
    }

    /// The help line again, for a while.
    pub fn help(&mut self) {
        self.say(HELP);
    }
}

/// What the keys do, shown as the editor opens and on ?.
pub const HELP: &str = "Tab camera  |  , . slow motion  |  I O marks, L loop  |  K camera key, Backspace takes one away  |  T aim at rider  |  R record a clip  |  H hide  |  ? help";

/// `0:12.3`.
pub fn short_time(t: f32) -> String {
    let t = t.max(0.0);
    let m = (t / 60.0) as u32;
    format!("{m}:{:04.1}", t - m as f32 * 60.0)
}

/// `1/2x`, `1/20x`.
fn speed_text(s: f32) -> String {
    if s >= 1.0 {
        "1x".into()
    } else {
        format!("1/{}x", (1.0 / s).round() as u32)
    }
}

fn path_at(keys: &[Key], time: f32) -> Option<Camera> {
    let first = keys.first()?;
    let last = keys.last()?;
    if keys.len() == 1 || time <= first.time {
        return Some(first.camera);
    }
    if time >= last.time {
        return Some(last.camera);
    }
    let i = keys.partition_point(|k| k.time <= time).clamp(1, keys.len() - 1);
    let (a, b) = (&keys[i - 1], &keys[i]);
    let p0 = &keys[i.saturating_sub(2)];
    let p3 = &keys[(i + 1).min(keys.len() - 1)];
    let f = ((time - a.time) / (b.time - a.time).max(1e-4)).clamp(0.0, 1.0);
    // Turns go the short way round from the key before.
    let yaw = |k: &Key, from: f32| from + wrap_angle(k.camera.yaw - from);
    let ya = a.camera.yaw;
    let y0 = yaw(p0, ya);
    let yb = yaw(b, ya);
    let y3 = yaw(p3, yb);
    let pos = catmull(p0.camera.position, a.camera.position, b.camera.position, p3.camera.position, f);
    let one = |v0: f32, v1: f32, v2: f32, v3: f32| catmull(Vec3::splat(v0), Vec3::splat(v1), Vec3::splat(v2), Vec3::splat(v3), f).x;
    Some(Camera {
        position: pos,
        yaw: one(y0, ya, yb, y3),
        pitch: one(p0.camera.pitch, a.camera.pitch, b.camera.pitch, p3.camera.pitch).clamp(-1.55, 1.55),
        fov: a.camera.fov + (b.camera.fov - a.camera.fov) * f,
        roll: a.camera.roll + (b.camera.roll - a.camera.roll) * f,
    })
}

fn wrap_angle(a: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    (a + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI
}

/// The Catmull-Rom curve through `b` and `c` at `f`.
fn catmull(a: Vec3, b: Vec3, c: Vec3, d: Vec3, f: f32) -> Vec3 {
    let (f2, f3) = (f * f, f * f * f);
    0.5 * ((2.0 * b) + (c - a) * f + (2.0 * a - 5.0 * b + 4.0 * c - d) * f2 + (3.0 * b - a - 3.0 * c + d) * f3)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cam(x: f32, yaw: f32) -> Camera {
        Camera { position: Vec3::new(x, 10.0, 0.0), yaw, pitch: 0.0, fov: 85.0, roll: 0.0 }
    }

    #[test]
    fn the_path_passes_through_its_keys() {
        let mut e = Editor::new();
        e.add_key(2.0, cam(100.0, 0.0));
        e.add_key(0.0, cam(0.0, 0.0));
        e.add_key(4.0, cam(300.0, 0.0));
        assert_eq!(e.keys.iter().map(|k| k.time).collect::<Vec<_>>(), [0.0, 2.0, 4.0]);
        for k in e.keys.clone() {
            assert!((e.path_at(k.time).unwrap().position - k.camera.position).length() < 1e-3);
        }
        let mid = e.path_at(1.0).unwrap().position.x;
        assert!(mid > 0.0 && mid < 100.0, "{mid}");
        // Held before the first and after the last.
        assert_eq!(e.path_at(-5.0).unwrap().position.x, 0.0);
        assert_eq!(e.path_at(9.0).unwrap().position.x, 300.0);
    }

    #[test]
    fn the_path_turns_the_short_way() {
        let mut e = Editor::new();
        e.add_key(0.0, cam(0.0, 3.0));
        e.add_key(1.0, cam(10.0, -3.0));
        let half = e.path_at(0.5).unwrap().yaw;
        // From 3.0 to -3.0 the short way is past pi, not through 0.
        assert!(wrap_angle(half - std::f32::consts::PI).abs() < 0.2, "{half}");
    }

    #[test]
    fn a_key_close_to_another_takes_its_place() {
        let mut e = Editor::new();
        e.add_key(1.0, cam(0.0, 0.0));
        e.add_key(1.02, cam(5.0, 0.0));
        assert_eq!(e.keys.len(), 1);
        assert_eq!(e.keys[0].camera.position.x, 5.0);
        e.remove_key(0.0);
        assert!(e.keys.is_empty());
    }

    #[test]
    fn looping_goes_back_to_in() {
        let mut e = Editor::new();
        e.set_in(5.0);
        e.set_out(8.0);
        assert_eq!(e.wrap(9.0, 1.0), None);
        e.flip_loop();
        assert_eq!(e.wrap(9.0, 1.0), Some(5.0));
        assert_eq!(e.wrap(6.0, 1.0), None);
        assert_eq!(e.wrap(4.0, -1.0), Some(8.0));
        // Out before In drops In.
        e.set_out(3.0);
        assert_eq!(e.mark_in, None);
    }

    #[test]
    fn slow_motion_steps_stop_at_the_ends() {
        let mut e = Editor::new();
        e.step_slow(1);
        assert_eq!(e.speed(), 1.0);
        for _ in 0..10 {
            e.step_slow(-1);
        }
        assert_eq!(e.speed(), 0.05);
        e.step_slow(1);
        assert_eq!(e.speed(), 0.125);
        assert_eq!(speed_text(0.125), "1/8x");
    }

    #[test]
    fn the_views_go_round() {
        let mut e = Editor::new();
        assert_eq!(e.next_view(), View::Free);
        assert_eq!(e.next_view(), View::Race);
        e.add_key(0.0, cam(0.0, 0.0));
        e.add_key(1.0, cam(1.0, 0.0));
        e.next_view();
        assert_eq!(e.next_view(), View::Path);
        assert_eq!(e.next_view(), View::Race);
    }
}
