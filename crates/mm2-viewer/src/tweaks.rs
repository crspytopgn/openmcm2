//! The fork's Additional Tweaks (not the game's): read from the command
//! line the launcher makes, changed in Options' Tweaks box, and saved
//! back to the launcher's `launcher.ini` so the launcher and the next
//! start see them. A change counts from the next race.

use std::sync::{OnceLock, RwLock};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tweaks {
    /// The difficulty table's steps (`--difficulty=`), Easy to Hard; see
    /// [`mm2_settings::DIFFICULTY`].
    pub difficulty: [[u32; 3]; 4],
    /// How much steering in the air bends the flight, % of the fork's
    /// own, 0 none (`--air-steer=N`; `--air-steer` alone is 100).
    pub air_steer: u32,
    /// How much braking in the air shortens the flight, % of the fork's
    /// own, 0 none (`--air-brake=N`; `--air-brake` alone is 100).
    pub air_brake: u32,
    /// G or a pad's Back puts the player back on the bike after a crash
    /// (off with `--no-quick-get-up`).
    pub quick_get_up: bool,
    /// The live gap to the best lap (`--lap-gap`).
    pub lap_gap: bool,
    /// The steadier Chase Cam (`--steady-camera`).
    pub steady_camera: bool,
    /// The Chase Cam's field of view, degrees (`--fov=N`; the game's 85).
    pub fov: u32,
    /// The modernised HUD (off with `--original-race-display`).
    pub modern_hud: bool,
    /// On an ultrawide window, the race display in a 16:9 middle
    /// (`--centred-display`).
    pub centred: bool,
    /// Pad vibration, 0 to 100, or `Some(None)` for off (`--vibration=N`
    /// or `off`); `None` leaves it to the profile's Force Feedback.
    pub vibration: Option<Option<u32>>,
    /// Frames at the screen's refresh (off with `--no-vsync`).
    pub vsync: bool,
    /// The frame rate at the bottom of the screen (`--show-fps`, or F2).
    pub show_fps: bool,
}

/// The field of view's range, degrees.
pub const FOV: (u32, u32) = (60, 120);

impl Tweaks {
    pub fn from_args(args: &[String]) -> Tweaks {
        let has = |f: &str| args.iter().any(|a| a == f);
        let value = |f: &str| args.iter().find_map(|a| a.strip_prefix(f));
        let strength = |f: &str| match value(&format!("{f}=")) {
            Some(n) => n.parse::<u32>().map_or(0, |n| n.min(mm2_settings::AIR_MOST)),
            None => if has(f) { 100 } else { 0 },
        };
        Tweaks {
            difficulty: value("--difficulty=")
                .and_then(mm2_settings::parse_difficulty)
                .unwrap_or(mm2_settings::DIFFICULTY),
            air_steer: strength("--air-steer"),
            air_brake: strength("--air-brake"),
            quick_get_up: !has("--no-quick-get-up"),
            lap_gap: has("--lap-gap"),
            steady_camera: has("--steady-camera"),
            fov: value("--fov=")
                .and_then(|n| n.parse::<u32>().ok())
                .map_or(85, |f| f.clamp(FOV.0, FOV.1)),
            modern_hud: !has("--original-race-display"),
            centred: has("--centred-display"),
            vibration: value("--vibration=").and_then(|v| match v {
                "off" => Some(None),
                n => Some(Some(n.parse::<u32>().ok()?.min(100))),
            }),
            vsync: !has("--no-vsync"),
            show_fps: has("--show-fps"),
        }
    }

    /// Puts these into the launcher's settings.
    fn into_settings(self, s: &mut mm2_settings::Settings) {
        s.difficulty = self.difficulty;
        s.air_steer = self.air_steer;
        s.air_brake = self.air_brake;
        s.quick_get_up = self.quick_get_up;
        s.lap_gap = self.lap_gap;
        s.steady_camera = self.steady_camera;
        s.fov = self.fov;
        s.sharp_display = self.modern_hud;
        s.centred_display = self.centred;
        s.vsync = self.vsync;
        s.show_fps = self.show_fps;
        match self.vibration {
            Some(Some(n)) => {
                s.vibration = true;
                s.vibration_strength = n;
            }
            Some(None) => s.vibration = false,
            None => {}
        }
    }
}

fn now() -> &'static RwLock<Tweaks> {
    static NOW: OnceLock<RwLock<Tweaks>> = OnceLock::new();
    NOW.get_or_init(|| {
        let args: Vec<String> = std::env::args().collect();
        RwLock::new(Tweaks::from_args(&args))
    })
}

/// The tweaks as they are now.
pub fn get() -> Tweaks {
    *now().read().unwrap()
}

/// Changes the tweaks and saves them to `launcher.ini`.
pub fn set(t: Tweaks) {
    *now().write().unwrap() = t;
    let mut s = mm2_settings::Settings::load();
    t.into_settings(&mut s);
    if let Err(e) = s.save() {
        eprintln!("tweaks: couldn't save {}: {e}", mm2_settings::Settings::path().display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_launchers_command_line_round_trips() {
        let mut s = mm2_settings::Settings {
            difficulty: [[4, 6, 8], [3, 5, 7], [9, 6, 5], [3, 5, 10]],
            air_steer: 0,
            air_brake: 150,
            fov: 100,
            sharp_display: false,
            vibration: true,
            vibration_strength: 30,
            vsync: false,
            show_fps: true,
            quick_get_up: false,
            ..Default::default()
        };
        let mut args = vec!["mm2-viewer".to_string()];
        args.extend(s.args());
        let t = Tweaks::from_args(&args);
        assert_eq!(t.difficulty[2][0], 9);
        assert_eq!((t.air_steer, t.air_brake), (0, 150));
        assert!(!t.quick_get_up && !t.modern_hud && !t.vsync && t.show_fps);
        assert_eq!((t.fov, t.vibration), (100, Some(Some(30))));
        let before = s.clone();
        t.into_settings(&mut s);
        assert_eq!(s, before);
    }

    #[test]
    fn without_the_launcher_vibration_is_the_profiles() {
        let t = Tweaks::from_args(&["mm2-viewer".into(), "--vibration=off".into()]);
        assert_eq!(t.vibration, Some(None));
        let t = Tweaks::from_args(&["mm2-viewer".into()]);
        assert_eq!((t.vibration, t.fov, t.modern_hud), (None, 85, true));
        let t = Tweaks::from_args(&["mm2-viewer".into(), "--air-steer".into()]);
        assert_eq!((t.air_steer, t.air_brake), (100, 0));
    }
}
