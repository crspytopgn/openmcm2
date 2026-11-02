//! The launcher's settings, kept in `launcher.ini` beside the port's own
//! saves, and the command line they make for `mm2-viewer`: shared by the
//! launcher, which edits them, and the game, which starts with them when
//! run on its own.

use std::path::PathBuf;

/// The window sizes on offer by shape: its name, what it suits, and the
/// sizes with their names.
pub const SHAPES: [(&str, &str, &[(u32, u32, &str)]); 3] = [
    (
        "4:3 Classic",
        "The original game's shape, for older square monitors.",
        &[(800, 600, "SVGA"), (1024, 768, "XGA"), (1280, 960, ""), (1600, 1200, "UXGA")],
    ),
    (
        "16:9 Widescreen",
        "Most monitors, laptops and TVs.",
        &[
            (1280, 720, "HD 720p"),
            (1600, 900, "HD+"),
            (1920, 1080, "Full HD 1080p"),
            (2560, 1440, "QHD 1440p"),
            (3840, 2160, "4K UHD"),
        ],
    ),
    (
        "21:9 Ultrawide",
        "Ultrawide monitors: the race shows more to the sides.",
        &[(2560, 1080, "UW-FHD"), (3440, 1440, "UW-QHD"), (3840, 1600, "UW-QHD+")],
    ),
];

/// Whether a window `size` fits on `monitor` with room to spare: narrower,
/// and short enough to leave the title bar and taskbar showing.
pub fn fits((w, h): (u32, u32), monitor: Option<(u32, u32)>) -> bool {
    monitor.is_none_or(|m| w < m.0 && h <= m.1 - m.1 / 12)
}

/// The folder the port keeps its saves in (the same as `mm2-viewer`'s),
/// where the launcher puts the copied game and its settings too.
pub fn user_dir() -> PathBuf {
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    if let Some(dir) = env("MM2_USER_DIR") {
        return dir;
    }
    if cfg!(target_os = "macos") {
        if let Some(home) = env("HOME") {
            return home.join("Library/Application Support/MM2");
        }
    } else if cfg!(windows) {
        if let Some(app) = env("APPDATA") {
            return app.join("MM2");
        }
    } else if let Some(data) = env("XDG_DATA_HOME") {
        return data.join("mm2");
    } else if let Some(home) = env("HOME") {
        return home.join(".local/share/mm2");
    }
    PathBuf::from("mm2-user")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Low,
    Medium,
    High,
    Ultra,
}

impl Quality {
    pub const ALL: [Quality; 4] = [Quality::Low, Quality::Medium, Quality::High, Quality::Ultra];
    pub fn name(self) -> &'static str {
        ["low", "medium", "high", "ultra"][self as usize]
    }

    /// Whether the upgrade `name` is on with this preset unless changed,
    /// as `--enhanced=quality=` sets it.
    pub fn has(self, name: &str) -> bool {
        let off: &[&str] = match self {
            Quality::Low => &["ao", "shafts", "smooth", "cinematic", "grass"],
            Quality::Medium => &["shafts", "smooth"],
            _ => &[],
        };
        !off.contains(&name) && UPGRADES.iter().any(|u| u.0 == name && u.2)
    }
}

/// The enhanced look's switches the launcher shows, each with the name
/// `--enhanced` knows it by, what it is, and whether it's on unless
/// turned off.
/// The difficulty table: by Easy, Normal and Hard, a step from 1 to 10
/// for each of [`DIFFICULTY_STEPS`]' rows, higher always harder. The
/// game's own are steps 3, 5 and 7.
pub const DIFFICULTY: [[u32; 3]; 4] = [[3, 5, 7]; 4];

/// The most air steering and braking go to, % of the fork's own.
pub const AIR_MOST: u32 = 200;

/// An air steering or braking strength as `launcher.ini` holds it: a %
/// (a multiple of 10), or from before it was one, on (`1`) or off.
fn air(v: &str) -> u32 {
    match v {
        "1" | "true" => 100,
        _ => v.parse::<u32>().map_or(0, |n| n.min(AIR_MOST)),
    }
}

/// The most a difficulty step goes to.
pub const MOST_STEP: u32 = 10;

/// What each difficulty step, 1 to 10, stands for: the computer riders'
/// engine power, how easily they crash (and how often they try a trick in
/// a race) and how big a hit the player takes before crashing, each as a %
/// of the game's Normal, and the computer riders' wait between tricks in
/// the Stunt Quarry (s). Steps 3, 5 and 7 are the game's Easy, Normal and
/// Hard, rounded; 8 to 10 climb steeply, so 10 is far past Hard.
pub const DIFFICULTY_STEPS: [[u32; 10]; 4] = [
    [70, 77, 85, 92, 100, 107, 115, 135, 160, 180],
    [150, 130, 115, 107, 100, 88, 77, 55, 35, 20],
    [130, 120, 110, 105, 100, 97, 95, 80, 65, 50],
    [35, 30, 25, 20, 15, 10, 5, 3, 1, 0],
];

/// What `row`'s difficulty `step` stands for (see [`DIFFICULTY_STEPS`]).
pub fn difficulty_value(row: usize, step: u32) -> u32 {
    DIFFICULTY_STEPS[row.min(3)][step.clamp(1, MOST_STEP) as usize - 1]
}

/// The difficulty table as `launcher.ini` and `--difficulty=` write it:
/// its twelve steps, row by row.
pub fn difficulty_text(t: &[[u32; 3]; 4]) -> String {
    t.iter().flatten().map(u32::to_string).collect::<Vec<_>>().join(",")
}

/// A difficulty table from [`difficulty_text`]'s numbers, if all twelve
/// are there and are steps (an older table of percentages is not).
pub fn parse_difficulty(text: &str) -> Option<[[u32; 3]; 4]> {
    let n: Vec<u32> = text.split(',').map(|v| v.trim().parse().ok()).collect::<Option<_>>()?;
    let mut t = [[0; 3]; 4];
    for (i, v) in t.iter_mut().flatten().enumerate() {
        *v = *n.get(i)?;
    }
    (n.len() == 12 && n.iter().all(|v| (1..=MOST_STEP).contains(v))).then_some(t)
}

pub const UPGRADES: [(&str, &str, bool); 16] = [
    ("lighting", "Per-pixel lighting", true),
    ("shadows", "Sun shadows", true),
    ("distance", "Far draw distance and haze", true),
    ("bloom", "HDR bloom", true),
    ("ao", "Ambient occlusion", true),
    ("ground", "Detailed ground", true),
    ("grass", "3D grass", true),
    ("smooth", "Smooth hills", true),
    ("dust", "Dust clouds", true),
    ("mud", "Mud and spray", true),
    ("bikes", "Glossier bikes", true),
    ("shafts", "Light shafts", true),
    ("cinematic", "Replay blur and depth of field", false),
    ("ruts", "Lasting tyre ruts", false),
    ("scenery", "Trackside props and crowd", false),
    ("grade", "Colour grade", false),
];

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// The game folder the port reads.
    pub game: Option<PathBuf>,
    pub hires: Option<PathBuf>,
    pub use_hires: bool,
    pub enhanced: bool,
    pub quality: Quality,
    /// `time=`: none keeps the track's own sky.
    pub time: String,
    pub weather: String,
    pub detail: f32,
    /// Each of [`UPGRADES`], on or off.
    pub upgrades: Vec<bool>,
    pub fullscreen: bool,
    pub window: (u32, u32),
    pub vsync: bool,
    /// On a screen wider than 16:9, the race display in a 16:9 middle
    /// (`--centred-display`).
    pub centred_display: bool,
    pub show_fps: bool,
    /// The difficulty table (`--difficulty=`); see [`DIFFICULTY`].
    pub difficulty: [[u32; 3]; 4],
    /// Pad vibration from the riding and its strength, 0 to 100
    /// (`--vibration=N`, or `off`).
    pub vibration: bool,
    pub vibration_strength: u32,
    /// The modernised HUD (off: `--original-race-display`).
    pub sharp_display: bool,
    /// How much steering in the air bends the flight, % of the fork's
    /// own, 0 the game's none (`--air-steer=N`); see [`AIR_MOST`].
    pub air_steer: u32,
    /// How much braking in the air shortens the flight, % of the fork's
    /// own, 0 the game's none (`--air-brake=N`).
    pub air_brake: u32,
    /// G or a pad's Back puts the player back on the bike after a crash
    /// (off: `--no-quick-get-up`).
    pub quick_get_up: bool,
    /// The live gap to the best lap (`--lap-gap`).
    pub lap_gap: bool,
    /// The steadier camera (`--steady-camera`).
    pub steady_camera: bool,
    /// The Chase Cam's field of view, degrees (`--fov=N`; the game's 85).
    pub fov: u32,
    /// Look for a newer OpenMCM2 when the launcher opens.
    pub check_updates: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            game: None,
            hires: None,
            use_hires: true,
            enhanced: false,
            quality: Quality::High,
            time: String::new(),
            weather: "clear".into(),
            detail: 0.5,
            upgrades: UPGRADES.iter().map(|u| u.2).collect(),
            fullscreen: false,
            window: (1024, 768),
            vsync: true,
            centred_display: false,
            show_fps: false,
            difficulty: DIFFICULTY,
            vibration: true,
            vibration_strength: 50,
            sharp_display: true,
            air_steer: 100,
            air_brake: 100,
            quick_get_up: true,
            lap_gap: true,
            steady_camera: true,
            fov: 85,
            check_updates: true,
        }
    }
}

impl Settings {
    pub fn path() -> PathBuf {
        user_dir().join("launcher.ini")
    }

    pub fn load() -> Settings {
        std::fs::read_to_string(Self::path()).map(|t| Self::parse(&t)).unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(user_dir())?;
        std::fs::write(Self::path(), self.text())
    }

    pub fn parse(text: &str) -> Settings {
        let mut s = Settings::default();
        let yes = |v: &str| v == "1" || v == "true";
        for (k, v) in text.lines().filter_map(|l| l.split_once('=')) {
            let v = v.trim();
            match k.trim() {
                "game" if !v.is_empty() => s.game = Some(v.into()),
                "hires" if !v.is_empty() => s.hires = Some(v.into()),
                "use_hires" => s.use_hires = yes(v),
                "enhanced" => s.enhanced = yes(v),
                "quality" => {
                    s.quality = Quality::ALL.into_iter().find(|q| q.name() == v).unwrap_or(Quality::High)
                }
                "time" => s.time = v.into(),
                "weather" => s.weather = v.into(),
                "detail" => s.detail = v.parse::<f32>().unwrap_or(0.5).clamp(0.0, 1.0),
                "fullscreen" => s.fullscreen = yes(v),
                "window" => {
                    if let Some((w, h)) = v.split_once('x') {
                        if let (Ok(w), Ok(h)) = (w.parse(), h.parse()) {
                            s.window = (w, h);
                        }
                    }
                }
                "vsync" => s.vsync = yes(v),
                "centred_display" => s.centred_display = yes(v),
                "show_fps" => s.show_fps = yes(v),
                "difficulty" => s.difficulty = parse_difficulty(v).unwrap_or(DIFFICULTY),
                "vibration" => s.vibration = yes(v),
                "sharp_display" => s.sharp_display = yes(v),
                "air_steer" => s.air_steer = air(v),
                "air_brake" => s.air_brake = air(v),
                "quick_get_up" => s.quick_get_up = yes(v),
                "lap_gap" => s.lap_gap = yes(v),
                "steady_camera" => s.steady_camera = yes(v),
                "fov" => s.fov = v.parse::<u32>().unwrap_or(85).clamp(60, 120),
                "vibration_strength" => s.vibration_strength = v.parse::<u32>().unwrap_or(50).min(100),
                "check_updates" => s.check_updates = yes(v),
                // Files from before version 2 kept the replay blur on by
                // default; it starts off now, so their setting is dropped.
                "upgrade.cinematic" if !text.lines().any(|l| l.trim() == "version=2") => {}
                k => {
                    if let Some(i) = UPGRADES.iter().position(|u| k == format!("upgrade.{}", u.0)) {
                        s.upgrades[i] = yes(v);
                    }
                }
            }
        }
        s
    }

    pub fn text(&self) -> String {
        let p = |o: &Option<PathBuf>| o.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        let b = |v: bool| if v { "1" } else { "0" };
        let mut t = format!(
            "version=2\ngame={}\nhires={}\nuse_hires={}\nenhanced={}\nquality={}\ntime={}\nweather={}\ndetail={}\nfullscreen={}\nwindow={}x{}\nvsync={}\ncentred_display={}\nshow_fps={}\ndifficulty={}\nvibration={}\nvibration_strength={}\nsharp_display={}\nair_steer={}\nair_brake={}\nquick_get_up={}\nlap_gap={}\nsteady_camera={}\nfov={}\ncheck_updates={}\n",
            p(&self.game),
            p(&self.hires),
            b(self.use_hires),
            b(self.enhanced),
            self.quality.name(),
            self.time,
            self.weather,
            self.detail,
            b(self.fullscreen),
            self.window.0,
            self.window.1,
            b(self.vsync),
            b(self.centred_display),
            b(self.show_fps),
            difficulty_text(&self.difficulty),
            b(self.vibration),
            self.vibration_strength,
            b(self.sharp_display),
            self.air_steer,
            self.air_brake,
            b(self.quick_get_up),
            b(self.lap_gap),
            b(self.steady_camera),
            self.fov,
            b(self.check_updates),
        );
        for (u, on) in UPGRADES.iter().zip(&self.upgrades) {
            t += &format!("upgrade.{}={}\n", u.0, b(*on));
        }
        t
    }

    /// `mm2-viewer`'s arguments for these settings.
    pub fn args(&self) -> Vec<String> {
        let mut a = Vec::new();
        if let Some(g) = &self.game {
            a.push(g.display().to_string());
        }
        if let (true, Some(h)) = (self.use_hires, &self.hires) {
            a.push("--hires".into());
            a.push(h.display().to_string());
        }
        if self.enhanced {
            let mut list = vec![format!("quality={}", self.quality.name())];
            // Only what differs from the preset, after it so a switch wins.
            for (u, on) in UPGRADES.iter().zip(&self.upgrades) {
                if *on != self.quality.has(u.0) {
                    list.push(if *on { u.0.to_string() } else { format!("no-{}", u.0) });
                }
            }
            if !self.time.is_empty() {
                list.push(format!("time={}", self.time));
            }
            if self.weather != "clear" {
                list.push(format!("weather={}", self.weather));
            }
            if (self.detail - 0.5).abs() > 1e-3 {
                list.push(format!("detail={:.2}", self.detail));
            }
            a.push(format!("--enhanced={}", list.join(",")));
        }
        if self.fullscreen {
            a.push("--fullscreen".into());
        } else {
            a.push("--window".into());
            a.push(format!("{}x{}", self.window.0, self.window.1));
        }
        if !self.vsync {
            a.push("--no-vsync".into());
        }
        if self.centred_display {
            a.push("--centred-display".into());
        }
        if self.show_fps {
            a.push("--show-fps".into());
        }
        if self.difficulty != DIFFICULTY {
            a.push(format!("--difficulty={}", difficulty_text(&self.difficulty)));
        }
        if self.air_steer > 0 {
            a.push(format!("--air-steer={}", self.air_steer));
        }
        if self.air_brake > 0 {
            a.push(format!("--air-brake={}", self.air_brake));
        }
        if !self.quick_get_up {
            a.push("--no-quick-get-up".into());
        }
        if self.lap_gap {
            a.push("--lap-gap".into());
        }
        if self.steady_camera {
            a.push("--steady-camera".into());
        }
        if self.fov != 85 {
            a.push(format!("--fov={}", self.fov));
        }
        if !self.sharp_display {
            a.push("--original-race-display".into());
        }
        a.push(match self.vibration {
            true => format!("--vibration={}", self.vibration_strength),
            false => "--vibration=off".into(),
        });
        a
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_broken_difficulty_table_is_the_games() {
        assert_eq!(Settings::parse("difficulty=1,2,3\n").difficulty, DIFFICULTY);
        assert_eq!(parse_difficulty(&difficulty_text(&DIFFICULTY)), Some(DIFFICULTY));
        // The percentages the table held before it had steps.
        let old = "difficulty=85,100,115,115,100,77,110,100,95,25,15,5\n";
        assert_eq!(Settings::parse(old).difficulty, DIFFICULTY);
        // Air steering and braking from when they were on or off.
        let old = Settings::parse("air_steer=1\nair_brake=0\n");
        assert_eq!((old.air_steer, old.air_brake), (100, 0));
    }

    #[test]
    fn round_trip() {
        let mut s = Settings::default();
        s.game = Some("/x/GAME".into());
        s.enhanced = true;
        s.upgrades[1] = false;
        s.upgrades[13] = true;
        s.time = "dusk".into();
        s.window = (1920, 1080);
        s.centred_display = true;
        s.difficulty[2][0] = 9;
        s.air_brake = 150;
        s.quick_get_up = false;
        s.vibration_strength = 80;
        s.steady_camera = false;
        s.fov = 100;
        s.check_updates = false;
        assert_eq!(Settings::parse(&s.text()), s);
        assert_eq!(
            s.args(),
            [
                "/x/GAME",
                "--enhanced=quality=high,no-shadows,ruts,time=dusk",
                "--window",
                "1920x1080",
                "--centred-display",
                "--difficulty=3,5,7,3,5,7,9,5,7,3,5,7",
                "--air-steer=100",
                "--air-brake=150",
                "--no-quick-get-up",
                "--lap-gap",
                "--fov=100",
                "--vibration=80"
            ]
        );
    }
}
