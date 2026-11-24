//! The Course Builder (an extra, not the game's): make a track by drawing
//! its racing line on the land, dropping jumps onto the line from a kit,
//! shaping and painting the ground and placing objects, then ride it at
//! once. It writes an ordinary `.ENV` into the game's `TERAFORM` folder,
//! so the track shows in the track list like any added track.
//!
//! The screen: a top bar (file, undo, Ride it), a tool strip and the
//! tool's options down the left, a hint line along the bottom and the land
//! filling the window. Every button and slider explains itself on hover.

mod panel;
pub mod view;

use crate::job::Job;
use crate::ui::{self, Gfx};
use glam::{Mat4, Vec3, Vec4};
use mm2_course::bake::{self, Field};
use mm2_course::kit::Kind;
use mm2_course::path::Path as Line;
use mm2_course::{Brush, Course, Craft, Flight, Flora, LandKind, Mode, Object, Paint, Planting, Point, River, Stroke, Surface};
use panel::{Input, Painter};
use std::path::{Path, PathBuf};

/// Set to ride the track as soon as the builder opens on it
/// (`--ride-course`, from the Tools page's Course Builder).
pub static RIDE_ON_OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

static OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The menus' Course Builder button: the game opens the builder.
pub fn ask_open() {
    OPEN.store(true, std::sync::atomic::Ordering::Relaxed);
}

pub fn take_open() -> bool {
    OPEN.swap(false, std::sync::atomic::Ordering::Relaxed)
}

/// A track there is to copy.
/// The tracks in the game folder's TERAFORM, by file, when there are no
/// menus to list them.
pub fn tracks_in_folder() -> Vec<GameTrack> {
    let game = GAME.get().cloned().flatten();
    let mut out = Vec::new();
    // The Free Pack's tracks, built from their recipes when first copied.
    let teraform = crate::free_tracks::home(game.as_deref());
    for mode in Mode::ALL {
        for (path, def) in crate::free_tracks::listed(&teraform, mode.folder()) {
            let courses = crate::free_tracks::courses(def);
            if courses.is_empty() {
                out.push(GameTrack { mode, name: def.name.to_string(), path: path.clone(), course: 0 });
            }
            for (course, name) in courses.into_iter().enumerate() {
                out.push(GameTrack { mode, name, path: path.clone(), course });
            }
        }
    }
    let Some(game) = game else { return out };
    for mode in Mode::ALL {
        let dir = game.join("TERAFORM").join(mode.folder());
        let Ok(list) = std::fs::read_dir(&dir) else { continue };
        let mut files: Vec<PathBuf> = list
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("env")))
            .collect();
        files.sort();
        for path in files {
            let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            if crate::free_tracks::listed(&teraform, mode.folder()).iter().any(|(p, _)| *p == path) {
                continue;
            }
            out.push(GameTrack { mode, name, path, course: 0 });
        }
    }
    out
}

/// The game folder the builder bakes into, for [`tracks_in_folder`].
static GAME: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();

/// What a gamepad's buttons do in the builder.
pub enum PadButton {
    Remove,
    Tool(i32),
    Undo,
    Ride,
    Top,
    Back,
}

pub struct GameTrack {
    pub mode: Mode,
    pub name: String,
    pub path: PathBuf,
    pub course: usize,
}

/// What the builder asks the game to do.
pub enum Ask {
    Stay,
    /// Ride the track baked at this path.
    /// Ride the track; with `watch`, the computer rides it while the
    /// player watches (the original builder's Test Computer Bike).
    Ride { env: PathBuf, mode: Mode, name: String, watch: bool },
    /// Leave the builder.
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tool {
    Line,
    Jumps,
    Shape,
    Paint,
    Objects,
    Nature,
    Motion,
    Land,
}

const TOOLS: [(Tool, &str, &str); 8] = [
    (Tool::Line, "Line", "Draw the racing line. Click the ground to add points, drag them to move. (1)"),
    (Tool::Jumps, "Jumps", "Drop jumps, whoops and rollers onto the line from the kit. (2)"),
    (Tool::Shape, "Shape", "Raise, lower, smooth and flatten the ground with a brush. (3)"),
    (Tool::Paint, "Paint", "Paint dirt, grass, sand, mud or hardpack. Each grips differently. (4)"),
    (Tool::Objects, "Objects", "Place hay bales, banners, tents and more. (5)"),
    (Tool::Nature, "Nature", "Paint trees, bushes and grass, and draw rivers to splash through. (6)"),
    (Tool::Motion, "Flying", "Planes, a helicopter or the blimp flying round paths you draw over the track. (7)"),
    (Tool::Land, "Land", "The land under the track, the sky, the sun and the track's name. (8)"),
];

/// Objects to place: name, model, about.
const OBJECTS: [(&str, &str, &str); 20] = [
    ("Hay bale", "HayBale.slt", "A big round straw bale, lying on its side."),
    ("Bale stack", "TripleBale03.slt", "Three bales stacked, the Supercross edge marker."),
    ("Wrapped bale", "AlpineStarsHB.SLT", "A bale in a sponsor wrap, as the stadiums line their tracks."),
    ("Banner", "Fox03.SLT", "A sponsor banner on a frame."),
    ("Tent", "Tent01.slt", "A pit tent."),
    ("Big tent", "Tent02.slt", "A big hospitality tent."),
    ("Camera tower", "CameraPlatform01.slt", "A TV camera platform."),
    ("Race trailer", "NatTrailer01.slt", "A team's race trailer."),
    ("Pit trailer", "Pit_Trailer.slt", "A small pit trailer."),
    ("Light pole", "LightPole.slt", "A tall light pole."),
    ("Toilet", "Toilet.slt", "A portable toilet."),
    ("Water tank", "WaterTank01.slt", "A water tank on legs."),
    ("Tractor", "Tractor.slt", "A farm tractor."),
    ("Bobcat", "Bobcat.SLT", "A small track loader."),
    ("Giant tyre", "GiantTire01.slt", "A huge earth-mover tyre."),
    ("Barrel", "50Gal.slt", "A 50 gallon drum."),
    ("Farm house", "FarmHouse.slt", "A farm house."),
    ("Picket fence", "Fence_Picket.slt", "A stretch of white picket fence."),
    ("Wire fence", "Fence_BarbedWire.slt", "A stretch of barbed wire fence."),
    ("Scarecrow", "Scarecrow.slt", "A scarecrow."),
];

/// The stadiums' floor, ft (x, z low and high corners).
const FLOOR: ([f32; 2], [f32; 2]) = ([780.0, 780.0], [1525.0, 1520.0]);

/// The colours of the builder's panels: warm black, muted lime.
pub const PANEL: [f32; 4] = [0.07, 0.07, 0.065, 0.86];
pub const ACCENT: [f32; 3] = [0.64, 0.76, 0.29];
const LINE_COLOUR: [f32; 4] = [1.0, 1.0, 1.0, 0.75];
const JUMP_COLOUR: [f32; 4] = [1.0, 0.72, 0.25, 0.95];

/// How far a pad's stick goes before the builder takes it.
pub const PAD_DEAD: f32 = 0.2;

/// An orbiting camera over the land, in the game's coordinates.
#[derive(Debug, Clone, Copy)]
struct Camera {
    target: Vec3,
    yaw: f32,
    pitch: f32,
    dist: f32,
}

impl Camera {
    fn eye(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        self.target + Vec3::new(cp * sy, sp, cp * cy) * self.dist
    }

    /// The camera's matrix in drawing space (the game's z turned round).
    fn view_proj(&self, aspect: f32) -> Mat4 {
        let d = |v: Vec3| Vec3::new(v.x, v.y, -v.z);
        let up = if self.pitch > 1.5 { Vec3::new(-self.yaw.sin(), 0.0, self.yaw.cos()) } else { Vec3::Y };
        let near = (self.dist * 0.01).clamp(1.0, 20.0);
        Mat4::perspective_rh(50f32.to_radians(), aspect, near, 40_000.0) * Mat4::look_at_rh(d(self.eye()), d(self.target), d(up))
    }
}

/// What the pointer is over on the land.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Hover {
    None,
    /// One of the line's points.
    Point(usize),
    /// The line, at this distance along it.
    Line(f32),
    Jump(usize),
    Object(usize),
    /// A river's point: the river, the point.
    River(usize, usize),
    /// A flight path's point.
    Flight(usize, usize),
    /// Bare ground.
    Ground,
}

/// What a file the player is picking is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PickFor {
    Heights,
    Ground,
    Plants,
    Model,
}

/// What a click does with the Line tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineMode {
    Draw,
    /// Baja and Enduro: a click on the track adds a gate, or takes one away.
    Gates,
    /// Moves the start: along a lap, or anywhere for a funnel start.
    Start,
}

/// What the Nature tool does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NatureMode {
    Plant,
    Clear,
    River,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Drag {
    None,
    Point(usize),
    /// A jump, held this far from its start.
    Jump(usize, f32),
    Object(usize),
    River(usize, usize),
    Flight(usize, usize),
    Brush,
    Turn,
    Pan,
}

/// A pop-up over the screen.
enum Popup {
    /// A new track; with `surprise`, a whole track made up to ride now.
    New { surprise: bool },
    /// Your tracks, and the tracks there are to copy (of one mode, from
    /// a row down).
    Open { files: Vec<PathBuf>, mode: Mode, scroll: usize },
    /// The kind of race the track is for, to change it.
    Kind,
    /// Which game to save the track for.
    Save,
}

thread_local! {
    /// A closed builder's menus cache, for the next one to open: its
    /// textures live in the window's sprites, which outlive the builder.
    static KEPT_UI: std::cell::RefCell<Option<ui::Ui>> = const { std::cell::RefCell::new(None) };
}

impl Drop for Builder {
    fn drop(&mut self) {
        if let Some(ui) = self.ui.take() {
            KEPT_UI.set(Some(ui));
        }
    }
}

pub struct Builder {
    pub course: Course,
    /// Which of the pad's sticks and triggers (left x, y, right x, y,
    /// zoom) have been seen at rest since the builder opened: one that
    /// never sits in the middle (a worn or odd stick, a trigger read as
    /// resting at its end, a wheel or 3D mouse taken for a pad) would
    /// otherwise spin or slide the view on its own.
    pad_live: [bool; 5],
    history: Vec<Course>,
    future: Vec<Course>,
    /// The game folder, where baked tracks go (`TERAFORM`).
    game: Option<PathBuf>,
    /// Where `.course` files are kept.
    dir: PathBuf,
    land: Option<(Mode, mm2_course::Land, usize, Vec<f32>)>,
    /// The copied track the land came from (its address), if any.
    land_src: usize,
    field: Option<Field>,
    line: Line,
    gates: Vec<([f32; 2], [f32; 2])>,
    dirty: bool,
    baked_at: f32,
    view: Option<view::View>,
    cam: Camera,
    tool: Tool,
    kind: Kind,
    /// The jump picked for its sliders.
    picked: Option<usize>,
    brush: Brush,
    radius: f32,
    strength: f32,
    surface: Surface,
    paint_radius: f32,
    object: usize,
    nature: NatureMode,
    line_mode: LineMode,
    /// Which page of the Land tool shows.
    land_tab: usize,
    flora: Flora,
    plant_radius: f32,
    plant_density: f32,
    /// The river and the flight path being drawn.
    river: Option<usize>,
    flight: Option<usize>,
    craft: Craft,
    /// The plants as the preview's ground grows them (where, which kind),
    /// and when they were last grown.
    plants: Vec<([f32; 3], Flora)>,
    plants_at: f32,
    /// The window's size, and the pointer on the builder's screen (480
    /// high, as wide as the window's shape makes it).
    size: (u32, u32),
    pointer: (f32, f32),
    ground: Option<Vec3>,
    hover: Hover,
    drag: Drag,
    drag_from: (f32, f32),
    input: Input,
    keys: std::collections::HashSet<winit::keyboard::KeyCode>,
    shift: bool,
    ctrl: bool,
    naming: bool,
    /// Renaming has just begun: the first letter typed replaces the name.
    name_fresh: bool,
    popup: Option<Popup>,
    toast: Option<(String, f32)>,
    now: f32,
    ui: Option<ui::Ui>,
    bake_job: Option<(Job<Result<PathBuf, String>>, bool)>,
    /// Making the track for the original game: its file name and bytes.
    export_job: Option<Job<Result<(String, Vec<u8>), String>>>,
    /// When the running bake or export began, for its bar.
    busy_since: f32,
    /// How long the last bake (0) and export for the original (1) of each
    /// kind of race took, s, to fill the bar by.
    took: [[f32; 4]; 2],
    /// Files asked for in a browser, not yet given.
    picking: Option<PickFor>,
    /// The next ride is the computer's, to watch.
    watch: bool,
    /// A Free Pack track being built from its recipe to copy.
    copy_job: Option<Job<Result<Course, String>>>,
    ask: Option<Ask>,
    saved_at: f32,
    unsaved: bool,
    /// A track has been ridden from the builder.
    pub rode: bool,
    /// The tracks there are to copy, once the game has listed them.
    pub tracks: Vec<GameTrack>,
    pub wants_tracks: bool,
}

impl Builder {
    /// Opens the builder on `file` (a `.course` or a builder `.ENV`), or
    /// on the last track worked on, or a new National track.
    pub fn new(game: Option<PathBuf>, file: Option<&Path>) -> Builder {
        let _ = GAME.set(game.clone());
        let dir = crate::profile::user_dir().join("Courses");
        let last = crate::fs::read_to_string(dir.join("last.txt")).ok().map(|s| PathBuf::from(s.trim()));
        let course = file
            .or(last.as_deref())
            .and_then(load_file)
            .unwrap_or_else(|| Course::new(Mode::National));
        let mut b = Builder {
            course,
            history: Vec::new(),
            future: Vec::new(),
            game,
            dir,
            land: None,
            land_src: 0,
            field: None,
            line: Line::default(),
            gates: Vec::new(),
            dirty: true,
            baked_at: -1.0,
            view: None,
            cam: Camera { target: Vec3::ZERO, yaw: 0.0, pitch: 0.95, dist: 1000.0 },
            pad_live: [false; 5],
            tool: Tool::Line,
            kind: Kind::Tabletop,
            picked: None,
            brush: Brush::Raise,
            radius: 60.0,
            strength: 0.5,
            surface: Surface::Mud,
            paint_radius: 25.0,
            object: 1,
            nature: NatureMode::Plant,
            line_mode: LineMode::Draw,
            land_tab: 0,
            flora: Flora::Mixed,
            plant_radius: 60.0,
            plant_density: 0.6,
            river: None,
            flight: None,
            craft: Craft::Biplane,
            plants: Vec::new(),
            plants_at: -10.0,
            size: (1280, 720),
            pointer: (0.0, 0.0),
            ground: None,
            hover: Hover::None,
            drag: Drag::None,
            drag_from: (0.0, 0.0),
            input: Input::default(),
            keys: Default::default(),
            shift: false,
            ctrl: false,
            naming: false,
            name_fresh: false,
            popup: None,
            toast: None,
            now: 0.0,
            ui: None,
            bake_job: None,
            export_job: None,
            busy_since: 0.0,
            took: [[4.0, 9.0, 15.0, 9.0], [12.0, 35.0, 50.0, 35.0]],
            picking: None,
            watch: false,
            copy_job: None,
            ask: None,
            saved_at: 0.0,
            unsaved: false,
            rode: false,
            tracks: Vec::new(),
            wants_tracks: false,
        };
        b.frame_track();
        // Opened to ride (the Tools page's Ride it): bake and go.
        if RIDE_ON_OPEN.swap(false, std::sync::atomic::Ordering::Relaxed) {
            b.bake_track(true);
        }
        b
    }

    /// The builder's screen width (it is 480 high).
    fn width(&self) -> f32 {
        480.0 * self.size.0 as f32 / self.size.1.max(1) as f32
    }

    /// Looks at the whole track.
    fn frame_track(&mut self) {
        self.refresh_line();
        let side = self.course.mode.side();
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for p in &self.course.line.points {
            lo = [lo[0].min(p.x), lo[1].min(p.z)];
            hi = [hi[0].max(p.x), hi[1].max(p.z)];
        }
        if lo[0] > hi[0] {
            lo = [side * 0.4; 2];
            hi = [side * 0.6; 2];
        }
        let mid = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
        let span = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(200.0);
        let y = match &self.field {
            Some(f) => f.height(mid[0], mid[1]),
            None => mm2_course::land::height(&self.course.land, side, mid[0], mid[1]),
        };
        self.cam = Camera { target: Vec3::new(mid[0], y, mid[1]), yaw: 0.0, pitch: 0.95, dist: span * 1.25 };
    }

    fn refresh_line(&mut self) {
        self.line = self.course.path();
        self.gates = if self.course.mode.laps() { Vec::new() } else { mm2_course::env::gates(&self.course, &self.line) };
    }

    /// Every n-th of the game's samples the preview uses.
    fn every(&self) -> usize {
        match self.course.mode {
            Mode::Supercross => 2,
            _ => 4,
        }
    }

    /// Bakes the quick preview again when something changed (at most 25
    /// times a second while dragging).
    fn rebake(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if !self.dirty || (self.drag != Drag::None && self.now - self.baked_at < 0.04) {
            return;
        }
        let every = self.every();
        let src = self.course.source.as_ref().map_or(0, |s| std::sync::Arc::as_ptr(s) as usize);
        let key_ok = self.land.as_ref().is_some_and(|(m, l, e, _)| *m == self.course.mode && *l == self.course.land && *e == every) && self.land_src == src;
        self.land_src = src;
        if !key_ok {
            let land = bake::land(&self.course, every);
            self.land = Some((self.course.mode, self.course.land.clone(), every, land));
        }
        let land = &self.land.as_ref().unwrap().3;
        let field = bake::bake(&self.course, every, land);
        self.refresh_line();
        if let Some(v) = &mut self.view {
            match &self.course.source {
                // A copy shows its own ground's colours, except where painted.
                Some(src) => {
                    let (n, step) = (field.n, field.step);
                    let surfaces = &field.surfaces;
                    let ground = |k: usize| {
                        let (x, z) = ((k % n) as f32 * step, (k / n) as f32 * step);
                        (surfaces[k] == src.surface(x, z)).then(|| src.colour(x, z))
                    };
                    v.set_field(device, queue, &field, Some(&ground));
                }
                None => v.set_field(device, queue, &field, None),
            }
        }
        // The plants, grown again while nothing is dragged (or now and
        // then while painting them).
        if self.drag == Drag::None || self.now - self.plants_at > 0.4 {
            let species = mm2_course::nature::species(&self.course);
            self.plants = mm2_course::nature::plants(&self.course, &field).iter().map(|p| (p.at, species[p.species as usize].flora)).collect();
            self.plants_at = self.now;
        }
        self.field = Some(field);
        self.dirty = false;
        self.baked_at = self.now;
    }

    fn height(&self, x: f32, z: f32) -> f32 {
        self.field.as_ref().map_or(0.0, |f| f.height(x, z))
    }

    /// Keeps the course as it is for Undo, before a change.
    fn remember(&mut self) {
        self.history.push(self.course.clone());
        if self.history.len() > 300 {
            self.history.remove(0);
        }
        self.future.clear();
    }

    /// After a change: draw it again, and save it soon.
    fn changed(&mut self) {
        self.dirty = true;
        self.unsaved = true;
    }

    fn undo(&mut self) {
        if let Some(c) = self.history.pop() {
            self.future.push(std::mem::replace(&mut self.course, c));
            self.picked = None;
            self.river = None;
            self.flight = None;
            self.changed();
            self.say("Undone");
        }
    }

    fn redo(&mut self) {
        if let Some(c) = self.future.pop() {
            self.history.push(std::mem::replace(&mut self.course, c));
            self.picked = None;
            self.changed();
            self.say("Redone");
        }
    }

    /// A fresh seed for the one-click helpers.
    fn dice(&mut self) -> u32 {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u32).unwrap_or(1);
        t ^ (self.now * 1000.0) as u32
    }

    fn say(&mut self, text: &str) {
        self.toast = Some((text.to_string(), self.now));
    }

    fn course_path(&self) -> PathBuf {
        self.dir.join(format!("{}.course", self.course.stem()))
    }

    /// Writes the `.course` file.
    fn save(&mut self) {
        let path = self.course_path();
        let _ = crate::fs::create_dir_all(&self.dir);
        match crate::fs::write(&path, self.course.to_json()) {
            Ok(()) => {
                let _ = crate::fs::write(self.dir.join("last.txt"), path.to_string_lossy().as_bytes());
                self.unsaved = false;
                self.saved_at = self.now;
            }
            Err(e) => eprintln!("{}: {e}", path.display()),
        }
    }

    /// Where the baked track goes: the game's `TERAFORM/<mode>` folder, or
    /// beside the courses without the game.
    /// The game folder it bakes into (the user's own when none is known).
    pub fn game_dir(&self) -> PathBuf {
        self.game.clone().unwrap_or_else(|| self.dir.clone())
    }

    fn env_path(&self) -> PathBuf {
        let base = match &self.game {
            Some(g) => g.join("TERAFORM"),
            None => self.dir.join("TERAFORM"),
        };
        base.join(self.course.mode.folder()).join(format!("{}.ENV", self.course.stem()))
    }

    /// Saves, then bakes the full track on another thread; with `ride`,
    /// the game rides it once it is written.
    fn bake_track(&mut self, ride: bool) {
        if self.bake_job.is_some() {
            return;
        }
        self.save();
        let course = self.course.clone();
        let path = self.env_path();
        let job = Job::spawn(move || {
            let baked = mm2_course::env::write_with(&course, &crate::fine_ground::builder_pictures());
            if let Some(dir) = path.parent() {
                crate::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            crate::fs::write(&path, &baked.env).map_err(|e| format!("{}: {e}", path.display()))?;
            // In a browser the game folder lasts the visit: the track is
            // kept among the saves too, where the page puts players' tracks
            // back into the game each time it starts.
            #[cfg(target_arch = "wasm32")]
            if let (Some(folder), Some(file)) = (path.parent().and_then(|p| p.file_name()), path.file_name()) {
                let keep = PathBuf::from("/user/Tracks").join(folder).join(file);
                crate::fs::write(&keep, &baked.env).map_err(|e| format!("{}: {e}", keep.display()))?;
            }
            Ok(path)
        });
        self.bake_job = Some((job, ride));
        self.busy_since = self.now;
    }

    /// Makes the track for the original Motocross Madness 2 on another
    /// thread: a browser downloads it, a computer saves it beside the
    /// builder's tracks, in a folder named for the original's.
    fn export_original(&mut self) {
        if self.export_job.is_some() {
            return;
        }
        self.save();
        let course = self.course.clone();
        self.export_job = Some(Job::spawn(move || {
            let env = mm2_course::env::write_original(&course, &crate::fine_ground::builder_pictures())?;
            Ok((format!("{}.ENV", course.stem()), env))
        }));
        self.busy_since = self.now;
        self.say("Making the track for the original game...");
    }

    fn mode_index(&self) -> usize {
        Mode::ALL.iter().position(|m| *m == self.course.mode).unwrap_or(1)
    }

    /// Where an export for the original game goes on a computer.
    #[cfg(not(target_arch = "wasm32"))]
    fn export_dir(&self) -> PathBuf {
        crate::profile::user_dir().join("For original MM2").join("Teraform").join(self.course.mode.name())
    }

    /// An export for the original game that has finished.
    /// Asks the player for a picture (or a model's files).
    fn ask_file(&mut self, what: PickFor) {
        let (title, exts, many): (&str, &[&str], bool) = match what {
            PickFor::Heights => ("A height map picture", &["png", "tga", "raw", "r16", "r8"], false),
            PickFor::Ground | PickFor::Plants => ("A picture", &["png", "tga"], false),
            PickFor::Model => ("A model and its pictures", &["slt", "tga"], true),
        };
        match crate::pick::ask(title, exts, many) {
            Some(files) => self.picked(what, files),
            None => self.picking = Some(what),
        }
    }

    /// The files picked for `what`.
    fn picked(&mut self, what: PickFor, files: Vec<crate::pick::Picked>) {
        use mm2_course::picture::{Embedded, HeightMap, Picture, Shared};
        if what == PickFor::Model {
            self.add_model(files);
            return;
        }
        let Some((name, bytes)) = files.into_iter().next() else {
            return;
        };
        if let Err(e) = Picture::read(&name, &bytes) {
            self.say(&format!("Couldn't read that picture: {e}"));
            return;
        }
        let file = Embedded::new(&name, &bytes);
        self.remember();
        match what {
            PickFor::Heights => {
                let old = self.course.land.heightmap.take().unwrap_or_default();
                let mut m = HeightMap::new(file);
                m.tallest = old.tallest;
                m.mix = old.mix;
                self.course.land.heightmap = Some(m);
                self.say("Height map loaded: the land is raised from it");
            }
            PickFor::Ground => {
                self.course.look.ground = Some(mm2_course::GroundPicture { picture: Shared::new(file), track_on_top: true });
                self.say("Ground picture laid over the land");
            }
            PickFor::Plants => {
                self.course.plants.push(Planting {
                    flora: self.flora,
                    radius: 0.0,
                    density: self.plant_density,
                    clear: false,
                    everywhere: false,
                    dabs: Vec::new(),
                    auto: false,
                    map: Some(Shared::new(file)),
                });
                self.say("Plants grown from the map: white grows thickest");
            }
            PickFor::Model => {}
        }
        self.changed();
    }

    /// The player's own model: its `.slt` and the `.tga` pictures it names.
    fn add_model(&mut self, files: Vec<crate::pick::Picked>) {
        use mm2_course::picture::Embedded;
        let is = |n: &str, e: &str| n.to_ascii_lowercase().ends_with(e);
        let Some((name, slt)) = files.iter().find(|(n, _)| is(n, ".slt")).cloned() else {
            if !files.is_empty() {
                self.say("Pick the model's .slt file, with its .tga pictures");
            }
            return;
        };
        if let Err(e) = mm2_formats::slt::parse(&slt) {
            self.say(&format!("Couldn't read {name}: {e}"));
            return;
        }
        // The pictures it names, from those picked.
        let text = String::from_utf8_lossy(&slt).to_ascii_lowercase();
        let wanted: Vec<String> = text
            .lines()
            .filter_map(|l| l.trim().strip_prefix("texturemap="))
            .map(|t| t.trim().trim_matches('"').rsplit(['\\', '/']).next().unwrap_or("").to_string())
            .filter(|t| !t.is_empty())
            .collect();
        let textures: Vec<Embedded> = files
            .iter()
            .filter(|(n, _)| is(n, ".tga") && (wanted.is_empty() || wanted.contains(&n.to_ascii_lowercase())))
            .map(|(n, b)| Embedded::new(n, b))
            .collect();
        let missing: Vec<&String> = wanted.iter().filter(|w| !textures.iter().any(|t| t.name.eq_ignore_ascii_case(w))).collect();
        self.remember();
        self.course.models.retain(|m| !m.name().eq_ignore_ascii_case(&name));
        self.course.models.push(mm2_course::UserModel { slt: Embedded::new(&name, &slt), textures });
        self.object = OBJECTS.len() + self.course.models.len() - 1;
        self.tool = Tool::Objects;
        self.changed();
        if missing.is_empty() {
            self.say(&format!("{name} added: click the ground to place it"));
        } else {
            let list: Vec<&str> = missing.iter().map(|s| s.as_str()).collect();
            self.say(&format!("{name} added, but these pictures weren't picked: {}", list.join(", ")));
        }
    }

    /// The model the Objects tool places.
    fn object_model(&self) -> String {
        match self.object.checked_sub(OBJECTS.len()) {
            Some(k) => self.course.models.get(k).map_or(OBJECTS[0].1.to_string(), |m| m.name().to_string()),
            None => OBJECTS[self.object.min(OBJECTS.len() - 1)].1.to_string(),
        }
    }

    /// Keeps the ground as the bake colours it as a picture, to paint over
    /// and load back.
    fn save_ground(&mut self) {
        let png = mm2_course::env::ground_png(&self.course, 960);
        let name = format!("{} ground.png", self.course.stem());
        match crate::pick::save(&name, &png) {
            Ok(Some(at)) => self.say(&format!("Saved {at}")),
            Ok(None) => {}
            Err(e) => self.say(&format!("Couldn't save the picture: {e}")),
        }
    }

    fn finish_export(&mut self) {
        if !self.export_job.as_ref().is_some_and(|j| j.is_finished()) {
            return;
        }
        let job = self.export_job.take().unwrap();
        let k = self.mode_index();
        self.took[1][k] = (self.now - self.busy_since).max(1.0);
        let (name, env) = match job.join() {
            Ok(Ok(made)) => made,
            Ok(Err(e)) => {
                eprintln!("making the track for the original game: {e}");
                self.say("Couldn't make the track for the original game");
                return;
            }
            Err(_) => {
                self.say("Couldn't make the track for the original game");
                return;
            }
        };
        let folder = self.course.mode.name();
        #[cfg(target_arch = "wasm32")]
        {
            if crate::web::save_file(&name, &env) {
                self.say(&format!("{name} is in your downloads. Put it in Motocross Madness 2's Teraform\\{folder} folder."));
            } else {
                self.say("Couldn't download the track");
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let dir = self.export_dir();
            let path = dir.join(&name);
            match crate::fs::create_dir_all(&dir).and_then(|()| crate::fs::write(&path, &env)) {
                Ok(()) => {
                    self.say(&format!("Saved {}. Copy it into Motocross Madness 2's Teraform\\{folder} folder.", path.display()));
                    open_folder(&dir);
                }
                Err(e) => {
                    eprintln!("{}: {e}", path.display());
                    self.say("Couldn't save the track for the original game");
                }
            }
        }
    }

    /// What the builder wants the game to do now.
    pub fn take_ask(&mut self) -> Ask {
        self.finish_export();
        if let Some(what) = self.picking {
            if let Some(files) = crate::pick::poll() {
                self.picking = None;
                self.picked(what, files);
            }
        }
        if self.bake_job.as_ref().is_some_and(|(j, _)| j.is_finished()) {
            let (job, ride) = self.bake_job.take().unwrap();
            let k = self.mode_index();
            self.took[0][k] = (self.now - self.busy_since).max(1.0);
            match job.join() {
                Ok(Ok(path)) => {
                    if ride {
                        self.rode = true;
                        return Ask::Ride { env: path, mode: self.course.mode, name: self.course.name.clone(), watch: std::mem::take(&mut self.watch) };
                    }
                    self.say("Saved, and in the track list");
                }
                Ok(Err(e)) => {
                    eprintln!("baking the track: {e}");
                    #[cfg(target_arch = "wasm32")]
                    web_sys::console::error_1(&format!("baking the track: {e}").into());
                    self.say("Couldn't write the track");
                }
                Err(_) => self.say("Couldn't bake the track"),
            }
        }
        self.ask.take().unwrap_or(Ask::Stay)
    }

    /// Back from riding the track.
    pub fn back(&mut self) {
        self.keys.clear();
        self.drag = Drag::None;
        self.input = Input::default();
        self.pad_live = [false; 5];
    }

    /// The pad's left stick, right stick and zoom triggers as the builder
    /// takes them: each reads 0 until it has once been seen back in its
    /// dead zone, so an axis stuck off centre does nothing.
    pub fn pad_axes(&mut self, v: [f32; 5]) -> [f32; 5] {
        let mut out = [0.0; 5];
        for i in 0..5 {
            if v[i].abs() < PAD_DEAD {
                self.pad_live[i] = true;
            }
            if self.pad_live[i] {
                out[i] = v[i];
            }
        }
        out
    }

    // ---- input ----

    /// The pointer moved to `(x, y)` window pixels.
    pub fn mouse_move(&mut self, x: f64, y: f64) {
        let k = 480.0 / self.size.1.max(1) as f32;
        let p = (x as f32 * k, y as f32 * k);
        let (dx, dy) = (p.0 - self.pointer.0, p.1 - self.pointer.1);
        self.pointer = p;
        self.input.pointer = p;
        match self.drag {
            Drag::Turn => {
                self.cam.yaw -= dx * 0.008;
                self.cam.pitch = (self.cam.pitch + dy * 0.006).clamp(0.12, 1.56);
            }
            Drag::Pan => {
                let s = self.cam.dist / 480.0 * 0.9;
                let (sy, cy) = self.cam.yaw.sin_cos();
                // Grab the ground: it moves with the pointer.
                self.cam.target.x -= (dx * cy - dy * sy) * s;
                self.cam.target.z -= (-dx * sy - dy * cy) * s;
                self.clamp_target();
            }
            _ => {}
        }
    }

    fn clamp_target(&mut self) {
        let side = self.course.mode.side();
        self.cam.target.x = self.cam.target.x.clamp(0.0, side);
        self.cam.target.z = self.cam.target.z.clamp(0.0, side);
        self.cam.target.y = self.height(self.cam.target.x, self.cam.target.z);
    }

    fn over_panel(&self) -> bool {
        self.popup.is_some() || self.input.over(self.pointer)
    }

    pub fn mouse_button(&mut self, button: winit::event::MouseButton, down: bool) {
        use winit::event::MouseButton as B;
        if down {
            self.drag_from = self.pointer;
        }
        if self.over_panel() || self.input.held {
            if button == B::Left {
                if down {
                    self.input.press = Some(self.pointer);
                    self.input.held = true;
                } else {
                    self.input.released = true;
                    self.input.held = false;
                }
            }
            if self.drag == Drag::None {
                return;
            }
        }
        if !down {
            self.end_drag();
            return;
        }
        match button {
            B::Right if self.shift => self.drag = Drag::Pan,
            B::Middle => self.drag = Drag::Pan,
            B::Right => {
                // A right click on a thing removes it; on the land, it turns
                // the camera.
                if !self.remove_hovered() {
                    self.drag = Drag::Turn;
                }
            }
            B::Left => self.press(),
            _ => {}
        }
    }

    fn end_drag(&mut self) {
        if matches!(self.drag, Drag::Brush) {
            self.say_saved_soon();
        }
        if !matches!(self.drag, Drag::None | Drag::Turn | Drag::Pan) {
            self.changed();
        }
        self.drag = Drag::None;
    }

    fn say_saved_soon(&mut self) {
        self.unsaved = true;
    }

    /// A gamepad's sticks and buttons (the pointer itself moves through
    /// `mouse_move`): the right stick turns the view, the triggers zoom
    /// (`zoom` + in), `pan` moves the view when the pointer is pushed
    /// against the screen's edge.
    pub fn pad_view(&mut self, right: [f32; 2], zoom: f32, pan: [f32; 2], dt: f32) {
        let dead = |v: f32| if v.abs() < PAD_DEAD { 0.0 } else { v };
        self.cam.yaw -= dead(right[0]) * 2.2 * dt;
        self.cam.pitch = (self.cam.pitch + dead(right[1]) * 1.4 * dt).clamp(0.12, 1.56);
        if dead(zoom) != 0.0 {
            self.cam.dist = (self.cam.dist * 0.87f32.powf(dead(zoom) * dt * 8.0)).clamp(40.0, self.course.mode.side() * 1.3);
        }
        if pan != [0.0, 0.0] {
            let s = self.cam.dist * 0.9 * dt;
            let (sy, cy) = self.cam.yaw.sin_cos();
            self.cam.target.x += (pan[0] * cy - pan[1] * sy) * s;
            self.cam.target.z += (-pan[0] * sy - pan[1] * cy) * s;
            self.clamp_target();
        }
    }

    /// A gamepad button: `Remove` takes off what the pointer is over,
    /// `Tool` steps through the tools.
    pub fn pad_button(&mut self, b: PadButton) {
        match b {
            PadButton::Remove => {
                if !self.over_panel() {
                    self.remove_hovered();
                }
            }
            PadButton::Tool(step) => {
                let i = TOOLS.iter().position(|t| t.0 == self.tool).unwrap_or(0) as i32;
                self.tool = TOOLS[(i + step).rem_euclid(TOOLS.len() as i32) as usize].0;
            }
            PadButton::Undo => self.undo(),
            PadButton::Ride => self.bake_track(true),
            PadButton::Top => self.top_view(),
            PadButton::Back => self.key(winit::keyboard::KeyCode::Escape, true, None),
        }
    }

    /// The pointer in window pixels, for a gamepad to move on from.
    pub fn pointer_px(&self) -> (f64, f64) {
        let k = self.size.1.max(1) as f32 / 480.0;
        ((self.pointer.0 * k) as f64, (self.pointer.1 * k) as f64)
    }

    pub fn wheel(&mut self, lines: f32) {
        if self.over_panel() {
            self.input.wheel += lines;
            return;
        }
        let step = if self.shift { 5.0 } else { 1.0 };
        match (self.tool, self.hover) {
            (Tool::Line, Hover::Point(i)) => {
                self.remember();
                let p = &mut self.course.line.points[i];
                p.width = (p.width + lines * 2.0).clamp(10.0, 120.0);
                self.changed();
            }
            (Tool::Jumps, Hover::Jump(i)) => {
                self.remember();
                let j = &mut self.course.jumps[i];
                if self.shift {
                    j.length = (j.length + lines * 5.0).clamp(10.0, 400.0);
                } else {
                    j.height = (j.height + lines * 0.5).clamp(0.5, 30.0);
                }
                self.picked = Some(i);
                self.changed();
            }
            (Tool::Motion, Hover::Flight(f, i)) => {
                self.remember();
                let p = &mut self.course.flights[f].points[i];
                p[1] = (p[1] + lines * 10.0 * step).clamp(20.0, 1500.0);
                self.flight = Some(f);
                self.changed();
            }
            (Tool::Nature, Hover::River(r, _)) => {
                self.remember();
                let r = &mut self.course.rivers[r];
                r.width = (r.width + lines * 2.0 * step).clamp(6.0, 120.0);
                self.changed();
            }
            (Tool::Objects, Hover::Object(i)) => {
                self.remember();
                self.course.objects[i].yaw = (self.course.objects[i].yaw + lines * 15.0 * step).rem_euclid(360.0);
                self.changed();
            }
            _ => {
                self.cam.dist = (self.cam.dist * 0.87f32.powf(lines)).clamp(40.0, self.course.mode.side() * 1.3);
            }
        }
    }

    /// The course's name while it is being typed (a phone shows its
    /// keyboard for it).
    pub fn typing_text(&self) -> Option<String> {
        self.naming.then(|| if self.name_fresh { String::new() } else { self.course.name.clone() })
    }

    pub fn key(&mut self, code: winit::keyboard::KeyCode, down: bool, text: Option<&str>) {
        use winit::keyboard::KeyCode as K;
        match code {
            K::ShiftLeft | K::ShiftRight => self.shift = down,
            K::ControlLeft | K::ControlRight | K::SuperLeft | K::SuperRight => self.ctrl = down,
            _ => {}
        }
        if !down {
            self.keys.remove(&code);
            return;
        }
        if self.naming {
            match code {
                K::Enter | K::NumpadEnter | K::Escape => {
                    self.naming = false;
                    if self.course.name.trim().is_empty() {
                        self.course.name = self.free_name(self.course.mode);
                    }
                    self.changed();
                }
                K::Backspace => {
                    if self.name_fresh {
                        self.course.name.clear();
                    }
                    self.course.name.pop();
                    self.name_fresh = false;
                }
                _ => {
                    let typed: Vec<char> = text.unwrap_or("").chars().filter(|c| !c.is_control()).collect();
                    if self.name_fresh && !typed.is_empty() {
                        self.course.name.clear();
                        self.name_fresh = false;
                    }
                    for c in typed {
                        if self.course.name.chars().count() < 28 {
                            self.course.name.push(c);
                        }
                    }
                }
            }
            return;
        }
        self.keys.insert(code);
        if self.ctrl {
            match code {
                K::KeyZ if self.shift => self.redo(),
                K::KeyZ => self.undo(),
                K::KeyY => self.redo(),
                K::KeyS => {
                    self.bake_track(false);
                    self.say("Saving...");
                }
                _ => {}
            }
            return;
        }
        match code {
            K::Escape => {
                if self.popup.take().is_none() && self.river.take().is_none() && self.flight.take().is_none() {
                    if self.picked.take().is_none() {
                        self.save();
                        self.ask = Some(Ask::Exit);
                    }
                }
            }
            K::Digit1 => self.tool = Tool::Line,
            K::Digit2 => self.tool = Tool::Jumps,
            K::Digit3 => self.tool = Tool::Shape,
            K::Digit4 => self.tool = Tool::Paint,
            K::Digit5 => self.tool = Tool::Objects,
            K::Digit6 => self.tool = Tool::Nature,
            K::Digit7 => self.tool = Tool::Motion,
            K::Digit8 => self.tool = Tool::Land,
            K::KeyR => self.bake_track(true),
            K::Tab => self.top_view(),
            K::KeyF => self.frame_track(),
            K::Delete | K::Backspace => {
                self.remove_hovered();
            }
            K::BracketLeft => self.radius = (self.radius / 1.2).max(8.0),
            K::BracketRight => self.radius = (self.radius * 1.2).min(400.0),
            K::KeyT => {
                // Set the start on the line where the pointer is.
                if matches!(self.hover, Hover::Line(_) | Hover::Jump(_)) {
                    let s = if let Hover::Line(s) = self.hover { s } else { self.line_at_pointer().unwrap_or(0.0) };
                    self.remember();
                    self.course.line.start = s;
                    self.changed();
                    self.say("Start moved here");
                }
            }
            _ => {}
        }
    }

    fn top_view(&mut self) {
        self.cam.pitch = if self.cam.pitch > 1.5 { 0.95 } else { 1.56 };
    }

    // ---- editing ----

    fn line_at_pointer(&self) -> Option<f32> {
        let g = self.ground?;
        self.line.nearest([g.x, g.z]).map(|n| n.s)
    }

    /// Removes what the pointer is over; true if there was something.
    fn remove_hovered(&mut self) -> bool {
        match self.hover {
            Hover::Point(i) if self.tool == Tool::Line => {
                self.remember();
                self.course.line.points.remove(i);
                self.changed();
                true
            }
            Hover::Jump(i) if self.tool == Tool::Jumps => {
                self.remember();
                self.course.jumps.remove(i);
                self.picked = None;
                self.changed();
                true
            }
            Hover::Object(i) if self.tool == Tool::Objects => {
                self.remember();
                self.course.objects.remove(i);
                self.changed();
                true
            }
            Hover::River(r, i) if self.tool == Tool::Nature => {
                self.remember();
                self.course.rivers[r].points.remove(i);
                if self.course.rivers[r].points.is_empty() {
                    self.course.rivers.remove(r);
                    self.river = None;
                }
                self.changed();
                true
            }
            Hover::Flight(f, i) if self.tool == Tool::Motion => {
                self.remember();
                self.course.flights[f].points.remove(i);
                if self.course.flights[f].points.is_empty() {
                    self.course.flights.remove(f);
                    self.flight = None;
                }
                self.changed();
                true
            }
            _ => false,
        }
    }

    /// The left button went down on the land.
    fn press(&mut self) {
        let Some(g) = self.ground else {
            self.drag = Drag::Turn;
            return;
        };
        match self.tool {
            Tool::Line if self.line_mode == LineMode::Gates && !self.course.mode.laps() => {
                if let Hover::Line(s) = self.hover {
                    self.remember();
                    let near = self.course.line.gates.iter().position(|&at| (at - s).abs() < 30.0);
                    match near {
                        Some(k) => {
                            self.course.line.gates.remove(k);
                        }
                        None => self.course.line.gates.push(s),
                    }
                    self.changed();
                }
            }
            Tool::Line if self.line_mode == LineMode::Start => {
                if self.course.mode.laps() {
                    if let Hover::Line(s) = self.hover {
                        self.remember();
                        self.course.line.start = s;
                        self.changed();
                    }
                } else {
                    // Out in the open, facing the line's start.
                    self.remember();
                    let to = self.line.at(self.course.line.start);
                    let yaw = (to[0] - g.x).atan2(to[1] - g.z).to_degrees();
                    let on_line = self.line.nearest([g.x, g.z]).is_some_and(|n| n.side.abs() < self.line.width[n.i] / 2.0);
                    if on_line {
                        // On the track: an ordinary start there.
                        self.course.line.start_spot = None;
                        if let Hover::Line(s) = self.hover {
                            self.course.line.start = s;
                        }
                    } else {
                        self.course.line.start_spot = Some([g.x, g.z, yaw]);
                    }
                    self.changed();
                }
            }
            Tool::Line => match self.hover {
                Hover::Point(i) => {
                    self.remember();
                    self.drag = Drag::Point(i);
                }
                Hover::Line(_) => {
                    // A new point on the line, where it was clicked.
                    let n = self.line.nearest([g.x, g.z]);
                    let seg = n.map_or(0, |n| self.segment_of(n.i));
                    self.remember();
                    let width = self.course.line.points.get(seg).map_or(self.course.mode.track_width(), |p| p.width);
                    self.course.line.points.insert(seg + 1, Point { x: g.x, z: g.z, width });
                    self.drag = Drag::Point(seg + 1);
                    self.changed();
                }
                _ => {
                    self.remember();
                    let at = self.insert_at(g);
                    let width = self.course.line.points.get(at.saturating_sub(1)).map_or(self.course.mode.track_width(), |p| p.width);
                    self.course.line.points.insert(at, Point { x: g.x, z: g.z, width });
                    self.drag = Drag::Point(at);
                    self.changed();
                }
            },
            Tool::Jumps => match self.hover {
                Hover::Jump(i) => {
                    self.remember();
                    let s = self.line_at_pointer().unwrap_or(0.0);
                    let off = self.line.ahead(self.course.jumps[i].at, s);
                    self.drag = Drag::Jump(i, off);
                    self.picked = Some(i);
                }
                Hover::Line(s) => {
                    self.remember();
                    let mut j = self.kind.make(self.course.mode, 0.0);
                    j.at = s - j.length / 2.0;
                    if self.line.closed {
                        j.at = j.at.rem_euclid(self.line.length.max(1.0));
                    } else {
                        j.at = j.at.max(0.0);
                    }
                    self.course.jumps.push(j);
                    self.picked = Some(self.course.jumps.len() - 1);
                    self.drag = Drag::Jump(self.course.jumps.len() - 1, j.length / 2.0);
                    self.changed();
                }
                _ => self.drag = Drag::Turn,
            },
            Tool::Shape => {
                self.remember();
                self.course.strokes.push(Stroke { brush: self.brush, radius: self.radius, strength: self.strength, target: g.y, dabs: vec![[g.x, g.z]] });
                self.drag = Drag::Brush;
                self.changed();
            }
            Tool::Paint => {
                self.remember();
                self.course.paint.push(Paint { surface: self.surface, radius: self.paint_radius, dabs: vec![[g.x, g.z]] });
                self.drag = Drag::Brush;
                self.changed();
            }
            Tool::Objects => match self.hover {
                Hover::Object(i) => {
                    self.remember();
                    self.drag = Drag::Object(i);
                }
                _ => {
                    self.remember();
                    let yaw = self.line.nearest([g.x, g.z]).map_or(0.0, |n| {
                        let d = self.line.dir[n.i];
                        d[0].atan2(d[1]).to_degrees()
                    });
                    let model = self.object_model();
                    self.course.objects.push(Object { model, x: g.x, z: g.z, yaw, scale: 1.0 });
                    self.drag = Drag::Object(self.course.objects.len() - 1);
                    self.changed();
                }
            },
            Tool::Nature => match (self.nature, self.hover) {
                (NatureMode::River, Hover::River(r, i)) => {
                    self.remember();
                    self.river = Some(r);
                    self.drag = Drag::River(r, i);
                }
                (NatureMode::River, _) => {
                    self.remember();
                    match self.river.filter(|&r| r < self.course.rivers.len()) {
                        Some(r) => self.course.rivers[r].points.push([g.x, g.z]),
                        None => {
                            self.course.rivers.push(River { points: vec![[g.x, g.z]], width: 28.0, depth: 2.5 });
                            self.river = Some(self.course.rivers.len() - 1);
                        }
                    }
                    let r = self.river.unwrap();
                    self.drag = Drag::River(r, self.course.rivers[r].points.len() - 1);
                    self.changed();
                }
                _ => {
                    self.remember();
                    let clear = self.nature == NatureMode::Clear;
                    self.course.plants.push(Planting { flora: self.flora, radius: self.plant_radius, density: self.plant_density, clear, everywhere: false, dabs: vec![[g.x, g.z]], auto: false, map: None });
                    self.drag = Drag::Brush;
                    self.changed();
                }
            },
            Tool::Motion => match self.hover {
                Hover::Flight(f, i) => {
                    self.remember();
                    self.flight = Some(f);
                    self.craft = self.course.flights[f].craft;
                    self.drag = Drag::Flight(f, i);
                }
                _ => {
                    self.remember();
                    let height = self.craft.height();
                    match self.flight.filter(|&f| f < self.course.flights.len()) {
                        Some(f) => {
                            // After the point nearest, or at the end.
                            let at = self.flight_insert(f, g);
                            self.course.flights[f].points.insert(at, [g.x, height, g.z]);
                            self.drag = Drag::Flight(f, at);
                        }
                        None => {
                            self.course.flights.push(Flight { craft: self.craft, points: vec![[g.x, height, g.z]], speed: self.craft.speed() });
                            let f = self.course.flights.len() - 1;
                            self.flight = Some(f);
                            self.drag = Drag::Flight(f, 0);
                        }
                    }
                    self.changed();
                }
            },
            Tool::Land => self.drag = Drag::Turn,
        }
    }

    /// Where a new point at `g` goes in flight `f`'s loop: where it
    /// lengthens the loop least.
    fn flight_insert(&self, f: usize, g: Vec3) -> usize {
        let pts = &self.course.flights[f].points;
        if pts.len() < 3 {
            return pts.len();
        }
        let d = |a: [f32; 3], b: [f32; 3]| ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
        let q = [g.x, 0.0, g.z];
        (0..pts.len())
            .min_by(|&i, &j| {
                let cost = |k: usize| d(pts[k], q) + d(q, pts[(k + 1) % pts.len()]) - d(pts[k], pts[(k + 1) % pts.len()]);
                cost(i).total_cmp(&cost(j))
            })
            .map_or(pts.len(), |k| k + 1)
    }

    /// Which of the player's points comes before path sample `i`.
    fn segment_of(&self, i: usize) -> usize {
        let at = &self.line.at_point;
        let mut seg = 0;
        for (k, &start) in at.iter().enumerate() {
            if start <= i {
                seg = k;
            }
        }
        seg
    }

    /// Where a new point at `g` goes in the list: at the end of an open
    /// line, or into a loop where it lengthens it least.
    fn insert_at(&self, g: Vec3) -> usize {
        let pts = &self.course.line.points;
        if !self.course.line.closed || pts.len() < 3 {
            return pts.len();
        }
        let d = |a: &Point, b: &Point| ((a.x - b.x).powi(2) + (a.z - b.z).powi(2)).sqrt();
        let q = Point { x: g.x, z: g.z, width: 0.0 };
        (0..pts.len())
            .min_by(|&i, &j| {
                let cost = |k: usize| {
                    let (a, b) = (&pts[k], &pts[(k + 1) % pts.len()]);
                    d(a, &q) + d(&q, b) - d(a, b)
                };
                cost(i).total_cmp(&cost(j))
            })
            .map_or(pts.len(), |k| k + 1)
    }

    /// Drags in progress follow the pointer.
    fn follow_drag(&mut self) {
        let Some(g) = self.ground else { return };
        match self.drag {
            Drag::Point(i) => {
                if let Some(p) = self.course.line.points.get_mut(i) {
                    if (p.x - g.x).abs() + (p.z - g.z).abs() > 0.2 {
                        p.x = g.x;
                        p.z = g.z;
                        self.dirty = true;
                    }
                }
            }
            Drag::Jump(i, off) => {
                if let (Some(n), true) = (self.line.nearest([g.x, g.z]), i < self.course.jumps.len()) {
                    let mut at = n.s - off;
                    if self.line.closed {
                        at = at.rem_euclid(self.line.length.max(1.0));
                    }
                    if (self.course.jumps[i].at - at).abs() > 0.3 {
                        self.course.jumps[i].at = at;
                        self.dirty = true;
                    }
                }
            }
            Drag::Object(i) => {
                if let Some(o) = self.course.objects.get_mut(i) {
                    o.x = g.x;
                    o.z = g.z;
                    self.dirty = true;
                }
            }
            Drag::River(r, i) => {
                if let Some(p) = self.course.rivers.get_mut(r).and_then(|r| r.points.get_mut(i)) {
                    if (p[0] - g.x).abs() + (p[1] - g.z).abs() > 0.5 {
                        *p = [g.x, g.z];
                        self.dirty = true;
                    }
                }
            }
            Drag::Flight(f, i) => {
                if let Some(p) = self.course.flights.get_mut(f).and_then(|f| f.points.get_mut(i)) {
                    p[0] = g.x;
                    p[2] = g.z;
                }
            }
            Drag::Brush => {
                let spacing = |r: f32| (r * 0.25).max(2.0);
                match self.tool {
                    Tool::Shape => {
                        if let Some(s) = self.course.strokes.last_mut() {
                            let last = *s.dabs.last().unwrap();
                            if ((last[0] - g.x).powi(2) + (last[1] - g.z).powi(2)).sqrt() >= spacing(s.radius) {
                                s.dabs.push([g.x, g.z]);
                                self.dirty = true;
                            }
                        }
                    }
                    Tool::Paint => {
                        if let Some(s) = self.course.paint.last_mut() {
                            let last = *s.dabs.last().unwrap();
                            if ((last[0] - g.x).powi(2) + (last[1] - g.z).powi(2)).sqrt() >= spacing(s.radius) {
                                s.dabs.push([g.x, g.z]);
                                self.dirty = true;
                            }
                        }
                    }
                    Tool::Nature => {
                        if let Some(s) = self.course.plants.last_mut() {
                            let last = *s.dabs.last().unwrap();
                            if ((last[0] - g.x).powi(2) + (last[1] - g.z).powi(2)).sqrt() >= spacing(s.radius) {
                                s.dabs.push([g.x, g.z]);
                                self.dirty = true;
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    // ---- each frame ----

    /// Moves the camera by the held keys, finds what the pointer is over
    /// and follows drags.
    pub fn update(&mut self, dt: f32, now: f32) {
        use winit::keyboard::KeyCode as K;
        self.now = now;
        if self.copy_job.as_ref().is_some_and(|j| j.is_finished()) {
            match self.copy_job.take().unwrap().join() {
                Ok(Ok(c)) => {
                    self.open_copy(c);
                    self.say("Built. This is your copy to change");
                }
                Ok(Err(e)) => {
                    eprintln!("copying the track: {e}");
                    self.say("That track can't be copied");
                }
                Err(_) => self.say("That track can't be copied"),
            }
        }
        let keys = self.keys.clone();
        let held = |k: K| keys.contains(&k);
        let speed = self.cam.dist * if self.shift { 2.4 } else { 1.0 } * dt;
        let (mut fx, mut fz) = (0.0, 0.0);
        if held(K::KeyW) || held(K::ArrowUp) {
            fz -= 1.0;
        }
        if held(K::KeyS) || held(K::ArrowDown) {
            fz += 1.0;
        }
        if held(K::KeyA) || held(K::ArrowLeft) {
            fx -= 1.0;
        }
        if held(K::KeyD) || held(K::ArrowRight) {
            fx += 1.0;
        }
        if fx != 0.0 || fz != 0.0 {
            let (sy, cy) = self.cam.yaw.sin_cos();
            self.cam.target.x += (fx * cy + fz * sy) * speed;
            self.cam.target.z += (-fx * sy + fz * cy) * speed;
            self.clamp_target();
        }
        if held(K::KeyQ) {
            self.cam.yaw += dt * 1.5;
        }
        if held(K::KeyE) {
            self.cam.yaw -= dt * 1.5;
        }
        self.ground = if self.over_panel() { None } else { self.pick() };
        self.follow_drag();
        if self.drag == Drag::None {
            self.hover = self.find_hover();
        }
        // Saves on its own a moment after a change.
        if self.unsaved && self.drag == Drag::None && now - self.saved_at > 3.0 {
            self.save();
        }
    }

    fn aspect(&self) -> f32 {
        self.size.0 as f32 / self.size.1.max(1) as f32
    }

    /// The land under the pointer.
    fn pick(&self) -> Option<Vec3> {
        let field = self.field.as_ref()?;
        let vp = self.cam.view_proj(self.aspect());
        let inv = vp.inverse();
        let nx = self.pointer.0 / self.width() * 2.0 - 1.0;
        let ny = 1.0 - self.pointer.1 / 480.0 * 2.0;
        let a = inv * Vec4::new(nx, ny, 0.0, 1.0);
        let b = inv * Vec4::new(nx, ny, 1.0, 1.0);
        let (a, b) = (a.truncate() / a.w, b.truncate() / b.w);
        // Back to the game's coordinates.
        let g = |v: Vec3| Vec3::new(v.x, v.y, -v.z);
        let (o, dir) = (g(a), (g(b) - g(a)).normalize());
        let side = self.course.mode.side();
        let step = field.step * 0.75;
        let mut t = 0.0;
        let mut prev = o;
        let far = self.cam.dist * 8.0 + side;
        while t < far {
            let p = o + dir * t;
            if p.x >= -1.0 && p.z >= -1.0 && p.x <= side + 1.0 && p.z <= side + 1.0 && p.y <= field.height(p.x, p.z) {
                // Halve the last step a few times.
                let (mut lo, mut hi) = (prev, p);
                for _ in 0..8 {
                    let m = (lo + hi) * 0.5;
                    if m.y <= field.height(m.x, m.z) {
                        hi = m;
                    } else {
                        lo = m;
                    }
                }
                return Some(Vec3::new(hi.x.clamp(0.0, side), field.height(hi.x, hi.z), hi.z.clamp(0.0, side)));
            }
            prev = p;
            t += step.max(t * 0.004);
        }
        None
    }

    /// Where a game position falls on the builder's screen, if in front.
    fn project(&self, vp: &Mat4, p: Vec3) -> Option<[f32; 2]> {
        let c = *vp * Vec4::new(p.x, p.y, -p.z, 1.0);
        if c.w <= 0.1 {
            return None;
        }
        let (x, y) = (c.x / c.w, c.y / c.w);
        Some([(x + 1.0) / 2.0 * self.width(), (1.0 - y) / 2.0 * 480.0])
    }

    fn find_hover(&self) -> Hover {
        let Some(g) = self.ground else { return Hover::None };
        let vp = self.cam.view_proj(self.aspect());
        let near_px = |p: Vec3, r: f32| {
            self.project(&vp, p).is_some_and(|s| (s[0] - self.pointer.0).powi(2) + (s[1] - self.pointer.1).powi(2) < r * r)
        };
        match self.tool {
            Tool::Line => {
                for (i, p) in self.course.line.points.iter().enumerate() {
                    if near_px(Vec3::new(p.x, self.height(p.x, p.z), p.z), 9.0) {
                        return Hover::Point(i);
                    }
                }
            }
            Tool::Nature if self.nature == NatureMode::River => {
                for (r, river) in self.course.rivers.iter().enumerate() {
                    for (i, p) in river.points.iter().enumerate() {
                        if near_px(Vec3::new(p[0], self.height(p[0], p[1]), p[1]), 9.0) {
                            return Hover::River(r, i);
                        }
                    }
                }
                return Hover::Ground;
            }
            Tool::Nature => return Hover::Ground,
            Tool::Motion => {
                for (f, flight) in self.course.flights.iter().enumerate() {
                    for (i, p) in flight.points.iter().enumerate() {
                        if near_px(Vec3::new(p[0], self.height(p[0], p[2]) + p[1], p[2]), 10.0) {
                            return Hover::Flight(f, i);
                        }
                    }
                }
                return Hover::Ground;
            }
            Tool::Objects => {
                for (i, o) in self.course.objects.iter().enumerate() {
                    if near_px(Vec3::new(o.x, self.height(o.x, o.z), o.z), 9.0) {
                        return Hover::Object(i);
                    }
                }
                return Hover::Ground;
            }
            _ => {}
        }
        if let Some(n) = self.line.nearest([g.x, g.z]) {
            let hw = self.line.width[n.i] / 2.0;
            if n.side.abs() <= hw + 4.0 {
                if self.tool == Tool::Jumps {
                    for (i, j) in self.course.jumps.iter().enumerate() {
                        let ds = self.line.ahead(j.at, n.s);
                        if ds >= 0.0 && ds <= j.length {
                            return Hover::Jump(i);
                        }
                    }
                }
                return Hover::Line(n.s);
            }
        }
        Hover::Ground
    }

    // ---- drawing ----

    /// Draws the builder over `target`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sprites: &mut crate::sprites::Sprites,
        format: wgpu::TextureFormat,
        target: &wgpu::TextureView,
        size: (u32, u32),
        game: &Path,
    ) -> Vec<wgpu::CommandBuffer> {
        self.size = size;
        if self.view.is_none() {
            self.view = Some(view::View::new(device, format));
            self.dirty = true;
        }
        if self.field.is_none() {
            self.dirty = true;
            self.drag = Drag::None;
        }
        let first = self.field.is_none();
        self.rebake(device, queue);
        if first {
            self.frame_track();
        }
        let vp = self.cam.view_proj(self.aspect());
        let ring = match (self.tool, self.ground) {
            (Tool::Shape, Some(g)) => Some(([g.x, g.z], self.radius, [1.0, 1.0, 1.0])),
            (Tool::Paint, Some(g)) => Some(([g.x, g.z], self.paint_radius, view::surface_colour(self.surface).map(|v| (v * 1.6).min(1.0)))),
            (Tool::Nature, Some(g)) if self.nature != NatureMode::River => Some(([g.x, g.z], self.plant_radius, if self.nature == NatureMode::Clear { [1.0, 0.55, 0.4] } else { [0.55, 0.95, 0.4] })),
            _ => None,
        };
        let sky = [0.62, 0.70, 0.80];
        let land = self.view.as_mut().unwrap().render(device, queue, target, size, vp, {
            let e = self.cam.eye();
            Vec3::new(e.x, e.y, -e.z)
        }, ring, sky);

        sprites.scale = size.1 as f32 / 480.0;
        sprites.stretch = true;
        // The last builder's pictures and texts, still on the GPU, rather
        // than a new set each time the builder opens.
        let mut ui = self
            .ui
            .take()
            .or_else(|| KEPT_UI.take().filter(|u| u.game() == game))
            .unwrap_or_else(|| ui::Ui::new(game));
        {
            let mut gfx = Gfx { sprites, device, queue };
            let mut input = std::mem::take(&mut self.input);
            input.begin();
            let mut p = Painter::new(&mut ui, &mut gfx, self.width());
            p.hide = input.panel_rects();
            self.draw_marks(&mut p, &vp);
            p.hide.clear();
            self.draw_panels(&mut p, &mut input);
            input.end(&mut p, self.now);
            self.input = input;
        }
        self.ui = Some(ui);
        let mut encoder = device.create_command_encoder(&Default::default());
        sprites.render(device, &mut encoder, target, size, false);
        sprites.stretch = false;
        vec![land, encoder.finish()]
    }

    /// The line, its points, the jumps, the start, gates and objects, over
    /// the land.
    fn draw_marks(&self, p: &mut Painter, vp: &Mat4) {
        let line = &self.line;
        if line.is_empty() && self.course.line.points.len() < 2 {
            for (i, pt) in self.course.line.points.iter().enumerate() {
                if let Some(s) = self.project(vp, Vec3::new(pt.x, self.height(pt.x, pt.z) + 1.0, pt.z)) {
                    self.handle(p, s, self.hover == Hover::Point(i));
                }
            }
            return;
        }
        let at = |s: usize| {
            let q = line.pts[s];
            Vec3::new(q[0], self.height(q[0], q[1]) + 0.8, q[1])
        };
        // The centre line, every few samples.
        let every = ((self.cam.dist / 300.0) as usize).clamp(1, 8);
        let n = line.pts.len();
        let mut prev: Option<[f32; 2]> = None;
        let count = if line.closed { n + 1 } else { n };
        let mut k = 0;
        while k < count {
            let i = k % n;
            let s = self.project(vp, at(i));
            if let (Some(a), Some(b)) = (prev, s) {
                p.line(a, b, 1.3, LINE_COLOUR);
            }
            prev = s;
            k += every;
            if k >= count && k - every != count - 1 {
                k = count - 1;
                if line.closed {
                    let s = self.project(vp, at(0));
                    if let (Some(a), Some(b)) = (prev, s) {
                        p.line(a, b, 1.3, LINE_COLOUR);
                    }
                    break;
                }
            }
        }
        // Arrows showing the way round.
        let arrow_every = (self.cam.dist / 4.0).clamp(80.0, 600.0);
        let mut s = arrow_every / 2.0;
        while s < line.length {
            let i = line.index_at(s);
            let d = line.dir[i];
            let side = line.side(i);
            let c = at(i);
            let tip = c + Vec3::new(d[0], 0.0, d[1]) * 6.0;
            let l = c + Vec3::new(-d[0] * 4.0 + side[0] * 5.0, 0.0, -d[1] * 4.0 + side[1] * 5.0);
            let r = c + Vec3::new(-d[0] * 4.0 - side[0] * 5.0, 0.0, -d[1] * 4.0 - side[1] * 5.0);
            if let (Some(t), Some(l), Some(r)) = (self.project(vp, tip), self.project(vp, l), self.project(vp, r)) {
                p.line(l, t, 1.6, [1.0, 1.0, 1.0, 0.5]);
                p.line(r, t, 1.6, [1.0, 1.0, 1.0, 0.5]);
            }
            s += arrow_every;
        }
        // Jumps: their stretch of line in amber, with their name.
        for (ji, j) in self.course.jumps.iter().enumerate() {
            let hot = self.tool == Tool::Jumps && (self.hover == Hover::Jump(ji) || self.picked == Some(ji));
            let steps = (j.length / 4.0).ceil().max(2.0) as usize;
            let mut prev: Option<[f32; 2]> = None;
            for k in 0..=steps {
                let s = j.at + j.length * k as f32 / steps as f32;
                let i = line.index_at(s);
                let q = line.pts[i];
                let y = self.height(q[0], q[1]) + 1.0;
                let hw = line.width[i] / 2.0;
                let side = line.side(i);
                let here = self.project(vp, Vec3::new(q[0], y, q[1]));
                if let (Some(a), Some(b)) = (prev, here) {
                    p.line(a, b, if hot { 4.0 } else { 2.6 }, JUMP_COLOUR);
                }
                if k == 0 || k == steps {
                    let e1 = self.project(vp, Vec3::new(q[0] + side[0] * hw, y, q[1] + side[1] * hw));
                    let e2 = self.project(vp, Vec3::new(q[0] - side[0] * hw, y, q[1] - side[1] * hw));
                    if let (Some(a), Some(b)) = (e1, e2) {
                        p.line(a, b, 1.5, JUMP_COLOUR);
                    }
                }
                prev = here;
            }
            if self.tool == Tool::Jumps || hot {
                let mid = line.at(j.at + j.length / 2.0);
                if let Some(s) = self.project(vp, Vec3::new(mid[0], self.height(mid[0], mid[1]) + j.height + 4.0, mid[1])) {
                    let label = if hot { format!("{} {:.0} ft high, {:.0} long", j.kind.name(), j.height, j.length) } else { j.kind.name().to_string() };
                    p.label(s, &label, if hot { [1.0, 0.85, 0.5, 1.0] } else { [1.0, 1.0, 1.0, 0.9] });
                }
            }
        }
        // The start line.
        {
            let i = line.index_at(self.course.line.start);
            let (q, side, hw) = (line.pts[i], line.side(i), line.width[i] / 2.0);
            let y = self.height(q[0], q[1]) + 1.0;
            let e1 = self.project(vp, Vec3::new(q[0] + side[0] * hw, y, q[1] + side[1] * hw));
            let e2 = self.project(vp, Vec3::new(q[0] - side[0] * hw, y, q[1] - side[1] * hw));
            if let (Some(a), Some(b)) = (e1, e2) {
                p.line(a, b, 3.0, [ACCENT[0], ACCENT[1], ACCENT[2], 1.0]);
                p.label([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0 - 10.0], if self.course.mode.laps() { "START / FINISH" } else { "START" }, [0.85, 1.0, 0.6, 1.0]);
            }
        }
        // Gates for Baja and Enduro.
        for (k, (g, d)) in self.gates.iter().enumerate() {
            let side = [-d[1], d[0]];
            let y = self.height(g[0], g[1]);
            for s in [-1.0, 1.0] {
                let post = Vec3::new(g[0] + side[0] * 18.0 * s, y, g[1] + side[1] * 18.0 * s);
                if let (Some(a), Some(b)) = (self.project(vp, post), self.project(vp, post + Vec3::Y * 20.0)) {
                    p.line(a, b, 2.0, [0.95, 0.35, 0.25, 1.0]);
                }
            }
            if let Some(top) = self.project(vp, Vec3::new(g[0], y + 24.0, g[1])) {
                let last = k + 1 == self.gates.len();
                p.label(top, &if last { "FINISH".to_string() } else { format!("GATE {}", k + 1) }, [1.0, 0.75, 0.7, 1.0]);
            }
        }
        // A funnel start out in the open.
        if let Some([x, z, yaw]) = self.course.line.start_spot.filter(|_| !self.course.mode.laps()) {
            let y = self.height(x, z) + 1.0;
            let (sn, cs) = yaw.to_radians().sin_cos();
            let to = line.at(self.course.line.start);
            if let (Some(a), Some(b)) = (self.project(vp, Vec3::new(x, y, z)), self.project(vp, Vec3::new(to[0], self.height(to[0], to[1]) + 1.0, to[1]))) {
                p.line(a, b, 1.5, [ACCENT[0], ACCENT[1], ACCENT[2], 0.6]);
            }
            for s in [-1.0f32, 1.0] {
                let e = Vec3::new(x - cs * 30.0 * s, y, z + sn * 30.0 * s);
                if let (Some(a), Some(b)) = (self.project(vp, Vec3::new(x, y, z)), self.project(vp, e)) {
                    p.line(a, b, 3.0, [ACCENT[0], ACCENT[1], ACCENT[2], 1.0]);
                }
            }
            if let Some(a) = self.project(vp, Vec3::new(x, y + 6.0, z)) {
                p.label([a[0], a[1] - 10.0], "FUNNEL START", [0.85, 1.0, 0.6, 1.0]);
            }
        }
        // The stadium's floor, which a Supercross track stays on.
        if self.course.mode == Mode::Supercross {
            let (lo, hi) = (FLOOR.0, FLOOR.1);
            let corners = [[lo[0], lo[1]], [hi[0], lo[1]], [hi[0], hi[1]], [lo[0], hi[1]]];
            let at = |c: [f32; 2]| self.project(vp, Vec3::new(c[0], self.height(c[0], c[1]) + 0.5, c[1]));
            for k in 0..4 {
                if let (Some(a), Some(b)) = (at(corners[k]), at(corners[(k + 1) % 4])) {
                    p.line(a, b, 1.5, [1.0, 1.0, 1.0, 0.35]);
                }
            }
            if let Some(a) = at([(lo[0] + hi[0]) / 2.0, hi[1]]) {
                p.label([a[0], a[1] - 10.0], "STADIUM FLOOR", [1.0, 1.0, 1.0, 0.7]);
            }
        }
        // The player's points.
        if self.tool == Tool::Line {
            for (i, pt) in self.course.line.points.iter().enumerate() {
                if let Some(s) = self.project(vp, Vec3::new(pt.x, self.height(pt.x, pt.z) + 1.0, pt.z)) {
                    self.handle(p, s, self.hover == Hover::Point(i) || self.drag == Drag::Point(i));
                }
            }
            if let Hover::Point(i) = self.hover {
                let pt = self.course.line.points[i];
                if let Some(s) = self.project(vp, Vec3::new(pt.x, self.height(pt.x, pt.z) + 1.0, pt.z)) {
                    p.label([s[0], s[1] - 14.0], &format!("{:.0} ft wide", pt.width), [1.0, 1.0, 1.0, 1.0]);
                }
            }
        }
        // Objects.
        for (i, o) in self.course.objects.iter().enumerate() {
            if let Some(s) = self.project(vp, Vec3::new(o.x, self.height(o.x, o.z) + 2.0, o.z)) {
                let hot = self.hover == Hover::Object(i) || self.drag == Drag::Object(i);
                let r = if hot { 5.0 } else { 3.5 };
                p.fill([s[0] - r, s[1] - r, r * 2.0, r * 2.0], if hot { [ACCENT[0], ACCENT[1], ACCENT[2], 1.0] } else { [0.95, 0.9, 0.75, 0.95] });
                if hot || self.tool == Tool::Objects {
                    let name = OBJECTS.iter().find(|(_, m, _)| m.eq_ignore_ascii_case(&o.model)).map_or(o.model.as_str(), |(n, _, _)| n);
                    p.label([s[0], s[1] - 12.0], name, [1.0, 1.0, 1.0, 0.9]);
                }
            }
        }
        self.draw_nature(p, vp);
        self.draw_flights(p, vp);
    }

    /// The plants as dots, and the rivers' points.
    fn draw_nature(&self, p: &mut Painter, vp: &Mat4) {
        // At most a few thousand dots, spread over them all.
        let every = (self.plants.len() / 5000).max(1);
        for (at, flora) in self.plants.iter().step_by(every) {
            let Some(s) = self.project(vp, Vec3::new(at[0], at[1] + 1.0, at[2])) else { continue };
            let (r, c) = match flora {
                Flora::Trees => (2.2, [0.13, 0.32, 0.12, 0.95]),
                Flora::Bushes => (1.5, [0.30, 0.45, 0.16, 0.9]),
                _ => (1.0, [0.62, 0.72, 0.35, 0.8]),
            };
            p.fill([s[0] - r, s[1] - r, r * 2.0, r * 2.0], c);
        }
        if self.tool != Tool::Nature || self.nature != NatureMode::River {
            return;
        }
        for (ri, river) in self.course.rivers.iter().enumerate() {
            let line = mm2_course::nature::river_line(river);
            let mut prev: Option<[f32; 2]> = None;
            for q in line.iter().step_by(4) {
                let s = self.project(vp, Vec3::new(q[0], self.height(q[0], q[1]) + 1.0, q[1]));
                if let (Some(a), Some(b)) = (prev, s) {
                    p.line(a, b, 1.5, [0.55, 0.8, 1.0, 0.8]);
                }
                prev = s;
            }
            for (i, q) in river.points.iter().enumerate() {
                if let Some(s) = self.project(vp, Vec3::new(q[0], self.height(q[0], q[1]) + 1.0, q[1])) {
                    self.handle(p, s, self.hover == Hover::River(ri, i) || self.drag == Drag::River(ri, i) || (self.river == Some(ri) && i + 1 == river.points.len()));
                }
            }
        }
    }

    /// Each flight's loop in the sky, its points, and the craft flying it.
    fn draw_flights(&self, p: &mut Painter, vp: &Mat4) {
        let ground = |x: f32, z: f32| self.height(x, z);
        let showing = self.tool == Tool::Motion;
        for (fi, f) in self.course.flights.iter().enumerate() {
            let route = mm2_course::flight::Route::new(mm2_course::flight::path(f, &ground));
            let picked = self.flight == Some(fi);
            if showing && route.length > 10.0 {
                let pts = mm2_course::flight::path(f, &ground);
                let step = (pts.len() / 160).max(1);
                let mut prev: Option<[f32; 2]> = None;
                let first = pts.first().and_then(|q| self.project(vp, Vec3::from(*q)));
                for q in pts.iter().step_by(step) {
                    let s = self.project(vp, Vec3::from(*q));
                    if let (Some(a), Some(b)) = (prev, s) {
                        p.line(a, b, if picked { 1.8 } else { 1.2 }, [0.75, 0.88, 1.0, if picked { 0.85 } else { 0.5 }]);
                    }
                    prev = s;
                }
                if let (Some(a), Some(b)) = (prev, first) {
                    p.line(a, b, 1.2, [0.75, 0.88, 1.0, 0.5]);
                }
                for (i, q) in f.points.iter().enumerate() {
                    let top = Vec3::new(q[0], ground(q[0], q[2]) + q[1], q[2]);
                    let foot = Vec3::new(q[0], ground(q[0], q[2]), q[2]);
                    if let (Some(a), Some(b)) = (self.project(vp, top), self.project(vp, foot)) {
                        p.line(a, b, 1.0, [1.0, 1.0, 1.0, 0.35]);
                        let hot = self.hover == Hover::Flight(fi, i) || self.drag == Drag::Flight(fi, i);
                        self.handle(p, a, hot);
                        if hot {
                            p.label([a[0], a[1] - 14.0], &format!("{:.0} ft up", q[1]), [1.0, 1.0, 1.0, 1.0]);
                        }
                    }
                }
            }
            // The craft itself, going round as it will in the race.
            if route.length > 10.0 {
                let pose = route.pose(f.craft, self.now * f.speed, f.speed);
                let at = Vec3::from(pose.at);
                let look = Vec3::from(pose.look);
                let side = look.cross(Vec3::Y).normalize_or_zero();
                let size = match f.craft {
                    Craft::Blimp => 60.0,
                    Craft::CargoPlane => 50.0,
                    _ => 22.0,
                };
                let (nose, l, r) = (at + look * size, at - look * size * 0.6 + side * size * 0.7, at - look * size * 0.6 - side * size * 0.7);
                if let (Some(n), Some(a), Some(b)) = (self.project(vp, nose), self.project(vp, l), self.project(vp, r)) {
                    let c = [1.0, 0.9, 0.55, 1.0];
                    p.line(a, n, 2.2, c);
                    p.line(b, n, 2.2, c);
                    p.line(a, b, 2.2, c);
                    if showing {
                        p.label([n[0], n[1] - 12.0], f.craft.name(), [1.0, 0.95, 0.8, 1.0]);
                    }
                }
            }
        }
    }

    fn handle(&self, p: &mut Painter, s: [f32; 2], hot: bool) {
        let r = if hot { 5.5 } else { 4.0 };
        p.fill([s[0] - r - 1.0, s[1] - r - 1.0, r * 2.0 + 2.0, r * 2.0 + 2.0], [0.05, 0.05, 0.05, 0.9]);
        let c = if hot { [ACCENT[0], ACCENT[1], ACCENT[2], 1.0] } else { [1.0, 1.0, 1.0, 1.0] };
        p.fill([s[0] - r, s[1] - r, r * 2.0, r * 2.0], c);
    }

    fn hint(&self) -> String {
        if self.naming {
            return "Type the track's name, then Enter.".into();
        }
        let common = "Right-drag turns the view, middle-drag or WASD moves, wheel zooms, Tab looks straight down.";
        let what = match (self.tool, self.hover) {
            (Tool::Line, _) if self.line_mode == LineMode::Gates && !self.course.mode.laps() => "Click the track to add a gate there, or click a gate to take it away. The finish stays at the end.",
            (Tool::Line, _) if self.line_mode == LineMode::Start && self.course.mode.laps() => "Click the track to put the start and finish line there.",
            (Tool::Line, _) if self.line_mode == LineMode::Start => "Click the track to start there, or open ground beside it for a funnel start.",
            (Tool::Line, Hover::Point(_)) => "Drag to move this point. Wheel: track width here. Right-click or Delete removes it.",
            (Tool::Line, Hover::Line(_)) => "Click to add a point here and drag it. T moves the start here.",
            (Tool::Line, _) => "Click the ground to add a point to the line.",
            (Tool::Jumps, Hover::Jump(_)) => "Drag along the line to move it. Wheel: height, Shift+wheel: length. Right-click removes it.",
            (Tool::Jumps, Hover::Line(_)) => "Click to drop the chosen jump here.",
            (Tool::Jumps, _) => "Pick a jump on the left, then click the line to drop it.",
            (Tool::Shape, _) => "Drag over the ground to shape it. [ and ] change the brush size.",
            (Tool::Paint, _) => "Drag over the ground to paint it. Each surface grips and kicks up dust its own way.",
            (Tool::Objects, Hover::Object(_)) => "Drag to move it. Wheel turns it. Right-click removes it.",
            (Tool::Objects, _) => "Pick an object on the left, then click the ground to place it.",
            (Tool::Nature, Hover::River(..)) => "Drag to move this point of the river. Wheel: how wide the river is. Right-click removes the point.",
            (Tool::Nature, _) if self.nature == NatureMode::River => "Click along the ground to draw a river from its start to its end. It runs downhill and cuts its own bed.",
            (Tool::Nature, _) if self.nature == NatureMode::Clear => "Drag over the ground to take plants away.",
            (Tool::Nature, _) => "Drag over the ground to plant it. They never grow on the track.",
            (Tool::Motion, Hover::Flight(..)) => "Drag to move this point. Wheel: how high it flies here (Shift: faster). Right-click removes it.",
            (Tool::Motion, _) => "Click over the land to add points to the flight path. It flies round them in a loop.",
            (Tool::Land, _) => "Choose the land, sky and sun. Changing the land keeps your line and jumps.",
        };
        format!("{what}   {common}")
    }

    fn draw_panels(&mut self, p: &mut Painter, input: &mut Input) {
        let w = self.width();
        // ---- top bar ----
        p.panel(input, [0.0, 0.0, w, 26.0]);
        // The title, where there is room for it beside the name.
        let name_x = if w >= 740.0 {
            p.text([10.0, 7.0], "COURSE BUILDER", 13.0, true, [ACCENT[0], ACCENT[1], ACCENT[2], 1.0]);
            10.0 + p.text_width("COURSE BUILDER", 13.0, true) + 14.0
        } else {
            8.0
        };
        // The kind of race, a button of its own to change it.
        let kind = format!("Race: {}", self.course.mode.name());
        let kw = p.text_width(&kind, 12.0, false) + 14.0;
        // Room up to the buttons on the right; a long name is cut short
        // (while typing, its start, so what is typed shows).
        let room = w - 8.0 - 384.0 - name_x - 6.0 - kw - 4.0;
        let name = if self.naming && self.name_fresh {
            format!("[{}]", self.course.name)
        } else if self.naming {
            format!("{}_", self.course.name)
        } else {
            self.course.name.clone()
        };
        let mut label = name.clone();
        let fits = |p: &mut Painter, l: &str| p.text_width(l, 12.0, false) + 12.0 <= room;
        if !fits(p, &label) {
            if self.naming {
                while label.chars().count() > 3 && !fits(p, &format!("...{label}")) {
                    label.remove(0);
                }
                label = format!("...{label}");
            } else {
                while label.chars().count() > 3 && !fits(p, &format!("{label}...")) {
                    label.pop();
                }
                label.push_str("...");
            }
        }
        let nw = p.text_width(&label, 12.0, false) + 12.0;
        let tip = if self.naming { "Type the new name, then Enter. Click again when done." } else { "Click to rename the track, then type. Its name shows in the track list." };
        if p.button(input, "name", [name_x, 4.0, nw, 18.0], &label, tip, self.naming) {
            self.naming = !self.naming;
            self.name_fresh = self.naming;
            if !self.naming {
                if self.course.name.trim().is_empty() {
                    self.course.name = self.free_name(self.course.mode);
                }
                self.changed();
            }
        }
        if p.button(input, "kind", [name_x + nw + 4.0, 4.0, kw, 18.0], &kind, "The kind of race this track is for. Click to change it: Supercross, National, Baja or Enduro.", matches!(self.popup, Some(Popup::Kind))) {
            self.popup = Some(Popup::Kind);
        }
        let mut x = w - 8.0;
        let mut right = |p: &mut Painter, input: &mut Input, id: &str, label: &str, tip: &str, width: f32, on: bool| {
            x -= width + 4.0;
            p.button(input, id, [x, 4.0, width, 18.0], label, tip, on)
        };
        if right(p, input, "exit", "Exit", "Leave the builder. Your track is saved as you go.", 44.0, false) {
            self.save();
            self.ask = Some(Ask::Exit);
        }
        let riding = self.bake_job.as_ref().is_some_and(|(_, r)| *r);
        if right(p, input, "ride", if riding { "Baking..." } else { "RIDE IT  (R)" }, "Bake the track and ride it now. Esc comes back here.", 84.0, true) {
            self.bake_track(true);
        }
        let saving = self.bake_job.as_ref().is_some_and(|(_, r)| !*r) || self.export_job.is_some();
        if right(p, input, "save", if saving { "Saving..." } else { "Save / Export" }, "Save the track for OpenMCM2 (and its track lists), or export it for the original Motocross Madness 2. Ctrl+S saves for OpenMCM2.", 84.0, matches!(self.popup, Some(Popup::Save))) && !saving {
            self.popup = Some(Popup::Save);
        }
        if right(p, input, "redo", "Redo", "Redo what was undone (Ctrl+Y).", 44.0, false) {
            self.redo();
        }
        if right(p, input, "undo", "Undo", "Undo the last change (Ctrl+Z).", 44.0, false) {
            self.undo();
        }
        if right(p, input, "open", "Open", "Open a track you made, or edit a copy of any track in the game.", 44.0, false) {
            self.popup = Some(Popup::Open { files: self.course_files(), mode: self.course.mode, scroll: 0 });
            self.wants_tracks = self.tracks.is_empty();
        }
        if right(p, input, "new", "New", "Start a new track: Supercross, National, Baja or Enduro.", 40.0, false) {
            self.popup = Some(Popup::New { surprise: false });
        }

        // ---- tools ----
        let left = 8.0;
        let col = 150.0;
        let mut y = 34.0;
        p.panel(input, [left - 4.0, y - 4.0, col + 8.0, 4.0 * 22.0 + 8.0]);
        for (k, (tool, name, tip)) in TOOLS.iter().enumerate() {
            let (cx, cy) = (left + (k % 2) as f32 * (col / 2.0 + 1.0), y + (k / 2) as f32 * 22.0);
            if p.button(input, name, [cx, cy, col / 2.0 - 1.0, 20.0], name, tip, self.tool == *tool) {
                self.tool = *tool;
            }
        }
        y += 4.0 * 22.0 + 10.0;
        let top = y;
        // The options: measured by drawing them after a background the
        // height of last frame's.
        let height = input.panel_height;
        p.panel(input, [left - 4.0, top - 4.0, col + 8.0, height + 8.0]);
        match self.tool {
            Tool::Line => self.line_options(p, input, &mut y, left, col),
            Tool::Jumps => self.jump_options(p, input, &mut y, left, col),
            Tool::Shape => self.shape_options(p, input, &mut y, left, col),
            Tool::Paint => self.paint_options(p, input, &mut y, left, col),
            Tool::Objects => self.object_options(p, input, &mut y, left, col),
            Tool::Nature => self.nature_options(p, input, &mut y, left, col),
            Tool::Motion => self.motion_options(p, input, &mut y, left, col),
            Tool::Land => self.land_options(p, input, &mut y, left, col),
        }
        input.panel_height = y - top;

        // ---- the view ----
        // Zoom, tilt and turn as sliders too, for a slow wheel or a trackpad.
        p.panel(input, [w - 214.0, 372.0, 210.0, 56.0]);
        let far = self.course.mode.side() * 1.3;
        let near = 40.0f32;
        let zoom = 1.0 - (self.cam.dist / near).ln() / (far / near).ln();
        let shown = format!("{:.0}%", zoom.clamp(0.0, 1.0) * 100.0);
        if let (Some(v), _) = p.slider(input, "zoom", [w - 208.0, 375.0, 96.0, 24.0], "Zoom", "How close the view is. The wheel, a pinch or the pad's triggers zoom too.", zoom, (0.0, 1.0), &shown) {
            self.cam.dist = near * (far / near).powf(1.0 - v);
        }
        let tilt = (self.cam.pitch - 0.12) / (1.56 - 0.12);
        let shown = format!("{:.0}\u{b0}", self.cam.pitch.to_degrees());
        if let (Some(v), _) = p.slider(input, "tilt", [w - 104.0, 375.0, 96.0, 24.0], "Tilt", "How steeply the view looks down: low over the ground, up to straight down. Right-drag tilts it too.", tilt, (0.0, 1.0), &shown) {
            self.cam.pitch = 0.12 + v * (1.56 - 0.12);
        }
        let turn = self.cam.yaw.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
        let shown = format!("{:.0}\u{b0}", turn * 360.0);
        if let (Some(v), _) = p.slider(input, "turn", [w - 208.0, 401.0, 200.0, 24.0], "Turn", "Turns the view round the track. Right-drag turns it too.", turn, (0.0, 1.0), &shown) {
            self.cam.yaw = v * std::f32::consts::TAU;
        }
        p.panel(input, [w - 214.0, 430.0, 210.0, 26.0]);
        let busy = self.bake_job.is_some();
        if p.button(input, "watch", [w - 211.0, 433.0, 59.0, 20.0], if busy && self.watch { "Baking..." } else { "Watch AI" }, "Bake the track and watch a computer rider lap it, to see how it rides, as the original builder's Test Computer Bike (P takes over). Esc comes back here.", false) && !busy {
            self.watch = true;
            self.bake_track(true);
        }
        if p.button(input, "top", [w - 149.0, 433.0, 66.0, 20.0], "Top view", "Look straight down on the track, or back (Tab).", self.cam.pitch > 1.5) {
            self.top_view();
        }
        if p.button(input, "frame", [w - 80.0, 433.0, 73.0, 20.0], "Whole track", "Bring the whole track into view (F).", false) {
            self.frame_track();
        }

        // ---- hint line ----
        let hint = self.hint();
        p.panel(input, [0.0, 460.0, w, 20.0]);
        p.text([10.0, 465.0], &hint, 11.0, false, [0.85, 0.85, 0.8, 1.0]);
        // Saved / baking state at the right of the hint line.
        let busy = if self.export_job.is_some() {
            Some((1, "Making it for MM2"))
        } else if self.bake_job.is_some() {
            Some((0, "Saving"))
        } else {
            None
        };
        if let Some((which, what)) = busy {
            // The bake can't say how far it's got: the bar follows how long
            // the last one took, easing on if this one runs longer.
            let expect = self.took[which][self.mode_index()];
            let t = self.now - self.busy_since;
            let f = (1.0 - (-t / expect * 1.6).exp()).min(0.97);
            let left = (expect - t).max(1.0);
            let label = if t < expect { format!("{what}: about {left:.0} s") } else { format!("{what}: nearly there") };
            let bw = 110.0;
            let lw = p.text_width(&label, 11.0, false);
            p.text([w - bw - lw - 18.0, 465.0], &label, 11.0, false, [0.85, 0.85, 0.8, 1.0]);
            let bar = [w - bw - 10.0, 466.0, bw, 8.0];
            p.fill(bar, [0.18, 0.18, 0.16, 1.0]);
            p.fill([bar[0], bar[1], bar[2] * f, bar[3]], [ACCENT[0] * 0.8, ACCENT[1] * 0.8, ACCENT[2] * 0.8, 1.0]);
        }
        let state = if busy.is_some() {
            String::new()
        } else if self.unsaved {
            "Not saved yet".to_string()
        } else {
            "Saved".to_string()
        };
        let sw = p.text_width(&state, 11.0, false);
        p.text([w - sw - 10.0, 465.0], &state, 11.0, false, [0.6, 0.6, 0.55, 1.0]);

        // ---- toast ----
        if let Some((text, at)) = &self.toast {
            let age = self.now - at;
            if age < 2.0 {
                let a = (2.0 - age).min(1.0);
                let tw = p.text_width(text, 13.0, true) + 24.0;
                p.fill([(w - tw) / 2.0, 40.0, tw, 24.0], [0.07, 0.07, 0.065, 0.85 * a]);
                p.text([(w - tw) / 2.0 + 12.0, 46.0], text, 13.0, true, [1.0, 1.0, 1.0, a]);
            }
        }

        // ---- pop-ups ----
        match self.popup.take() {
            Some(Popup::New { mut surprise }) => {
                let (pw, ph) = (300.0, 200.0);
                let (px, py) = ((w - pw) / 2.0, 120.0);
                p.fill([0.0, 0.0, w, 480.0], [0.0, 0.0, 0.0, 0.35]);
                p.panel(input, [px, py, pw, ph]);
                p.text([px + 14.0, py + 12.0], "NEW TRACK", 14.0, true, [ACCENT[0], ACCENT[1], ACCENT[2], 1.0]);
                p.text([px + 14.0, py + 32.0], "Pick a kind of race. You start with a ready loop to change,", 11.0, false, [0.85, 0.85, 0.8, 1.0]);
                p.text([px + 14.0, py + 45.0], "or turn on Surprise me for a whole track to ride now.", 11.0, false, [0.85, 0.85, 0.8, 1.0]);
                let mut keep = true;
                for (k, mode) in Mode::ALL.iter().enumerate() {
                    let tip = match mode {
                        Mode::Supercross => "A tight track of jumps on a stadium floor, lined with bales.",
                        Mode::National => "An outdoor lap track cut into the hills.",
                        Mode::Baja => "A long run across open desert from gate to gate.",
                        Mode::Enduro => "Gates through hills and mountains.",
                    };
                    let r = [px + 14.0 + (k % 2) as f32 * 138.0, py + 64.0 + (k / 2) as f32 * 30.0, 134.0, 26.0];
                    if p.button(input, mode.name(), r, mode.name(), tip, false) {
                        self.save();
                        self.remember();
                        self.course = if surprise {
                            mm2_course::auto::surprise(*mode, self.dice())
                        } else {
                            Course::new(*mode)
                        };
                        self.course.name = self.free_name(*mode);
                        self.field = None;
                        self.land = None;
                        self.picked = None;
                        self.changed();
                        keep = false;
                    }
                }
                let label = if surprise { "Surprise me: on" } else { "Surprise me: off" };
                if p.button(input, "surprise", [px + 14.0, py + 130.0, 272.0, 22.0], label, "On: the new track comes with its own line, hills and jumps, ready to ride. Off: a plain loop to build on.", surprise) {
                    surprise = !surprise;
                }
                if p.button(input, "cancel", [px + pw - 84.0, py + ph - 30.0, 70.0, 20.0], "Cancel", "Keep working on this track.", false) {
                    keep = false;
                }
                if keep {
                    self.popup = Some(Popup::New { surprise });
                }
            }
            Some(Popup::Open { files, mode, scroll }) => {
                let (pw, ph) = (470.0, 360.0);
                let (px, py) = ((w - pw) / 2.0, 60.0);
                p.fill([0.0, 0.0, w, 480.0], [0.0, 0.0, 0.0, 0.35]);
                p.panel(input, [px, py, pw, ph]);
                let accent = [ACCENT[0], ACCENT[1], ACCENT[2], 1.0];
                let soft = [0.85, 0.85, 0.8, 1.0];
                let (mut keep, mut mode, mut scroll) = (true, mode, scroll);
                let mut opened: Option<Course> = None;
                // Your own tracks, on the left.
                let col = 190.0;
                p.text([px + 14.0, py + 12.0], "YOUR TRACKS", 13.0, true, accent);
                if files.is_empty() {
                    p.text([px + 14.0, py + 36.0], "None saved yet.", 11.0, false, soft);
                }
                for (k, f) in files.iter().take(13).enumerate() {
                    let name = f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    let r = [px + 14.0, py + 34.0 + k as f32 * 22.0, col, 20.0];
                    if p.button(input, &format!("open{k}"), r, &name, "Open this track to carry on with it.", false) {
                        match load_file(f) {
                            Some(c) => opened = Some(c),
                            None => self.say("Couldn't open that track"),
                        }
                    }
                }
                // A copy of any track there is, on the right.
                let rx = px + 14.0 + col + 16.0;
                let rw = pw - (rx - px) - 14.0;
                p.text([rx, py + 12.0], "EDIT A COPY", 13.0, true, accent);
                for (k, m) in Mode::ALL.iter().enumerate() {
                    let bw = (rw - 6.0) / 4.0;
                    let r = [rx + k as f32 * (bw + 2.0), py + 32.0, bw, 18.0];
                    if p.button(input, &format!("copymode{k}"), r, m.name(), "Show this kind of race's tracks.", mode == *m) {
                        mode = *m;
                        scroll = 0;
                    }
                }
                let list: Vec<(String, PathBuf, usize)> = self.tracks.iter().filter(|t| t.mode == mode).map(|t| (t.name.clone(), t.path.clone(), t.course)).collect();
                let rows = 11;
                if input.over((rx, py + 56.0)) || (input.pointer.0 >= rx && input.pointer.1 > py + 54.0 && input.pointer.1 < py + ph - 34.0) {
                    let wheel = input.wheel;
                    if wheel < 0.0 {
                        scroll = (scroll + 1).min(list.len().saturating_sub(rows));
                    } else if wheel > 0.0 {
                        scroll = scroll.saturating_sub(1);
                    }
                }
                if self.tracks.is_empty() {
                    p.text([rx, py + 60.0], "Finding the tracks...", 11.0, false, soft);
                } else if list.is_empty() {
                    p.text([rx, py + 60.0], "No tracks of this kind.", 11.0, false, soft);
                }
                for (k, t) in list.iter().skip(scroll).take(rows).enumerate() {
                    let r = [rx, py + 56.0 + k as f32 * 22.0, rw, 20.0];
                    let tip = "Opens a copy of this track: add jumps, shape and paint its ground, place objects or redraw its line. The track itself stays as it is.";
                    let (tname, tpath, tcourse) = t;
                    if p.button(input, &format!("copy{}", scroll + k), r, tname, tip, false) {
                        let name = self.copy_name(tname);
                        if !crate::fs::is_file(tpath) && self.copy_job.is_none() {
                            // A Free Pack track not built yet: built on
                            // another thread, the builder saying so.
                            let (path, course) = (tpath.clone(), *tcourse);
                            self.copy_job = Some(Job::spawn(move || copy_of(&path, mode, course, &name)));
                            self.say("Building the track from its recipe...");
                            keep = false;
                        } else {
                            match copy_of(tpath, mode, *tcourse, &name) {
                                Ok(c) => opened = Some(c),
                                Err(e) => {
                                    eprintln!("{}: {e}", tpath.display());
                                    self.say("That track can't be copied");
                                }
                            }
                        }
                    }
                }
                if list.len() > rows {
                    p.text([rx, py + ph - 28.0], &format!("{}-{} of {} (wheel scrolls)", scroll + 1, (scroll + rows).min(list.len()), list.len()), 10.0, false, soft);
                }
                if p.button(input, "cancel", [px + pw - 84.0, py + ph - 28.0, 70.0, 20.0], "Cancel", "Keep working on this track.", false) {
                    keep = false;
                }
                if let Some(c) = opened {
                    self.open_copy(c);
                    keep = false;
                }
                if keep {
                    self.popup = Some(Popup::Open { files, mode, scroll });
                }
            }
            Some(Popup::Kind) => {
                let (pw, ph) = (320.0, 168.0);
                let (px, py) = ((w - pw) / 2.0, 110.0);
                p.fill([0.0, 0.0, w, 480.0], [0.0, 0.0, 0.0, 0.35]);
                p.panel(input, [px, py, pw, ph]);
                p.text([px + 14.0, py + 12.0], "KIND OF RACE", 14.0, true, [ACCENT[0], ACCENT[1], ACCENT[2], 1.0]);
                let copy = self.course.base.is_some();
                let say = if copy { "A copy stays the kind of race its track is. Use New for another." } else { "Change this track to another kind. Your line, jumps and the rest move to fit." };
                for (k, line) in panel::wrap(p, say, pw - 28.0, 11.0).iter().enumerate() {
                    p.text([px + 14.0, py + 32.0 + k as f32 * 13.0], line, 11.0, false, [0.85, 0.85, 0.8, 1.0]);
                }
                let mut keep = true;
                for (k, mode) in Mode::ALL.iter().enumerate() {
                    let tip = match mode {
                        Mode::Supercross => "A tight track on a stadium floor, lined with bales. The track is shrunk to fit the floor.",
                        Mode::National => "An outdoor lap track cut into the hills.",
                        Mode::Baja => "A long run across open desert from gate to gate.",
                        Mode::Enduro => "Gates through hills and mountains.",
                    };
                    let r = [px + 14.0 + (k % 2) as f32 * 148.0, py + 66.0 + (k / 2) as f32 * 30.0, 144.0, 26.0];
                    if p.button(input, &format!("kind{k}"), r, mode.name(), tip, self.course.mode == *mode) && !copy && self.course.mode != *mode {
                        self.remember();
                        self.course.change_mode(*mode);
                        self.field = None;
                        self.land = None;
                        self.picked = None;
                        self.changed();
                        self.say(&format!("Now a {} track", mode.name()));
                        keep = false;
                    }
                }
                if p.button(input, "cancel", [px + pw - 84.0, py + ph - 30.0, 70.0, 20.0], "Close", "Keep the track as it is.", false) {
                    keep = false;
                }
                if keep {
                    self.popup = Some(Popup::Kind);
                }
            }
            Some(Popup::Save) => {
                let (pw, ph) = (340.0, 150.0);
                let (px, py) = ((w - pw) / 2.0, 110.0);
                p.fill([0.0, 0.0, w, 480.0], [0.0, 0.0, 0.0, 0.35]);
                p.panel(input, [px, py, pw, ph]);
                p.text([px + 14.0, py + 12.0], "SAVE / EXPORT FOR", 14.0, true, [ACCENT[0], ACCENT[1], ACCENT[2], 1.0]);
                let say = "OpenMCM2 loads both, but its own save keeps the finer ground and opens again here.";
                for (k, line) in panel::wrap(p, say, pw - 28.0, 11.0).iter().enumerate() {
                    p.text([px + 14.0, py + 32.0 + k as f32 * 13.0], line, 11.0, false, [0.85, 0.85, 0.8, 1.0]);
                }
                let mut keep = true;
                let half = (pw - 32.0) / 2.0;
                if p.button(input, "save-open", [px + 14.0, py + 70.0, half, 30.0], "OpenMCM2", "Save the track and put it in OpenMCM2's track list for its race (Ctrl+S).", true) {
                    self.bake_track(false);
                    self.say("Saving...");
                    keep = false;
                }
                if p.button(input, "save-orig", [px + 18.0 + half, py + 70.0, half, 30.0], "Original MM2", "Make a copy that plays in the original Motocross Madness 2 from 2000, to put in its Teraform folder. Anyone can then race it without OpenMCM2.", false) {
                    self.export_original();
                    keep = false;
                }
                if p.button(input, "save-close", [px + pw - 84.0, py + ph - 30.0, 70.0, 20.0], "Close", "Don't save now.", false) {
                    keep = false;
                }
                if keep {
                    self.popup = Some(Popup::Save);
                }
            }
            None => {}
        }
    }

    /// Starts work on a copied track.
    fn open_copy(&mut self, c: Course) {
        self.save();
        self.history.clear();
        self.future.clear();
        self.course = c;
        self.field = None;
        self.land = None;
        self.picked = None;
        self.dirty = true;
        self.unsaved = true;
    }

    /// A copy's name, numbered if a saved track has it already.
    fn copy_name(&self, of: &str) -> String {
        let taken: Vec<String> = self.course_files().iter().filter_map(|f| f.file_stem().map(|s| s.to_string_lossy().to_ascii_uppercase())).collect();
        let stem = |name: &str| Course { name: name.to_string(), ..Course::default() }.stem();
        (1..100)
            .map(|k| if k == 1 { of.to_string() } else { format!("{of} {k}") })
            .find(|n| !taken.contains(&stem(&format!("{n} copy"))))
            .unwrap_or_else(|| of.to_string())
    }

    /// A name for a new track that no saved track has yet.
    fn free_name(&self, mode: Mode) -> String {
        let taken: Vec<String> = self.course_files().iter().filter_map(|f| f.file_stem().map(|s| s.to_string_lossy().to_ascii_uppercase())).collect();
        for n in 1..100 {
            let name = format!("My {} {n}", mode.name());
            let c = Course { name: name.clone(), ..Course::default() };
            if !taken.contains(&c.stem()) {
                return name;
            }
        }
        format!("My {} Track", mode.name())
    }

    fn course_files(&self) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = crate::fs::read_dir(&self.dir)
            .map(|it| it.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("course"))).collect())
            .unwrap_or_default();
        out.sort();
        out
    }

    fn heading(p: &mut Painter, y: &mut f32, left: f32, text: &str) {
        p.text([left, *y], text, 12.0, true, [ACCENT[0], ACCENT[1], ACCENT[2], 1.0]);
        *y += 16.0;
    }

    fn note(p: &mut Painter, y: &mut f32, left: f32, col: f32, text: &str) {
        for line in panel::wrap(p, text, col, 10.5) {
            p.text([left, *y], &line, 10.5, false, [0.75, 0.75, 0.7, 1.0]);
            *y += 12.0;
        }
        *y += 4.0;
    }

    /// A slider that keeps the change for Undo once let go.
    #[allow(clippy::too_many_arguments)]
    fn slider(&mut self, p: &mut Painter, input: &mut Input, y: &mut f32, left: f32, col: f32, id: &str, label: &str, tip: &str, value: f32, range: (f32, f32), shown: String) -> Option<f32> {
        let r = [left, *y, col, 26.0];
        *y += 30.0;
        let (out, started) = p.slider(input, id, r, label, tip, value, range, &shown);
        if started {
            self.remember();
        }
        out
    }

    /// Draw, place gates, or move the start.
    fn line_modes(&mut self, p: &mut Painter, input: &mut Input, y: &mut f32, left: f32, col: f32) {
        let laps = self.course.mode.laps();
        let modes: &[(LineMode, &str, &str)] = if laps {
            &[(LineMode::Draw, "Draw", "Click the ground to add points; drag them to move."), (LineMode::Start, "Move start", "Click the track to put the start and finish line there (or press T over it).")]
        } else {
            &[
                (LineMode::Draw, "Draw", "Click the ground to add points; drag them to move."),
                (LineMode::Gates, "Gates", "Place the checkpoint gates yourself: click the track to add one, click a gate to take it away. The finish stays at the end."),
                (LineMode::Start, "Start", "Click the track to start there, or the open ground beside it for a funnel start: the riders line up out in the open and race into the track."),
            ]
        };
        let w = col / modes.len() as f32;
        for (k, (m, label, tip)) in modes.iter().enumerate() {
            if p.button(input, label, [left + k as f32 * w, *y, w - 2.0, 20.0], label, tip, self.line_mode == *m) {
                if *m == LineMode::Gates && self.course.line.gates.is_empty() {
                    // Start from the gates the track has now.
                    self.remember();
                    let line = &self.line;
                    let last = self.gates.len().saturating_sub(1);
                    self.course.line.gates = self.gates.iter().take(last).filter_map(|(g, _)| line.nearest(*g).map(|n| n.s)).collect();
                    self.changed();
                }
                self.line_mode = *m;
            }
        }
        *y += 26.0;
        let half = col / 2.0 - 1.0;
        let mut row = false;
        if !laps && !self.course.line.gates.is_empty() {
            row = true;
            if p.button(input, "gauto", [left, *y, half, 20.0], "Gates by themselves", "Back to a gate every few hundred feet.", false) {
                self.remember();
                self.course.line.gates.clear();
                self.changed();
            }
        }
        if !laps && self.course.line.start_spot.is_some() {
            row = true;
            if p.button(input, "sauto", [left + half + 2.0, *y, half, 20.0], "Start on the track", "No funnel: the riders line up on the track at its start.", false) {
                self.remember();
                self.course.line.start_spot = None;
                self.changed();
            }
        }
        if row {
            *y += 26.0;
        }
    }

    fn line_options(&mut self, p: &mut Painter, input: &mut Input, y: &mut f32, left: f32, col: f32) {
        Self::heading(p, y, left, "THE LINE");
        self.line_modes(p, input, y, left, col);
        let closed = self.course.line.closed;
        if p.button(input, "loop", [left, *y, col / 2.0 - 2.0, 20.0], "Loop", "The line comes round to its start: a lap.", closed) && !closed {
            self.remember();
            self.course.line.closed = true;
            self.changed();
        }
        if p.button(input, "open-line", [left + col / 2.0 + 2.0, *y, col / 2.0 - 2.0, 20.0], "Point to point", "The line ends away from its start (Baja and Enduro).", !closed) && closed {
            self.remember();
            self.course.line.closed = false;
            self.changed();
        }
        *y += 26.0;
        let avg = self.course.line.points.iter().map(|p| p.width).sum::<f32>() / self.course.line.points.len().max(1) as f32;
        if let Some(v) = self.slider(p, input, y, left, col, "width", "Track width", "How wide the whole track is. The wheel over a point sets it there alone.", avg, (12.0, 90.0), format!("{avg:.0} ft")) {
            let k = v / avg.max(1.0);
            for pt in &mut self.course.line.points {
                pt.width = (pt.width * k).clamp(10.0, 120.0);
            }
            self.changed();
        }
        let b = self.course.line.berms;
        if let Some(v) = self.slider(p, input, y, left, col, "berms", "Berms", "How high the corners bank up on their outside, so you can rail them.", b, (0.0, 1.0), format!("{:.0}%", b * 100.0)) {
            self.course.line.berms = v;
            self.changed();
        }
        if p.button(input, "smooth-line", [left, *y, col / 2.0 - 2.0, 20.0], "Smooth", "Evens out the line's wiggles.", false) {
            self.remember();
            let pts = self.course.line.points.clone();
            let n = pts.len();
            if n >= 3 {
                for i in 0..n {
                    let (a, b) = if self.course.line.closed {
                        (pts[(i + n - 1) % n], pts[(i + 1) % n])
                    } else if i == 0 || i + 1 == n {
                        continue;
                    } else {
                        (pts[i - 1], pts[i + 1])
                    };
                    let q = &mut self.course.line.points[i];
                    q.x = q.x * 0.5 + (a.x + b.x) * 0.25;
                    q.z = q.z * 0.5 + (a.z + b.z) * 0.25;
                }
            }
            self.changed();
        }
        if p.button(input, "clear-line", [left + col / 2.0 + 2.0, *y, col / 2.0 - 2.0, 20.0], "Start again", "Clears the line and its jumps, to draw a new one. Undo brings it back.", false) {
            self.remember();
            self.course.line.points.clear();
            self.course.jumps.clear();
            self.course.line.start = 0.0;
            self.changed();
        }
        *y += 26.0;
        let len = self.line.length;
        Self::note(p, y, left, col, &format!("{} points, {:.0} ft of track ({:.2} miles).", self.course.line.points.len(), len, len / 5280.0));
    }

    fn jump_options(&mut self, p: &mut Painter, input: &mut Input, y: &mut f32, left: f32, col: f32) {
        Self::heading(p, y, left, "JUMP KIT");
        for (k, kind) in Kind::ALL.iter().enumerate() {
            let r = [left + (k % 2) as f32 * (col / 2.0 + 1.0), *y + (k / 2) as f32 * 22.0, col / 2.0 - 1.0, 20.0];
            if p.button(input, kind.name(), r, kind.name(), kind.about(), self.kind == *kind) {
                self.kind = *kind;
            }
        }
        *y += (Kind::ALL.len().div_ceil(2)) as f32 * 22.0 + 6.0;
        let half = col / 2.0 - 1.0;
        if p.button(input, "jfill", [left, *y, half, 20.0], "Fill with jumps", "Adds jumps along the straights, keeping the ones you placed and room to ride in and land. Click again for more, Undo to take them off.", false) {
            self.remember();
            let seed = self.dice();
            let added = mm2_course::auto::fill_jumps(&mut self.course, seed);
            self.picked = None;
            self.changed();
            self.say(&if added == 0 { "No room left for more jumps".to_string() } else { format!("Added {added} jumps") });
        }
        if p.button(input, "jclear", [left + half + 2.0, *y, half, 20.0], "Clear jumps", "Takes every jump off the line. Undo brings them back.", false) {
            self.remember();
            self.course.jumps.clear();
            self.picked = None;
            self.changed();
        }
        *y += 26.0;
        if let Some(i) = self.picked.filter(|&i| i < self.course.jumps.len()) {
            let j = self.course.jumps[i];
            Self::heading(p, y, left, &j.kind.name().to_ascii_uppercase());
            if let Some(v) = self.slider(p, input, y, left, col, "jh", "Height", "How tall it is. The wheel over a jump changes it too.", j.height, (0.5, 25.0), format!("{:.1} ft", j.height)) {
                self.course.jumps[i].height = v;
                self.changed();
            }
            if let Some(v) = self.slider(p, input, y, left, col, "jl", "Length", "How long it is along the track. Shift and the wheel change it too.", j.length, (10.0, 300.0), format!("{:.0} ft", j.length)) {
                self.course.jumps[i].length = v;
                self.changed();
            }
            if p.button(input, "jremove", [left, *y, col, 20.0], "Remove this jump", "Takes it off the track (right-click does too).", false) {
                self.remember();
                self.course.jumps.remove(i);
                self.picked = None;
                self.changed();
            }
            *y += 26.0;
        } else {
            Self::note(p, y, left, col, "Click the line to drop the jump picked above. Click a jump to change its size.");
        }
    }

    fn shape_options(&mut self, p: &mut Painter, input: &mut Input, y: &mut f32, left: f32, col: f32) {
        Self::heading(p, y, left, "BRUSH");
        for (k, b) in Brush::ALL.iter().enumerate() {
            let tip = match b {
                Brush::Raise => "Builds the ground up where you drag.",
                Brush::Lower => "Digs the ground down where you drag.",
                Brush::Smooth => "Evens out bumps and hard edges.",
                Brush::Flatten => "Levels the ground to the height where you started dragging.",
                Brush::Roughen => "Adds small lumps and ruts.",
            };
            let r = [left + (k % 2) as f32 * (col / 2.0 + 1.0), *y + (k / 2) as f32 * 22.0, col / 2.0 - 1.0, 20.0];
            if p.button(input, b.name(), r, b.name(), tip, self.brush == *b) {
                self.brush = *b;
            }
        }
        *y += 3.0 * 22.0 + 4.0;
        let r = [left, *y, col, 26.0];
        *y += 30.0;
        if let (Some(v), _) = p.slider(input, "size", r, "Size", "How wide the brush is ([ and ]).", self.radius, (8.0, 300.0), &format!("{:.0} ft", self.radius)) {
            self.radius = v;
        }
        let r = [left, *y, col, 26.0];
        *y += 30.0;
        if let (Some(v), _) = p.slider(input, "strength", r, "Strength", "How much each pass of the brush changes the ground.", self.strength, (0.05, 1.0), &format!("{:.0}%", self.strength * 100.0)) {
            self.strength = v;
        }
        Self::note(p, y, left, col, "Brush strokes go on top of the track, so you can shape the jumps and the land round them.");
    }

    fn paint_options(&mut self, p: &mut Painter, input: &mut Input, y: &mut f32, left: f32, col: f32) {
        Self::heading(p, y, left, "SURFACE");
        for (k, s) in Surface::ALL.iter().enumerate() {
            let tip = match s {
                Surface::Dirt => "Loose dirt: full grip, dusty.",
                Surface::Grass => "Grass: a little slower and less grip.",
                Surface::Sand => "Sand: drags the bike and slides.",
                Surface::Mud => "Mud: slow and slippery, no dust.",
                Surface::Rock => "Hardpack: fast and grippy, throws no dirt.",
            };
            let r = [left + (k % 2) as f32 * (col / 2.0 + 1.0), *y + (k / 2) as f32 * 22.0, col / 2.0 - 1.0, 20.0];
            if p.button(input, s.name(), r, s.name(), tip, self.surface == *s) {
                self.surface = *s;
            }
            let c = view::surface_colour(*s);
            p.fill([r[0] + r[2] - 12.0, r[1] + 6.0, 8.0, 8.0], [c[0], c[1], c[2], 1.0]);
        }
        *y += 3.0 * 22.0 + 4.0;
        let r = [left, *y, col, 26.0];
        *y += 30.0;
        if let (Some(v), _) = p.slider(input, "psize", r, "Size", "How wide the paint brush is.", self.paint_radius, (5.0, 200.0), &format!("{:.0} ft", self.paint_radius)) {
            self.paint_radius = v;
        }
        Self::note(p, y, left, col, "The track is dirt by itself. Paint mud in a dip or sand in a corner to change how it rides.");
    }

    fn nature_options(&mut self, p: &mut Painter, input: &mut Input, y: &mut f32, left: f32, col: f32) {
        Self::heading(p, y, left, "NATURE");
        let third = (col - 4.0) / 3.0;
        for (k, (mode, name, tip)) in [
            (NatureMode::Plant, "Plant", "Paint trees, bushes and grass onto the land."),
            (NatureMode::Clear, "Clear", "Take plants away where you drag."),
            (NatureMode::River, "River", "Draw a river or stream to splash through."),
        ]
        .into_iter()
        .enumerate()
        {
            if p.button(input, &format!("nm{k}"), [left + k as f32 * (third + 2.0), *y, third, 20.0], name, tip, self.nature == mode) {
                self.nature = mode;
            }
        }
        *y += 26.0;
        if self.nature == NatureMode::River {
            let r = self.river.filter(|&r| r < self.course.rivers.len());
            let (w, d) = r.map_or((28.0, 2.5), |r| (self.course.rivers[r].width, self.course.rivers[r].depth));
            if let Some(i) = r {
                if let Some(v) = self.slider(p, input, y, left, col, "rwidth", "Width", "How wide the river is. The wheel over one of its points changes it too.", w, (6.0, 120.0), format!("{w:.0} ft")) {
                    self.course.rivers[i].width = v;
                    self.changed();
                }
                if let Some(v) = self.slider(p, input, y, left, col, "rdepth", "Depth", "How deep the water is. Shallow water is a splash; deep water slows a bike right down.", d, (0.5, 8.0), format!("{d:.1} ft")) {
                    self.course.rivers[i].depth = v;
                    self.changed();
                }
            }
            let half = col / 2.0 - 1.0;
            if p.button(input, "rnew", [left, *y, half, 20.0], "New river", "Finish this river; the next click starts another.", false) {
                self.river = None;
            }
            if p.button(input, "rdel", [left + half + 2.0, *y, half, 20.0], "Remove river", "Takes this river away and fills its bed back in. Undo brings it back.", false) {
                if let Some(i) = r {
                    self.remember();
                    self.course.rivers.remove(i);
                    self.river = None;
                    self.changed();
                }
            }
            *y += 26.0;
            Self::note(p, y, left, col, "Click along the ground from where the river starts to where it ends. It flows downhill, cuts its own muddy bed, and riders splash through it.");
            Self::note(p, y, left, col, &format!("{} rivers. They show in the original game as a muddy bed; the water itself shows in OpenMCM2.", self.course.rivers.len()));
            return;
        }
        for (k, f) in Flora::ALL.iter().enumerate() {
            let tip = match f {
                Flora::Mixed => "Trees with bushes and grass round them, as the land grows on its own.",
                Flora::Trees => "Trees only. The trunks are solid, so keep them off the racing line.",
                Flora::Bushes => "Low bushes and shrubs.",
                Flora::Grass => "Tufts of tall grass.",
            };
            let r = [left + (k % 2) as f32 * (col / 2.0 + 1.0), *y + (k / 2) as f32 * 22.0, col / 2.0 - 1.0, 20.0];
            if p.button(input, f.name(), r, f.name(), tip, self.flora == *f) {
                self.flora = *f;
            }
        }
        *y += 2.0 * 22.0 + 4.0;
        let r = [left, *y, col, 26.0];
        *y += 30.0;
        if let (Some(v), _) = p.slider(input, "plsize", r, "Size", "How wide the brush is.", self.plant_radius, (10.0, 400.0), &format!("{:.0} ft", self.plant_radius)) {
            self.plant_radius = v;
        }
        if self.nature == NatureMode::Plant {
            let r = [left, *y, col, 26.0];
            *y += 30.0;
            if let (Some(v), _) = p.slider(input, "pldense", r, "Density", "How thickly they grow: a few here and there, up to a forest.", self.plant_density, (0.05, 1.0), &format!("{:.0}%", self.plant_density * 100.0)) {
                self.plant_density = v;
            }
        }
        let stadium = self.course.mode == Mode::Supercross;
        let tip = if stadium {
            "Grows grass, bushes and trees round the track by itself. Not inside a stadium: change the race kind to outdoors first."
        } else {
            "Grows the land round your track by itself: tall grass along the edges, bushes just off the line and groves of trees further out. Press again after changing the track to grow it round the new line. Your own planting and clearing stay."
        };
        if p.button(input, "pldress", [left, *y, col, 22.0], "Dress the scenery", tip, false) && !stadium {
            self.remember();
            mm2_course::nature::redress(&mut self.course);
            self.changed();
        }
        *y += 28.0;
        let half = col / 2.0 - 1.0;
        if p.button(input, "plall", [left, *y, half, 20.0], "Cover the land", "Grows the chosen plants over the whole land at this density, off the track. Paint or clear on top after.", false) {
            self.remember();
            self.course.plants.push(Planting { flora: self.flora, radius: 0.0, density: self.plant_density, clear: false, everywhere: true, dabs: Vec::new(), auto: false, map: None });
            self.changed();
        }
        if p.button(input, "plnone", [left + half + 2.0, *y, half, 20.0], "Clear all", "Takes every plant away. Undo brings them back.", false) {
            self.remember();
            self.course.plants.clear();
            self.changed();
        }
        *y += 26.0;
        if p.button(input, "plmap", [left, *y, col, 20.0], "Grow from a plant map...", "Grows the chosen plants from a grey picture over the whole land: white where they grow thickest at this density, black where none do. Its top is the far side of the map.", false) && !stadium {
            self.ask_file(PickFor::Plants);
        }
        *y += 26.0;
        Self::note(p, y, left, col, &format!("{} plants. They never grow on the track, and they show in the original game too.", self.plants.len()));
    }

    fn motion_options(&mut self, p: &mut Painter, input: &mut Input, y: &mut f32, left: f32, col: f32) {
        Self::heading(p, y, left, "FLYING");
        for (k, c) in Craft::ALL.iter().enumerate() {
            let tip = match c {
                Craft::Biplane => "A crop-dusting biplane, buzzing low over the track.",
                Craft::Helicopter => "A news helicopter, slow and steady over the action.",
                Craft::CargoPlane => "A big cargo plane, high and wide.",
                Craft::Blimp => "The blimp, drifting slowly overhead.",
            };
            let r = [left + (k % 2) as f32 * (col / 2.0 + 1.0), *y + (k / 2) as f32 * 22.0, col / 2.0 - 1.0, 20.0];
            if p.button(input, c.name(), r, c.name(), tip, self.craft == *c) {
                self.craft = *c;
                if let Some(f) = self.flight.filter(|&f| f < self.course.flights.len()) {
                    if self.course.flights[f].craft != *c {
                        self.remember();
                        self.course.flights[f].craft = *c;
                        self.course.flights[f].speed = c.speed();
                        self.changed();
                    }
                }
            }
        }
        *y += 2.0 * 22.0 + 4.0;
        let half = col / 2.0 - 1.0;
        if let Some(f) = self.flight.filter(|&f| f < self.course.flights.len()) {
            let v = self.course.flights[f].speed;
            if let Some(v) = self.slider(p, input, y, left, col, "fspeed", "Speed", "How fast it flies round its path.", v, (5.0, 300.0), format!("{:.0} mph", v * 0.6818)) {
                self.course.flights[f].speed = v;
                self.changed();
            }
            let h = self.course.flights[f].points.iter().map(|p| p[1]).sum::<f32>() / self.course.flights[f].points.len().max(1) as f32;
            if let Some(v) = self.slider(p, input, y, left, col, "fheight", "Height", "How high it flies over the ground. The wheel over a point sets that point alone.", h, (20.0, 1200.0), format!("{h:.0} ft")) {
                let k = v - h;
                for p in &mut self.course.flights[f].points {
                    p[1] = (p[1] + k).clamp(20.0, 1500.0);
                }
                self.changed();
            }
            if p.button(input, "fdel", [left + half + 2.0, *y, half, 20.0], "Remove it", "Takes this flight away. Undo brings it back.", false) {
                self.remember();
                self.course.flights.remove(f);
                self.flight = None;
                self.changed();
            }
        }
        if p.button(input, "fnew", [left, *y, half, 20.0], "New flight", "The next click starts another flight path, for the craft picked above.", false) {
            self.flight = None;
        }
        *y += 26.0;
        Self::note(p, y, left, col, "Click over the land to add points; it flies round them in a loop. One or two points make a circle. Drag points to move them, the wheel sets their height.");
        Self::note(p, y, left, col, &format!("{} flying. They use the game's own models and sounds, so they fly in the original game too.", self.course.flights.len()));
    }

    fn object_options(&mut self, p: &mut Painter, input: &mut Input, y: &mut f32, left: f32, col: f32) {
        Self::heading(p, y, left, "OBJECTS");
        for (k, (name, _, about)) in OBJECTS.iter().enumerate() {
            let r = [left + (k % 2) as f32 * (col / 2.0 + 1.0), *y + (k / 2) as f32 * 20.0, col / 2.0 - 1.0, 18.0];
            if p.button(input, name, r, name, about, self.object == k) {
                self.object = k;
            }
        }
        *y += OBJECTS.len().div_ceil(2) as f32 * 20.0 + 4.0;
        Self::heading(p, y, left, "YOUR OWN MODELS");
        let mut remove = None;
        for (k, m) in self.course.models.iter().enumerate() {
            let at = OBJECTS.len() + k;
            let name = m.name().to_string();
            if p.button(input, &format!("um{k}"), [left, *y, col - 24.0, 18.0], &name, "Your own model, packed into the track. Click the ground to place it.", self.object == at) {
                self.object = at;
            }
            if p.button(input, &format!("umx{k}"), [left + col - 22.0, *y, 22.0, 18.0], "x", "Takes this model out of the track, with every one placed.", false) {
                remove = Some(k);
            }
            *y += 20.0;
        }
        if let Some(k) = remove {
            self.remember();
            let name = self.course.models.remove(k).slt.name;
            self.course.objects.retain(|o| !o.model.eq_ignore_ascii_case(&name));
            if self.object >= OBJECTS.len() {
                self.object = 0;
            }
            self.changed();
        }
        if p.button(input, "umadd", [left, *y, col, 20.0], "Add a model...", "Bring in a model of your own, as the original builder's user models: pick its .slt file and the .tga pictures it uses together. It goes inside the track, so it shows in the original game too.", false) {
            self.ask_file(PickFor::Model);
        }
        *y += 26.0;
        let on = self.course.dress.edge_bales;
        if p.button(input, "bales", [left, *y, col, 20.0], if on { "Bales along the edges: on" } else { "Bales along the edges: off" }, "Lines both edges of the track with wrapped bales, as the stadiums do.", on) {
            self.remember();
            self.course.dress.edge_bales = !on;
            self.changed();
        }
        *y += 26.0;
        Self::note(p, y, left, col, &format!("{} placed. Objects face along the track when placed.", self.course.objects.len()));
    }

    /// The height map and the ground picture.
    fn picture_options(&mut self, p: &mut Painter, input: &mut Input, y: &mut f32, left: f32, col: f32) {
        Self::heading(p, y, left, "HEIGHT MAP");
        match self.course.land.heightmap.clone() {
            None => {
                if p.button(input, "hmload", [left, *y, col, 20.0], "Load a height map picture...", "Raise the land from a grey picture: black is the lowest ground, white the highest, its top the far side of the map. PNG (8 or 16-bit), TGA (as the original builder's 257 x 257, or POV-Ray's 16-bit) or raw 16-bit squares. Any size.", false) {
                    self.ask_file(PickFor::Heights);
                }
                *y += 26.0;
            }
            Some(m) => {
                Self::note(p, y, left, col, &format!("From {}.", m.picture.name));
                if let Some(v) = self.slider(p, input, y, left, col, "hmtall", "Tallest", "How high white stands above black.", m.tallest, (10.0, 1500.0), format!("{:.0} ft", m.tallest)) {
                    if let Some(h) = &mut self.course.land.heightmap {
                        h.tallest = v;
                    }
                    self.changed();
                }
                if let Some(v) = self.slider(p, input, y, left, col, "hmmix", "Picture", "How much of the land is the picture's: all of it, or blended with the hills above.", m.mix, (0.0, 1.0), format!("{:.0}%", m.mix * 100.0)) {
                    if let Some(h) = &mut self.course.land.heightmap {
                        h.mix = v;
                    }
                    self.changed();
                }
                let half = col / 2.0 - 1.0;
                if p.button(input, "hmagain", [left, *y, half, 20.0], "Another...", "Load a different height map picture.", false) {
                    self.ask_file(PickFor::Heights);
                }
                if p.button(input, "hmoff", [left + half + 2.0, *y, half, 20.0], "Remove", "Back to the hills above.", false) {
                    self.remember();
                    self.course.land.heightmap = None;
                    self.changed();
                }
                *y += 26.0;
            }
        }
        Self::heading(p, y, left, "GROUND PICTURE");
        let half = col / 2.0 - 1.0;
        match self.course.look.ground.clone() {
            None => {
                if p.button(input, "gpload", [left, *y, half, 20.0], "Load...", "Lay a picture of your own over the whole land, seen from above, its top the far side (960 x 960 as the original builder's, or any size). Save the ground first to paint over it.", false) {
                    self.ask_file(PickFor::Ground);
                }
            }
            Some(g) => {
                Self::note(p, y, left, col, &format!("{} covers the land.", g.picture.file.name));
                if p.button(input, "gptrack", [left, *y, col, 20.0], "Track's dirt drawn on top", "On: the track keeps its own dirt over your picture. Off: your picture alone, track and all.", g.track_on_top) {
                    self.remember();
                    if let Some(g) = &mut self.course.look.ground {
                        g.track_on_top = !g.track_on_top;
                    }
                    self.changed();
                }
                *y += 26.0;
                if p.button(input, "gpoff", [left, *y, half, 20.0], "Remove", "Back to the ground the builder paints.", false) {
                    self.remember();
                    self.course.look.ground = None;
                    self.changed();
                }
            }
        }
        if p.button(input, "gpsave", [left + half + 2.0, *y, half, 20.0], "Save it...", "Keep the ground as a 960 x 960 picture, track and all, to paint over in any paint program and load back.", false) {
            self.save_ground();
        }
        *y += 26.0;
    }

    fn land_options(&mut self, p: &mut Painter, input: &mut Input, y: &mut f32, left: f32, col: f32) {
        if let Some(base) = &self.course.base {
            Self::heading(p, y, left, "A COPY");
            let of = std::path::Path::new(&base.file).file_name().map_or(String::new(), |n| n.to_string_lossy().to_string());
            Self::note(p, y, left, col, &format!("This is a copy of {of}. Its land, sky, scenery and objects stay the track's own; the track itself isn't changed."));
            Self::note(p, y, left, col, "Add jumps, shape and paint the ground, place objects, or redraw the line (Line, Start again) for a new course on the same land.");
            Self::note(p, y, left, col, "Click the name in the top bar to rename the copy.");
            return;
        }
        let sx = self.course.mode == Mode::Supercross;
        // Three pages (two in a stadium), so each fits on the screen.
        let tabs: &[(&str, &str)] = if sx {
            &[("Race", "The kind of race and the stadium."), ("Light", "The sun, the shade and the haze.")]
        } else {
            &[("Land", "The kind of race and the land under the track."), ("Pictures", "Raise the land from a height map, or lay your own picture over the ground."), ("Sky", "The sky, the sun, the shade and the haze.")]
        };
        self.land_tab = self.land_tab.min(tabs.len() - 1);
        let w = col / tabs.len() as f32;
        for (k, (label, tip)) in tabs.iter().enumerate() {
            if p.button(input, &format!("ltab{k}"), [left + k as f32 * w, *y, w - 2.0, 20.0], label, tip, self.land_tab == k) {
                self.land_tab = k;
            }
        }
        *y += 28.0;
        match (sx, self.land_tab) {
            (false, 0) | (true, 0) => {
        Self::heading(p, y, left, "KIND OF RACE");
        let label = format!("{}  (change)", self.course.mode.name());
        if p.button(input, "kind-land", [left, *y, col, 20.0], &label, "Make this a Supercross, National, Baja or Enduro track. Your line, jumps and the rest move to fit.", false) {
            self.popup = Some(Popup::Kind);
        }
        *y += 26.0;
                if sx {
            Self::heading(p, y, left, "STADIUM");
            let st = mm2_course::STADIUMS;
            let at = st.iter().position(|(f, _)| f.eq_ignore_ascii_case(&self.course.look.stadium)).unwrap_or(0);
            if p.button(input, "stadium", [left, *y, col, 20.0], st[at].1, "Click for the next stadium. Keep the track inside its floor.", false) {
                self.remember();
                self.course.look.stadium = st[(at + 1) % st.len()].0.into();
                self.changed();
            }
            *y += 26.0;
                        } else {
            Self::heading(p, y, left, "LAND");
            for (k, kind) in LandKind::ALL.iter().enumerate() {
                let tip = match kind {
                    LandKind::Rolling => "Green rolling hills.",
                    LandKind::Valley => "A long valley between two ridges.",
                    LandKind::Desert => "Dry desert with dunes, pans and low mesas.",
                    LandKind::Mountains => "A high meadow ringed by steep peaks.",
                    LandKind::Flat => "Flat ground.",
                };
                let r = [left + (k % 2) as f32 * (col / 2.0 + 1.0), *y + (k / 2) as f32 * 22.0, col / 2.0 - 1.0, 20.0];
                if p.button(input, kind.name(), r, kind.name(), tip, self.course.land.kind == *kind) && self.course.land.kind != *kind {
                    self.remember();
                    self.course.land.kind = *kind;
                    self.changed();
                }
            }
            *y += 3.0 * 22.0 + 4.0;
            let h = self.course.land.hills;
            if let Some(v) = self.slider(p, input, y, left, col, "hills", "Hills", "How tall the hills are.", h, (0.0, 1.0), format!("{:.0}%", h * 100.0)) {
                self.course.land.hills = v;
                self.changed();
            }
            let r = self.course.land.rough;
            if let Some(v) = self.slider(p, input, y, left, col, "rough", "Bumps", "How bumpy the ground is between the hills (the track is smoothed).", r, (0.0, 1.0), format!("{:.0}%", r * 100.0)) {
                self.course.land.rough = v;
                self.changed();
            }
            if p.button(input, "reseed", [left, *y, col, 20.0], "Another land like this", "Makes new hills of the same kind. Your line and jumps stay.", false) {
                self.remember();
                self.course.land.seed = self.course.land.seed.wrapping_mul(1_103_515_245).wrapping_add(12_345) % 100_000;
                self.changed();
            }
            *y += 26.0;
                }
            }
            (false, 1) => self.picture_options(p, input, y, left, col),
            _ => {
                if !sx {
            Self::heading(p, y, left, "SKY");
            let skies = mm2_course::SKIES;
            let at = skies.iter().position(|(f, _)| f.eq_ignore_ascii_case(&self.course.look.sky)).unwrap_or(0);
            if p.button(input, "sky", [left, *y, col, 20.0], skies[at].1, "Click for the next sky.", false) {
                self.remember();
                self.course.look.sky = skies[(at + 1) % skies.len()].0.into();
                self.changed();
            }
            *y += 26.0;
                }
        let sun = self.course.look.sun;
        if let Some(v) = self.slider(p, input, y, left, col, "sun", "Sun", "Low evening sun to high midday sun.", sun, (0.0, 1.0), if sun < 0.35 { "Evening".into() } else if sun < 0.7 { "Afternoon".into() } else { "Midday".into() }) {
            self.course.look.sun = v;
            self.changed();
        }
        let turn = self.course.look.sun_turn.rem_euclid(360.0);
        let compass = ["North", "North-east", "East", "South-east", "South", "South-west", "West", "North-west"][((turn + 22.5) / 45.0) as usize % 8];
        if let Some(v) = self.slider(p, input, y, left, col, "sunturn", "Sun from", "Which way the sunlight comes from, so the shadows fall where you want them.", turn, (0.0, 359.0), compass.into()) {
            self.course.look.sun_turn = v;
            self.changed();
        }
        let amb = self.course.look.ambient;
        if let Some(v) = self.slider(p, input, y, left, col, "ambient", "Shade", "How dark the shadows are: deep shade to soft, bright shade.", amb, (0.0, 1.0), if amb < 0.35 { "Dark".into() } else if amb < 0.7 { "Normal".into() } else { "Bright".into() }) {
            self.course.look.ambient = v;
            self.changed();
        }
        let fogs = mm2_course::FOGS;
        let at = fogs.iter().position(|(n, _)| n.eq_ignore_ascii_case(&self.course.look.fog));
        let label = format!("Haze: {}", at.map_or("the land's own", |i| fogs[i].0));
        if p.button(input, "fog", [left, *y, col, 20.0], &label, "The colour of the haze in the distance. Click for the next.", false) {
            self.remember();
            self.course.look.fog = match at {
                None => fogs[0].0.into(),
                Some(i) if i + 1 < fogs.len() => fogs[i + 1].0.into(),
                Some(_) => String::new(),
            };
            self.changed();
        }
        *y += 26.0;
            }
        }
        Self::note(p, y, left, col, "Click the name in the top bar to rename the track.");
    }
}

/// Reads a `.course` file, or the course inside a builder `.ENV`.
fn load_file(path: &Path) -> Option<Course> {
    let bytes = crate::fs::read(path).ok()?;
    let mut c = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("env")) {
        match mm2_course::env::course_in(&bytes) {
            Some(c) => c,
            // Any other track: a copy of it to change.
            None => {
                let name = format!("{} copy", path.file_stem()?.to_string_lossy());
                return mm2_course::copy::import(&bytes, &path.to_string_lossy(), None, 0, &name)
                    .map_err(|e| eprintln!("{}: {e}", path.display()))
                    .ok();
            }
        }
    } else {
        Course::from_json(&String::from_utf8_lossy(&bytes)).ok()?
    };
    // A copy reads its track again.
    if let Some(base) = c.base.clone() {
        let _ = crate::free_tracks::ensure(Path::new(&base.file));
        let bytes = crate::fs::read(&base.file).map_err(|e| eprintln!("{}: {e}", base.file)).ok()?;
        mm2_course::copy::attach(&mut c, &bytes).map_err(|e| eprintln!("{}: {e}", base.file)).ok()?;
    }
    Some(c)
}

/// A copy of the track at `path` (its `course`th course) to change.
fn copy_of(path: &Path, mode: Mode, course: usize, name: &str) -> Result<Course, String> {
    crate::free_tracks::ensure(path)?;
    let bytes = crate::fs::read(path).map_err(|e| e.to_string())?;
    mm2_course::copy::import(&bytes, &path.to_string_lossy(), Some(mode), course, &format!("{name} copy"))
}

#[cfg(test)]
mod pad_tests {
    use super::*;

    #[test]
    fn stuck_stick_does_not_turn_the_view() {
        let mut b = Builder::new(None, None);
        let yaw = b.cam.yaw;
        // The right stick held at its end from the start (a stick that
        // rests off centre): nothing, frame after frame.
        for _ in 0..60 {
            let [_, _, rx, ry, z] = b.pad_axes([0.0, 0.0, -1.0, 0.0, 0.0]);
            b.pad_view([rx, ry], z, [0.0; 2], 1.0 / 60.0);
        }
        assert_eq!(b.cam.yaw, yaw);
        // Once seen in the middle, it turns the view.
        b.pad_axes([0.0; 5]);
        let [_, _, rx, ry, z] = b.pad_axes([0.0, 0.0, -1.0, 0.0, 0.0]);
        b.pad_view([rx, ry], z, [0.0; 2], 1.0 / 60.0);
        assert!(b.cam.yaw != yaw);
    }
}

/// Shows `dir` in the computer's file browser.
#[cfg(not(target_arch = "wasm32"))]
fn open_folder(dir: &std::path::Path) {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(opener).arg(dir).spawn();
}
