//! A free-flying camera.

use glam::{Mat4, Vec3};

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub position: Vec3,
    /// Radians around +y; 0 looks along +x.
    pub yaw: f32,
    /// Radians up from the horizon.
    pub pitch: f32,
    /// The field of view across a 4:3 screen, degrees (the game's own
    /// measure; wider windows keep the height, up to 21:9).
    pub fov: f32,
    /// Radians the view tilts about the way it looks (the Helmet Cam
    /// leaning with the bike); 0 keeps the horizon level.
    pub roll: f32,
}

/// The game's usual field of view, degrees.
pub const FOV: f32 = 85.0;

/// tan(half the view up the screen) for `fov` degrees across a 4:3 screen
/// in a window `aspect` wide over high: wider windows keep the height, up
/// to 21:9 (wider ones are drawn between black bars, see [`crate::bars`]).
pub fn half_height(fov: f32, aspect: f32) -> f32 {
    let half = (fov.to_radians() / 2.0).tan() * 0.75;
    if aspect > crate::bars::WIDEST {
        half * crate::bars::WIDEST / aspect
    } else {
        half
    }
}

impl Camera {
    /// A camera at `eye` looking at `target`.
    pub fn looking_at(eye: Vec3, target: Vec3) -> Self {
        let d = (target - eye).normalize_or(Vec3::X);
        Camera {
            position: eye,
            yaw: d.z.atan2(d.x),
            pitch: d.y.clamp(-1.0, 1.0).asin(),
            fov: FOV,
            roll: 0.0,
        }
    }

    /// The field of view up a window `aspect` wide over high, radians.
    pub fn fovy(&self, aspect: f32) -> f32 {
        2.0 * half_height(self.fov, aspect).atan()
    }

    pub fn forward(&self) -> Vec3 {
        Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
    }

    pub fn right(&self) -> Vec3 {
        self.forward().cross(Vec3::Y).normalize()
    }

    pub fn view(&self) -> Mat4 {
        let up = if self.roll == 0.0 {
            Vec3::Y
        } else {
            glam::Quat::from_axis_angle(self.forward(), self.roll) * Vec3::Y
        };
        Mat4::look_to_rh(self.position, self.forward(), up)
    }

    pub fn look(&mut self, dx: f32, dy: f32) {
        self.yaw += dx;
        self.pitch = (self.pitch - dy).clamp(-1.55, 1.55);
    }
}
