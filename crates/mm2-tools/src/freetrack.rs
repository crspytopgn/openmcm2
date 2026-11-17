//! `mm2-tools freetrack <out.env> [seed]`: a new outdoor test track made
//! from scratch (rolling hills, a dirt loop with a few jumps), written in
//! the `.ENV` layout the engine reads. Nothing in it comes from the
//! original game: it is the free pack's first, plain test track.

use mm2_formats::{rs2, trn};
use std::path::Path;

/// Samples per side (80 cells of 16 units) and feet per unit: 3840 ft.
const SIZE: usize = 1281;
const SCALE: f32 = 3.0;
/// Pixels per 64-unit tile texture.
const TEX: usize = 128;
/// Half the track's width, ft.
const HALF_WIDTH: f32 = 22.0;

pub(crate) fn hash(x: i32, z: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9e37_79b1) ^ (z as u32).wrapping_mul(0x85eb_ca77) ^ seed.wrapping_mul(0xc2b2_ae3d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    (h & 0xffff) as f32 / 65535.0
}

pub(crate) fn noise(x: f32, z: f32, seed: u32) -> f32 {
    let (xi, zi) = (x.floor() as i32, z.floor() as i32);
    let (fx, fz) = (x - xi as f32, z - zi as f32);
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sz) = (s(fx), s(fz));
    let a = hash(xi, zi, seed) * (1.0 - sx) + hash(xi + 1, zi, seed) * sx;
    let b = hash(xi, zi + 1, seed) * (1.0 - sx) + hash(xi + 1, zi + 1, seed) * sx;
    a * (1.0 - sz) + b * sz
}

pub(crate) fn fbm(x: f32, z: f32, seed: u32) -> f32 {
    (0..5).map(|o| noise(x * (1 << o) as f32, z * (1 << o) as f32, seed + o) / (1 << o) as f32).sum::<f32>() / 1.94
}

/// The track's centre line, a closed loop in feet.
fn centre_line(seed: u32) -> Vec<[f32; 2]> {
    let c = SIZE as f32 * SCALE / 2.0;
    let n = 720;
    (0..n)
        .map(|i| {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            let wobble = 1.0 + 0.18 * (3.0 * a + seed as f32).sin() + 0.1 * (5.0 * a).cos();
            [c + 900.0 * a.cos() * wobble, c + 620.0 * (a.sin() * wobble + 0.25 * (2.0 * a).sin())]
        })
        .collect()
}

/// Distance from a point to the loop, and how far along it (0 to 1).
fn nearest(line: &[[f32; 2]], p: [f32; 2]) -> (f32, f32) {
    let mut best = (f32::MAX, 0.0);
    for (i, a) in line.iter().enumerate() {
        let b = line[(i + 1) % line.len()];
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dz) / (dx * dx + dz * dz)).clamp(0.0, 1.0);
        let (qx, qz) = (a[0] + dx * t - p[0], a[1] + dz * t - p[1]);
        let d = (qx * qx + qz * qz).sqrt();
        if d < best.0 {
            best = (d, (i as f32 + t) / line.len() as f32);
        }
    }
    best
}

/// A jump's extra height at `along` (0 to 1 round the loop): a few
/// tabletops and doubles.
fn jumps(along: f32) -> f32 {
    let bumps: [(f32, f32, f32); 5] = [(0.08, 0.018, 7.0), (0.24, 0.01, 5.0), (0.27, 0.01, 5.0), (0.52, 0.025, 8.0), (0.78, 0.015, 6.0)];
    bumps
        .iter()
        .map(|&(at, half, h)| {
            let d = ((along - at).abs()).min(1.0 - (along - at).abs()) / half;
            if d < 1.0 { h * (1.0 - d * d).sqrt().min(1.0) * if d < 0.6 { 1.0 } else { (1.0 - d) / 0.4 } } else { 0.0 }
        })
        .sum()
}

/// The course as a `.tdf`: one closed node of cross-sections round the
/// loop, and start and finish lines at its first point.
fn course(line: &[[f32; 2]]) -> Vec<u8> {
    let mut out = Vec::new();
    let tag = |out: &mut Vec<u8>, t: u8| out.push(t);
    let f = |out: &mut Vec<u8>, v: f32| out.extend_from_slice(&v.to_le_bytes());
    let u = |out: &mut Vec<u8>, v: u32| out.extend_from_slice(&v.to_le_bytes());
    // Points every ~40 ft, the loop closed on its first.
    let mut points: Vec<[f32; 2]> = Vec::new();
    let mut length = 0.0;
    for (i, p) in line.iter().enumerate() {
        let q = line[(i + 1) % line.len()];
        length += ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2)).sqrt();
        if points.last().is_none_or(|l: &[f32; 2]| ((p[0] - l[0]).powi(2) + (p[1] - l[1]).powi(2)).sqrt() >= 40.0) {
            points.push(*p);
        }
    }
    points.push(points[0]);
    let side = |i: usize| {
        let (a, b) = (points[i.saturating_sub(1)], points[(i + 1).min(points.len() - 1)]);
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let l = (dx * dx + dz * dz).sqrt().max(1e-3);
        ([dx / l, dz / l], [-dz / l, dx / l])
    };
    tag(&mut out, 1);
    u(&mut out, 1);
    for (t, v) in [(2, 0.0), (3, 0.0), (4, 1.0)] {
        tag(&mut out, t);
        f(&mut out, v);
    }
    tag(&mut out, 5);
    u(&mut out, 0);
    tag(&mut out, 6);
    u(&mut out, 1);
    tag(&mut out, 7);
    f(&mut out, length);
    for (i, p) in points.iter().enumerate() {
        let (_, n) = side(if i + 1 == points.len() { 0 } else { i });
        tag(&mut out, 8);
        for (t, x, z) in [
            (9, p[0], p[1]),
            (10, p[0] + n[0] * HALF_WIDTH, p[1] + n[1] * HALF_WIDTH),
            (11, p[0] - n[0] * HALF_WIDTH, p[1] - n[1] * HALF_WIDTH),
        ] {
            tag(&mut out, t);
            for v in [x, 0.0, z] {
                f(&mut out, v);
            }
        }
    }
    tag(&mut out, 13);
    u(&mut out, 1);
    tag(&mut out, 14);
    u(&mut out, 0);
    u(&mut out, 0);
    // The start a little past the first point, the finish 7 ft behind it.
    let (d, n) = side(1);
    let start = points[1];
    for (t, back) in [(15, 0.0), (16, 7.0)] {
        tag(&mut out, t);
        for v in [start[0] - d[0] * back, 0.0, start[1] - d[1] * back, d[0], 0.0, d[1], n[0] * HALF_WIDTH, 0.0, n[1] * HALF_WIDTH, HALF_WIDTH * 2.0] {
            f(&mut out, v);
        }
    }
    out
}

pub(crate) fn rgb555(r: f32, g: f32, b: f32) -> u16 {
    let c = |v: f32| ((v.clamp(0.0, 1.0) * 31.0).round() as u16) & 31;
    c(r) << 10 | c(g) << 5 | c(b)
}

pub fn build(out: &Path, seed: u32) -> Result<(), Box<dyn std::error::Error>> {
    let line = centre_line(seed);
    let mut heights = vec![0.0f32; SIZE * SIZE];
    let mut dirt = vec![0.0f32; SIZE * SIZE];
    for z in 0..SIZE {
        for x in 0..SIZE {
            let (fx, fz) = (x as f32 * SCALE, z as f32 * SCALE);
            let hills = fbm(fx / 1400.0, fz / 1400.0, seed) * 140.0;
            let (d, along) = nearest(&line, [fx, fz]);
            // The track runs at a smoothed height, cut into the hills.
            let base = fbm(fx / 2600.0, fz / 2600.0, seed) * 140.0 * 0.9 + 6.0;
            let w = ((d - HALF_WIDTH) / 40.0).clamp(0.0, 1.0);
            let w = w * w * (3.0 - 2.0 * w);
            let on_track = base + if d < HALF_WIDTH + 4.0 { jumps(along) } else { 0.0 };
            heights[z * SIZE + x] = on_track * (1.0 - w) + hills.max(base - 4.0) * w;
            dirt[z * SIZE + x] = (1.0 - ((d - HALF_WIDTH) / 6.0).clamp(0.0, 1.0)).max(0.0);
        }
        if z % 128 == 0 {
            eprintln!("terrain {}%", z * 100 / SIZE);
        }
    }
    // Track = zone 1 (dust and dirt), grass = zone 2.
    let surfaces: Vec<u8> = dirt.iter().map(|&d| if d > 0.5 { 0 } else { 1 }).collect();

    // One texture per 64-unit tile, painted from the dirt mask with
    // soft, low-contrast colour noise.
    let tiles_side = (SIZE - 1) / 64;
    let mut textures = Vec::new();
    let mut tiles = Vec::new();
    for tz in 0..tiles_side {
        for tx in 0..tiles_side {
            let mut px = Vec::with_capacity(TEX * TEX);
            for j in 0..TEX {
                for i in 0..TEX {
                    let (u, v) = (tx * 64 * TEX + i * 64, tz * 64 * TEX + j * 64);
                    let (sx, sz) = (u as f32 / TEX as f32, v as f32 / TEX as f32);
                    let (x0, z0) = ((sx as usize).min(SIZE - 1), (sz as usize).min(SIZE - 1));
                    let d = dirt[z0 * SIZE + x0];
                    let n = fbm(sx * SCALE / 9.0, sz * SCALE / 9.0, seed + 7) - 0.5;
                    let grass = [0.33 + n * 0.08, 0.40 + n * 0.10, 0.22 + n * 0.05];
                    let soil = [0.47 + n * 0.08, 0.38 + n * 0.07, 0.28 + n * 0.05];
                    let c: Vec<f32> = (0..3).map(|k| grass[k] * (1.0 - d) + soil[k] * d).collect();
                    px.push(rgb555(c[0], c[1], c[2]));
                }
            }
            tiles.push(Some(textures.len() as u16));
            textures.push((TEX as u32, px));
        }
    }
    let terrain = trn::Build { size: SIZE, heights: &heights, surfaces: &surfaces, scale: SCALE, textures: &textures, tiles: &tiles }.write()?;

    let stem = out.file_stem().and_then(|s| s.to_str()).unwrap_or("FREE01").to_string();
    let start = line[0];
    let next = line[4];
    let (dx, dz) = (next[0] - start[0], next[1] - start[1]);
    let l = (dx * dx + dz * dz).sqrt();
    let sx = (start[0] / SCALE) as usize;
    let sz = (start[1] / SCALE) as usize;
    let sy = heights[sz * SIZE + sx] + 6.0;
    let scene = format!(
        "[SceneInfo]\r\nSceneName=\"Free Test Hills\"\r\nDefaultStartPosition={:.1},{:.1},{:.1}\r\nEventType=National\r\nDefaultStartDirection={:.3},0.0,{:.3}\r\n\r\n\
[Environment]\r\nTerrainFile={stem}.trn\r\nCubeFile=terra05.cub\r\nTerrainWidth=5\r\nTerrainBreadth=5\r\nTerrainWidthScale=3.0\r\nTerrainBreadthScale=3.0\r\nParticleTextureMapName=Track15P.tga\r\n\r\n\
[Lights]\r\nNumberOfLights=2\r\n[Light1]\r\nType=Ambient\r\nColorRGB=115,115,120\r\n[Light2]\r\nType=Directional\r\nColorRGB=195,192,185\r\nPosition={:.1},1800.0,{:.1}\r\nTargetPosition={:.1},60.0,{:.1}\r\nPlaceLightAtInfinity=T\r\nCastsShadows=T\r\nEmitsLight=T\r\n\r\n\
[Fog]\r\nColorRGB=176,180,188\r\n\r\n[Sounds]\r\nCrowdPresent=F\r\n\r\n\
[TerrainZone1]\r\nGenerateDust=T\r\nGenerateDirt=T\r\n\r\n[TerrainZone2]\r\nSurfaceFriction=1.0\r\nSurfaceDrag=1.1\r\nSurfaceTraction=0.9\r\nGenerateDust=F\r\nGenerateDirt=T\r\n",
        start[0], sy, start[1], dx / l, dz / l,
        SIZE as f32 * SCALE + 1000.0, SIZE as f32 * SCALE * 0.4,
        SIZE as f32 * SCALE / 2.0, SIZE as f32 * SCALE / 2.0,
    );
    let trn_name = format!("{stem}.trn");
    let scn_name = format!("{stem}.scn");
    let tdf_name = format!("{stem}.tdf");
    let tdf = course(&line);
    let archive = rs2::build(&[(&scn_name, scene.as_bytes()), (&trn_name, &terrain), (&tdf_name, &tdf)]);
    std::fs::write(out, archive)?;
    eprintln!("wrote {} ({} MB), start at {:.0},{:.0}", out.display(), std::fs::metadata(out)?.len() / 1_000_000, start[0], start[1]);
    Ok(())
}
