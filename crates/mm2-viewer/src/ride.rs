//! The chase camera and the terrain lookups the riders share. The
//! computer riders' bike is `mm2_physics::computer` (the game's two-wheel
//! model, 0x414000 and 0x40faa0); the player's is `mm2_physics::Bike`.

use glam::Vec3;
use mm2_formats::trn::Terrain;
use mm2_physics::camera::{Preset, View};

/// What the rider is asking for this frame; shared with the player's bike.
pub use mm2_physics::Controls;

/// The fork's steadier camera (`--steady-camera`, not the game's): the
/// Chase Cam rises and looks with the bike more gently, so bumps don't
/// shake the view. It still turns with the bike as the game's does.
fn steady() -> bool {
    crate::tweaks::get().steady_camera
}

/// How much wider than the game's the cameras see (`--fov=N`, the
/// fork's, for the Chase Cam's 85°), degrees.
fn wider() -> f32 {
    crate::tweaks::get().fov as f32 - 85.0
}

/// The terrain's surface normal at `at` (0x50d3b0): the stored normals
/// of the corners of the triangle under it, each weighted by one over its
/// squared distance. On an even cell's lower triangle the original takes
/// the far corner's normal from (x, z+1) instead of (x+1, z+1); kept.
pub fn normal(terrain: &Terrain, at: Vec3) -> Vec3 {
    let (x, z, fx, fz) = cell(terrain, at.x, at.z);
    let d = [
        fx * fx + fz * fz,
        (1.0 - fx).powi(2) + fz * fz,
        fx * fx + (1.0 - fz).powi(2),
        (1.0 - fx).powi(2) + (1.0 - fz).powi(2),
    ]
    .map(|d: f32| if d < 0.001 { 0.001 } else { d });
    let corners = if (x ^ z) & 1 == 0 {
        if d[2] <= d[1] { [0, 2, 3] } else { [0, 1, 2] }
    } else if d[3] <= d[0] {
        [1, 2, 3]
    } else {
        [0, 1, 2]
    };
    let mut n = Vec3::ZERO;
    for c in corners {
        let s = terrain.normals[(z + c / 2) * terrain.size + x + c % 2];
        n += Vec3::new(s[0] as f32, s[1] as f32, s[2] as f32) / 127.0 / d[c];
    }
    n.normalize_or(Vec3::Y)
}

/// The race camera (KrustyBikeCamera on BikeCamera, VehicleCamera and
/// FollowCamera, FollowCam.cpp 0x462ee0..0x4670fb; `docs/physics.md`
/// section 20). Each view (state) is a distance, a pitch and a yaw from
/// the way the bike is heading; the eye goes there and the look point
/// leads the bike, both through the game's own filters: each axis moves
/// `min(dt, 0.3) / 0.3` of the way a frame (0x463140's eases). On top,
/// the camera swings with how fast the bike turns, lifts over rising
/// ground ahead, and, in a crash, stands off to the side and keeps the
/// rider and the bike in the picture.
///
/// The states: 0 Chase, 1 Slot, 2 Sky, 3 Rider, 4 User (the camera
/// button's cycle), 5 frozen (Numpad 0), 6 Reverse Cam and 7 Thrill Cam
/// (held). The Helmet Cam is the fork's, after User Cam.
#[derive(Debug, Clone, Copy)]
pub struct Chase {
    /// The eye and the point it looks at, as last drawn.
    eye: Vec3,
    target: Vec3,
    /// The view the camera button picked (states 0 to 4), kept between
    /// races like the game's global +0x2930.
    pub view: View,
    /// The held views: Reverse (6) and Thrill (7).
    held: Option<Held>,
    /// The view the camera comes back to the frame after a held view ends
    /// (+0x248 = 6, +0x250 = 7): it snaps there, no easing.
    snap: bool,
    /// The distance (+0x220), pitch (+0x22c) and yaw (+0x234) the view
    /// puts the eye at, and those to go back to after a crash (+0x2c4..).
    distance: f32,
    pitch: f32,
    yaw: f32,
    saved: (f32, f32, f32),
    /// The view's own before a held view (+0x224, +0x230, +0x238).
    before_held: (f32, f32, f32, f32),
    /// The User Cam's place (global +0x2934.., numpad *, 0x4981d0).
    user: Preset,
    /// The way the camera follows the bike, level (+0x29c).
    heading: Vec3,
    /// The bike's place led along its velocity (+0x2b4).
    lead: Vec3,
    /// The turn the camera takes from the bike's turn rate (+0x308), the
    /// lift over rising ground (+0x2d8), the yaw round the bike in a
    /// crash (+0x23c) and the height over it then (+0x2d4).
    turn_yaw: f32,
    lift: f32,
    world_yaw: f32,
    height: f32,
    /// The eye's and the look point's filters (+0x288.., +0x27c..).
    eye_ease: Option<[Ease; 3]>,
    look_ease: [Ease; 3],
    /// The field of view, degrees (+0x16c), what it eases to (+0x258,
    /// through 0.55), and the Rider Cam's (+0x2f0).
    fov: f32,
    want_fov: f32,
    fov_ease: Ease,
    rider_fov: f32,
    /// The distance's filter (+0x298), for the crash view.
    distance_ease: Ease,
    /// +0x274, +0x277: the eye goes straight to its spot, no easing.
    settled: bool,
    settle_checked: bool,
    /// +0x276: the crash view is on.
    crash_view: bool,
    /// +0x278, +0x279, +0x304: the yaw turned past 2 rad, and the blend.
    was_wide: bool,
    blending: bool,
    blend: f32,
    /// The bike's heading and whether it was upside down when it crashed
    /// (+0x45c, +0x464), and whether it was down last frame.
    crash_yaw: (f32, bool),
    was_crashed: bool,
    /// The crash was a roll past 110° (crash reason 6, 0x40a950): the
    /// camera follows on as it was until the rider lands (0x4174b0).
    rolled: bool,
    /// The bike's velocity last frame, for a turn rate when the bike
    /// doesn't give one (the computer's and the taped bikes).
    was_velocity: Vec3,
    /// Where and how fast the bike left the ground last (bike +0x488,
    /// +0x494), for the Thrill Cam.
    takeoff: (Vec3, Vec3),
    was_airborne: bool,
    /// The frozen camera (state 5, Numpad 0, 0x4679e0): the eye stays and
    /// turns to watch the bike; how far it was (+0x2c0).
    frozen: Option<Vec3>,
    frozen_distance: f32,
    /// The Helmet Cam (not in the original), after User Cam in the cycle.
    pub helmet: bool,
    /// The Helmet Cam's eye, its height softened apart from its place.
    head: Vec3,
    /// The view's tilt, rad: the Rider Cam's and the Helmet Cam's; the
    /// Helmet Cam's head: how far it leans with the bike; the landing
    /// dip, ft, and how fast it moves; whether the bike was in the air
    /// and how fast it was coming down.
    roll: f32,
    lean: f32,
    dip: (f32, f32),
    helmet_air: (bool, f32),
}

/// A held view: the Reverse Cam (state 6) or the Thrill Cam (7).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Held {
    Reverse,
    Thrill(Vec3),
}

/// One of the camera's filters (FollowCameraEase, 20 bytes, 0x463140).
/// The game moves the value `min(dt, rate) / rate` of the way each frame,
/// so how far the camera trails depends on the frame rate; the original's
/// recorded chase camera (`trace` below) trails as that does at a high
/// frame rate, which is what `1 − e^(−dt / rate)` gives at any.
#[derive(Debug, Clone, Copy)]
struct Ease {
    value: f32,
    rate: f32,
}

impl Ease {
    fn new(value: f32, rate: f32) -> Self {
        Ease { value, rate }
    }

    fn update(&mut self, to: f32, dt: f32) -> f32 {
        self.value += (to - self.value) * (1.0 - (-dt.max(0.0) / self.rate).exp());
        self.value
    }
}

fn eases(v: Vec3, rate: f32) -> [Ease; 3] {
    [Ease::new(v.x, rate), Ease::new(v.y, rate), Ease::new(v.z, rate)]
}

fn ease3(e: &mut [Ease; 3], to: Vec3, dt: f32) -> Vec3 {
    Vec3::new(e[0].update(to.x, dt), e[1].update(to.y, dt), e[2].update(to.z, dt))
}

/// `v` turned `angle` rad about `axis` (d3drm's D3DRMVectorRotate).
fn rotate(v: Vec3, axis: Vec3, angle: f32) -> Vec3 {
    let k = axis.normalize_or(Vec3::Y);
    let (s, c) = angle.sin_cos();
    v * c + k.cross(v) * s + k * k.dot(v) * (1.0 - c)
}

/// 1 − |angle| / (π/2), at least 0 (0x52c510).
fn angle_weight(angle: f32) -> f32 {
    (1.0 - angle.abs() * std::f32::consts::FRAC_2_PI).max(0.0)
}

/// The bike's pitch and roll, rad, from its nose and up
/// (OrientationAnglesFromVectors, 0x4b5a60): the nose up and the bike
/// leaned to its right are positive.
fn pitch_roll(nose: Vec3, up: Vec3) -> (f32, f32) {
    let len = (nose.x * nose.x + nose.z * nose.z).sqrt();
    let (c, s) = if len == 0.0 { (1.0, 0.0) } else { (nose.z / len, nose.x / len) };
    let fz = c * nose.z + s * nose.x;
    let uz = c * up.z + s * up.x;
    let ux = c * up.x - s * up.z;
    let len = (fz * fz + nose.y * nose.y).sqrt();
    let (cp, sp) = if len == 0.0 { (1.0, 0.0) } else { (fz / len, nose.y / len) };
    let pitch = cp.clamp(-1.0, 1.0).acos().copysign(if nose.y < 0.0 { -1.0 } else { 1.0 });
    let uy = up.y * cp - uz * sp;
    let len = (uy * uy + ux * ux).sqrt();
    let cr = if len == 0.0 { 1.0 } else { uy / len };
    let mut roll = cr.clamp(-1.0, 1.0).acos();
    if ux < 0.0 {
        roll = -roll;
    }
    if (pitch.abs() - std::f32::consts::FRAC_PI_2).abs() < 0.1745 {
        roll = 0.0;
    }
    (pitch, roll)
}

/// What the player holds for the camera this frame: the Thrill Cam and
/// the Reverse Cam (actions 10 and 11), and Numpad 8, 2, 4 and 6, which
/// turn the view (0x463a30).
#[derive(Debug, Clone, Copy, Default)]
pub struct CamKeys {
    pub thrill: bool,
    pub reverse: bool,
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
}

/// The bike the camera follows this frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct Followed {
    /// The bike model's origin (its part 0, 0x52bb60).
    pub position: Vec3,
    /// Its heading on the level, and its nose and up.
    pub forward: Vec3,
    pub nose: Vec3,
    pub up: Vec3,
    pub velocity: Vec3,
    /// The speed along the bike's nose (+0x434).
    pub speed: f32,
    pub crashed: bool,
    pub airborne: bool,
    /// The rear tyre is more than 2 ft off the ground (0x4980a0's
    /// rear depth below −2).
    pub high: bool,
    /// How fast the tyres turn the bike's way, rad/s (+0x43c); worked out
    /// from the velocity when the bike doesn't say.
    pub turn_rate: Option<f32>,
    /// The rider's `Head` frame, if it is drawn.
    pub head: Option<Vec3>,
    /// The race is over for the only player: the camera stops swinging
    /// with the turns (0x498130's +0x7a4).
    pub finished_alone: bool,
    /// The bike is down because it rolled or pitched over (crash reasons
    /// 6 and 8; which one the camera tells from the bike).
    pub flipped: bool,
    /// The thrown rider is still in the bail motion (Wrecker state 1), or
    /// has touched the ground or something (state 3); a bike with no
    /// thrown rider counts as down.
    pub wreck_bail: bool,
    pub wreck_down: bool,
}

impl Chase {
    /// The follow camera's settings (0x4e2d70 → 0x463140): the look point
    /// 27 ft ahead (+0x228), 7 ft over the bike in a crash (+0x2d0), and
    /// 60° round from it (+0x2e8).
    const LOOK_AHEAD: f32 = 27.0;
    const CRASH_HEIGHT: f32 = 7.0;
    const CRASH_SIDE: f32 = 1.048;
    /// The eye's and the look point's filter, s (0x464ee0).
    const EASE: f32 = 0.3;
    /// The widest view (Camera +0x1dc) and the field of view the camera
    /// starts with (+0x16c).
    const MAX_FOV: f32 = 109.0;
    const START_FOV: f32 = 77.0;
    /// The bike's top speed the cameras measure by (+0x438).
    const TOP_SPEED: f32 = 180.0;
    /// The fork's steadier camera: slower filters.
    const STEADY_EASE: f32 = 0.45;

    /// Behind a bike at `position` heading `forward`.
    pub fn new(position: Vec3, forward: Vec3) -> Self {
        let heading = Vec3::new(forward.x, 0.0, forward.z).normalize_or(Vec3::Z);
        let chase = View::Chase.preset().unwrap();
        let eye = position - heading * 5.86 + Vec3::Y * 10.15;
        let start = Self::START_FOV + wider();
        let mut c = Chase {
            eye,
            target: position + heading * 12.7,
            view: View::Chase,
            held: None,
            snap: false,
            distance: chase.distance,
            pitch: chase.elevation,
            yaw: chase.yaw,
            saved: (chase.distance, chase.elevation, chase.yaw),
            before_held: (0.0, 0.0, 0.0, 0.0),
            user: Preset { fov: Some(85.0), ..chase },
            heading,
            lead: position,
            turn_yaw: 0.0,
            lift: 0.0,
            world_yaw: 0.0,
            height: Self::CRASH_HEIGHT,
            eye_ease: None,
            look_ease: eases(position, Self::EASE),
            fov: start,
            want_fov: start,
            fov_ease: Ease::new(start, 0.55),
            rider_fov: start,
            distance_ease: Ease::new(chase.distance, 0.3),
            settled: false,
            settle_checked: false,
            crash_view: false,
            was_wide: false,
            blending: false,
            blend: 0.0,
            crash_yaw: (0.0, false),
            was_crashed: false,
            rolled: false,
            was_velocity: Vec3::ZERO,
            takeoff: (position, Vec3::ZERO),
            was_airborne: false,
            frozen: None,
            frozen_distance: 0.0,
            helmet: false,
            head: position + Vec3::Y * Self::HELMET_UP,
            roll: 0.0,
            lean: 0.0,
            dip: (0.0, 0.0),
            helmet_air: (false, 0.0),
        };
        c.set_view(View::Chase);
        c
    }

    /// The same camera behind a new spot, keeping the view and zoom.
    pub fn restart(&self, position: Vec3, forward: Vec3) -> Self {
        let mut c = Chase::new(position, forward);
        c.helmet = self.helmet;
        c.frozen = self.frozen;
        c.user = self.user;
        c.rider_fov = self.rider_fov;
        c.set_view(self.view);
        c.want_fov = self.want_fov;
        c.fov = self.want_fov;
        c.fov_ease.value = self.want_fov;
        c
    }

    /// Puts the camera in `view` (SetCameraState, 0x466e50).
    fn set_view(&mut self, view: View) {
        self.view = view;
        match view {
            View::Rider => {
                self.want_fov = self.rider_fov;
                self.distance = 0.0;
                self.pitch = 0.0;
                self.yaw = 0.0;
            }
            View::User => {
                self.rider_fov = self.want_fov;
                let u = self.user;
                (self.distance, self.pitch, self.yaw) = (u.distance, u.elevation, u.yaw);
                self.want_fov = u.fov.unwrap_or(85.0);
            }
            _ => {
                let p = view.preset().unwrap();
                (self.distance, self.pitch, self.yaw) = (p.distance, p.elevation, p.yaw);
                if let Some(fov) = p.fov {
                    self.want_fov = (fov + wider()).clamp(10.0, Self::MAX_FOV + wider().max(0.0));
                }
            }
        }
        self.saved = (self.distance, self.pitch, self.yaw);
    }

    /// The next view on a camera press; the view's name, to flash. A
    /// frozen camera goes back to the view it froze in instead (0x466ad0,
    /// and the replay's camera button, 0x489c20); a crashed bike keeps
    /// its view (0x52bff0).
    pub fn next_view(&mut self) -> &'static str {
        if self.frozen.take().is_some() {
            return if self.helmet { HELMET_CAM } else { self.view.name() };
        }
        if self.was_crashed && !self.helmet {
            return self.view.name();
        }
        // The Helmet Cam comes after the game's last view.
        if self.helmet {
            self.helmet = false;
        } else if self.view == View::User {
            self.helmet = true;
            return HELMET_CAM;
        }
        self.set_view(self.view.next());
        self.view.name()
    }

    /// F1: straight into the Helmet Cam, or back out to the view it came
    /// from; the view's name, to flash.
    pub fn toggle_helmet(&mut self) -> &'static str {
        self.frozen = None;
        self.helmet = !self.helmet;
        if self.helmet { HELMET_CAM } else { self.view.name() }
    }

    /// Numpad 0 (0x466ad0's key 0x52): freezes the camera where it is, or
    /// lets it go back to its view. Whether it is frozen now.
    pub fn freeze(&mut self) -> bool {
        self.frozen = match self.frozen {
            Some(_) => None,
            None => Some(self.eye),
        };
        self.frozen.is_some()
    }

    /// Numpad *: the view as it is now becomes the User Cam (0x4981d0).
    pub fn save_user(&mut self) {
        if self.frozen.is_none() && self.held.is_none() && !self.helmet && !self.was_crashed {
            self.user = Preset {
                distance: self.distance,
                elevation: self.pitch,
                yaw: self.yaw,
                fov: Some(self.want_fov),
            };
        }
    }

    /// The view is from the rider's own eyes (Rider Cam or the Helmet
    /// Cam), so the rider's body and sign are left out.
    pub fn first_person(&self) -> bool {
        self.frozen.is_none() && (self.helmet || self.view == View::Rider)
    }

    /// Home and End: the field of view narrows or widens by 3° (10° to
    /// 109°, Camera +0x1e0, +0x1dc).
    pub fn zoom(&mut self, by: f32) {
        let most = Self::MAX_FOV + wider().max(0.0);
        self.want_fov = (self.want_fov + by * mm2_physics::camera::FOV_STEP).clamp(10.0, most);
    }

    /// How far the view tilts about the way it looks, rad, in game
    /// coordinates: the Rider Cam and the Helmet Cam lean with the bike.
    pub fn roll(&self) -> f32 {
        self.roll
    }

    /// The field of view, degrees: the Helmet Cam sees a little wider,
    /// as a helmet's opening does.
    pub fn fov(&self) -> f32 {
        if self.helmet && self.frozen.is_none() {
            return (self.fov + Self::HELMET_WIDER).min(120.0);
        }
        self.fov
    }

    fn ease_rate(&self) -> f32 {
        if steady() { Self::STEADY_EASE } else { Self::EASE }
    }

    /// The way the camera follows (0x52c030): the bike's velocity on the
    /// level, or below 10 ft/s a mix of it and the bike's nose, eased.
    fn follow_heading(&self, bike: &Followed) -> Vec3 {
        let forward_moving = bike.speed >= 0.0;
        let v = bike.velocity;
        let mut h = if bike.speed.abs() < 10.0 {
            let mut velocity = if forward_moving { v } else { -v };
            velocity.y = 0.0;
            if velocity.x == 0.0 && velocity.z == 0.0 {
                velocity = bike.nose;
            }
            let mut facing = bike.nose * v.length().max(0.01);
            facing.y = 0.0;
            if facing.x == 0.0 && facing.z == 0.0 {
                facing = velocity;
            }
            let mut h = (velocity - facing) * (bike.speed.abs() / 10.0) + facing;
            h.y = if forward_moving { 0.0 } else { h.y * 0.5 };
            let h = h.normalize_or(Vec3::ZERO);
            self.heading * 0.82 + h * 0.18
        } else {
            let mut h = if forward_moving { v } else { -v };
            h.y = if forward_moving { 0.0 } else { h.y * 0.5 * 0.18 + self.heading.y * 0.82 };
            h.normalize_or(Vec3::ZERO)
        };
        if h == Vec3::ZERO {
            h = self.heading;
        }
        h
    }

    /// The look point while following (0x52c510): ahead along the heading,
    /// turned with the bike's turn, further at speed and nearer in the
    /// wide views, raised when climbing, lifted over rising ground ahead,
    /// and 3 ft up.
    fn look_ahead(&mut self, bike: &Followed, still: bool, rider: Option<f32>, terrain: &Terrain) -> Vec3 {
        let zoom = self.fov / Self::MAX_FOV;
        let (mut distance, offset) = match rider {
            Some(_) => (1.0, 0.0),
            None => {
                let v = bike.velocity;
                let along = v.dot(self.heading);
                let weight = angle_weight(self.yaw) * angle_weight(self.pitch);
                let scale = self.distance * 0.0111 * zoom + 1.0;
                let lead = if still {
                    0.0
                } else {
                    (1.0 - self.distance / 600.0) * 1.8 * (scale * scale * along / Self::TOP_SPEED)
                };
                let length = v.length();
                let climb = if length != 0.0 && v.y < -5.0 {
                    (v.y / length + 1.0) * lead
                } else if length != 0.0 && v.y > 5.0 {
                    (1.0 - v.y / length) * lead
                } else {
                    lead
                };
                (zoom * weight + climb, (1.0 - zoom) * (v.y * 0.25))
            }
        };
        if self.crash_view {
            distance *= 0.8;
        }
        let side = Vec3::new(self.heading.z, 0.0, -self.heading.x);
        let along = match rider {
            Some(pitch) => rotate(self.heading, side, -pitch),
            None => self.heading,
        };
        let mut direction = rotate(along, Vec3::Y, self.turn_yaw * 0.7);
        direction *= if self.held == Some(Held::Reverse) {
            -Self::LOOK_AHEAD * 0.25
        } else {
            distance * Self::LOOK_AHEAD
        };
        let mut look = self.lead + direction;
        look.y += offset;
        // Over rising ground ahead of a bike on the ground.
        if !bike.airborne {
            let reach = self.pitch.abs() * std::f32::consts::FRAC_2_PI * 1.2 + 0.3;
            let ahead = self.lead + direction * reach;
            let floor = ground(terrain, ahead.x, ahead.z);
            if floor > ahead.y {
                self.lift = (floor - ahead.y) * 0.6;
            } else {
                self.lift *= 0.01;
            }
        } else {
            self.lift *= 0.01;
        }
        look.y += self.lift + 3.0;
        look
    }

    /// Where the eye goes this frame (0x464b30): round the heading by the
    /// view's pitch and yaw while following; in a crash, out to the side
    /// of where the bike was going, keeping the rider in the picture.
    fn place_eye(&mut self, following: bool, close_in: bool, base: Vec3, head: Vec3, dt: f32) -> Vec3 {
        if let Some(eye) = self.frozen {
            return eye;
        }
        if let Some(Held::Thrill(spot)) = self.held {
            return spot;
        }
        if following {
            let yaw = self.turn_yaw + self.yaw;
            let eye = if self.view == View::Rider && self.held.is_none() {
                head
            } else {
                let axis = Vec3::new(self.heading.z, 0.0, -self.heading.x);
                let pitch = if self.heading.y == 0.0 { self.pitch } else { (self.pitch * self.heading.y).max(0.8) };
                let r = rotate(rotate(self.heading, axis, pitch), Vec3::Y, yaw);
                self.lead - r * self.distance
            };
            if !self.crash_view && self.held != Some(Held::Reverse) {
                self.saved = (self.distance, self.pitch, self.yaw);
                self.world_yaw = yaw - 16.0;
            }
            return eye;
        }
        // Into the crash view: round to the side the camera was on.
        if self.world_yaw < -8.0 {
            use std::f32::consts::{FRAC_PI_2, PI, TAU};
            self.height = Self::CRASH_HEIGHT;
            let mut w = self.world_yaw + 16.0;
            if w < 0.0 {
                self.pitch = 0.0;
                self.yaw = 0.0;
                w = if w < -FRAC_PI_2 { Self::CRASH_SIDE - PI } else { -Self::CRASH_SIDE };
            } else if w > FRAC_PI_2 {
                w = PI - Self::CRASH_SIDE;
            } else {
                w = Self::CRASH_SIDE;
            }
            w += self.crash_yaw.0;
            if self.crash_yaw.1 {
                w += PI;
            }
            if w >= TAU {
                w -= TAU;
            }
            self.world_yaw = w;
            self.distance_ease.value = self.distance;
        }
        self.keep_in_picture(head, close_in, dt);
        let r = rotate(Vec3::Z, Vec3::Y, self.yaw + self.world_yaw);
        let mut eye = base - r * self.distance;
        eye.y += self.height;
        eye
    }

    /// The crash view keeps the rider's head and the bike in the picture
    /// (0x416ed0 → 0x4636e0): either off the top or the bottom raises or
    /// lowers the eye 5 ft/s; both in, and near each other on screen, it
    /// closes in to 20 ft.
    fn keep_in_picture(&mut self, head: Vec3, close_in: bool, dt: f32) {
        // After a roll-over it rises more gently (0x416ed0).
        let (rise, drop) = if self.rolled { (2.5, 4.0) } else { (5.0, 5.0) };
        // Seen as last frame's camera saw them, on a 640×480 screen.
        let forward = (self.target - self.eye).normalize_or(Vec3::Z);
        let right = Vec3::Y.cross(forward).normalize_or(Vec3::X);
        let up = forward.cross(right);
        let fx = 1.0 / (self.fov.to_radians() / 2.0).tan();
        let fy = fx * 4.0 / 3.0;
        let mut screen = [Vec3::ZERO; 2];
        for (i, p) in [head, self.lead].into_iter().enumerate() {
            let d = p - self.eye;
            let z = d.dot(forward);
            let (x, y) = (d.dot(right) * fx / z, d.dot(up) * fy / z);
            let mut code = 0;
            if x < -1.0 {
                code = 1;
            } else if x > 1.0 {
                code = 2;
            }
            if y > 1.0 {
                code |= 4;
            } else if y < -1.0 {
                code |= 8;
            }
            if z < 1.0 {
                code |= 0x10;
            }
            if code != 0 {
                self.distance = self.distance_ease.update(self.distance, dt);
                match code {
                    4 => self.height += dt * rise,
                    8 => self.height -= dt * drop,
                    _ => {}
                }
                return;
            }
            let depth = 1.0 - 1.0 / z;
            screen[i] = Vec3::new(640.0 * (x * 0.5 + 0.5), 480.0 * (0.5 - y * 0.5), 480.0 * depth);
        }
        let (near, far) = if screen[0].z <= screen[1].z { (screen[0], screen[1]) } else { (screen[1], screen[0]) };
        if close_in && far.x - near.x < 64.0 && far.y - near.y < 48.0 {
            let to = self.distance - (near.z.min(335.0) - 15.0) * dt * 0.028;
            self.distance = self.distance_ease.update(to, dt).max(20.0);
        }
    }

    /// Moves towards the spot behind the bike and returns the view, in
    /// game coordinates: the eye and the point it looks at (0x465c20).
    pub fn follow(&mut self, bike: &Followed, keys: CamKeys, terrain: &Terrain, dt: f32) -> (Vec3, Vec3) {
        let crashed = bike.crashed;
        if bike.airborne && !self.was_airborne {
            self.takeoff = (bike.position, bike.velocity);
        }
        self.was_airborne = bike.airborne;
        if crashed && !self.was_crashed {
            let f = bike.forward;
            self.crash_yaw = (f.x.atan2(f.z), bike.up.y < 0.0);
            // Reason 8 is the nose straight up or down (0.9962), 6 a roll
            // past 110° (1.919862 rad).
            let (_, roll) = pitch_roll(bike.nose, bike.up);
            self.rolled = bike.flipped && bike.nose.y.abs() <= 0.996_194_7 && roll.abs() > 1.919_862;
        }
        if !crashed {
            self.rolled = false;
        }
        self.was_crashed = crashed;
        // How fast the bike turns (+0x43c): on the ground only.
        let turn_rate = bike.turn_rate.unwrap_or_else(|| {
            let (a, b) = (self.was_velocity, bike.velocity);
            if bike.airborne || dt <= 0.0 || a.length_squared() < 1.0 {
                0.0
            } else {
                (a.z * b.x - a.x * b.z).atan2(a.x * b.x + a.z * b.z) / dt
            }
        });
        self.was_velocity = bike.velocity;
        let head = bike
            .head
            .filter(|h| h.distance(bike.position) < 15.0)
            .unwrap_or(bike.position + bike.up * 5.3);
        let (bike_pitch, bike_roll) = pitch_roll(bike.nose, bike.up);

        self.heading = self.follow_heading(bike);
        self.roll = 0.0;
        if self.crash_view && !crashed && self.held != Some(Held::Reverse) {
            self.crash_view = false;
            (self.distance, self.pitch, self.yaw) = self.saved;
        }
        let wide = self.yaw.abs() > 2.0;
        if self.held != Some(Held::Reverse) && !self.was_wide && wide {
            self.blending = true;
        }
        // The bike's place, led along its velocity (0x52ca10).
        self.lead = bike.position;
        if !crashed && !matches!(self.held, Some(Held::Thrill(_))) {
            self.lead += bike.velocity * dt.min(0.2);
        }
        if let Some(eye) = self.frozen {
            let d = eye - self.lead;
            self.frozen_distance = (d.z * d.z + d.x * d.x + d.y).max(0.0).sqrt();
        }

        // The Helmet Cam (the fork's) has its own way.
        if self.helmet && self.frozen.is_none() && !keys.reverse && !crashed {
            let (eye, target) = self.helmet_view(bike, terrain, dt);
            self.eye_ease = Some(eases(eye, self.ease_rate()));
            self.look_ease = eases(target, self.ease_rate());
            self.fov = self.fov_ease.update(self.want_fov, dt);
            return (eye, target);
        }

        // The view keys (0x463a30): Numpad 8 and 2 tilt, 4 and 6 turn, 30°
        // a second (faster close in); a frozen camera moves 300 ft/s.
        let rider = self.view == View::Rider && self.held.is_none() && self.frozen.is_none();
        let mut moved = false;
        if rider || self.held.is_some() {
            self.settled = true;
        } else if let Some(eye) = &mut self.frozen {
            let step = dt * 300.0;
            let side = Vec3::Y.cross(self.target - *eye).normalize_or(Vec3::X);
            if keys.up {
                eye.y += step;
            }
            if keys.down && self.frozen_distance > 6.5 {
                eye.y -= step;
            }
            if keys.left {
                *eye -= side * step;
            }
            if keys.right {
                *eye += side * step;
            }
            moved = keys.up || keys.down || keys.left || keys.right;
        } else {
            let turn = (dt * 30.0).to_radians() * if self.distance < 50.0 { 3.0 - self.distance * 0.04 } else { 1.0 };
            let wrap = |a: f32| {
                use std::f32::consts::{PI, TAU};
                if a > PI { a - TAU } else if a < -PI { a + TAU } else { a }
            };
            if keys.up {
                self.pitch = (self.pitch + turn).clamp(-1.55, 1.55);
            }
            if keys.down {
                self.pitch = (self.pitch - turn).clamp(-1.55, 1.55);
            }
            if keys.left {
                self.yaw = wrap(self.yaw + turn);
            }
            if keys.right {
                self.yaw = wrap(self.yaw - turn);
            }
            moved = keys.up || keys.down || keys.left || keys.right;
        }
        // The turn the camera swings with (0x498130).
        let ratio = self.fov / Self::MAX_FOV;
        self.turn_yaw = if rider {
            ratio * turn_rate * 0.55
        } else if !wide && !bike.finished_alone {
            ratio * turn_rate * 0.42
        } else {
            0.0
        };
        // The camera follows the bike unless it is down; Sky Cam always,
        // and after a roll-over until the rider lands (0x4174b0).
        let following = self.frozen.is_some()
            || self.view == View::Sky
            || !crashed
            || (self.rolled && !bike.wreck_down);
        let still = wide || self.settled;
        let to = match self.held {
            _ if self.frozen.is_some() => {
                // The frozen camera looks 3 ft over the bike, led along the
                // way it goes, further away and the narrower the view
                // (0x498340).
                let d = self.frozen_distance;
                let k = d * 0.009 + 1.0;
                let lead = (1.0 - d * 0.000_526_315_77) * ratio * (k * k * bike.velocity.dot(self.heading) / Self::TOP_SPEED) * 35.0;
                self.lead + self.heading * lead + Vec3::Y * 3.0
            }
            Some(Held::Thrill(_)) => self.lead + Vec3::Y * 3.0,
            _ if following => self.look_ahead(bike, still, rider.then_some(bike_pitch), terrain),
            // Between the rider's head and the bike (0x416fb0).
            _ => head + (self.lead - head) * 0.38,
        };
        let eye = self.place_eye(following, !bike.wreck_bail, to, head, dt);
        let rate = self.ease_rate();
        if self.eye_ease.is_none() {
            self.eye_ease = Some(eases(eye, rate));
            self.look_ease = eases(to, rate);
        }

        if self.frozen.is_some() {
            let near = 20.0;
            let far = 280.0;
            self.want_fov = ((far - self.frozen_distance) / (far - near) * 60.0 + 10.0).clamp(10.0, 70.0);
            if moved {
                self.want_fov = (self.want_fov - self.fov) * 0.75 + self.fov;
            }
        } else if let Some(Held::Thrill(_)) = self.held {
            self.want_fov = thrill_fov(self.eye, to);
            self.fov_ease.value = self.want_fov;
        } else if following {
            if !rider {
                if self.view == View::User && !crashed {
                    self.save_user();
                }
                self.settled = self.settle(bike, eye, wide, moved);
            }
        } else if crashed {
            self.crash_view = true;
        }
        let mut eye_ease = self.eye_ease.unwrap_or_else(|| eases(eye, rate));
        let mut target = ease3(&mut self.look_ease, to, dt);
        let mut smoothed = ease3(&mut eye_ease, eye, dt);
        if self.blending {
            self.blend = (self.blend + dt).min(1.0);
            self.settled = false;
            smoothed += (eye - smoothed) * self.blend;
            if !moved && self.blend >= 1.0 {
                eye_ease = eases(eye, rate);
                self.look_ease = eases(target, rate);
                self.blend = 0.0;
                self.blending = false;
            }
        }
        if self.snap {
            self.snap = false;
            self.settled = true;
            eye_ease = eases(eye, rate);
            self.look_ease = eases(to, rate);
            smoothed = eye;
            target = to;
        } else if self.settled {
            smoothed = eye;
            target = to;
        }
        self.eye_ease = Some(eye_ease);
        // At least 3.5 ft over the ground (0x497fa0).
        let floor = ground(terrain, smoothed.x, smoothed.z) + 3.5;
        if floor > smoothed.y {
            if self.pitch < 0.0 && self.distance > 0.0 {
                self.pitch += (floor - smoothed.y) / self.distance;
            }
            smoothed.y = floor;
        }
        if rider && following {
            // The view leans with the bike (0x417340): the game's roll is
            // positive leaned right, the drawing's the other way.
            self.roll = -bike_roll.cos() * bike_roll;
        }
        self.eye = smoothed;
        self.target = target;

        // The held views take over next frame, or let go.
        let thrill = keys.thrill
            && (matches!(self.held, Some(Held::Thrill(_))) || (bike.airborne && bike.high))
            && self.frozen.is_none();
        match self.held {
            None if thrill && !self.crash_view => {
                self.before_held = (self.distance, self.pitch, self.yaw, self.want_fov);
                let spot = self.thrill_spot(terrain);
                self.held = Some(Held::Thrill(spot));
                self.eye = spot;
                self.target = self.lead + Vec3::Y * 3.0;
                self.want_fov = thrill_fov(spot, self.target);
                self.fov_ease.value = self.want_fov;
            }
            Some(Held::Thrill(_)) if !thrill => self.let_go(),
            None if keys.reverse && self.frozen.is_none() && !self.crash_view => {
                self.before_held = (self.distance, self.pitch, self.yaw, self.want_fov);
                self.held = Some(Held::Reverse);
                // 17 ft away at 0.75 rad, turned round (0x52bfa0).
                (self.distance, self.pitch, self.yaw) = (17.0, 0.75, std::f32::consts::PI);
                self.want_fov = 85.0 + wider();
                self.fov_ease.value = self.want_fov;
            }
            Some(Held::Reverse) if !keys.reverse => self.let_go(),
            _ => {}
        }
        if let Some(Held::Thrill(spot)) = self.held {
            self.eye = spot;
        }
        self.fov = self.fov_ease.update(self.want_fov, dt);
        self.was_wide = wide;
        (self.eye, self.target)
    }

    /// A held view ends: back to the view's own, snapping there next frame.
    fn let_go(&mut self) {
        let (d, p, y, fov) = self.before_held;
        (self.distance, self.pitch, self.yaw, self.want_fov) = (d, p, y, fov);
        self.fov_ease.value = fov;
        self.held = None;
        self.snap = true;
    }

    /// Whether the eye goes straight to its spot (0x52cc80): only once
    /// the yaw has turned past 2 rad and the eye is on its spot.
    fn settle(&mut self, bike: &Followed, eye: Vec3, wide: bool, moved: bool) -> bool {
        if self.held == Some(Held::Reverse) {
            return true;
        }
        let rate = self.ease_rate();
        let on = |e: &Option<[Ease; 3]>, within: f32| {
            e.is_some_and(|e| {
                (e[0].value - eye.x).abs() < within && (e[2].value - eye.z).abs() < within && (e[1].value - eye.y).abs() < within
            })
        };
        if wide {
            self.settle_checked = true;
            if bike.speed > 0.1 {
                if !moved && self.settled && on(&self.eye_ease, 10.0) {
                    self.eye_ease = Some(eases(eye, rate));
                    self.look_ease = eases(self.target, rate);
                }
                return true;
            }
            if moved {
                return false;
            }
            if !self.settled && on(&self.eye_ease, 0.01) {
                self.eye_ease = Some(eases(eye, rate));
                self.look_ease = eases(self.target, rate);
                return true;
            }
            return self.settled;
        }
        if bike.speed > 0.1 && self.settle_checked {
            if moved {
                return true;
            }
            self.settle_checked = false;
        }
        false
    }

    /// The Thrill Cam's spot (0x52bcf0): ahead of the jump and out to the
    /// side, from where and how fast the bike took off, at least 7 ft over
    /// the ground there.
    fn thrill_spot(&self, terrain: &Terrain) -> Vec3 {
        let (p, v) = self.takeoff;
        let lead = v.y * (1.0 / 32.2) * 1.5;
        let side = Vec3::new(v.z, 0.0, -v.x);
        let mut at = p + v * lead - v * 0.2 + side * 0.45;
        let mut base = p.y;
        if v.y > 10.0 {
            base += v.y * lead * 0.19;
        }
        at.y = (ground(terrain, at.x, at.z) + 7.0).max(base - 1.0);
        at
    }
}

/// The Thrill Cam's field of view from its level distance to what it
/// watches, degrees: 70° at 20 ft to 10° at 200 ft (0x52bcf0).
fn thrill_fov(eye: Vec3, at: Vec3) -> f32 {
    let d = Vec3::new(eye.x - at.x, 0.0, eye.z - at.z).length();
    mm2_physics::camera::flyby_fov(d)
}

/// The name the Helmet Cam flashes.
pub const HELMET_CAM: &str = "Helmet Cam";

impl Chase {
    /// The rider's eyes above and ahead of the bike's centre, ft: just in
    /// front of the goggles (the rider's `Head` frame sits about 5.3 to
    /// 5.5 ft up and 0.4 to 1 ft back while riding), so the bars, the
    /// gloves and the front of the bike are in view below.
    const HELMET_UP: f32 = 5.0;
    const HELMET_AHEAD: f32 = 0.1;
    /// How long the neck takes to soak up the bike's bumps, s, and how far
    /// the eye may drift from the bike up or down, ft: a little, so the
    /// bike stays put in view.
    const NECK_RISE: f32 = 0.09;
    const NECK_GIVE: f32 = 0.15;
    /// How far the look dips below the bike's own line: the bars low in
    /// view, the track well ahead above them.
    const LOOK_DOWN: f32 = 0.34;
    /// How much wider the Helmet Cam sees than the view it came from, degrees.
    const HELMET_WIDER: f32 = 12.0;
    /// How much of the bike's lean the head takes, and how fast it
    /// follows, s: nearly all of it and at once, so the head and the bike
    /// move as one.
    const LEAN_WITH_BIKE: f32 = 0.9;
    const LEAN_EASE: f32 = 0.03;
    /// The landing dip: ft for each ft/s coming down (up to `DIP_MOST`),
    /// and the neck's spring, 1/s, and damping.
    const DIP_PER_FALL: f32 = 0.004;
    const DIP_MOST: f32 = 0.15;
    const DIP_SPRING: f32 = 14.0;
    const DIP_DAMP: f32 = 0.55;

    /// The Helmet Cam: from the rider's helmet, the head held on the bike
    /// as one unit, looking where the bike points and leaning with it.
    /// Only the bumps up and down and a landing are softened a little.
    /// Only the camera changes, never the riding.
    fn helmet_view(&mut self, bike: &Followed, terrain: &Terrain, dt: f32) -> (Vec3, Vec3) {
        let yaw = Vec3::new(self.heading.x, 0.0, self.heading.z).normalize_or(Vec3::Z);
        let forward = Vec3::new(bike.forward.x, 0.0, bike.forward.z).normalize_or(yaw);
        // `forward` is level; the bike's own nose, tipped with its up.
        let nose = (forward - bike.up * forward.dot(bike.up)).normalize_or(forward);
        let want = bike.position + bike.up * Self::HELMET_UP + nose * Self::HELMET_AHEAD;
        let rise = 1.0 - (-dt / Self::NECK_RISE).exp();
        // A long way off (a restart, a reset): straight there.
        if self.head.distance(want) > 20.0 {
            self.head = want;
        }
        // Along the ground the eye stays with the head: any lag at speed
        // puts it inside the rider's own helmet.
        self.head.x = want.x;
        self.head.z = want.z;
        // Carried with the bike's climb first, so a long slope leaves no
        // lag, only the bumps, and those only a little.
        self.head.y += bike.velocity.y * dt;
        self.head.y += (want.y - self.head.y) * rise;
        self.head.y = self.head.y.clamp(want.y - Self::NECK_GIVE, want.y + Self::NECK_GIVE);
        let floor = ground(terrain, self.head.x, self.head.z) + 0.5;
        self.head.y = self.head.y.max(floor);
        // A landing: the neck gives a little and springs back.
        let (was_air, falling) = self.helmet_air;
        if bike.airborne {
            self.helmet_air = (true, falling.max(-bike.velocity.y));
        } else {
            if was_air {
                let hit = (falling * Self::DIP_PER_FALL).min(Self::DIP_MOST);
                self.dip.1 -= hit * Self::DIP_SPRING;
            }
            self.helmet_air = (false, 0.0);
        }
        let (mut d, mut v) = self.dip;
        let w = Self::DIP_SPRING;
        v += (-w * w * d - 2.0 * Self::DIP_DAMP * w * v) * dt;
        d = (d + v * dt).clamp(-Self::DIP_MOST, Self::DIP_MOST);
        self.dip = (d, v);
        // The head leans with the bike.
        let right = forward.cross(Vec3::Y);
        let lean = bike.up.dot(right).clamp(-0.9, 0.9).asin();
        self.roll_ease(lean * Self::LEAN_WITH_BIKE, dt);
        // Where the bike points, its pitch and all, a little down.
        let pitch = nose.y.clamp(-0.9, 0.9) - Self::LOOK_DOWN;
        let dir = (forward + Vec3::Y * pitch).normalize_or(forward);
        self.eye = self.head + Vec3::Y * self.dip.0;
        self.target = self.eye + dir * 30.0;
        (self.eye, self.target)
    }

    /// The head's lean eases toward `want`; `roll` is zeroed each frame
    /// for the other views, so the Helmet Cam keeps its own in `lean`.
    fn roll_ease(&mut self, want: f32, dt: f32) {
        self.lean += (want - self.lean) * (1.0 - (-dt / Self::LEAN_EASE).exp());
        self.roll = self.lean;
    }
}

/// The terrain cell under game `x`, `z` and the offset into it, in
/// terrain units.
fn cell(terrain: &Terrain, x: f32, z: f32) -> (usize, usize, f32, f32) {
    let last = terrain.size.saturating_sub(2) as f32;
    let (tx, tz) = ((x / terrain.scale).max(0.0), (z / terrain.scale).max(0.0));
    let (cx, cz) = (tx.floor().min(last), tz.floor().min(last));
    (cx as usize, cz as usize, tx - cx, tz - cz)
}

/// Terrain height in game units at game `x`, `z` (0x50d3b0). Each cell is
/// two triangles, split from (x, z) to (x+1, z+1) on cells where x + z is
/// even and from (x+1, z) to (x, z+1) where it is odd.
pub fn ground(terrain: &Terrain, x: f32, z: f32) -> f32 {
    let (cx, cz, fx, fz) = cell(terrain, x, z);
    let h = |dx: usize, dz: usize| terrain.height(cx + dx, cz + dz);
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
    y * terrain.scale
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_camera_stays_and_narrows_with_distance() {
        let terrain = Terrain {
            size: 9,
            heights: vec![0.0; 81],
            normals: vec![[0, 127, 0]; 81],
            surfaces: vec![0; 81],
            scale: 30.0,
            textures: Vec::new(),
            tiles: Vec::new(),
        };
        let mut c = Chase::new(Vec3::ZERO, Vec3::Z);
        let mut bike = Followed {
            position: Vec3::ZERO,
            forward: Vec3::Z,
            up: Vec3::Y,
            velocity: Vec3::ZERO,
            speed: 0.0,
            crashed: false,
            airborne: false,
            high: false,
            ..Default::default()
        };
        let (eye, _) = c.follow(&bike, CamKeys::default(), &terrain, 0.1);
        assert!(c.freeze());
        bike.position = Vec3::new(0.0, 0.0, 270.0);
        let (still, _) = c.follow(&bike, CamKeys::default(), &terrain, 0.1);
        assert_eq!(still, eye);
        // The look point eases over (0.3 s), the eye stays.
        let mut target = Vec3::ZERO;
        for _ in 0..40 {
            let (still, t) = c.follow(&bike, CamKeys::default(), &terrain, 0.1);
            assert_eq!(still, eye);
            target = t;
        }
        assert!(target.distance(bike.position + Vec3::Y * 3.0) < 0.01, "{target}");
        assert!(c.fov() < 15.0, "{}", c.fov());
        // The camera button lets it go, keeping the view.
        assert_eq!(c.next_view(), View::Chase.name());
        let (moved, _) = c.follow(&bike, CamKeys::default(), &terrain, 0.1);
        assert_ne!(moved, eye);
    }

    #[test]
    fn f1_goes_straight_to_the_helmet_cam_and_back() {
        let mut c = Chase::new(Vec3::ZERO, Vec3::Z);
        c.next_view();
        assert_eq!(c.toggle_helmet(), HELMET_CAM);
        assert!(c.helmet);
        assert_eq!(c.toggle_helmet(), View::Slot.name());
        assert!(!c.helmet);
        assert_eq!(c.view, View::Slot);
    }

    #[test]
    fn helmet_landing_dips_and_springs_back() {
        let terrain = Terrain {
            size: 9,
            heights: vec![0.0; 81],
            normals: vec![[0, 127, 0]; 81],
            surfaces: vec![0; 81],
            scale: 30.0,
            textures: Vec::new(),
            tiles: Vec::new(),
        };
        let mut c = Chase::new(Vec3::new(100.0, 0.0, 100.0), Vec3::Z);
        c.helmet = true;
        let mut bike = Followed {
            position: Vec3::new(100.0, 10.0, 100.0),
            forward: Vec3::Z,
            up: Vec3::Y,
            velocity: Vec3::new(0.0, -30.0, 40.0),
            speed: 50.0,
            crashed: false,
            airborne: true,
            high: true,
            ..Default::default()
        };
        c.follow(&bike, CamKeys::default(), &terrain, 1.0 / 60.0);
        bike.airborne = false;
        bike.velocity.y = 0.0;
        let mut lowest: f32 = 0.0;
        for _ in 0..120 {
            c.follow(&bike, CamKeys::default(), &terrain, 1.0 / 60.0);
            lowest = lowest.min(c.dip.0);
        }
        assert!(lowest < -0.03, "the head dips a little on landing: {lowest}");
        assert!(lowest >= -Chase::DIP_MOST, "but only a little: {lowest}");
        assert!(c.dip.0.abs() < 0.02, "and springs back: {}", c.dip.0);
    }

    #[test]
    fn helmet_looks_where_the_bike_points_and_leans_with_it() {
        let terrain = Terrain {
            size: 9,
            heights: vec![0.0; 81],
            normals: vec![[0, 127, 0]; 81],
            surfaces: vec![0; 81],
            scale: 30.0,
            textures: Vec::new(),
            tiles: Vec::new(),
        };
        let mut c = Chase::new(Vec3::new(100.0, 0.0, 100.0), Vec3::Z);
        c.helmet = true;
        // Leaning 30° into a hard turn, the heading swinging round.
        let lean = 0.5f32;
        for i in 0..60 {
            let a = i as f32 * 0.04;
            let forward = Vec3::new(a.sin(), 0.0, a.cos());
            let right = forward.cross(Vec3::Y);
            let bike = Followed {
                position: Vec3::new(100.0, 1.0, 100.0),
                forward,
                up: (Vec3::Y * lean.cos() + right * lean.sin()).normalize(),
                velocity: forward * 40.0,
                speed: 40.0,
                crashed: false,
                airborne: false,
                high: false,
                ..Default::default()
            };
            let (eye, target) = c.follow(&bike, CamKeys::default(), &terrain, 1.0 / 60.0);
            let d = target - eye;
            let level = Vec3::new(d.x, 0.0, d.z).normalize();
            assert!(level.dot(forward) > 0.9999, "the head turns with the bike, frame {i}");
        }
        assert!((c.roll() - lean * Chase::LEAN_WITH_BIKE).abs() < 0.01, "and leans with it: {}", c.roll());
    }
}

#[cfg(test)]
mod trace {
    use super::*;

    /// The original's chase camera on a straight run from rest
    /// (/mnt/project-files/camera_vs_speed_noframes.txt): time, speed,
    /// eye behind the bike, ft, and the look's pitch down, degrees.
    const ORIGINAL: [(f32, f32, f32, f32); 26] = [
        (0.0, 0.0, 5.87, 28.44),
        (0.2, 7.8, 6.60, 27.60),
        (0.4, 17.2, 8.31, 24.75),
        (0.7, 28.3, 10.58, 21.50),
        (0.9, 40.8, 13.41, 18.40),
        (1.1, 43.9, 15.93, 16.40),
        (1.3, 49.2, 17.43, 15.17),
        (1.5, 53.8, 19.80, 14.20),
        (1.7, 57.6, 21.08, 13.47),
        (1.9, 60.9, 21.49, 12.90),
        (2.1, 64.3, 23.00, 12.42),
        (2.4, 67.3, 23.56, 12.01),
        (2.6, 70.3, 25.52, 11.66),
        (2.8, 73.0, 26.17, 11.35),
        (3.0, 75.4, 26.44, 11.08),
        (3.2, 77.8, 27.41, 10.83),
        (3.4, 80.1, 28.44, 10.61),
        (3.6, 82.1, 28.19, 10.41),
        (3.8, 84.2, 28.66, 10.22),
        (4.0, 86.2, 30.10, 10.05),
        (4.2, 87.9, 30.11, 9.89),
        (4.4, 90.0, 31.10, 9.74),
        (4.6, 90.8, 31.33, 9.60),
        (4.9, 89.6, 31.86, 9.56),
        (5.1, 88.5, 30.95, 9.58),
        (5.3, 87.3, 31.83, 9.59),
    ];

    fn speed_at(t: f32) -> f32 {
        let i = ORIGINAL.iter().rposition(|r| r.0 <= t).unwrap_or(0);
        let (a, b) = (ORIGINAL[i], ORIGINAL[(i + 1).min(ORIGINAL.len() - 1)]);
        if b.0 <= a.0 { a.1 } else { a.1 + (b.1 - a.1) * (t - a.0) / (b.0 - a.0) }
    }

    /// Rides the trace at `fps` and gives the rms differences: behind,
    /// ft, and pitch, degrees.
    fn ride(fps: f32, show: bool) -> (f32, f32) {
        let terrain = Terrain {
            size: 9,
            heights: vec![0.0; 81],
            normals: vec![[0, 127, 0]; 81],
            surfaces: vec![0; 81],
            scale: 300.0,
            textures: Vec::new(),
            tiles: Vec::new(),
        };
        let start = Vec3::new(100.0, 0.0, 100.0);
        let mut c = Chase::new(start, Vec3::Z);
        c.fov = 85.0;
        c.fov_ease.value = 85.0;
        let dt = 1.0 / fps;
        let mut z: f32 = 0.0;
        let mut t: f32 = -1.0;
        let (mut eb, mut ep, mut n) = (0.0, 0.0, 0.0);
        let mut next = 0;
        while next < ORIGINAL.len() {
            let v = speed_at(t.max(0.0));
            z += v * dt;
            t += dt;
            let bike = Followed {
                position: start + Vec3::Z * z,
                forward: Vec3::Z,
                nose: Vec3::Z,
                up: Vec3::Y,
                velocity: Vec3::Z * v,
                speed: v,
                turn_rate: Some(0.0),
                ..Default::default()
            };
            let (eye, target) = c.follow(&bike, CamKeys::default(), &terrain, dt);
            if t >= ORIGINAL[next].0 {
                let behind = bike.position.z - eye.z;
                let d = target - eye;
                let pitch = (-d.y).atan2(d.z).to_degrees();
                let (_, _, ob, op) = ORIGINAL[next];
                if show {
                    eprintln!("{t:4.1} {v:5.1}  behind {behind:5.2} (orig {ob:5.2})  pitch {pitch:5.2} (orig {op:5.2})");
                }
                eb += (behind - ob).powi(2);
                ep += (pitch - op).powi(2);
                n += 1.0;
                next += 1;
            }
        }
        ((eb / n).sqrt(), (ep / n).sqrt())
    }

    fn flat() -> Terrain {
        Terrain {
            size: 9,
            heights: vec![0.0; 81],
            normals: vec![[0, 127, 0]; 81],
            surfaces: vec![0; 81],
            scale: 300.0,
            textures: Vec::new(),
            tiles: Vec::new(),
        }
    }

    #[test]
    fn a_crash_is_watched_from_the_side() {
        let terrain = flat();
        let at = Vec3::new(500.0, 0.0, 500.0);
        let mut c = Chase::new(at, Vec3::Z);
        let mut bike = Followed {
            position: at,
            forward: Vec3::Z,
            nose: Vec3::Z,
            up: Vec3::Y,
            head: Some(at + Vec3::Y * 5.3),
            ..Default::default()
        };
        for _ in 0..60 {
            c.follow(&bike, CamKeys::default(), &terrain, 1.0 / 60.0);
        }
        bike.crashed = true;
        bike.wreck_down = true;
        let mut eye = Vec3::ZERO;
        for _ in 0..240 {
            eye = c.follow(&bike, CamKeys::default(), &terrain, 1.0 / 60.0).0;
        }
        let d = Vec3::new(eye.x - at.x, 0.0, eye.z - at.z);
        // The view's 11.72 ft off, 60° round from behind (+0x2e8, 0x464b30),
        // 7 ft over the point between the head and the bike.
        assert!((d.length() - 11.72).abs() < 0.1, "{eye}");
        let round = d.x.abs().atan2(-d.z).to_degrees();
        assert!((round - 60.0).abs() < 1.0, "{round}° round");
        assert!((eye.y - (5.3 * 0.62 + 7.0)).abs() < 0.5, "{eye}");
        // And back behind once the bike is up again.
        bike.crashed = false;
        for _ in 0..240 {
            eye = c.follow(&bike, CamKeys::default(), &terrain, 1.0 / 60.0).0;
        }
        assert!((at.z - eye.z - 5.86).abs() < 0.1 && (eye.x - at.x).abs() < 0.1, "{eye}");
    }

    #[test]
    fn a_roll_over_is_followed_until_the_rider_lands() {
        let terrain = flat();
        let at = Vec3::new(500.0, 0.0, 500.0);
        let mut c = Chase::new(at, Vec3::Z);
        let mut bike = Followed {
            position: at,
            forward: Vec3::Z,
            nose: Vec3::Z,
            up: Vec3::Y,
            head: Some(at + Vec3::Y * 5.3),
            ..Default::default()
        };
        for _ in 0..60 {
            c.follow(&bike, CamKeys::default(), &terrain, 1.0 / 60.0);
        }
        // Rolled 120° onto its side and over.
        let roll = 2.1f32;
        bike.up = Vec3::new(roll.sin(), roll.cos(), 0.0);
        bike.crashed = true;
        bike.flipped = true;
        let mut eye = Vec3::ZERO;
        for _ in 0..120 {
            eye = c.follow(&bike, CamKeys::default(), &terrain, 1.0 / 60.0).0;
        }
        assert!((at.z - eye.z - 5.86).abs() < 0.1 && (eye.x - at.x).abs() < 0.1, "still behind: {eye}");
        // The rider lands: the crash view takes over, 60° round, and from
        // the far side as the bike lies upside down (+0x464).
        bike.wreck_down = true;
        for _ in 0..240 {
            eye = c.follow(&bike, CamKeys::default(), &terrain, 1.0 / 60.0).0;
        }
        let d = Vec3::new(eye.x - at.x, 0.0, eye.z - at.z);
        assert!((d.x.abs().atan2(-d.z).to_degrees() - 120.0).abs() < 1.0, "{eye}");
    }

    #[test]
    fn the_rider_cam_is_the_riders_head_leaning_with_the_bike() {
        let terrain = flat();
        let at = Vec3::new(500.0, 0.0, 500.0);
        let mut c = Chase::new(at, Vec3::Z);
        for _ in 0..3 {
            c.next_view();
        }
        assert_eq!(c.view, View::Rider);
        let head = at + Vec3::new(0.2, 5.4, -0.6);
        let lean = 0.4f32;
        let bike = Followed {
            position: at,
            forward: Vec3::Z,
            nose: Vec3::Z,
            up: Vec3::new(lean.sin(), lean.cos(), 0.0),
            velocity: Vec3::Z * 40.0,
            speed: 40.0,
            head: Some(head),
            ..Default::default()
        };
        let (eye, _) = c.follow(&bike, CamKeys::default(), &terrain, 1.0 / 60.0);
        assert_eq!(eye, head);
        // Leaned right in the game (+x), the drawing's roll is negative.
        assert!((c.roll() + lean.cos() * lean).abs() < 1e-4, "{}", c.roll());
        // Its field of view is the camera's first, 77°.
        for _ in 0..120 {
            c.follow(&bike, CamKeys::default(), &terrain, 1.0 / 60.0);
        }
        assert!((c.fov() - 77.0 - wider()).abs() < 0.1, "{}", c.fov());
    }

    #[test]
    fn chase_cam_trails_as_the_original_does() {
        // The game leads the look by a frame's travel, so at 30 fps it
        // trails 2.5 ft nearer than recorded; 60 and up match.
        for fps in [60.0, 144.0] {
            let (b, p) = ride(fps, std::env::var_os("MM2_SHOW").is_some());
            eprintln!("{fps} fps: behind rms {b:.2} ft, pitch rms {p:.2}°");
            assert!(b < 1.2 && p < 0.5, "{fps} fps: behind {b}, pitch {p}");
        }
    }
}
