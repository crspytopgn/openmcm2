//! The free pack: new, openly licensed art and sound that lets the game
//! run without the original's files. It is a plain folder:
//!
//! * `sound/engine/<class>/` for the classes `125`, `250`, `400`, `500` and
//!   `600`: one sound per engine sample (Ogg Vorbis, or a WAV), named
//!   after its list and a number (`Idle01.ogg`, `LowTorque02.ogg`, ...
//!   `Rev01.ogg`), and an optional
//!   `bands.ini` with the original's `LowTorqueMaxSpeed`,
//!   `MidTorqueMaxSpeed` and `HighTorqueMaxSpeed` (mph).
//!   With an `engine.ini` there (`OnLoop`, `OffLoop`, `IdleLoop` file
//!   names and the revs each was recorded at as `OnLoopRpm`, `OffLoopRpm`,
//!   `IdleLoopRpm`; `IdleRpm`, `MaxRpm`, and optionally `GearTopSpeeds` in mph), the class
//!   plays those three steady loops pitched to the bike's revs instead.
//! * `sound/fx/`: the race effects under the original's names
//!   (`landhard01`, `wreck01`, `fall01`, `crowdloop`, `applause01` ...),
//!   as `.ogg` or `.wav`. Any it lacks fall back to the original's.
//!
//! * `stockmap.toml`: the stand-ins for the original's models, as
//!   `[model]` and `[eco]` tables of `name = { file = "objects/x.glb" }`
//!   (or just the path). A track's model that the game's files lack is
//!   loaded from its `.glb` (see `mm2_formats::glb`).
//!
//! * `[bikes]` and `[riders]` in `stockmap.toml`: the pack's own bikes and
//!   rider outfits, `key = { model = "bikes/x.glb", livery = "bikes/p.png",
//!   cc = 250, four_stroke = false, name = "Blue 250" }` and `key = { model
//!   = "riders/r.glb", outfit = "riders/o.png", name = "Blue" }`. The
//!   models copy the original's frame trees (see [`BIKE_FRAMES`] and
//!   [`RIDER_FRAMES`]) so the game's motions drive them; the textured
//!   material takes the livery or outfit.
//!
//! The pack is used when `--freepack DIR` (in a browser, `?freepack=DIR`)
//! or `MM2_FREEPACK` names its folder. With no game
//! files at all, the pack's folder stands in for `RES`, and each archive
//! it lacks reads as empty.

use std::path::PathBuf;
use glam::{Mat4, Vec3};

/// The folder `--freepack` named, which goes before `MM2_FREEPACK`.
static GIVEN: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Uses the pack in `dir` (`--freepack DIR`): the launcher's and the
/// browser page's "free graphics". A browser has no environment to read.
pub fn set_root(dir: PathBuf) {
    let _ = GIVEN.set(dir);
}

/// The pack that fills in what a cut-down game lacks (`--fill-from DIR`):
/// the demo's missing tracks, bikes, menus and sounds, under the
/// original graphics. The same as `MM2_FREEPACK`, which a browser can't set.
static FILL: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Uses the pack in `dir` beside the game's own files (`--fill-from`).
pub fn set_fill(dir: PathBuf) {
    let _ = FILL.set(dir);
}

/// The game's folder (the full game's or the demo's), when there is one.
static GAME: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Notes the game's folder, so the free tracks (which live in the pack's
/// folder) can still ride the game's own bikes and hear its sounds.
pub fn set_game(game: &std::path::Path) {
    let _ = GAME.set(game.to_path_buf());
}

/// The game's own `RES` folder, for the bikes, riders, their numbers and
/// sounds on every track, the pack's free tracks too: unless the free
/// graphics were picked, the original's (or the demo's) win, and the pack
/// only fills in what they lack.
pub fn original_res() -> Option<PathBuf> {
    if chosen() {
        return None;
    }
    let res = GAME.get()?.join("RES");
    let has = |name: &str| crate::fs::is_file(res.join(name));
    (has("BIKE.RES") || has("ENGINE.RES") || has("AUDIO.RES")).then_some(res)
}

/// Whether the player picked the free graphics (`--freepack`), rather
/// than a pack set up beside the original's files.
pub fn chosen() -> bool {
    GIVEN.get().is_some()
}

/// Which menus the player picked: the new look (the pack's art), or the
/// original's. 0: not said (the new look goes with the free graphics).
static MENUS: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
/// The pack the new menus come from with the original graphics.
static MENUS_FROM: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Picks the menus: the new look (`--new-menus[=DIR]`, from the pack in
/// `dir` or the free graphics' own), or the original's
/// (`--original-menus`, which needs the game's files).
pub fn set_menus(new: bool, dir: Option<PathBuf>) {
    MENUS.store(if new { 1 } else { 2 }, std::sync::atomic::Ordering::Relaxed);
    if let Some(dir) = dir {
        let _ = MENUS_FROM.set(dir);
    }
}

/// The player's own MM2 (or demo) folder, for the original menus over
/// the free graphics.
static MENUS_GAME: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Notes the game whose menus go over the free graphics.
pub fn set_menus_game(game: &std::path::Path) {
    let _ = MENUS_GAME.set(game.to_path_buf());
}

/// The game folder the menus come from when it isn't the game's own:
/// with the free graphics picked (the pack's folder standing in for the
/// game's) and the original menus, the player's MM2's.
pub fn menus_game() -> Option<PathBuf> {
    (chosen() && ui_root().is_none()).then(|| MENUS_GAME.get().cloned()).flatten()
}

/// Whether the menus wear the new look.
pub fn new_menus() -> bool {
    ui_root().is_some()
}

/// The pack the menus' art, layout, words and sounds come from, when the
/// menus wear the new look.
pub fn ui_root() -> Option<PathBuf> {
    match MENUS.load(std::sync::atomic::Ordering::Relaxed) {
        2 => None,
        _ => MENUS_FROM.get().cloned().filter(|d| crate::fs::is_dir(d)).or_else(root),
    }
}

/// The free pack's folder, when one is in use.
pub fn root() -> Option<PathBuf> {
    let dir = match GIVEN.get().or(FILL.get()) {
        Some(dir) => dir.clone(),
        None => PathBuf::from(std::env::var_os("MM2_FREEPACK")?),
    };
    crate::fs::is_dir(&dir).then_some(dir)
}

/// The pack's engine sample folder for one class (`"125"` ... `"600"`).
pub fn engine_dir(class: &str) -> Option<PathBuf> {
    let dir = root()?.join("sound").join("engine").join(class);
    crate::fs::is_dir(&dir).then_some(dir)
}

/// The pack's race effects folder.
pub fn fx_dir() -> Option<PathBuf> {
    let dir = root()?.join("sound").join("fx");
    crate::fs::is_dir(&dir).then_some(dir)
}

/// The pack's stand-ins for the original's models, as `stockmap.toml`
/// lists them: model or eco name (lower case, no extension) to its
/// `.glb`, relative to the pack.
fn stockmap() -> &'static std::collections::HashMap<String, StandIn> {
    static MAP: std::sync::OnceLock<std::collections::HashMap<String, StandIn>> = std::sync::OnceLock::new();
    MAP.get_or_init(|| {
        let Some(root) = root() else { return Default::default() };
        let text = crate::fs::read_to_string(root.join("stockmap.toml")).unwrap_or_default();
        let map = parse_stockmap(&text, &root);
        if !map.is_empty() {
            eprintln!("free pack: {} stand-in models", map.len());
        }
        map
    })
}

/// The pack's sky boxes, `[sky] "<name>.cub" = [six face PNGs]`, by
/// lower-case name.
fn skies() -> &'static std::collections::HashMap<String, Vec<PathBuf>> {
    static MAP: std::sync::OnceLock<std::collections::HashMap<String, Vec<PathBuf>>> = std::sync::OnceLock::new();
    MAP.get_or_init(|| {
        let Some(root) = root() else { return Default::default() };
        let text = crate::fs::read_to_string(root.join("stockmap.toml")).unwrap_or_default();
        parse_skies(&text, &root)
    })
}

fn parse_skies(text: &str, root: &std::path::Path) -> std::collections::HashMap<String, Vec<PathBuf>> {
    let mut map = std::collections::HashMap::new();
    let mut section = String::new();
    for line in text.lines() {
        let line = without_comment(line).trim();
        if line.starts_with('[') {
            section = line.trim_matches(['[', ']', ' ']).to_ascii_lowercase();
            continue;
        }
        if section != "sky" {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let faces: Vec<PathBuf> = value
            .trim()
            .trim_matches(['[', ']'])
            .split(',')
            .map(|f| f.trim().trim_matches('"'))
            .filter(|f| !f.is_empty())
            .map(|f| root.join(f))
            .collect();
        if faces.len() == 6 {
            map.insert(key.trim().trim_matches('"').to_ascii_lowercase(), faces);
        }
    }
    map
}

/// The pack's sky box `name` (`terra05.cub`), for a scene whose sky the
/// game's files lack: every tile of the four sides' upper halves and of
/// the top, from six square face pictures.
pub fn sky(name: &str) -> Option<(mm2_formats::cub::Cube, crate::hires::SkyImages)> {
    use mm2_formats::cub::{Cube, TILES};
    use mm2_formats::trn::{PixelFormat, Texture};
    let faces = skies().get(&name.trim().to_ascii_lowercase())?;
    let mut side = 0;
    let mut pixels = Vec::new();
    for path in faces {
        let bytes = crate::fs::read(path).map_err(|e| eprintln!("{}: {e}", path.display())).ok()?;
        let (w, h, rgba) = crate::hires::decode_png(&bytes).map_err(|e| eprintln!("{}: {e}", path.display())).ok()?;
        if w != h || (side != 0 && w != side) {
            eprintln!("{}: sky faces must be square and the same size", path.display());
            return None;
        }
        side = w;
        pixels.extend(rgba);
    }
    let tile = || Some(Texture { size: 1, format: PixelFormat::Rgb555, levels: vec![vec![0]] });
    let cube = Cube {
        faces: (0..6)
            .map(|f| (0..TILES * TILES).map(|t| match f { 0..=3 => (t < TILES * 2).then(tile).flatten(), 4 => tile(), _ => None }).collect())
            .collect(),
    };
    Some((cube, crate::hires::SkyImages { side, faces: pixels }))
}

/// One stand-in: its `.glb`, and for a plant its height and spread (ft),
/// when the map gives them.
#[derive(Debug, Clone, PartialEq)]
struct StandIn {
    path: PathBuf,
    height: Option<f32>,
    radius: Option<f32>,
}

/// A plant stand-in's height and spread, ft, for placing it where the
/// original's eco-system data is missing.
pub fn plant_size(stem: &str) -> Option<(f32, f32)> {
    let s = stockmap().get(&stem.trim().to_ascii_lowercase())?;
    Some((s.height?, s.radius?))
}

/// Reads the `[model]` and `[eco]` tables: `name = "path"` or
/// `name = { file = "path", height = H, radius = R, ... }`.
fn parse_stockmap(text: &str, root: &std::path::Path) -> std::collections::HashMap<String, StandIn> {
    let mut map = std::collections::HashMap::new();
    let mut section = String::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            section = line.trim_matches(['[', ']', ' ']).to_ascii_lowercase();
            continue;
        }
        if section != "model" && section != "eco" {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let key = key.trim().trim_matches('"');
        let stem = key.rsplit_once('.').map_or(key, |(s, _)| s).trim().to_ascii_lowercase();
        let value = value.trim();
        let field = |name: &str| -> Option<String> {
            value.strip_prefix('{')?.split(',').find_map(|kv| {
                let (k, v) = kv.split_once('=')?;
                (k.trim() == name).then(|| v.trim().trim_end_matches('}').trim().trim_matches('"').to_string())
            })
        };
        let file = if value.starts_with('{') { field("file") } else { Some(value.trim_matches('"').to_string()) };
        let num = |name: &str| field(name).and_then(|v| v.parse::<f32>().ok()).filter(|v| *v > 0.0);
        if let Some(file) = file.filter(|f| f.ends_with(".glb")) {
            map.insert(stem, StandIn { path: root.join(file), height: num("height"), radius: num("radius") });
        }
    }
    map
}

/// The original bike's frames, in its order (`Honda.slb`).
pub const BIKE_FRAMES: [&str; 8] = ["Bike", "HandleBars", "KickStart", "NullFrame", "SwingArm", "Shocks", "TireFront", "TireRear"];
/// The original rider's frames, in its order (`Rider02.slb`).
pub const RIDER_FRAMES: [&str; 16] = [
    "Pelvis", "LwrTorso", "UprLegL", "UprLegR", "UprTorso", "Head", "UprArmL", "UprArmR", "LwrArmL", "HandL", "LwrArmR",
    "HandR", "LwrLegL", "FootL", "LwrLegR", "FootR",
];

/// A material texture that is drawn in its own colour and never takes
/// the rider's number, as the original's untextured bike material does.
pub const SOLID: &str = "#solid";

/// A material texture for a number plate: the rider's number printed on
/// a plain board in the material's colour.
pub const BOARD: &str = "#board";

/// One of the pack's bikes.
#[derive(Debug, Clone, PartialEq)]
pub struct Bike {
    /// The bike's shape: its model's file stem (`fp_bike_mx2`).
    pub shape: String,
    /// The livery's texture name: its file stem (`fp_livery_blue`).
    pub livery: String,
    pub cc: u32,
    pub four_stroke: bool,
    pub name: String,
    /// In the menus' bike list. The other colours of a model
    /// (`grid_only = true`) only paint the computer riders: the player
    /// picks their colours on the Kit & Paint page.
    pub listed: bool,
}

/// One of the pack's rider outfits.
#[derive(Debug, Clone, PartialEq)]
pub struct Outfit {
    pub shape: String,
    /// The outfit's texture name: its file stem.
    pub outfit: String,
    pub name: String,
}

#[derive(Debug, Default)]
struct Rides {
    bikes: Vec<Bike>,
    outfits: Vec<Outfit>,
    /// Shapes (lower case) to their `.glb` and whether each is a rider.
    shapes: std::collections::HashMap<String, (PathBuf, bool)>,
    /// Livery and outfit texture names (lower case) to their pictures.
    paints: std::collections::HashMap<String, PathBuf>,
}

fn rides() -> &'static Rides {
    static RIDES: std::sync::OnceLock<Rides> = std::sync::OnceLock::new();
    RIDES.get_or_init(|| {
        let Some(root) = root() else { return Rides::default() };
        let text = crate::fs::read_to_string(root.join("stockmap.toml")).unwrap_or_default();
        let rides = parse_rides(&text, &root);
        if !rides.bikes.is_empty() {
            eprintln!("free pack: {} bikes, {} outfits", rides.bikes.len(), rides.outfits.len());
        }
        rides
    })
}

/// A path's file stem, lower case.
fn stem_of(path: &str) -> String {
    let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
    file.rsplit_once('.').map_or(file, |(s, _)| s).to_ascii_lowercase()
}

/// `key = { a = "x", b = 2 }` as its fields.
fn inline_table(value: &str) -> Vec<(String, String)> {
    let Some(body) = value.trim().strip_prefix('{') else { return Vec::new() };
    body.trim_end_matches('}')
        .split(',')
        .filter_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            Some((k.trim().to_string(), v.trim().trim_matches('"').to_string()))
        })
        .collect()
}

fn parse_rides(text: &str, root: &std::path::Path) -> Rides {
    // The browser, and Low graphics on a computer, ride the lighter models.
    parse_rides_for(text, root, cfg!(target_arch = "wasm32") || crate::graphics::low_detail())
}

/// As [`parse_rides`]; `web` loads a ride's lighter `web_model` and
/// `web_livery` / `web_outfit` where the map gives them (the browser),
/// under the same names, so races and lobbies match across the two.
fn parse_rides_for(text: &str, root: &std::path::Path, web: bool) -> Rides {
    let mut rides = Rides::default();
    let mut section = String::new();
    for line in text.lines() {
        let line = without_comment(line).trim();
        if line.starts_with('[') {
            section = line.trim_matches(['[', ']', ' ']).to_ascii_lowercase();
            continue;
        }
        if section != "bikes" && section != "riders" {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let fields = inline_table(value);
        let get = |k: &str| fields.iter().find(|f| f.0 == k).map(|f| f.1.clone());
        let rider = section == "riders";
        let (Some(model), Some(paint)) = (get("model"), get(if rider { "outfit" } else { "livery" })) else { continue };
        let lighter = |k: &str, full: &String| if web { get(&format!("web_{k}")).unwrap_or_else(|| full.clone()) } else { full.clone() };
        let shape = stem_of(&model);
        rides.shapes.insert(shape.clone(), (root.join(lighter("model", &model)), rider));
        let texture = stem_of(&paint);
        rides.paints.insert(texture.clone(), root.join(lighter(if rider { "outfit" } else { "livery" }, &paint)));
        let name = get("name").unwrap_or_else(|| key.trim().trim_matches('"').to_string());
        if rider {
            rides.outfits.push(Outfit { shape, outfit: texture, name });
        } else {
            rides.bikes.push(Bike {
                shape,
                livery: texture,
                cc: get("cc").and_then(|v| v.parse().ok()).unwrap_or(250),
                four_stroke: get("four_stroke").is_some_and(|v| v == "true" || v == "1"),
                name,
                listed: !get("grid_only").is_some_and(|v| v == "true" || v == "1"),
            });
        }
    }
    rides
}

/// The pack's bikes, in the map's order.
pub fn bikes() -> &'static [Bike] {
    &rides().bikes
}

/// The pack's bikes the menus offer: one of each model and engine size.
pub fn listed_bikes() -> impl Iterator<Item = &'static Bike> {
    bikes().iter().filter(|b| b.listed)
}

/// The pack's rider outfits, in the map's order.
pub fn outfits() -> &'static [Outfit] {
    &rides().outfits
}

/// The rider model (`fp_rider.mcf`) an outfit of the pack goes on.
/// The podium's stand-in for the game's `Winner.mcf` when the rider on
/// the grid is one of the pack's (`fp_rider.mcf` stands as
/// `winfp_rider.mcf`): the same rider, playing the podium's motions (the
/// game's when it is there, else the pack's own).
pub fn winner_model(rider: &str) -> Option<String> {
    let (template, shape) = animated(rider)?;
    (template == "Rider.mcf").then(|| format!("win{shape}.mcf"))
}

pub fn outfit_model(outfit: &str) -> Option<String> {
    let outfit = outfit.trim().to_ascii_lowercase();
    let o = rides().outfits.iter().find(|o| o.outfit == outfit).or_else(|| {
        // A mixed outfit goes on its set's rider.
        let set = paint_set(&outfit)?;
        rides().outfits.iter().find(|o| o.outfit.strip_prefix(set.as_str()).is_some_and(|r| r.starts_with('_')))
    })?;
    Some(format!("{}.mcf", o.shape))
}

/// For an animated model the game's files lack but the pack has
/// (`fp_bike_mx2.mcf`, or `uifp_bike_mx2.mcf` for the menus' turning one):
/// the original's `.mcf` whose motions it plays, and the shape to load.
pub fn animated(name: &str) -> Option<(&'static str, String)> {
    let stem = stem_of(name);
    // `ui…` for the menus' turning one, `win…` for a rider on the podium.
    let (kind, shape) = match (stem.strip_prefix("ui"), stem.strip_prefix("win")) {
        (Some(rest), _) if rides().shapes.contains_key(rest) => (1, rest.to_string()),
        (_, Some(rest)) if rides().shapes.contains_key(rest) => (2, rest.to_string()),
        _ => (0, stem),
    };
    let &(_, rider) = rides().shapes.get(&shape)?;
    let template = match (kind, rider) {
        (0, false) => "Bike.mcf",
        (0, true) => "Rider.mcf",
        (1, false) => "UIHonda.mcf",
        (1, true) => "UIRider.mcf",
        (_, true) => crate::podium::WINNER,
        (_, false) => return None,
    };
    Some((template, shape))
}

/// For an animated model of the game's (`30SecMan.mcf`) that is missing,
/// as without the game: the pack's stand-in of the same name, to stand
/// still where the game's would have played its motions.
pub fn still_stand_in(name: &str) -> Option<String> {
    let stem = stem_of(name.trim());
    name.trim().to_ascii_lowercase().ends_with(".mcf").then_some(())?;
    stockmap().contains_key(&stem).then_some(stem)
}

/// The pack's supercross start gate, which the free tracks place as a
/// still model (mm2-course free.rs), and the animation the viewer makes
/// of it so that it drops as the original's `StartGate.mcf` does.
pub const GATE: &str = "fp_startgate";
pub const GATE_MCF: &str = "fp_startgate.mcf";

/// Whether `name` is the pack's start gate, still or animated.
pub fn is_gate(name: &str) -> bool {
    stem_of(name.trim()) == GATE
}

/// How long the pack's gate bars take to fall, in seconds (the original's
/// `Gate.mot`: 7 frames at 30 a second), and how far past upright they
/// come to rest, in degrees: forward over the hinge onto the dirt in
/// front, as a real one slaps down. (The original's own tips 36°.)
const GATE_FALL: f32 = 0.2;
const GATE_REST: f32 = 104.0;
const GATE_FPS: f32 = 30.0;

/// The pack's start gate made to drop. Its model is one piece, so the
/// drop bars (every face reaching 2 ft or more up: nothing else on it
/// stands that tall) go onto a frame of their own, hinged along their
/// foot; with them comes the motion ("Drop") that lays them forward,
/// away from the pad the riders stand on, from its first frame (the bars
/// up) when the race's gate drops. A model without such faces comes back
/// as it was, with no motion.
pub fn gate_drop(mut model: mm2_formats::slb::Model) -> (mm2_formats::slb::Model, Vec<(String, mm2_formats::anim::Motion, f32, bool)>) {
    use mm2_formats::anim::{Key, Motion};
    const TALL: f32 = 2.0;
    if model.frames.len() != 1 {
        return (model, Vec::new());
    }
    let Some(lod) = model.lods.first_mut() else { return (model, Vec::new()) };
    let is_bar = |s: &mm2_formats::slb::Surface, f: &[u16; 3]| f.iter().any(|&v| s.positions.get(v as usize).is_some_and(|p| p[1] >= TALL));
    // The hinge (the bars' lowest edge) and which way they fall.
    let (mut foot, mut bar_z, mut bars, mut rest_z, mut rest) = (f32::MAX, 0.0, 0usize, 0.0, 0usize);
    for s in &lod.surfaces {
        for f in &s.faces {
            let bar = is_bar(s, f);
            for &v in f {
                let p = s.positions[v as usize];
                if bar {
                    foot = foot.min(p[1]);
                    bar_z += p[2];
                    bars += 1;
                } else {
                    rest_z += p[2];
                    rest += 1;
                }
            }
        }
    }
    if bars == 0 || rest == 0 {
        return (model, Vec::new());
    }
    let (bar_z, rest_z) = (bar_z / bars as f32, rest_z / rest as f32);
    let ahead = if bar_z >= rest_z { 1.0 } else { -1.0 };
    // Each surface's faces, the rest first and the bars after, so each
    // run of vertices hangs on one frame.
    for s in &mut lod.surfaces {
        let old = s.clone();
        let mut out = mm2_formats::slb::Surface {
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            colors: Vec::new(),
            faces: Vec::new(),
            materials: old.materials.clone(),
            groups: Vec::new(),
        };
        let mut first_bar = 0;
        for pass in [false, true] {
            if pass {
                first_bar = out.positions.len() as u32;
            }
            let mut moved: std::collections::HashMap<u16, u16> = std::collections::HashMap::new();
            for f in old.faces.iter().filter(|f| is_bar(&old, f) == pass) {
                let face = f.map(|v| {
                    *moved.entry(v).or_insert_with(|| {
                        let i = v as usize;
                        out.positions.push(old.positions[i]);
                        if let Some(n) = old.normals.get(i) {
                            out.normals.push(*n);
                        }
                        if let Some(uv) = old.uvs.get(i) {
                            out.uvs.push(*uv);
                        }
                        out.colors.push(old.colors.get(i).copied().unwrap_or([255; 4]));
                        (out.positions.len() - 1) as u16
                    })
                });
                out.faces.push(face);
            }
        }
        out.groups = if first_bar == 0 {
            vec![(1, 0)]
        } else if first_bar as usize == out.positions.len() {
            vec![(0, 0)]
        } else {
            vec![(0, 0), (1, first_bar)]
        };
        *s = out;
    }
    // (The pack's model has its one frame where the model is, so the
    // hinge's place on it is the bars' own.)
    let hinge = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, foot, bar_z, 1.0];
    model.frames.push(mm2_formats::slb::Frame { name: "bars".into(), matrix: hinge, parent: Some(0) });
    // Falling as a hinged bar does, faster and faster, then a little
    // bounce off the dirt.
    let at = |deg: f32| {
        let (s, c) = deg.to_radians().sin_cos();
        vec![Key { frame: 1, look: [0.0, -ahead * s, c], up: [0.0, c, ahead * s], position: [0.0, foot, bar_z] }]
    };
    let falling = (GATE_FALL * GATE_FPS).round() as usize;
    let mut frames: Vec<Vec<Key>> = (0..=falling).map(|k| at(GATE_REST * (k as f32 / falling as f32).powi(2))).collect();
    frames.extend([GATE_REST - 7.0, GATE_REST - 2.0, GATE_REST].map(at));
    (model, vec![("Drop".into(), Motion { frames }, GATE_FPS, false)])
}


/// The free rider's or bike's own riding motions, for when the original's
/// aren't there (the free graphics on their own): the same names and
/// phases as the game's (lean, turn, wheelie, cross-ups, looking back,
/// backing up, the crash's bails, falls and landings, and the tricks), so
/// the riding mix, the wreck and the stunts drive them as they do the
/// game's. Made
/// for the pack by IK onto the free bikes' seat, pegs and bars
/// (`tools/freepack/free_motions.py`), not taken from the game.
pub fn own_motions(rider: bool) -> Vec<(String, mm2_formats::anim::Motion, f32, bool)> {
    own_table().iter().filter(|m| m.rider == rider).map(|m| (m.name.clone(), m.motion.clone(), m.fps, m.looping)).collect()
}

/// One of the pack's own motions.
struct OwnMotion {
    name: String,
    fps: f32,
    looping: bool,
    rider: bool,
    motion: mm2_formats::anim::Motion,
}

/// The pack's own motions, read from `free_motions.bin` (made by
/// `tools/freepack/free_motions.py`; see there for the layout). Built
/// into the desktop game, but in a browser read from the page's store
/// (`web/index.html` fetches it while the menus are up, so the browser
/// build doesn't carry its 1.0 MB). An empty table is tried again on the
/// next call, so the free riders ride properly once it arrives.
fn own_table() -> &'static [OwnMotion] {
    static TABLE: std::sync::Mutex<Option<&'static [OwnMotion]>> = std::sync::Mutex::new(None);
    let mut table = TABLE.lock().unwrap_or_else(|e| e.into_inner());
    if table.as_ref().map_or(true, |t| t.is_empty()) {
        let found = load_own_table();
        if !found.is_empty() {
            *table = Some(Box::leak(found.into_boxed_slice()));
        } else if table.is_none() {
            *table = Some(&[]);
        }
    }
    match *table {
        Some(t) => t,
        None => &[],
    }
}

/// Reads the table from wherever it lives for this build.
fn load_own_table() -> Vec<OwnMotion> {
    // `MM2_FREE_MOTIONS=FILE`: try a freshly made table without a rebuild.
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(t) = std::env::var_os("MM2_FREE_MOTIONS").and_then(|p| std::fs::read(p).ok()).and_then(|d| read_own(&d)) {
        return t;
    }
    #[cfg(not(target_arch = "wasm32"))]
    return read_own(include_bytes!("free_motions.bin")).unwrap_or_default();
    #[cfg(target_arch = "wasm32")]
    return crate::fs::read("/assets/free_motions.bin").ok().and_then(|d| read_own(&d)).unwrap_or_default();
}

fn read_own(data: &[u8]) -> Option<Vec<OwnMotion>> {
    if data.get(..4)? != b"FPMO" {
        return None;
    }
    let mut at = 4usize;
    let mut take = |n: usize| -> Option<&[u8]> {
        let b = data.get(at..at + n)?;
        at += n;
        Some(b)
    };
    let count = u16::from_le_bytes(take(2)?.try_into().ok()?);
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let len = take(1)?[0] as usize;
        let name = String::from_utf8(take(len)?.to_vec()).ok()?;
        let fps = f32::from_le_bytes(take(4)?.try_into().ok()?);
        let (looping, rider) = (take(1)?[0] != 0, take(1)?[0] != 0);
        let frames = u16::from_le_bytes(take(2)?.try_into().ok()?) as usize;
        let keys = take(1)?[0] as usize;
        let mut motion = mm2_formats::anim::Motion { frames: Vec::with_capacity(frames) };
        for _ in 0..frames {
            let mut list = Vec::with_capacity(keys);
            for _ in 0..keys {
                let b = take(19)?;
                let q = |i: usize| i16::from_le_bytes([b[1 + 2 * i], b[2 + 2 * i]]) as f32;
                let axis = |i: usize| [q(i) / 30000.0, q(i + 1) / 30000.0, q(i + 2) / 30000.0];
                list.push(mm2_formats::anim::Key {
                    frame: b[0] as usize,
                    look: axis(0),
                    up: axis(3),
                    position: [q(6) / 1000.0, q(7) / 1000.0, q(8) / 1000.0],
                });
            }
            motion.frames.push(list);
        }
        out.push(OwnMotion { name, fps, looping, rider, motion });
    }
    Some(out)
}

/// The tricks for the pack's own riders, as the race reads them: each
/// trick's own rider motion with its frames a second and length, and the
/// bike's (`b<trick>`, as the game names them) where the pack has one.
pub fn own_tricks() -> Vec<Option<crate::race::TrickMotion>> {
    let names = mm2_physics::stunt::TRICKS
        .iter()
        .map(|t| t.to_string())
        .chain(mm2_physics::stunt::TRICKS.iter().map(|t| format!("{t}Alt")));
    names
        .map(|name| {
            let m = own_table().iter().find(|m| m.rider && m.name.eq_ignore_ascii_case(&name))?;
            let bike = format!("b{name}");
            let bike = own_table()
                .iter()
                .find(|b| !b.rider && b.name.eq_ignore_ascii_case(&bike))
                .map_or(String::new(), |b| b.name.clone());
            Some(crate::race::TrickMotion {
                rider: m.name.clone(),
                bike,
                fps: m.fps,
                frames: m.motion.frames.len() as i32,
            })
        })
        .collect()
}

/// Whether `name` (an animated model's `.mcf`) is one of the pack's
/// riders (`Some(true)`) or bikes (`Some(false)`), as the game drives them.
pub fn ridden(name: &str) -> Option<bool> {
    match animated(name)?.0 {
        "Rider.mcf" | "UIRider.mcf" | crate::podium::WINNER => Some(true),
        "Bike.mcf" | "UIHonda.mcf" => Some(false),
        _ => None,
    }
}

/// Whether `name` is the pack's rider on a bike or in the menus, which
/// rides the pack's own motions even when the original's are there
/// (not the podium's winner, which keeps the original's stand).
pub fn rides_own_motions(name: &str) -> bool {
    std::env::var_os("MM2_ORIGINAL_RIDER_MOTIONS").is_none()
        && matches!(animated(name).map(|a| a.0), Some("Rider.mcf" | "UIRider.mcf"))
}

/// Whether `name` is one of the pack's liveries or outfits.
pub fn is_paint(name: &str) -> bool {
    let stem = stem_of(name);
    rides().paints.contains_key(&stem) || mixed(&stem).is_some()
}

/// A livery or outfit picture by its texture name.
pub fn paint(name: &str) -> Option<crate::hires::Image> {
    let stem = stem_of(name);
    let Some(path) = rides().paints.get(&stem) else {
        return mix(&stem);
    };
    let (w, h, rgba) = read_png(path)?;
    Some(image(w, h, &rgba))
}

fn read_png(path: &std::path::Path) -> Option<(u32, u32, Vec<u8>)> {
    let bytes = crate::fs::read(path).map_err(|e| eprintln!("{}: {e}", path.display())).ok()?;
    crate::hires::decode_png(&bytes).map_err(|e| eprintln!("{}: {e}", path.display())).ok()
}

/// The paint colours a mixed livery or outfit takes (`fp_rider_hq_blue_white`:
/// blue with white trim), as the HD set's own colourways have them.
pub const COLOURS: [(&str, [f32; 3]); 12] = [
    ("black", [0.08, 0.08, 0.09]),
    ("blue", [0.06, 0.25, 0.78]),
    ("green", [0.05, 0.45, 0.15]),
    ("grey", [0.42, 0.43, 0.45]),
    ("lime", [0.55, 0.80, 0.05]),
    ("navy", [0.05, 0.10, 0.32]),
    ("orange", [0.95, 0.42, 0.02]),
    ("purple", [0.38, 0.10, 0.62]),
    ("red", [0.80, 0.07, 0.06]),
    ("teal", [0.0, 0.50, 0.52]),
    ("white", [0.92, 0.92, 0.90]),
    ("yellow", [0.98, 0.80, 0.05]),
];

fn colour(name: &str) -> Option<[f32; 3]> {
    COLOURS.iter().find(|c| c.0 == name).map(|c| c.1)
}

/// A mixed paint's colours: main, trim, and the helmet's (the kit's main
/// colour unless named).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mix {
    pub main: [f32; 3],
    pub trim: [f32; 3],
    pub helmet: [f32; 3],
}

/// The set a livery or outfit belongs to when the set has paint masks
/// (`fp_rider_hq_blue` -> `fp_rider_hq`): its colours can be mixed.
pub fn paint_set(name: &str) -> Option<String> {
    let stem = stem_of(name);
    let mut set = stem.as_str();
    while let Some((rest, last)) = set.rsplit_once('_') {
        if colour(last).is_none() {
            break;
        }
        set = rest;
    }
    (set != stem && masks(set).is_some()).then(|| set.to_string())
}

/// The masks of a paint set: `<set>.mask.png` and `<set>.base.png`, beside
/// its colourways.
fn masks(set: &str) -> Option<(PathBuf, PathBuf)> {
    let dir = rides()
        .paints
        .iter()
        // only `<set>_<colour>...` names, so `fp_rider` never picks up `fp_rider_hq_blue`
        .find(|(k, _)| {
            k.strip_prefix(set)
                .and_then(|r| r.strip_prefix('_'))
                .is_some_and(|r| r.split('_').all(|c| colour(c).is_some()))
        })?
        .1
        .parent()?
        .to_path_buf();
    let mask = dir.join(format!("{set}.mask.png"));
    crate::fs::is_file(&mask).then(|| (mask, dir.join(format!("{set}.base.png"))))
}

/// For a livery or outfit named `<set>_<main>_<trim>[_<helmet>]` whose set
/// has paint masks: the masks and the colours.
fn mixed(stem: &str) -> Option<(PathBuf, PathBuf, Mix)> {
    let mut set = stem;
    let mut cols = Vec::new();
    while cols.len() < 3 {
        let Some((rest, last)) = set.rsplit_once('_') else { break };
        let Some(c) = colour(last) else { break };
        cols.insert(0, c);
        set = rest;
    }
    let (main, trim) = (*cols.first()?, *cols.get(1)?);
    let (mask, base) = masks(set)?;
    Some((mask, base, Mix { main, trim, helmet: cols.get(2).copied().unwrap_or(main) }))
}

/// The texture name of a set's paint in these colours.
pub fn mix_name(set: &str, main: &str, trim: &str, helmet: Option<&str>) -> String {
    match helmet {
        Some(h) => format!("{set}_{main}_{trim}_{h}"),
        None => format!("{set}_{main}_{trim}"),
    }
}

/// A mixed livery or outfit, painted from its set's masks: the main colour
/// where the mask's red is (the helmet's where its blue is), the trim where
/// its green is, both shaded by its alpha, over everything else (rubber,
/// metal, skin, the dark kit panels).
fn mix(stem: &str) -> Option<crate::hires::Image> {
    let (mask, base, cols) = mixed(stem)?;
    let (w, h, m) = read_png(&mask)?;
    let (bw, bh, b) = read_png(&base)?;
    if (w, h) != (bw, bh) {
        return None;
    }
    Some(image(w, h, &mix_pixels(&m, &b, cols)))
}

fn mix_pixels(mask: &[u8], base: &[u8], c: Mix) -> Vec<u8> {
    let mut out = vec![255u8; mask.len()];
    for ((o, m), b) in out.chunks_exact_mut(4).zip(mask.chunks_exact(4)).zip(base.chunks_exact(4)) {
        let [r, g, hm, a] = [m[0], m[1], m[2], m[3]].map(|v| v as f32 / 255.0);
        for i in 0..3 {
            let main = c.main[i] + (c.helmet[i] - c.main[i]) * hm;
            let v = (r * main + g * c.trim[i]) * a + b[i] as f32 / 255.0;
            o[i] = (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        }
    }
    out
}

/// A bike or rider shape: its frames as the original's, the textured
/// material first (the one a livery or outfit replaces), and the rest
/// drawn in their own colours.
fn shape(stem: &str) -> Option<(mm2_formats::slb::Model, Vec<mm2_formats::glb::Image>, PathBuf)> {
    let (path, rider) = rides().shapes.get(stem)?.clone();
    let data = crate::fs::read(&path).map_err(|e| eprintln!("{}: {e}", path.display())).ok()?;
    let frames: &[&str] = if rider { &RIDER_FRAMES } else { &BIKE_FRAMES };
    let (mut model, images) = mm2_formats::glb::parse_framed(&data, &format!("fp:{stem}:"), frames)
        .map_err(|e| eprintln!("{}: {e}", path.display()))
        .ok()?;
    let mut names = mm2_formats::glb::material_names(&data).unwrap_or_default();
    if let Some(t) = model.materials.iter().position(|m| !m.texture.is_empty()).filter(|&t| t != 0) {
        model.materials.swap(0, t);
        if t < names.len() {
            names.swap(0, t);
        }
        let swap = |m: u32| if m == 0 { t as u32 } else if m == t as u32 { 0 } else { m };
        for s in model.lods.iter_mut().flat_map(|l| l.surfaces.iter_mut()) {
            s.materials.iter_mut().for_each(|m| *m = swap(*m));
        }
    }
    // A material named "plate" shows the rider's number; the rest keep
    // their colours.
    let plates: Vec<bool> = (0..model.materials.len()).map(|i| names.get(i).is_some_and(|n| n.eq_ignore_ascii_case("plate"))).collect();
    for (m, &plate) in model.materials.iter_mut().zip(&plates) {
        if plate {
            m.texture = BOARD.to_string();
        } else if m.texture.is_empty() {
            m.texture = SOLID.to_string();
        }
    }
    Some((model, images, path))
}

/// The race markers drawn over the riders and the course: the sign over
/// the player's bike, Moto-Tag's IT sign, the runway pointer, the Visual
/// Cue arrow and the countdown's figures.
const MARKERS: [&str; 11] =
    ["youind", "itind", "pointer", "viscue", "one", "two", "three", "four", "five", "x", "decimalpoint"];

/// The most a marker's material glows (linear, per channel). The pack's
/// first markers glowed at three times white, which the enhanced
/// lighting's bloom blew up into a blaze over the rider; at this they
/// still read in shade and stay under the bloom's white.
const MARKER_GLOW: f32 = 0.3;

fn soften_glow(materials: &mut [mm2_formats::slb::Material]) {
    for m in materials {
        if let Some(p) = m.pbr.as_mut() {
            let most = p.emissive_factor.iter().copied().fold(0.0f32, f32::max);
            if most > MARKER_GLOW {
                p.emissive_factor = p.emissive_factor.map(|e| e * MARKER_GLOW / most);
            }
        }
    }
}

/// The pack's checkpoint arches: the stock `waypnt` and the variants the
/// pack adds beside it (an inflatable rainbow arch, a truss gantry, a
/// second square inflatable), so a Baja or Enduro course isn't one arch
/// over and over.
const WAYPOINT_VARIANTS: [&str; 4] = ["waypnt", "fp_waypnt_2", "fp_waypnt_3", "fp_waypnt_4"];

/// The arch to stand at checkpoint `index` of a course whose own name
/// hashes to `seed`: the variants the pack holds in turn, from a start the
/// course picks, so each track has its own order and neighbouring gates
/// differ. With an older pack, or none, it is the stock arch.
pub fn waypoint_variant(index: usize, seed: u32) -> &'static str {
    let have: Vec<&'static str> = WAYPOINT_VARIANTS.iter().copied().filter(|v| stockmap().contains_key(*v)).collect();
    if have.len() < 2 {
        return WAYPOINT_VARIANTS[0];
    }
    have[(index + seed as usize) % have.len()]
}

/// The pack's stand-in for the model `stem` (bare, no extension), and the
/// images its materials name.
pub fn model(stem: &str) -> Option<(mm2_formats::slb::Model, Vec<(String, crate::hires::Image)>)> {
    let stem = stem.trim().to_ascii_lowercase();
    let (model, images, path) = match shape(&stem) {
        Some(found) => found,
        None => {
            let path = stockmap().get(&stem)?.path.clone();
            let data = crate::fs::read(&path).map_err(|e| eprintln!("{}: {e}", path.display())).ok()?;
            let (mut model, images) = mm2_formats::glb::parse(&data, &format!("fp:{stem}:"))
                .map_err(|e| eprintln!("{}: {e}", path.display()))
                .ok()?;
            if MARKERS.contains(&stem.as_str()) {
                soften_glow(&mut model.materials);
            }
            split_blades(&stem, &mut model);
            make_props(&stem, &mut model);
            (model, images, path)
        }
    };
    let images = images
        .into_iter()
        .filter_map(|mut im| {
            // A picture several models share: a file beside the model.
            if let Some(uri) = &im.uri {
                let file = beside(&path, uri);
                im.data = crate::fs::read(&file).map_err(|e| eprintln!("{}: {e}", file.display())).ok()?;
            }
            let decoded = match im.mime.as_str() {
                "image/png" => crate::hires::decode_png(&im.data),
                "image/jpeg" => decode_jpeg(&im.data),
                other => Err(format!("{other} is not read").into()),
            };
            let (w, h, rgba) = decoded
                .map_err(|e| eprintln!("{}: {}: {e}", path.display(), im.name))
                .ok()?;
            Some((im.name, image(w, h, &rgba)))
        })
        .collect();
    Some((model, images))
}

/// The free pack's craft are each one piece; the original's turn their
/// propellers and rotors on frames of their own. Those parts are split
/// out here onto frames named as the original's, so they can spin
/// (crate::drive::BLADES): each model's part as the box it lies in, game
/// axes, its hub (the middle of the box when not given) and the axis it
/// turns about (the frame's z; z when not given).
type Part = (&'static str, &'static str, [f32; 3], [f32; 3], Option<[f32; 3]>, Option<[f32; 3]>);
const BLADE_PARTS: [Part; 8] = [
    ("supercub", "Cub_Prop", [-2.0, -4.0, -11.0], [2.0, 4.0, -9.5], None, None),
    // The helicopter stands 6 ft off its middle; its rotor turns on the
    // mast top, and the tail rotor on the boom's left.
    ("bellranger", "BellRngr_Main-Rotor", [-25.0, 11.07, -55.0], [35.0, 14.0, 1.5], Some([6.09, 11.6, -26.11]), None),
    ("bellranger", "BellRngr_Tail-Rotor", [4.6, 4.0, 2.5], [4.95, 12.2, 8.8], Some([4.83, 8.09, 5.63]), None),
    ("helinocam", "BellRngr_Main-Rotor", [-25.0, 11.07, -55.0], [35.0, 14.0, 1.5], Some([6.09, 11.6, -26.11]), None),
    ("helinocam", "BellRngr_Tail-Rotor", [4.6, 4.0, 2.5], [4.95, 12.2, 8.8], Some([4.83, 8.09, 5.63]), None),
    ("beech18", "Prop1", [2.6, -4.6, -22.1], [13.8, 7.2, -20.7], Some([8.15, 1.3, -21.5]), None),
    ("beech18", "Prop2", [-13.8, -4.6, -22.1], [-2.6, 7.2, -20.7], Some([-8.15, 1.3, -21.5]), None),
    // The biplane was made nose to -x.
    ("biplane", "Bip_Prop", [-11.45, -5.2, -1.8], [-10.85, 3.2, 1.3], Some([-11.1, -1.0, -0.33]), Some([-1.0, 0.0, 0.0])),
];

/// Propellers the free pack's craft were made without (their engines
/// end in a spinner), made here on frames of their own: each one's model,
/// frame, hub and axis (game axes, nose-ward), its radius, ft, and how
/// many blades it has. The DC-3's were drawn as a still disc, which is
/// taken away.
type Prop = (&'static str, &'static str, [f32; 3], [f32; 3], f32, usize);
const MADE_PROPS: [Prop; 6] = [
    ("dc-3", "PropL", [0.26, 3.06, -35.41], [0.428, 0.0, -0.904], 5.4, 3),
    ("dc-3", "PropR", [17.6, 3.1, -27.23], [0.428, 0.0, -0.904], 5.4, 3),
    ("cargoplane", "Prop1", [40.6, 4.74, -77.3], [1.0, 0.0, 0.0], 6.4, 4),
    ("cargoplane", "Prop2", [40.6, 4.74, -52.25], [1.0, 0.0, 0.0], 6.4, 4),
    ("cargoplane", "Prop3", [40.6, 4.74, -8.45], [1.0, 0.0, 0.0], 6.4, 4),
    ("cargoplane", "Prop4", [40.6, 4.74, 16.55], [1.0, 0.0, 0.0], 6.4, 4),
];
/// The models whose props were a still disc, drawn within this far of
/// the hub along its axis and round it, ft.
const PROP_DISCS: [(&str, f32, f32); 1] = [("dc-3", 0.3, 5.7)];

/// A frame at `hub` whose z is `axis`, as a row-vector matrix.
fn hub_frame(hub: [f32; 3], axis: Option<[f32; 3]>) -> [f32; 16] {
    let z = Vec3::from(axis.unwrap_or([0.0, 0.0, 1.0])).normalize();
    let x = if z.y.abs() > 0.9 { Vec3::X } else { Vec3::Y.cross(z).normalize() };
    let y = z.cross(x);
    let mut m = [0.0f32; 16];
    m[0..3].copy_from_slice(&x.to_array());
    m[4..7].copy_from_slice(&y.to_array());
    m[8..11].copy_from_slice(&z.to_array());
    m[12..15].copy_from_slice(&hub);
    m[15] = 1.0;
    m
}

/// Makes `stem`'s missing propellers (MADE_PROPS): thin twisted blades,
/// black with yellow tips, in the material of the spinner they sit on.
fn make_props(stem: &str, model: &mut mm2_formats::slb::Model) {
    let props: Vec<&Prop> = MADE_PROPS.iter().filter(|p| p.0 == stem).collect();
    if props.is_empty() || model.frames.len() != 1 {
        return;
    }
    for prop in props {
        let frame = model.frames.len() as u32;
        let matrix = hub_frame(prop.2, Some(prop.3));
        model.frames.push(mm2_formats::slb::Frame { name: prop.1.into(), matrix, parent: Some(0) });
        let place = Mat4::from_cols_array(&matrix);
        let hub = Vec3::from(prop.2);
        let axis = Vec3::from(prop.3).normalize();
        let disc = PROP_DISCS.iter().find(|d| d.0 == stem);
        for lod in &mut model.lods {
            if let Some(&(_, depth, radius)) = disc {
                let in_disc = |p: &[f32; 3]| {
                    let q = Vec3::from(*p) - hub;
                    let along = q.dot(axis);
                    along.abs() <= depth && (q - along * axis).length() <= radius
                };
                for s in &mut lod.surfaces {
                    let positions = &s.positions;
                    s.faces.retain(|f| !f.iter().all(|&v| positions.get(v as usize).is_some_and(in_disc)));
                }
            }
            // The spinner: the surface with the point nearest the hub.
            let Some((near, v)) = lod
                .surfaces
                .iter()
                .enumerate()
                .flat_map(|(i, s)| s.positions.iter().enumerate().map(move |(v, p)| (i, v, Vec3::from(*p).distance(hub))))
                .min_by(|a, b| a.2.total_cmp(&b.2))
                .map(|(i, v, _)| (i, v))
            else {
                continue;
            };
            let like = &lod.surfaces[near];
            let uv = like.uvs.get(v).copied();
            let mut s = mm2_formats::slb::Surface {
                positions: Vec::new(),
                normals: Vec::new(),
                uvs: Vec::new(),
                colors: Vec::new(),
                faces: Vec::new(),
                materials: like.materials.clone(),
                groups: vec![(frame, 0)],
            };
            let r = prop.4;
            for b in 0..prop.5 {
                let turn = b as f32 * std::f32::consts::TAU / prop.5 as f32;
                // Root to tip in four pieces, the last yellow; each piece
                // a flat box, twisted less towards the tip.
                let stations = [0.35, 0.45, 0.7, 0.88, 1.0];
                for k in 0..4 {
                    let colour = if k == 3 { [0, 190, 235, 255] } else { [28, 28, 30, 255] };
                    let corner = |st: usize, side: f32, face: f32| {
                        let t = stations[st];
                        let chord = r * (0.13 - 0.05 * t);
                        let pitch = (55.0 - 35.0 * t).to_radians();
                        let thick = r * 0.012;
                        let (sp, cp) = pitch.sin_cos();
                        // Along the blade (x), across it (y) and its face (z).
                        let local = Vec3::new(t * r, side * chord * 0.5 * cp - face * thick * sp, side * chord * 0.5 * sp + face * thick * cp);
                        place.transform_point3(Mat4::from_rotation_z(turn).transform_point3(local))
                    };
                    let quad = |s: &mut mm2_formats::slb::Surface, q: [Vec3; 4]| {
                        let n = (q[1] - q[0]).cross(q[2] - q[0]).normalize_or_zero();
                        let at = s.positions.len() as u16;
                        for p in q {
                            s.positions.push(p.to_array());
                            s.normals.push(n.to_array());
                            s.colors.push(colour);
                            if let Some(uv) = uv {
                                s.uvs.push(uv);
                            }
                        }
                        s.faces.push([at, at + 1, at + 2]);
                        s.faces.push([at, at + 2, at + 3]);
                    };
                    let c = |st, side, face| corner(st, side, face);
                    let (a, e) = (k, k + 1);
                    // Front and back faces, both edges, and the tip.
                    quad(&mut s, [c(a, -1.0, 1.0), c(a, 1.0, 1.0), c(e, 1.0, 1.0), c(e, -1.0, 1.0)]);
                    quad(&mut s, [c(a, 1.0, -1.0), c(a, -1.0, -1.0), c(e, -1.0, -1.0), c(e, 1.0, -1.0)]);
                    quad(&mut s, [c(a, 1.0, 1.0), c(a, 1.0, -1.0), c(e, 1.0, -1.0), c(e, 1.0, 1.0)]);
                    quad(&mut s, [c(a, -1.0, -1.0), c(a, -1.0, 1.0), c(e, -1.0, 1.0), c(e, -1.0, -1.0)]);
                    if k == 3 {
                        quad(&mut s, [c(e, -1.0, 1.0), c(e, 1.0, 1.0), c(e, 1.0, -1.0), c(e, -1.0, -1.0)]);
                    }
                }
            }
            lod.surfaces.push(s);
        }
    }
}

fn split_blades(stem: &str, model: &mut mm2_formats::slb::Model) {
    let parts: Vec<&Part> = BLADE_PARTS.iter().filter(|p| p.0 == stem).collect();
    if parts.is_empty() || model.frames.len() != 1 {
        return;
    }
    let inside = |part: &Part, p: &[f32; 3]| (0..3).all(|i| p[i] >= part.2[i] && p[i] <= part.3[i]);
    // Each part's frame, at its hub.
    let first = model.frames.len() as u32;
    for part in &parts {
        let hub = part.4.unwrap_or_else(|| [0, 1, 2].map(|i| (part.2[i] + part.3[i]) / 2.0));
        let matrix = hub_frame(hub, part.5);
        model.frames.push(mm2_formats::slb::Frame { name: part.1.into(), matrix, parent: Some(0) });
    }
    for lod in &mut model.lods {
        for s in &mut lod.surfaces {
            if s.groups.len() > 1 {
                continue;
            }
            // Each face on the body (0) or on the first part it lies in.
            let n = s.positions.len();
            let side: Vec<usize> = s
                .faces
                .iter()
                .map(|f| {
                    parts
                        .iter()
                        .position(|part| f.iter().all(|&v| s.positions.get(v as usize).is_some_and(|p| inside(part, p))))
                        .map_or(0, |k| k + 1)
                })
                .collect();
            if side.iter().all(|&k| k == 0) {
                continue;
            }
            // The body's points first, then each part's, each copied once.
            let mut remap = vec![u32::MAX; n * (parts.len() + 1)];
            let (mut order, mut faces, mut groups) = (Vec::new(), Vec::new(), Vec::new());
            for k in 0..=parts.len() {
                let start = order.len() as u32;
                let before = faces.len();
                for (f, _) in s.faces.iter().zip(&side).filter(|(_, &sd)| sd == k) {
                    faces.push(f.map(|v| {
                        let key = v as usize + k * n;
                        if remap[key] == u32::MAX {
                            remap[key] = order.len() as u32;
                            order.push(v as usize);
                        }
                        remap[key] as u16
                    }));
                }
                if faces.len() > before || k == 0 {
                    groups.push((if k == 0 { 0 } else { first + k as u32 - 1 }, start));
                }
            }
            if order.len() > u16::MAX as usize {
                continue;
            }
            let pick = |len: usize| len == n;
            if pick(s.positions.len()) {
                s.positions = order.iter().map(|&i| s.positions[i]).collect();
            }
            if pick(s.normals.len()) {
                s.normals = order.iter().map(|&i| s.normals[i]).collect();
            }
            if pick(s.uvs.len()) {
                s.uvs = order.iter().map(|&i| s.uvs[i]).collect();
            }
            if pick(s.colors.len()) {
                s.colors = order.iter().map(|&i| s.colors[i]).collect();
            }
            s.faces = faces;
            s.groups = groups;
        }
    }
}

/// The pack's stand-in for the model `stem`, without its pictures (for
/// its shape alone, as a solid object's collision).
pub fn model_shape(stem: &str) -> Option<mm2_formats::slb::Model> {
    let stem = stem.trim().to_ascii_lowercase();
    if let Some((model, _, _)) = shape(&stem) {
        return Some(model);
    }
    let path = stockmap().get(&stem)?.path.clone();
    let data = crate::fs::read(&path).map_err(|e| eprintln!("{}: {e}", path.display())).ok()?;
    mm2_formats::glb::parse(&data, &format!("fp:{stem}:")).ok().map(|(model, _)| model)
}

/// `uri` (a glTF image path, `/`-separated) from the folder `model` is in,
/// with `..` taken out so the browser's file list finds it too.
fn beside(model: &std::path::Path, uri: &str) -> PathBuf {
    let mut out = model.parent().map(PathBuf::from).unwrap_or_default();
    for part in uri.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            p => out.push(p),
        }
    }
    out
}

/// Decodes a JPEG as top-down RGBA8.
fn decode_jpeg(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), Box<dyn std::error::Error>> {
    use zune_jpeg::zune_core::{bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions};
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
    let mut decoder = zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(bytes), options);
    let rgba = decoder.decode().map_err(|e| format!("{e:?}"))?;
    let info = decoder.info().ok_or("no JPEG header")?;
    Ok((info.width as u32, info.height as u32, rgba))
}

/// A top-down RGBA8 image as a square power-of-two texture with its mip
/// chain, in the game's bottom-up rows.
fn image(w: u32, h: u32, rgba: &[u8]) -> crate::hires::Image {
    let side = w.max(h).next_power_of_two().clamp(1, 2048) as usize;
    let mut top = vec![0u8; side * side * 4];
    for y in 0..side {
        // Bottom-up: the last row first.
        let sy = ((side - 1 - y) * h as usize / side).min(h as usize - 1);
        for x in 0..side {
            let sx = (x * w as usize / side).min(w as usize - 1);
            let s = (sy * w as usize + sx) * 4;
            top[(y * side + x) * 4..][..4].copy_from_slice(&rgba[s..s + 4]);
        }
    }
    let mut levels = vec![top];
    let mut s = side / 2;
    while s > 0 {
        let next = mm2_formats::hires::downsample(levels.last().unwrap(), s);
        levels.push(next);
        s /= 2;
    }
    crate::hires::Image { size: side as u32, levels }
}

/// The pack's menu art, `ui/<DAT>/<path in the DAT>.png` (the path's
/// backslashes as slashes, any case), and `ui/<DAT>.toml`:
/// `[controls.Name] rect = [x, y, w, h]` moves or resizes a control (as
/// the dialog gives it, in 640x480 pixels), and `[images]
/// "path/in/dat.png" = [w, h]` sets the size a picture covers (else half
/// its pixels: the pack's art is drawn at twice the 640x480 size).
struct UiArt {
    files: std::collections::HashMap<String, PathBuf>,
    rects: std::collections::HashMap<(String, String), [i32; 4]>,
    /// Controls whose label the pack's picture already shows
    /// (`text = false`): the game's text isn't drawn over it.
    no_text: std::collections::HashSet<(String, String)>,
    sizes: std::collections::HashMap<String, (u32, u32)>,
    /// Controls the pack adds, by dialog.
    added: std::collections::HashMap<String, Vec<NewControl>>,
}

/// A button the pack adds to a dialog (`new = true` in its
/// `[controls.Name]`): `images = { up, lit, down, grey }` (paths in the
/// pack's folder for that dialog), `tip`, and `action` (`open_url:<url>`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NewControl {
    pub name: String,
    pub rect: [i32; 4],
    /// Up, lit, down and grey pictures, as names `Ui::image` finds.
    pub images: [Option<String>; 4],
    pub tip: Option<String>,
    pub action: Option<String>,
}

fn ui_art() -> Option<&'static UiArt> {
    static ART: std::sync::OnceLock<Option<UiArt>> = std::sync::OnceLock::new();
    ART.get_or_init(|| load_art(ui_root()?.join("ui"))).as_ref()
}

/// The pack's menu art for the screens the game lacks (the demo's), with
/// the original menus.
fn fill_art() -> Option<&'static UiArt> {
    static ART: std::sync::OnceLock<Option<UiArt>> = std::sync::OnceLock::new();
    ART.get_or_init(|| load_art(screens_root()?.join("ui"))).as_ref()
}

/// The dialogs the game has no `UI/*.DAT` for, standing in from the pack.
static FILLED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Notes that dialog `stem` comes from the pack's layout, so its pictures
/// and places do too even with the original menus.
pub fn mark_filled(stem: &str) {
    let stem = stem.to_ascii_lowercase();
    let mut f = FILLED.lock().unwrap();
    if !f.contains(&stem) {
        f.push(stem);
    }
}

/// The art dialog `stem` wears: the new look's, or for a filled-in dialog
/// with the original menus, the pack's.
fn art_for(stem: &str) -> Option<&'static UiArt> {
    ui_art().or_else(|| {
        let filled = FILLED.lock().unwrap().contains(&stem.to_ascii_lowercase());
        filled.then(fill_art).flatten()
    })
}

/// Where the pack's screens come from: the new look's pack, or with the
/// original menus, the pack filling in a game that lacks some (the demo).
fn screens_root() -> Option<PathBuf> {
    ui_root().or_else(|| FILL.get().cloned().filter(|d| crate::fs::is_dir(d)))
}

fn load_art(dir: PathBuf) -> Option<UiArt> {
    let mut art = UiArt { files: Default::default(), rects: Default::default(), no_text: Default::default(), sizes: Default::default(), added: Default::default() };
    let mut stack = vec![dir.clone()];
    while let Some(d) = stack.pop() {
        for e in crate::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if crate::fs::is_dir(&p) {
                stack.push(p);
                continue;
            }
            let Ok(rel) = p.strip_prefix(&dir) else { continue };
            let rel = rel.to_string_lossy().replace('\\', "/").to_ascii_lowercase();
            if rel.ends_with(".png") {
                art.files.insert(rel, p);
            } else if let Some(stem) = rel.strip_suffix(".toml").filter(|s| !s.contains('/')) {
                let text = crate::fs::read_to_string(&p).unwrap_or_default();
                let (rects, sizes) = parse_layout(&text);
                let added = parse_added(&text);
                if !added.is_empty() {
                    art.added.insert(stem.to_string(), added);
                }
                art.rects.extend(rects.into_iter().map(|(name, r)| ((stem.to_string(), name), r)));
                art.no_text.extend(parse_no_text(&text).into_iter().map(|name| (stem.to_string(), name)));
                art.sizes.extend(sizes.into_iter().map(|(path, size)| (format!("{stem}/{path}"), size)));
            }
        }
    }
    eprintln!("free pack: {} menu pictures", art.files.len());
    Some(art)
}

/// A layout file's control rects (by lower-case name) and picture sizes
/// (by lower-case path).
fn parse_layout(text: &str) -> (Vec<(String, [i32; 4])>, Vec<(String, (u32, u32))>) {
    let (mut rects, mut sizes) = (Vec::new(), Vec::new());
    let mut section = String::new();
    let numbers = |v: &str| -> Vec<f32> {
        v.trim().trim_start_matches('[').trim_end_matches(']').split(',').filter_map(|n| n.trim().parse().ok()).collect()
    };
    for line in text.lines() {
        let line = without_comment(line).trim();
        if line.starts_with('[') {
            section = line.trim_matches(['[', ']']).trim().to_string();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let key = key.trim().trim_matches('"');
        if let Some(name) = section.strip_prefix("controls.") {
            let v = numbers(value);
            if key == "rect" && v.len() == 4 {
                rects.push((name.trim_matches('"').to_ascii_lowercase(), [v[0], v[1], v[2], v[3]].map(|x| x.round() as i32)));
            }
        } else if section == "images" {
            let v = numbers(value);
            if v.len() == 2 && v[0] > 0.0 && v[1] > 0.0 {
                sizes.push((key.replace('\\', "/").to_ascii_lowercase(), (v[0] as u32, v[1] as u32)));
            }
        }
    }
    (rects, sizes)
}

/// The controls a layout file marks `text = false`.
fn parse_no_text(text: &str) -> Vec<String> {
    let mut section = String::new();
    let mut out = Vec::new();
    for line in text.lines() {
        let line = without_comment(line).trim();
        if line.starts_with('[') {
            section = line.trim_matches(['[', ']']).trim().to_string();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        if let Some(name) = section.strip_prefix("controls.") {
            if key.trim() == "text" && value.trim() == "false" {
                out.push(name.trim_matches('"').to_ascii_lowercase());
            }
        }
    }
    out
}

/// A TOML line without its `#` comment (a `#` inside quotes stays).
fn without_comment(line: &str) -> &str {
    let mut quoted = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '#' if !quoted => return &line[..i],
            _ => {}
        }
    }
    line
}

/// The buttons a layout file adds (its `new = true` controls).
fn parse_added(text: &str) -> Vec<NewControl> {
    let mut out: Vec<(NewControl, bool)> = Vec::new();
    let string = |v: &str| v.trim().trim_matches('"').to_string();
    for line in text.lines() {
        let line = without_comment(line).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(head) = line.strip_prefix('[') {
            let head = head.trim_end_matches(']').trim();
            if let Some(name) = head.strip_prefix("controls.") {
                out.push((NewControl { name: name.trim_matches('"').to_string(), ..Default::default() }, false));
            } else {
                out.push((NewControl::default(), false));
            }
            continue;
        }
        let Some((c, new)) = out.last_mut() else { continue };
        if c.name.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let value = value.trim();
        match key.trim() {
            "new" => *new = value == "true",
            "rect" => {
                let v: Vec<f32> = value.trim_matches(['[', ']']).split(',').filter_map(|n| n.trim().parse().ok()).collect();
                if v.len() == 4 {
                    c.rect = [v[0], v[1], v[2], v[3]].map(|x| x.round() as i32);
                }
            }
            "tip" => c.tip = Some(string(value)),
            "action" => c.action = Some(string(value)),
            "images" => {
                for kv in value.trim_matches(['{', '}']).split(',') {
                    let Some((k, v)) = kv.split_once('=') else { continue };
                    let slot = match k.trim() {
                        "up" => 0,
                        "lit" => 1,
                        "down" => 2,
                        "grey" => 3,
                        _ => continue,
                    };
                    // Found by `Ui::image` as `<path>.tga`: no extension.
                    let path = string(v);
                    let path = path.rsplit_once('.').map_or(path.as_str(), |(p, _)| p).to_string();
                    c.images[slot] = Some(path);
                }
            }
            _ => {}
        }
    }
    out.into_iter().filter(|(c, new)| *new && !c.name.is_empty()).map(|(c, _)| c).collect()
}

/// The buttons the pack adds to dialog `stem`.
pub fn ui_added(stem: &str) -> &'static [NewControl] {
    art_for(stem).and_then(|a| a.added.get(&stem.to_ascii_lowercase())).map_or(&[], |v| v.as_slice())
}

/// What a button the pack added to dialog `stem` does when clicked.
pub fn ui_action(stem: &str, name: &str) -> Option<&'static str> {
    ui_added(stem).iter().find(|c| c.name.eq_ignore_ascii_case(name))?.action.as_deref()
}

/// The pack's picture for `file` in the dialog archive `stem`, and the
/// size it should cover when the pack sets one.
pub fn ui_image(stem: &str, file: &str) -> Option<(mm2_formats::tga::Image, Option<(u32, u32)>)> {
    ui_image_in(stem, stem, file)
}

/// [`ui_image`] for a picture dialog `dialog` shows (one of GLOBAL's in a
/// dialog the pack fills in wears the pack's art, like the rest of it).
pub fn ui_image_in(dialog: &str, stem: &str, file: &str) -> Option<(mm2_formats::tga::Image, Option<(u32, u32)>)> {
    let art = art_for(dialog)?;
    let path = file.replace('\\', "/").to_ascii_lowercase();
    let path = path.rsplit_once('.').map_or(path.as_str(), |(p, _)| p).to_string();
    let key = format!("{}/{path}", stem.to_ascii_lowercase());
    let png = art.files.get(&format!("{key}.png"))?;
    let bytes = crate::fs::read(png).ok()?;
    let (width, height, rgba) = crate::hires::decode_png(&bytes)
        .map_err(|e| eprintln!("{}: {e}", png.display()))
        .ok()?;
    Some((mm2_formats::tga::Image { width, height, rgba }, art.sizes.get(&format!("{key}.png")).copied()))
}

/// The pack's stand-in for the game's `RES/GLOBAL.RES` picture `name`
/// (the dirt particles, the terrain's detail noise): `global/<name>.png`.
pub fn global_image(name: &str) -> Option<mm2_formats::tga::Image> {
    let stem = name.rsplit_once('.').map_or(name, |(s, _)| s).to_ascii_lowercase();
    let png = root()?.join("global").join(format!("{stem}.png"));
    let bytes = crate::fs::read(&png).ok()?;
    let (width, height, mut rgba) = crate::hires::decode_png(&bytes)
        .map_err(|e| eprintln!("{}: {e}", png.display()))
        .ok()?;
    // The detail noises are grey pictures, but the terrain blends to the
    // detail's colour by its alpha (the game's are one dark earth colour
    // with the noise in alpha): turn the grey into that.
    if rgba.chunks_exact(4).all(|p| p[3] == 255 && p[0] == p[1] && p[1] == p[2]) {
        for p in rgba.chunks_exact_mut(4) {
            let a = ((p[0] as f32 - DETAIL_GREY) * DETAIL_GAIN).clamp(0.0, 255.0) as u8;
            p.copy_from_slice(&[DETAIL_EARTH[0], DETAIL_EARTH[1], DETAIL_EARTH[2], a]);
        }
    }
    Some(mm2_formats::tga::Image { width, height, rgba })
}

/// The race display's pictures the pack draws in the game's layout, each
/// the size of the game's own (640x480 pixels), the pack's at any
/// multiple of that.
const HUD: [(&str, (u32, u32)); 3] = [("instr1", (128, 128)), ("bajascore", (128, 128)), ("radar128", (128, 128))];

/// The pack's own picture for the race display's `name` (`instr1.tga`,
/// `BajaScore.tga`, `radar128.tga`), for playing without the game's
/// `RES/GLOBAL.RES`: the image and the size it covers.
pub fn hud_image(name: &str) -> Option<(mm2_formats::tga::Image, (u32, u32))> {
    let stem = name.rsplit_once('.').map_or(name, |(s, _)| s).to_ascii_lowercase();
    let size = HUD.iter().find(|(n, _)| *n == stem)?.1;
    let png = root()?.join("hud").join(format!("{stem}.png"));
    let bytes = crate::fs::read(&png).ok()?;
    let (width, height, rgba) = crate::hires::decode_png(&bytes)
        .map_err(|e| eprintln!("{}: {e}", png.display()))
        .ok()?;
    Some((mm2_formats::tga::Image { width, height, rgba }, size))
}

/// Whether the pack has all of the race display's pictures.
pub fn has_hud() -> bool {
    root().is_some_and(|r| HUD.iter().all(|(n, _)| crate::fs::is_file(&r.join("hud").join(format!("{n}.png")))))
}

/// A grey detail noise's turn into the terrain's detail: the earth colour
/// it darkens towards, and how its grey becomes the blend (the pack's
/// greys, 70 to 175 about a middle of 127, to the game's 0 to 150 about 67).
const DETAIL_EARTH: [u8; 3] = [34, 17, 0];
const DETAIL_GREY: f32 = 83.0;
const DETAIL_GAIN: f32 = 1.53;

/// The pack's place for control `name` of dialog `stem`.
/// Whether the menus wear the pack's new look.
pub fn ui_art_on() -> bool {
    ui_art().is_some()
}

/// Whether the pack's picture for control `name` already shows its label.
pub fn ui_no_text(stem: &str, name: &str) -> bool {
    art_for(stem).is_some_and(|a| a.no_text.contains(&(stem.to_ascii_lowercase(), name.to_ascii_lowercase())))
}

pub fn ui_rect(stem: &str, name: &str) -> Option<[i32; 4]> {
    art_for(stem)?.rects.get(&(stem.to_ascii_lowercase(), name.to_ascii_lowercase())).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_glow_softly() {
        let glowing = |e: [f32; 3]| mm2_formats::slb::Material {
            texture: String::new(),
            pbr: Some(mm2_formats::slb::Pbr { emissive_factor: e, ..Default::default() }),
        };
        let mut m = [glowing([3.0, 2.46, 0.36]), glowing([0.1, 0.0, 0.0])];
        soften_glow(&mut m);
        let e = m[0].pbr.as_ref().unwrap().emissive_factor;
        assert!((e[0] - MARKER_GLOW).abs() < 1e-6 && (e[1] - 0.246).abs() < 1e-4, "{e:?}");
        assert_eq!(m[1].pbr.as_ref().unwrap().emissive_factor, [0.1, 0.0, 0.0]);
    }

    #[test]
    fn own_motions_cover_riding_crashes_and_tricks() {
        let rider = own_motions(true);
        for name in ["LeanR02", "Turn2D", "CrosOv2", "Endo", "Kahuna", "Fall01", "HeadHit", "SuperMan", "BarKneelAlt", "Victory", "WaitR", "StretchR", "Podium3_02", "Podium4_01", "Podium5_03"] {
            assert!(rider.iter().any(|m| m.0 == name), "{name}");
        }
        assert!(rider.iter().filter(|m| m.0 != "BackUpBottom").all(|m| m.1.frames.iter().all(|f| f.len() == 16)));
        assert!(own_motions(false).iter().any(|m| m.0 == "BTurn2D"));
        let tricks = own_tricks();
        assert_eq!(tricks.len(), 32);
        assert_eq!(tricks.iter().flatten().count(), 22);
        let kneel = tricks[0].as_ref().unwrap();
        assert_eq!((kneel.fps, kneel.frames), (18.0, 30));
        assert_eq!(kneel.bike, "bBarKneel");
    }

    #[test]
    fn reads_the_stockmap() {
        let text = "missing = [\"x\"]\n[model]\ntriplebale = { file = \"objects/triplebale.glb\", kind = \"prop\" }\n\"GiantTire01.slt\" = {file=\"objects/gianttire01.glb\"}\n[eco]\npine01 = \"eco/pine01.glb\"\n[sky]\n\"terra05.cub\" = [\"a.png\"]\n";
        let map = parse_stockmap(text, std::path::Path::new("/p"));
        assert_eq!(map.len(), 3);
        assert_eq!(map["triplebale"].path, PathBuf::from("/p/objects/triplebale.glb"));
        assert_eq!(map["gianttire01"].path, PathBuf::from("/p/objects/gianttire01.glb"));
        assert_eq!(map["pine01"].path, PathBuf::from("/p/eco/pine01.glb"));
        let map = parse_stockmap("[eco]\nsag = { file = \"eco/sag.glb\", height = 22.5, radius = 4, sway = 0 }\n", std::path::Path::new("/p"));
        assert_eq!((map["sag"].height, map["sag"].radius), (Some(22.5), Some(4.0)));
    }

    #[test]
    fn reads_an_added_button() {
        let text = "[controls.MenuHelp]\nrect = [1, 2, 3, 4]\n[controls.FreeDonate]\nrect = [452, 256, 160, 56]\nnew = true  # added\nkind = \"button\"\nimages = { up = \"01_Main/FreeDonateUp.png\", lit = \"01_Main/FreeDonateLit.png\", down = \"01_Main/FreeDonateDown.png\", grey = \"01_Main/FreeDonateGrey.png\" }\ntip = \"Support #1 OpenMCM2\"\naction = \"open_url:https://github.com/sponsors/crspytopgn\"\n";
        let added = parse_added(text);
        assert_eq!(added.len(), 1);
        let c = &added[0];
        assert_eq!(c.name, "FreeDonate");
        assert_eq!(c.rect, [452, 256, 160, 56]);
        assert_eq!(c.images[0].as_deref(), Some("01_Main/FreeDonateUp"));
        assert_eq!(c.images[3].as_deref(), Some("01_Main/FreeDonateGrey"));
        assert_eq!(c.action.as_deref(), Some("open_url:https://github.com/sponsors/crspytopgn"));
    }

    #[test]
    fn reads_the_bikes_and_outfits() {
        let text = "[bikes]\nfp_bike_mx2_blue = { model = \"bikes/fp_bike_mx2.glb\", livery = \"bikes/fp_livery_blue.png\", cc = 125, four_stroke = false, name = \"Blue 125\" }\n\
                    [riders]\nfp_rider_red = { model = \"riders/fp_rider.glb\", outfit = \"riders/fp_rider_red.png\", name = \"Red\" }\n";
        let rides = parse_rides(text, std::path::Path::new("/p"));
        assert_eq!(
            rides.bikes,
            vec![Bike { shape: "fp_bike_mx2".into(), livery: "fp_livery_blue".into(), cc: 125, four_stroke: false, name: "Blue 125".into(), listed: true }]
        );
        assert_eq!(rides.outfits, vec![Outfit { shape: "fp_rider".into(), outfit: "fp_rider_red".into(), name: "Red".into() }]);
        assert_eq!(rides.shapes["fp_rider"], (std::path::PathBuf::from("/p/riders/fp_rider.glb"), true));
        assert!(rides.paints.contains_key("fp_livery_blue"));
    }

    #[test]
    fn other_colours_only_paint_the_grid() {
        let text = "[bikes]\na = { model = \"bikes/fp_bike_mx2.glb\", livery = \"bikes/fp_livery_blue.png\", name = \"Ridgeback 250\" }\n\
                    b = { model = \"bikes/fp_bike_mx2.glb\", livery = \"bikes/fp_livery_red.png\", name = \"Ridgeback 250\", grid_only = true }\n";
        let rides = parse_rides(text, std::path::Path::new("/p"));
        assert_eq!(rides.bikes.iter().map(|b| b.listed).collect::<Vec<_>>(), [true, false]);
    }

    #[test]
    fn the_browser_loads_the_lighter_ride() {
        let text = "[bikes]\nfp_bike_hq_blue = { model = \"bikes/hq/fp_bike_hq.glb\", livery = \"bikes/hq/fp_livery_hq_blue.png\", web_model = \"bikes/hq/web/fp_bike_hq.glb\", web_livery = \"bikes/hq/web/fp_livery_hq_blue.png\", cc = 250 }\n";
        for web in [false, true] {
            let rides = parse_rides_for(text, std::path::Path::new("/p"), web);
            assert_eq!(rides.bikes[0].shape, "fp_bike_hq");
            assert_eq!(rides.bikes[0].livery, "fp_livery_hq_blue");
            let dir = if web { "/p/bikes/hq/web" } else { "/p/bikes/hq" };
            assert_eq!(rides.shapes["fp_bike_hq"].0, PathBuf::from(format!("{dir}/fp_bike_hq.glb")));
            assert_eq!(rides.paints["fp_livery_hq_blue"], PathBuf::from(format!("{dir}/fp_livery_hq_blue.png")));
        }
    }

    #[test]
    fn mixes_a_paint_from_its_masks() {
        // Full main colour, half trim, a quarter shade over a grey base.
        let mask = [255, 0, 0, 255, 0, 255, 0, 128];
        let base = [0, 0, 0, 255, 64, 64, 64, 255];
        let c = Mix { main: [1.0, 0.5, 0.0], trim: [0.0, 0.0, 1.0], helmet: [0.0, 1.0, 0.0] };
        let out = mix_pixels(&mask, &base, c);
        assert_eq!(&out[..4], &[255, 128, 0, 255]);
        assert_eq!(&out[4..7], &[64, 64, 192]);
        // On the helmet (mask blue) the main colour is the helmet's.
        let out = mix_pixels(&[255, 0, 255, 255], &[0, 0, 0, 255], c);
        assert_eq!(&out[..3], &[0, 255, 0]);
        assert_eq!(mix_name("fp_rider_hq", "red", "white", Some("black")), "fp_rider_hq_red_white_black");
        assert_eq!(colour("teal"), Some([0.0, 0.50, 0.52]));
        assert_eq!(colour("pink"), None);
    }

    #[test]
    fn reads_the_skies() {
        let map = parse_skies("[sky]\n\"Terra05.cub\" = [\"a0.png\", \"a1.png\", \"a2.png\", \"a3.png\", \"a4.png\", \"a5.png\"]\n", std::path::Path::new("/p"));
        assert_eq!(map["terra05.cub"][5], PathBuf::from("/p/a5.png"));
    }

    #[test]
    fn reads_a_menu_layout() {
        let (rects, sizes) = parse_layout("[controls.SingleBtn]\nrect = [10, 20, 200, 48] # bigger\n[images]\n\"01_Main/SingleUp.png\" = [200, 48]\n");
        assert_eq!(rects, vec![("singlebtn".to_string(), [10, 20, 200, 48])]);
        assert_eq!(sizes, vec![("01_main/singleup.png".to_string(), (200, 48))]);
    }
}

/// The pack's own layout of the front-end dialog `name` (`MAINMENU`), from
/// `menus/<NAME>.dtm`: the menus with no game files.
pub fn menu_layout(name: &str) -> Option<Vec<u8>> {
    let dir = screens_root()?.join("menus");
    let want = format!("{name}.dtm");
    crate::fs::read(dir.join(&want)).ok().or_else(|| {
        // Any case.
        let found = crate::fs::read_dir(&dir).ok()?.flatten().find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(&want))?;
        crate::fs::read(found.path()).ok()
    })
}

/// The Free Pack's own menu sound `file` (`Beep13.wav`), from `sound/ui`:
/// as Ogg Vorbis (`Beep13.ogg`) first, else the WAV.
pub fn ui_sound(file: &str) -> Option<Vec<u8>> {
    let dir = ui_root()?.join("sound").join("ui");
    mm2_formats::wav::other_name(file)
        .and_then(|ogg| crate::fs::read(dir.join(ogg)).ok())
        .or_else(|| crate::fs::read(dir.join(file)).ok())
}

/// Whether the pack has menus of its own.
pub fn has_menus() -> bool {
    menu_layout("MAINMENU").is_some()
}

/// Whether the free graphics' pack has menus of its own, whichever menus
/// were picked.
pub fn pack_has_menus() -> bool {
    root().is_some_and(|r| {
        let dir = r.join("menus");
        crate::fs::is_file(dir.join("MAINMENU.dtm"))
            || crate::fs::read_dir(&dir).ok().is_some_and(|mut d| d.any(|e| e.is_ok_and(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case("MAINMENU.dtm"))))
    })
}

/// The menus' text in the pack's own words (`menus/strings.txt`: lines of
/// `id<TAB>text`, `\n` for a new line), standing in for `UILANG.DLL`.
pub fn menu_strings() -> std::collections::HashMap<u32, String> {
    menu_text("strings.txt")
}

/// The game's own text in the pack's words (`menus/lang.txt`), standing
/// in for `LANG.DLL`.
pub fn game_strings() -> std::collections::HashMap<u32, String> {
    menu_text("lang.txt")
}

/// The pack's words for the menus and the game, for the screens it fills
/// in: what the game's own `UILANG.DLL` and `LANG.DLL` lack.
pub fn fill_strings() -> (std::collections::HashMap<u32, String>, std::collections::HashMap<u32, String>) {
    if ui_root().is_some() || FILL.get().is_none() {
        return Default::default();
    }
    (menu_text_in(screens_root(), "strings.txt"), menu_text_in(screens_root(), "lang.txt"))
}

fn menu_text(name: &str) -> std::collections::HashMap<u32, String> {
    menu_text_in(ui_root(), name)
}

fn menu_text_in(root: Option<PathBuf>, name: &str) -> std::collections::HashMap<u32, String> {
    let Some(root) = root else { return Default::default() };
    let text = crate::fs::read_to_string(root.join("menus").join(name)).unwrap_or_default();
    text.lines()
        .filter_map(|l| {
            let (id, s) = l.split_once('\t')?;
            Some((id.trim().parse().ok()?, s.replace("\\n", "\n")))
        })
        .collect()
}

/// One of the free menus' own files (`menus/pc-schedule.txt`), as text.
pub fn menu_file(name: &str) -> Option<String> {
    crate::fs::read_to_string(screens_root()?.join("menus").join(name)).ok()
}

/// The credits to roll with no credits movie (`menus/credits.txt`).
pub fn credits() -> Option<Vec<String>> {
    let text = crate::fs::read_to_string(screens_root()?.join("menus").join("credits.txt")).ok()?;
    Some(text.lines().map(str::to_string).collect())
}
