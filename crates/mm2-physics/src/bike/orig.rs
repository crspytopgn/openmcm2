//! The game's own vehicle step, followed call for call (0x52ffb0 and what
//! it calls): `Bike::original` switches the bike to this from the older
//! `tick`. Ours throughout; the game's order of work, its state and its
//! numbers.
//!
//! The model's origin is what moves (the game's `position`, the scene
//! node's place); the bike's `position` stays the `NullFrame` point
//! (`COM`) on top of it, as the rest of the engine reads it.

use glam::{Mat3, Vec3};

use super::*;

/// The fork's anchor is the bars' frame; its stroke runs from there to
/// the front axle (InlineShock 0x4fe9b0 sets it from the two frames).
const FORK_MIN: f32 = 1.0;
/// The swingarm swings 32° at most (RotatingShock, 0x4ff7d0).
const SWING_MAX: f32 = 32.0 * 0.017_453_29;
/// Shock rates (Bike's loader): stiffness from soft to stiff over the
/// travel, the damping ratio, and the speed past which damping eases.
const SHOCK_KNEE: f32 = 80.0;
const RETRACT: f32 = 0.25;
/// The suspension's damping mass takes 0.9 of the bike (Vehicle 0x52ab70).
const SUSPENSION_SHARE: f32 = 0.9;
/// The pump (0x40d3d0) and the drag.
const LEAN_SPEED: f32 = 32.0;
const SIDE_LIE: f32 = 0.996_194_7;
/// The rider's weight shift: eased over 6.3 steps (Bike's loader).
const COM_EASE: f32 = STEP * 6.3;
/// Smoothed vertical acceleration's time constant (7 steps).
const LIFT_EASE: f32 = STEP * 7.0;
/// The Wrecker's 15 probes (Bike::CreateWrecker, 0x40c0f0) with the rider
/// in the centred riding pose (Rider.mcf's Turn2D, first frame), in the
/// bike's model space: for when nothing poses the rider each frame.
const RIDING_PROBES: [Vec3; 15] = [
    Vec3::new(-0.020, 4.185, -1.641),
    Vec3::new(-0.019, 4.919, -1.150),
    Vec3::new(-0.012, 6.412, -0.003),
    Vec3::new(-0.646, 5.437, -0.526),
    Vec3::new(0.644, 5.436, -0.555),
    Vec3::new(-1.075, 4.561, -0.392),
    Vec3::new(-1.072, 3.767, 0.713),
    Vec3::new(1.000, 4.530, -0.402),
    Vec3::new(1.055, 3.745, 0.713),
    Vec3::new(-0.575, 3.123, -0.308),
    Vec3::new(-0.651, 1.114, -0.202),
    Vec3::new(-0.615, 0.848, -1.063),
    Vec3::new(0.606, 3.152, -0.340),
    Vec3::new(0.702, 1.154, -0.228),
    Vec3::new(0.628, 0.887, -1.086),
];

/// The nose-over limit on the shipped tracks: every terrain header has 3
/// at Terrain+0xbe8, and anything but 1 makes it cos 70° (UpdateCrashState).
const NOSE_OVER: f32 = 0.342;

/// One shock (Shock 0x4fe9xx; InlineShock for the fork, RotatingShock
/// for the swingarm).
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Shock {
    pub length: f32,
    pub prev_length: f32,
    pub cleared: f32,
    pub ratio: f32,
    pub at_limit: bool,
    pub extending: bool,
    /// The swingarm's turn, rad.
    pub angle: f32,
    pub spring: Vec3,
    pub damper: Vec3,
    pub displacement: Vec3,
    pub step_displacement: Vec3,
    pub velocity: Vec3,
    pub speed: f32,
    pub impulse: f32,
}

/// A collision point's state (CollisionPoint; a tyre's is its base).
#[derive(Debug, Clone, Copy)]
pub(super) struct Point {
    pub world: Vec3,
    pub surface: Vec3,
    pub normal: Vec3,
    pub relative: Vec3,
    pub applied: Vec3,
    pub friction_dir: Vec3,
    pub rot_velocity: Vec3,
    pub normal_load: f32,
    pub friction_force: Vec3,
    pub friction_mag: f32,
    pub grip: f32,
    pub tangent_speed: f32,
    pub spin_speed: f32,
    pub penetration: f32,
    pub threshold: f32,
    pub load: f32,
    pub in_contact: bool,
    pub state: u8,
    /// The body points' own place on their frame, and the friction they
    /// were made with (0.6).
    pub local: Vec3,
    pub on_bars: bool,
    pub coefficient: f32,
}

impl Default for Point {
    fn default() -> Self {
        Point {
            world: Vec3::ZERO,
            surface: Vec3::ZERO,
            normal: Vec3::Y,
            relative: Vec3::ZERO,
            applied: Vec3::ZERO,
            friction_dir: Vec3::ZERO,
            rot_velocity: Vec3::ZERO,
            normal_load: 0.0,
            friction_force: Vec3::ZERO,
            friction_mag: 0.0,
            grip: 1.0,
            tangent_speed: 0.0,
            spin_speed: 0.0,
            penetration: -999.0,
            threshold: 0.0,
            load: 0.5,
            in_contact: false,
            state: 0,
            local: Vec3::ZERO,
            on_bars: false,
            coefficient: 0.6,
        }
    }
}

/// A tyre (Tire, 0x5135f0..0x51b1d0).
#[derive(Debug, Clone, Copy)]
pub(super) struct Wheel {
    pub cp: Point,
    pub center: Vec3,
    pub roll: Vec3,
    pub side: Vec3,
    pub forward: Vec3,
    pub lever: Vec3,
    pub slip_vector: Vec3,
    pub slip_speed: f32,
    pub tangent_cos: f32,
    pub tangent_sin: f32,
    pub drive_force: Vec3,
    pub drive_mag: f32,
    pub drive_share: f32,
    pub complement: f32,
    pub scale_b: f32,
    pub ramp: f32,
    pub engaged: bool,
    pub in_contact: bool,
    pub report: bool,
    pub lean_cos: f32,
    pub rest_load: f32,
    pub on_object: bool,
}

impl Default for Wheel {
    fn default() -> Self {
        Wheel {
            cp: Point::default(),
            center: Vec3::ZERO,
            roll: Vec3::Z,
            side: Vec3::NEG_X,
            forward: Vec3::Z,
            lever: Vec3::ZERO,
            slip_vector: Vec3::ZERO,
            slip_speed: 0.0,
            tangent_cos: 0.0,
            tangent_sin: 1.0,
            drive_force: Vec3::ZERO,
            drive_mag: 0.0,
            drive_share: 1.0,
            complement: 0.0,
            scale_b: 1.0,
            ramp: 0.0,
            engaged: false,
            in_contact: false,
            report: false,
            lean_cos: 1.0,
            rest_load: 0.5,
            on_object: false,
        }
    }
}

/// The body's angles (OrientationAnglesFromVectors, 0x4b5a60).
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Angles {
    pub forward: Vec3,
    pub up: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub sin_roll: f32,
    pub cos_roll: f32,
    pub cos_pitch: f32,
    pub sin_pitch: f32,
}

impl Angles {
    fn of(forward: Vec3, up: Vec3) -> Self {
        let (ax, ay, az) = (forward.x, forward.y, forward.z);
        let (bx, by, bz) = (up.x, up.y, up.z);
        let len = (ax * ax + az * az).sqrt();
        let (c, s) = if len == 0.0 { (1.0, 0.0) } else { (az / len, ax / len) };
        let mut yaw = c.clamp(-1.0, 1.0).acos();
        if ax < 0.0 {
            yaw = -yaw;
        }
        let fz = c * az + s * ax;
        let uz = c * bz + s * bx;
        let ux = c * bx - s * bz;
        let len = (fz * fz + ay * ay).sqrt();
        let (cos_pitch, sin_pitch) = if len == 0.0 { (1.0, 0.0) } else { (fz / len, ay / len) };
        let mut pitch = cos_pitch.clamp(-1.0, 1.0).acos();
        if ay < 0.0 {
            pitch = -pitch;
        }
        let uy = by * cos_pitch - uz * sin_pitch;
        let len = (uy * uy + ux * ux).sqrt();
        let (cos_roll, sin_roll) = if len == 0.0 { (1.0, 0.0) } else { (uy / len, ux / len) };
        let mut roll = cos_roll.clamp(-1.0, 1.0).acos();
        if ux < 0.0 {
            roll = -roll;
        }
        let half = std::f32::consts::FRAC_PI_2;
        let tau = std::f32::consts::TAU;
        if (pitch - half).abs() < 0.1745 {
            yaw -= roll;
            if yaw < 0.0 {
                yaw += tau;
            }
            roll = 0.0;
        } else if (pitch + half).abs() < 0.1745 {
            yaw += roll;
            if yaw > tau {
                yaw -= tau;
            }
            roll = 0.0;
        }
        Angles { forward, up, yaw, pitch, roll, sin_roll, cos_roll, cos_pitch, sin_pitch }
    }
}

/// What the game's step keeps between steps that the older model does
/// not.
#[derive(Debug, Clone, Copy)]
pub(super) struct Orig {
    /// The frame's time, for the rider's pose (SelectRiderPose).
    pub frame: f32,
    pub shocks: [Shock; 2],
    pub wheels: [Wheel; 2],
    /// Left bar end, right bar end, tail (Bike's loader).
    pub points: [Point; 3],
    pub angle: f32,
    pub steer_rate: f32,
    /// The rider's weight shift (Bike +0x640), 0.5 centred.
    pub steer_value: f32,
    pub lean_angle: f32,
    pub lean_cos: f32,
    pub turn_angle: f32,
    pub forward_speed: f32,
    pub moving_forward: bool,
    pub prev_speed: f32,
    pub acceleration: Vec3,
    pub prev_lean: f32,
    pub landing_latched: bool,
    pub wobble_time: f32,
    pub wobble_stage: u8,
    pub wobble_sign: f32,
    pub wobble_offset: f32,
    pub points_touching: bool,
    pub touching: u32,
    pub wheel_point: bool,
    pub wheels_in_contact: u32,
    pub front_lifted: bool,
    pub pivot: Vec3,
    pub world_spin: Vec3,
    pub saved: Angles,
    pub ready: bool,
}

impl Default for Orig {
    fn default() -> Self {
        let bar = |x: f32| Point {
            local: Vec3::new(x, 0.225, -0.228),
            on_bars: true,
            threshold: 0.0,
            penetration: -999.0,
            ..Point::default()
        };
        let tail = Point {
            local: Vec3::new(0.0, 3.345, -3.206),
            threshold: -0.3,
            penetration: -0.3 - 999.0,
            ..Point::default()
        };
        let rest_rear = (FRONT_AXLE.z - COM.z).abs() / wheel_base();
        let mut wheels = [Wheel::default(); 2];
        wheels[0].rest_load = 1.0 - rest_rear;
        wheels[1].rest_load = rest_rear;
        Orig {
            frame: 0.0,
            shocks: [Shock::default(); 2],
            wheels,
            points: [bar(-1.305), bar(1.305), tail],
            angle: 0.0,
            steer_rate: 0.0,
            steer_value: 1.0 - rest_rear,
            lean_angle: 0.0,
            lean_cos: 1.0,
            turn_angle: 0.0,
            forward_speed: 0.0,
            moving_forward: true,
            prev_speed: 0.0,
            acceleration: Vec3::ZERO,
            prev_lean: 0.0,
            landing_latched: false,
            wobble_time: 0.0,
            wobble_stage: 0,
            wobble_sign: 0.0,
            wobble_offset: 0.0,
            points_touching: false,
            touching: 0,
            wheel_point: false,
            wheels_in_contact: 0,
            front_lifted: false,
            pivot: COM,
            world_spin: Vec3::ZERO,
            saved: Angles::of(Vec3::Z, Vec3::Y),
            ready: false,
        }
    }
}

/// The two wheels' frames' distance along the ground (Bike's loader).
fn wheel_base() -> f32 {
    (FRONT_AXLE.z - REAR_AXLE.z).abs()
}

/// The fork's stroke, model space, from the bars toward the axle.
fn fork_axis() -> Vec3 {
    (FRONT_AXLE - HANDLEBARS).normalize()
}

fn fork_length() -> f32 {
    (FRONT_AXLE - HANDLEBARS).length()
}

fn arm_length() -> f32 {
    (REAR_AXLE - SWINGARM_PIVOT).length()
}

fn chord() -> f32 {
    (SWING_MAX * 0.5).sin() * arm_length() * 2.0
}

fn min_length() -> f32 {
    FORK_MIN.min(fork_length())
}

/// The world's height and normal under `p` (Terrain 0x507c10), and `p`
/// put on it.
fn query(ground: &dyn Ground, p: Vec3) -> (Vec3, Vec3) {
    let (h, n) = ground.terrain(p.x, p.z);
    (Vec3::new(p.x, h, p.z), n.normalize_or(Vec3::Y))
}

fn norm_or(v: Vec3, fallback: Vec3) -> Vec3 {
    if v == Vec3::ZERO {
        fallback
    } else {
        v.normalize_or_zero()
    }
}

fn project(v: Vec3, onto: Vec3) -> Vec3 {
    if onto == Vec3::ZERO {
        Vec3::ZERO
    } else {
        onto * (v.dot(onto) / onto.dot(onto))
    }
}

fn sign(x: f32) -> f32 {
    if x < 0.0 { -1.0 } else { 1.0 }
}

/// The static-friction clamp on the spin (0x4cb6e0): `a` the spin, `b`
/// the acceleration so far, `c` the friction's.
fn settle(a: &mut Vec3, b: &mut Vec3, c: Vec3, dt: f32) {
    for k in 0..3 {
        if c[k] == 0.0 {
            continue;
        }
        if a[k] == 0.0 {
            if b[k] != 0.0 && sign(b[k]) != sign(c[k]) {
                b[k] = if b[k].abs() > c[k].abs() { b[k] + c[k] } else { 0.0 };
            }
        } else {
            let t = dt * b[k] + a[k];
            let d = dt * c[k];
            if sign(t) == sign(d) || t.abs() > d.abs() || t == 0.0 {
                b[k] += c[k];
            } else {
                a[k] = 0.0;
                b[k] = 0.0;
            }
        }
    }
}

/// Where the model's origin is and how it is turned, and the game's node
/// calls on it.
#[derive(Debug, Clone, Copy)]
struct Frame {
    origin: Vec3,
    o: Mat3,
}

impl Frame {
    fn point(&self, local: Vec3) -> Vec3 {
        self.origin + self.o * local
    }
    fn local_point(&self, world: Vec3) -> Vec3 {
        self.o.transpose() * (world - self.origin)
    }
    fn local(&self, world: Vec3) -> Vec3 {
        self.o.transpose() * world
    }
    fn world(&self, local: Vec3) -> Vec3 {
        self.o * local
    }
    /// SoultreeObject::RotateAboutPoint: `pivot` and `axis` local.
    fn rotate_about(&mut self, pivot: Vec3, axis: Vec3, angle: f32) {
        if !angle.is_finite() || axis == Vec3::ZERO {
            return;
        }
        let at = self.point(pivot);
        self.o *= Mat3::from_axis_angle(axis.normalize(), angle);
        self.origin = at - self.o * pivot;
    }
}

impl Bike {
    fn frame(&self) -> Frame {
        Frame { origin: self.position - self.orientation * COM, o: self.orientation }
    }

    fn set_frame(&mut self, f: Frame) {
        self.orientation = f.o;
        self.orthonormalize();
        self.position = f.origin + self.orientation * COM;
    }

    /// The bars' frame: the model turned by the steering about the fork
    /// (SteeringControl 0x4cd2c0: −angle about the axis).
    fn bars(&self, o: Mat3) -> Mat3 {
        o * Mat3::from_axis_angle(fork_axis(), -self.o.angle)
    }

    fn wheel_center(&self, i: usize, f: &Frame) -> Vec3 {
        let s = &self.o.shocks[i];
        if i == 0 {
            let length = if s.at_limit { min_length() } else { s.length };
            f.point(FRONT_AXLE - fork_axis() * length)
        } else {
            let arm = REAR_AXLE - SWINGARM_PIVOT;
            f.point(SWINGARM_PIVOT + Mat3::from_rotation_x(s.angle) * arm)
        }
    }

    /// Loads the old model's tyres from the shocks' lengths, so the
    /// switch can be thrown on a bike already riding.
    fn orig_start(&mut self) {
        self.o = Orig::default();
        for i in 0..2 {
            let s = self.tyres[i].squeeze.max(0.0);
            let sh = &mut self.o.shocks[i];
            if i == 0 {
                sh.length = s.min(min_length());
                sh.ratio = sh.length / min_length();
            } else {
                sh.length = s.min(chord());
                sh.angle = sh.length / arm_length();
                sh.ratio = sh.length / chord();
            }
            sh.prev_length = sh.length;
            sh.cleared = sh.length;
        }
        self.o.angle = self.steer;
        let f = self.frame();
        self.o.saved = Angles::of(f.o.z_axis, f.o.y_axis);
        self.o.prev_speed = self.velocity.length();
        self.o.world_spin = f.o * self.spin;
        self.o.prev_lean = self.rider.1;
        self.o.ready = true;
    }

    /// Advances the bike by `dt` the way the game's frame does (0x52ffb0,
    /// SoultreePhysicsBaseObject 0x506b20 for the steps).
    pub(super) fn step_original(&mut self, ground: &dyn Ground, controls: Controls, dt: f32) {
        if !self.o.ready {
            self.orig_start();
        }
        self.hit = None;
        self.o.frame = dt;
        let frame_time = dt + self.pending;
        let whole = (frame_time / STEP) as u32;
        let mut steps;
        if whole > 0 {
            self.pending = frame_time - whole as f32 * STEP;
            steps = whole.min(MAX_STEPS);
            self.h = STEP;
        } else {
            if self.fixed_step {
                self.pending = frame_time;
                return;
            }
            self.pending = 0.0;
            self.h = if dt != 0.0 { frame_time } else { STEP };
            steps = 1;
        }
        let crashed = self.crash.is_some();
        // UpdateControlInput (0x52f130): nothing while crashed.
        let c = if crashed { Controls::default() } else { controls };
        let prev_lean = self.o.prev_lean;
        self.rider.0 = c.steer.clamp(-1.0, 1.0);
        self.rider.1 += (c.lean.clamp(-1.0, 1.0) - self.rider.1).clamp(-0.25, 0.25);
        if crashed {
            self.rider = (0.0, 0.0);
        }
        self.o.prev_lean = self.rider.1;
        let rest = if !self.fixed_step && self.pending > 1e-4 { self.pending } else { 0.0 };
        // ComputeSteerRate (0x52f2f0), toward the bars' lock over the
        // frame.
        if !crashed {
            let want = self.rider.0 * self.max_steer();
            let rate = want - self.o.angle;
            self.o.steer_rate = if rate.abs() < 1e-5 { 0.0 } else { rate / (steps as f32 * self.h + rest) };
        } else {
            self.o.steer_rate = 0.0;
        }
        self.launch(dt, prev_lean);
        self.o.points_touching = false;
        if !crashed && steps != 1 {
            let refresh = self.should_refresh();
            if self.refresh_points(ground, refresh) {
                self.o.points_touching = self.o.touching > 0;
            }
        }
        loop {
            let crashed_now = self.tick_original(ground, c, steps);
            if crashed_now {
                steps = 1;
                self.pending = 0.0;
            }
            steps -= 1;
            if steps == 0 && !self.fixed_step && self.pending > 1e-4 {
                steps = 1;
                self.h = self.pending;
                self.pending = 0.0;
            }
            if steps == 0 {
                break;
            }
        }
    }

    /// Where the first of the frame's points (bar ends, tail) touches the
    /// ground, for the original step: Vehicle::UpdateContactEffect's third
    /// pass (0x52c2d0) puts the dust there once no tyre is down, as when a
    /// crashed bike slides on its side.
    pub fn frame_contact(&self) -> Option<Vec3> {
        if !self.original {
            return None;
        }
        self.o.points.iter().find(|p| p.in_contact && p.penetration >= 0.0).map(|p| p.surface)
    }

    /// Bike::GetMaxSteerAngle (0x40c6e0): less lock the faster.
    fn max_steer(&self) -> f32 {
        let full = STEER_LOCK * self.sliders.steering;
        let limit = (full - (self.o.prev_speed / STEER_FADE * full * full).sqrt()).max(0.0);
        full.min(limit)
    }

    /// ShouldRefreshContacts (0x40b4e0): the frame's points are looked at
    /// only leaned over or pitched far.
    fn should_refresh(&self) -> bool {
        self.o.saved.roll.abs() > 0.2618 || self.o.saved.pitch.abs() > 1.05
    }

    /// The rider's pump (0x40d3d0): a quick lean forward off a rising
    /// bike kicks one shock.
    fn launch(&mut self, frame: f32, prev_lean: f32) {
        if self.lift <= 5.0 || frame == 0.0 {
            return;
        }
        let [front, rear] = self.o.wheels;
        if !rear.engaged && !front.engaged {
            return;
        }
        let rate = (self.rider.1 - prev_lean) / frame;
        if self.rider.1 <= 0.4 || rate <= 11.0 {
            return;
        }
        let impulse = ((self.lift * 0.6).min(100.0) * rate * 5.0).min(10000.0);
        if rear.engaged && rear.cp.tangent_speed > 30.0 {
            self.o.shocks[1].impulse = impulse;
        } else if front.engaged && front.cp.tangent_speed > 30.0 {
            self.o.shocks[0].impulse = impulse;
        }
    }

    /// The frame's collision points against the terrain (0x43ad80).
    fn refresh_points(&mut self, ground: &dyn Ground, enabled: bool) -> bool {
        if !enabled {
            return false;
        }
        let f = self.frame();
        let bars = self.bars(f.o);
        let mut count = 0;
        for p in &mut self.o.points {
            let world = if p.on_bars {
                f.origin + f.o * HANDLEBARS + bars * p.local
            } else {
                f.point(p.local)
            };
            p.world = world;
            let (surface, normal) = query(ground, world);
            p.surface = surface;
            p.normal = normal;
            p.penetration = (surface.y - world.y) * normal.y;
            p.in_contact = p.penetration >= p.threshold;
            if p.in_contact {
                count += 1;
            }
            p.grip = ground.friction(world);
        }
        self.o.touching = count;
        true
    }

    /// One step (the body of 0x52ffb0's loop). True when it crashed.
    fn tick_original(&mut self, ground: &dyn Ground, c: Controls, steps: u32) -> bool {
        self.previous = Some((self.position, self.orientation, self.velocity));
        let h = self.h;
        let lst = self.last_h;
        let crashed = self.crash.is_some();
        // Mass: once the rider is thrown (the Wrecker running, IsWrecking)
        // the bike's weight alone (UpdateMass(0), 0x52ffb0's crashed branch).
        let rider = if crashed { 0.0 } else { RIDER };
        let weight = self.spec.weight + rider;
        let mass = weight * 0.031_055_9;
        let inv_mass = 1.0 / mass;
        let suspension_mass = (SUSPENSION_SHARE * self.spec.weight + rider) * 0.031_055_9;
        let inv_inertia = if crashed {
            Vec3::ONE / (CRASHED_INERTIA * (self.spec.weight * 0.031_055_9) / 12.0)
        } else {
            Vec3::ONE / (INERTIA * mass / 12.0)
        };

        // Brakes' ramps, the engine and the gearbox.
        if !crashed {
            // The brake is a button (UpdateWheelRampLevels): its ramp
            // runs to full.
            for w in &mut self.o.wheels {
                if c.brake > 0.0 {
                    if w.ramp < 1.0 {
                        w.ramp = (w.ramp + h * BRAKE_ON * self.sliders.press_brakes).min(1.0);
                    }
                } else if w.ramp > 0.0 {
                    w.ramp = (w.ramp - h * BRAKE_OFF * self.sliders.release_brakes).max(0.0);
                }
            }
        }
        let backing = !crashed && self.reverse && self.o.forward_speed < BACK_UP_SPEED;
        if !backing {
            self.paddle.0 = 0.0;
        }
        let disengaged = !crashed && (backing || self.clutch);
        let throttle_on = !crashed && (self.racing || disengaged) && c.throttle > 0.0;
        self.drivetrain.update(h, throttle_on, disengaged);
        // The back-up motion's phase (0x4933e0, in the rider's pose).
        if backing {
            let speed = self.o.prev_speed;
            let (phase, rate) = &mut self.paddle;
            if *phase < 0.45 {
                *rate = speed.max(1.0) * h * PADDLE_RATE;
            }
            *phase += *rate;
            if *phase > 1.0 {
                *phase -= 1.0;
            }
        }

        let mut prev_speed = self.velocity.length();
        if prev_speed < 0.001 || !prev_speed.is_finite() {
            prev_speed = 0.0;
            self.velocity = Vec3::ZERO;
        }
        self.o.prev_speed = prev_speed;
        let mut linear = prev_speed;
        let mut f = self.frame();
        let saved = Angles::of(f.o.z_axis, f.o.y_axis);
        self.o.saved = saved;
        let mut center = f.point(COM);
        if !crashed {
            self.o.angle += self.o.steer_rate * h;
            if !self.o.angle.is_finite() || self.o.angle.abs() < 1e-4 {
                self.o.angle = 0.0;
            }
        }
        let torque_scale = 1.0;
        self.place_wheels(ground, &f, &saved, lst);
        let wic = self.o.wheels_in_contact;
        let all = wic == 2;
        let any = wic > 0;
        let mut speed = prev_speed;
        let mut force = Vec3::new(0.0, -weight, 0.0);
        let [front, rear] = self.o.wheels;
        let basis = (front.cp.world - rear.cp.world).normalize_or(f.o.z_axis);
        let lean_pivot = f.local_point(rear.cp.world);
        self.lean_from_normal(basis, &f, &saved);
        let refresh = crashed || (steps == 1 && self.should_refresh());
        let res = self.refresh_points(ground, refresh);
        if res {
            self.o.points_touching = self.o.touching > 0;
        }
        let w_world = self.o.world_spin;
        self.wheels_in_contact(center, w_world, lst, crashed, linear);
        // UpdateLocalCenterOfMass (Bike 0x40d540): the rider's weight
        // shift moves it along the bike.
        let mut local_com = COM;
        if !crashed {
            local_com.x = 0.0;
            local_com.z += (self.o.steer_value - 0.5) * 4.0;
            center = f.point(local_com);
        }
        self.o.pivot = local_com;
        self.o.front_lifted = wic > 0 && self.o.wheels[0].cp.penetration < -2.5;
        // ApplyDrag (0x506ce0).
        if prev_speed > 0.001 && linear != 0.0 {
            let drag = self.velocity * -(DRAG * linear);
            let length = drag.length();
            if length > 0.0 {
                let charge = linear.min(speed).max(0.0);
                let rate = mass / h;
                let limit = rate * charge;
                if length > limit {
                    force += drag * (limit / length);
                    speed -= charge;
                } else {
                    force += drag;
                    speed -= length / rate;
                }
            }
        }
        self.distribute(force, center, h);
        let alignment = saved.forward.dot(self.o.wheels[1].roll).abs();
        let now_air = !any && !self.o.points_touching;
        self.airborne_state(now_air, crashed, &saved);
        self.airborne = now_air;
        if self.airborne {
            self.o.landing_latched = false;
        }
        let contact_total = self.o.touching + wic;
        let mut lift = self.o.wheels[1].cp.normal;
        if contact_total > 0 {
            if any {
                if !crashed {
                    lift = self.ground_normal_and_turn(&saved, h, linear);
                }
                self.wheel_drive(c, linear, crashed);
            } else {
                self.o.wheel_point = false;
                self.o.turn_angle = 0.0;
                self.turn_rate = 0.0;
            }
            if self.o.points_touching {
                if !res {
                    let bars = self.bars(f.o);
                    for p in &mut self.o.points {
                        if p.in_contact {
                            p.world = if p.on_bars {
                                f.origin + f.o * HANDLEBARS + bars * p.local
                            } else {
                                f.point(p.local)
                            };
                        }
                    }
                }
                self.contact_states(center, w_world, linear);
            }
        } else {
            self.o.wheel_point = false;
            self.o.turn_angle = 0.0;
            self.turn_rate = 0.0;
        }
        if crashed {
            self.o.turn_angle = 0.0;
            self.turn_rate = 0.0;
        }

        // A bottomed shock, or the frame on the ground: the touching
        // points' merged impulse (0x43aa30, 0x5050b0).
        if self.o.wheel_point || (self.o.points_touching && (!any || crashed)) {
            let scale = if crashed { Vec3::ONE } else { Vec3::new(0.3, 0.1, 0.3) };
            let mut pts: Vec<Point> = self.o.wheels.iter().map(|w| w.cp).collect();
            pts.extend(self.o.points.iter().copied());
            let mut sum = Vec3::ZERO;
            let mut normal = Vec3::ZERO;
            let mut merged = 0;
            let mut deepest = 0.0f32;
            for (k, p) in pts.iter().enumerate() {
                if !p.in_contact {
                    continue;
                }
                if k == 0 || merged == 0 {
                    if merged == 0 {
                        deepest = p.penetration.max(0.0);
                        sum = p.world;
                        normal = p.normal;
                        merged = 1;
                        continue;
                    }
                }
                if p.penetration > 0.0 && p.penetration > deepest {
                    deepest = p.penetration;
                }
                merged += 1;
                sum += p.world;
                normal += p.normal;
            }
            if merged > 0 {
                if merged > 1 {
                    sum /= merged as f32;
                    normal = normal.normalize_or_zero();
                }
                let arm = sum - center;
                let rel = self.velocity + (w_world * scale).cross(arm);
                let push = normal * deepest;
                f.origin += push;
                center += push;
                let restitution = if crashed { 0.1 } else { RESTITUTION };
                self.impulse(restitution, inv_mass, &f, normal, rel, arm, inv_inertia, scale);
                linear = self.velocity.length();
            }
            if prev_speed * 1.3 < linear && linear > 5.0 {
                self.velocity *= prev_speed / linear;
                linear = prev_speed;
            }
            if prev_speed < 0.001 && linear < 0.1 {
                self.velocity = Vec3::ZERO;
                self.spin.y = 0.0;
                linear = 0.0;
            }
        }
        let turn_mag = self.o.turn_angle.abs();
        let mut local_torque = Vec3::ZERO;
        let mut world_torque = Vec3::ZERO;
        // AccumulateContactForces (0x506ed0): the frame's points slide.
        if self.o.touching > 0 {
            for p in &mut self.o.points {
                if p.state == 0 {
                    continue;
                }
                let share = force * p.load;
                let n = share.dot(p.normal);
                p.normal_load = if n >= 0.0 { 0.0 } else { -n };
                if p.tangent_speed <= 0.001 && p.spin_speed <= 0.001 {
                    p.friction_mag = 0.0;
                    p.friction_force = Vec3::ZERO;
                } else {
                    let m = -p.coefficient * p.grip * p.normal_load;
                    p.friction_force = p.friction_dir * m;
                    p.friction_mag = m.abs();
                }
                force += p.friction_force;
                local_torque += f.local(p.relative.cross(p.friction_force));
                if p.state == 2 {
                    local_torque *= torque_scale;
                    p.state = 0;
                }
            }
        }
        if any {
            self.drive_and_friction(&mut speed, &mut world_torque, &mut force, mass, crashed);
            if !crashed || !self.o.points_touching {
                for i in 0..2 {
                    if self.o.wheels[i].engaged {
                        let load = self.o.wheels[i].cp.load;
                        self.damp(i, load, suspension_mass);
                    }
                }
                self.shock_forces(&mut force, &mut local_torque, &f);
            }
            if !self.airborne {
                for i in 0..2 {
                    let w = self.o.wheels[i];
                    if w.in_contact && i == 1 {
                        force += w.drive_force;
                        let t = self.drive_torque(w.cp.relative, w.drive_force, linear, &saved);
                        local_torque += f.local(t);
                    }
                }
            }
        }

        if std::env::var_os("MM2_DBG").is_some() {
            let [fw, rw] = self.o.wheels;
            eprintln!("roll {:.3?} side {:.3?} fd {:.3?} fwd {:.3?} n {:.3?} t {:.3} v {:.2} F {:?} drive {:.1} share {:.3} slip {:.1} fN {:.1} rN {:.1} ff {:.1} rf {:.1} gear {} dis {} spd {:.2}", rw.roll, rw.side, rw.cp.friction_dir, self.frame().o.z_axis, rw.cp.normal, self.air_time, linear, force, rw.drive_mag, rw.drive_share, rw.slip_speed, fw.cp.normal_load, rw.cp.normal_load, fw.cp.friction_mag, rw.cp.friction_mag, self.drivetrain.gear, self.drivetrain.disengaged, speed);
        }
        // IntegrateMotion (KrustyBike 0x48f6a0, 0x506f30).
        let mut alpha = local_torque * inv_inertia;
        let friction_alpha = f.local(world_torque) * inv_inertia;
        settle(&mut self.spin, &mut alpha, friction_alpha, h);
        self.spin += alpha * h;
        self.spin *= SPIN_DECAY;
        let mut total = force;
        if !self.racing {
            total.x = 0.0;
            total.z = 0.0;
        }
        self.o.acceleration = total * inv_mass;
        self.velocity += self.o.acceleration * h;
        linear = self.velocity.length();
        let rise = (self.velocity.y - self.last_velocity.y) / h;
        let blend = h.min(LIFT_EASE) / LIFT_EASE;
        self.lift += (rise - self.lift) * blend;
        let slide = self.slide_angle(turn_mag, h, inv_inertia);
        let aim = self.rotation_pivot(slide, &f);
        let mode = self.classify(slide);
        // AdvancePosition (0x497490).
        f.origin += (self.velocity * 3.0 - self.last_velocity) * 0.5 * h;
        if !self.racing {
            self.o.wheels[1].drive_force = Vec3::ZERO;
            self.o.wheels[1].drive_mag = 0.0;
        }
        // RotateByAngularVelocity (0x52fd10).
        let angle = self.spin.length() * h;
        if angle.is_finite() && angle >= 1e-4 {
            let pivot = self.o.pivot;
            f.rotate_about(pivot, self.spin.normalize(), angle);
        }
        if crashed {
            // ApplyCrashedControl does nothing.
        } else if self.airborne {
            self.air_control(c, &mut f, &saved, h, linear);
        } else if mode != 0 {
            if (mode == 1 || mode == 2) && (turn_mag >= 1e-4 || slide.abs() > 1e-4) {
                let tilt = if turn_mag >= 1e-4 { lift } else { self.contact_normal() };
                let axis = f.local(tilt);
                let by = if slide != 0.0 && turn_mag < 0.003 { slide } else { slide + self.o.turn_angle };
                f.rotate_about(aim, axis, by);
                self.spin *= GROUND_SPIN;
            }
            if mode == 1 || mode == 3 {
                self.lean_correction(basis, lean_pivot, &mut f, &saved, h, alignment);
            }
        }
        self.set_frame(f);
        self.update_angles();
        // ClampSubStepsOnCrash (Bike 0x40aa30 → EvaluateCrash).
        let mut crashed_now = false;
        if !crashed {
            if let Some(why) = self.orig_crash(ground, prev_speed, linear, alignment, &saved, h) {
                if std::env::var_os("MM2_CRASHDBG").is_some() {
                    eprintln!("crash {:?} speed {:.1} prev {:.1} roll {:.2} fwd {:.2?} align {:.2} lc {:.2} rs {:.2}", why, linear, prev_speed, saved.roll, saved.forward, alignment, self.o.lean_cos, self.o.wheels[1].tangent_sin);
                }
                let time = if why == Crash::Edge { CRASH_TIME } else { self.crash_time };
                self.crash = Some((why, time));
                self.wreck = Some(self.throw(why));
                crashed_now = true;
            }
        }
        // ...or, on the frame's last step, SelectRiderPose (0x407440): the
        // cross-up air pose starts or ends.
        if !crashed && !crashed_now && steps == 1 && (self.fixed_step || self.pending <= 1e-4) {
            self.sync_tyres();
            self.pose(self.o.frame);
        }
        // ResolveSubstepCollisions (0x52fc60): the solid objects.
        let bounce = if self.crash.is_some() { 0.1 } else { RESTITUTION };
        let before = self.velocity.length();
        self.hit_objects(ground, mass, bounce);
        // The object's contact crashes the bike through the same speed
        // test (the collision's callback sets the crash; 0x52fc60 then
        // ends the frame's steps).
        if self.crash.is_none() && !crashed_now {
            if std::env::var_os("MM2_CRASHDBG").is_some() && self.velocity.length() < before - 5.0 {
                eprintln!("object hit {:.1} -> {:.1} prev {:.1}", before, self.velocity.length(), prev_speed);
            }
            let after = self.velocity.length();
            let step = if h < STEP { (STEP * 0.8).max(h) } else { h };
            let forgiven = if self.forgiving_landings && self.lands_level() { FORGIVING } else { 1.0 };
            if after < before && after - prev_speed < self.crash_tolerance * forgiven * -CRASH_DROP * step {
                self.crash = Some((Crash::Impact, self.crash_time));
                self.wreck = Some(self.throw(Crash::Impact));
                crashed_now = true;
            }
        }
        self.o.world_spin = self.orientation * self.spin;
        self.last_h = h;
        self.last_velocity = self.velocity;
        self.sync_tyres();
        if crashed {
            self.crash_step(ground, h);
            if self.crash.is_none() {
                // Put back on the course: the shocks start over.
                self.orig_start();
            }
        }
        crashed_now
    }

    /// PlaceWheels (0x52eea0): each shock springs back a quarter of the
    /// step, the tyre feels for the ground, and one still clear springs
    /// back the rest and feels again.
    fn place_wheels(&mut self, ground: &dyn Ground, f: &Frame, saved: &Angles, lst: f32) {
        for i in 0..2 {
            self.retract(i, lst * 0.25);
            self.probe(ground, i, f, saved);
            if !self.o.wheels[i].in_contact {
                self.retract(i, lst * 0.75);
                self.probe(ground, i, f, saved);
            } else {
                clear_forces(&mut self.o.shocks[i]);
            }
        }
        let mut n = 0;
        let mut unflagged = true;
        for i in [0usize, 1] {
            if self.o.wheels[i].in_contact {
                if i == 1 && unflagged {
                    self.o.wheels[i].report = true;
                    unflagged = false;
                }
                n += 1;
            }
        }
        if unflagged {
            self.o.wheels[0].report = true;
        }
        self.o.wheels_in_contact = n;
    }

    /// Shock::Retract (InlineShock 0x4ff100, RotatingShock 0x4ff950).
    fn retract(&mut self, i: usize, amount: f32) {
        let lean_cos = self.o.wheels[i].lean_cos;
        let s = &mut self.o.shocks[i];
        if s.ratio != 0.0 {
            let full = if i == 0 { min_length() } else { chord() };
            let by = amount * (full / (lean_cos * RETRACT));
            if !by.is_finite() || by < 0.0 {
                return;
            }
            if by >= s.length {
                *s = Shock::default();
                return;
            }
            s.length -= by;
            if i == 0 {
                s.ratio = s.length / full;
            } else {
                let delta = s.length / arm_length() - s.angle;
                if delta.is_finite() {
                    s.angle += delta;
                    s.ratio = s.length / full;
                }
            }
        }
        clear_forces(s);
    }

    /// Tire::UpdateSuspensionProbe (0x519c30): three looks at the ground
    /// under the rim, the deepest kept; then the solid objects.
    fn probe(&mut self, ground: &dyn Ground, i: usize, f: &Frame, saved: &Angles) {
        let center = self.wheel_center(i, f);
        let radius = TYRE_RADII[i];
        let (forward, mut side) = if i == 0 {
            let b = self.bars(f.o);
            let fwd = b.z_axis;
            (fwd, fwd.cross(b.y_axis).normalize_or_zero())
        } else {
            (saved.forward, saved.forward.cross(saved.up).normalize_or_zero())
        };
        let (mut surface, n0) = query(ground, center);
        let mut normal = n0;
        let mut roll = norm_or(n0.cross(side), forward);
        let mut lateral = roll.cross(side);
        if lateral.dot(n0) > 0.0 {
            lateral = -lateral;
            side = -side;
        }
        let first = center + lateral * radius;
        let mut world = first;
        let (s1, n1) = query(ground, first);
        surface = s1;
        let mut pen = (s1.y - first.y) * n1.y;
        let pt = if saved.pitch.abs() != std::f32::consts::FRAC_PI_2 {
            let reach = saved.sin_roll * radius;
            if i == 0 {
                let base = roll * radius + center;
                let rim = roll.cross(Vec3::Y).normalize_or_zero();
                rim * reach + base
            } else {
                let rotated = Vec3::new(saved.yaw.sin(), 0.0, saved.yaw.cos());
                let base = rotated * -radius + center;
                let rim = rotated.cross(Vec3::Y).normalize_or_zero();
                rim * reach + base
            }
        } else {
            saved.up * -radius + center
        };
        let (_, n2) = query(ground, pt);
        let probe_roll = norm_or(n2.cross(side), forward);
        let mut lateral2 = probe_roll.cross(side);
        if lateral2.dot(n2) > 0.0 {
            lateral2 = -lateral2;
        }
        let rim_point = center + lateral2 * radius;
        let (s3, n3) = query(ground, rim_point);
        let delta = s3.y - rim_point.y;
        let score = delta * n3.y;
        if score > pen {
            lateral = lateral2;
            world = rim_point;
            pen = score;
            surface = s3;
            if n2 != n3 {
                normal = n3;
                roll = norm_or(n3.cross(side), forward);
            } else {
                normal = n2;
                pen = delta * n2.y;
                roll = probe_roll;
            }
        } else if n0 != n1 {
            normal = n1;
            roll = norm_or(n1.cross(side), forward);
        } else {
            pen = (surface.y - world.y) * normal.y;
        }
        // A solid object under the tyre. The tyre's own collision shape
        // (Tire slot 8, 0x435830) is eight spokes from the axle to a
        // half circle of the radius in the wheel's plane, from straight
        // ahead through straight down (along `lateral`) to straight
        // behind. A spoke cut at fraction t of its length presses the
        // tyre (1 - t) * radius into the object, along the face normal.
        let mut on_object = false;
        let down = norm_or(lateral - forward * lateral.dot(forward), lateral);
        let ahead = norm_or(forward, forward);
        for k in 0..8 {
            let angle = k as f32 * 0.142857149 * std::f32::consts::PI + std::f32::consts::FRAC_PI_2;
            let rim = center + (down * -angle.cos() + ahead * angle.sin()) * radius;
            if let Some(cr) = ground.crossing(center, rim) {
                let pressed = (1.0 - cr.along) * radius;
                if pressed > pen {
                    pen = pressed;
                    surface = cr.point;
                    normal = cr.normal.normalize_or(Vec3::Y);
                    world = rim;
                    roll = norm_or(normal.cross(side), forward);
                    on_object = true;
                }
            }
        }
        if pen < 0.0 && pen > -0.01 {
            pen = 0.0;
        }
        let grip = ground.friction(world);
        let scale_b = ground.traction(world);
        let w = &mut self.o.wheels[i];
        w.center = center;
        w.forward = forward;
        w.side = side;
        w.roll = roll;
        w.cp.world = world;
        w.cp.surface = surface;
        w.cp.normal = normal;
        w.cp.penetration = pen;
        w.in_contact = pen >= 0.0;
        w.cp.grip = grip;
        w.scale_b = scale_b;
        w.report = false;
        w.engaged = false;
        w.cp.in_contact = false;
        w.cp.tangent_speed = 0.0;
        w.cp.spin_speed = 0.0;
        w.on_object = on_object;
    }

    /// Vehicle::UpdateLeanFromNormal (0x5286a0): the lean against the
    /// wheels' ground.
    fn lean_from_normal(&mut self, forward: Vec3, f: &Frame, saved: &Angles) {
        let [a, b] = self.o.wheels;
        let normal = ((a.cp.normal + b.cp.normal) * 0.5).normalize_or(Vec3::Y);
        let perp = normal - forward * normal.dot(forward);
        if perp != Vec3::ZERO {
            let perp = perp.normalize();
            let side_axis = f.o.z_axis.cross(f.o.y_axis);
            let side = side_axis.cross(forward).normalize_or_zero();
            let x = perp.cross(side);
            let s = if forward.dot(x) < 0.0 { 1.0 } else { -1.0 };
            let len = x.length().min(1.0);
            self.o.lean_angle = (len * s).asin();
            self.o.lean_cos = self.o.lean_angle.cos();
        } else {
            self.o.lean_angle = saved.roll;
            self.o.lean_cos = saved.cos_roll;
        }
        let lc = self.o.lean_cos;
        for w in &mut self.o.wheels {
            w.lean_cos = lc;
        }
    }

    /// UpdateWheelsInContact (0x52f090): each touching tyre's slip, then
    /// its shock against the ground.
    fn wheels_in_contact(&mut self, center: Vec3, w_world: Vec3, lst: f32, crashed: bool, linear: f32) {
        self.o.wheel_point = false;
        let f = self.frame();
        for i in 0..2 {
            if self.o.wheels[i].in_contact {
                self.contact_patch(i, center, w_world, linear);
                self.update_shock(i, lst, crashed, &f);
                if self.o.wheels[i].cp.in_contact {
                    self.o.wheel_point = true;
                }
            } else {
                let s = &mut self.o.shocks[i];
                s.prev_length = s.cleared;
            }
        }
    }

    /// Tire::UpdateContactPatch (0x5135f0).
    fn contact_patch(&mut self, i: usize, center: Vec3, w: Vec3, speed: f32) {
        let v = self.velocity;
        let mut out_speed = None;
        let mut out_forward = None;
        {
            let t = &mut self.o.wheels[i];
            let n = t.cp.normal;
            t.cp.relative = t.cp.world - center;
            t.cp.rot_velocity = w.cross(t.cp.relative);
            t.lever = (n * w.dot(n)).cross(t.cp.relative);
            let mut done = false;
            if speed > 0.001 {
                let mut tangent = v - n * v.dot(n);
                if tangent != Vec3::ZERO {
                    tangent = tangent.normalize();
                }
                let d = v.dot(tangent);
                t.cp.tangent_speed = d;
                if d > 0.001 {
                    if t.report {
                        out_speed = Some(d);
                    }
                    t.slip_vector = tangent * d + t.lever;
                    let len = t.slip_vector.length();
                    t.cp.friction_dir = t.slip_vector * (1.0 / len);
                    t.cp.spin_speed = t.lever.length();
                    let rc = t.cp.friction_dir.dot(t.roll);
                    t.slip_speed = len * rc.abs();
                    if t.report {
                        out_forward = Some(rc >= 0.0);
                    }
                    done = true;
                }
            }
            if !done {
                t.cp.tangent_speed = 0.0;
                t.slip_vector = t.lever;
                let len = t.lever.length();
                t.cp.spin_speed = len;
                if t.report {
                    out_speed = Some(len);
                }
                if len > 0.001 {
                    t.cp.friction_dir = t.slip_vector / len;
                } else {
                    t.cp.friction_dir = Vec3::ZERO;
                    t.cp.spin_speed = 0.0;
                }
                let rc = t.cp.friction_dir.dot(t.roll);
                t.slip_speed = rc.abs() * t.cp.spin_speed;
                if t.report {
                    out_forward = Some(rc >= 0.0);
                }
            }
        }
        if let Some(s) = out_speed {
            self.o.forward_speed = s;
        }
        if let Some(fw) = out_forward {
            self.o.moving_forward = fw;
        }
    }

    /// Tire::UpdateShock (0x513c70) and the shock's solve against the
    /// ground (InlineShock 0x4ff1f0, RotatingShock 0x4ffa50).
    fn update_shock(&mut self, i: usize, dt: f32, crashed: bool, f: &Frame) {
        let w = self.o.wheels[i];
        let s = self.o.shocks[i];
        if !(w.in_contact && (w.cp.penetration != 0.0 || s.ratio != 0.0)) {
            self.o.wheels[i].cp.in_contact = false;
            return;
        }
        let depth = w.cp.penetration;
        let n = w.cp.normal;
        let v = self.velocity;
        let (load, active) = if i == 0 {
            let anchor = f.point(HANDLEBARS);
            let to = w.center - anchor;
            let mut dist = to.dot(n);
            if dist >= 0.0 {
                dist = to.dot(Vec3::new(-n.x, n.y, -n.z));
            }
            let (ratio, miss) = if dist < 0.0 { (depth / dist, false) } else { (0.0, true) };
            let len = to.length();
            let over = (fork_length() - len).max(0.0);
            let sh = &mut self.o.shocks[0];
            sh.length = over - len * ratio;
            sh.displacement = to * -(sh.length / len);
            let pv = w.cp.rot_velocity + v;
            sh.speed = project(pv, sh.displacement).length();
            let max = sh.speed * dt;
            clamp_and_step(sh, max, dt, min_length(), None);
            if !sh.at_limit && !miss {
                (0.0, false)
            } else {
                (depth - sh.step_displacement.dot(n), true)
            }
        } else {
            let anchor = f.point(SWINGARM_PIVOT);
            let to = w.center - anchor;
            let swing = w.side.cross(to);
            let mut dist = swing.dot(n);
            let sh = &mut self.o.shocks[1];
            let mut early = None;
            if dist >= 0.0 {
                if dist == 0.0 {
                    sh.speed = 0.0;
                    early = Some((depth, v.dot(n) < -1.0));
                } else {
                    dist = swing.dot(Vec3::new(-n.x, n.y, -n.z));
                }
            }
            if let Some(e) = early {
                e
            } else {
                let (ratio, miss) = if dist < 0.0 { (depth / dist, false) } else { (0.0, true) };
                let arm = arm_length();
                sh.length = sh.cleared - ratio * arm;
                sh.displacement = swing * (-sh.length / arm);
                sh.speed = project(v + w.cp.rot_velocity, sh.displacement).length();
                let max = sh.speed * dt;
                clamp_and_step(sh, max, dt, chord(), Some(arm));
                if !sh.at_limit && !miss {
                    (0.0, false)
                } else {
                    (depth - n.dot(sh.step_displacement), true)
                }
            }
        };
        let step = self.o.shocks[i].step_displacement;
        let t = &mut self.o.wheels[i];
        t.cp.in_contact = active || crashed;
        t.cp.penetration = load;
        t.cp.world += step;
        t.cp.surface = if active { t.cp.world + t.cp.normal * load } else { t.cp.world };
        t.engaged = true;
    }

    /// Bike::DistributeContactForce (0x406ee0): the weight split between
    /// what touches, the farther from the centre the less; and the
    /// rider's weight shift eases toward the lean.
    fn distribute(&mut self, force: Vec3, center: Vec3, h: f32) {
        let ease = |value: &mut f32, target: f32| {
            let blend = h.min(COM_EASE) / COM_EASE;
            *value += (target - *value) * blend;
        };
        if self.o.points_touching {
            // Every touching point, the bottomed tyres' among them.
            let mut d = [0.0f32; 5];
            let mut total = 0.0;
            let mut active = 0;
            let mut last = 0;
            for k in 0..5 {
                let p = if k < 2 { &mut self.o.wheels[k].cp } else { &mut self.o.points[k - 2] };
                if p.in_contact {
                    d[k] = center.distance(p.world);
                    total += d[k];
                    active += 1;
                    last = k;
                } else {
                    p.load = 0.0;
                    p.applied = Vec3::ZERO;
                }
            }
            for k in 0..5 {
                let p = if k < 2 { &mut self.o.wheels[k].cp } else { &mut self.o.points[k - 2] };
                if !p.in_contact {
                    continue;
                }
                if active == 1 {
                    if k == last {
                        p.load = 1.0;
                        p.applied = force;
                    }
                } else {
                    p.load = 1.0 - d[k] / total;
                    p.applied = force * p.load;
                }
            }
            return;
        }
        if self.stunting {
            ease(&mut self.o.steer_value, 0.5);
            for w in &mut self.o.wheels {
                w.cp.load = w.rest_load;
                w.cp.applied = force * w.cp.load;
            }
            return;
        }
        ease(&mut self.o.steer_value, self.rider.1 * 0.25 + 0.5);
        let [front, rear] = [self.o.wheels[0].in_contact, self.o.wheels[1].in_contact];
        match self.o.wheels_in_contact {
            2 => {
                let lf = center.distance(self.o.wheels[0].cp.world);
                let lr = center.distance(self.o.wheels[1].cp.world);
                let r = lf / (lr + lf);
                self.o.wheels[1].cp.load = r;
                self.o.wheels[1].cp.applied = force * r;
                self.o.wheels[0].cp.load = 1.0 - r;
                self.o.wheels[0].cp.applied = force * (1.0 - r);
            }
            1 => {
                let (on, off) = if front { (0, 1) } else { (1, 0) };
                let _ = rear;
                self.o.wheels[on].cp.load = 1.0;
                self.o.wheels[on].cp.applied = force;
                self.o.wheels[off].cp.load = 0.0;
                self.o.wheels[off].cp.applied = Vec3::ZERO;
            }
            _ => {
                for w in &mut self.o.wheels {
                    w.cp.load = 0.0;
                    w.cp.applied = Vec3::ZERO;
                }
            }
        }
    }

    /// Bike::UpdateAirborneState (0x40b570) and the Vehicle's.
    fn airborne_state(&mut self, now: bool, crashed: bool, saved: &Angles) {
        if !crashed {
            if self.airborne {
                if !now && !self.o.landing_latched {
                    self.o.wobble_time = 0.0;
                }
            } else if now {
                self.o.wobble_stage = 1;
                self.o.wobble_sign = sign(saved.roll);
                self.o.wobble_offset = self.spin.z;
            }
        }
        if crashed {
            self.o.landing_latched = false;
        } else if self.airborne && !now && !self.o.landing_latched {
            self.o.landing_latched = true;
        }
        if !self.airborne && now {
            self.air_time = 0.0;
        }
    }

    /// Bike::ComputeGroundNormalAndTurn (0x406bd0): the ground under the
    /// tyres, and the bars bending the velocity (DeflectVelocity 0x52d800).
    fn ground_normal_and_turn(&mut self, saved: &Angles, h: f32, linear: f32) -> Vec3 {
        let [front, rear] = self.o.wheels;
        let near = front.in_contact || (rear.in_contact && front.cp.penetration > -0.2) || self.o.front_lifted;
        if !near {
            self.o.turn_angle = 0.0;
            self.turn_rate = 0.0;
            return rear.cp.normal;
        }
        let mut out = front.cp.normal;
        if rear.cp.penetration > -0.2 {
            out = (out + rear.cp.normal).normalize_or_zero();
            let weight = (front.cp.grip + rear.cp.grip) * 0.5;
            let perp = front.roll.cross(out).normalize_or_zero();
            let scale = if self.o.front_lifted {
                1.0 / (saved.cos_pitch * 0.35).max(0.001)
            } else {
                1.0
            };
            self.o.turn_angle = self.deflect(front.side, perp, weight.min(1.0), scale, h, linear);
            self.turn_rate = self.o.turn_angle / h;
            return out;
        }
        self.o.turn_angle = 0.0;
        self.turn_rate = 0.0;
        out
    }

    /// Vehicle::DeflectVelocity (0x52d800): the velocity turned onto the
    /// bars' circle, returning the angle.
    fn deflect(&mut self, point: Vec3, dir: Vec3, c: f32, d: f32, h: f32, linear: f32) -> f32 {
        let a = self.o.angle;
        let radius = if a.abs() == 0.0 { 0.0 } else { wheel_base() / a.sin() };
        let k = radius * point.dot(dir) * d;
        if k.abs() > 0.0 {
            let m = self.o.forward_speed;
            let r = m * m / -k * h;
            let s = (c * 1.5).min(1.0);
            let nv = dir * r * s + self.velocity;
            let len = nv.length();
            if len.abs() >= 0.01 {
                let sg = if k < 0.0 { -1.0 } else { 1.0 };
                let angle = (nv.dot(self.velocity) / (len * linear)).acos().abs() * sg;
                if angle.is_finite() {
                    self.velocity = nv * (linear / len);
                    return if self.o.moving_forward { angle } else { -angle };
                }
            }
        }
        0.0
    }

    /// KrustyBike::UpdateWheelDriveForces (0x492b20) and the rear's
    /// wheelspin (Tire::UpdateDriveShare 0x513f90).
    fn wheel_drive(&mut self, c: Controls, linear: f32, crashed: bool) {
        if !self.o.wheels[1].in_contact {
            return;
        }
        if self.racing {
            let backing = self.reverse && self.paddle.0 > 0.0 && self.o.forward_speed < BACK_UP_SPEED;
            let ramp = if backing && (PUSH_START..PUSH_END).contains(&self.paddle.0) {
                (PUSH_END - self.paddle.0) * PUSH
            } else {
                0.0
            };
            let d = &self.drivetrain;
            let roll = self.o.wheels[1].roll;
            let (force, mag) = if d.disengaged {
                if backing { (roll * -ramp, ramp) } else { (Vec3::ZERO, 0.0) }
            } else {
                let p = d.power(d.rpm) * crate::drivetrain::RATIOS[d.gear] * 341.62;
                let mag = if linear < 1.01 { p } else { p / linear };
                (roll * mag, mag)
            };
            let w = &mut self.o.wheels[1];
            w.drive_force = force;
            w.drive_mag = mag;
            if !self.on_course {
                w.drive_force *= 0.5;
                w.drive_mag *= 0.5;
            }
        }
        let saved = self.o.saved;
        let accel = self.o.acceleration.length();
        let mass = (self.spec.weight + RIDER) * 0.031_055_9;
        let lean = self.rider.1;
        let w = &mut self.o.wheels[1];
        if w.slip_speed < SPIN_SPEED && !crashed {
            let boost = if saved.forward.y > 0.0 && saved.cos_pitch * 1.25 > 1.0 {
                saved.cos_pitch * 1.25
            } else {
                1.0
            };
            let grip = 1.05 - lean * 0.25;
            let excess = w.drive_mag / mass - accel;
            let share = if excess < 1.01 { 1.0 } else { (400.0 - excess.min(320.0)) * 0.0025 };
            let minimum = if w.slip_speed > SPIN_SPEED {
                1.0
            } else {
                1.0 - (SPIN_SPEED - w.slip_speed) / SPIN_SPEED
            };
            let share = (share * grip * boost).min(1.0).max(minimum);
            w.drive_share = share * w.scale_b;
            w.drive_force *= w.drive_share;
            w.complement = 1.0 - w.drive_share;
        } else {
            w.drive_share = 1.0;
            w.complement = 0.0;
        }
    }

    /// UpdateContactStates (0x506d40): the frame's touching points' slip
    /// (CollisionPoint 0x43a640).
    fn contact_states(&mut self, center: Vec3, w: Vec3, speed: f32) {
        let v = self.velocity;
        for p in &mut self.o.points {
            if p.penetration < 0.0 {
                p.state = 0;
                continue;
            }
            if p.state != 2 {
                p.state = 1;
            }
            let n = p.normal;
            p.relative = p.world - center;
            let spin = (n * n.dot(w)).cross(p.relative);
            let mut done = false;
            if speed > 0.001 {
                let mut tangent = v - n * v.dot(n);
                if tangent != Vec3::ZERO {
                    tangent = tangent.normalize();
                }
                let along = tangent.dot(v);
                p.tangent_speed = along;
                if along > 0.001 {
                    let d = spin + tangent * along;
                    p.friction_dir = d / d.length();
                    p.spin_speed = spin.length();
                    done = true;
                }
            }
            if !done {
                p.tangent_speed = 0.0;
                let len = spin.length();
                p.spin_speed = len;
                if len > 0.001 {
                    p.friction_dir = spin / len;
                } else {
                    p.friction_dir = Vec3::ZERO;
                    p.spin_speed = 0.0;
                }
            }
        }
    }

    /// ResolveContactImpulse (0x500220) for this body.
    #[allow(clippy::too_many_arguments)]
    fn impulse(&mut self, restitution: f32, inv_mass: f32, f: &Frame, normal: Vec3, rel: Vec3, arm: Vec3, inv_inertia: Vec3, scale: Vec3) {
        let approach = rel.dot(normal);
        if approach < 0.0 {
            let axis = arm.cross(normal);
            let scaled = f.local(axis) * inv_inertia;
            let c = f.world(scaled).cross(arm);
            let j = ((restitution + 1.0) * -approach) / (c.dot(normal) + inv_mass);
            self.velocity += normal * j * inv_mass;
            self.spin += scaled * j * scale;
        }
    }

    /// ApplyWheelDriveAndFriction (0x527a20): the brakes, then each tyre's
    /// rolling and side friction, within what is left of the speed.
    fn drive_and_friction(&mut self, speed: &mut f32, torque: &mut Vec3, force: &mut Vec3, mass: f32, crashed: bool) {
        let h = self.h;
        let driven = self.o.wheels.iter().filter(|w| w.in_contact).count();
        if !crashed && driven > 0 {
            let share = 1.0 / driven as f32;
            let forward = self.o.moving_forward;
            for i in 0..2 {
                let w = self.o.wheels[i];
                if !w.in_contact || w.ramp == 0.0 || w.slip_speed == 0.0 {
                    continue;
                }
                // Tire::ApplyDrive (0x514170).
                let grip = w.cp.grip * BRAKE * w.ramp;
                let push = w.roll * -grip;
                let dir = if forward { push } else { -push };
                let amount = (share * w.slip_speed).min(*speed).max(0.0);
                let rate = mass / h;
                let needed = rate * amount;
                let push = if grip > needed {
                    *speed -= amount;
                    dir * (needed / grip)
                } else {
                    *speed -= grip / rate;
                    dir
                };
                *force += push;
                *torque += w.cp.relative.cross(push) * BRAKE_TORQUE[i];
            }
        }
        for i in 0..2 {
            let w = &mut self.o.wheels[i];
            if !w.in_contact {
                continue;
            }
            let load = w.cp.applied.dot(w.cp.normal);
            w.cp.normal_load = if load >= 0.0 { 0.0 } else { -load };
            // Tire's friction (0x51b1d0).
            if w.cp.tangent_speed <= 0.001 && w.cp.spin_speed <= 0.001 {
                w.cp.friction_mag = 0.0;
                w.cp.friction_force = Vec3::ZERO;
            } else {
                let rc = w.cp.friction_dir.dot(w.roll).abs().min(1.0);
                w.tangent_cos = rc;
                w.tangent_sin = (1.0 - rc * rc).sqrt();
                let n = w.cp.normal_load;
                let m = (-ROLLING * w.tangent_cos * n - 2.0 * (SIDE[i] / 2.0) * w.tangent_sin * n) * w.cp.grip;
                w.cp.friction_force = w.cp.friction_dir * m;
                w.cp.friction_mag = m.abs();
            }
            let budget = w.cp.spin_speed + w.cp.tangent_speed;
            let scale = w.cp.friction_mag;
            if scale > 0.0 && budget != 0.0 {
                let taken = speed.min(budget).max(0.0);
                let rate = mass / h;
                let impulse = if scale > rate * taken {
                    *speed -= taken;
                    w.cp.friction_force * (rate * taken / scale)
                } else {
                    *speed -= scale / rate;
                    w.cp.friction_force
                };
                *force += impulse;
                *torque += w.cp.relative.cross(impulse);
            }
        }
    }

    /// Shock::ApplyDamping (0x4feb20).
    fn damp(&mut self, i: usize, load: f32, suspension_mass: f32) {
        let s = &mut self.o.shocks[i];
        let (soft, stiff) = SPRING[i];
        let k = soft + (stiff - soft) * s.ratio;
        let m = load * suspension_mass;
        let len = s.velocity.length();
        let mut coeff = (m * k).max(0.0).sqrt() * DAMPING * -2.0;
        if len > SHOCK_KNEE {
            coeff *= ((len - SHOCK_KNEE) * 0.5 + SHOCK_KNEE) / len;
        }
        s.damper = s.velocity * coeff;
        s.spring = s.displacement * k + s.damper;
    }

    /// Vehicle::ApplyShockForces (0x52eac0): the springs up the ground's
    /// normal, less leaned; only their pitch turns the bike; and the
    /// pump's kick, once.
    fn shock_forces(&mut self, force: &mut Vec3, local_torque: &mut Vec3, f: &Frame) {
        let lean_cos = self.o.lean_cos;
        for i in 0..2 {
            let w = self.o.wheels[i];
            if !w.engaged || w.cp.in_contact {
                continue;
            }
            let s = &mut self.o.shocks[i];
            let spring = project(s.spring, w.cp.normal) * lean_cos;
            if s.impulse != 0.0 {
                let kick = project(s.displacement * s.impulse, Vec3::Y) * lean_cos;
                *force += kick;
                s.impulse = 0.0;
            }
            self.o.wheels[i].cp.applied += spring;
            *force += spring;
            let mut t = f.local(w.cp.relative.cross(spring));
            t.y = 0.0;
            t.z = 0.0;
            *local_torque += t;
        }
    }

    /// Bike::ComputeDriveTorque (0x407660): the drive lifts the nose only
    /// with the rider back.
    fn drive_torque(&mut self, arm: Vec3, force: Vec3, linear: f32, saved: &Angles) -> Vec3 {
        let v = arm.cross(force);
        if linear < 10.0 && saved.pitch > 0.8727 && self.o.wheels[0].in_contact {
            if self.spin.x < 0.0 {
                self.spin.x = 0.0;
            }
            Vec3::ZERO
        } else if self.spin.x < -1.0 {
            v * 0.2
        } else {
            let lean = saved.pitch.abs().min(1.0);
            let k = (-(self.rider.1 + 0.125)).max(0.0);
            v * ((1.4 - lean) * k * 1.6)
        }
    }

    /// Bike::ComputeSlideAngle (0x40ce00): the rear stepping out, and a
    /// power slide.
    fn slide_angle(&mut self, threshold: f32, h: f32, inv_inertia: Vec3) -> f32 {
        let mut result = 0.0;
        let all = self.o.wheels_in_contact == 2;
        let [front, rear] = self.o.wheels;
        if all && rear.cp.tangent_speed > 7.0 && rear.tangent_sin >= 1e-4 && front.tangent_sin < 0.5 {
            // The roll across the slip (the game writes this one out by hand).
            let x = rear.roll.cross(rear.cp.friction_dir);
            let len2 = x.length_squared();
            if len2 != 1.0 {
                let len = len2.sqrt();
                if len < 0.4795 {
                    let a = len.asin();
                    if a.is_finite() {
                        let s = if x.y < 0.0 { -1.0 } else { 1.0 };
                        result = s * a.min(h * 0.8);
                        if threshold > result.abs() {
                            result = 0.0;
                        }
                    }
                }
            }
        }
        let (angle, step, left) = &mut self.slide;
        let mut decay = false;
        if all {
            if rear.drive_share < 0.9 && rear.drive_mag > 0.0 {
                let half = wheel_base() * 0.5;
                let mut s = (half + half * rear.complement) * inv_inertia.y * h * rear.complement * rear.drive_mag;
                let lean = self.o.lean_angle.abs();
                if lean > 0.005 {
                    s = (s * 32.0).min(1.57);
                    let lf = (lean * 5.0).min(1.5);
                    s = s * h * lf;
                    *angle = if self.o.lean_angle < 0.0 { -s } else { s };
                    *step = STEP * -1.333 * *angle;
                    *left = (0.75 / STEP - 0.5).round() as i32;
                } else {
                    *angle = 0.0;
                }
            } else if *angle != 0.0 {
                decay = true;
            }
        } else if self.o.wheels_in_contact == 0 || *angle == 0.0 {
            *angle = 0.0;
        } else {
            decay = true;
        }
        if decay {
            let old = *angle;
            *angle += *step;
            *left -= 1;
            if old < 0.0 {
                if *angle > 0.0 || *left < 0 {
                    *angle = 0.0;
                }
            } else if *angle < 0.0 || *left < 0 {
                *angle = 0.0;
            }
        }
        if *angle != 0.0 { *angle } else { result }
    }

    /// Bike::ComputeRotationPivot (0x40cba0), local.
    fn rotation_pivot(&self, slide: f32, f: &Frame) -> Vec3 {
        if self.airborne || self.o.points_touching {
            return self.o.pivot;
        }
        let [front, rear] = self.o.wheels;
        let (fp, rp) = (front.cp.world, rear.cp.world);
        let at = if self.o.wheels_in_contact == 2 {
            if slide != 0.0 {
                fp
            } else if rear.drive_share < 0.9 {
                let bias = (rear.drive_share - 0.4).max(0.0);
                (rp - fp) * bias + fp
            } else {
                (rp - fp) * 0.5 + fp
            }
        } else {
            fp + (rp - fp) * 0.5
        };
        f.local(at - f.origin)
    }

    /// Bike::ClassifyGroundContact (0x40d200).
    fn classify(&self, slide: f32) -> u8 {
        if self.o.front_lifted {
            return 2;
        }
        let [front, rear] = self.o.wheels;
        if self.o.wheels_in_contact > 0 && self.crash.is_none() {
            if self.o.wheels_in_contact == 2 || self.turn_rate != 0.0 || slide != 0.0 {
                return 1;
            }
            if rear.in_contact {
                if front.cp.penetration > -5.2 {
                    return 3;
                }
            } else if front.in_contact && rear.cp.penetration > -5.2 {
                return 1;
            }
        }
        0
    }

    /// Bike::GetContactGroundNormal (0x40b170).
    fn contact_normal(&self) -> Vec3 {
        let [front, rear] = self.o.wheels;
        if self.o.wheels_in_contact == 0 {
            return rear.cp.normal;
        }
        if front.in_contact {
            if rear.in_contact {
                return ((front.cp.normal + rear.cp.normal) * 0.5).normalize_or_zero();
            }
            return front.cp.normal;
        }
        rear.cp.normal
    }

    /// Bike::GetLeanAngle (0x40c650).
    fn lean_angle(&self, saved: &Angles) -> f32 {
        if self.crash.is_none() && self.o.prev_speed < LEAN_SPEED {
            return saved.roll;
        }
        let bars = self.o.angle.abs();
        let mut lean = self.o.lean_angle;
        if bars < 0.523_598_8 {
            lean -= (self.o.lean_angle - saved.roll) * ((0.523_598_8 - bars) * 1.909_859_3);
        }
        lean
    }

    /// Bike::ApplyLeanCorrection (0x40c830): leans the bike toward what
    /// the turn wants, no faster than its rate.
    #[allow(clippy::too_many_arguments)]
    fn lean_correction(&mut self, basis: Vec3, point: Vec3, f: &mut Frame, saved: &Angles, h: f32, alignment: f32) {
        let mut target = (self.o.angle * 0.05 + self.turn_rate * 0.2) * alignment;
        let mag = target.abs();
        let (slide, _, _) = self.slide;
        if slide != 0.0 {
            let lim = sign(slide) * 0.087;
            if mag < lim.abs() {
                target = lim;
            }
        }
        if mag > LEAN_LIMIT {
            target = if target < 0.0 { -LEAN_LIMIT } else { LEAN_LIMIT };
        }
        let mut err = self.lean_angle(saved) - target;
        if err.is_finite() && err.abs() >= 1e-4 {
            let err_sign = sign(err);
            let roll_sign = sign(saved.roll);
            let mut lim = LEAN_RATE;
            if err_sign == roll_sign {
                lim *= 0.8;
            }
            if err.abs() / h > lim {
                err = sign(err) * h * lim;
            }
            if self.o.wheels_in_contact > 0 {
                let axis = f.local(basis);
                if axis.abs().max_element() > 0.001 {
                    f.rotate_about(point, axis, err);
                    let s = axis * err / h;
                    for k in 0..3 {
                        let d = s[k];
                        let v = &mut self.spin[k];
                        if d > 0.0 {
                            if *v < 0.0 {
                                *v = (d * 1.5 + *v).min(0.0);
                            }
                        } else if *v > 0.0 {
                            *v = (d * 1.5 + *v).max(0.0);
                        }
                    }
                }
            }
        }
    }

    /// Bike::ApplyAirControl (0x40bcc0).
    fn air_control(&mut self, c: Controls, f: &mut Frame, saved: &Angles, h: f32, mut linear: f32) {
        self.air_time += h;
        let dt = h * AIR_PITCH;
        let lean_force = self.sliders.pitch * self.rider.1 * dt;
        let mut force = if self.stunting {
            Vec3::new(lean_force * 0.5, 0.0, 0.0)
        } else {
            Vec3::new(lean_force, self.rider.0 * h * AIR_YAW, 0.0)
        };
        if self.air_pose && self.o.wheels[1].cp.penetration < -2.0 {
            let rear = self.o.wheels[1];
            if self.brake_gyro && rear.ramp > 0.0 {
                force.x += dt * rear.ramp;
            }
            if self.gas_gyro && c.throttle > 0.0 && self.drivetrain.throttle > 0.0 && !self.drivetrain.disengaged {
                force.x -= dt * self.drivetrain.throttle;
            }
            if linear > 0.0 {
                let heading = self.velocity / linear;
                let dot = saved.forward.dot(heading);
                if dot > 0.55 && dot < 0.984 {
                    let across = Vec3::new(heading.z, 0.0, -heading.x);
                    let s = if self.pose_phase < 0.5 { 1.0 } else { -1.0 };
                    let push = s * ((1.0 - dot) * 3.412_969_1 * linear * 0.075) * h;
                    self.velocity += across * push;
                    self.velocity *= linear / (push + linear);
                    linear = self.velocity.length();
                }
            }
        }
        let _ = linear;
        let mag = force.length();
        if mag.is_finite() && mag >= 1e-4 {
            let pivot = self.o.pivot;
            f.rotate_about(pivot, force / mag, mag);
        }
        self.spin *= AIR_SPIN;
        // The roll rights itself, wobbling (wobbleTime, +0x6e8..).
        let elapsed = self.o.wobble_time + h;
        self.o.wobble_time = elapsed;
        let stage = self.o.wobble_stage;
        if stage > 0 && stage <= 2 {
            let roll = saved.roll;
            let sg = sign(roll);
            let mut rate = (roll / h).abs();
            let cube = elapsed * elapsed * elapsed * 0.357_792_47;
            if cube < rate {
                rate = cube;
            }
            self.spin.z = sg * rate + self.o.wobble_offset;
            self.o.wobble_offset *= 0.9;
            if sg != self.o.wobble_sign {
                let mut next = stage + 1;
                if next > 2 {
                    next = 1;
                }
                self.o.wobble_stage = next;
                self.o.wobble_sign = sg;
                self.o.wobble_offset = self.spin.z;
                self.o.wobble_time = elapsed * 0.5;
            }
        }
    }

    /// Bike::EvaluateCrash (0x40a950) with KrustyBike's (0x492650).
    fn orig_crash(&mut self, ground: &dyn Ground, prev_speed: f32, linear: f32, alignment: f32, saved: &Angles, h: f32) -> Option<Crash> {
        if self.off_the_edge(ground) {
            return Some(Crash::Edge);
        }
        // The rider's probes (the Wrecker's bones): those under the
        // ground, their middle in the rider's frame; above 1 ft is a crash
        // (reason 11). Without a posed rider, the centred riding pose.
        {
            let place = self.place();
            let probes: &[Vec3] = if self.rider_probes.is_empty() { &RIDING_PROBES } else { &self.rider_probes };
            let (mut sum, mut n) = (Vec3::ZERO, 0.0);
            for p in probes {
                let w = place.transform_point3(*p);
                if w.y < ground.terrain(w.x, w.z).0 {
                    sum += *p;
                    n += 1.0;
                }
            }
            if n > 0.0 && sum.y / n > 1.0 {
                if std::env::var_os("MM2_CRASHDBG").is_some() {
                    eprintln!("probe crash: {} probes, middle {:?}", n, sum / n);
                }
                return Some(Crash::Hit);
            }
        }
        let [front, rear] = self.o.wheels;
        if (self.airborne || !self.air_pose)
            && front.in_contact
            && !rear.in_contact
            && alignment < if std::env::var_os("MM2_ENDO766").is_some() { CRASH_ENDO } else { NOSE_OVER }
            && self.spin.x > 0.0
        {
            return Some(Crash::Endo);
        }
        if saved.forward.y.abs() > SIDE_LIE {
            return Some(Crash::Flipped);
        }
        if saved.roll.abs() > CRASH_ROLL {
            return Some(Crash::Flipped);
        }
        if rear.tangent_sin > CRASH_LOWSIDE && self.o.lean_cos < CRASH_LOWSIDE {
            return Some(Crash::Lowside);
        }
        let step = if h < STEP { (STEP * 0.8).max(h) } else { h };
        let forgiven = if self.forgiving_landings && self.lands_level() { FORGIVING } else { 1.0 };
        if linear - prev_speed < self.crash_tolerance * forgiven * -CRASH_DROP * step {
            return Some(Crash::Impact);
        }
        None
    }

    /// The older model's tyres, from this one's wheels: what the rest of
    /// the engine reads (dust, sound, the rider's poses, drawing).
    fn sync_tyres(&mut self) {
        for i in 0..2 {
            let w = self.o.wheels[i];
            let s = self.o.shocks[i];
            let t = &mut self.tyres[i];
            t.squeeze = if i == 0 {
                if s.at_limit { min_length() } else { s.length }
            } else {
                s.angle * arm_length()
            }
            .clamp(0.0, t.travel);
            t.felt = t.squeeze;
            t.contact = w.cp.world;
            t.normal = w.cp.normal;
            t.forward = w.roll;
            t.axle = w.side;
            t.depth = w.cp.penetration;
            t.down = w.in_contact;
            t.load = w.cp.load;
            t.normal_force = w.cp.normal_load;
            t.slip = w.tangent_sin;
            t.moving = w.cp.friction_dir;
            t.on_object = w.on_object;
            t.bottomed = if w.cp.in_contact { w.cp.penetration.max(0.0) } else { 0.0 };
        }
        self.steer = self.o.angle;
        self.brake = self.o.wheels[1].ramp;
        self.traction = self.o.wheels[1].drive_share;
        self.drive = self.o.wheels[1].drive_mag;
        self.accel = self.o.acceleration.length();
        self.last_lean = self.rider.1;
    }
}

/// Shock::ClearForces (0x4fe9e0).
fn clear_forces(s: &mut Shock) {
    s.at_limit = false;
    s.spring = Vec3::ZERO;
    s.damper = Vec3::ZERO;
    s.cleared = s.length;
    s.extending = false;
    s.speed = 0.0;
    s.displacement = Vec3::ZERO;
    s.step_displacement = Vec3::ZERO;
    s.velocity = Vec3::ZERO;
}

/// The shock's end stop and its rate over the step (InlineShock 0x4fee80,
/// RotatingShock 0x4ff790); `arm` the swingarm's length.
fn clamp_and_step(s: &mut Shock, max: f32, dt: f32, full: f32, arm: Option<f32>) {
    if s.length >= full {
        s.at_limit = true;
        s.ratio = 1.0;
        s.displacement *= full / s.length;
        s.length = full;
        if arm.is_some() {
            s.angle = SWING_MAX;
        }
    } else {
        s.at_limit = false;
        s.ratio = s.length / full;
        if let Some(a) = arm {
            s.angle = s.length / a;
        }
    }
    let delta = s.prev_length - s.length;
    if s.length != 0.0 {
        let c = if delta >= 0.0 { delta.min(max) } else { delta.max(-max) };
        s.velocity = s.displacement * (c / (dt * s.length));
        s.step_displacement = s.displacement * -(delta / s.length);
    } else {
        s.velocity = Vec3::ZERO;
        s.step_displacement = Vec3::ZERO;
    }
    s.extending = s.length > s.prev_length;
    s.prev_length = s.length;
}
