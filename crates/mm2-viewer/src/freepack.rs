//! The free pack: new, openly licensed art and sound that lets the game
//! run without the original's files. It is a plain folder:
//!
//! * `sound/engine/<class>/` for the classes `125`, `250`, `400`, `500` and
//!   `600`: one WAV per engine sample, named after its list and a number
//!   (`Idle01.wav`, `LowTorque02.wav`, ... `Rev01.wav`), and an optional
//!   `bands.ini` with the original's `LowTorqueMaxSpeed`,
//!   `MidTorqueMaxSpeed` and `HighTorqueMaxSpeed` (mph).
//!   With an `engine.ini` there (`OnLoop`, `OffLoop`, `IdleLoop` file
//!   names and the revs each was recorded at as `OnLoopRpm`, `OffLoopRpm`,
//!   `IdleLoopRpm`; `IdleRpm`, `MaxRpm`, and optionally `GearTopSpeeds` in mph), the class
//!   plays those three steady loops pitched to the bike's revs instead.
//! * `sound/fx/`: the race effects under the original's names
//!   (`landhard01.wav`, `wreck01.wav`, `fall01.wav`, `crowdloop.wav`,
//!   `applause01.wav` ...). Any it lacks fall back to the original's.
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

/// The folder `--freepack` named, which goes before `MM2_FREEPACK`.
static GIVEN: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Uses the pack in `dir` (`--freepack DIR`): the launcher's and the
/// browser page's "free graphics". A browser has no environment to read.
pub fn set_root(dir: PathBuf) {
    let _ = GIVEN.set(dir);
}

/// Whether the player picked the free graphics (`--freepack`), rather
/// than a pack set up beside the original's files.
pub fn chosen() -> bool {
    GIVEN.get().is_some()
}

/// The free pack's folder, when one is in use.
pub fn root() -> Option<PathBuf> {
    let dir = match GIVEN.get() {
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
        let shape = stem_of(&model);
        rides.shapes.insert(shape.clone(), (root.join(&model), rider));
        let texture = stem_of(&paint);
        rides.paints.insert(texture.clone(), root.join(&paint));
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
            });
        }
    }
    rides
}

/// The pack's bikes, in the map's order.
pub fn bikes() -> &'static [Bike] {
    &rides().bikes
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
    rides().outfits.iter().find(|o| o.outfit == outfit).map(|o| format!("{}.mcf", o.shape))
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

/// The pack's own motions, read once from `free_motions.bin` (made by
/// `tools/freepack/free_motions.py`; see there for the layout).
fn own_table() -> &'static [OwnMotion] {
    static TABLE: std::sync::OnceLock<Vec<OwnMotion>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| read_own(include_bytes!("free_motions.bin")).unwrap_or_default())
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
/// trick's own motion with its frames a second and length (no bike
/// motion: the bike holds still under the rider).
pub fn own_tricks() -> Vec<Option<crate::race::TrickMotion>> {
    let names = mm2_physics::stunt::TRICKS
        .iter()
        .map(|t| t.to_string())
        .chain(mm2_physics::stunt::TRICKS.iter().map(|t| format!("{t}Alt")));
    names
        .map(|name| {
            let m = own_table().iter().find(|m| m.rider && m.name.eq_ignore_ascii_case(&name))?;
            Some(crate::race::TrickMotion {
                rider: m.name.clone(),
                bike: String::new(),
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
        "Rider.mcf" | crate::podium::WINNER => Some(true),
        "Bike.mcf" => Some(false),
        _ => None,
    }
}

/// Whether `name` is one of the pack's liveries or outfits.
pub fn is_paint(name: &str) -> bool {
    rides().paints.contains_key(&stem_of(name))
}

/// A livery or outfit picture by its texture name.
pub fn paint(name: &str) -> Option<crate::hires::Image> {
    let path = rides().paints.get(&stem_of(name))?;
    let bytes = crate::fs::read(path).map_err(|e| eprintln!("{}: {e}", path.display())).ok()?;
    let (w, h, rgba) = crate::hires::decode_png(&bytes).map_err(|e| eprintln!("{}: {e}", path.display())).ok()?;
    Some(image(w, h, &rgba))
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

/// The collision hull of the pack's bike `name` (it has no `.col`): the
/// frame's shell round every part but the wheels, in model space, and
/// each wheel's round its own centre.
pub fn bike_hull(name: &str) -> Option<mm2_physics::bike::Hull> {
    let stem = stem_of(name);
    if rides().shapes.get(&stem)?.1 {
        return None;
    }
    let (model, _, _) = shape(&stem)?;
    let world = model.world_matrices();
    let wheel = |name: &str| BIKE_FRAMES.iter().position(|f| *f == name);
    let (front, rear) = (wheel("TireFront")?, wheel("TireRear")?);
    let (body, bars) = (wheel("Bike")?, wheel("HandleBars")?);
    let mut parts: [Vec<glam::Vec3>; 3] = Default::default();
    for s in model.lods.first()?.surfaces.iter() {
        for (v, p) in s.positions.iter().enumerate() {
            let frame = s.frame_of(v);
            let p = glam::Vec3::from(*p);
            let (list, centre) = match frame {
                f if f == front => (1, world[f]),
                f if f == rear => (2, world[f]),
                // The swingarm, shocks and kickstart hang below the frame
                // and move: the game's hull leaves them out too.
                f if f == body || f == bars => (0, [0.0; 16]),
                _ => continue,
            };
            parts[list].push(p - glam::Vec3::new(centre[12], centre[13], centre[14]));
        }
    }
    let [frame, a, b] = &parts;
    Some(mm2_physics::bike::Hull::from_points(frame, [a, b]))
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
            let (model, images) = mm2_formats::glb::parse(&data, &format!("fp:{stem}:"))
                .map_err(|e| eprintln!("{}: {e}", path.display()))
                .ok()?;
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
    ART.get_or_init(|| {
        let dir = root()?.join("ui");
        let mut art = UiArt { files: Default::default(), rects: Default::default(), sizes: Default::default(), added: Default::default() };
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
                    art.sizes.extend(sizes.into_iter().map(|(path, size)| (format!("{stem}/{path}"), size)));
                }
            }
        }
        eprintln!("free pack: {} menu pictures", art.files.len());
        Some(art)
    })
    .as_ref()
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
    ui_art().and_then(|a| a.added.get(&stem.to_ascii_lowercase())).map_or(&[], |v| v.as_slice())
}

/// What a button the pack added to dialog `stem` does when clicked.
pub fn ui_action(stem: &str, name: &str) -> Option<&'static str> {
    ui_added(stem).iter().find(|c| c.name.eq_ignore_ascii_case(name))?.action.as_deref()
}

/// The pack's picture for `file` in the dialog archive `stem`, and the
/// size it should cover when the pack sets one.
pub fn ui_image(stem: &str, file: &str) -> Option<(mm2_formats::tga::Image, Option<(u32, u32)>)> {
    let art = ui_art()?;
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

/// A grey detail noise's turn into the terrain's detail: the earth colour
/// it darkens towards, and how its grey becomes the blend (the pack's
/// greys, 70 to 175 about a middle of 127, to the game's 0 to 150 about 67).
const DETAIL_EARTH: [u8; 3] = [34, 17, 0];
const DETAIL_GREY: f32 = 83.0;
const DETAIL_GAIN: f32 = 1.53;

/// The pack's place for control `name` of dialog `stem`.
pub fn ui_rect(stem: &str, name: &str) -> Option<[i32; 4]> {
    ui_art()?.rects.get(&(stem.to_ascii_lowercase(), name.to_ascii_lowercase())).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_motions_cover_riding_crashes_and_tricks() {
        let rider = own_motions(true);
        for name in ["LeanR02", "Turn2D", "CrosOv2", "Endo", "Kahuna", "Fall01", "HeadHit", "SuperMan", "BarKneelAlt", "Podium3_02", "Podium4_01", "Podium5_03"] {
            assert!(rider.iter().any(|m| m.0 == name), "{name}");
        }
        assert!(rider.iter().filter(|m| m.0 != "BackUpBottom").all(|m| m.1.frames.iter().all(|f| f.len() == 16)));
        assert!(own_motions(false).iter().any(|m| m.0 == "BTurn2D"));
        let tricks = own_tricks();
        assert_eq!(tricks.len(), 32);
        assert_eq!(tricks.iter().flatten().count(), 22);
        let kneel = tricks[0].as_ref().unwrap();
        assert_eq!((kneel.fps, kneel.frames), (18.0, 30));
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
            vec![Bike { shape: "fp_bike_mx2".into(), livery: "fp_livery_blue".into(), cc: 125, four_stroke: false, name: "Blue 125".into() }]
        );
        assert_eq!(rides.outfits, vec![Outfit { shape: "fp_rider".into(), outfit: "fp_rider_red".into(), name: "Red".into() }]);
        assert_eq!(rides.shapes["fp_rider"], (std::path::PathBuf::from("/p/riders/fp_rider.glb"), true));
        assert!(rides.paints.contains_key("fp_livery_blue"));
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
