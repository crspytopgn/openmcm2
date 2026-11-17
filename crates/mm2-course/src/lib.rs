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
pub mod copy;
pub mod env;
pub mod kit;
pub mod land;
pub mod path;

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
    pub look: Look,
    /// Things the bake adds along the track by itself.
    pub dress: Dress,
    /// For a copy of a track that already exists: which one.
    pub base: Option<copy::Base>,
    /// That track, read in (not saved: read again from `base`).
    #[serde(skip)]
    pub source: Option<std::sync::Arc<copy::Source>>,
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
}

impl Default for Land {
    fn default() -> Self {
        Land {
            kind: LandKind::Rolling,
            seed: 1,
            hills: 0.5,
            rough: 0.3,
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
}

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
}

impl Default for Look {
    fn default() -> Self {
        Look {
            sky: "Terra05.cub".into(),
            sun: 0.7,
            sun_turn: 30.0,
            stadium: "B1B_02.seg".into(),
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
            },
            jumps: Vec::new(),
            strokes: Vec::new(),
            paint: Vec::new(),
            objects: Vec::new(),
            look: Look {
                sky: if mode == Mode::Baja {
                    "Terra12.cub".into()
                } else {
                    Look::default().sky
                },
                ..Look::default()
            },
            dress,
            base: None,
            source: None,
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
