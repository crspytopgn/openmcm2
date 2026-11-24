//! The Course Builder's tracks.
//!
//! A [`Course`] is what a player makes in the builder: a mode, a land, the
//! racing line drawn over it, the jumps dropped on that line, brush strokes,
//! paint and objects. It is kept as a small text file (`<name>.course`,
//! JSON) and baked into an ordinary `.ENV` track ([`env::write`]) that the
//! game loads like any other, so builder tracks race, show in the track
//! list and travel in network races as community tracks do.
//!
//! The track is the line: from it the bake carves the track bed and its
//! shoulders, banks the corners, lays the course the computer riders follow
//! (the `.tdf`), the start and finish lines, and for Baja and Enduro the
//! checkpoint gates (the `.wpt`).

pub mod auto;
pub mod bake;
pub mod berm;
pub mod copy;
pub mod rebuild;
pub mod ridecheck;
pub mod env;
pub mod flight;
pub mod free;
pub mod free_tune;
pub mod kit;
pub mod land;
pub mod nature;
pub mod path;
pub mod picture;
pub mod preview;

pub use bake::Field;

use serde::{Deserialize, Serialize};

/// The file's layout version.
pub const VERSION: u32 = 1;

/// A player-made track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Course {
    pub version: u32,
    /// Shown in the track list.
    pub name: String,
    pub author: String,
    pub mode: Mode,
    pub land: Land,
    pub line: Line,
    pub jumps: Vec<Jump>,
    /// Shaping brush strokes, applied in order after the track is carved.
    pub strokes: Vec<Stroke>,
    /// Surface paint, applied in order over the land's and the track's own.
    pub paint: Vec<Paint>,
    pub objects: Vec<Object>,
    /// Plant brush strokes: trees, bushes and grass, in order.
    pub plants: Vec<Planting>,
    /// Rivers and streams.
    pub rivers: Vec<River>,
    /// Planes, helicopters and the blimp, each flying its own loop.
    pub flights: Vec<Flight>,
    pub look: Look,
    /// Models of the player's own, packed into the track.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<UserModel>,
    /// Things the bake adds along the track by itself.
    pub dress: Dress,
    /// For a copy of a track that already exists: which one.
    pub base: Option<copy::Base>,
    /// That track, read in (not saved: read again from `base`).
    #[serde(skip)]
    pub source: Option<std::sync::Arc<copy::Source>>,
    /// Bare land to lay the track on instead of the land recipe's: heights
    /// (ft) at each of the mode's samples, rows along +z (a rebuilt track,
    /// [`rebuild`]). Not saved.
    #[serde(skip)]
    pub ground: Option<std::sync::Arc<Vec<f32>>>,
}

impl Default for Course {
    fn default() -> Self {
        Course::new(Mode::National)
    }
}

/// The four kinds of race a track can be for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Mode {
    Supercross,
    #[default]
    National,
    Baja,
    Enduro,
}

impl Mode {
    pub const ALL: [Mode; 4] = [Mode::Supercross, Mode::National, Mode::Baja, Mode::Enduro];

    pub fn name(self) -> &'static str {
        match self {
            Mode::Supercross => "Supercross",
            Mode::National => "National",
            Mode::Baja => "Baja",
            Mode::Enduro => "Enduro",
        }
    }

    /// The game's folder under `TERAFORM` for this kind of track.
    pub fn folder(self) -> &'static str {
        match self {
            Mode::Supercross => "SX",
            Mode::National => "NATIONAL",
            Mode::Baja => "BAJA",
            Mode::Enduro => "ENDURO",
        }
    }

    /// The scene's `EventType`.
    pub fn event_type(self) -> &'static str {
        self.name()
    }

    /// `TerrainWidth`: the terrain is `256 × n + 1` samples a side, as on
    /// the shipped tracks (SX 3, National and Enduro 5, Baja 6).
    pub fn terrain_width(self) -> usize {
        match self {
            Mode::Supercross => 3,
            Mode::National | Mode::Enduro => 5,
            Mode::Baja => 6,
        }
    }

    /// Feet per terrain sample (`TerrainWidthScale`).
    pub fn scale(self) -> f32 {
        match self {
            Mode::Baja => 4.5,
            _ => 3.0,
        }
    }

    pub fn samples(self) -> usize {
        self.terrain_width() * 256 + 1
    }

    /// The land's side, ft.
    pub fn side(self) -> f32 {
        (self.samples() - 1) as f32 * self.scale()
    }

    /// A lap race (round and round) or a gate race (checkpoint to checkpoint).
    pub fn laps(self) -> bool {
        matches!(self, Mode::Supercross | Mode::National)
    }

    /// The track's width when a point is first laid, ft.
    pub fn track_width(self) -> f32 {
        match self {
            Mode::Supercross => 26.0,
            Mode::National => 44.0,
            Mode::Baja => 40.0,
            Mode::Enduro => 30.0,
        }
    }

    /// How far beyond the track's edge it blends back into the land, ft.
    pub fn shoulder(self) -> f32 {
        match self {
            Mode::Supercross => 7.0,
            Mode::National => 34.0,
            Mode::Baja => 40.0,
            Mode::Enduro => 26.0,
        }
    }
}

/// The ground the track is laid on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Land {
    pub kind: LandKind,
    pub seed: u32,
    /// How tall the hills are, 0 to 1.
    pub hills: f32,
    /// How bumpy the ground is between them, 0 to 1.
    pub rough: f32,
    /// A height map picture to raise the land from.
    pub heightmap: Option<picture::HeightMap>,
}

impl Default for Land {
    fn default() -> Self {
        Land {
            kind: LandKind::Rolling,
            seed: 1,
            hills: 0.5,
            rough: 0.3,
            heightmap: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LandKind {
    /// Green rolling hills.
    #[default]
    Rolling,
    /// A wide valley between two ridges.
    Valley,
    /// Dry, flat-ish desert with dunes and washes.
    Desert,
    /// Steep mountains with a high meadow.
    Mountains,
    /// Flat ground (a stadium floor).
    Flat,
}

impl LandKind {
    pub const ALL: [LandKind; 5] = [
        LandKind::Rolling,
        LandKind::Valley,
        LandKind::Desert,
        LandKind::Mountains,
        LandKind::Flat,
    ];

    pub fn name(self) -> &'static str {
        match self {
            LandKind::Rolling => "Rolling hills",
            LandKind::Valley => "Valley",
            LandKind::Desert => "Desert",
            LandKind::Mountains => "Mountains",
            LandKind::Flat => "Flat",
        }
    }

    /// The surface off the track.
    pub fn ground(self) -> Surface {
        match self {
            LandKind::Desert => Surface::Sand,
            LandKind::Flat => Surface::Dirt,
            _ => Surface::Grass,
        }
    }
}

/// The racing line: points the player lays, joined by a smooth curve.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Line {
    pub points: Vec<Point>,
    /// The line comes back to its first point (a lap).
    pub closed: bool,
    /// How much the corners bank up on the outside, 0 to 1.
    pub berms: f32,
    /// Where the start is, ft along the line.
    pub start: f32,
    /// Gates placed by hand (Baja and Enduro), ft along the line; none
    /// for gates every few hundred feet.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub gates: Vec<f32>,
    /// A start off the line (Baja and Enduro): where the riders line up
    /// in the open, x and z ft and the way they face (degrees from +z
    /// towards +x), to funnel into the track.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_spot: Option<[f32; 3]>,
    /// The AI line, where it leaves the track's middle: handles of ft
    /// along the line and ft across it (to the side `Path::side` points,
    /// the course's first edge), eased between. None: the middle.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ai: Vec<[f32; 2]>,
}

impl Line {
    /// How far across the track the AI line runs `s` ft along a line
    /// `length` ft long, before it is kept inside the track's edges.
    pub fn ai_at(&self, s: f32, length: f32) -> f32 {
        let mut keys: Vec<[f32; 2]> = self.ai.clone();
        if keys.is_empty() {
            return 0.0;
        }
        keys.sort_by(|a, b| a[0].total_cmp(&b[0]));
        let ease = |a: [f32; 2], b: [f32; 2], s: f32| {
            let span = b[0] - a[0];
            let k = if span > 1e-3 { ((s - a[0]) / span).clamp(0.0, 1.0) } else { 0.0 };
            let k = k * k * (3.0 - 2.0 * k);
            a[1] + (b[1] - a[1]) * k
        };
        let (first, last) = (keys[0], keys[keys.len() - 1]);
        if self.closed && length > 0.0 {
            // Round the lap: past the last handle it eases back to the first.
            let s = s.rem_euclid(length);
            if s < first[0] {
                return ease([last[0] - length, last[1]], first, s);
            }
            if s >= last[0] {
                return ease(last, [first[0] + length, first[1]], s);
            }
        } else if s <= first[0] {
            return first[1];
        } else if s >= last[0] {
            return last[1];
        }
        let i = keys.partition_point(|k| k[0] <= s).max(1);
        ease(keys[i - 1], keys[i], s)
    }
}

/// How close to an edge the AI line may run, ft.
pub const AI_EDGE: f32 = 3.0;

/// One point the line passes through.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub z: f32,
    /// The track's width here, ft.
    pub width: f32,
}

/// A jump or bump set on the line.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Jump {
    pub kind: kit::Kind,
    /// Where it begins, ft along the line.
    pub at: f32,
    /// How long it is along the line, ft.
    pub length: f32,
    /// How tall, ft.
    pub height: f32,
}

/// A shaping brush.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Brush {
    Raise,
    Lower,
    Smooth,
    /// Levels to the height where the stroke began.
    Flatten,
    Roughen,
}

impl Brush {
    pub const ALL: [Brush; 5] = [
        Brush::Raise,
        Brush::Lower,
        Brush::Smooth,
        Brush::Flatten,
        Brush::Roughen,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Brush::Raise => "Raise",
            Brush::Lower => "Lower",
            Brush::Smooth => "Smooth",
            Brush::Flatten => "Flatten",
            Brush::Roughen => "Roughen",
        }
    }
}

/// One drag of a shaping brush.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub brush: Brush,
    /// ft.
    pub radius: f32,
    /// 0 to 1.
    pub strength: f32,
    /// The height Flatten levels to, ft.
    #[serde(default)]
    pub target: f32,
    /// Where the brush touched, in order.
    pub dabs: Vec<[f32; 2]>,
}

/// What the ground is made of: each is one of the scene's terrain zones,
/// with its own grip and dust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Surface {
    Dirt,
    Grass,
    Sand,
    Mud,
    Rock,
}

impl Surface {
    pub const ALL: [Surface; 5] = [
        Surface::Dirt,
        Surface::Grass,
        Surface::Sand,
        Surface::Mud,
        Surface::Rock,
    ];

    /// The terrain zone, from 0 (`[TerrainZone1]`).
    pub fn zone(self) -> u8 {
        self as u8
    }

    pub fn from_zone(z: u8) -> Surface {
        Surface::ALL
            .get(z as usize)
            .copied()
            .unwrap_or(Surface::Dirt)
    }

    pub fn name(self) -> &'static str {
        match self {
            Surface::Dirt => "Dirt",
            Surface::Grass => "Grass",
            Surface::Sand => "Sand",
            Surface::Mud => "Mud",
            Surface::Rock => "Hardpack",
        }
    }
}

/// One drag of the paint brush.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Paint {
    pub surface: Surface,
    pub radius: f32,
    pub dabs: Vec<[f32; 2]>,
}

/// A model of the player's own (the original builder's user models): an
/// `.slt` and the `.tga` pictures it names, packed into the track, so it
/// shows in the original game too. Placed as an [`Object`] by its name.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UserModel {
    pub slt: picture::Embedded,
    pub textures: Vec<picture::Embedded>,
}

impl UserModel {
    /// The name objects place it by.
    pub fn name(&self) -> &str {
        &self.slt.name
    }

    /// Its files, as packed into the track.
    pub fn files(&self) -> Vec<(String, Vec<u8>)> {
        std::iter::once(&self.slt).chain(&self.textures).map(|f| (f.name.clone(), f.bytes())).collect()
    }
}

/// A stock object placed by name (`MODELS.RES` or the free pack's stand-in).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Object {
    /// The `.slt` name, such as `TripleBale03.slt`.
    pub model: String,
    pub x: f32,
    pub z: f32,
    /// Which way it faces, degrees from +z towards +x.
    pub yaw: f32,
    #[serde(default = "one")]
    pub scale: f32,
}

fn one() -> f32 {
    1.0
}

/// What a plant brush plants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Flora {
    Trees,
    Bushes,
    Grass,
    /// Trees with bushes and grass under them.
    Mixed,
}

impl Flora {
    pub const ALL: [Flora; 4] = [Flora::Mixed, Flora::Trees, Flora::Bushes, Flora::Grass];

    pub fn name(self) -> &'static str {
        match self {
            Flora::Trees => "Trees",
            Flora::Bushes => "Bushes",
            Flora::Grass => "Grass",
            Flora::Mixed => "Wild",
        }
    }
}

/// One drag of the plant brush. Density 0 to 1; `clear` takes plants
/// away instead. With `everywhere`, it covers the whole land.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Planting {
    pub flora: Flora,
    pub radius: f32,
    pub density: f32,
    #[serde(default)]
    pub clear: bool,
    #[serde(default)]
    pub everywhere: bool,
    pub dabs: Vec<[f32; 2]>,
    /// Grown by "Dress the scenery" round the line ([`nature::scenery`]),
    /// so the next press can grow it again round the line as it is then.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub auto: bool,
    /// A plant map: a picture over the whole land, white where these grow
    /// thickest and black where none do (the original builder's
    /// placement maps), its top the far side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map: Option<picture::Shared>,
}

/// A river: its middle line, how wide and deep it is, ft. It runs
/// downhill from its first point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct River {
    pub points: Vec<[f32; 2]>,
    pub width: f32,
    pub depth: f32,
}

/// What flies a flight path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Craft {
    Biplane,
    CargoPlane,
    Helicopter,
    Blimp,
}

impl Craft {
    pub const ALL: [Craft; 4] = [Craft::Biplane, Craft::Helicopter, Craft::CargoPlane, Craft::Blimp];

    pub fn name(self) -> &'static str {
        match self {
            Craft::Biplane => "Biplane",
            Craft::CargoPlane => "Cargo plane",
            Craft::Helicopter => "Helicopter",
            Craft::Blimp => "Blimp",
        }
    }

    /// Its model, as `MODELS.RES` names it.
    pub fn model(self) -> &'static str {
        match self {
            Craft::Biplane => "Biplane.slt",
            Craft::CargoPlane => "CargoPlane.slt",
            Craft::Helicopter => "HeliNoCam.slt",
            Craft::Blimp => "Blimp.slt",
        }
    }

    /// A good speed to fly at, ft/s.
    pub fn speed(self) -> f32 {
        match self {
            Craft::Biplane => 110.0,
            Craft::CargoPlane => 160.0,
            Craft::Helicopter => 70.0,
            Craft::Blimp => 25.0,
        }
    }

    /// A good height over the ground, ft.
    pub fn height(self) -> f32 {
        match self {
            Craft::Biplane => 160.0,
            Craft::CargoPlane => 320.0,
            Craft::Helicopter => 110.0,
            Craft::Blimp => 260.0,
        }
    }
}

/// A craft flying round a loop through its points: x, height over the
/// ground, z (ft). Speed in ft/s.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Flight {
    pub craft: Craft,
    pub points: Vec<[f32; 3]>,
    pub speed: f32,
}

/// The sky and light.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Look {
    /// The sky box in `SKIES.RES`.
    pub sky: String,
    /// The sun's height, 0 (low, evening) to 1 (midday).
    pub sun: f32,
    /// The sun's direction round the sky, degrees.
    pub sun_turn: f32,
    /// Supercross: the stadium, by its `COLSEG.RES` name.
    pub stadium: String,
    /// How bright the shade is (the ambient light), 0 (dark shadows) to 1
    /// (soft, bright shade).
    pub ambient: f32,
    /// The haze in the distance: one of [`FOGS`] by name, or empty for the
    /// land's own.
    pub fog: String,
    /// A picture of the player's own laid over the whole land.
    pub ground: Option<GroundPicture>,
}

/// A ground picture: the whole land seen from above, its top the map's
/// far (north) side, as the original builder's 960 x 960 ground textures.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GroundPicture {
    pub picture: picture::Shared,
    /// Draws the track's own dirt over the picture.
    pub track_on_top: bool,
}

/// The haze colours a track can have, by name; the original game lets a
/// scene change only the colour of its fog.
pub const FOGS: [(&str, [u8; 3]); 7] = [
    ("Clear blue", [176, 182, 190]),
    ("Morning mist", [211, 210, 210]),
    ("Desert dust", [196, 186, 170]),
    ("Warm evening", [214, 178, 140]),
    ("Grey storm", [128, 132, 138]),
    ("Night", [40, 44, 60]),
    ("Arena", [125, 125, 183]),
];

impl Default for Look {
    fn default() -> Self {
        Look {
            sky: "Terra05.cub".into(),
            sun: 0.7,
            sun_turn: 30.0,
            stadium: "B1B_02.seg".into(),
            ambient: 0.5,
            fog: String::new(),
            ground: None,
        }
    }
}

/// The stadiums a Supercross track can be in, as the shipped ones use them.
pub const STADIUMS: [(&str, &str); 4] = [
    ("B1B_02.seg", "Big Dome"),
    ("Euro.seg", "Euro Arena"),
    ("Football.seg", "Football Stadium"),
    ("AsDome03.seg", "Astro Dome"),
];

/// The skies in `SKIES.RES`.
pub const SKIES: [(&str, &str); 13] = [
    ("Terra05.cub", "Clear"),
    ("Terra01.cub", "Blue, high cloud"),
    ("Terra03.cub", "Hazy"),
    ("Terra04.cub", "Broken cloud"),
    ("Terra06.cub", "Overcast"),
    ("Terra09.cub", "Late afternoon"),
    ("Terra11.cub", "Mountain"),
    ("Terra12.cub", "Desert"),
    ("Terra15.cub", "Grey"),
    ("Terra16.cub", "Summer"),
    ("Terra17.cub", "Dusk"),
    ("Terra18.cub", "Stormy"),
    ("Burn03.cub", "Smoke"),
];

/// What the bake lays along the track by itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Dress {
    /// Hay bales along both edges (Supercross tuff-block style).
    pub edge_bales: bool,
    /// Trees and bushes off the track, 0 to 1.
    pub trees: f32,
    /// Banners and tents near the start.
    pub start_area: bool,
}

impl Default for Dress {
    fn default() -> Self {
        Dress {
            edge_bales: false,
            trees: 0.4,
            start_area: true,
        }
    }
}

impl Course {
    /// A new track of this mode: a ready loop (or route) with a few jumps,
    /// so there is something to ride before anything is changed.
    pub fn new(mode: Mode) -> Course {
        let side = mode.side();
        let c = side / 2.0;
        let width = mode.track_width();
        let (rx, rz, n) = match mode {
            Mode::Supercross => (300.0, 190.0, 10),
            Mode::National => (950.0, 650.0, 12),
            Mode::Baja => (2300.0, 1900.0, 14),
            Mode::Enduro => (1200.0, 900.0, 12),
        };
        let points: Vec<Point> = if mode == Mode::Supercross {
            // Back and forth across the stadium floor, as the shipped
            // Supercross tracks run.
            [
                (-270.0, -230.0),
                (-270.0, 240.0),
                (-185.0, 300.0),
                (-100.0, 240.0),
                (-100.0, -110.0),
                (-10.0, -170.0),
                (80.0, -110.0),
                (80.0, 240.0),
                (175.0, 300.0),
                (270.0, 240.0),
                (270.0, -230.0),
                (200.0, -300.0),
                (0.0, -310.0),
                (-200.0, -300.0),
            ]
            .iter()
            .map(|&(x, z)| Point {
                x: c + x,
                z: c + z,
                width,
            })
            .collect()
        } else {
            (0..n)
                .map(|i| {
                    let a = i as f32 / n as f32 * std::f32::consts::TAU;
                    let wob = 1.0 + 0.16 * (3.0 * a + 0.6).sin();
                    Point {
                        x: c + rx * a.cos() * wob,
                        z: c + rz * a.sin() * wob,
                        width,
                    }
                })
                .collect()
        };
        let (land, dress) = match mode {
            Mode::Supercross => (
                Land {
                    kind: LandKind::Flat,
                    seed: 1,
                    hills: 0.0,
                    rough: 0.0,
                    heightmap: None,
                },
                Dress {
                    edge_bales: true,
                    trees: 0.0,
                    start_area: false,
                },
            ),
            Mode::Baja => (
                Land {
                    kind: LandKind::Desert,
                    seed: 1,
                    hills: 0.5,
                    rough: 0.4,
                    heightmap: None,
                },
                Dress {
                    trees: 0.15,
                    ..Dress::default()
                },
            ),
            Mode::Enduro => (
                Land {
                    kind: LandKind::Mountains,
                    seed: 1,
                    hills: 0.45,
                    rough: 0.35,
                    heightmap: None,
                },
                Dress::default(),
            ),
            Mode::National => (Land::default(), Dress::default()),
        };
        let mut course = Course {
            version: VERSION,
            name: format!("My {} Track", mode.name()),
            author: String::new(),
            mode,
            land,
            line: Line {
                points,
                closed: true,
                berms: 0.5,
                start: 0.0,
                ..Line::default()
            },
            jumps: Vec::new(),
            strokes: Vec::new(),
            paint: Vec::new(),
            objects: Vec::new(),
            plants: Vec::new(),
            rivers: Vec::new(),
            flights: Vec::new(),
            look: Look {
                sky: if mode == Mode::Baja {
                    "Terra12.cub".into()
                } else {
                    Look::default().sky
                },
                ..Look::default()
            },
            models: Vec::new(),
            dress,
            base: None,
            source: None,
            ground: None,
        };
        // A few jumps spread round the loop, clear of the start.
        let length = course.path().length;
        let kit: &[(kit::Kind, f32)] = match mode {
            Mode::Supercross => &[
                (kit::Kind::Triple, 0.1),
                (kit::Kind::Rhythm, 0.27),
                (kit::Kind::Whoops, 0.47),
                (kit::Kind::Double, 0.62),
                (kit::Kind::Tabletop, 0.8),
            ],
            Mode::National => &[
                (kit::Kind::Tabletop, 0.15),
                (kit::Kind::Double, 0.38),
                (kit::Kind::Roller, 0.6),
                (kit::Kind::StepUp, 0.8),
            ],
            _ => &[
                (kit::Kind::Kicker, 0.25),
                (kit::Kind::Roller, 0.55),
                (kit::Kind::Whoops, 0.8),
            ],
        };
        for &(kind, at) in kit {
            course.jumps.push(kind.make(mode, at * length));
        }
        course
    }

    /// The smooth line through the points, sampled every couple of feet.
    /// Makes this a track of another kind of race, keeping its shape:
    /// the line, jumps, brush work and objects moved and sized to fit the
    /// new land (a stadium's floor for Supercross). Not for a copy of a
    /// track, whose land is that track's.
    pub fn change_mode(&mut self, mode: Mode) -> bool {
        if mode == self.mode || self.base.is_some() {
            return false;
        }
        let fresh = Course::new(mode);
        let bounds = |pts: &[Point]| {
            pts.iter().fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), p| ([lo[0].min(p.x), lo[1].min(p.z)], [hi[0].max(p.x), hi[1].max(p.z)]))
        };
        let (lo, hi) = bounds(&self.line.points);
        let (mid, half) = if lo[0] <= hi[0] {
            ([(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0], [((hi[0] - lo[0]) / 2.0).max(1.0), ((hi[1] - lo[1]) / 2.0).max(1.0)])
        } else {
            ([self.mode.side() / 2.0; 2], [1.0; 2])
        };
        let c = mode.side() / 2.0;
        let k = if mode == Mode::Supercross {
            // Inside the stadium's floor, as the new track's own line is.
            let (flo, fhi) = bounds(&fresh.line.points);
            (((fhi[0] - flo[0]) / 2.0 + 20.0) / half[0]).min(((fhi[1] - flo[1]) / 2.0 + 20.0) / half[1])
        } else {
            // As big as it was on its old land, kept well inside the new.
            let room = mode.side() * 0.42;
            (mode.side() / self.mode.side()).min(room / half[0]).min(room / half[1])
        };
        let at = |x: f32, z: f32| [c + (x - mid[0]) * k, c + (z - mid[1]) * k];
        let width = fresh.mode.track_width() / self.mode.track_width();
        for p in &mut self.line.points {
            [p.x, p.z] = at(p.x, p.z);
            p.width = (p.width * width).clamp(10.0, 120.0);
        }
        self.line.start *= k;
        for j in &mut self.jumps {
            j.at *= k;
        }
        for d in self.strokes.iter_mut().flat_map(|s| s.dabs.iter_mut()).chain(self.paint.iter_mut().flat_map(|p| p.dabs.iter_mut())) {
            *d = at(d[0], d[1]);
        }
        for o in &mut self.objects {
            [o.x, o.z] = at(o.x, o.z);
        }
        for d in self.plants.iter_mut().flat_map(|p| p.dabs.iter_mut()).chain(self.rivers.iter_mut().flat_map(|r| r.points.iter_mut())) {
            *d = at(d[0], d[1]);
        }
        for p in self.flights.iter_mut().flat_map(|f| f.points.iter_mut()) {
            [p[0], p[2]] = at(p[0], p[2]);
        }
        if mode.laps() {
            self.line.closed = true;
        }
        // A stadium's flat floor, or open land again after one.
        if mode == Mode::Supercross || self.mode == Mode::Supercross {
            self.land = fresh.land.clone();
        }
        self.dress = fresh.dress.clone();
        self.look.stadium = fresh.look.stadium.clone();
        if self.mode == Mode::Supercross || mode == Mode::Baja {
            self.look.sky = fresh.look.sky.clone();
        }
        let old = format!("My {} ", self.mode.name());
        if let Some(rest) = self.name.strip_prefix(&old) {
            self.name = format!("My {} {rest}", mode.name());
        }
        self.mode = mode;
        // Dressed scenery grows again for the new land (none in a stadium).
        if self.plants.iter().any(|p| p.auto) {
            nature::redress(self);
        }
        true
    }

    pub fn path(&self) -> path::Path {
        path::Path::new(&self.line.points, self.line.closed)
    }

    /// Reads a `.course` file's text.
    pub fn from_json(text: &str) -> Result<Course, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// The name as a file stem: letters and digits, at most 8 (the game's
    /// own track names are short and the menus have little room).
    pub fn stem(&self) -> String {
        let s: String = self
            .name
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .take(8)
            .collect();
        if s.is_empty() {
            "MYTRACK".into()
        } else {
            s.to_ascii_uppercase()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn course_file_round_trip() {
        for mode in Mode::ALL {
            let c = Course::new(mode);
            let back = Course::from_json(&c.to_json()).unwrap();
            assert_eq!(c, back);
            assert!(!c.jumps.is_empty());
        }
    }

    #[test]
    fn a_track_changes_kind_and_keeps_its_shape() {
        for from in Mode::ALL {
            for to in Mode::ALL {
                let mut c = crate::auto::surprise(from, 3);
                let (points, jumps) = (c.line.points.len(), c.jumps.len());
                c.change_mode(to);
                assert_eq!(c.mode, to);
                assert_eq!((c.line.points.len(), c.jumps.len()), (points, jumps));
                let side = to.side();
                assert!(c.line.points.iter().all(|p| p.x > 0.0 && p.z > 0.0 && p.x < side && p.z < side), "{from:?} to {to:?}");
                let baked = crate::env::write(&c);
                assert!(!baked.env.is_empty());
            }
        }
    }

    #[test]
    fn old_files_fill_in_defaults() {
        let c = Course::from_json(r#"{"name":"Old","mode":"Baja"}"#).unwrap();
        assert_eq!(c.mode, Mode::Baja);
        assert_eq!(c.name, "Old");
    }

    #[test]
    fn sizes_match_the_shipped_tracks() {
        assert_eq!(Mode::Supercross.samples(), 769);
        assert_eq!(Mode::National.samples(), 1281);
        assert_eq!(Mode::Baja.samples(), 1537);
        assert_eq!(Mode::Baja.side(), 6912.0);
    }
}
