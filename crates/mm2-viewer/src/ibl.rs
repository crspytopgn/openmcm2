//! Image-based light for the enhanced drawing: what the sky round about
//! gives a surface, none of it the game's.
//!
//! A small cube map of the sky (the scene's own sky box, or the real sky's
//! colours for the hour and weather) with the ground below it, worked out
//! on the CPU when a track loads and again when its light changes (the
//! hour, the weather, a stadium's night), never per frame:
//!
//! - its radiance, blurred for rougher and rougher surfaces down its mips
//!   (GGX, importance sampled, as Karis 2013 prefilters), which the shaders
//!   read for what a surface reflects;
//! - its irradiance as nine spherical harmonics (Ramamoorthi and Hanrahan
//!   2001), for the light a rough surface takes from the sky all round.
//!
//! Done on the CPU, it needs nothing a browser's WebGPU or an old OpenGL
//! lacks, and costs a few milliseconds a change. Linear colour throughout,
//! in the drawing's own units: a white matte surface lit by the sky shows
//! its irradiance's colour.

use glam::Vec3;

/// Texels a side of the cube's sharpest level, and how many levels down
/// it (each half the last), the last the roughest.
pub const SIDE: u32 = 32;
pub const LEVELS: u32 = 5;
/// The cube's format: half floats, for a sky brighter than white.
pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// Where the sky's colours come from.
#[derive(Clone)]
pub enum Source {
    /// The scene's own sky box: six faces of `side`² RGBA8 (sRGB), laid
    /// out as `sky::corner` lays them, in game coordinates; texels with no
    /// tile have alpha 0.
    Painted { side: u32, faces: Vec<u8> },
    /// The real sky's colours (enhance::daylight), linear.
    Real,
    /// A stadium: under a roof, the stands all round.
    Indoors,
}

/// The light the cube is made for.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Light {
    /// Toward the sun (drawing coordinates), and its colour and the
    /// ambient's as the game gives them (0 to 1, not linear).
    pub to_sun: Vec3,
    pub sun: Vec3,
    pub ambient: Vec3,
    /// The sky's colours overhead and at the horizon (linear).
    pub zenith: Vec3,
    pub horizon: Vec3,
    /// The haze's colour (the game's, 0 to 1), what shows where a sky box
    /// has no tile.
    pub fog: Vec3,
    /// How much of the sky is cloud, and how much it is night.
    pub cover: f32,
    pub night: f32,
}

impl Light {
    /// Whether `other` is close enough to need no new cube.
    pub fn near(&self, other: &Light) -> bool {
        let close = |a: Vec3, b: Vec3| (a - b).abs().max_element() < 0.01;
        close(self.to_sun, other.to_sun)
            && close(self.sun, other.sun)
            && close(self.ambient, other.ambient)
            && close(self.zenith, other.zenith)
            && close(self.horizon, other.horizon)
            && close(self.fog, other.fog)
            && (self.cover - other.cover).abs() < 0.01
            && (self.night - other.night).abs() < 0.01
    }
}

/// The cube worked out: each level's six faces (+x, -x, +y, -y, +z, -z,
/// drawing coordinates, as the GPU takes a cube), RGB, and the irradiance's
/// harmonics, ready for `irradiance` in `enhanced_fns.wgsl`.
pub struct Baked {
    pub levels: Vec<Vec<[f32; 3]>>,
    pub sh: [[f32; 4]; 9],
}

fn to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The light's linear parts, as the enhanced drawing's PBR uses them: the
/// ambient's, and the sun's on a surface square on to it, so that a white
/// matte surface comes out as the game's own light (`(ambient + sun)`
/// taken to linear) would draw it.
pub fn linear_light(sun: Vec3, ambient: Vec3) -> (Vec3, Vec3) {
    let lin = |c: Vec3| c.max(Vec3::ZERO).powf(2.2);
    let a = lin(ambient);
    (lin(ambient + sun) - a, a)
}

fn luminance(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

/// Face `face`'s direction through texel (`x`, `y`) of `side`
/// (drawing coordinates, the GPU's cube layout).
pub fn direction(face: usize, x: f32, y: f32, side: u32) -> Vec3 {
    let u = 2.0 * (x + 0.5) / side as f32 - 1.0;
    let v = 2.0 * (y + 0.5) / side as f32 - 1.0;
    let d = match face {
        0 => Vec3::new(1.0, -v, -u),
        1 => Vec3::new(-1.0, -v, u),
        2 => Vec3::new(u, 1.0, v),
        3 => Vec3::new(u, -1.0, -v),
        4 => Vec3::new(u, -v, 1.0),
        _ => Vec3::new(-u, -v, -1.0),
    };
    d.normalize()
}

/// The face and texel (0 to 1 across, down) a direction falls on.
fn texel_of(d: Vec3) -> (usize, f32, f32) {
    let a = d.abs();
    let (face, u, v, m) = if a.x >= a.y && a.x >= a.z {
        if d.x > 0.0 {
            (0, -d.z, -d.y, a.x)
        } else {
            (1, d.z, -d.y, a.x)
        }
    } else if a.y >= a.z {
        if d.y > 0.0 {
            (2, d.x, d.z, a.y)
        } else {
            (3, d.x, -d.z, a.y)
        }
    } else if d.z > 0.0 {
        (4, d.x, -d.y, a.z)
    } else {
        (5, -d.x, -d.y, a.z)
    };
    (face, (u / m + 1.0) * 0.5, (v / m + 1.0) * 0.5)
}

/// What a painted sky box shows toward `g` (game coordinates): its colour
/// (linear), or none where it has no tile.
fn painted_at(side: u32, faces: &[u8], g: Vec3) -> Option<Vec3> {
    let a = g.abs();
    // The faces as sky::corner lays them: (u, v) across from the left and
    // up from the bottom, -1 to 1.
    let (face, u, v) = if a.z >= a.x && a.z >= a.y {
        if g.z > 0.0 {
            (0, g.x / a.z, g.y / a.z)
        } else {
            (2, -g.x / a.z, g.y / a.z)
        }
    } else if a.x >= a.y {
        if g.x < 0.0 {
            (1, g.z / a.x, g.y / a.x)
        } else {
            (3, -g.z / a.x, g.y / a.x)
        }
    } else if g.y > 0.0 {
        (4, g.x / a.y, -g.z / a.y)
    } else {
        (5, g.x / a.y, g.z / a.y)
    };
    let s = side as usize;
    let x = (((u + 1.0) * 0.5 * side as f32) as usize).min(s - 1);
    let y = (((1.0 - v) * 0.5 * side as f32) as usize).min(s - 1);
    let p = &faces[(face * s * s + y * s + x) * 4..][..4];
    (p[3] > 127).then(|| Vec3::new(to_linear(p[0]), to_linear(p[1]), to_linear(p[2])))
}

/// A painted sky box's faces shrunk to `side`² each, for the cube's
/// source (the cube needs no more than a few texels a degree).
pub fn shrink(side: u32, faces: &[u8], to: u32) -> Source {
    let to = to.min(side).max(1);
    let (s, t) = (side as usize, to as usize);
    let step = s / t;
    let mut out = vec![0u8; 6 * t * t * 4];
    for f in 0..6 {
        for y in 0..t {
            for x in 0..t {
                let mut sum = [0u32; 4];
                let mut kept = 0;
                for dy in 0..step {
                    for dx in 0..step {
                        let p = &faces[(f * s * s + (y * step + dy) * s + x * step + dx) * 4..][..4];
                        if p[3] > 127 {
                            for k in 0..3 {
                                sum[k] += p[k] as u32;
                            }
                            kept += 1;
                        }
                    }
                }
                let o = &mut out[(f * t * t + y * t + x) * 4..][..4];
                if kept * 2 >= step * step {
                    for k in 0..3 {
                        o[k] = (sum[k] / kept as u32) as u8;
                    }
                    o[3] = 255;
                }
            }
        }
    }
    Source::Painted { side: to, faces: out }
}

/// What the sky shows toward `d` (drawing coordinates), linear, before it
/// is scaled to the scene's light; `None` below the horizon where the
/// ground is put in.
fn sky_at(source: &Source, light: &Light, d: Vec3) -> Option<Vec3> {
    let up = d.y;
    match source {
        Source::Painted { side, faces } => {
            let g = Vec3::new(d.x, d.y, -d.z);
            match painted_at(*side, faces, g) {
                Some(c) => Some(c),
                // No tile: the clear colour, the haze's, shows there.
                None if up > -0.02 => Some(light.fog.max(Vec3::ZERO).powf(2.2)),
                None => None,
            }
        }
        Source::Real => {
            if up < -0.02 {
                return None;
            }
            let t = up.max(0.0).sqrt();
            let mut c = light.horizon.lerp(light.zenith, t);
            // The sun's glow, and the bright sky round it.
            let toward = d.dot(light.to_sun).max(0.0);
            let glow = toward.powf(8.0) * 0.6 + toward.powf(64.0) * 2.0;
            c += Vec3::new(1.0, 0.85, 0.65) * glow * (1.0 - light.cover * 0.8) * (1.0 - light.night);
            Some(c)
        }
        Source::Indoors => {
            // The roof's dark, the stands' painted light round about.
            let a = light.ambient.max(Vec3::ZERO).powf(2.2);
            Some(a * if up > 0.5 { 0.6 } else { 1.0 })
        }
    }
}

/// The cube for `light` from `source`, at `side` texels a side.
pub fn bake(source: &Source, light: &Light, side: u32) -> Baked {
    let side = side.max(4);
    // The sky first, to scale it to the scene's ambient light: a white
    // matte surface facing up takes as much from it as from the game's
    // ambient (a little more, as the old lighting's sky was brighter than
    // its ground), whatever the picture's own brightness.
    let n = (side * side) as usize;
    let mut sky: Vec<Option<Vec3>> = Vec::with_capacity(6 * n);
    for face in 0..6 {
        for y in 0..side {
            for x in 0..side {
                sky.push(sky_at(source, light, direction(face, x as f32, y as f32, side)));
            }
        }
    }
    let (sun_lin, amb_lin) = linear_light(light.sun, light.ambient);
    // The sky's irradiance (over pi) on a surface facing up.
    let mut up_sum = Vec3::ZERO;
    let mut weight_sum = 0.0;
    for face in 0..6 {
        for y in 0..side {
            for x in 0..side {
                let d = direction(face, x as f32, y as f32, side);
                let w = solid_angle(x, y, side);
                if let Some(c) = sky[face * n + (y * side + x) as usize] {
                    if d.y > 0.0 {
                        up_sum += c * d.y * w;
                    }
                }
                if d.y > 0.0 {
                    weight_sum += d.y * w;
                }
            }
        }
    }
    let up_irr = up_sum / weight_sum.max(1e-6);
    // Keep the sky's colour, at the ambient's brightness.
    let target = luminance(amb_lin) * 1.15;
    let scale = target / luminance(up_irr).max(1e-4);
    // The ground: lit by that sky and the sun (less of it: what stands
    // round about shades some), a dusty mid-brown. Out in the sun it is
    // about as bright as the sky, as real ground is.
    let ground_albedo = Vec3::new(0.2, 0.17, 0.13);
    let ground = ground_albedo * (amb_lin * 1.15 + sun_lin * light.to_sun.y.max(0.0) * 0.6);
    let level0: Vec<Vec3> = (0..6 * n)
        .map(|i| {
            let face = i / n;
            let (x, y) = ((i % n) as u32 % side, (i % n) as u32 / side);
            let d = direction(face, x as f32, y as f32, side);
            match sky[i] {
                Some(c) => {
                    // The sky box's painted hills meet the ground softly.
                    let c = c * scale;
                    if d.y < 0.0 {
                        c.lerp(ground, (-d.y / 0.25).clamp(0.0, 1.0))
                    } else {
                        c
                    }
                }
                None => ground,
            }
        })
        .collect();
    let sh = harmonics(&level0, side);
    // Box-filtered mips of the sharp cube, for the prefilter to read from
    // without aliasing.
    let mut pyramid = vec![(side, level0.clone())];
    while pyramid.last().unwrap().0 > 1 {
        let (s, last) = pyramid.last().unwrap();
        let h = s / 2;
        let mut next = Vec::with_capacity(6 * (h * h) as usize);
        for face in 0..6 {
            for y in 0..h {
                for x in 0..h {
                    let at = |dx: u32, dy: u32| last[face * (s * s) as usize + ((2 * y + dy) * s + 2 * x + dx) as usize];
                    next.push((at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1)) * 0.25);
                }
            }
        }
        pyramid.push((h, next));
    }
    let mut levels = vec![level0];
    for level in 1..LEVELS {
        let s = (side >> level).max(1);
        let roughness = level as f32 / (LEVELS - 1) as f32;
        let mut out = Vec::with_capacity(6 * (s * s) as usize);
        for face in 0..6 {
            for y in 0..s {
                for x in 0..s {
                    out.push(prefilter(&pyramid, direction(face, x as f32, y as f32, s), roughness));
                }
            }
        }
        levels.push(out);
    }
    Baked {
        levels: levels.into_iter().map(|l| l.into_iter().map(|c| c.into()).collect()).collect(),
        sh,
    }
}

/// The solid angle of texel (`x`, `y`) of a face `side` across.
fn solid_angle(x: u32, y: u32, side: u32) -> f32 {
    let u = 2.0 * (x as f32 + 0.5) / side as f32 - 1.0;
    let v = 2.0 * (y as f32 + 0.5) / side as f32 - 1.0;
    let d = 1.0 + u * u + v * v;
    4.0 / (side * side) as f32 / (d * d.sqrt())
}

fn radical_inverse(mut i: u32) -> f32 {
    i = i.reverse_bits();
    i as f32 * 2.328_306_4e-10
}

/// The cube toward `n` as a surface of `roughness` facing it sees it
/// (n = v = r), from the pyramid level fitting each sample's footprint.
fn prefilter(pyramid: &[(u32, Vec<Vec3>)], n: Vec3, roughness: f32) -> Vec3 {
    const SAMPLES: u32 = 64;
    let a = (roughness * roughness).max(1e-3);
    let a2 = a * a;
    let up = if n.y.abs() < 0.999 { Vec3::Y } else { Vec3::X };
    let tx = up.cross(n).normalize();
    let ty = n.cross(tx);
    let side0 = pyramid[0].0 as f32;
    let texel = 4.0 * std::f32::consts::PI / (6.0 * side0 * side0);
    let mut sum = Vec3::ZERO;
    let mut weight = 0.0;
    for i in 0..SAMPLES {
        let (e1, e2) = (i as f32 / SAMPLES as f32, radical_inverse(i));
        let phi = 2.0 * std::f32::consts::PI * e1;
        let cos_t = ((1.0 - e2) / (1.0 + (a2 - 1.0) * e2)).sqrt();
        let sin_t = (1.0 - cos_t * cos_t).max(0.0).sqrt();
        let h = (tx * (phi.cos() * sin_t) + ty * (phi.sin() * sin_t) + n * cos_t).normalize();
        let l = 2.0 * n.dot(h) * h - n;
        let nl = n.dot(l);
        if nl <= 0.0 {
            continue;
        }
        // The sample's share of the sphere (pdf of GGX's half vectors, as
        // seen from n = v), and so which level it reads.
        let nh = n.dot(h).max(0.0);
        let dd = nh * nh * (a2 - 1.0) + 1.0;
        let pdf = a2 / (std::f32::consts::PI * dd * dd) / 4.0;
        let share = 1.0 / (SAMPLES as f32 * pdf + 1e-4);
        let lod = (0.5 * (share / texel).log2() + 1.0).clamp(0.0, (pyramid.len() - 1) as f32);
        let (s, texels) = &pyramid[lod.round() as usize];
        let (face, u, v) = texel_of(l);
        let x = ((u * *s as f32) as u32).min(s - 1);
        let y = ((v * *s as f32) as u32).min(s - 1);
        sum += texels[face * (s * s) as usize + (y * s + x) as usize] * nl;
        weight += nl;
    }
    sum / weight.max(1e-6)
}

/// The irradiance (over pi) of `cube` as nine harmonics, each already
/// carrying its basis' constant and its band's cosine convolution, so the
/// shader takes the sum of each times its polynomial: 1, y, z, x, xy, yz,
/// 3z²-1, xz, x²-y².
fn harmonics(cube: &[Vec3], side: u32) -> [[f32; 4]; 9] {
    let mut l = [Vec3::ZERO; 9];
    let n = (side * side) as usize;
    for face in 0..6 {
        for y in 0..side {
            for x in 0..side {
                let d = direction(face, x as f32, y as f32, side);
                let w = solid_angle(x, y, side);
                let c = cube[face * n + (y * side + x) as usize] * w;
                let basis = [
                    0.282095,
                    0.488603 * d.y,
                    0.488603 * d.z,
                    0.488603 * d.x,
                    1.092548 * d.x * d.y,
                    1.092548 * d.y * d.z,
                    0.315392 * (3.0 * d.z * d.z - 1.0),
                    1.092548 * d.x * d.z,
                    0.546274 * (d.x * d.x - d.y * d.y),
                ];
                for k in 0..9 {
                    l[k] += c * basis[k];
                }
            }
        }
    }
    // Each band's cosine lobe (pi, 2pi/3, pi/4), over pi, and the basis'
    // constants again for the shader's polynomials.
    let band = [1.0, 2.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0, 0.25, 0.25, 0.25, 0.25, 0.25];
    let constant = [0.282095, 0.488603, 0.488603, 0.488603, 1.092548, 1.092548, 0.315392, 1.092548, 0.546274];
    let mut out = [[0.0; 4]; 9];
    for k in 0..9 {
        out[k] = (l[k] * band[k] * constant[k]).extend(0.0).into();
    }
    out
}

/// The irradiance (over pi) toward `n` from harmonics made by [`bake`],
/// as the shader works it out.
#[cfg(test)]
pub fn irradiance(sh: &[[f32; 4]; 9], n: Vec3) -> Vec3 {
    let p = [1.0, n.y, n.z, n.x, n.x * n.y, n.y * n.z, 3.0 * n.z * n.z - 1.0, n.x * n.z, n.x * n.x - n.y * n.y];
    (0..9).map(|k| Vec3::new(sh[k][0], sh[k][1], sh[k][2]) * p[k]).sum()
}

/// `x` as a half float's bits.
fn half(x: f32) -> u16 {
    let bits = x.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let x = x.abs().min(65504.0);
    if x < 6.103_515_6e-5 {
        // Too small for a normal half: flushed to zero.
        return sign;
    }
    let bits = x.to_bits();
    let exp = ((bits >> 23) & 0xff) as i32 - 127 + 15;
    let mantissa = bits & 0x7f_ffff;
    // Rounded to nearest.
    let mut h = ((exp as u32) << 10) | (mantissa >> 13);
    if mantissa & 0x1000 != 0 {
        h += 1;
    }
    sign | h as u16
}

/// The cube's levels as the GPU takes them: each level's faces in turn,
/// RGBA half floats.
pub fn texels(baked: &Baked) -> Vec<u8> {
    // wgpu wants mip-major for a cube's layers: layer by layer, each with
    // all its levels (TextureDataOrder::LayerMajor).
    let side = (baked.levels[0].len() as f32 / 6.0).sqrt() as usize;
    let mut out = Vec::new();
    for face in 0..6 {
        for (level, texels) in baked.levels.iter().enumerate() {
            let s = (side >> level).max(1);
            for c in &texels[face * s * s..(face + 1) * s * s] {
                for k in [c[0], c[1], c[2], 1.0] {
                    out.extend_from_slice(&half(k).to_le_bytes());
                }
            }
        }
    }
    out
}

/// The GPU's cube for `baked`.
pub fn upload(device: &wgpu::Device, queue: &wgpu::Queue, baked: &Baked) -> wgpu::TextureView {
    use wgpu::util::DeviceExt;
    let side = (baked.levels[0].len() as f32 / 6.0).sqrt() as u32;
    device
        .create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("sky's light"),
                size: wgpu::Extent3d {
                    width: side,
                    height: side,
                    depth_or_array_layers: 6,
                },
                mip_level_count: baked.levels.len() as u32,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &texels(baked),
        )
        .create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn light() -> Light {
        Light {
            to_sun: Vec3::new(0.3, 0.8, 0.2).normalize(),
            sun: Vec3::splat(0.7),
            ambient: Vec3::splat(0.4),
            zenith: Vec3::new(0.13, 0.27, 0.62),
            horizon: Vec3::new(0.58, 0.68, 0.82),
            fog: Vec3::splat(0.7),
            cover: 0.4,
            night: 0.0,
        }
    }

    #[test]
    fn faces_and_texels_agree() {
        for face in 0..6 {
            for (x, y) in [(0.0, 0.0), (5.0, 9.0), (31.0, 31.0)] {
                let d = direction(face, x, y, 32);
                let (f, u, v) = texel_of(d);
                assert_eq!(f, face);
                assert!((u * 32.0 - (x + 0.5)).abs() < 1e-3 && (v * 32.0 - (y + 0.5)).abs() < 1e-3);
            }
        }
    }

    #[test]
    fn a_white_surface_facing_up_takes_the_ambient() {
        let baked = bake(&Source::Real, &light(), 16);
        let (_, amb) = linear_light(light().sun, light().ambient);
        let up = irradiance(&baked.sh, Vec3::Y);
        let down = irradiance(&baked.sh, -Vec3::Y);
        // The harmonics' ringing blurs it a little.
        assert!((luminance(up) / (luminance(amb) * 1.15) - 1.0).abs() < 0.12, "{up} {amb}");
        // The sky is bluer and brighter than the ground.
        assert!(up.z > up.x && down.x > down.z && luminance(down) < luminance(up), "{up} {down}");
        assert_eq!(baked.levels.len(), LEVELS as usize);
        assert_eq!(texels(&baked).len(), 6 * (16 * 16 + 8 * 8 + 4 * 4 + 2 * 2 + 1) * 8);
    }

    #[test]
    fn halves() {
        assert_eq!(half(1.0), 0x3c00);
        assert_eq!(half(-2.0), 0xc000);
        assert_eq!(half(0.5), 0x3800);
        assert_eq!(half(0.0), 0);
    }
}
