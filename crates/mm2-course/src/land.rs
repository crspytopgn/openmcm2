//! The land under the track: each [`LandKind`] is a recipe of smooth
//! noise, made the same way every time from its seed.

use crate::{Land, LandKind};

fn hash(x: i32, z: i32, seed: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9e37_79b1)
        ^ (z as u32).wrapping_mul(0x85eb_ca77)
        ^ seed.wrapping_mul(0xc2b2_ae3d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a_2d39);
    h ^ (h >> 15)
}

/// Gradient noise, about -1 to 1.
pub fn noise(x: f32, z: f32, seed: u32) -> f32 {
    let (xi, zi) = (x.floor() as i32, z.floor() as i32);
    let (fx, fz) = (x - xi as f32, z - zi as f32);
    let grad = |ix: i32, iz: i32, dx: f32, dz: f32| {
        let a = (hash(ix, iz, seed) & 0xffff) as f32 / 65535.0 * std::f32::consts::TAU;
        a.cos() * dx + a.sin() * dz
    };
    let s = |t: f32| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let (sx, sz) = (s(fx), s(fz));
    let a = grad(xi, zi, fx, fz) * (1.0 - sx) + grad(xi + 1, zi, fx - 1.0, fz) * sx;
    let b =
        grad(xi, zi + 1, fx, fz - 1.0) * (1.0 - sx) + grad(xi + 1, zi + 1, fx - 1.0, fz - 1.0) * sx;
    (a * (1.0 - sz) + b * sz) * 1.414
}

/// Layered noise, about -1 to 1.
pub fn fbm(x: f32, z: f32, seed: u32, octaves: u32) -> f32 {
    let (mut sum, mut amp, mut f, mut total) = (0.0, 1.0, 1.0, 0.0);
    for o in 0..octaves {
        sum += noise(x * f, z * f, seed.wrapping_add(o * 101)) * amp;
        total += amp;
        amp *= 0.5;
        f *= 2.03;
    }
    sum / total
}

/// Sharp-crested noise for mountains and dunes, 0 to 1.
fn ridged(x: f32, z: f32, seed: u32, octaves: u32) -> f32 {
    let (mut sum, mut amp, mut f, mut total) = (0.0, 1.0, 1.0, 0.0);
    for o in 0..octaves {
        let n = 1.0 - noise(x * f, z * f, seed.wrapping_add(o * 57)).abs();
        sum += n * n * amp;
        total += amp;
        amp *= 0.5;
        f *= 2.1;
    }
    sum / total
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The lowest the land goes, ft: room for dips below the track.
pub const BASE: f32 = 40.0;

/// The land's height at (x, z), ft, on a map `side` ft across.
pub fn height(land: &Land, side: f32, x: f32, z: f32) -> f32 {
    let seed = land.seed;
    let c = side / 2.0;
    let (u, v) = ((x - c) / c, (z - c) / c);
    let hills = land.hills.clamp(0.0, 1.0);
    let rough = land.rough.clamp(0.0, 1.0);
    // Fine bumps everywhere, which the track bed smooths out.
    let detail = fbm(x / 90.0, z / 90.0, seed ^ 0x55, 3) * rough * 5.0;
    // Hills round the edge hide the end of the world: a ring, not a box,
    // its edge wandering.
    let r = (u * u + v * v).sqrt() * 0.75
        + u.abs().max(v.abs()) * 0.25
        + fbm(x / 1600.0, z / 1600.0, seed ^ 13, 3) * 0.12;
    let rim = smoothstep(0.78, 1.08, r);
    let base = match land.kind {
        LandKind::Flat => return 4.0 + detail * 0.2,
        LandKind::Rolling => {
            let warp = fbm(x / 2500.0, z / 2500.0, seed ^ 7, 2) * 700.0;
            let hill = fbm((x + warp) / 1700.0, (z - warp) / 1700.0, seed, 5);
            (hill * 0.95 + 0.5).max(0.0) * 280.0 * hills
        }
        LandKind::Valley => {
            // Ridges each side of a long valley that bends.
            let bend = fbm(x / 3000.0, 0.5, seed ^ 3, 2) * 0.35;
            let across = (v - bend).abs();
            let walls = smoothstep(0.2, 0.9, across).powf(1.4) * 340.0;
            (walls + (fbm(x / 1100.0, z / 1100.0, seed, 4) * 0.8 + 0.5) * 90.0) * hills.max(0.15)
        }
        LandKind::Desert => {
            // Long smooth dunes across the wind, flat pans between, and a
            // few low mesas.
            let warp = fbm(x / 1800.0, z / 1800.0, seed ^ 17, 2) * 400.0;
            let wave = ((x * 0.85 + z * 0.5 + warp) / 520.0).sin() * 0.5 + 0.5;
            let dunes =
                wave * wave * (fbm(x / 1500.0, z / 1500.0, seed, 3) * 0.8 + 0.5).clamp(0.0, 1.0);
            let pans = smoothstep(-0.25, 0.35, fbm(x / 2600.0, z / 2600.0, seed ^ 9, 3));
            let mesas = smoothstep(0.42, 0.48, fbm(x / 2000.0, z / 2000.0, seed ^ 11, 4)) * 120.0;
            let swell = (fbm(x / 3000.0, z / 3000.0, seed ^ 19, 3) * 0.5 + 0.5) * 80.0;
            (dunes * 45.0 * pans + mesas + swell) * hills.max(0.1)
        }
        LandKind::Mountains => {
            // A meadow in the middle, peaks round it.
            let rr = (u * u + v * v).sqrt();
            let lift = smoothstep(0.2, 0.95, rr);
            let warp = fbm(x / 2000.0, z / 2000.0, seed ^ 23, 2) * 500.0;
            let peaks = ridged((x + warp) / 3000.0, (z - warp) / 3000.0, seed, 4) * 560.0;
            let meadow = (fbm(x / 1200.0, z / 1200.0, seed ^ 5, 4) * 0.9 + 0.5) * 110.0;
            (meadow + peaks * lift) * hills.max(0.15)
        }
    };
    BASE + base + detail + rim * (110.0 + 200.0 * hills)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_stays_in_range_and_repeats() {
        for i in 0..2000 {
            let (x, z) = (i as f32 * 0.37, i as f32 * 0.91);
            let n = noise(x, z, 3);
            assert!(n.abs() <= 1.05, "{n}");
            assert_eq!(n, noise(x, z, 3));
        }
    }

    #[test]
    fn every_land_is_above_its_floor() {
        for kind in LandKind::ALL {
            let land = Land {
                kind,
                seed: 4,
                hills: 1.0,
                rough: 1.0,
            };
            for i in 0..400 {
                let (x, z) = ((i * 37 % 3840) as f32, (i * 91 % 3840) as f32);
                let h = height(&land, 3840.0, x, z);
                assert!(h > 0.0 && h < 1200.0, "{kind:?} {h}");
            }
        }
    }
}
