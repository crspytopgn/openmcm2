//! The rider thrown off in a crash, after the game's Wrecker (wrecker.cpp,
//! update 0x535d50).
//!
//! The game does not use a jointed ragdoll. The rider is one rigid body
//! that plays a crash motion from Rider.mcf while it tumbles:
//!
//! * A crash on the ground starts a **bail** (0x537890): a motion chosen by
//!   the kind of crash (Endo, Kahuna, BackOver, LeftOver, RightOver, list
//!   +0x48) plays with the rider still on the bike, and once it is half
//!   done (0x535d50, state 1) the rider is let go (0x537b00) with the
//!   speed and spin the motion gave the pelvis over the last frame
//!   (0x537bf0), and one of Fall01 to Fall04 fades in over 0.5 s.
//! * A crash in the air **tumbles** at once (0x537980): one of Fall01 to
//!   Fall04 at random (list +0x50), blended in over 0.5 s, the body thrown
//!   with the bike's speed and spin.
//! * When the body first lands it picks one of LeftHit, RightHit,
//!   FeetHitL, FeetHitR or HeadHit at random (list +0x58, kept in +0xd4)
//!   and fades it in over 0.5 s once the fall's own fade is done (the
//!   motion player's +0x34), then slides to a stop; then the rider is put
//!   back on the bike.
//!
//! The body is the rider's pelvis (the Wrecker's +0xb8, the first frame
//! under the model's root, whose world placement +0x6c the 2026-10-24
//! recordings follow). The motion poses the rest of the rider round it,
//! so a motion change or fade moves the limbs but never jumps the body
//! (0x535d50 re-roots the model at the end of each fade to keep the
//! pelvis where it was).
//!
//! The body touches with Rider.col's six part shapes (loaded in
//! 0x535830), which move with the motion playing; the viewer hands their
//! posed corners over. The deepest corner in the ground is answered by the
//! rider's collision model (0x43cd50): no bounce (+0xb8 = 0), only 5 %
//! of the impulse that would stop it (1 − +0xf4, 0.95), at most 2000, and
//! the body's speed and spin cut by 0.99 × the step's time (+0xf0). The
//! body's inverse inertia is zeroed (0x4a2950 into +0x1d0), so contacts
//! never turn it; the fall motions do the tumbling. Until the corners are
//! set the body touches as one sphere. 0x538050
//! also keeps the parts out of the bike: a box in the bike's model space
//! (centre (0, 2, 0), half size (0.5, 1.5, 3) ft, set in 0x4924e6) pushes
//! any part inside it out at 5 × its depth per second. 0x537690 only
//! throws dirt where the parts slide.

use glam::{Mat4, Quat, Vec3};

use crate::Ground;

/// The rider's centre in the shared bike and rider model space (Rider02
/// bounds).
pub const RIDER_CENTRE: Vec3 = Vec3::new(0.0, 3.0, -0.6);
const RADIUS: f32 = 0.9;
const GRAVITY: f32 = 32.2;
const BOUNCE: f32 = 0.3;
const FRICTION: f32 = 0.8;
/// How long a bail holds the rider on the bike, s: half its motion
/// (Rider.mcf: frames at 15 a second), or this for a motion not listed.
const BAIL_TIME: f32 = 0.5;
const BAILS: [(&str, f32); 5] = [("Endo", 12.0), ("LeftOver", 10.0), ("RightOver", 10.0), ("BackOver", 13.0), ("Kahuna", 25.0)];
const BAIL_FPS: f32 = 15.0;
/// How long a fall or a landing motion fades in, s (0x4aa1e0's span).
pub const FADE: f32 = 0.5;
/// The rider is at rest below this speed, ft/s, for this long, s.
const REST_SPEED: f32 = 2.0;
const REST_TIME: f32 = 0.75;
/// The longest a wreck lasts, s.
const LONGEST: f32 = 6.0;

/// The bike's box the thrown rider is kept out of (0x538050), and how hard.
const BIKE_BOX_CENTRE: Vec3 = Vec3::new(0.0, 2.0, 0.0);
const BIKE_BOX_HALF: Vec3 = Vec3::new(0.5, 1.5, 3.0);
const BIKE_BOX_PUSH: f32 = 5.0;

/// The rider's collision model (0x43c870, set up in 0x535830): the
/// share of a contact's stopping impulse it drops (+0xf4), the most one
/// impulse gives (+0xf8), the cut to speed and spin per second of each
/// contact (+0xf0), and the body's mass (its vtable +0x74).
const IMPULSE_DROPPED: f32 = 0.95;
const IMPULSE_MOST: f32 = 2000.0;
const DAMPING: f32 = 0.99;
const MASS: f32 = 200.0;
/// How far a solid object is felt round each corner, ft [guess].
const CORNER_RADIUS: f32 = 0.05;

/// Crash motions in Rider.mcf.
pub const FALLS: [&str; 4] = ["Fall01", "Fall02", "Fall03", "Fall04"];
pub const HITS: [&str; 5] = ["LeftHit", "RightHit", "FeetHitL", "FeetHitR", "HeadHit"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Still on the bike, playing the bail motion.
    Bail,
    /// Thrown, in the air.
    Tumble,
    /// Down on the ground, sliding to a stop.
    Down,
}

/// The rider body's rigid-body state (PhysicsRigidBody, step 0x4cc630):
/// the centre of mass's velocity, the turn rate in the model's own axes,
/// and the two-step Adams-Bashforth integrator's last derivatives
/// (0x4a1d10: `x += (3·x'ₙ − x'ₙ₋₁)·h/2`), cleared by ResetState (0x4cc500)
/// when the rider is let go.
#[derive(Debug, Clone, Copy, Default)]
struct History {
    position: Vec3,
    velocity: Vec3,
    turn: glam::Vec4,
    spin: Vec3,
}

#[derive(Debug, Clone)]
pub struct Wreck {
    /// The Rider.mcf motion to play now.
    pub motion: &'static str,
    pub stage: Stage,
    /// The rider body's centre (the pelvis) and its centre of mass's
    /// velocity.
    pub position: Vec3,
    pub velocity: Vec3,
    /// The rigid body's node: the rider model's root (Wrecker::CreateBodies
    /// sets the body on the character's model node; the pelvis, its
    /// first child, is posed under it by the motion).
    root: Vec3,
    root_turn: Quat,
    /// Turn rate, rad/s, in the root's axes (PhysicsBody +0x160).
    spin: Vec3,
    /// The centre of mass and the inertia's diagonal, in the root's
    /// space, from the posed rider's bounds (Wrecker::UpdateInertia,
    /// 0x5377c0: half sizes h, I = m/12 · (h_y² + h_z², h_x² + h_z²,
    /// h_x² + h_y²)).
    mass_centre: Vec3,
    inertia: Vec3,
    history: History,
    time: f32,
    still: f32,
    seed: u32,
    /// The body touched the ground or an object this step (the Wrecker's
    /// +0xb0, from its collision model's +0x58), which the race sounds
    /// read for the rider's grunts.
    pub touching: bool,
    /// Rider.col's corners where the motion has them now, in the
    /// pelvis's own space; empty for one sphere.
    points: Vec<Vec3>,
    /// The Wrecker's 15 probes on the rider's bones (Bike::CreateWrecker,
    /// 0x40c0f0) where the motion has them now, in the pelvis's space:
    /// what touches the ground. Empty until the viewer poses them; then
    /// Rider.col's corners only meet the solid objects.
    probes: Vec<Vec3>,
    /// The pelvis in model space as the motion has it now.
    pelvis: Mat4,
    /// Whether the viewer has said where the motion puts the pelvis.
    posed: bool,
    /// How long the bail motion runs, s, and how many frames it has.
    bail_frames: f32,
    /// When the motion fading in now is fully in, s.
    faded: f32,
    /// A fade is running; at its end the root is moved so the pelvis
    /// stays put (0x535d50's carry of the rider pose, +0xb4).
    fading: bool,
    carry: bool,
    /// The landing motion waiting for that (+0xd4).
    queued: Option<&'static str>,
    /// The pelvis at the last pose, while the bail plays: when, and its
    /// world placement, for the speed and spin it lets go with.
    last: Option<(f32, Mat4)>,
    /// The root before the last step, to draw it between steps.
    previous: Option<(Vec3, Quat)>,
}

/// The default half sizes of the rider's bounds before the viewer poses
/// the rider, ft [guess: Rider02 at rest].
const REST_HALF: Vec3 = Vec3::new(1.2, 3.0, 1.5);

impl Wreck {
    /// A wreck starting from a bike placed at `bike` (model to world)
    /// moving at `velocity` and spinning at `spin` (world).
    pub fn new(
        bike: Mat4,
        velocity: Vec3,
        spin: Vec3,
        motion: &'static str,
        bail: bool,
        seed: u32,
    ) -> Self {
        let (_, root_turn, root) = bike.to_scale_rotation_translation();
        let root_turn = root_turn.normalize();
        let mut w = Wreck {
            motion,
            stage: if bail { Stage::Bail } else { Stage::Tumble },
            position: bike.transform_point3(RIDER_CENTRE),
            velocity,
            root,
            root_turn,
            spin: root_turn.inverse() * spin,
            mass_centre: RIDER_CENTRE,
            inertia: Vec3::ZERO,
            history: History::default(),
            time: 0.0,
            still: 0.0,
            seed,
            touching: false,
            points: Vec::new(),
            probes: Vec::new(),
            pelvis: Mat4::from_translation(RIDER_CENTRE),
            posed: false,
            bail_frames: BAILS.iter().find(|b| b.0 == motion).map_or(BAIL_TIME * 2.0 * BAIL_FPS, |b| b.1),
            faded: if bail { 0.0 } else { FADE },
            fading: !bail,
            carry: false,
            queued: None,
            last: None,
            previous: None,
        };
        w.update_inertia();
        w
    }

    fn root_place(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.root_turn, self.root)
    }

    /// The body: the pelvis on the track.
    pub fn body(&self) -> Mat4 {
        self.root_place() * self.pelvis
    }

    /// Where to draw the rider: the model origin and axes it shares with
    /// the bike.
    pub fn place(&self) -> Mat4 {
        self.root_place()
    }

    /// Where to draw the rider `t` (0 to 1) of the way from the step
    /// before to the last one (ours: the game draws once a step; drawn
    /// at the last step's place, the thrown rider jumped on in 50 steps
    /// a second while the bike and the screen ran smoothly).
    pub fn drawn_place(&self, t: f32) -> Mat4 {
        let Some((root, turn)) = self.previous else {
            return self.place();
        };
        if root.distance(self.root) > 20.0 {
            return self.place();
        }
        Mat4::from_rotation_translation(turn.slerp(self.root_turn, t), root.lerp(self.root, t))
    }

    /// Where the motion playing puts the pelvis (`pelvis`, model space)
    /// and Rider.col's corners (`points`, the pelvis's space), as the
    /// viewer poses them each frame. The model's root stays where it is
    /// and the pelvis moves with the motion, except when a fade has just
    /// finished: then the root is carried so the pelvis stays put
    /// (0x535d50).
    pub fn set_pose(&mut self, pelvis: Mat4, points: Vec<Vec3>) {
        let body = self.body();
        if self.stage == Stage::Bail && self.posed {
            self.last = Some((self.time, body));
        }
        self.posed = true;
        if self.carry {
            let place = body * pelvis.inverse();
            let (_, turn, at) = place.to_scale_rotation_translation();
            self.root = at;
            self.root_turn = turn.normalize();
            self.carry = false;
        }
        self.pelvis = pelvis;
        self.points = points;
        self.update_inertia();
        self.position = self.body().w_axis.truncate();
    }

    /// Where the motion puts the Wrecker's bone probes now, in the
    /// pelvis's space.
    pub fn set_probes(&mut self, probes: Vec<Vec3>) {
        self.probes = probes;
        if self.points.is_empty() {
            self.update_inertia();
        }
    }

    /// Wrecker::UpdateInertia (0x5377c0): the centre of mass and the
    /// inertia from the posed rider's bounds in the model's space (here
    /// Rider.col's corners, or the bone probes, as the motion has them).
    fn update_inertia(&mut self) {
        let corners = if self.points.is_empty() { &self.probes } else { &self.points };
        let (centre, half) = if corners.is_empty() {
            (RIDER_CENTRE, REST_HALF)
        } else {
            let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            for p in corners {
                let m = self.pelvis.transform_point3(*p);
                lo = lo.min(m);
                hi = hi.max(m);
            }
            ((lo + hi) * 0.5, ((hi - lo) * 0.5).max(Vec3::splat(0.05)))
        };
        let h2 = half * half;
        self.mass_centre = centre;
        self.inertia = Vec3::new(h2.y + h2.z, h2.x + h2.z, h2.x + h2.y) * (MASS / 12.0);
    }

    fn pick<const N: usize>(&mut self, from: [&'static str; N]) -> &'static str {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        from[self.seed as usize % N]
    }

    /// The turn rate in world axes, rad/s.
    pub fn world_spin(&self) -> Vec3 {
        self.root_turn * self.spin
    }

    fn mass_centre_world(&self) -> Vec3 {
        self.root + self.root_turn * self.mass_centre
    }

    /// One step of `h` seconds. `bike` is where the bike is now, for a
    /// bail. Returns whether the wreck is over.
    pub fn step(&mut self, ground: &dyn Ground, h: f32, bike: Mat4, bike_velocity: Vec3) -> bool {
        self.previous = Some((self.root, self.root_turn));
        self.time += h;
        if self.stage == Stage::Bail {
            let (_, turn, at) = bike.to_scale_rotation_translation();
            self.root = at;
            self.root_turn = turn.normalize();
            self.velocity = bike_velocity;
            let body = self.body();
            self.position = body.w_axis.truncate();
            // Let go once less than half a second of the bail motion is
            // left (0x535d50, state 1: (frames − frame) / fps < 0.5).
            let frame = (self.time * BAIL_FPS).floor();
            if (self.bail_frames - frame) / BAIL_FPS < 0.5 {
                // With the pelvis's own speed and turn since the last pose
                // (0x537bf0): the turn as the cross of the old and new up
                // rows, or of the forward rows when up hasn't moved.
                if let Some((t, was)) = self.last.filter(|l| self.time - l.0 > 1e-3) {
                    let dt = self.time - t;
                    self.velocity = (self.position - was.w_axis.truncate()) / dt;
                    let (up0, up1) = (was.y_axis.truncate(), body.y_axis.truncate());
                    let turn = if up0 != up1 {
                        up0.cross(up1)
                    } else {
                        was.z_axis.truncate().cross(body.z_axis.truncate())
                    };
                    self.spin = self.root_turn.inverse() * (turn / dt);
                } else {
                    self.spin = Vec3::ZERO;
                }
                self.history = History::default();
                self.stage = Stage::Tumble;
                self.motion = self.pick(FALLS);
                self.faded = self.time + FADE;
                self.fading = true;
            }
            return false;
        }
        let was = self.position;
        self.integrate(h);
        let push = push_off_bike(bike, self.body().w_axis.truncate()) * h;
        self.root += push;
        if !self.probes.is_empty() {
            self.touch_probes(ground, h);
        } else if !self.points.is_empty() {
            self.touch_parts(ground, h);
        } else {
            self.touch_sphere(ground, h);
        }
        self.position = self.body().w_axis.truncate();
        if self.fading && self.time >= self.faded {
            self.fading = false;
            self.carry = true;
            if let Some(m) = self.queued.take() {
                self.motion = m;
                self.faded = self.time + FADE;
                self.fading = true;
            }
        }
        // At rest by how far it moves: held on the ground, the body keeps
        // some speed into it.
        if self.stage == Stage::Down && (self.position - was).length() < REST_SPEED * h {
            self.still += h;
        } else {
            self.still = 0.0;
        }
        self.still >= REST_TIME || self.time >= LONGEST
    }

    /// PhysicsRigidBody::Step (0x4cc630) with the gravity model's force
    /// (SelectiveGravityModel, 32.2 ft/s² × the mass): the centre of mass,
    /// its velocity, the turn (a quaternion) and the turn rate (Euler's
    /// equations, I⁻¹(−ω × Iω)) each take an Adams-Bashforth step; the
    /// root is then put back round the moved centre of mass.
    fn integrate(&mut self, h: f32) {
        let centre = self.mass_centre_world();
        let d_position = self.velocity;
        let d_velocity = Vec3::new(0.0, -GRAVITY, 0.0);
        let q = self.root_turn;
        let w = self.spin;
        let dq = q * Quat::from_xyzw(w.x, w.y, w.z, 0.0);
        let d_turn = glam::Vec4::from(dq) * 0.5;
        let d_spin = if self.inertia.min_element() > 0.0 {
            -(w.cross(self.inertia * w)) / self.inertia
        } else {
            Vec3::ZERO
        };
        let k = h * 0.5;
        let centre = centre + (d_position * 3.0 - self.history.position) * k;
        self.velocity += (d_velocity * 3.0 - self.history.velocity) * k;
        let turn = glam::Vec4::from(q) + (d_turn * 3.0 - self.history.turn) * k;
        self.spin += (d_spin * 3.0 - self.history.spin) * k;
        self.history = History { position: d_position, velocity: d_velocity, turn: d_turn, spin: d_spin };
        self.root_turn = Quat::from_vec4(turn).normalize();
        self.root = centre - self.root_turn * self.mass_centre;
    }

    /// ConstraintMethodCollisionModel::ApplyContactImpulse (0x43bdb0) for
    /// a contact at `point` with the ground or an object along `normal`:
    /// no bounce (+0xb8 = 0), only 5 % of the impulse that would stop the
    /// point (1 − +0xf4), at most 2000 (+0xf8), through the body's mass
    /// and inertia, so a contact off the centre of mass turns it; then the
    /// speed and the turn rate are cut by 0.99 × the step (+0xf0).
    fn contact_impulse(&mut self, point: Vec3, normal: Vec3, h: f32) {
        let arm = point - self.mass_centre_world();
        let world_spin = self.world_spin();
        let moving = self.velocity + world_spin.cross(arm);
        let num = -normal.dot(moving);
        let inverse = self.root_turn.inverse();
        let (arm_l, normal_l) = (inverse * arm, inverse * normal);
        let inv_inertia = Vec3::ONE / self.inertia.max(Vec3::splat(1e-6));
        let turn_term = ((arm_l.cross(normal_l) * inv_inertia).cross(arm_l)).dot(normal_l);
        let mut j = num / (1.0 / MASS + turn_term) * (1.0 - IMPULSE_DROPPED);
        if j > IMPULSE_MOST {
            j = IMPULSE_MOST;
        }
        let impulse = normal * j;
        let keep = 1.0 - h * DAMPING;
        self.velocity = (self.velocity * MASS + impulse) / MASS * keep;
        let momentum = self.inertia * self.spin + arm_l.cross(inverse * impulse);
        self.spin = momentum * inv_inertia * keep;
    }

    /// The Wrecker's tumble (ConstraintMethodCollisionModel::Step,
    /// 0x43c8a0): Rider.col's shapes meet the solid objects (the deepest
    /// touch is answered with a contact impulse, 0x43b800, and the body is
    /// pushed out by 1.005 × the overlap); then the deepest bone probe
    /// under the ground takes the contact impulse along the ground's
    /// normal at its point and the body is pushed out by 1.005 × its
    /// depth. Only the ground contact counts as touching (+0x58).
    fn touch_probes(&mut self, ground: &dyn Ground, h: f32) {
        let place = self.body();
        let mut out: Option<(f32, Vec3, Vec3)> = None;
        for p in &self.points {
            let at = place.transform_point3(*p);
            if let Some(hit) = ground.solid(at, CORNER_RADIUS) {
                if hit.depth > out.map_or(0.0, |o| o.0) {
                    out = Some((hit.depth, hit.normal, at));
                }
            }
        }
        if let Some((depth, normal, at)) = out {
            self.contact_impulse(at, normal, h);
            self.root += normal * depth * 1.005;
        }
        let place = self.body();
        let mut deepest: Option<(f32, Vec3, Vec3)> = None;
        for p in &self.probes {
            let at = place.transform_point3(*p);
            let (height, normal) = ground.terrain(at.x, at.z);
            if at.y < height && height - at.y > deepest.map_or(0.0, |d| d.0) {
                deepest = Some((height - at.y, normal.normalize_or(Vec3::Y), at));
            }
        }
        self.touching = deepest.is_some();
        let Some((depth, normal, at)) = deepest else {
            return;
        };
        // The impulse acts at the surface point less the push-out
        // (bestPoint + offset), where the probe is.
        self.contact_impulse(Vec3::new(at.x, at.y + depth, at.z) - normal * depth, normal, h);
        self.root += normal * depth * 1.005;
        if self.stage == Stage::Tumble {
            self.stage = Stage::Down;
            self.queued = Some(self.pick(HITS));
        }
    }

    /// The deepest posed corner in the ground or an object, without the
    /// bone probes: the same contact, pushed out along the normal.
    fn touch_parts(&mut self, ground: &dyn Ground, h: f32) {
        let place = self.body();
        let mut deepest: Option<(f32, Vec3, Vec3)> = None;
        for p in &self.points {
            let at = place.transform_point3(*p);
            let (height, normal) = ground.terrain(at.x, at.z);
            let normal = normal.normalize_or(Vec3::Y);
            let mut touch = ((height - at.y) * normal.y, normal);
            if let Some(hit) = ground.solid(at, CORNER_RADIUS) {
                if hit.depth > touch.0 {
                    touch = (hit.depth, hit.normal);
                }
            }
            if touch.0 > deepest.map_or(0.0, |d| d.0) {
                deepest = Some((touch.0, touch.1, at));
            }
        }
        self.touching = deepest.is_some();
        let Some((depth, normal, at)) = deepest else {
            return;
        };
        self.contact_impulse(at, normal, h);
        self.root += normal * depth;
        if self.stage == Stage::Tumble {
            self.stage = Stage::Down;
            self.queued = Some(self.pick(HITS));
        }
    }

    /// One sphere round the body's centre, until the parts are known.
    fn touch_sphere(&mut self, ground: &dyn Ground, h: f32) {
        let centre = self.body().w_axis.truncate();
        let (height, normal) = ground.terrain(centre.x, centre.z);
        let normal = normal.normalize_or(Vec3::Y);
        let mut touch = ((height + RADIUS - centre.y) * normal.y, normal);
        if let Some(hit) = ground.solid(centre, RADIUS) {
            if hit.depth > touch.0 {
                touch = (hit.depth, hit.normal);
            }
        }
        let (depth, normal) = touch;
        self.touching = depth > 0.0;
        if depth > 0.0 {
            self.root += normal * depth;
            let into = self.velocity.dot(normal);
            if into < 0.0 {
                self.velocity -= normal * into * (1.0 + BOUNCE);
            }
            let along = self.velocity - normal * self.velocity.dot(normal);
            let slow = (FRICTION * GRAVITY * h).min(along.length());
            self.velocity -= along.normalize_or(Vec3::ZERO) * slow;
            self.spin *= 0.9;
            if self.stage == Stage::Tumble {
                self.stage = Stage::Down;
                self.queued = Some(self.pick(HITS));
            }
        }
    }
}

/// How fast the bike's box pushes a rider part at `at` out of it, ft/s
/// (0x538050): the depth to the nearest face, times 5, out through that
/// face, in world space. Zero outside the box. (The game sums per-axis
/// depths over the parts inside and averages them; how it picks the side
/// on each axis is not clear from the code, so this takes the nearest
/// face, which a part just inside the box gives either way.)
pub fn push_off_bike(bike: Mat4, at: Vec3) -> Vec3 {
    let d = bike.inverse().transform_point3(at) - BIKE_BOX_CENTRE;
    if d.abs().cmpge(BIKE_BOX_HALF).any() {
        return Vec3::ZERO;
    }
    let depth = BIKE_BOX_HALF - d.abs();
    let axis = if depth.x <= depth.y && depth.x <= depth.z {
        Vec3::X
    } else if depth.y <= depth.z {
        Vec3::Y
    } else {
        Vec3::Z
    };
    let out = axis * depth * d.signum();
    bike.transform_vector3(out * BIKE_BOX_PUSH)
}

/// A fall motion picked by `seed`.
pub fn fall(seed: u32) -> &'static str {
    FALLS[seed as usize % FALLS.len()]
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Corners given in model space, in the pelvis's space at rest.
    fn model_points(points: &[Vec3]) -> Vec<Vec3> {
        points.iter().map(|p| *p - RIDER_CENTRE).collect()
    }

    #[test]
    fn pushes_the_rider_off_the_bike() {
        let bike = Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0));
        // Just right of the seat: pushed right by 5 × the 0.2 ft depth.
        let v = push_off_bike(bike, Vec3::new(10.3, 2.0, 0.0));
        assert!((v - Vec3::new(1.0, 0.0, 0.0)).length() < 1e-4, "{v}");
        assert_eq!(push_off_bike(bike, Vec3::new(11.0, 2.0, 0.0)), Vec3::ZERO);
    }

    #[test]
    fn corners_stop_on_the_ground_without_bouncing() {
        let start = Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0));
        let mut w = Wreck::new(start, Vec3::new(20.0, -10.0, 0.0), Vec3::ZERO, "Fall01", false, 1);
        w.set_pose(Mat4::from_translation(RIDER_CENTRE), model_points(&[Vec3::new(0.5, 0.5, 0.0), Vec3::new(-0.5, 0.5, 0.0)]));
        let mut rose = false;
        for _ in 0..300 {
            let was = w.velocity.y;
            if w.step(&crate::Flat, 0.02, start, Vec3::ZERO) {
                break;
            }
            rose |= w.touching && w.velocity.y > 0.0 && was <= 0.0;
            let low = w.points.iter().map(|p| w.body().transform_point3(*p).y).fold(f32::MAX, f32::min);
            assert!(low > -1e-3, "{low}");
        }
        assert!(!rose, "bounced");
        // Touching off its centre of mass, the body turns, but no faster
        // than a tumble.
        assert!(w.world_spin().length() < 10.0, "{}", w.world_spin());
        assert!(w.velocity.x.abs() < 2.0, "{}", w.velocity);
    }

    #[test]
    fn bone_probes_stop_on_the_ground_without_bouncing() {
        let start = Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0));
        let mut w = Wreck::new(start, Vec3::new(20.0, -10.0, 0.0), Vec3::ZERO, "Fall01", false, 1);
        w.set_pose(Mat4::from_translation(RIDER_CENTRE), Vec::new());
        w.set_probes(model_points(&[Vec3::new(0.5, 0.5, 0.0), Vec3::new(-0.5, 0.5, 0.0), Vec3::new(0.0, 2.0, 0.0)]));
        let mut rose = false;
        for _ in 0..300 {
            let was = w.velocity.y;
            if w.step(&crate::Flat, 0.02, start, Vec3::ZERO) {
                break;
            }
            rose |= w.touching && w.velocity.y > 0.0 && was <= 0.0;
            let low = w.probes.iter().map(|p| w.body().transform_point3(*p).y).fold(f32::MAX, f32::min);
            assert!(low > -1e-3, "{low}");
        }
        assert!(!rose, "bounced");
        assert!(w.velocity.x.abs() < 2.0, "{}", w.velocity);
    }

    #[test]
    fn a_bail_lets_go_half_a_second_before_its_motion_ends_with_the_pelvis_speed() {
        // LeftOver is 10 frames at 15 a second: let go once fewer than
        // 7.5 frames are left, at frame 3 (0.2 s). The motion moves the
        // pelvis forward 0.5 ft a frame, 30 ft/s.
        let bike = Mat4::IDENTITY;
        let mut w = Wreck::new(bike, Vec3::ZERO, Vec3::ZERO, "LeftOver", true, 7);
        let h = 1.0 / 60.0;
        for k in 0..30 {
            w.set_pose(Mat4::from_translation(RIDER_CENTRE + Vec3::Z * (0.5 * k as f32)), Vec::new());
            w.step(&crate::Flat, h, bike, Vec3::ZERO);
            let t = (k + 1) as f32 * h;
            if t < 0.199 {
                assert_eq!(w.stage, Stage::Bail, "{t}");
            } else {
                assert_ne!(w.stage, Stage::Bail, "{t}");
                assert!(FALLS.contains(&w.motion), "{}", w.motion);
                break;
            }
        }
        assert!((w.velocity - Vec3::Z * 30.0).length() < 0.5, "{}", w.velocity);
    }

    #[test]
    fn the_landing_waits_for_the_fall_to_fade_in() {
        // Thrown in the air just above the ground: down at once, but the
        // landing motion only starts once the fall's 0.5 s fade is done.
        let start = Mat4::from_translation(Vec3::new(0.0, -2.0, 0.0));
        let mut w = Wreck::new(start, Vec3::new(10.0, 0.0, 0.0), Vec3::ZERO, "Fall02", false, 3);
        w.set_pose(Mat4::from_translation(RIDER_CENTRE), model_points(&[Vec3::new(0.0, 2.5, 0.0)]));
        let h = 1.0 / 60.0;
        let mut t = 0.0;
        while t < 0.7 {
            w.step(&crate::Flat, h, start, Vec3::ZERO);
            t += h;
            if t < FADE - h {
                assert_eq!(w.motion, "Fall02", "{t}");
            }
        }
        assert_eq!(w.stage, Stage::Down);
        assert!(HITS.contains(&w.motion), "{}", w.motion);
    }

    #[test]
    fn slides_as_far_as_the_original() {
        // The original's slides from 30-37 ft/s cover 23-27 ft before the
        // reset, 2.3-3 s after the crash (SX01 recordings).
        // Thrown along the ground, the body's centre 0.5 ft up.
        let start = Mat4::from_translation(Vec3::new(0.0, -2.5, 0.0));
        let mut w = Wreck::new(start, Vec3::new(33.0, 0.0, 0.0), Vec3::ZERO, "Fall01", false, 1);
        w.set_pose(Mat4::from_translation(RIDER_CENTRE), model_points(&[Vec3::new(0.0, 2.6, -2.0), Vec3::new(0.0, 2.6, 1.0)]));
        for _ in 0..(2.6f32 / 0.02) as usize {
            w.step(&crate::Flat, 0.02, start, Vec3::ZERO);
        }
        let went = w.position.x;
        assert!((20.0..34.0).contains(&went), "{went}");
    }
}
