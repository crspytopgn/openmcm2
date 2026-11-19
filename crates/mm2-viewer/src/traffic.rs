//! A scene's scripted traffic: the cars, trucks and trains that drive set
//! loops (CarProcedural, update 0x430ca0). Each one is drawn as an
//! animated object and is solid to the bikes through its `.col`.

use glam::{Mat4, Vec3};
use mm2_formats::{col, rs2, scn};
use mm2_physics::collision::{Solid, World};
use mm2_physics::procedural::{self, Path, Vehicle};
use std::ops::Range;

use crate::ride;
use mm2_formats::trn::Terrain;

/// One vehicle, its solids in the track's world and its drawn object.
pub struct Car {
    pub vehicle: Vehicle,
    pub solids: Range<usize>,
    pub object: Option<usize>,
    /// Its place in the scene's procedural animations, for its sound.
    pub source: usize,
    /// The player drives it (crate::drive): off its path, where they
    /// leave it.
    pub driven: bool,
    /// Its model (the `.slt` name).
    pub model: String,
}

#[derive(Default)]
pub struct Traffic {
    pub cars: Vec<Car>,
    /// The world's solids before the first vehicle's: the track's own.
    pub first: usize,
}

/// How far a vehicle may step up onto an object in one update, ft (our
/// pick).
const CLIMB: f32 = 6.0;

/// The scene's vehicles and their models, from `RES/PROCVUE.RES`'s paths,
/// The source of a vehicle that stands parked, not one of the scene's.
pub const PARKED: usize = usize::MAX;

/// each with its index in the scene's procedural animations.
pub fn load(scene: &scn::Scene, procvue: Option<&rs2::Archive>) -> Vec<(usize, Vehicle, scn::Placement)> {
    let Some(vues) = procvue else {
        return Vec::new();
    };
    scene
        .procedural
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let text = String::from_utf8_lossy(vues.get(&p.vue)?);
            let mut path = Path::parse(&text, p.fps);
            if path.keys.len() < 2 {
                return None;
            }
            for k in &mut path.keys {
                k.1 = procedural::to_game(k.1);
            }
            let vehicle = Vehicle::new(path, p.lag_distance, Vec3::from(p.offset));
            let model = scn::Placement {
                model: p.model.clone(),
                position: [0.0; 3],
                look: [0.0, 0.0, 1.0],
                up: [0.0, 1.0, 0.0],
                scale: [1.0; 3],
                lighting: Default::default(),
            };
            Some((i, vehicle, model))
        })
        .collect()
}

/// A vehicle's body solids from its `.col` in `RES/COLSEG.RES`: the parts
/// not hung on a frame (wheels).
pub fn solids(model: &str, colseg: &rs2::Archive) -> Vec<Solid> {
    let stem = model.rsplit_once('.').map_or(model, |(s, _)| s);
    let name = format!("{}.col", stem.trim().to_ascii_lowercase());
    colseg
        .entries()
        .iter()
        .find(|e| e.name.to_ascii_lowercase() == name)
        .and_then(|e| col::parse(colseg.data(e)).ok())
        .map(|c| {
            c.parts
                .iter()
                .filter(|p| p.frame.is_none())
                .map(|p| Solid::new(&p.body, glam::Mat3::IDENTITY, Vec3::new(0.0, -1.0e6, 0.0)))
                .collect()
        })
        .unwrap_or_default()
}

/// The ground a vehicle stands on at `x`, `z`: the terrain or a track
/// object's top under `above` (30 ft over the terrain when not given), as
/// its height and normal.
pub fn ground_at(terrain: &Terrain, statics: &[Solid], x: f32, z: f32, above: Option<f32>) -> (f32, Vec3) {
    let h = ride::ground(terrain, x, z);
    let n = ride::normal(terrain, Vec3::new(x, 0.0, z));
    let top = above.map_or(h + 30.0, |a| a.max(h));
    statics
        .iter()
        .filter_map(|s| s.crossing(Vec3::new(x, top, z), Vec3::new(x, h - 0.1, z)))
        .min_by(|a, b| a.along.total_cmp(&b.along))
        .map_or((h, n), |c| (c.point.y, c.normal))
}

impl Traffic {
    /// Runs every vehicle's clock on by `dt` and moves its solids.
    pub fn update(&mut self, dt: f32, terrain: &Terrain, world: &mut World) {
        for car in &mut self.cars {
            // It drives over the track's objects too, as the Ski Lodge's
            // trucks cross its bridge (0x439e10): from a little above
            // where it was, down to the terrain.
            let statics = &world.solids[..self.first];
            let placed = car.vehicle.position != Vec3::ZERO;
            let above = car.vehicle.position.y + CLIMB;
            if !car.driven {
                car.vehicle.update(dt, |x, z| ground_at(terrain, statics, x, z, placed.then_some(above)));
            }
            let (turn, at) = (car.vehicle.rotation(), car.vehicle.position);
            for s in &mut world.solids[car.solids.clone()] {
                s.place(turn, at, car.vehicle.velocity);
            }
        }
    }

    /// Where each vehicle's object is drawn.
    pub fn places(&self) -> impl Iterator<Item = (usize, Mat4)> + '_ {
        self.cars.iter().filter_map(|c| {
            let mut place = Mat4::from_mat3(c.vehicle.rotation());
            // A model made facing back is turned round to face its way.
            let yaw = crate::drive::model_yaw(&c.model);
            if yaw != 0.0 {
                place *= Mat4::from_rotation_y(yaw);
            }
            place.w_axis = c.vehicle.position.extend(1.0);
            Some((c.object?, place))
        })
    }
}
