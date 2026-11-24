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

/// "1920x1080" as (1920, 1080).
pub fn parse_size(v: &str) -> Option<(u32, u32)> {
    let (w, h) = v.trim().split_once('x')?;
    Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
}

/// Which of [`SHAPES`] a `width × height` is nearest.
pub fn shape_of((w, h): (u32, u32)) -> usize {
    match w as f32 / h.max(1) as f32 {
        r if r < 1.55 => 0,
        r if r < 2.1 => 1,
        _ => 2,
    }
}

/// The biggest of `sizes` that fits as a window on `monitor`, or the
/// second smallest when it isn't known; none when none fit.
pub fn fitting(sizes: &[(u32, u32, &str)], monitor: Option<(u32, u32)>) -> Option<(u32, u32)> {
    let best = match monitor {
        Some(_) => sizes.iter().filter(|z| fits((z.0, z.1), monitor)).last(),
        None => sizes.get(1),
    };
    best.map(|z| (z.0, z.1))
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
    /// High's look with ray tracing, at 2x anti-aliasing.
    RayTraced,
    Ultra,
    /// Ultra drawn finer for the strongest cards: supersampled, with the
    /// detail reaching further.
    Max,
}

/// The anti-aliasing the launcher offers, one of them: its name, what it
/// says, and what it tells `--enhanced`. MSAA smooths the models' and
/// the ground's edges; TAA blends each frame, drawn a fraction of a pixel
/// aside, with the ones before, smoothing those and the fine textures
/// (billboards, leaves, fences) too, in place of MSAA.
pub const ANTIALIASING: [(&str, &str, &str); 5] = [
    ("off", "Off", "msaa=1"),
    ("msaa2", "MSAA 2×", "msaa=2"),
    ("msaa4", "MSAA 4×", "msaa=4"),
    ("msaa8", "MSAA 8×", "msaa=8"),
    ("taa", "TAA (temporal)", "msaa=1,taa"),
];

/// A scene resolution as saved: `auto`, or 50% to 100%.
pub fn parse_resolution(v: &str) -> Option<String> {
    match v.strip_suffix('%').and_then(|p| p.parse::<u32>().ok()) {
        Some(p) if (50..=100).contains(&p) => Some(format!("{p}%")),
        _ => (v == "auto").then(|| "auto".into()),
    }
}

/// The size the 3D scene is drawn at, as `--enhanced=resolution=` knows
/// it, and what the launcher says: the preset's own (`auto`: no more than
/// its pixels, [`Quality::most_pixels`]), or a share of the screen. Drawn
/// smaller than the screen, it is stretched over it, the same shape; the
/// menus and race display stay sharp.
/// The times of day (`time=`), with their names: none keeps the track's
/// own sky. The launcher and the game's event page offer the same.
pub const TIMES: [(&str, &str); 7] = [
    ("", "Track's own"),
    ("dawn", "Dawn"),
    ("morning", "Morning"),
    ("noon", "Noon"),
    ("afternoon", "Afternoon"),
    ("dusk", "Dusk"),
    ("night", "Night"),
];
/// The weathers (`weather=`), with their names.
pub const WEATHERS: [(&str, &str); 3] = [("clear", "Clear"), ("overcast", "Overcast"), ("rain", "Rain")];

pub const RESOLUTIONS: [(&str, &str); 6] = [
    ("auto", "Auto"),
    ("50%", "50%"),
    ("67%", "67%"),
    ("75%", "75%"),
    ("85%", "85%"),
    ("100%", "100% (the screen's own)"),
];

impl Quality {
    pub const ALL: [Quality; 6] = [Quality::Low, Quality::Medium, Quality::High, Quality::RayTraced, Quality::Ultra, Quality::Max];
    pub fn name(self) -> &'static str {
        ["low", "medium", "high", "raytraced", "ultra", "max"][self as usize]
    }

    /// What the launcher calls it.
    pub fn label(self) -> &'static str {
        ["Low", "Medium", "High", "Ray traced", "Ultra", "Max"][self as usize]
    }

    /// The anti-aliasing this preset draws with ([`ANTIALIASING`]).
    pub fn antialiasing(self) -> &'static str {
        ["off", "taa", "msaa4", "msaa2", "msaa8", "msaa4"][self as usize]
    }

    /// The most pixels this preset draws the scene with on Auto, as the
    /// launcher says it.
    pub fn most_pixels(self) -> &'static str {
        ["up to 1280×720 worth", "up to 1920×1080 worth", "up to 2560×1440 worth", "up to 2560×1440 worth", "the screen's own", "up to 3840×2160 worth, shrunk to the screen"][self as usize]
    }

    /// Whether the upgrade `name` is on with this preset unless changed,
    /// as `--enhanced=quality=` sets it.
    pub fn has(self, name: &str) -> bool {
        let off: &[&str] = match self {
            Quality::Low => &["ao", "shafts", "smooth", "cinematic", "grass", "gtao", "contact", "ssr", "bounce"],
            Quality::Medium => &["shafts", "smooth", "gtao", "ssr", "bounce"],
            _ => &[],
        };
        // On with this preset though off unless asked. The trackside
        // props and crowd and the colour grade are in none: a matter of
        // taste, left to the player.
        let on: &[&str] = match self {
            Quality::RayTraced => &["raytracing"],
            Quality::Ultra => &["ruts"],
            Quality::Max => &["supersample", "far"],
            _ => &[],
        };
        if on.contains(&name) {
            return true;
        }
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

pub const UPGRADES: [(&str, &str, bool); 31] = [
    ("lighting", "Per-pixel lighting", true),
    ("shadows", "Sun shadows", true),
    ("distance", "Far draw distance and haze", true),
    ("bloom", "HDR bloom", false),
    ("ao", "Ambient occlusion", true),
    ("ground", "Detailed ground", true),
    ("grass", "3D grass", true),
    ("smooth", "Smooth hills", true),
    ("dust", "Dust clouds", true),
    ("mud", "Mud and spray", true),
    ("bikes", "Glossier bikes", false),
    ("shafts", "Light shafts", true),
    ("cinematic", "Replay blur and depth of field", false),
    ("ruts", "Lasting tyre ruts", false),
    ("scenery", "Trackside props and crowd", false),
    ("grade", "Colour grade", false),
    ("gtao", "Better ambient occlusion (GTAO)", true),
    ("contact", "Contact shadows", true),
    ("ssr", "Reflections in puddles", true),
    ("skylight", "Sky light", true),
    ("pbr", "Realistic materials and reflections", true),
    ("filmic", "Film-like colour (free graphics)", true),
    ("air", "Sunlit haze", true),
    ("nightlights", "Stadium lights at night", true),
    // Only on a GPU that can trace rays; elsewhere the shadow maps stay.
    ("raytracing", "Ray tracing: shadows and reflections", false),
    // The Max preset's own.
    ("supersample", "Supersampling (sharper than the screen)", false),
    ("far", "Detail reaching further", false),
    ("roost", "Flying dirt (roost)", true),
    // Holds the frame rate by drawing the scene a little softer while
    // frames come late; the preset's own size while they keep time.
    ("dynamic", "Keep the frame rate (dynamic resolution)", true),
    // Newer light and upscaling: bounce light from High up, FSR on every
    // preset (it only works when the scene is drawn smaller than the
    // screen, or with TAA).
    ("bounce", "Bounce light", true),
    ("fsr", "AMD FSR upscaling", true),
];

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// The game folder the port reads.
    pub game: Option<PathBuf>,
    /// `graphics=free`: play with the free pack's art and sound
    /// (`--freepack DIR`) instead of the original game's; the original's
    /// unless picked.
    pub free_graphics: bool,
    /// The free pack's folder.
    pub freepack: Option<PathBuf>,
    /// `menus=new`: the menus in the new look (the Free Pack's art and
    /// layout, `--new-menus`), with either graphics; `menus=original` (the
    /// default) keeps the original game's, which need its files.
    pub new_menus: bool,
    /// `menus_pack`: a folder with only the new look menus (the Free
    /// Pack's menus part), for the new menus with the original graphics
    /// when the whole Free Pack isn't there. The Free Pack wins if both are.
    pub menus_pack: Option<PathBuf>,
    pub hires: Option<PathBuf>,
    pub use_hires: bool,
    pub enhanced: bool,
    /// `auto_quality`: the game picks Low, Medium or High each time it
    /// starts, for the graphics card and the screen (`quality=auto`), in
    /// place of [`Settings::quality`] and the choices under it. New
    /// settings start with it; files from before it keep their preset.
    pub auto: bool,
    pub quality: Quality,
    /// The anti-aliasing ([`ANTIALIASING`]) and the most pixels the scene
    /// is drawn with ([`RESOLUTIONS`]): the preset's unless changed.
    pub antialiasing: String,
    pub resolution: String,
    /// `time=`: none keeps the track's own sky.
    pub time: String,
    pub weather: String,
    pub detail: f32,
    /// Each of [`UPGRADES`], on or off.
    pub upgrades: Vec<bool>,
    pub fullscreen: bool,
    /// The full screen's resolution in pixels (`--screen WxH`): (0, 0) is
    /// the screen's own, drawn without changing the screen's mode.
    pub screen: (u32, u32),
    /// The window's size inside, in the screen's points (pixels on most
    /// screens; half as many each way on a Retina screen).
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
    /// The modernised HUD (`--modern-hud`).
    pub sharp_display: bool,
    /// How much steering in the air bends the flight, % of the fork's
    /// own, 0 the game's none (`--air-steer=N`); see [`AIR_MOST`].
    pub air_steer: u32,
    /// How much braking in the air shortens the flight, % of the fork's
    /// own, 0 the game's none (`--air-brake=N`).
    pub air_brake: u32,
    /// G or a pad's Back puts the player back on the bike after a crash
    /// (`--quick-get-up`).
    pub quick_get_up: bool,
    /// The live gap to the best lap (`--lap-gap`).
    pub lap_gap: bool,
    /// The steadier camera (`--steady-camera`).
    pub steady_camera: bool,
    /// The Chase Cam's field of view, degrees (`--fov=N`; the game's 85).
    pub fov: u32,
    /// Look for a newer OpenMCM2 when the launcher opens.
    pub check_updates: bool,
    /// A small note on the game's main menu when a newer OpenMCM2 is out,
    /// for players who start the game without the launcher.
    pub update_notice: bool,
    /// Daniel's cleaner starts: the computer riders keep their order
    /// across the gate off the start (`--clean-starts`).
    pub clean_starts: bool,
    /// National and Supercross races may be one lap long (`--one-lap`).
    pub one_lap: bool,
    /// The speedometer in km/h (`--kmh`).
    pub kmh: bool,
    /// Hard landings level on both wheels don't crash
    /// (`--forgiving-landings`).
    pub forgiving_landings: bool,
    /// The race's sound goes quiet while the game's window is in the
    /// background (`--mute-in-background`).
    pub mute_in_background: bool,
    /// Ctrl+P saves a picture of the screen to the saves folder's
    /// Screenshots (`--screenshot-key`).
    pub screenshot_key: bool,
    /// Draw with OpenGL instead of the GPU's best API, for older graphics
    /// cards without Vulkan (`--opengl`; not on a Mac).
    pub opengl: bool,
    /// The 0.5.0 extras: skins, gear, track filter, pad tuning, picture,
    /// cameras, riders, leaderboards and lobby downloads.
    pub more: More,
}

/// The extras added in 0.5.0, all off (the game's own) by default.
#[derive(Debug, Clone, PartialEq)]
pub struct More {
    /// No longer read: the game always offers the `.tga` paints and
    /// outfits of its `BIKES` and `RIDERS` folders, as the original does.
    /// Kept so older settings files and `--own-skins` still load.
    pub own_skins: bool,
    /// The amateur bikes and outfits of the Pro Circuit in every event
    /// (`--all-gear`).
    pub all_gear: bool,
    /// All, Base game and Custom in the track lists (`--track-filter`).
    pub track_filter: bool,
    /// A pad stick's dead zone, % of its travel (`--pad-dead-zone=N`; the
    /// game's 4).
    pub pad_dead_zone: u32,
    /// How far a stick steers, % (`--pad-sensitivity=N`; the game's 100).
    pub pad_sensitivity: u32,
    /// How much a stick's steering is smoothed, % (`--pad-smoothing=N`;
    /// the game's 0).
    pub pad_smoothing: u32,
    /// How curved a stick's steering is, % (`--pad-curve=N`): 100 is the
    /// game's (cubed: fine near the middle, steep at the edge), 0 straight.
    pub pad_curve: u32,
    /// The most frames a second, 0 for no cap (`--fps-cap=N`).
    pub fps_cap: u32,
    /// Smoother edges after the picture is drawn (`--fxaa`).
    pub fxaa: bool,
    /// Sharpening, % (`--sharpen=N`).
    pub sharpen: u32,
    /// Riders on the gate in a single race, 0 for the game's own count
    /// (`--gate-riders=N`).
    pub riders: u32,
    /// Lap leaderboards: best laps kept and shared with the leaderboard
    /// server; on unless turned off (`--no-leaderboards`). The address
    /// (`--leaderboard-url=URL`, `none` for none) also takes the playtime.
    pub leaderboards: bool,
    pub leaderboard_url: String,
    /// The lobby fetches a host's track or a rider's own skins the player
    /// is missing; on unless turned off (`--no-lobby-downloads`).
    pub lobby_downloads: bool,
    /// Races this player hosts are listed in the server browser (the
    /// leaderboard server's open races); on unless turned off
    /// (`--no-list-races`).
    pub list_races: bool,
    /// The playtime tracker sends anonymous totals (no name) to the
    /// leaderboard server; on unless turned off (`--no-share-playtime`).
    pub share_playtime: bool,
    /// The Course Builder's link on the main menu's chain; on unless
    /// turned off (`--no-builder-menu`).
    pub builder_menu: bool,
    /// The engine sounds from the player's own game or demo, freshened as
    /// they load (EQ, a touch of pitch and saturation, seamless loops),
    /// with the free graphics too; on unless turned off
    /// (`--no-fresh-sounds`).
    pub fresh_sounds: bool,
    /// Drive the scenery: in Practice and free roam, V gets in the
    /// track's cars, trucks, trains and aircraft (`--drive-vehicles`).
    pub drive_vehicles: bool,
    /// Online ghosts: the fastest lap on each leaderboard goes up with
    /// its ghost, and the Saved Ghost Race Files list offers them to race
    /// (`--online-ghosts`).
    pub online_ghosts: bool,
    /// A cartoon slide whistle as a free-mode rider is thrown back off
    /// the world's edge; on unless turned off (`--no-whistle`).
    pub whistle: bool,
    /// Photo mode: in a race, F freezes it with the display hidden, the
    /// camera flies free and Enter saves a picture (`--photo-mode`).
    pub photo_mode: bool,
    /// With the MM2 demo: its other tracks rebuilt, the original's route
    /// and scenery on new ground (`--demo-tracks`).
    pub demo_tracks: bool,
    /// Our own free tracks (the Free Pack's) in the single race and
    /// multiplayer track lists (`--free-tracks`). Off: they're listed only
    /// for an event that has no other track. The Pro Circuit, replays and
    /// a host's race on one still use them either way.
    pub free_tracks: bool,
}

/// The game's own pad dead zone, %.
pub const PAD_DEAD_ZONE: u32 = 4;
/// The most riders on the gate the extra allows.
pub const RIDERS_MOST: u32 = 30;
/// The shared leaderboard server the game uses unless told otherwise.
pub const LEADERBOARD_URL: &str = "https://openmcm2-laps.openmcm2-laps.workers.dev";

impl Default for More {
    fn default() -> Self {
        More {
            own_skins: false,
            all_gear: false,
            track_filter: false,
            pad_dead_zone: PAD_DEAD_ZONE,
            pad_sensitivity: 100,
            pad_smoothing: 0,
            pad_curve: 100,
            fps_cap: 0,
            fxaa: false,
            sharpen: 0,
            riders: 0,
            leaderboards: true,
            leaderboard_url: LEADERBOARD_URL.into(),
            lobby_downloads: true,
            list_races: true,
            share_playtime: true,
            builder_menu: true,
            fresh_sounds: true,
            drive_vehicles: false,
            online_ghosts: false,
            whistle: true,
            photo_mode: false,
            demo_tracks: false,
            free_tracks: false,
        }
    }
}

impl More {
    /// Reads one `key=value` line; false if it isn't one of these.
    fn read(&mut self, k: &str, v: &str) -> bool {
        let yes = v == "1" || v == "true";
        let n = |most: u32, or: u32| v.parse::<u32>().map_or(or, |n| n.min(most));
        match k {
            "own_skins" => self.own_skins = yes,
            "all_gear" => self.all_gear = yes,
            "track_filter" => self.track_filter = yes,
            "pad_dead_zone" => self.pad_dead_zone = n(50, PAD_DEAD_ZONE),
            "pad_sensitivity" => self.pad_sensitivity = n(200, 100).max(25),
            "pad_smoothing" => self.pad_smoothing = n(90, 0),
            "pad_curve" => self.pad_curve = n(100, 100),
            "fps_cap" => self.fps_cap = n(500, 0),
            "fxaa" => self.fxaa = yes,
            "sharpen" => self.sharpen = n(100, 0),
            // The Helmet Cam was an extra until 0.5.4; it is always there now.
            "helmet_cam" => {}
            "riders" => self.riders = n(RIDERS_MOST, 0),
            "leaderboards" => self.leaderboards = yes,
            // Empty (as an early 0.5.0 build saved it) is the default.
            "leaderboard_url" => {
                self.leaderboard_url = match v.trim() {
                    "" => LEADERBOARD_URL.into(),
                    "none" => String::new(),
                    v => v.to_string(),
                }
            }
            "share_playtime" => self.share_playtime = yes,
            "lobby_downloads" => self.lobby_downloads = yes,
            "list_races" => self.list_races = yes,
            "builder_menu" => self.builder_menu = yes,
            "fresh_sounds" => self.fresh_sounds = yes,
            // The old off-by-default Freshen extra (0.7.x): its saved 0 is
            // not a choice, so the new default stands.
            "freshen_engines" => {}
            "drive_vehicles" => self.drive_vehicles = yes,
            "online_ghosts" => self.online_ghosts = yes,
            "whistle" => self.whistle = yes,
            "photo_mode" => self.photo_mode = yes,
            "demo_tracks" => self.demo_tracks = yes,
            "free_tracks" => self.free_tracks = yes,
            _ => return false,
        }
        true
    }

    fn text(&self) -> String {
        let b = |v: bool| if v { "1" } else { "0" };
        format!(
            "own_skins={}\nall_gear={}\ntrack_filter={}\npad_dead_zone={}\npad_sensitivity={}\npad_smoothing={}\npad_curve={}\nfps_cap={}\nfxaa={}\nsharpen={}\nriders={}\nleaderboards={}\nleaderboard_url={}\nlobby_downloads={}\nlist_races={}\nshare_playtime={}\nbuilder_menu={}\nfresh_sounds={}\ndrive_vehicles={}\nonline_ghosts={}\nwhistle={}\nphoto_mode={}\ndemo_tracks={}\nfree_tracks={}\n",
            b(self.own_skins),
            b(self.all_gear),
            b(self.track_filter),
            self.pad_dead_zone,
            self.pad_sensitivity,
            self.pad_smoothing,
            self.pad_curve,
            self.fps_cap,
            b(self.fxaa),
            self.sharpen,
            self.riders,
            b(self.leaderboards),
            if self.leaderboard_url.is_empty() { "none" } else { &self.leaderboard_url },
            b(self.lobby_downloads),
            b(self.list_races),
            b(self.share_playtime),
            b(self.builder_menu),
            b(self.fresh_sounds),
            b(self.drive_vehicles),
            b(self.online_ghosts),
            b(self.whistle),
            b(self.photo_mode),
            b(self.demo_tracks),
            b(self.free_tracks),
        )
    }

    /// The viewer's arguments for these, only where they differ from the
    /// game's own.
    pub fn args(&self) -> Vec<String> {
        let d = More::default();
        let mut a = Vec::new();
        for (on, flag) in [
            (self.own_skins, "--own-skins"),
            (self.all_gear, "--all-gear"),
            (self.track_filter, "--track-filter"),
            (self.fxaa, "--fxaa"),
            (!self.leaderboards, "--no-leaderboards"),
            (!self.share_playtime, "--no-share-playtime"),
            (!self.lobby_downloads, "--no-lobby-downloads"),
            (!self.list_races, "--no-list-races"),
            (!self.builder_menu, "--no-builder-menu"),
            (!self.fresh_sounds, "--no-fresh-sounds"),
            (self.drive_vehicles, "--drive-vehicles"),
            (self.online_ghosts, "--online-ghosts"),
            (!self.whistle, "--no-whistle"),
            (self.photo_mode, "--photo-mode"),
            (self.demo_tracks, "--demo-tracks"),
            (self.free_tracks, "--free-tracks"),
        ] {
            if on {
                a.push(flag.to_string());
            }
        }
        for (v, dv, flag) in [
            (self.pad_dead_zone, d.pad_dead_zone, "--pad-dead-zone"),
            (self.pad_sensitivity, d.pad_sensitivity, "--pad-sensitivity"),
            (self.pad_smoothing, d.pad_smoothing, "--pad-smoothing"),
            (self.pad_curve, d.pad_curve, "--pad-curve"),
            (self.fps_cap, d.fps_cap, "--fps-cap"),
            (self.sharpen, d.sharpen, "--sharpen"),
            (self.riders, d.riders, "--gate-riders"),
        ] {
            if v != dv {
                a.push(format!("{flag}={v}"));
            }
        }
        if self.leaderboard_url != LEADERBOARD_URL {
            // Empty: kept on this computer only.
            a.push(format!("--leaderboard-url={}", if self.leaderboard_url.is_empty() { "none" } else { &self.leaderboard_url }));
        }
        a
    }

    /// Reads them back from the viewer's arguments.
    pub fn from_args(args: &[String]) -> More {
        let d = More::default();
        let has = |f: &str| args.iter().any(|a| a == f);
        let value = |f: &str| args.iter().find_map(|a| a.strip_prefix(f).and_then(|r| r.strip_prefix('=')));
        let mut m = More {
            own_skins: has("--own-skins"),
            all_gear: has("--all-gear"),
            track_filter: has("--track-filter"),
            fxaa: has("--fxaa"),
            lobby_downloads: !has("--no-lobby-downloads"),
            list_races: !has("--no-list-races"),
            leaderboards: !has("--no-leaderboards"),
            share_playtime: !has("--no-share-playtime"),
            builder_menu: !has("--no-builder-menu"),
            fresh_sounds: !has("--no-fresh-sounds"),
            drive_vehicles: has("--drive-vehicles"),
            online_ghosts: has("--online-ghosts"),
            whistle: !has("--no-whistle"),
            photo_mode: has("--photo-mode"),
            demo_tracks: has("--demo-tracks"),
            free_tracks: has("--free-tracks"),
            ..d.clone()
        };
        for (flag, key) in [
            ("--pad-dead-zone", "pad_dead_zone"),
            ("--pad-sensitivity", "pad_sensitivity"),
            ("--pad-smoothing", "pad_smoothing"),
            ("--pad-curve", "pad_curve"),
            ("--fps-cap", "fps_cap"),
            ("--sharpen", "sharpen"),
            ("--gate-riders", "riders"),
            ("--leaderboard-url", "leaderboard_url"),
        ] {
            if let Some(v) = value(flag) {
                m.read(key, v);
            }
        }
        m
    }
}

/// The game's field of view, degrees.
pub const GAME_FOV: u32 = 85;

impl Default for Settings {
    fn default() -> Self {
        Settings {
            game: None,
            free_graphics: false,
            new_menus: false,
            freepack: None,
            menus_pack: None,
            hires: None,
            use_hires: true,
            enhanced: false,
            auto: true,
            quality: Quality::High,
            antialiasing: Quality::High.antialiasing().into(),
            resolution: "auto".into(),
            time: String::new(),
            weather: "clear".into(),
            detail: 0.5,
            // The High preset's, as the quality is.
            upgrades: UPGRADES.iter().map(|u| Quality::High.has(u.0)).collect(),
            fullscreen: false,
            screen: (0, 0),
            window: (1024, 768),
            vsync: true,
            centred_display: false,
            show_fps: false,
            difficulty: DIFFICULTY,
            // Everything the original game didn't have starts off, so
            // the game plays as it did unless the player turns it on.
            vibration: false,
            vibration_strength: 50,
            sharp_display: false,
            air_steer: 0,
            air_brake: 0,
            quick_get_up: false,
            lap_gap: false,
            steady_camera: false,
            fov: GAME_FOV,
            check_updates: true,
            update_notice: true,
            clean_starts: false,
            one_lap: false,
            kmh: false,
            forgiving_landings: false,
            mute_in_background: false,
            screenshot_key: false,
            opengl: false,
            more: More::default(),
        }
    }
}

impl Settings {
    pub fn path() -> PathBuf {
        user_dir().join("launcher.ini")
    }

    /// Whether every extra the original game didn't have is off, as it
    /// starts.
    pub fn is_original(&self) -> bool {
        let mut o = self.clone();
        o.original();
        o == *self
    }

    /// Turns off every extra the original game didn't have, keeping the
    /// display, graphics and game folder as they are.
    pub fn original(&mut self) {
        let d = Settings::default();
        self.centred_display = d.centred_display;
        self.difficulty = d.difficulty;
        self.vibration = d.vibration;
        self.sharp_display = d.sharp_display;
        self.air_steer = d.air_steer;
        self.air_brake = d.air_brake;
        self.quick_get_up = d.quick_get_up;
        self.lap_gap = d.lap_gap;
        self.steady_camera = d.steady_camera;
        self.fov = d.fov;
        self.clean_starts = d.clean_starts;
        self.one_lap = d.one_lap;
        self.kmh = d.kmh;
        self.forgiving_landings = d.forgiving_landings;
        self.mute_in_background = d.mute_in_background;
        self.screenshot_key = d.screenshot_key;
        // The leaderboard's address is where to send laps, not an extra.
        // Nor are the leaderboards, sharing playtime, lobby downloads and
        // the server browser, which change nothing in a race.
        let url = std::mem::take(&mut self.more.leaderboard_url);
        let m = &self.more;
        self.more = More {
            leaderboard_url: url,
            leaderboards: m.leaderboards,
            share_playtime: m.share_playtime,
            lobby_downloads: m.lobby_downloads,
            list_races: m.list_races,
            builder_menu: m.builder_menu,
            whistle: m.whistle,
            ..More::default()
        };
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
        // From before they were shown, the preset's.
        let (mut antialiasing, mut resolution) = (None, None);
        // Temporal anti-aliasing was an effect of its own, on top of MSAA.
        let mut temporal = false;
        let version = text
            .lines()
            .find_map(|l| l.trim().strip_prefix("version=")?.parse::<u32>().ok())
            .unwrap_or(1);
        for (k, v) in text.lines().filter_map(|l| l.split_once('=')) {
            let v = v.trim();
            match k.trim() {
                "game" if !v.is_empty() => s.game = Some(v.into()),
                "graphics" => s.free_graphics = v == "free",
                "menus" => s.new_menus = v == "new",
                "freepack" if !v.is_empty() => s.freepack = Some(v.into()),
                "menus_pack" if !v.is_empty() => s.menus_pack = Some(v.into()),
                "hires" if !v.is_empty() => s.hires = Some(v.into()),
                "use_hires" => s.use_hires = yes(v),
                "enhanced" => s.enhanced = yes(v),
                "auto_quality" => s.auto = yes(v),
                "quality" => {
                    s.quality = Quality::ALL.into_iter().find(|q| q.name() == v).unwrap_or(Quality::High)
                }
                "antialiasing" => antialiasing = ANTIALIASING.iter().find(|a| a.0 == v).map(|a| a.0.to_string()),
                // As it was saved before TAA joined it: the samples.
                "msaa" if antialiasing.is_none() => {
                    antialiasing = ANTIALIASING.iter().find(|a| a.2 == format!("msaa={v}")).map(|a| a.0.to_string())
                }
                "upgrade.taa" => temporal = yes(v),
                "resolution" => resolution = parse_resolution(v),
                "time" => s.time = v.into(),
                "weather" => s.weather = v.into(),
                "detail" => s.detail = v.parse::<f32>().unwrap_or(0.5).clamp(0.0, 1.0),
                "fullscreen" => s.fullscreen = yes(v),
                "screen" => s.screen = parse_size(v).unwrap_or((0, 0)),
                "window" => {
                    if let Some(z) = parse_size(v).filter(|z| z.0 > 0 && z.1 > 0) {
                        s.window = z;
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
                "update_notice" => s.update_notice = yes(v),
                "clean_starts" => s.clean_starts = yes(v),
                "one_lap" => s.one_lap = yes(v),
                "kmh" => s.kmh = yes(v),
                "forgiving_landings" => s.forgiving_landings = yes(v),
                "mute_in_background" => s.mute_in_background = yes(v),
                "screenshot_key" => s.screenshot_key = yes(v),
                "opengl" => s.opengl = yes(v),
                // Before version 4 lobby downloads were off by default
                // and every file kept that; they start on now.
                "lobby_downloads" if version < 4 => {}
                k if s.more.read(k, v) => {}
                // Files from before version 2 kept the replay blur on by
                // default; it starts off now, so their setting is dropped.
                "upgrade.cinematic" if version < 2 => {}
                // Before version 3 the bloom and glossy bikes were on by
                // default; they start off now, so those files' are dropped.
                "upgrade.bloom" | "upgrade.bikes" if version < 3 => {}
                k => {
                    if let Some(i) = UPGRADES.iter().position(|u| k == format!("upgrade.{}", u.0)) {
                        s.upgrades[i] = yes(v);
                    }
                }
            }
        }
        s.antialiasing = if temporal { "taa".into() } else { antialiasing.unwrap_or_else(|| s.quality.antialiasing().into()) };
        s.resolution = resolution.unwrap_or_else(|| "auto".into());
        // Saved before Auto was offered: the preset picked then.
        if !text.lines().any(|l| l.split_once('=').is_some_and(|(k, _)| k.trim() == "auto_quality")) {
            s.auto = false;
        }
        s
    }

    pub fn text(&self) -> String {
        let p = |o: &Option<PathBuf>| o.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        let b = |v: bool| if v { "1" } else { "0" };
        let mut t = format!(
            "version=4\ngame={}\nhires={}\nuse_hires={}\nenhanced={}\nauto_quality={}\nquality={}\nantialiasing={}\nresolution={}\ntime={}\nweather={}\ndetail={}\nfullscreen={}\nscreen={}x{}\nwindow={}x{}\nvsync={}\ncentred_display={}\nshow_fps={}\ndifficulty={}\nvibration={}\nvibration_strength={}\nsharp_display={}\nair_steer={}\nair_brake={}\nquick_get_up={}\nlap_gap={}\nsteady_camera={}\nfov={}\ncheck_updates={}\nupdate_notice={}\nclean_starts={}\none_lap={}\nkmh={}\nmute_in_background={}\nscreenshot_key={}\nopengl={}\nforgiving_landings={}\n",
            p(&self.game),
            p(&self.hires),
            b(self.use_hires),
            b(self.enhanced),
            b(self.auto),
            self.quality.name(),
            self.antialiasing,
            self.resolution,
            self.time,
            self.weather,
            self.detail,
            b(self.fullscreen),
            self.screen.0,
            self.screen.1,
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
            b(self.update_notice),
            b(self.clean_starts),
            b(self.one_lap),
            b(self.kmh),
            b(self.mute_in_background),
            b(self.screenshot_key),
            b(self.opengl),
            b(self.forgiving_landings),
        );
        t += &format!(
            "graphics={}\nfreepack={}\nmenus={}\nmenus_pack={}\n",
            if self.free_graphics { "free" } else { "original" },
            p(&self.freepack),
            if self.new_menus { "new" } else { "original" },
            p(&self.menus_pack)
        );
        t += &self.more.text();
        for (u, on) in UPGRADES.iter().zip(&self.upgrades) {
            t += &format!("upgrade.{}={}\n", u.0, b(*on));
        }
        t
    }

    /// `mm2-viewer`'s arguments for these settings.
    pub fn args(&self) -> Vec<String> {
        let mut a = Vec::new();
        match (&self.freepack, &self.game) {
            // The free pack's art and sound; the original's menus need the
            // game's files too, the new ones need only the pack.
            (Some(f), game) if self.free_graphics => {
                a.push("--freepack".into());
                a.push(f.display().to_string());
                match game {
                    Some(g) if !self.new_menus && !is_demo(g) => {
                        a.push(g.display().to_string());
                        a.push("--original-menus".into());
                    }
                    _ => a.push("--new-menus".into()),
                }
            }
            // The demo with the original look, the Free Pack filling in its
            // missing tracks, bikes and the few screens it doesn't have.
            // Its own menus unless the new look is picked.
            (Some(f), Some(g)) if is_demo(g) => {
                a.push(g.display().to_string());
                a.push("--fill-from".into());
                a.push(f.display().to_string());
                if self.new_menus {
                    a.push(format!("--new-menus={}", f.display()));
                } else {
                    a.push("--original-menus".into());
                }
            }
            (pack, Some(g)) => {
                a.push(g.display().to_string());
                // The new menus from the Free Pack, or from the menus
                // downloaded on their own.
                match pack.as_ref().or(self.menus_pack.as_ref()) {
                    Some(f) if self.new_menus => a.push(format!("--new-menus={}", f.display())),
                    Some(_) => a.push("--original-menus".into()),
                    None => {}
                }
            }
            _ => {}
        }
        if let (true, Some(h)) = (self.use_hires && !self.free_graphics, &self.hires) {
            a.push("--hires".into());
            a.push(h.display().to_string());
        }
        if self.enhanced && self.auto {
            // The game picks the rest for the computer it's on.
            let mut list = vec!["quality=auto".to_string()];
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
        } else if self.enhanced {
            let mut list = vec![format!("quality={}", self.quality.name())];
            if self.antialiasing != self.quality.antialiasing() {
                if let Some(a) = ANTIALIASING.iter().find(|a| a.0 == self.antialiasing) {
                    list.push(a.2.to_string());
                }
            }
            if self.resolution != "auto" {
                list.push(format!("resolution={}", self.resolution));
            }
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
            if self.screen != (0, 0) {
                a.push("--screen".into());
                a.push(format!("{}x{}", self.screen.0, self.screen.1));
            }
        } else {
            a.push("--window".into());
            a.push(format!("{}x{}", self.window.0, self.window.1));
        }
        if !self.vsync {
            a.push("--no-vsync".into());
        }
        if self.opengl && !cfg!(target_os = "macos") {
            a.push("--opengl".into());
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
        if self.quick_get_up {
            a.push("--quick-get-up".into());
        }
        if self.lap_gap {
            a.push("--lap-gap".into());
        }
        if self.steady_camera {
            a.push("--steady-camera".into());
        }
        if self.fov != GAME_FOV {
            a.push(format!("--fov={}", self.fov));
        }
        if self.sharp_display {
            a.push("--modern-hud".into());
        }
        for (on, flag) in [
            (self.clean_starts, "--clean-starts"),
            (self.one_lap, "--one-lap"),
            (self.kmh, "--kmh"),
            (self.forgiving_landings, "--forgiving-landings"),
            (self.mute_in_background, "--mute-in-background"),
            (self.screenshot_key, "--screenshot-key"),
        ] {
            if on {
                a.push(flag.into());
            }
        }
        a.extend(self.more.args());
        a.push(match self.vibration {
            true => format!("--vibration={}", self.vibration_strength),
            false => "--vibration=off".into(),
        });
        a
    }
}

pub mod laps;
pub mod playtime;
pub mod races;
pub mod release;

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
    fn presets_show_their_anti_aliasing_and_resolution() {
        // A file from before they were shown takes its preset's.
        let old = Settings::parse("quality=ultra\n");
        assert_eq!((old.antialiasing.as_str(), old.resolution.as_str()), ("msaa8", "auto"));
        // Saved as samples, or TAA as an effect of its own.
        assert_eq!(Settings::parse("quality=high\nmsaa=2\n").antialiasing, "msaa2");
        assert_eq!(Settings::parse("quality=high\nmsaa=4\nupgrade.taa=1\n").antialiasing, "taa");
        let mut s = Settings::default();
        s.enhanced = true;
        s.auto = false;
        s.quality = Quality::RayTraced;
        s.antialiasing = s.quality.antialiasing().into();
        s.resolution = "auto".into();
        for (u, on) in UPGRADES.iter().zip(s.upgrades.iter_mut()) {
            *on = s.quality.has(u.0);
        }
        // Ray tracing is the preset's own: nothing to add.
        assert!(s.args().iter().any(|a| a == "--enhanced=quality=raytraced"));
        s.resolution = "85%".into();
        assert!(s.args().iter().any(|a| a == "--enhanced=quality=raytraced,resolution=85%"));
        // Any share from 50% to 100%, from the slider.
        assert_eq!(Settings::parse("resolution=72%
").resolution, "72%");
        assert_eq!(Settings::parse("resolution=20%
").resolution, "auto");
        // TAA in place of MSAA.
        s.antialiasing = "taa".into();
        assert!(s.args().iter().any(|a| a == "--enhanced=quality=raytraced,msaa=1,taa,resolution=85%"));
    }

    #[test]
    fn round_trip() {
        let mut s = Settings::default();
        s.game = Some("/x/GAME".into());
        s.enhanced = true;
        s.auto = false;
        s.upgrades[1] = false;
        s.upgrades[13] = true;
        s.time = "dusk".into();
        s.window = (1920, 1080);
        s.screen = (1280, 720);
        s.centred_display = true;
        s.difficulty[2][0] = 9;
        s.air_steer = 100;
        s.air_brake = 150;
        s.quick_get_up = true;
        s.lap_gap = true;
        s.vibration = true;
        s.vibration_strength = 80;
        s.fov = 100;
        s.check_updates = false;
        s.update_notice = false;
        s.kmh = true;
        s.forgiving_landings = true;
        s.screenshot_key = true;
        s.antialiasing = "msaa2".into();
        assert_eq!(Settings::parse(&s.text()), s);
        assert_eq!(
            s.args(),
            [
                "/x/GAME",
                "--enhanced=quality=high,msaa=2,no-shadows,ruts,time=dusk",
                "--window",
                "1920x1080",
                "--centred-display",
                "--difficulty=3,5,7,3,5,7,9,5,7,3,5,7",
                "--air-steer=100",
                "--air-brake=150",
                "--quick-get-up",
                "--lap-gap",
                "--fov=100",
                "--kmh",
                "--forgiving-landings",
                "--screenshot-key",
                "--vibration=80"
            ]
        );
        assert!(!s.is_original());
        s.original();
        assert!(s.is_original());
    }

    #[test]
    fn auto_graphics() {
        // New settings pick for the computer; old files keep their preset.
        assert!(Settings::default().auto);
        assert!(!Settings::parse("enhanced=1\nquality=high\n").auto);
        let mut s = Settings::default();
        s.enhanced = true;
        s.time = "dusk".into();
        assert!(s.args().iter().any(|a| a == "--enhanced=quality=auto,time=dusk"));
        assert!(Settings::parse(&s.text()).auto);
        s.auto = false;
        assert!(!Settings::parse(&s.text()).auto);
        assert!(s.args().iter().any(|a| a.starts_with("--enhanced=quality=high")));
    }

    #[test]
    fn full_screen_resolution() {
        let mut s = Settings::default();
        s.fullscreen = true;
        // The screen's own: no mode change asked for.
        assert!(s.args().iter().any(|a| a == "--fullscreen"));
        assert!(!s.args().iter().any(|a| a == "--screen"));
        s.screen = (1280, 720);
        let a = s.args();
        let at = a.iter().position(|a| a == "--screen").unwrap();
        assert_eq!(a[at + 1], "1280x720");
        assert_eq!(Settings::parse(&s.text()).screen, (1280, 720));
        // Files from before it start at the screen's own.
        assert_eq!(Settings::parse("fullscreen=1\n").screen, (0, 0));
        // A window keeps its size whatever the full screen's.
        s.fullscreen = false;
        assert!(!s.args().iter().any(|a| a == "--screen"));
        assert_eq!(shape_of((2560, 1080)), 2);
        assert_eq!(fitting(SHAPES[1].2, Some((1920, 1080))), Some((1600, 900)));
    }

    #[test]
    fn the_new_extras_round_trip() {
        let mut s = Settings::default();
        s.more = More {
            own_skins: true,
            all_gear: true,
            track_filter: true,
            pad_dead_zone: 10,
            pad_sensitivity: 120,
            pad_smoothing: 30,
            pad_curve: 50,
            fps_cap: 144,
            fxaa: true,
            sharpen: 40,
            riders: 20,
            leaderboards: false,
            leaderboard_url: "https://example.org/mm2".into(),
            lobby_downloads: false,
            list_races: false,
            share_playtime: false,
            builder_menu: false,
            fresh_sounds: false,
            drive_vehicles: true,
            online_ghosts: true,
            whistle: false,
            photo_mode: true,
            demo_tracks: true,
            free_tracks: true,
        };
        s.free_graphics = true;
        s.freepack = Some("/packs/free".into());
        assert_eq!(Settings::parse(&s.text()), s);
        assert_eq!(s.args()[..2], ["--freepack".to_string(), "/packs/free".to_string()]);
        assert!(s.args().contains(&"--new-menus".to_string()));
        {
            // The original's menus over the free graphics, then the new
            // menus over the original's.
            let mut t = s.clone();
            t.game = Some("/games/mm2".into());
            assert_eq!(t.args()[2..4], ["/games/mm2".to_string(), "--original-menus".to_string()]);
            t.free_graphics = false;
            t.new_menus = true;
            assert_eq!(Settings::parse(&t.text()).new_menus, true);
            assert_eq!(t.args()[..2], ["/games/mm2".to_string(), "--new-menus=/packs/free".to_string()]);
            // The menus alone, with no Free Pack.
            t.freepack = None;
            t.menus_pack = Some("/packs/menus".into());
            assert_eq!(Settings::parse(&t.text()), t);
            assert_eq!(t.args()[..2], ["/games/mm2".to_string(), "--new-menus=/packs/menus".to_string()]);
            t.new_menus = false;
            assert_eq!(t.args()[..2], ["/games/mm2".to_string(), "--original-menus".to_string()]);
        }
        let mut args = vec!["mm2-viewer".to_string()];
        args.extend(s.args());
        assert_eq!(More::from_args(&args), s.more);
        assert!(!s.is_original());
        s.original();
        assert!(s.is_original());
        assert_eq!(s.more.leaderboard_url, "https://example.org/mm2");
        assert!(!s.more.share_playtime && !s.more.leaderboards, "not extras: kept");
        assert!(!s.more.lobby_downloads && !s.more.list_races, "not extras: kept");
        let d = More::default();
        assert!(d.share_playtime && d.leaderboards && d.lobby_downloads && d.list_races, "on unless turned off");
        // Files from before 0.5.3 saved lobby downloads off, the old default.
        assert!(Settings::parse("version=3\nlobby_downloads=0\n").more.lobby_downloads);
        assert!(!Settings::parse("version=4\nlobby_downloads=0\n").more.lobby_downloads);
        let none = More { leaderboard_url: String::new(), ..More::default() };
        assert_eq!(More::from_args(&none.args()).leaderboard_url, "");
        let kept = Settings { more: none.clone(), ..Settings::default() };
        assert_eq!(Settings::parse(&kept.text()).more.leaderboard_url, "");
        assert_eq!(Settings::parse("leaderboard_url=\n").more.leaderboard_url, LEADERBOARD_URL);
        assert_eq!(More::from_args(&[]), More::default());
        assert!(More::default().args().is_empty());
    }

    #[test]
    fn the_extras_start_off() {
        let s = Settings::default();
        assert!(s.is_original());
        let args = s.args();
        assert_eq!(&args[args.len() - 1..], ["--vibration=off"]);
    }
}

/// The file an unpacked Motocross Madness 2 demo has in its folder
/// (`mm2_formats::demo::MARKER`).
pub const DEMO_MARKER: &str = "DEMO.TXT";

/// Whether the game folder `game` is the unpacked demo.
pub fn is_demo(game: &std::path::Path) -> bool {
    game.join(DEMO_MARKER).is_file()
}
