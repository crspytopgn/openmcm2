//! The player's choices, kept in `launcher.ini` beside the port's own
//! saves, and the command line they make for `mm2-viewer`.

use std::path::PathBuf;

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
    ("cinematic", "Replay blur and depth of field", true),
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
    pub show_fps: bool,
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
            show_fps: false,
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
                "show_fps" => s.show_fps = yes(v),
                "check_updates" => s.check_updates = yes(v),
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
            "game={}\nhires={}\nuse_hires={}\nenhanced={}\nquality={}\ntime={}\nweather={}\ndetail={}\nfullscreen={}\nwindow={}x{}\nvsync={}\nshow_fps={}\ncheck_updates={}\n",
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
            b(self.show_fps),
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
        if self.show_fps {
            a.push("--show-fps".into());
        }
        a
    }
}

/// `mm2-viewer` beside the launcher (as the release zips and the Mac app
/// put it), or in the cargo target folder when run from a checkout.
pub fn viewer() -> Option<PathBuf> {
    let name = format!("mm2-viewer{}", std::env::consts::EXE_SUFFIX);
    let exe = std::env::current_exe().ok()?;
    let here = exe.parent()?;
    [here.join(&name), here.join("../Resources").join(&name)]
        .into_iter()
        .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut s = Settings::default();
        s.game = Some("/x/GAME".into());
        s.enhanced = true;
        s.upgrades[1] = false;
        s.upgrades[13] = true;
        s.time = "dusk".into();
        s.window = (1920, 1080);
        s.check_updates = false;
        assert_eq!(Settings::parse(&s.text()), s);
        assert_eq!(
            s.args(),
            [
                "/x/GAME",
                "--enhanced=quality=high,no-shadows,ruts,time=dusk",
                "--window",
                "1920x1080"
            ]
        );
    }
}
