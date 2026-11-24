//! The Tools page's "Watch an AI lap": the game's computer riders lapping
//! a Course Builder track in the page's own 3D view, with no game to load.
//! Physics and AI only: the riders are `mm2_physics::computer::Rider` (the
//! game's two-wheel model and controller), led along the `.tdf` course the
//! way the race leads them (`mm2-viewer`'s `race.rs`, `Racer::plan` and
//! `Racer::ride`), and a crash throws them onto the full bike model until
//! they get up, as in a race. Their frame is the race's fixed one, so
//! watching faster runs more frames, never longer ones.

mod course;

use course::{flat, Course, Place};
use glam::{Mat3, Vec3};
use mm2_formats::scn::{self, Zone};
use mm2_formats::trn::Terrain;
use mm2_formats::{faoe, rs2, tdf, Container};
use mm2_physics::ai::{Mover, Path};
use mm2_physics::computer::{self, Rider};
use mm2_physics::{race, Controls, Ground};
use wasm_bindgen::prelude::*;

/// The computer riders' frame in a race, s (`race::AI_FRAME`).
const FRAME: f32 = 0.028;
/// Off the course this long and the controller puts the rider back on
/// (0x415170), s.
const OFF_COURSE_RESET: f32 = 5.0;
/// The middle AI level of three.
const LEVEL: usize = 1;
/// Off the start the riders hold their slot, then ease into their lanes.
const START_HOLD: f32 = 2.0;
const START_EASE: f32 = 6.0;
const EDGE_ROOM: f32 = 4.0;
/// A computer rider passes a gate anywhere within this many times its
/// width (as the race has it).
const COMPUTER_GATE: f32 = 2.0;
/// The gate arch: it stands this far above the ground, half its width and
/// its height as the game's gate box cuts them (`mm2-viewer`'s gates.rs).
const GATE_RAISE: f32 = 2.0;
const GATE_HALF_WIDTH: f32 = 18.5 * 0.98;
const GATE_HEIGHT: f32 = 19.4 * 0.67;
/// Floats a rider gives the page each frame, see [`Sim::poses`].
pub const POSE: usize = 16;

/// The track the riders ride: the terrain, its zones' grip, and whether
/// the world's edge throws a rider back (all but Supercross). The page's
/// tracks have no objects, so nothing solid stands on them.
struct Track {
    terrain: Terrain,
    zones: [Zone; 8],
    edge: bool,
}

impl Track {
    fn zone_index(&self, at: Vec3) -> u8 {
        let t = &self.terrain;
        let last = t.size - 1;
        let x = ((at.x / t.scale).max(0.0).round() as usize).min(last);
        let z = ((at.z / t.scale).max(0.0).round() as usize).min(last);
        t.surfaces[z * t.size + x] & 7
    }

    fn zone(&self, at: Vec3) -> Zone {
        self.zones[self.zone_index(at) as usize]
    }
}

fn cell(t: &Terrain, x: f32, z: f32) -> (usize, usize, f32, f32) {
    let last = t.size.saturating_sub(2) as f32;
    let (tx, tz) = ((x / t.scale).max(0.0), (z / t.scale).max(0.0));
    let (cx, cz) = (tx.floor().min(last), tz.floor().min(last));
    (cx as usize, cz as usize, tx - cx, tz - cz)
}

/// Terrain height (0x50d3b0), as `mm2-viewer`'s `ride::ground`.
fn height(t: &Terrain, x: f32, z: f32) -> f32 {
    let (cx, cz, fx, fz) = cell(t, x, z);
    let h = |dx: usize, dz: usize| t.height(cx + dx, cz + dz);
    let (h00, h10, h01, h11) = (h(0, 0), h(1, 0), h(0, 1), h(1, 1));
    let y = if (cx ^ cz) & 1 == 0 {
        if fx <= fz {
            (h11 - h01) * fx + (h00 - h01) * (1.0 - fz) + h01
        } else {
            (h10 - h00) * fx + (h11 - h10) * fz + h00
        }
    } else if fx + fz >= 1.0 {
        (h11 - h01) * fx + (h10 - h11) * (1.0 - fz) + h01
    } else {
        (h10 - h00) * fx + (h01 - h00) * fz + h00
    };
    y * t.scale
}

/// The surface normal (0x50d3b0), as `mm2-viewer`'s `ride::normal`.
fn normal(t: &Terrain, x: f32, z: f32) -> Vec3 {
    let (cx, cz, fx, fz) = cell(t, x, z);
    let d = [
        fx * fx + fz * fz,
        (1.0 - fx).powi(2) + fz * fz,
        fx * fx + (1.0 - fz).powi(2),
        (1.0 - fx).powi(2) + (1.0 - fz).powi(2),
    ]
    .map(|d: f32| if d < 0.001 { 0.001 } else { d });
    let corners = if (cx ^ cz) & 1 == 0 {
        if d[2] <= d[1] {
            [0, 2, 3]
        } else {
            [0, 1, 2]
        }
    } else if d[3] <= d[0] {
        [1, 2, 3]
    } else {
        [0, 1, 2]
    };
    let mut n = Vec3::ZERO;
    for c in corners {
        let s = t.normals[(cz + c / 2) * t.size + cx + c % 2];
        n += Vec3::new(s[0] as f32, s[1] as f32, s[2] as f32) / 127.0 / d[c];
    }
    n.normalize_or(Vec3::Y)
}

impl Ground for Track {
    fn terrain(&self, x: f32, z: f32) -> (f32, Vec3) {
        (height(&self.terrain, x, z), normal(&self.terrain, x, z))
    }

    fn edge(&self) -> Option<(f32, f32)> {
        let side = (self.terrain.size - 1) as f32 * self.terrain.scale;
        self.edge.then_some((side, side))
    }

    fn friction(&self, at: Vec3) -> f32 {
        self.zone(at).friction
    }

    fn traction(&self, at: Vec3) -> f32 {
        self.zone(at).traction
    }
}

/// One computer rider and the race's hold on it.
struct One {
    rider: Rider,
    /// Thrown: the full bike model until it gets up.
    wreck: Option<Box<mm2_physics::Bike>>,
    /// Its confirmed place on the course, and whether it is on it now.
    place: Option<Place>,
    on_course: bool,
    off_course: f32,
    air_lean: f32,
    /// Its own lane, and where across the gate it started.
    lane: f32,
    from: f32,
    /// Where it stood last frame (to draw between frames), and its
    /// probe, for the finish line.
    was: (Vec3, Vec3, Vec3),
    probe: Vec3,
    target: usize,
    passed: usize,
    laps: u32,
    lap_start: f32,
    last_lap: f32,
    best_lap: f32,
    /// In a gate race: the next gate, the place on the course the line
    /// is laid from, and where the bike was last frame.
    gate: usize,
    line_place: Option<Place>,
    last: Vec3,
}

impl One {
    /// Where the rider is and its axes: the two-wheel model's, or the
    /// thrown bike's.
    fn stance(&self) -> (Vec3, Vec3, Vec3) {
        match &self.wreck {
            Some(w) => {
                let m = w.place();
                (
                    m.w_axis.truncate(),
                    m.z_axis.truncate(),
                    m.y_axis.truncate(),
                )
            }
            None => (self.rider.position, self.rider.look, self.rider.up),
        }
    }
}

#[wasm_bindgen]
pub struct Sim {
    track: Track,
    course: Course,
    /// A Baja or Enduro track's gates, from its `.wpt`; none in a lap race.
    gates: Vec<race::Line>,
    riders: Vec<One>,
    clock: f32,
    left: f32,
    supercross: bool,
}

fn err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

#[wasm_bindgen]
impl Sim {
    /// The riders on `env` (a baked `.ENV`): `count` of them on the gate.
    /// `supercross` for a stadium track, whose edge holds no one back.
    #[wasm_bindgen(constructor)]
    pub fn new(env: Vec<u8>, count: usize, supercross: bool) -> Result<Sim, JsError> {
        let bytes = match Container::detect(&env) {
            Container::Rs2 => env,
            Container::Faoe => faoe::decode(&env, "track.env").map_err(err)?,
            Container::Unknown => return Err(JsError::new("not a track")),
        };
        let ar = rs2::Archive::parse(&bytes).map_err(err)?;
        let find = |ext: &str| {
            ar.entries()
                .iter()
                .find(|e| e.name.to_ascii_lowercase().ends_with(ext))
                .map(|e| ar.data(e))
        };
        let terrain = mm2_formats::trn::parse(
            find(".trn").ok_or_else(|| JsError::new("the track has no ground"))?,
        )
        .map_err(err)?;
        let def = tdf::parse(find(".tdf").ok_or_else(|| JsError::new("the track has no course"))?)
            .map_err(err)?;
        let zones = find(".scn")
            .and_then(|s| scn::Scene::parse(s).ok())
            .map_or_else(Default::default, |s| s.zones);
        let course =
            Course::new(&def).ok_or_else(|| JsError::new("the course is too short to ride"))?;
        let start = def
            .start_line()
            .ok_or_else(|| JsError::new("the track has no start"))?;
        let track = Track {
            terrain,
            zones,
            edge: !supercross,
        };
        // A Baja or Enduro's gates, from its `.wpt` (the race's line
        // through each arch, `mm2-viewer`'s `Gate::line`).
        let gates: Vec<race::Line> = find(".wpt")
            .map(|w| {
                mm2_physics::ai::waypoints(&String::from_utf8_lossy(w))
                    .into_iter()
                    .map(|(at, look)| {
                        let look = look.normalize_or(Vec3::Z);
                        let y = height(&track.terrain, at.x, at.z);
                        race::Line {
                            centre: Vec3::new(at.x, y, at.z),
                            forward: look,
                            half_width: Vec3::new(look.z, 0.0, -look.x) * GATE_HALF_WIDTH,
                            height: GATE_HEIGHT + GATE_RAISE,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut sim = Sim {
            track,
            course,
            gates,
            riders: Vec::new(),
            clock: 0.0,
            left: 0.0,
            supercross,
        };
        sim.grid(&start, count.clamp(1, 12));
        Ok(sim)
    }

    /// Rides on `seconds` of race time, in the race's whole frames.
    pub fn advance(&mut self, seconds: f32) {
        self.left += seconds.clamp(0.0, 1.0);
        while self.left >= FRAME {
            self.left -= FRAME;
            self.frame();
        }
    }

    /// Every rider's pose, drawn between its last two frames: position
    /// (3), forward (3), up (3), steer, lean, front and rear springs,
    /// speed ft/s, airborne (0/1), crashed (0/1).
    pub fn poses(&self) -> Vec<f32> {
        let k = self.left / FRAME;
        let mut out = Vec::with_capacity(self.riders.len() * POSE);
        for one in &self.riders {
            let (p, f, u) = one.stance();
            let (wp, wf, wu) = one.was;
            let p = wp.lerp(p, k);
            let f = wf.lerp(f, k).normalize_or(f);
            let u = wu.lerp(u, k).normalize_or(u);
            let r = &one.rider;
            let down = one.wreck.is_some();
            out.extend_from_slice(&[p.x, p.y, p.z, f.x, f.y, f.z, u.x, u.y, u.z]);
            out.extend_from_slice(&[r.steer, r.lean, r.springs[0], r.springs[1]]);
            out.push(if down { 0.0 } else { r.speed.max(0.0) });
            out.push(r.airborne as u8 as f32);
            out.push(down as u8 as f32);
        }
        out
    }

    /// The world's side, ft: the ground runs from 0 to this on x and z.
    pub fn side(&self) -> f32 {
        (self.track.terrain.size - 1) as f32 * self.track.terrain.scale
    }

    /// The ground's height at `n` x `n` points evenly over the world, row
    /// by row, for the page to draw what the riders ride on.
    pub fn heights(&self, n: usize) -> Vec<f32> {
        let n = n.max(2);
        let step = self.side() / (n - 1) as f32;
        (0..n * n)
            .map(|k| {
                height(
                    &self.track.terrain,
                    (k % n) as f32 * step,
                    (k / n) as f32 * step,
                )
            })
            .collect()
    }

    /// The ground's zone (0..7, as the track paints it) at the same
    /// points as [`Sim::heights`].
    pub fn zones(&self, n: usize) -> Vec<u8> {
        let n = n.max(2);
        let step = self.side() / (n - 1) as f32;
        (0..n * n)
            .map(|k| {
                self.track
                    .zone_index(Vec3::new((k % n) as f32 * step, 0.0, (k / n) as f32 * step))
            })
            .collect()
    }

    /// The ground's height at `x`, `z`, ft.
    pub fn height(&self, x: f32, z: f32) -> f32 {
        height(&self.track.terrain, x, z)
    }

    pub fn count(&self) -> usize {
        self.riders.len()
    }

    /// Race time so far, s.
    pub fn clock(&self) -> f32 {
        self.clock
    }

    /// Each rider's laps done, last lap and best lap (s, 0 for none yet).
    pub fn laps(&self) -> Vec<f32> {
        self.riders
            .iter()
            .flat_map(|r| [r.laps as f32, r.last_lap, r.best_lap])
            .collect()
    }
}

impl Sim {
    /// The gate: the riders side by side behind the start line, each with
    /// a lane of its own off the line (±7 ft, from the game's rand()).
    fn grid(&mut self, start: &tdf::Line, count: usize) {
        let at = Vec3::from(start.position);
        let way = flat(Vec3::from(start.direction)).normalize_or(Vec3::Z);
        let across = flat(Vec3::from(start.across));
        let half = across.length().max(6.0);
        let side = across.normalize_or(Vec3::new(way.z, 0.0, -way.x));
        let gap = (half * 2.0 / count as f32).min(5.0);
        for i in 0..count {
            let off = (i as f32 - (count - 1) as f32 / 2.0) * gap;
            let spot = at - way * 8.0 + side * off;
            let mut seed = (i as u32 + 1).wrapping_mul(2_654_435_761) ^ 0x5bd1_e995;
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let lane = ((seed >> 16) & 0x7fff) as f32 / 32768.0 * 14.0 - 7.0;
            // Lanes count to the course's right (z, 0, −x).
            let from = off * side.dot(Vec3::new(way.z, 0.0, -way.x)).signum();
            let rider = Rider::new(&self.track, spot, way, from, computer::engine_class(250));
            let p = rider.position;
            self.riders.push(One {
                rider,
                wreck: None,
                place: None,
                on_course: true,
                off_course: 0.0,
                air_lean: 0.0,
                lane,
                from,
                was: (p, way, Vec3::Y),
                probe: Vec3::ZERO,
                target: self.course.nearest(spot),
                passed: 0,
                laps: 0,
                lap_start: 0.0,
                last_lap: 0.0,
                best_lap: 0.0,
                gate: 0,
                line_place: None,
                last: p,
            });
        }
    }

    fn frame(&mut self) {
        self.clock += FRAME;
        let tolerance = computer::tolerance(LEVEL, self.supercross);
        // Where every bike is and will be in a quarter second, for each
        // rider's obstacle scan (as the race gives it).
        let going: Vec<(Vec3, Vec3)> = self
            .riders
            .iter()
            .map(|r| {
                let v = r.wreck.as_ref().map_or(r.rider.velocity, |w| w.velocity);
                (r.stance().0, v)
            })
            .collect();
        for i in 0..self.riders.len() {
            let movers: Vec<Mover> = (0..going.len())
                .filter(|&j| j != i)
                .map(|j| Mover {
                    at: going[j].0,
                    to: going[j].0 + going[j].1 * 0.25,
                })
                .collect();
            let one = &mut self.riders[i];
            one.was = one.stance();
            if self.gates.is_empty() {
                ride(
                    &self.track,
                    &self.course,
                    one,
                    self.clock,
                    tolerance,
                    &movers,
                );
                lap(&self.course, one, self.clock);
            } else {
                ride_gates(
                    &self.track,
                    &self.course,
                    &self.gates,
                    one,
                    tolerance,
                    &movers,
                );
                pass_gate(&self.gates, one, self.clock);
            }
        }
    }
}

/// One frame of a computer rider in a lap race (`Racer::ride` and the
/// lap race branch of `Racer::plan`).
fn ride(
    track: &Track,
    course: &Course,
    one: &mut One,
    clock: f32,
    tolerance: f32,
    movers: &[Mover],
) {
    let ground: &dyn Ground = track;
    // Where it is on the course (0x4eb170, from 3.5 ft ahead).
    let (at, look, _) = one.stance();
    one.on_course = course
        .follow(at + look * 3.5, flat(look), &mut one.place)
        .is_some();
    one.off_course = if one.on_course {
        0.0
    } else {
        one.off_course + FRAME
    };
    if let Some(w) = &mut one.wreck {
        w.reset_to = one.place.map(|p| (course.point(p), course.way(p)));
        w.step(ground, Controls::default(), FRAME);
        if w.crash.is_none() {
            let place = w.place();
            let (lane, class) = (one.rider.lane, one.rider.class);
            one.rider = Rider::new(
                ground,
                place.w_axis.truncate(),
                place.z_axis.truncate(),
                lane,
                class,
            );
            one.wreck = None;
        }
        return;
    }
    if one.off_course >= OFF_COURSE_RESET {
        if let Some(p) = one.place {
            let (at, way) = (course.point(p), course.way(p));
            let (lane, class) = (one.rider.lane, one.rider.class);
            one.rider = Rider::new(ground, at - way * 3.5, way, lane, class);
        }
        one.off_course = 0.0;
    }
    // The line: ten seconds' riding along the course from its place, a
    // tenth at a time, lifted 2 ft over the bike.
    let r = &mut one.rider;
    let (pos, speed) = (r.position, r.speed);
    let raise = |p: Vec3| Vec3::new(p.x, pos.y + 2.0, p.z);
    let points: Vec<Vec3> = match one.place {
        None => vec![pos + r.look * speed],
        Some(place) if one.on_course => (0..100)
            .map(|i| raise(course.point(course.walk(place, i as f32 * 0.1 * speed))))
            .collect(),
        Some(place) => vec![raise(course.point(course.walk(place, 100.0)))],
    };
    let mut path = Path::new(points, speed * 0.25);
    path.limit = 1000.0;
    path.power = computer::POWER[LEVEL];
    path.widen = true;
    r.near_only = one.on_course;
    r.tolerance = tolerance;
    r.air_pose = true;
    // Off the start the line runs from its slot into its own lane.
    let lane = one.lane;
    r.lane = match one.place {
        Some(on) if clock < START_HOLD + START_EASE => {
            let k = ((clock - START_HOLD) / START_EASE).clamp(0.0, 1.0);
            let k = k * k * (3.0 - 2.0 * k);
            let room = (course.half_width(on) - EDGE_ROOM).max(0.0);
            (one.from + (lane - one.from) * k).clamp(-room, room)
        }
        _ => lane,
    };
    let crash = r.step(ground, &mut path, FRAME, &mut one.air_lean, false, movers);
    r.lane = lane;
    if crash {
        one.wreck = Some(throw(
            ground,
            r,
            one.place.map(|p| (course.point(p), course.way(p))),
        ));
    }
}

/// One frame of a computer rider in a Baja or Enduro (`Racer::ride` and
/// the gate branch of `Racer::plan`): along the course from its kept
/// place toward the next gate's place, ten seconds' riding laid out a
/// tenth at a time, while that gets it nearer the gate; else straight at
/// the gate. A crashed rider gets up where it lies, facing the next gate.
fn ride_gates(
    track: &Track,
    course: &Course,
    gates: &[race::Line],
    one: &mut One,
    tolerance: f32,
    movers: &[Mover],
) {
    let ground: &dyn Ground = track;
    let gate = gates[one.gate.min(gates.len() - 1)];
    if let Some(w) = &mut one.wreck {
        w.reset_to = None;
        w.step(ground, Controls::default(), FRAME);
        if w.crash.is_none() {
            let place = w.place();
            let at = place.w_axis.truncate();
            let to = flat(gate.centre - at);
            let facing = if to.length_squared() > 1e-6 {
                to
            } else {
                place.z_axis.truncate()
            };
            let (lane, class) = (one.rider.lane, one.rider.class);
            one.rider = Rider::new(ground, at, facing, lane, class);
            one.wreck = None;
        }
        return;
    }
    let r = &mut one.rider;
    let (at, speed) = (r.position, r.speed);
    let raise = |p: Vec3| Vec3::new(p.x, at.y + 2.0, p.z);
    let mut limit: f32 = 1000.0;
    let g = gate.centre;
    let far = |from: Place, to: Place| course.along(from, to, f32::MAX).unwrap_or(0.0);
    let target = course.nearest_place(g);
    let here = course.nearest_place(at);
    let mut kept = one.line_place.or(here);
    let (before, now) = match (kept, here, target) {
        (Some(k), Some(h), Some(t)) => (far(k, t), far(h, t)),
        _ => (0.0, 0.0),
    };
    if now > 0.0 && before > 0.0 && now < before {
        kept = here;
    }
    one.line_place = kept;
    let (dx, dz) = (at.x - g.x, at.z - g.z);
    let near_only = mm2_physics::ai::rough_sqrt(dx * dx + dz * dz) < speed;
    if near_only {
        let to = (g - at).normalize_or_zero();
        if to.dot(r.velocity.normalize_or_zero()) < 0.707_55 {
            limit = 40.0;
        }
    }
    let points: Vec<Vec3> = match kept {
        Some(k) if before > now => (0..100)
            .map(|i| raise(course.point(course.walk(k, i as f32 * 0.1 * speed))))
            .collect(),
        _ => vec![g],
    };
    let mut path = Path::new(points, speed * 0.25);
    path.limit = limit;
    path.power = computer::POWER[LEVEL];
    r.near_only = near_only;
    r.tolerance = tolerance;
    r.air_pose = true;
    let crash = r.step(ground, &mut path, FRAME, &mut one.air_lean, true, movers);
    if crash {
        one.wreck = Some(throw(ground, r, None));
    }
}

/// Through the next gate (`Racer::pass_gates`, a computer rider's gate
/// twice as wide); past the last, round again from the first, a lap.
fn pass_gate(gates: &[race::Line], one: &mut One, clock: f32) {
    let at = one.stance().0;
    let from = std::mem::replace(&mut one.last, at);
    let gate = gates[one.gate.min(gates.len() - 1)];
    let wide = race::Line {
        half_width: gate.half_width * COMPUTER_GATE,
        ..gate
    };
    if !wide.crossed(from, at, true) {
        return;
    }
    one.gate += 1;
    if one.gate >= gates.len() {
        one.gate = 0;
        let time = clock - one.lap_start;
        one.laps += 1;
        one.last_lap = time;
        one.best_lap = if one.best_lap == 0.0 {
            time
        } else {
            one.best_lap.min(time)
        };
        one.lap_start = clock;
    }
}

/// The crash (`Computer::crash`): the rider goes on the full bike model,
/// thrown, until the game's crash clock lets it up.
fn throw(ground: &dyn Ground, r: &Rider, reset_to: Option<(Vec3, Vec3)>) -> Box<mm2_physics::Bike> {
    let mut bike = mm2_physics::Bike::with_spec(ground, r.position, r.look, Default::default());
    let right = r.up.cross(r.look).normalize_or(Vec3::X);
    let up = r.look.cross(right);
    let axes = Mat3::from_cols(right, up, r.look);
    bike.set_state(
        r.position + axes * mm2_physics::bike::COM,
        r.velocity,
        axes,
        r.spin,
        [0.0; 2],
    );
    bike.reset_to = reset_to;
    bike.crash_time = mm2_physics::bike::GAME_CRASH_TIME;
    bike.touch_clock = true;
    bike.fixed_step = true;
    bike.fall(mm2_physics::Crash::Impact);
    Box::new(bike)
}

/// Moves its course target past points it has reached and counts laps
/// over the finish line (`Racer::track`).
fn lap(course: &Course, one: &mut One, clock: f32) {
    let (at, look, _) = one.stance();
    let p = flat(at);
    for _ in 0..course.points.len() {
        let here = course.points[one.target];
        let next = course.points[course.next[one.target]];
        let along = flat(next - here);
        if (p - flat(here)).dot(along) < 0.0 || (p - flat(next)).length() > along.length() + 40.0 {
            break;
        }
        one.target = course.next[one.target];
        one.passed += 1;
    }
    let probe = at + look * 3.5;
    let crossed = course.finish_line().crossed(one.probe, probe, true);
    one.probe = probe;
    if crossed && one.passed * 10 >= course.lap * 6 {
        let time = clock - one.lap_start;
        one.laps += 1;
        one.passed = 0;
        one.last_lap = time;
        one.best_lap = if one.best_lap == 0.0 {
            time
        } else {
            one.best_lap.min(time)
        };
        one.lap_start = clock;
    }
}
