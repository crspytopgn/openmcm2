//! A track's preview pictures, drawn from its own ground: the picture the
//! event page shows (`<track>.tga`, or `<track>01.tga` for a track of
//! courses, 452 × 220) and the small one (`<track>S.tga`, 285 × 139), in
//! the game's own 16-bit form. The land is seen from a corner, a little
//! from above, lit by a low sun, in the colours its ground is painted.

use mm2_formats::trn::Terrain;

/// The event page's picture.
pub const LARGE: (usize, usize) = (452, 220);
/// The small one.
pub const SMALL: (usize, usize) = (285, 139);

/// Adds the preview pictures a track's files lack: one for each course
/// (`<course>.tga`, after its `.tdf`) and the small one (`<track>S.tga`).
pub fn add(stem: &str, files: &mut Vec<(String, Vec<u8>)>) {
    let has = |files: &[(String, Vec<u8>)], name: &str| files.iter().any(|(n, _)| n.eq_ignore_ascii_case(name));
    let courses: Vec<String> = files
        .iter()
        .filter_map(|(n, _)| n.strip_suffix(".tdf").or_else(|| n.strip_suffix(".TDF")).map(str::to_string))
        .collect();
    let mut made = Vec::new();
    for course in &courses {
        let name = format!("{course}.tga");
        if !has(files, &name) {
            if let Some(rgb) = picture(course, files, LARGE) {
                made.push((name, tga16(&rgb, LARGE)));
            }
        }
    }
    let small = format!("{stem}S.tga");
    if !has(files, &small) {
        let first = courses.first().map_or(stem, String::as_str);
        if let Some(rgb) = picture(first, files, SMALL) {
            made.push((small, tga16(&rgb, SMALL)));
        }
    }
    files.extend(made);
}

/// The picture for preview `stem` (`CHANOS`, `BAJA01`), RGB8, from a
/// track's files: its terrain, looking over the course of the same name
/// (or the first).
pub fn picture(stem: &str, files: &[(String, Vec<u8>)], size: (usize, usize)) -> Option<Vec<u8>> {
    let ends = |n: &str, e: &str| n.to_ascii_lowercase().ends_with(e);
    let trn = files.iter().find(|(n, _)| ends(n, ".trn"))?;
    let terrain = mm2_formats::trn::parse(&trn.1).ok()?;
    let tdf = files
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(&format!("{stem}.tdf")))
        .or_else(|| files.iter().find(|(n, _)| ends(n, ".tdf")));
    let def = tdf.and_then(|(_, b)| mm2_formats::tdf::parse(b).ok());
    let s = terrain.scale.max(1e-3);
    let route: Vec<[f32; 3]> = def
        .iter()
        .flat_map(|d| d.nodes.iter().flat_map(|n| &n.sections))
        .map(|c| [c.center[0] / s, c.center[1] / s, c.center[2] / s])
        .collect();
    let focus = def.as_ref().and_then(|d| {
        let mut lo = [f32::MAX; 2];
        let mut hi = [f32::MIN; 2];
        for c in d.nodes.iter().flat_map(|n| &n.sections) {
            for p in [c.center, c.edges[0], c.edges[1]] {
                lo = [lo[0].min(p[0]), lo[1].min(p[2])];
                hi = [hi[0].max(p[0]), hi[1].max(p[2])];
            }
        }
        (lo[0] < hi[0]).then(|| [lo[0] / s, lo[1] / s, hi[0] / s, hi[1] / s])
    });
    Some(draw(&terrain, size, focus, &route))
}

/// The picture, RGB8, `size` pixels, looking over `focus` (a box in
/// terrain units, x0 z0 x1 z1), or the whole land.
///
/// Over a course that roams most of the land (baja, enduro), the eye is
/// higher and the `route` (points in terrain units) is drawn on, so one
/// track tells from another.
pub fn draw(t: &Terrain, size: (usize, usize), focus: Option<[f32; 4]>, route: &[[f32; 3]]) -> Vec<u8> {
    let (w, h) = size;
    let ground = colours(t);
    let n = t.size.max(2);
    let last = (n - 1) as f32;
    let [fx0, fz0, fx1, fz1] = focus.unwrap_or([0.0, 0.0, last, last]).map(|v| v.clamp(0.0, last));
    // Fine enough over the course, coarse enough to stay quick.
    let span = (fx1 - fx0).max(fz1 - fz0).max(8.0);
    let every = ((span / 140.0) as usize).max((n / 400).max(1)).max(1);
    let g = (n - 1) / every + 1;
    let at = |i: usize, j: usize| ((i * every).min(n - 1), (j * every).min(n - 1));
    let height = |x: usize, z: usize| t.height(x, z);
    // The eye: off the course's south-west corner, looking over it.
    let centre = [(fx0 + fx1) / 2.0, (fz0 + fz1) / 2.0];
    let mut ground_y = 0.0;
    let mut count = 0.0;
    for j in 0..=8 {
        for i in 0..=8 {
            let x = (fx0 + (fx1 - fx0) * i as f32 / 8.0) as usize;
            let z = (fz0 + (fz1 - fz0) * j as f32 / 8.0) as usize;
            ground_y += height(x.min(n - 1), z.min(n - 1));
            count += 1.0;
        }
    }
    ground_y /= count;
    let target = [centre[0], ground_y, centre[1]];
    let roams = (fx1 - fx0).max(fz1 - fz0) > last * 0.4;
    let (yaw, pitch) = (std::f32::consts::FRAC_PI_4 + 0.15, if roams { 0.95f32 } else { 0.50 });
    let forward = norm([yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos()]);
    let radius = ((fx1 - fx0).powi(2) + (fz1 - fz0).powi(2)).sqrt() / 2.0;
    let distance = radius * if roams { 2.3 } else { 1.6 } + 4.0;
    let eye = [0, 1, 2].map(|q| target[q] - forward[q] * distance);
    let right = norm([forward[2], 0.0, -forward[0]]);
    let up = [
        forward[1] * right[2] - forward[2] * right[1],
        forward[2] * right[0] - forward[0] * right[2],
        forward[0] * right[1] - forward[1] * right[0],
    ];
    let lens = w as f32 / 2.0 / (0.5f32 * 1.15).tan();
    let near = distance * 0.05;
    let place = |x: f32, y: f32, z: f32| {
        let v = [x - eye[0], y - eye[1], z - eye[2]];
        let dot = |a: [f32; 3]| v[0] * a[0] + v[1] * a[1] + v[2] * a[2];
        let (r, u, f) = (dot(right), dot(up), dot(forward));
        (f >= near).then(|| (w as f32 / 2.0 - r / f * lens, h as f32 / 2.0 - u / f * lens, 1.0 / f))
    };
    let screen: Vec<(f32, f32, f32)> = (0..g * g)
        .map(|k| {
            let (x, z) = at(k % g, k / g);
            let v = [x as f32 - eye[0], height(x, z) - eye[1], z as f32 - eye[2]];
            let dot = |a: [f32; 3]| v[0] * a[0] + v[1] * a[1] + v[2] * a[2];
            let (r, u, f) = (dot(right), dot(up), dot(forward));
            if f < near {
                return (f32::NAN, f32::NAN, f32::NAN);
            }
            // Nearer is larger.
            (w as f32 / 2.0 - r / f * lens, h as f32 / 2.0 - u / f * lens, 1.0 / f)
        })
        .collect();
    // Each corner's colour, lit by a low sun.
    let sun = norm([-0.55, 0.55, 0.62]);
    let lit: Vec<[f32; 3]> = (0..g * g)
        .map(|k| {
            let (x, z) = at(k % g, k / g);
            let hx = height((x + every).min(n - 1), z) - height(x.saturating_sub(every), z);
            let hz = height(x, (z + every).min(n - 1)) - height(x, z.saturating_sub(every));
            let normal = norm([-hx, 2.0 * every as f32, -hz]);
            let d = (normal[0] * sun[0] + normal[1] * sun[1] + normal[2] * sun[2]).max(0.0);
            let light = 0.45 + 0.75 * d;
            let c = ground[z * n + x];
            [c[0] * light * 1.03, c[1] * light, c[2] * light * 0.95]
        })
        .collect();
    // The sky behind, pale at the horizon.
    let mut rgb = vec![[0.0f32; 3]; w * h];
    for y in 0..h {
        let c = mix([0.42, 0.60, 0.82], [0.82, 0.85, 0.86], (y as f32 / h as f32 * 1.6).min(1.0));
        for x in 0..w {
            rgb[y * w + x] = c;
        }
    }
    let mut depth = vec![0.0f32; w * h];
    for j in 0..g - 1 {
        for i in 0..g - 1 {
            let k = j * g + i;
            for tri in [[k, k + 1, k + g], [k + 1, k + g + 1, k + g]] {
                if tri.iter().any(|v| screen[*v].0.is_nan()) {
                    continue;
                }
                triangle(&mut rgb, &mut depth, size, tri.map(|v| (screen[v], lit[v])));
            }
        }
    }
    if roams {
        // The way round, a soft pale ribbon, a little over the ground.
        let ink = [0.98, 0.92, 0.70];
        for pair in route.windows(2) {
            let ([ax, ay, az], [bx, by, bz]) = (pair[0], pair[1]);
            if (ax - bx).abs() + (az - bz).abs() > 40.0 {
                continue; // a jump between courses' ends
            }
            let (Some(a), Some(b)) = (place(ax, ay + 1.0, az), place(bx, by + 1.0, bz)) else { continue };
            let steps = ((b.0 - a.0).abs().max((b.1 - a.1).abs())).ceil().max(1.0) as usize;
            for k in 0..=steps {
                let f = k as f32 / steps as f32;
                let (x, y) = (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f);
                for (dx, dy, share) in [(0.0, 0.0, 0.45), (1.0, 0.0, 0.15), (0.0, 1.0, 0.15)] {
                    let (px, py) = ((x + dx) as isize, (y + dy) as isize);
                    if px >= 0 && py >= 0 && (px as usize) < w && (py as usize) < h {
                        let i = py as usize * w + px as usize;
                        rgb[i] = mix(rgb[i], ink, share);
                    }
                }
            }
        }
    }
    // Haze with distance, as the game has.
    let mut out = Vec::with_capacity(w * h * 3);
    for (c, d) in rgb.iter().zip(&depth) {
        let c = if *d > 0.0 {
            let far = ((1.0 / d) / (distance * 3.0)).clamp(0.0, 1.0);
            mix(*c, [0.76, 0.80, 0.84], far.powf(1.3) * 0.5)
        } else {
            *c
        };
        for q in c {
            out.push((q.clamp(0.0, 1.0).powf(0.9) * 255.0).round() as u8);
        }
    }
    out
}

/// The ground's colour at every sample, from the textures over it.
fn colours(t: &Terrain) -> Vec<[f32; 3]> {
    let n = t.size;
    let mut out = vec![[0.42, 0.36, 0.28]; n * n];
    // The textures at a few pixels a sample at most.
    let small: Vec<(u32, Vec<u8>)> = t
        .textures
        .iter()
        .map(|tex| {
            let level = tex.levels.len().saturating_sub(1).min(2);
            ((tex.size >> level).max(1), tex.rgba8(level))
        })
        .collect();
    for tile in &t.tiles {
        let Some((side, px)) = small.get(tile.texture as usize) else { continue };
        let size = tile.size.max(1);
        for z in tile.z..(tile.z + size + 1).min(n as u32) {
            for x in tile.x..(tile.x + size + 1).min(n as u32) {
                let u = ((x - tile.x) * side / size).min(side - 1);
                let v = ((z - tile.z) * side / size).min(side - 1);
                let p = ((v * side + u) * 4) as usize;
                if let Some(c) = px.get(p..p + 3) {
                    out[z as usize * n + x as usize] = [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0];
                }
            }
        }
    }
    out
}

type Corner = ((f32, f32, f32), [f32; 3]);

fn triangle(rgb: &mut [[f32; 3]], depth: &mut [f32], (w, h): (usize, usize), v: [Corner; 3]) {
    let [(a, ca), (b, cb), (c, cc)] = v;
    let area = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    if area.abs() < 1e-6 {
        return;
    }
    let x0 = a.0.min(b.0).min(c.0).floor().max(0.0) as usize;
    let x1 = (a.0.max(b.0).max(c.0).ceil() as isize).min(w as isize - 1);
    let y0 = a.1.min(b.1).min(c.1).floor().max(0.0) as usize;
    let y1 = (a.1.max(b.1).max(c.1).ceil() as isize).min(h as isize - 1);
    if x1 < 0 || y1 < 0 {
        return;
    }
    for y in y0..=y1 as usize {
        for x in x0..=x1 as usize {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let wa = ((b.0 - px) * (c.1 - py) - (b.1 - py) * (c.0 - px)) / area;
            let wb = ((c.0 - px) * (a.1 - py) - (c.1 - py) * (a.0 - px)) / area;
            let wc = 1.0 - wa - wb;
            if wa < -1e-4 || wb < -1e-4 || wc < -1e-4 {
                continue;
            }
            let d = wa * a.2 + wb * b.2 + wc * c.2;
            let i = y * w + x;
            if d > depth[i] {
                depth[i] = d;
                rgb[i] = [0, 1, 2].map(|q| wa * ca[q] + wb * cb[q] + wc * cc[q]);
            }
        }
    }
}

fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
    [v[0] / l, v[1] / l, v[2] / l]
}

fn mix(a: [f32; 3], b: [f32; 3], f: f32) -> [f32; 3] {
    [0, 1, 2].map(|q| a[q] + (b[q] - a[q]) * f)
}

/// A 16-bit (5:5:5) TGA, bottom row first, as the game's own previews.
pub fn tga16(rgb: &[u8], (w, h): (usize, usize)) -> Vec<u8> {
    let mut out = vec![0u8; 18];
    out[2] = 2;
    out[12..14].copy_from_slice(&(w as u16).to_le_bytes());
    out[14..16].copy_from_slice(&(h as u16).to_le_bytes());
    out[16] = 16;
    for y in (0..h).rev() {
        for x in 0..w {
            let p = &rgb[(y * w + x) * 3..];
            let q = |v: u8| (v as u16 * 31 + 127) / 255;
            let c = 0x8000 | q(p[0]) << 10 | q(p[1]) << 5 | q(p[2]);
            out.extend_from_slice(&c.to_le_bytes());
        }
    }
    out
}
