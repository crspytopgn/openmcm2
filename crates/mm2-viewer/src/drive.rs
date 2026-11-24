//! Drive the scenery (an extra, `--drive-vehicles`, off by default): in
//! Practice and free roam, V by one of the track's cars, trucks or trains
//! gets the player in to drive it, and V near nothing takes them up into
//! the nearest plane, helicopter or blimp in the sky to fly it. V again
//! gets out, back on the bike beside it. None of it is the game's: the
//! handling is simple and arcade-like, made to be fun, and the bike's own
//! riding is untouched.

use crate::scenery::Park;
use glam::{Mat4, Vec3};
use mm2_formats::trn::Terrain;
use mm2_physics::bike::Controls;
use mm2_physics::collision::World;

use crate::traffic::{self, Traffic};

/// Gravity, ft/s² (the game's).
const GRAVITY: f32 = 32.2;
/// How near a car's middle the bike must be to get in, ft, past its own
/// length.
const REACH: f32 = 14.0;

/// How a vehicle goes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// On its wheels, on the ground.
    Car,
    /// A train's car: on the ground too, but long and slow to turn.
    Rail,
    /// Wings: it needs speed to stay up, and banks to turn.
    Plane,
    /// Rotors or gas: it hangs in the air, climbs and sinks at will.
    Hover,
}

/// What the player is in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seat {
    /// A scripted vehicle (its index in the track's traffic).
    Car(usize),
    /// A flying animated object (its index among the animated objects).
    Air(usize),
}

/// A craft in the track's sky that can be flown.
#[derive(Debug, Clone)]
pub struct Craft {
    pub object: usize,
    pub kind: Kind,
    pub name: String,
}

/// Which of the track's animated objects fly, by their model.
pub fn aircraft<'a>(models: impl Iterator<Item = &'a str>) -> Vec<Craft> {
    models
        .enumerate()
        .filter_map(|(object, model)| {
            let kind = air_kind(model)?;
            Some(Craft { object, kind, name: nice_name(model) })
        })
        .collect()
}

fn air_kind(model: &str) -> Option<Kind> {
    let m = model.to_ascii_lowercase();
    const PLANES: [&str; 8] = ["supercub", "cargoplane", "biplane", "beech18", "airplane", "stearman", "dc3", "dc-3"];
    const HOVER: [&str; 5] = ["bellranger", "heli", "blimp", "ufo", "zeppelin"];
    if PLANES.iter().any(|p| m.starts_with(p)) {
        Some(Kind::Plane)
    } else if HOVER.iter().any(|p| m.starts_with(p)) {
        Some(Kind::Hover)
    } else {
        None
    }
}

/// How a scripted vehicle goes, by its model.
fn ground_kind(model: &str) -> Kind {
    let m = model.to_ascii_lowercase();
    const RAIL: [&str; 7] = ["engine", "boxcar", "cattlecar", "flatbed", "caboose", "coalcar", "gondola"];
    if RAIL.iter().any(|p| m.starts_with(p)) {
        Kind::Rail
    } else {
        Kind::Car
    }
}

/// A big vehicle: a bus, truck or train, which is longer, slower and
/// gets a camera further back.
fn big(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    const BIG: [&str; 16] = [
        "greyhound", "schoolbus", "fedup", "firetruck", "watertruck", "semi", "dumptruck", "scraper", "companytruck", "cargoplane", "blimp", "famtrk", "ufo", "combine", "dc-3", "beech",
    ];
    BIG.iter().any(|p| m.starts_with(p)) || ground_kind(model) == Kind::Rail
}

/// "FireTruck.Slt" as "Fire Truck", "4Runner_Blue.slt" as "4Runner".
pub fn nice_name(model: &str) -> String {
    let stem = model.rsplit_once('.').map_or(model, |(s, _)| s);
    let stem = stem.split('_').next().unwrap_or(stem);
    let known = [
        ("fedup", "Delivery Truck"),
        ("sts", "Sedan"),
        ("mx3", "Coupe"),
        ("300zx", "Sports Car"),
        ("cop", "Police Car"),
        ("greyhound", "Coach"),
        ("famtrkstr", "Family Truckster"),
        ("helinocam", "Helicopter"),
        ("bellranger", "Helicopter"),
        ("supercub", "Super Cub"),
        ("beech18", "Beech 18"),
        ("engine", "Locomotive"),
        ("semicab", "Semi Truck"),
        ("ufo", "UFO"),
    ];
    let low = stem.to_ascii_lowercase();
    if let Some((_, n)) = known.iter().find(|(k, _)| low.starts_with(k)) {
        return n.to_string();
    }
    let mut out = String::new();
    for (i, ch) in stem.chars().enumerate() {
        if i > 0 && ch.is_ascii_uppercase() && !out.ends_with(' ') && out.chars().last().is_some_and(|c| c.is_ascii_lowercase()) {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}

/// Which wheels the engine turns.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Wheels {
    Front,
    Rear,
    All,
}

/// What a road vehicle is like: our picks from what each one really is
/// (a CJ Jeep on soft long springs, a stiff 300ZX, a bus that leans), in
/// feet and seconds; accelerations stand for forces, per unit of weight.
#[derive(Debug, Clone, Copy)]
struct CarSpec {
    /// Half the wheelbase and half the track.
    half_base: f32,
    half_track: f32,
    /// The weight's height over the ground at rest.
    cg: f32,
    /// The wheels' radius: where the springs hold the body.
    wheel: f32,
    /// How far the springs give before the bump stops.
    travel: f32,
    /// The springs' bounce, Hz, and how damped it is (1 none past rest).
    hz: f32,
    damping: f32,
    /// Pull from a standstill, ft/s², and top speed, ft/s.
    accel: f32,
    top: f32,
    /// Tyre grip (1 a road tyre on dirt-packed road), brakes, ft/s², and
    /// the front wheels' lock, rad.
    grip: f32,
    brake: f32,
    lock: f32,
    driven: Wheels,
}

const fn spec(half_base: f32, half_track: f32, cg: f32, wheel: f32, travel: f32, hz: f32, damping: f32, accel: f32, top: f32, grip: f32, brake: f32, lock: f32, driven: Wheels) -> CarSpec {
    CarSpec { half_base, half_track, cg, wheel, travel, hz, damping, accel, top, grip, brake, lock, driven }
}

/// Each road vehicle's make-up, by its model's name.
fn car_spec(model: &str) -> CarSpec {
    use Wheels::*;
    let m = model.to_ascii_lowercase();
    let is = |p: &str| m.starts_with(p);
    // half base, half track, cg, wheel, travel, Hz, damping, pull, top, grip, brakes, lock, driven
    if is("jeep") {
        spec(3.9, 2.5, 2.3, 1.25, 0.9, 1.2, 0.3, 16.0, 125.0, 1.0, 26.0, 0.6, All)
    } else if is("4runner") {
        spec(4.4, 2.6, 2.4, 1.3, 0.8, 1.3, 0.35, 17.0, 150.0, 1.0, 27.0, 0.55, All)
    } else if is("cop") {
        spec(4.7, 2.7, 1.9, 1.1, 0.55, 1.6, 0.45, 22.0, 190.0, 1.05, 30.0, 0.55, Rear)
    } else if is("chevelle") {
        spec(4.6, 2.7, 1.8, 1.15, 0.6, 1.3, 0.28, 27.0, 185.0, 0.95, 26.0, 0.5, Rear)
    } else if is("sts") {
        spec(4.8, 2.7, 1.8, 1.1, 0.6, 1.25, 0.35, 19.0, 175.0, 1.0, 28.0, 0.5, Front)
    } else if is("300zx") {
        spec(4.1, 2.6, 1.5, 1.1, 0.4, 2.1, 0.5, 26.0, 225.0, 1.15, 32.0, 0.55, Rear)
    } else if is("mx3") {
        spec(4.0, 2.5, 1.6, 1.05, 0.45, 1.8, 0.45, 19.0, 180.0, 1.1, 30.0, 0.55, Front)
    } else if is("famtrk") {
        spec(4.8, 2.8, 1.9, 1.1, 0.7, 1.05, 0.22, 15.0, 150.0, 0.95, 24.0, 0.5, Rear)
    } else if is("greyhound") {
        spec(12.5, 3.8, 4.2, 1.7, 0.6, 1.1, 0.4, 6.5, 110.0, 0.85, 18.0, 0.5, Rear)
    } else if is("schoolbus") {
        spec(10.5, 3.6, 4.0, 1.6, 0.6, 1.2, 0.35, 6.0, 95.0, 0.85, 18.0, 0.5, Rear)
    } else if is("fedup") {
        spec(6.5, 3.3, 3.4, 1.5, 0.6, 1.4, 0.4, 9.0, 110.0, 0.9, 20.0, 0.5, Rear)
    } else if is("firetruck") {
        spec(8.5, 3.6, 3.8, 1.7, 0.6, 1.4, 0.4, 9.0, 105.0, 0.9, 20.0, 0.48, Rear)
    } else if is("semicab") {
        spec(7.0, 3.6, 3.6, 1.7, 0.5, 1.5, 0.45, 8.5, 115.0, 0.9, 18.0, 0.5, Rear)
    } else if is("dumptruck") || is("scraper") {
        spec(14.0, 7.5, 5.0, 2.6, 0.7, 1.3, 0.45, 6.0, 70.0, 0.95, 16.0, 0.45, Rear)
    } else if is("watertruck") || is("companytruck") {
        spec(7.5, 3.6, 3.6, 1.7, 0.6, 1.4, 0.4, 7.5, 95.0, 0.9, 18.0, 0.5, Rear)
    } else if ground_kind(model) == Kind::Rail {
        // Only its size counts: a train keeps to its rails.
        spec(26.0, 5.0, 6.0, 1.5, 0.3, 2.0, 0.5, 4.0, 100.0, 1.0, 10.0, 0.0, All)
    } else if is("airfield_tractor") {
        // An airport tug: tiny, low and slow.
        spec(2.6, 2.2, 1.6, 0.9, 0.2, 2.5, 0.3, 9.0, 40.0, 1.0, 18.0, 0.6, Rear)
    } else if is("combine") {
        spec(9.0, 5.5, 5.0, 2.5, 0.3, 1.6, 0.35, 4.5, 30.0, 1.0, 12.0, 0.45, Front)
    } else if is("tractor") {
        spec(3.6, 2.9, 2.6, 2.0, 0.25, 2.5, 0.25, 8.0, 36.0, 1.1, 18.0, 0.7, Rear)
    } else {
        spec(4.5, 2.7, 2.0, 1.2, 0.6, 1.4, 0.38, 18.0, 160.0, 1.0, 26.0, 0.55, Rear)
    }
}

/// What a craft is like: stall and top speed, ft/s, how quick it rolls
/// and pitches, rad/s, and its power, ft/s².
#[derive(Debug, Clone, Copy)]
struct AirSpec {
    stall: f32,
    top: f32,
    roll: f32,
    pitch: f32,
    power: f32,
    /// Its wheels' height under its middle, ft.
    gear: f32,
}

fn air_spec(model: &str) -> AirSpec {
    let m = model.to_ascii_lowercase().replace(' ', "");
    let is = |p: &str| m.starts_with(p);
    if is("supercub") {
        AirSpec { stall: 45.0, top: 170.0, roll: 2.3, pitch: 0.9, power: 40.0, gear: 3.5 }
    } else if is("biplane") {
        AirSpec { stall: 55.0, top: 190.0, roll: 1.9, pitch: 0.8, power: 42.0, gear: 4.5 }
    } else if is("beech") {
        AirSpec { stall: 75.0, top: 300.0, roll: 1.3, pitch: 0.6, power: 50.0, gear: 5.5 }
    } else if is("dc-3") || is("dc3") {
        AirSpec { stall: 80.0, top: 290.0, roll: 1.0, pitch: 0.5, power: 45.0, gear: 2.5 }
    } else if is("cargo") {
        AirSpec { stall: 90.0, top: 320.0, roll: 0.7, pitch: 0.4, power: 38.0, gear: 9.0 }
    } else if is("blimp") {
        AirSpec { stall: 0.0, top: 50.0, roll: 0.3, pitch: 0.0, power: 6.0, gear: 30.0 }
    } else if is("ufo") {
        AirSpec { stall: 0.0, top: 500.0, roll: 2.5, pitch: 0.0, power: 80.0, gear: 4.0 }
    } else if is("helicopter") || is("bell") || is("heli") {
        AirSpec { stall: 0.0, top: 200.0, roll: 1.4, pitch: 0.0, power: 30.0, gear: 3.0 }
    } else {
        AirSpec { stall: 55.0, top: 200.0, roll: 1.8, pitch: 0.8, power: 42.0, gear: 4.0 }
    }
}

/// What is parked on a track that has no traffic of its own, to drive
/// (the free tracks): the model, how far round it needs clear, ft, how
/// high its middle stands off the ground, and where it parks.
pub const FLEET: [(&str, f32, f32, Park); 31] = [
    ("Jeep.slt", 8.0, 0.0, Park::Lot),
    ("4Runner_Blue.slt", 8.0, 0.0, Park::Lot),
    ("MX3.slt", 8.0, 0.0, Park::Lot),
    ("300ZX.slt", 8.0, 0.0, Park::Lot),
    ("ChevelleRed.slt", 8.0, 0.0, Park::Lot),
    ("STS_Red.slt", 8.0, 0.0, Park::Lot),
    ("Cop.slt", 8.0, 0.0, Park::Lot),
    ("FamTrkStr.slt", 9.0, 0.0, Park::Lot),
    ("Jeep_Green.slt", 8.0, 0.0, Park::Lot),
    ("Cop_Navy.slt", 8.0, 0.0, Park::Lot),
    ("FedUpTruck.slt", 12.0, 0.0, Park::Lot),
    ("FireTruck.slt", 15.0, 0.0, Park::Lot),
    ("SemiCab.slt", 13.0, 0.0, Park::Lot),
    ("WaterTruck.slt", 14.0, 0.0, Park::Lot),
    ("CompanyTruck.slt", 13.0, 0.0, Park::Lot),
    ("SchoolBus.slt", 20.0, 0.0, Park::Lot),
    ("Greyhound.slt", 22.0, 0.0, Park::Lot),
    ("Airfield_Tractor.slt", 6.0, 0.0, Park::Lot),
    ("Tractor.slt", 10.0, 0.0, Park::Lot),
    ("Tractor_Old.slt", 8.0, 0.0, Park::Lot),
    ("Combine.slt", 30.0, 0.0, Park::Lot),
    ("DumpTruck.slt", 28.0, 0.0, Park::Lot),
    ("Scraper.slt", 32.0, 0.0, Park::Lot),
    ("BellRanger.slt", 30.0, 0.0, Park::Pad),
    ("UFO.slt", 32.0, 9.8, Park::Pad),
    ("Blimp.slt", 100.0, 60.0, Park::Pad),
    ("Supercub.slt", 18.0, 3.8, Park::Strip(500.0)),
    ("Biplane.slt", 16.0, 5.7, Park::Strip(550.0)),
    ("Beech18.slt", 32.0, 4.7, Park::Strip(800.0)),
    ("DC-3.slt", 50.0, 2.5, Park::Strip(1000.0)),
    ("CargoPlane.slt", 76.0, 25.6, Park::Strip(1400.0)),
];

/// How far a vehicle's model is turned about the up axis from facing its
/// way, rad. The original's face +z, as the game drives them, but some of
/// the free pack's stand-ins were made facing elsewhere (its aircraft and
/// a few trucks face -z, the biplane -x, the cargo plane +x, and the DC-3
/// 25° off -z), and are drawn turned to match.
pub fn model_yaw(model: &str) -> f32 {
    use std::f32::consts::{FRAC_PI_2, PI};
    if crate::freepack::root().is_none() {
        return 0.0;
    }
    let m = model.to_ascii_lowercase();
    const TURNED: [(&str, f32); 12] = [
        ("dumptruck", PI),
        ("firetruck", PI),
        ("greyhound", PI),
        ("semicab", PI),
        ("watertruck", PI),
        ("supercub", PI),
        ("biplane", FRAC_PI_2),
        ("beech18", PI),
        ("dc-3", PI + 25.3 * PI / 180.0),
        ("cargoplane", -FRAC_PI_2),
        ("bellranger", PI),
        ("helinocam", PI),
    ];
    TURNED.iter().find(|t| m.starts_with(t.0)).map_or(0.0, |t| t.1)
}

/// The stick's lean as a craft reads it. One thumb steers and leans
/// on a pad, so a hard turn pushes the stick a little forward or back
/// too, and a worn stick sits a little off the middle: neither should
/// tip the nose. The lean has a dead middle that widens as the stick
/// is pushed to the side, and the edge is still full.
fn flight_lean(lean: f32, steer: f32) -> f32 {
    let dead = 0.15 + 0.4 * steer.abs().min(1.0);
    lean.signum() * ((lean.abs() - dead) / (1.0 - dead)).clamp(0.0, 1.0)
}

/// The frames that spin on the craft models (the original's, and the free
/// pack's split out to match, freepack::split_blades): each one's name,
/// the axis it turns about and how fast, rad/s.
pub const BLADES: [(&str, Vec3, f32); 10] = [
    ("Cub_Prop", Vec3::Z, 38.0),
    ("Bip_Prop", Vec3::Z, 36.0),
    ("Prop1", Vec3::Z, 34.0),
    ("Prop2", Vec3::Z, 34.0),
    ("PropL", Vec3::Z, 30.0),
    ("PropR", Vec3::Z, 30.0),
    ("Prop3", Vec3::Z, 34.0),
    ("Prop4", Vec3::Z, 34.0),
    ("BellRngr_Main-Rotor", Vec3::Y, 24.0),
    ("BellRngr_Tail-Rotor", Vec3::X, 60.0),
];

/// How high a parked craft's middle stands, ft (it lands back there).
fn parked_lift(model: &str) -> Option<f32> {
    FLEET.iter().find(|f| f.0.eq_ignore_ascii_case(model)).map(|f| f.2)
}

/// The player at the wheel or the stick.
#[derive(Debug, Clone)]
pub struct Drive {
    pub seat: Seat,
    pub kind: Kind,
    pub name: String,
    /// Where its model stands and which way it faces, game coordinates:
    /// right, up and forward are kept square.
    pub position: Vec3,
    pub forward: Vec3,
    pub up: Vec3,
    /// Speed along its nose, ft/s (backward below 0).
    pub speed: f32,
    /// A car's or hovering craft's velocity, and a car's spin, rad/s.
    velocity: Vec3,
    spin: Vec3,
    /// A plane's sink, ft/s, under its stall speed.
    sink: f32,
    car: CarSpec,
    air: AirSpec,
    /// The front wheels' angle now, rad.
    steer: f32,
    /// How much each spring is squashed, 0 to its travel (for the sound
    /// of a hard landing).
    pub springs: [f32; 4],
    /// A train: how fast its clock runs to the scripted one, and its cars.
    rate: f32,
    consist: Vec<usize>,
    /// The craft's own scale on the track.
    scale: Vec3,
    big: bool,
    /// The chase camera's eye, smoothed.
    eye: Option<Vec3>,
    /// The seconds since getting in.
    pub time: f32,
    /// A craft's place in its own flight, to go back to after.
    pub home: Mat4,
    /// The way its model's nose points in the model.
    nose: Vec3,
    /// A hovering craft's level heading.
    heading: Option<Vec3>,
    /// A craft's root turn in its motion when taken over, kept under its
    /// place.
    pub key: Mat4,
    /// How long a car has lain over, s.
    upset: f32,
    /// Its engine's loop and pitch at rest (crate::drive_sound).
    pub voice: (&'static str, f32),
}

fn right_of(up: Vec3, forward: Vec3) -> Vec3 {
    up.cross(forward).normalize_or(Vec3::X)
}

/// `v` turned by `angle` toward `toward` (both square to each other).
fn turn(v: Vec3, toward: Vec3, angle: f32) -> Vec3 {
    v * angle.cos() + toward * angle.sin()
}

impl Drive {
    /// Into scripted vehicle `index`.
    pub fn car(traffic: &mut Traffic, index: usize, model: &str) -> Option<Drive> {
        // A parked aircraft is in the traffic too.
        let kind = air_kind(model).unwrap_or_else(|| ground_kind(model));
        let mut air = air_spec(model);
        if let Some(lift) = parked_lift(model).filter(|_| matches!(kind, Kind::Plane | Kind::Hover)) {
            air.gear = lift;
        }
        let path = traffic.cars.get(index)?.vehicle.path.clone();
        // A train's cars all go with the one the player is in.
        let consist: Vec<usize> = match kind {
            Kind::Rail => traffic.cars.iter().enumerate().filter(|(_, c)| c.vehicle.path == path).map(|(i, _)| i).collect(),
            _ => vec![index],
        };
        for &i in &consist {
            traffic.cars[i].driven = true;
        }
        let v = &traffic.cars[index].vehicle;
        let up = v.up.normalize_or(Vec3::Y);
        let forward = (v.heading - up * v.heading.dot(up)).normalize_or(Vec3::Z);
        Some(Drive {
            seat: Seat::Car(index),
            kind,
            name: nice_name(model),
            position: v.position,
            forward,
            up: forward.cross(right_of(up, forward)),
            speed: v.velocity.dot(forward),
            velocity: v.velocity,
            spin: Vec3::ZERO,
            sink: 0.0,
            car: car_spec(model),
            air,
            steer: 0.0,
            springs: [0.0; 4],
            rate: 1.0,
            consist,
            scale: Vec3::ONE,
            big: big(model),
            eye: None,
            time: 0.0,
            home: Mat4::IDENTITY,
            nose: Vec3::Z,
            heading: None,
            key: Mat4::IDENTITY,
            voice: crate::drive_sound::voice(model, kind),
            upset: 0.0,
        })
    }

    /// Into craft `c`, flying where it is at `time`: its root frame's place
    /// then, and its speed from where it was a moment before.
    pub fn craft(c: &Craft, model: &crate::objects::AnimatedModel, time: f32) -> Drive {
        let now = model.root_at(time);
        let before = model.root_at(time - 0.1);
        let scale = Vec3::new(now.x_axis.truncate().length(), now.y_axis.truncate().length(), now.z_axis.truncate().length());
        let turn = glam::Mat3::from_mat4(now) * glam::Mat3::from_diagonal(Vec3::ONE / scale.max(Vec3::splat(1e-6)));
        let nose = nose_axis(model, time);
        let forward = (turn * nose).normalize_or(Vec3::Z);
        let up = (turn * Vec3::Y).normalize_or(Vec3::Y);
        let position = now.w_axis.truncate();
        let velocity = (position - before.w_axis.truncate()) / 0.1;
        let air = air_spec(&c.name);
        eprintln!("drive: into the {}", c.name);
        Drive {
            seat: Seat::Air(c.object),
            kind: c.kind,
            name: c.name.clone(),
            position,
            forward,
            up: forward.cross(right_of(up, forward)),
            speed: velocity.dot(forward).max(if c.kind == Kind::Plane { air.stall * 1.6 } else { 0.0 }),
            velocity: if c.kind == Kind::Hover { velocity } else { Vec3::ZERO },
            spin: Vec3::ZERO,
            sink: 0.0,
            car: car_spec(""),
            air,
            steer: 0.0,
            springs: [0.0; 4],
            rate: 1.0,
            consist: Vec::new(),
            scale: if scale.min_element() > 0.0 { scale } else { Vec3::ONE },
            big: big(&model.model),
            eye: None,
            time: 0.0,
            home: model.place(),
            nose,
            heading: None,
            key: model.root_turn(time),
            voice: crate::drive_sound::voice(&model.model, c.kind),
            upset: 0.0,
        }
    }

    /// Its propellers' and rotors' frames, each one's axis and its turn
    /// now, rad: up to speed a moment after the engine starts.
    pub fn blades(&self) -> Vec<(&'static str, Vec3, f32)> {
        if !matches!(self.kind, Kind::Plane | Kind::Hover) {
            return Vec::new();
        }
        // Spun up over two seconds, then steady.
        let t = self.time;
        let turned = |rate: f32| rate * if t < 2.0 { t * t / 4.0 } else { t - 1.0 };
        BLADES.iter().map(|&(name, axis, rate)| (name, axis, turned(rate) % std::f32::consts::TAU)).collect()
    }

    /// How fast it goes flat out, ft/s.
    pub fn top_speed(&self) -> f32 {
        match self.kind {
            Kind::Car => self.car.top,
            Kind::Rail => 30.0,
            Kind::Plane | Kind::Hover => self.air.top,
        }
    }

    /// Its velocity, ft/s.
    pub fn velocity(&self) -> Vec3 {
        match self.kind {
            Kind::Car | Kind::Hover => self.velocity,
            Kind::Plane | Kind::Rail => self.forward * self.speed,
        }
    }

    /// How fast a car slides sideways, ft/s.
    pub fn slip(&self) -> f32 {
        let right = right_of(self.up, self.forward);
        self.velocity.dot(right).abs()
    }

    /// Whether a car has a wheel on the ground.
    pub fn on_ground(&self) -> bool {
        self.springs.iter().any(|s| *s > 0.0)
    }

    /// What it says on screen while driving.
    pub fn hint(&self) -> String {
        match self.kind {
            Kind::Plane => format!("Flying the {}: gas for power, lean back to climb, steer to bank. V or D-pad down to get out.", self.name),
            Kind::Hover => format!("Flying the {}: gas to climb, brake to sink, lean to go, steer to turn. V or D-pad down to get out.", self.name),
            Kind::Rail => format!("Driving the {}: gas and brake set the train's speed. V or D-pad down to get out.", self.name),
            Kind::Car => format!("Driving the {}. V or D-pad down to get out.", self.name),
        }
    }

    /// Where it is drawn: its place on the track, game coordinates.
    pub fn place(&self) -> Mat4 {
        let right = right_of(self.up, self.forward);
        let up = self.forward.cross(right);
        // The model's own axes onto the craft's: its nose along forward.
        let world = glam::Mat3::from_cols(self.forward, up, self.forward.cross(up));
        let model = glam::Mat3::from_cols(self.nose, Vec3::Y, self.nose.cross(Vec3::Y));
        let turn = world * model.transpose();
        let root = Mat4::from_cols(
            (turn.x_axis * self.scale.x).extend(0.0),
            (turn.y_axis * self.scale.y).extend(0.0),
            (turn.z_axis * self.scale.z).extend(0.0),
            self.position.extend(1.0),
        );
        // The place is under the root's turn in its motion.
        root * self.key.transpose()
    }

    /// Runs it on by `dt` with the player's `controls`.
    pub fn update(&mut self, controls: Controls, terrain: &Terrain, world: &World, traffic: &mut Traffic, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt;
        let controls = match self.kind {
            Kind::Plane | Kind::Hover => Controls { lean: flight_lean(controls.lean, controls.steer), ..controls },
            _ => controls,
        };
        match self.kind {
            Kind::Car => {
                // Small steps, for stiff springs.
                let steps = (dt * 240.0).ceil().max(1.0);
                for _ in 0..steps as usize {
                    self.roll(controls, terrain, world, traffic, dt / steps);
                }
                self.put_car(traffic);
            }
            Kind::Rail => self.rail(controls, terrain, world, traffic, dt),
            Kind::Plane => {
                self.fly(controls, terrain, dt);
                self.put_car(traffic);
            }
            Kind::Hover => {
                self.hover(controls, terrain, dt);
                self.put_car(traffic);
            }
        }
    }

    fn put_car(&self, traffic: &mut Traffic) {
        let Seat::Car(index) = self.seat else { return };
        if let Some(car) = traffic.cars.get_mut(index) {
            // Its model's own forward and up on the track.
            let turn = glam::Mat3::from_mat4(self.place());
            car.vehicle.position = self.position;
            car.vehicle.heading = (turn * Vec3::Z).normalize_or(self.forward);
            car.vehicle.up = (turn * Vec3::Y).normalize_or(self.up);
            car.vehicle.velocity = self.velocity;
        }
    }

    /// A train stays on its rails: gas and brake run its clock faster or
    /// slower than the timetable, and every car of it goes along.
    fn rail(&mut self, c: Controls, terrain: &Terrain, world: &World, traffic: &mut Traffic, dt: f32) {
        self.rate = (self.rate + (c.throttle * 0.25 - c.brake * 0.6) * dt).clamp(0.0, 3.0);
        let statics = traffic.first.min(world.solids.len());
        for &i in &self.consist {
            let car = &mut traffic.cars[i];
            let above = car.vehicle.position.y + 6.0;
            car.vehicle.update(dt * self.rate, |x, z| traffic::ground_at(terrain, world, statics, x, z, Some(above)));
            car.vehicle.velocity *= self.rate;
        }
        let Seat::Car(index) = self.seat else { return };
        let v = &traffic.cars[index].vehicle;
        self.position = v.position;
        let up = v.up.normalize_or(Vec3::Y);
        self.forward = (v.heading - up * v.heading.dot(up)).normalize_or(self.forward);
        self.up = self.forward.cross(right_of(up, self.forward));
        self.speed = v.velocity.length();
    }

    /// A car as a body on four springs: each wheel's spring holds the body
    /// up off the ground under it and damps its bounce; its tyre grips
    /// across and drives or brakes along, as far as its share of the
    /// weight lets it, so a car leans in turns, squats as it pulls away,
    /// dives as it brakes, slides when it asks too much, and flies off
    /// crests and lands on its springs. Walls and buildings stop it.
    fn roll(&mut self, c: Controls, terrain: &Terrain, world: &World, traffic: &Traffic, dt: f32) {
        let s = self.car;
        let Seat::Car(index) = self.seat else { return };
        let mine = traffic.cars.get(index).map(|c| c.solids.clone()).unwrap_or(0..0);
        let statics = traffic.first.min(world.solids.len());
        let right = right_of(self.up, self.forward);
        let centre = self.position + self.up * s.cg;
        let forward_speed = self.velocity.dot(self.forward);
        // The springs: four per unit of weight between them, bouncing at
        // their rate.
        let k = (std::f32::consts::TAU * s.hz).powi(2) / 4.0;
        let damp = 2.0 * s.damping * (k / 4.0).sqrt() * 2.0;
        let free = s.wheel + GRAVITY / 4.0 / k;
        // The wheel's turn: less lock at speed, eased in.
        let want = c.steer * s.lock / (1.0 + forward_speed.abs() / 70.0);
        self.steer += (want - self.steer) * (1.0 - (-8.0 * dt).exp());
        // The engine's pull, less as it nears its top speed; the brake held
        // at a standstill backs it up.
        let pull = if c.throttle > 0.0 {
            c.throttle * s.accel * (1.0 - (forward_speed / s.top).powi(2)).max(0.0)
        } else if c.brake > 0.0 && forward_speed < 1.5 {
            -c.brake * s.accel * 0.45 * (1.0 - (forward_speed / 30.0).powi(2)).max(0.0)
        } else {
            0.0
        };
        let braking = c.brake > 0.0 && forward_speed >= 1.5;
        let hold = c.throttle == 0.0 && c.brake == 0.0 && forward_speed.abs() < 3.0;
        let share = |front: bool| match (s.driven, front) {
            (Wheels::All, _) => 0.25,
            (Wheels::Front, true) | (Wheels::Rear, false) => 0.5,
            _ => 0.0,
        };
        let mut force = Vec3::new(0.0, -GRAVITY, 0.0);
        let mut torque = Vec3::ZERO;
        let mut touching = 0;
        for (w, (sx, sz)) in [(-1.0, 1.0), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)].into_iter().enumerate() {
            let front = sz > 0.0;
            let mount = self.position + right * (sx * s.half_track) + self.forward * (sz * s.half_base) + self.up * s.wheel;
            let (floor, normal) = traffic::ground_at(terrain, world, statics, mount.x, mount.z, Some(mount.y + 1.0));
            let length = (mount.y - floor) / self.up.y.max(0.3);
            // On its side or its back, no wheel is on the ground.
            if length >= free || self.up.y < 0.35 {
                self.springs[w] = 0.0;
                continue;
            }
            touching += 1;
            let contact = mount - self.up * length;
            let arm = contact - centre;
            let at = self.velocity + self.spin.cross(mount - centre);
            // However far into the ground (a hard landing nose first), the
            // spring is not squashed past its stop by much.
            let squash = (free - length).min(free + s.travel);
            let mut load = k * squash - damp * at.dot(self.up);
            // The bump stop, past the spring's travel.
            let past = squash - (free - s.wheel + s.travel);
            if past > 0.0 {
                load += k * 12.0 * past - damp * 2.0 * at.dot(self.up).min(0.0);
            }
            let load = load.max(0.0);
            self.springs[w] = squash.min(s.travel);
            // The tyre, on the ground's slope.
            let heading = if front { turn(self.forward, right, self.steer) } else { self.forward };
            let along = (heading - normal * heading.dot(normal)).normalize_or(heading);
            let across = normal.cross(along).normalize_or(right);
            let slip = self.velocity + self.spin.cross(arm);
            let mut side = -slip.dot(across) * 9.0;
            let mut drive = pull * share(front);
            let roll_speed = slip.dot(along);
            if braking {
                drive -= roll_speed.signum() * s.brake / 4.0 * c.brake;
            }
            if hold {
                drive -= roll_speed * 9.0;
            }
            drive -= roll_speed * 0.015;
            // The grip the tyre's load gives, shared along and across.
            let most = s.grip * load * 1.15;
            let total = (side * side + drive * drive).sqrt();
            if total > most && total > 0.0 {
                side *= most / total;
                drive *= most / total;
            }
            let push = self.up * load + across * side + along * drive;
            force += push;
            torque += arm.cross(push);
        }
        // A little air drag.
        force -= self.velocity * self.velocity.length() * 0.00015;
        // Turning: the body as a box of its length, width and height.
        let (l, w, h) = (s.half_base * 2.6, s.half_track * 2.2, s.cg * 1.6);
        let inertia = Vec3::new((l * l + h * h) / 12.0, (l * l + w * w) / 12.0, (w * w + h * h) / 12.0);
        let local = Vec3::new(torque.dot(right), torque.dot(self.up), torque.dot(self.forward)) / inertia;
        self.spin += (right * local.x + self.up * local.y + self.forward * local.z) * dt;
        self.spin *= (-if touching > 0 { 0.8 } else { 0.1 } * dt).exp();
        self.velocity += force * dt;
        // Into a wall or another vehicle: it stops along the wall and
        // bounces off a little.
        let step = self.velocity * dt;
        for (bx, bz) in [(-1.0, 1.0), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
            let corner = centre + right * (bx * s.half_track) + self.forward * (bz * (s.half_base + 2.0));
            let hit = world.crossing_among(corner, corner + step, |i| !mine.contains(&i), true);
            if let Some(hit) = hit {
                let n = Vec3::new(hit.normal.x, 0.0, hit.normal.z).normalize_or(hit.normal);
                let into = self.velocity.dot(n);
                if into < 0.0 {
                    self.velocity -= n * into * 1.3;
                    self.spin *= 0.5;
                }
            }
        }
        let centre = centre + self.velocity * dt;
        // Its turn.
        let angle = self.spin * dt;
        if angle.length_squared() > 0.0 {
            let q = glam::Quat::from_scaled_axis(angle);
            self.forward = (q * self.forward).normalize();
            self.up = (q * self.up).normalize();
        }
        self.up = self.forward.cross(right_of(self.up, self.forward)).normalize();
        // Never through the ground, whatever it lands on: on its body it
        // scrapes to a stop.
        let floor = crate::ride::ground(terrain, centre.x, centre.z);
        let low = floor + s.cg * 0.6;
        if centre.y < low {
            self.velocity.y = self.velocity.y.max(0.0);
            self.velocity *= (-2.5 * dt).exp();
            self.spin *= (-3.0 * dt).exp();
        }
        let centre = Vec3::new(centre.x, centre.y.max(low), centre.z);
        // Over on its side or its back and stopped, it is put back on its
        // wheels, as an arcade game does.
        if self.up.y < 0.35 && self.velocity.length() < 6.0 {
            self.upset += dt;
            if self.upset > 1.5 {
                self.upset = 0.0;
                self.forward = Vec3::new(self.forward.x, 0.0, self.forward.z).normalize_or(Vec3::Z);
                self.up = Vec3::Y;
                self.spin = Vec3::ZERO;
                self.velocity = Vec3::ZERO;
                let centre = Vec3::new(centre.x, floor + s.cg + s.wheel + 1.0, centre.z);
                self.position = centre - self.up * s.cg;
                self.speed = 0.0;
                return;
            }
        } else {
            self.upset = 0.0;
        }
        self.position = centre - self.up * s.cg;
        self.speed = self.velocity.dot(self.forward);
    }

    /// A plane: gas for power, lean back to climb and forward to dive,
    /// steering to bank, and a bank turns it. Under its stall speed it
    /// sinks; on the ground it rolls on its wheels and takes off again
    /// with speed.
    fn fly(&mut self, c: Controls, terrain: &Terrain, dt: f32) {
        let a = self.air;
        let mut right = right_of(self.up, self.forward);
        // Controls bite harder with speed.
        let bite = (self.speed / (a.stall * 1.5)).clamp(0.2, 1.0);
        // Pitch: lean back raises the nose, no further than steep.
        let nose = self.forward.y.clamp(-1.0, 1.0).asin();
        let mut pitch = -c.lean * a.pitch * bite;
        // Let go, it eases back toward level flight, as a plane trimmed
        // for it does.
        if c.lean.abs() < 0.05 {
            pitch = -nose * 0.7;
        }
        if (nose > 0.9 && pitch > 0.0) || (nose < -0.9 && pitch < 0.0) {
            pitch = 0.0;
        }
        let pitch = pitch * dt;
        let (f, u) = (turn(self.forward, self.up, pitch), turn(self.up, -self.forward, pitch));
        self.forward = f;
        self.up = u;
        // Roll: steering banks it as far as the stick is pushed, up to
        // about 65°, and let go it levels (an arcade plane never ends up
        // on its back).
        let bank = (-right.y).atan2(self.up.y);
        let want = c.steer.clamp(-1.0, 1.0) * 1.15;
        let roll = ((want - bank) * 3.0).clamp(-a.roll * bite, a.roll * bite);
        let (u, r) = (turn(self.up, right, roll * dt), turn(right, -self.up, roll * dt));
        self.up = u;
        right = r;
        // The bank turns it.
        let yaw = -right.y * a.roll * 0.55;
        let flat_right = right_of(Vec3::Y, self.forward);
        self.forward = turn(self.forward, flat_right, yaw * dt).normalize();
        self.up = self.forward.cross(right_of(self.up, self.forward)).normalize();
        // Speed: thrust, drag holding it to its top speed, gravity along
        // the nose.
        let thrust = a.power * (0.3 + 0.7 * c.throttle) - a.power * c.brake;
        self.speed += (thrust - GRAVITY * self.forward.y - self.speed * self.speed * (a.power / (a.top * a.top))) * dt;
        self.speed = self.speed.clamp(0.0, a.top * 1.4);
        let lift = ((self.speed - a.stall * 0.7) / (a.stall * 0.6)).clamp(0.0, 1.0);
        self.sink = (self.sink - GRAVITY * (1.0 - lift) * dt) * (1.0 - lift * (1.0 - (-3.0 * dt).exp()));
        // A stalled plane drops its nose.
        if lift < 0.5 {
            let (f, u) = (turn(self.forward, -self.up, (0.5 - lift) * 0.8 * dt), turn(self.up, self.forward, (0.5 - lift) * 0.8 * dt));
            self.forward = f;
            self.up = u;
        }
        self.position += self.forward * self.speed * dt + Vec3::Y * self.sink * dt;
        // The ground: it rolls on its wheels, nose no lower than level.
        let floor = crate::ride::ground(terrain, self.position.x, self.position.z) + a.gear;
        if self.position.y < floor {
            self.position.y = floor;
            self.sink = 0.0;
            if self.forward.y < 0.0 {
                let into = -self.forward.y * self.speed;
                self.forward = Vec3::new(self.forward.x, 0.0, self.forward.z).normalize_or(Vec3::Z);
                if into > 25.0 {
                    self.speed *= 0.5;
                }
            }
            // Steering works the tail wheel on the ground.
            let flat = Vec3::new(self.forward.x, 0.0, self.forward.z).normalize_or(Vec3::Z);
            let along = turn(flat, right_of(Vec3::Y, flat), c.steer * 0.8 * dt);
            self.forward = (along + Vec3::Y * self.forward.y.max(0.0)).normalize();
            let right = right_of(Vec3::Y, self.forward);
            self.up = self.forward.cross(right).normalize();
            self.speed = (self.speed - (4.0 + 30.0 * c.brake) * dt).max(0.0);
        }
    }

    /// A helicopter, blimp or UFO: gas climbs and brake sinks, lean forward
    /// goes and back backs, steering turns it on the spot; it tips the
    /// way it goes.
    fn hover(&mut self, c: Controls, terrain: &Terrain, dt: f32) {
        let a = self.air;
        // Its heading is kept level on its own: the tip below would turn
        // it if it were taken from the tipped nose.
        let flat = match self.heading {
            Some(h) => h,
            None => Vec3::new(self.forward.x, 0.0, self.forward.z).normalize_or(Vec3::Z),
        };
        let flat = turn(flat, right_of(Vec3::Y, flat), c.steer * a.roll * dt).normalize();
        self.heading = Some(flat);
        let side = right_of(Vec3::Y, flat);
        let mut accel = flat * (c.lean * a.power);
        accel.y = (c.throttle - c.brake) * a.power * 0.8;
        // Drag holding it to its top speed.
        let level = Vec3::new(self.velocity.x, 0.0, self.velocity.z);
        accel -= level * level.length() * (a.power / (a.top * a.top)) + level * 0.15;
        self.velocity += accel * dt;
        self.velocity.y *= (-1.5 * dt).exp();
        self.position += self.velocity * dt;
        let floor = crate::ride::ground(terrain, self.position.x, self.position.z) + a.gear;
        if self.position.y < floor {
            self.position.y = floor;
            self.velocity.y = self.velocity.y.max(0.0);
            self.velocity *= (-2.0 * dt).exp();
        }
        // Tipped toward where it is going, and into the turn.
        let go = a.power.max(1.0) * 2.0;
        let ahead = self.velocity.dot(flat) / go + c.lean * 0.12;
        let across = self.velocity.dot(side) / go + c.steer * 0.1;
        let tip = if self.big || a.top > 400.0 { 0.25 } else { 1.0 };
        let up = (Vec3::Y + flat * ahead.clamp(-0.4, 0.4) * tip + side * across.clamp(-0.4, 0.4) * tip).normalize();
        self.up = up;
        self.forward = (flat - up * flat.dot(up)).normalize();
        self.speed = self.velocity.length();
    }

    /// The chase camera's eye and target, game coordinates.
    pub fn camera(&mut self, terrain: &Terrain, dt: f32) -> (Vec3, Vec3) {
        let size = match self.kind {
            Kind::Car | Kind::Rail => self.car.half_base * 2.0 + 6.0,
            Kind::Plane => if self.big { 70.0 } else { 26.0 },
            // A helicopter's tail is long and high: the camera stays clear
            // of it.
            Kind::Hover => if self.big { 90.0 } else { 34.0 },
        };
        // From higher over a helicopter, so its long tail boom isn't seen end on.
        let lift = if self.kind == Kind::Hover || (self.big && self.kind == Kind::Car) { 0.5 } else { 0.3 };
        let (back, high) = (size * 1.05 + 4.0, size * lift + 3.5);
        let flat = Vec3::new(self.forward.x, 0.0, self.forward.z).normalize_or(Vec3::Z);
        // Planes are followed along their nose, cars level behind.
        let behind = match self.kind {
            Kind::Plane => (self.forward * 0.6 + flat * 0.4).normalize(),
            _ => flat,
        };
        let want = self.position - behind * back + Vec3::Y * high;
        let eye = match self.eye {
            Some(e) => e.lerp(want, 1.0 - (-5.0 * dt).exp()),
            None => want,
        };
        let floor = crate::ride::ground(terrain, eye.x, eye.z) + 3.0;
        let eye = Vec3::new(eye.x, eye.y.max(floor), eye.z);
        self.eye = Some(eye);
        (eye, self.position + self.forward * 12.0 + Vec3::Y * high * 0.35)
    }

    /// Where the bike goes when the player gets out: on the ground beside
    /// it, facing its way.
    pub fn way_out(&self, terrain: &Terrain) -> (Vec3, Vec3) {
        let flat = Vec3::new(self.forward.x, 0.0, self.forward.z).normalize_or(Vec3::Z);
        let side = match self.kind {
            Kind::Car | Kind::Rail => self.car.half_track + 5.0,
            _ => if self.big { 40.0 } else { 14.0 },
        };
        let at = self.position - right_of(Vec3::Y, flat) * side;
        let at = Vec3::new(at.x, crate::ride::ground(terrain, at.x, at.z), at.z);
        (at, flat)
    }

    /// Lets go of the vehicle: a car or train stays where it was left.
    pub fn leave(&self, traffic: &mut Traffic) {
        // A train runs on to its timetable again.
        if self.kind == Kind::Rail {
            for &i in &self.consist {
                traffic.cars[i].driven = false;
            }
        }
    }
}

/// Which way a craft's model points its nose: the level axis of its root
/// frame it flies along fastest, level, in four minutes of its motion.
fn nose_axis(model: &crate::objects::AnimatedModel, time: f32) -> Vec3 {
    let mut best = (0.0, Vec3::Z);
    for k in 0..240 {
        let t = time + k as f32;
        let (a, b) = (model.root_at(t), model.root_at(t + 0.25));
        let moved = b.w_axis.truncate() - a.w_axis.truncate();
        let moved = Vec3::new(moved.x, 0.0, moved.z);
        let speed = moved.length();
        if speed > best.0 {
            let local = glam::Mat3::from_mat4(a).transpose() * moved;
            let axis = [Vec3::X, -Vec3::X, Vec3::Z, -Vec3::Z]
                .into_iter()
                .max_by(|p, q| p.dot(local).total_cmp(&q.dot(local)))
                .unwrap_or(Vec3::Z);
            best = (speed, axis);
        }
    }
    best.1
}

/// The nearest scripted vehicle the bike at `at` can get in, if any.
pub fn nearest_car(traffic: &Traffic, at: Vec3) -> Option<usize> {
    traffic
        .cars
        .iter()
        .enumerate()
        // A trailer or wagon has no driver: its cab or tractor does.
        .filter(|(_, c)| c.object.is_some() && c.vehicle.lag <= 0.0)
        // `MM2_DRIVE_PICK=name`: only models starting so, for tests.
        .filter(|(_, c)| std::env::var("MM2_DRIVE_PICK").map_or(true, |p| c.model.to_ascii_lowercase().starts_with(&p.to_ascii_lowercase())))
        .map(|(i, c)| {
            let reach = REACH + if big(&c.model) { 14.0 } else { 6.0 };
            (i, c.vehicle.position.distance(at) - reach)
        })
        // `MM2_DRIVE_ANY`: any distance, for tests.
        .filter(|(_, d)| *d <= 0.0 || std::env::var_os("MM2_DRIVE_ANY").is_some())
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// The nearest craft to `at` at `time`, anywhere on the track.
pub fn nearest_craft<'a>(crafts: &'a [Craft], at: Vec3, root: impl Fn(usize) -> Option<Vec3>) -> Option<&'a Craft> {
    crafts
        .iter()
        .filter_map(|c| Some((c, root(c.object)?.distance(at))))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(c, _)| c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_read_well() {
        assert_eq!(nice_name("FireTruck.Slt"), "Fire Truck");
        assert_eq!(nice_name("4Runner_Blue.slt"), "4Runner");
        assert_eq!(nice_name("Jeep_grey.Slt"), "Jeep");
        assert_eq!(nice_name("HeliNoCam.slt"), "Helicopter");
        assert_eq!(nice_name("ChevelleRed.Slt"), "Chevelle Red");
    }

    #[test]
    fn crafts_are_found_by_model() {
        let models = ["Rider02.slt", "Supercub.slt", "HeliNoCam.slt", "CargoPlane.slt", "Gondola.slt"];
        let found = aircraft(models.iter().copied());
        let kinds: Vec<_> = found.iter().map(|c| (c.object, c.kind)).collect();
        assert_eq!(kinds, [(1, Kind::Plane), (2, Kind::Hover), (3, Kind::Plane)]);
    }

    #[test]
    fn steering_hard_does_not_tip_the_nose() {
        assert_eq!(flight_lean(0.12, 0.0), 0.0);
        assert_eq!(flight_lean(-0.4, 1.0), 0.0);
        assert!(flight_lean(-0.6, 0.0) < -0.5);
        assert_eq!(flight_lean(1.0, 1.0), 1.0);
        assert_eq!(flight_lean(-1.0, 0.0), -1.0);
    }
}
