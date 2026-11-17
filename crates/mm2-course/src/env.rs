//! Writing a course as the game's `.ENV`: an `RS2` archive of the terrain
//! (`.trn`), the scene (`.scn`), the course the riders follow (`.tdf`) and,
//! for Baja and Enduro, the checkpoint gates (`.wpt`).

use crate::bake::{self, Field};
use crate::path::Path;
use crate::{land, Course, Mode, Surface};
use mm2_formats::{rs2, trn};

/// Pixels a side of each 64-sample terrain tile's texture.
const TEX: usize = 128;
/// The game takes at most 34 gates.
pub const MOST_GATES: usize = 34;

/// Everything the bake made, for the builder to look at before writing.
pub struct Baked {
    pub field: Field,
    pub path: Path,
    pub gates: Vec<([f32; 2], [f32; 2])>,
    /// The `.ENV` archive.
    pub env: Vec<u8>,
}

/// A seamless ground picture: `side` square, sRGB colour 0..1, rows along +z.
pub struct Picture {
    pub side: usize,
    pub rgb: Vec<[f32; 3]>,
}

/// The ground materials a builder track is painted with, by the free
/// pack's ground picture names, and the feet one repeat of each covers.
pub const GROUND: [(&str, f32); 7] = [
    ("trackdirt", 40.0),
    ("grass", 48.0),
    ("desertsand", 64.0),
    ("mud", 40.0),
    ("rock", 64.0),
    ("sxclay", 32.0),
    ("loosedirt", 40.0),
];
const TRACKDIRT: usize = 0;
const SXCLAY: usize = 5;
const LOOSEDIRT: usize = 6;

/// The pictures the ground is painted from, by [`GROUND`] name. Any
/// missing are painted from flat colours and grain, as before.
#[derive(Default, Clone)]
pub struct Pictures {
    pub by_name: std::collections::HashMap<String, std::sync::Arc<Picture>>,
}

/// The ground material a sample of surface `s` is covered with off the line.
pub(crate) fn material(s: Surface) -> usize {
    match s {
        Surface::Dirt => LOOSEDIRT,
        Surface::Grass => 1,
        Surface::Sand => 2,
        Surface::Mud => 3,
        Surface::Rock => 4,
    }
}

/// A picture made ready for one track: shrunk to about a baked texel a
/// pixel, and matched to the builder's colour for its surface.
pub(crate) struct Ready {
    side: usize,
    rgb: Vec<[f32; 3]>,
    /// Pixels per foot.
    per_ft: f32,
}

impl Ready {
    fn new(pic: &Picture, repeat_ft: f32, texel_ft: f32, target: [f32; 3]) -> Option<Ready> {
        let mut side = pic.side;
        if side == 0 || !side.is_power_of_two() || pic.rgb.len() != side * side {
            return None;
        }
        let mut rgb = pic.rgb.clone();
        // Halve (a box filter, wrapping) until a pixel is about half a
        // baked texel, so the bake does not shimmer.
        while side > 4 && repeat_ft / side as f32 * 2.0 < texel_ft {
            let half = side / 2;
            let mut out = vec![[0.0f32; 3]; half * half];
            for j in 0..half {
                for i in 0..half {
                    let mut c = [0.0f32; 3];
                    for (di, dj) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let p = rgb[(j * 2 + dj) * side + i * 2 + di];
                        for q in 0..3 {
                            c[q] += p[q] * 0.25;
                        }
                    }
                    out[j * half + i] = c;
                }
            }
            rgb = out;
            side = half;
        }
        // Keep the picture's grain but the builder's soft colour, so tracks
        // stay as gentle as the rest of the game.
        let mut mean = [0.0f32; 3];
        for p in &rgb {
            for q in 0..3 {
                mean[q] += p[q] / rgb.len() as f32;
            }
        }
        for p in rgb.iter_mut() {
            for q in 0..3 {
                let rel = p[q] / mean[q].max(0.02) - 1.0;
                p[q] = target[q] * (1.0 + rel.clamp(-0.6, 0.8) * 0.8);
            }
        }
        Some(Ready { side, rgb, per_ft: side as f32 / repeat_ft })
    }

    /// The colour at (x, z) ft, bilinear and wrapping.
    pub(crate) fn at(&self, x: f32, z: f32) -> [f32; 3] {
        let (u, v) = (x * self.per_ft - 0.5, z * self.per_ft - 0.5);
        let (u0, v0) = (u.floor(), v.floor());
        let (fu, fv) = (u - u0, v - v0);
        let s = self.side as i64;
        let px = |a: i64, b: i64| self.rgb[(b.rem_euclid(s) * s + a.rem_euclid(s)) as usize];
        let (a, b) = (u0 as i64, v0 as i64);
        let (p00, p10, p01, p11) = (px(a, b), px(a + 1, b), px(a, b + 1), px(a + 1, b + 1));
        let mut c = [0.0; 3];
        for q in 0..3 {
            c[q] = (p00[q] * (1.0 - fu) + p10[q] * fu) * (1.0 - fv)
                + (p01[q] * (1.0 - fu) + p11[q] * fu) * fv;
        }
        c
    }
}

/// A picture's repeat, ft, nudged to go a whole number of times into a
/// 64-sample terrain tile, so tiles of open ground come out alike.
fn tiled(repeat: f32, step: f32) -> f32 {
    let tile = 64.0 * step;
    tile / (tile / repeat).round().max(1.0)
}

/// The colour each ground material is matched to.
pub(crate) fn target(m: usize) -> [f32; 3] {
    let d = colour(Surface::Dirt);
    match m {
        TRACKDIRT => [d[0] * 0.90, d[1] * 0.90, d[2] * 0.90],
        SXCLAY => [d[0] * 0.82, d[1] * 0.78, d[2] * 0.76],
        LOOSEDIRT => d,
        1 => colour(Surface::Grass),
        2 => colour(Surface::Sand),
        3 => colour(Surface::Mud),
        _ => colour(Surface::Rock),
    }
}

/// Bakes `course` at full size into an `.ENV` archive, its ground painted
/// from flat colours and grain.
pub fn write(course: &Course) -> Baked {
    write_with(course, &Pictures::default())
}

/// Bakes `course` at full size into an `.ENV` archive, its ground painted
/// from `pictures` where it has them.
pub fn write_with(course: &Course, pictures: &Pictures) -> Baked {
    let land = bake::land(course, 1);
    let field = bake::bake(course, 1, &land);
    let path = course.path();
    let stem = course.stem();
    let gates = if course.mode.laps() {
        Vec::new()
    } else {
        gates(course, &path)
    };
    let texel_ft = 64.0 * field.step / TEX as f32;
    let ready: Vec<Option<Ready>> = GROUND
        .iter()
        .enumerate()
        .map(|(m, (name, repeat))| {
            Ready::new(pictures.by_name.get(*name)?, tiled(*repeat, field.step), texel_ft, target(m))
        })
        .collect();
    // A copy: the track's own files with the changes.
    if let Some(src) = &course.source {
        let added = objects(course, &field, &path);
        let st = if path.is_empty() { ([0.0; 2], [0.0, 1.0]) } else { let (p, d, _) = start(course, &path); (p, d) };
        let made = crate::copy::files(course, src, &field, &ready, &added, || tdf(course, &field, &path), || wpt(&gates), st);
        if let Ok(mut files) = made {
            files.push((format!("{stem}.course"), course.to_json().into_bytes()));
            crate::preview::add(&stem, &mut files);
            let refs: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
            let env = rs2::build(&refs);
            return Baked { field, path, gates, env };
        }
    }
    let terrain = terrain(course, &field, &ready);
    let mut scene = scene(course, &field, &path, &stem);
    let (living, flights) = living(course, &field, &stem, None, 1);
    if !flights.is_empty() {
        scene += &format!("\r\n[Animations]\r\nNumberOfAnimations={}\r\n\r\n", flights.len());
        for f in &flights {
            scene += f;
        }
    }
    let tdf = tdf(course, &field, &path);
    let mut files: Vec<(String, Vec<u8>)> = vec![
        (format!("{stem}.scn"), scene.into_bytes()),
        (format!("{stem}.trn"), terrain),
    ];
    // The ground map: the viewer lays each material's full-size picture
    // over the baked tiles, so the ground is crisp close up.
    if ready.iter().any(Option::is_some) {
        files.push((format!("{stem}.gmt"), ground_map(course, &field, texel_ft)));
    }
    if course.mode.laps() {
        files.push((format!("{stem}.tdf"), tdf));
    } else {
        files.push((format!("{stem}01.tdf"), tdf));
        files.push((format!("{stem}01.wpt"), wpt(&gates).into_bytes()));
    }
    files.extend(living);
    // The builder's own file rides along, so a shared track can be opened
    // and changed again.
    files.push((format!("{stem}.course"), course.to_json().into_bytes()));
    // Pictures for the event page, as the game's own tracks carry.
    crate::preview::add(&stem, &mut files);
    let refs: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_slice()))
        .collect();
    let env = rs2::build(&refs);
    Baked {
        field,
        path,
        gates,
        env,
    }
}

/// Reads the builder's own file back out of a baked `.ENV`.
pub fn course_in(env: &[u8]) -> Option<Course> {
    let ar = rs2::Archive::parse(env).ok()?;
    let e = ar
        .entries()
        .iter()
        .find(|e| e.name.to_ascii_lowercase().ends_with(".course"))?;
    Course::from_json(&String::from_utf8_lossy(ar.data(e))).ok()
}

/// Gates along the line every few hundred feet, the last at the start.
pub fn gates(course: &Course, path: &Path) -> Vec<([f32; 2], [f32; 2])> {
    if path.is_empty() {
        return Vec::new();
    }
    let spacing = match course.mode {
        Mode::Baja => 700.0,
        _ => 450.0,
    };
    let span = if path.closed {
        path.length
    } else {
        path.length - course.line.start.max(0.0)
    };
    let count = ((span / spacing).round() as usize).clamp(2, MOST_GATES);
    let step = span / count as f32;
    (1..=count)
        .map(|k| {
            let s = course.line.start + step * k as f32
                - if !path.closed && k == count { 8.0 } else { 0.0 };
            let i = path.index_at(s);
            (path.pts[i], path.dir[i])
        })
        .collect()
}

fn wpt(gates: &[([f32; 2], [f32; 2])]) -> String {
    let mut s = format!(
        "; made by the OpenMCM2 Course Builder\r\n[Waypoints]\r\nNumWaypoints={}\r\n",
        gates.len()
    );
    for (k, (p, d)) in gates.iter().enumerate() {
        let n = k + 1;
        s += &format!("\r\nPoint{n}X={:.2}\r\nPoint{n}Z={:.2}\r\nPoint{n}LookX={:.3}\r\nPoint{n}LookZ={:.3}\r\n", p[0], p[1], d[0], d[1]);
    }
    s
}

/// The start line: across the track at `line.start`.
fn start(course: &Course, path: &Path) -> ([f32; 2], [f32; 2], f32) {
    let i = path.index_at(course.line.start);
    (path.pts[i], path.dir[i], path.width[i] / 2.0)
}

fn tdf(course: &Course, field: &Field, path: &Path) -> Vec<u8> {
    let mut out = Vec::new();
    let tag = |out: &mut Vec<u8>, t: u8| out.push(t);
    let f = |out: &mut Vec<u8>, v: f32| out.extend_from_slice(&v.to_le_bytes());
    let u = |out: &mut Vec<u8>, v: u32| out.extend_from_slice(&v.to_le_bytes());
    if path.is_empty() {
        return out;
    }
    let spacing = if course.mode == Mode::Supercross {
        24.0
    } else {
        40.0
    };
    // Cross-sections from the start round to it again.
    let mut picks = Vec::new();
    let s0 = course.line.start;
    let span = if path.closed {
        path.length
    } else {
        path.length - s0.max(0.0)
    };
    let count = ((span / spacing).ceil() as usize).max(3);
    for k in 0..count {
        picks.push(path.index_at(s0 + span * k as f32 / count as f32));
    }
    if path.closed {
        picks.push(picks[0]);
    } else {
        picks.push(path.pts.len() - 1);
    }
    let lap = course.mode.laps();
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
    f(&mut out, span);
    for &i in &picks {
        let (p, n, hw) = (path.pts[i], path.side(i), path.width[i] / 2.0);
        tag(&mut out, 8);
        for (t, x, z) in [
            (9, p[0], p[1]),
            (10, p[0] + n[0] * hw, p[1] + n[1] * hw),
            (11, p[0] - n[0] * hw, p[1] - n[1] * hw),
        ] {
            tag(&mut out, t);
            // The shipped lap tracks keep their course level; the gate
            // tracks' follows the ground.
            let y = if lap { 0.0 } else { field.height(x, z) };
            for v in [x, y, z] {
                f(&mut out, v);
            }
        }
    }
    tag(&mut out, 13);
    u(&mut out, 1);
    tag(&mut out, 14);
    u(&mut out, 0);
    u(&mut out, 0);
    let (p, d, hw) = start(course, path);
    let n = [-d[1], d[0]];
    // The start, and on a lap track the finish 7 ft behind it, as the
    // shipped National tracks have them.
    let lines: &[(u8, f32)] = if lap {
        &[(15, 0.0), (16, 7.0)]
    } else {
        &[(15, 0.0)]
    };
    for &(t, back) in lines {
        tag(&mut out, t);
        for v in [
            p[0] - d[0] * back,
            0.0,
            p[1] - d[1] * back,
            d[0],
            0.0,
            d[1],
            n[0] * hw,
            0.0,
            n[1] * hw,
            hw * 2.0,
        ] {
            f(&mut out, v);
        }
    }
    out
}

fn rgb555(c: [f32; 3]) -> u16 {
    let q = |v: f32| ((v.clamp(0.0, 1.0) * 31.0).round() as u16) & 31;
    q(c[0]) << 10 | q(c[1]) << 5 | q(c[2])
}

/// Each surface's colour, soft and a little dusty.
pub fn colour(s: Surface) -> [f32; 3] {
    match s {
        Surface::Dirt => [0.45, 0.35, 0.26],
        Surface::Grass => [0.31, 0.37, 0.21],
        Surface::Sand => [0.70, 0.60, 0.44],
        Surface::Mud => [0.29, 0.23, 0.18],
        Surface::Rock => [0.50, 0.47, 0.43],
    }
}

/// The ground map (`<track>.gmt`, as the free tracks write it): `GMT1`,
/// samples a side and the material count (u32), per material its name (16
/// bytes, zero padded) and feet a repeat (f32), the baked feet a texel
/// (f32), then per sample the first material, the second, the second's
/// share (0..255) and a spare byte.
fn ground_map(course: &Course, field: &Field, texel_ft: f32) -> Vec<u8> {
    let n = field.n;
    let line = if course.mode == Mode::Supercross { SXCLAY } else { TRACKDIRT };
    let mut out = Vec::with_capacity(16 + GROUND.len() * 20 + n * n * 4);
    out.extend_from_slice(b"GMT1");
    out.extend_from_slice(&(n as u32).to_le_bytes());
    out.extend_from_slice(&(GROUND.len() as u32).to_le_bytes());
    for (name, repeat) in GROUND {
        let mut f = [0u8; 16];
        f[..name.len()].copy_from_slice(name.as_bytes());
        out.extend_from_slice(&f);
        out.extend_from_slice(&tiled(repeat, field.step).to_le_bytes());
    }
    out.extend_from_slice(&texel_ft.to_le_bytes());
    for k in 0..n * n {
        let s = Surface::from_zone(field.surfaces[k]);
        let m = material(s);
        let on = 1.0 - field.track[k].min(1.0);
        if s == Surface::Dirt && on > 0.0 {
            out.extend_from_slice(&[m as u8, line as u8, (on * 255.0).round() as u8, 0]);
        } else {
            out.extend_from_slice(&[m as u8, m as u8, 0, 0]);
        }
    }
    out
}

fn terrain(course: &Course, field: &Field, ready: &[Option<Ready>]) -> Vec<u8> {
    let n = field.n;
    let tiles_side = (n - 1) / 64;
    let seed = course.land.seed;
    let sx = course.mode == Mode::Supercross;
    let mut textures = vec![(TEX as u32, Vec::new()); tiles_side * tiles_side];
    let pixels: Vec<Vec<u16>> = {
        let mut slots: Vec<Vec<u16>> = vec![Vec::new(); tiles_side * tiles_side];
        let threads = crate::bake::threads();
        let per = tiles_side.div_ceil(threads);
        if threads == 1 {
            for (t, slot) in slots.iter_mut().enumerate() {
                *slot = tile(field, t % tiles_side, t / tiles_side, seed, sx, ready);
            }
        } else {
        std::thread::scope(|s| {
            for (band, part) in slots.chunks_mut(per * tiles_side).enumerate() {
                s.spawn(move || {
                    for (k, slot) in part.iter_mut().enumerate() {
                        let t = band * per * tiles_side + k;
                        *slot = tile(field, t % tiles_side, t / tiles_side, seed, sx, ready);
                    }
                });
            }
        });
        }
        slots
    };
    // Tiles that came out alike share one picture.
    let mut seen: std::collections::HashMap<Vec<u16>, u16> = std::collections::HashMap::new();
    textures.clear();
    let tiles: Vec<Option<u16>> = pixels
        .into_iter()
        .map(|px| {
            let next = textures.len() as u16;
            let at = *seen.entry(px.clone()).or_insert(next);
            if at == next {
                textures.push((TEX as u32, px));
            }
            Some(at)
        })
        .collect();
    trn::Build {
        size: n,
        heights: &field.heights,
        surfaces: &field.surfaces,
        scale: course.mode.scale(),
        textures: &textures,
        tiles: &tiles,
    }
    .write()
    .unwrap_or_default()
}

/// One tile's texture: the surfaces blended softly, with low-contrast
/// grain, darker on the packed racing line and on steep faces.
fn tile(field: &Field, tx: usize, tz: usize, seed: u32, sx: bool, ready: &[Option<Ready>]) -> Vec<u16> {
    let n = field.n;
    let pictures = ready.iter().any(Option::is_some);
    let line = if sx { SXCLAY } else { TRACKDIRT };
    let mut px = Vec::with_capacity(TEX * TEX);
    let per = 64.0 / TEX as f32;
    for j in 0..TEX {
        for i in 0..TEX {
            let (fx, fz) = (
                tx as f32 * 64.0 + (i as f32 + 0.5) * per,
                tz as f32 * 64.0 + (j as f32 + 0.5) * per,
            );
            let (x0, z0) = (
                (fx.floor() as usize).min(n - 2),
                (fz.floor() as usize).min(n - 2),
            );
            let (ax, az) = (fx - x0 as f32, fz - z0 as f32);
            let (wx, wz) = (fx * field.step, fz * field.step);
            let mut c = [0.0f32; 3];
            let mut on_line = 0.0;
            // Each material's share of this pixel.
            let mut share = [0.0f32; GROUND.len()];
            for (dx, dz, w) in [
                (0, 0, (1.0 - ax) * (1.0 - az)),
                (1, 0, ax * (1.0 - az)),
                (0, 1, (1.0 - ax) * az),
                (1, 1, ax * az),
            ] {
                let k = (z0 + dz) * n + x0 + dx;
                let surface = Surface::from_zone(field.surfaces[k]);
                let s = colour(surface);
                for q in 0..3 {
                    c[q] += s[q] * w;
                }
                let on = 1.0 - field.track[k].min(1.0);
                on_line += on * w;
                let m = material(surface);
                if surface == Surface::Dirt {
                    share[m] += w * (1.0 - on);
                    share[line] += w * on;
                } else {
                    share[m] += w;
                }
            }
            // The pictures, where there are pictures; flat colour else.
            if pictures {
                let mut p = [0.0f32; 3];
                for (m, &w) in share.iter().enumerate() {
                    if w <= 0.0 {
                        continue;
                    }
                    let col = match &ready[m] {
                        Some(r) => r.at(wx, wz),
                        None => target(m),
                    };
                    for q in 0..3 {
                        p[q] += col[q] * w;
                    }
                }
                c = p;
            }
            let big = land::fbm(wx / 60.0, wz / 60.0, seed ^ 31, 3);
            let fine = land::noise(wx / 3.0, wz / 3.0, seed ^ 41) * 0.6
                + land::noise(wx / 1.1, wz / 1.1, seed ^ 43) * 0.4;
            // Slope from the neighbouring heights.
            let h = |x: usize, z: usize| field.heights[z.min(n - 1) * n + x.min(n - 1)];
            let slope = ((h(x0 + 1, z0) - h(x0, z0))
                .abs()
                .max((h(x0, z0 + 1) - h(x0, z0)).abs())
                / field.step)
                .min(1.5);
            // Grass yellows a little in patches.
            let mut dry = (land::fbm(wx / 140.0, wz / 140.0, seed ^ 53, 2) * 0.5 + 0.5) * 0.08;
            // The pictures carry their own grain and the line its own dirt;
            // and with nothing that changes from place to place, the open
            // ground's tiles come out alike and share one picture, keeping
            // a track within the browser's 256 terrain pictures.
            let (big, fine, slope, on_line) = if pictures {
                dry = 0.0;
                (0.0, 0.0, 0.0, on_line * 0.6)
            } else {
                (big, fine, slope, on_line)
            };
            let shade = 1.0 + big * 0.07 + fine * 0.05
                - slope * 0.06
                - on_line * if sx { 0.10 } else { 0.07 };
            let c = [
                c[0] * shade + dry * 0.6,
                c[1] * shade + dry * 0.4,
                c[2] * shade,
            ];
            px.push(rgb555(c));
        }
    }
    px
}

pub fn zone_section(k: usize, s: Surface) -> String {
    let (friction, drag, traction, dust, dirt) = match s {
        Surface::Dirt => (1.0, 1.0, 1.0, true, true),
        Surface::Grass => (1.0, 1.1, 0.9, false, true),
        Surface::Sand => (1.0, 1.4, 0.8, true, true),
        Surface::Mud => (1.0, 1.6, 0.65, false, true),
        Surface::Rock => (1.0, 0.95, 1.05, true, false),
    };
    let tf = |b: bool| if b { "T" } else { "F" };
    // Zone 1's grip is fixed by the game.
    if k == 0 {
        return format!(
            "[TerrainZone1]\r\nGenerateDust={}\r\nGenerateDirt={}\r\n\r\n",
            tf(dust),
            tf(dirt)
        );
    }
    format!(
        "[TerrainZone{}]\r\nSurfaceFriction={friction:.2}\r\nSurfaceDrag={drag:.2}\r\nSurfaceTraction={traction:.2}\r\nGenerateDust={}\r\nGenerateDirt={}\r\n\r\n",
        k + 1,
        tf(dust),
        tf(dirt)
    )
}

/// An object placed on the ground.
pub struct Placed {
    pub model: String,
    pub at: [f32; 3],
    pub look: [f32; 2],
    pub scale: f32,
}

fn objects(course: &Course, field: &Field, path: &Path) -> Vec<Placed> {
    let mut out: Vec<Placed> = course
        .objects
        .iter()
        .map(|o| {
            let (s, c) = o.yaw.to_radians().sin_cos();
            Placed {
                model: o.model.clone(),
                at: [o.x, field.height(o.x, o.z), o.z],
                look: [s, c],
                scale: o.scale,
            }
        })
        .collect();
    if course.dress.edge_bales && !path.is_empty() {
        // Bales along both edges, a gap left at the start.
        let mut s = 0.0;
        let mut k = 0;
        while s < path.length {
            let i = path.index_at(s);
            let gap = path.ahead(course.line.start - 20.0, s);
            if gap > 60.0 || gap < 0.0 {
                let (p, n, d, hw) = (
                    path.pts[i],
                    path.side(i),
                    path.dir[i],
                    path.width[i] / 2.0 + 2.5,
                );
                for side in [-1.0, 1.0] {
                    let (x, z) = (p[0] + n[0] * hw * side, p[1] + n[1] * hw * side);
                    // Keep off any other stretch of track.
                    if let Some(near) = path.nearest([x, z]) {
                        if near.side.abs() < path.width[near.i] / 2.0 + 1.5 {
                            continue;
                        }
                    }
                    let model = if k % 9 == 4 {
                        "Fox03.SLT"
                    } else {
                        "AlpineStarsHB.SLT"
                    };
                    out.push(Placed {
                        model: model.into(),
                        at: [x, field.height(x, z), z],
                        look: d,
                        scale: 1.0,
                    });
                }
            }
            s += 14.0;
            k += 1;
        }
    }
    out
}

fn scene(course: &Course, field: &Field, path: &Path, stem: &str) -> String {
    let mode = course.mode;
    let side = mode.side();
    let (p, d, _) = if path.is_empty() {
        ([side / 2.0; 2], [0.0, 1.0], 10.0)
    } else {
        start(course, path)
    };
    let y = field.height(p[0], p[1]) + 4.0;
    let mut s = String::new();
    s += "; made by the OpenMCM2 Course Builder\r\n[SceneInfo]\r\n";
    s += &format!("SceneName=\"{}\"\r\n", course.name.replace('"', "'"));
    // The ground's textures have no light painted in: OpenMCM2 shades it.
    s += "LightGround=1\r\n";
    s += &format!(
        "DefaultStartPosition={:.1},{:.1},{:.1}\r\nEventType={}\r\n",
        p[0],
        y,
        p[1],
        mode.event_type()
    );
    s += &format!("DefaultStartDirection={:.3},0.0,{:.3}\r\n", d[0], d[1]);
    if !mode.laps() {
        s += "NumberOfWaypointRaces=1\r\n";
    }
    s += "\r\n[Environment]\r\n";
    s += &format!("TerrainFile={stem}.trn\r\n");
    if mode != Mode::Supercross {
        s += &format!("CubeFile={}\r\n", course.look.sky);
    }
    let w = mode.terrain_width();
    let sc = mode.scale();
    s += &format!("TerrainWidth={w}\r\nTerrainBreadth={w}\r\nTerrainWidthScale={sc:.1}\r\nTerrainBreadthScale={sc:.1}\r\nParticleTextureMapName=Track15P.tga\r\n\r\n");
    // The sun from the look's height and turn.
    let (elev, turn) = (
        10.0 + course.look.sun.clamp(0.0, 1.0) * 65.0,
        course.look.sun_turn.to_radians(),
    );
    let (se, ce) = elev.to_radians().sin_cos();
    let c = side / 2.0;
    let far = 2000.0;
    let sun = [
        c + turn.sin() * ce * far,
        10.0 + se * far,
        c + turn.cos() * ce * far,
    ];
    let warm = 1.0 - course.look.sun.clamp(0.0, 1.0);
    let sun_col = [190.0 + warm * 20.0, 186.0 + warm * 2.0, 178.0 - warm * 30.0];
    let amb = if mode == Mode::Supercross {
        [90.0, 90.0, 110.0]
    } else {
        [112.0, 114.0, 122.0]
    };
    s += "[Lights]\r\nNumberOfLights=2\r\n";
    s += &format!(
        "[Light1]\r\nType=Ambient\r\nColorRGB={:.0},{:.0},{:.0}\r\n",
        amb[0], amb[1], amb[2]
    );
    s += &format!(
        "[Light2]\r\nType=Directional\r\nColorRGB={:.0},{:.0},{:.0}\r\nPosition={:.1},{:.1},{:.1}\r\nTargetPosition={c:.1},10.0,{c:.1}\r\nPlaceLightAtInfinity=T\r\nCastsShadows=T\r\nEmitsLight=T\r\n\r\n",
        sun_col[0], sun_col[1], sun_col[2], sun[0], sun[1], sun[2]
    );
    let fog = match (mode, course.land.kind) {
        (Mode::Supercross, _) => [125, 125, 183],
        (_, crate::LandKind::Desert) => [196, 186, 170],
        _ => [176, 182, 190],
    };
    s += &format!("[Fog]\r\nColorRGB={},{},{}\r\n\r\n", fog[0], fog[1], fog[2]);
    s += &format!(
        "[Sounds]\r\nCrowdPresent={}\r\n\r\n",
        if mode == Mode::Supercross { "T" } else { "F" }
    );
    for (k, surface) in Surface::ALL.iter().enumerate() {
        s += &zone_section(k, *surface);
    }
    let mut models = objects(course, field, path);
    if mode == Mode::Supercross {
        let seg = &course.look.stadium;
        let stem = seg.rsplit_once('.').map_or(seg.as_str(), |(a, _)| a);
        s += &format!("[ResourceFiles]\r\nResourceFile1={stem}.tex\r\n\r\n");
        s += &format!("[Stadium]\r\nFileName={seg}\r\nTop=500.0\r\nBottom=0.0\r\nScale=3.0\r\nPosition={c:.1},4.0,{c:.1}\r\n\r\n");
        let slt = format!(
            "{}.SLT",
            seg.rsplit_once('.').map_or(seg.as_str(), |(a, _)| a)
        );
        models.push(Placed {
            model: slt,
            at: [c, 4.0, c],
            look: [0.0, 1.0],
            scale: 1.0,
        });
    }
    s += &format!(
        "[StaticModels]\r\nNumberOfStaticModels={}\r\n\r\n",
        models.len()
    );
    for (k, m) in models.iter().enumerate() {
        s += &model_section(k + 1, m);
    }
    s
}

/// What makes the land live: the plants' eco-system (laid over `esb`, a
/// copied track's own), the rivers' water and the flights' files, and
/// the flights' `[AnimationN]` sections numbered from `first`.
pub fn living(course: &Course, field: &Field, stem: &str, esb: Option<(&str, &[u8])>, first: usize) -> (Vec<(String, Vec<u8>)>, Vec<String>) {
    let mut files = Vec::new();
    let plants = crate::nature::plants(course, field);
    if !plants.is_empty() {
        let species = crate::nature::species(course);
        let name = esb.map_or(format!("{stem}.esb"), |(n, _)| n.to_string());
        files.push((name, crate::nature::esb(course.mode.side(), &species, &plants, esb.map(|(_, b)| b))));
    }
    if !field.rivers.is_empty() {
        files.push((format!("{stem}.wtr"), crate::nature::water_file(&field.rivers).into_bytes()));
    }
    let (fly, sections) = crate::flight::files(course, stem, &|x, z| field.height(x, z), first);
    files.extend(fly);
    (files, sections)
}

/// One placed model's `[ModelN]` section.
pub fn model_section(n: usize, m: &Placed) -> String {
    format!(
        "[Model{}]\r\nSLT={}\r\nPosition={:.2},{:.2},{:.2}\r\nUseLighting=T\r\nLookVector={:.4},0.0,{:.4}\r\nUpVector=0.0,1.0,0.0\r\nScale={:.2},{:.2},{:.2}\r\n",
        n, m.model, m.at[0], m.at[1], m.at[2], m.look[0], m.look[1], m.scale, m.scale, m.scale
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mm2_formats::{scn, tdf};

    #[test]
    fn tracks_with_pictures_stay_within_256_terrain_pictures() {
        // A plain stand-in for each picture: grain that repeats.
        let side = 64;
        let rgb: Vec<[f32; 3]> = (0..side * side)
            .map(|i| {
                let v = 0.4 + ((i * 7919) % 97) as f32 / 400.0;
                [v, v * 0.9, v * 0.7]
            })
            .collect();
        let mut pics = Pictures::default();
        for (name, _) in GROUND {
            pics.by_name.insert(name.to_string(), std::sync::Arc::new(Picture { side, rgb: rgb.clone() }));
        }
        for mode in Mode::ALL {
            let c = crate::auto::surprise(mode, 7);
            let baked = write_with(&c, &pics);
            let ar = rs2::Archive::parse(&baked.env).unwrap();
            let e = ar.entries().iter().find(|e| e.name.to_ascii_lowercase().ends_with(".trn")).unwrap();
            let t = trn::parse(ar.data(e)).unwrap();
            assert!(t.textures.len() <= 256, "{mode:?}: {} pictures", t.textures.len());
            eprintln!("{mode:?}: {} terrain pictures", t.textures.len());
        }
    }

    #[test]
    fn plants_rivers_and_flights_go_into_the_track() {
        let mut c = Course::new(Mode::Enduro);
        let side = c.mode.side();
        c.plants.push(crate::Planting { flora: crate::Flora::Mixed, radius: 200.0, density: 0.7, clear: false, everywhere: false, dabs: vec![[side * 0.3, side * 0.3], [side * 0.35, side * 0.3]] });
        c.rivers.push(crate::River { points: vec![[side * 0.1, side * 0.5], [side * 0.9, side * 0.55]], width: 28.0, depth: 2.0 });
        c.flights.push(crate::Flight { craft: crate::Craft::Biplane, points: vec![[side * 0.5, 160.0, side * 0.5]], speed: 110.0 });
        let baked = write(&c);
        let ar = rs2::Archive::parse(&baked.env).unwrap();
        let has = |ext: &str| ar.entries().iter().find(|e| e.name.to_ascii_lowercase().ends_with(ext)).map(|e| ar.data(e));
        let eco = mm2_formats::esb::parse(has(".esb").unwrap()).unwrap();
        assert!(!eco.objects.is_empty());
        assert_eq!(crate::nature::read_water(&String::from_utf8_lossy(has(".wtr").unwrap())).len(), 1);
        assert!(has(".mcf").is_some() && has(".mot").is_some());
        let s = scn::Scene::parse(has(".scn").unwrap()).unwrap();
        assert_eq!(s.animations.len(), 1);
        assert!(s.animations[0].model.ends_with("_fly1.mcf"));
    }

    #[test]
    fn every_mode_bakes_a_track_the_readers_take() {
        for mode in Mode::ALL {
            let c = Course::new(mode);
            let baked = write(&c);
            let ar = rs2::Archive::parse(&baked.env).unwrap();
            let get = |ext: &str| {
                ar.entries()
                    .iter()
                    .find(|e| e.name.to_ascii_lowercase().ends_with(ext))
                    .map(|e| ar.data(e))
                    .unwrap()
            };
            let t = trn::parse(get(".trn")).unwrap();
            assert_eq!(t.size, mode.samples(), "{mode:?}");
            let s = scn::Scene::parse(get(".scn")).unwrap();
            assert_eq!(
                s.terrain_file.to_ascii_lowercase(),
                format!("{}.trn", c.stem().to_ascii_lowercase())
            );
            let d = tdf::parse(get(".tdf")).unwrap();
            assert!(d.nodes[0].sections.len() > 20, "{mode:?}");
            assert!(d.start.is_some());
            if mode.laps() {
                assert!(d.finish.is_some());
            } else {
                let w = String::from_utf8_lossy(get(".wpt")).to_string();
                assert!(w.contains("NumWaypoints="));
                assert!(baked.gates.len() >= 2 && baked.gates.len() <= MOST_GATES);
            }
            // Heights survive the trip through the file.
            let mid = c.path().at(200.0);
            let (fx, fz) = (
                (mid[0] / mode.scale()) as usize,
                (mid[1] / mode.scale()) as usize,
            );
            let want = baked.field.heights[fz * t.size + fx];
            let got = t.height(fx, fz) * t.scale;
            assert!((want - got).abs() < 0.2, "{mode:?} {want} {got}");
            assert_eq!(course_in(&baked.env).unwrap(), c);
            if mode == Mode::Supercross {
                assert!(s.stadium.is_some());
                assert!(s.models.len() > 50, "bales {}", s.models.len());
            }
        }
    }
}
