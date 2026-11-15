//! The sky's light over the land, baked once when a track loads: for each
//! of the terrain's samples, how much of the sky the hills round it leave
//! open (enhanced_fns.wgsl `sky_seen`). Open ground sees all of it; the
//! floor of a dip, the foot of a bank or a gully between hills less, so
//! the ambient light there is a little dimmer, as under a real sky.
//!
//! Each sample looks along eight ways over the heights, out to
//! [`REACH`] ft, for the highest the land rises; a level surface under a
//! horizon raised by an angle `h` sees cos²(h) of the sky's light that way,
//! the angle taken above the ground's own slope there.

use mm2_formats::trn::Terrain;

/// The ways each sample looks.
const WAYS: usize = 8;
/// How far it looks, ft, in steps growing by [`GROWTH`] from [`FIRST`].
const REACH: f32 = 400.0;
const FIRST: f32 = 2.0;
const GROWTH: f32 = 1.35;

/// The baked map: per sample, how much sky it sees (r) and its height (g,
/// from `low` across `range` ft), and what places it over the drawing.
pub struct SkyLight {
    pub texels: Vec<[u8; 4]>,
    /// Samples a side, and ft between them.
    pub side: u32,
    pub spacing: f32,
    pub low: f32,
    pub range: f32,
}

impl SkyLight {
    pub fn bake(terrain: &Terrain) -> SkyLight {
        let n = terrain.size;
        let s = terrain.scale;
        let height = |c: i64, r: i64| -> f32 {
            let c = c.clamp(0, n as i64 - 1) as usize;
            let r = r.clamp(0, n as i64 - 1) as usize;
            terrain.heights[r * n + c] * s
        };
        let mut steps = Vec::new();
        let mut d = FIRST;
        while d < REACH {
            steps.push(d);
            d *= GROWTH;
        }
        let ways: Vec<(f32, f32)> = (0..WAYS)
            .map(|i| {
                let a = i as f32 / WAYS as f32 * std::f32::consts::TAU;
                (a.cos(), a.sin())
            })
            .collect();
        let (mut low, mut high) = (f32::MAX, f32::MIN);
        for h in &terrain.heights {
            low = low.min(h * s);
            high = high.max(h * s);
        }
        let range = (high - low).max(1.0);
        let mut texels = vec![[255u8, 0, 0, 255]; n * n];
        for r in 0..n {
            for c in 0..n {
                let here = height(c as i64, r as i64);
                // The ground's own slope: land rising only as the slope
                // does hides none of the sky a surface on it faces.
                let (ci, ri) = (c as i64, r as i64);
                let gx = (height(ci + 1, ri) - height(ci - 1, ri)) / (2.0 * s);
                let gz = (height(ci, ri + 1) - height(ci, ri - 1)) / (2.0 * s);
                let mut seen = 0.0;
                for &(wx, wz) in &ways {
                    let slope = gx * wx + gz * wz;
                    let mut rise: f32 = 0.0;
                    for &d in &steps {
                        let (x, z) = (c as f32 + wx * d / s, r as f32 + wz * d / s);
                        // Past the edge the land is taken as level.
                        if x < 0.0 || z < 0.0 || x > (n - 1) as f32 || z > (n - 1) as f32 {
                            break;
                        }
                        rise = rise
                            .max((height(x.round() as i64, z.round() as i64) - here) / d - slope);
                    }
                    seen += 1.0 / (1.0 + rise * rise);
                }
                let seen = seen / WAYS as f32;
                texels[r * n + c] = [
                    (seen * 255.0).round() as u8,
                    (((here - low) / range) * 255.0).round() as u8,
                    0,
                    255,
                ];
            }
        }
        SkyLight {
            texels,
            side: n as u32,
            spacing: s,
            low,
            range,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terrain(size: usize, height: impl Fn(usize, usize) -> f32) -> Terrain {
        Terrain {
            size,
            heights: (0..size * size)
                .map(|i| height(i % size, i / size))
                .collect(),
            normals: vec![[0, 127, 0]; size * size],
            surfaces: vec![0; size * size],
            scale: 4.0,
            textures: Vec::new(),
            tiles: Vec::new(),
        }
    }

    #[test]
    fn open_ground_sees_the_whole_sky_and_a_ditch_less() {
        let flat = SkyLight::bake(&terrain(32, |_, _| 0.0));
        assert!(flat.texels.iter().all(|t| t[0] == 255));
        // A trench along the middle, 5 units (20 ft) deep, 2 wide.
        let ditch = SkyLight::bake(&terrain(
            32,
            |c, _| if (15..17).contains(&c) { -5.0 } else { 0.0 },
        ));
        let floor = ditch.texels[16 * 32 + 15][0];
        let open = ditch.texels[16 * 32 + 4][0];
        assert!(floor < 200, "{floor}");
        assert!(open > 250, "{open}");
    }
}
