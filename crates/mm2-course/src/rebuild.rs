//! Rebuilding a track the game's demo has only part of. The Motocross
//! Madness 2 demo carries, for each track it doesn't let you ride, the
//! scene (its scenery and where it stands), the course or the checkpoint
//! gates and the menu pictures, but not the ground. [`rebuild`] lays new
//! ground under them: hills fitted to where the scenery stands, the
//! original route carved in with jumps of the builder's own, painted from
//! the free pack's ground. The scenery is set on the new ground. It is
//! the original's route and scenery on ground that isn't the original's.

use crate::env::Pictures;
use crate::{auto, copy, land, Course, Mode, Point};
use mm2_formats::{rs2, scn, tdf};

/// What a rebuilt track's name says it is.
pub const LABEL: &str = "(Rebuilt)";

/// Which making of the rebuild a track is from: changed whenever the
/// rebuild changes, so tracks built before are built again.
/// Scenery lifted less than this (feet) over the old ground stands on the new ground.
const GROUNDED: f32 = 6.0;
/// How much higher (feet) scenery must sit than what's under it to be on it.
const STACKED: f32 = 2.5;

pub const MADE: &str = "6";

/// The name of the scene file in `files`, and its text.
fn scene_of(files: &[(String, Vec<u8>)]) -> Option<(&str, String)> {
    files
        .iter()
        .find(|(n, _)| n.to_ascii_lowercase().ends_with(".scn"))
        .map(|(n, d)| (n.as_str(), String::from_utf8_lossy(d).into_owned()))
}

/// Whether the archive in `bytes` is a track without its ground that
/// [`rebuild`] can rebuild: a scene for a race it knows, with a course or
/// gates, and no `.trn`.
pub fn can_rebuild(bytes: &[u8], file_name: &str) -> bool {
    let Ok(files) = copy::open(bytes, file_name) else { return false };
    let has = |ext: &str| files.iter().any(|(n, _)| n.to_ascii_lowercase().ends_with(ext));
    !has(".trn") && scene_of(&files).is_some_and(|(_, t)| mode_for(&files, &scn::Ini::parse(&t)).is_some())
}

/// The names the event list shows for a rebuilt track's races: its scene's
/// name, or for a gate race its first race's, with [`LABEL`].
pub fn names(bytes: &[u8], file_name: &str) -> Vec<String> {
    let Ok(files) = copy::open(bytes, file_name) else { return Vec::new() };
    let Some((_, text)) = scene_of(&files) else { return Vec::new() };
    let ini = scn::Ini::parse(&text);
    let info = ini.section("SceneInfo");
    let clean = |s: &str| s.trim().trim_matches('"').to_string();
    let scene = info.and_then(|s| s.get("SceneName")).map(clean).unwrap_or_else(|| file_name.to_string());
    let gates = files.iter().any(|(n, _)| n.to_ascii_lowercase().ends_with(".wpt"));
    let name = if gates { info.and_then(|s| s.get("WaypointRaceName1")).map(clean).unwrap_or(scene) } else { scene };
    vec![format!("{name} {LABEL}")]
}

/// The ground under the scenery: each model stands on it, so the lowest
/// model in each patch of ground gives its height there, and the rest is
/// filled in smoothly between them. With little scenery, the mode's own
/// land recipe fills the gaps. Heights in ft at every sample, rows along +z.
fn fitted_ground(course: &Course, models: &[[f32; 3]]) -> Vec<f32> {
    let mode = course.mode;
    let n = mode.samples();
    let scale = mode.scale();
    let side = mode.side();
    // The land recipe, for the hills away from the scenery.
    let recipe = |x: f32, z: f32| land::height(&course.land, side, x, z);
    // A coarse grid: 8 samples a cell.
    const CELL: usize = 8;
    let m = (n - 1) / CELL + 1;
    let cell_ft = CELL as f32 * scale;
    let mut low = vec![f32::INFINITY; m * m];
    for p in models {
        let (cx, cz) = ((p[0] / cell_ft).round(), (p[2] / cell_ft).round());
        // Height 0 is scenery the scene leaves to stand where it falls.
        if cx < 0.0 || cz < 0.0 || cx >= m as f32 || cz >= m as f32 || !p[1].is_finite() || p[1].abs() < 0.01 {
            continue;
        }
        let k = cz as usize * m + cx as usize;
        low[k] = low[k].min(p[1]);
    }
    // Scenery that doesn't stand on the ground (on a roof, up a pole, in
    // a pit) stands out from its neighbours: left out.
    let mut known: Vec<bool> = low.iter().map(|h| h.is_finite()).collect();
    let mut keep = known.clone();
    for z in 0..m {
        for x in 0..m {
            let k = z * m + x;
            if !known[k] {
                continue;
            }
            let mut near: Vec<f32> = Vec::new();
            for dz in -5i32..=5 {
                for dx in -5i32..=5 {
                    let (a, b) = (x as i32 + dx, z as i32 + dz);
                    if (dx, dz) != (0, 0) && a >= 0 && b >= 0 && (a as usize) < m && (b as usize) < m && known[b as usize * m + a as usize] {
                        near.push(low[b as usize * m + a as usize]);
                    }
                }
            }
            if near.len() >= 3 {
                near.sort_by(f32::total_cmp);
                let median = near[near.len() / 2];
                // At most a steep hillside's worth from the cells round it.
                if (low[k] - median).abs() > 12.0 + 0.35 * 5.0 * cell_ft {
                    keep[k] = false;
                }
            }
        }
    }
    known.copy_from_slice(&keep);
    let count = known.iter().filter(|k| **k).count();
    // Too little scenery to tell the lie of the land: the recipe's own.
    if count < 12 {
        let mut out = vec![0.0; n * n];
        for z in 0..n {
            for x in 0..n {
                out[z * n + x] = recipe(x as f32 * scale, z as f32 * scale);
            }
        }
        return out;
    }
    // The recipe's hills measured against the scenery's heights, so they
    // join up where the scenery ends.
    let mut offset = 0.0;
    for z in 0..m {
        for x in 0..m {
            if known[z * m + x] {
                offset += low[z * m + x] - recipe(x as f32 * cell_ft, z as f32 * cell_ft);
            }
        }
    }
    offset /= count as f32;
    // Filled in smoothly: relaxed from coarse to fine, starting from the
    // recipe's hills.
    let mut g: Vec<f32> = (0..m * m)
        .map(|k| if known[k] { low[k] } else { recipe((k % m) as f32 * cell_ft, (k / m) as f32 * cell_ft) + offset })
        .collect();
    for _ in 0..600 {
        let old = g.clone();
        for z in 0..m {
            for x in 0..m {
                let k = z * m + x;
                if known[k] {
                    continue;
                }
                let mut s = 0.0;
                let mut c = 0.0;
                for (dx, dz) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                    let (a, b) = (x as i32 + dx, z as i32 + dz);
                    if a >= 0 && b >= 0 && (a as usize) < m && (b as usize) < m {
                        s += old[b as usize * m + a as usize];
                        c += 1.0;
                    }
                }
                g[k] = s / c;
            }
        }
    }
    // Smoothed over the scenery's own cells too, so the ground has no
    // steps between them, and no steeper than a hillside can be ridden.
    for _ in 0..6 {
        let old = g.clone();
        for z in 1..m - 1 {
            for x in 1..m - 1 {
                let k = z * m + x;
                g[k] = (old[k] * 4.0 + old[k - 1] + old[k + 1] + old[k - m] + old[k + m]) / 8.0;
            }
        }
    }
    let most = 0.55 * cell_ft;
    for _ in 0..200 {
        let mut moved = false;
        for z in 0..m {
            for x in 0..m {
                let k = z * m + x;
                for j in [k.wrapping_sub(1), k + 1, k.wrapping_sub(m), k + m] {
                    if j >= m * m || (j % m).abs_diff(x) > 1 {
                        continue;
                    }
                    let d = g[k] - g[j];
                    if d.abs() > most {
                        let fix = (d.abs() - most) / 2.0 * d.signum();
                        g[k] -= fix;
                        g[j] += fix;
                        moved = true;
                    }
                }
            }
        }
        if !moved {
            break;
        }
    }
    // How far each cell is from the scenery, in cells (two passes).
    let mut far = vec![f32::INFINITY; m * m];
    for k in 0..m * m {
        if known[k] {
            far[k] = 0.0;
        }
    }
    for _ in 0..2 {
        for z in 0..m {
            for x in 0..m {
                let k = z * m + x;
                if x > 0 {
                    far[k] = far[k].min(far[k - 1] + 1.0);
                }
                if z > 0 {
                    far[k] = far[k].min(far[k - m] + 1.0);
                }
            }
        }
        for z in (0..m).rev() {
            for x in (0..m).rev() {
                let k = z * m + x;
                if x + 1 < m {
                    far[k] = far[k].min(far[k + 1] + 1.0);
                }
                if z + 1 < m {
                    far[k] = far[k].min(far[k + m] + 1.0);
                }
            }
        }
    }
    // Full size: the fitted ground between cells, with the recipe's own
    // small hills coming back away from the scenery.
    let at = |v: &[f32], fx: f32, fz: f32| {
        let (x0, z0) = ((fx.floor() as usize).min(m - 1), (fz.floor() as usize).min(m - 1));
        let (x1, z1) = ((x0 + 1).min(m - 1), (z0 + 1).min(m - 1));
        let (tx, tz) = (fx - x0 as f32, fz - z0 as f32);
        let a = v[z0 * m + x0] * (1.0 - tx) + v[z0 * m + x1] * tx;
        let b = v[z1 * m + x0] * (1.0 - tx) + v[z1 * m + x1] * tx;
        a * (1.0 - tz) + b * tz
    };
    let mut smooth_recipe = vec![0.0; m * m];
    for z in 0..m {
        for x in 0..m {
            smooth_recipe[z * m + x] = recipe(x as f32 * cell_ft, z as f32 * cell_ft);
        }
    }
    let mut out = vec![0.0; n * n];
    for z in 0..n {
        for x in 0..n {
            let (fx, fz) = (x as f32 / CELL as f32, z as f32 / CELL as f32);
            let base = at(&g, fx, fz);
            // The recipe's hills over and above its own smooth shape.
            let hills = recipe(x as f32 * scale, z as f32 * scale) - at(&smooth_recipe, fx, fz);
            let away = (at(&far, fx, fz) / 12.0).clamp(0.0, 1.0);
            out[z * n + x] = base + hills * (0.25 + 0.75 * away);
        }
    }
    out
}

/// The ground eased round the route: within a few hundred feet of it the
/// land comes to the route's own gently rising and falling height, so the
/// track isn't cut into a hillside or raised on a bank.
fn conform(course: &Course, base: &[f32], anchors: &[(f32, f32)]) -> Vec<f32> {
    let path = course.path();
    let n = course.mode.samples();
    let scale = course.mode.scale();
    if path.pts.len() < 2 {
        return base.to_vec();
    }
    let at = |x: f32, z: f32| {
        let i = (x / scale).round().clamp(0.0, (n - 1) as f32) as usize;
        let k = (z / scale).round().clamp(0.0, (n - 1) as f32) as usize;
        base[k * n + i]
    };
    // Every 20 ft along the route, its height smoothed over 400 ft.
    let every = 10;
    let pts: Vec<[f32; 2]> = path.pts.iter().step_by(every).copied().collect();
    let widths: Vec<f32> = path.width.iter().step_by(every).copied().collect();
    let raw: Vec<f32> = pts.iter().map(|p| at(p[0], p[1])).collect();
    let reach = (200.0 / (every as f32 * crate::path::STEP)) as i64;
    let len = pts.len() as i64;
    let mut profile: Vec<f32> = (0..len)
        .map(|i| {
            let (mut sum, mut count) = (0.0, 0.0);
            for d in -reach..=reach {
                let j = if path.closed { (i + d).rem_euclid(len) } else { (i + d).clamp(0, len - 1) };
                sum += raw[j as usize];
                count += 1.0;
            }
            sum / count
        })
        .collect();
    // Scenery standing on the route (a culvert, a start gate, an arch)
    // says how high the route is there: the profile comes to it.
    let along: Vec<f32> = path.s.iter().step_by(every).copied().collect();
    let gap = |a: f32, b: f32| {
        let d = (a - b).abs();
        if path.closed { d.min(path.length - d) } else { d }
    };
    let smooth = profile.clone();
    for (k, p) in profile.iter_mut().enumerate() {
        let (mut pull, mut most) = (0.0, 0.0f32);
        for &(s, y) in anchors {
            let t = (1.0 - (gap(along[k], s) - 30.0).max(0.0) / 160.0).clamp(0.0, 1.0);
            let w = t * t * (3.0 - 2.0 * t);
            if w > most {
                most = w;
                pull = y - smooth[k];
            }
        }
        *p += pull * most;
    }
    const CELL: usize = 8;
    let m = (n - 1) / CELL + 1;
    let cell_ft = CELL as f32 * scale;
    let mut delta = vec![0.0f32; m * m];
    for z in 0..m {
        for x in 0..m {
            let p = [x as f32 * cell_ft, z as f32 * cell_ft];
            let (mut best, mut j) = (f32::INFINITY, 0);
            for (k, q) in pts.iter().enumerate() {
                let d = (q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2);
                if d < best {
                    best = d;
                    j = k;
                }
            }
            let d = best.sqrt();
            let inner = widths[j] / 2.0 + 30.0;
            let outer = inner + 260.0;
            let t = ((d - inner) / (outer - inner)).clamp(0.0, 1.0);
            let w = 1.0 - t * t * (3.0 - 2.0 * t);
            delta[z * m + x] = w * (profile[j] - at(p[0], p[1]));
        }
    }
    let mut out = base.to_vec();
    for z in 0..n {
        for x in 0..n {
            let (fx, fz) = (x as f32 / CELL as f32, z as f32 / CELL as f32);
            let (x0, z0) = ((fx.floor() as usize).min(m - 1), (fz.floor() as usize).min(m - 1));
            let (x1, z1) = ((x0 + 1).min(m - 1), (z0 + 1).min(m - 1));
            let (tx, tz) = (fx - x0 as f32, fz - z0 as f32);
            let a = delta[z0 * m + x0] * (1.0 - tx) + delta[z0 * m + x1] * tx;
            let b = delta[z1 * m + x0] * (1.0 - tx) + delta[z1 * m + x1] * tx;
            out[z * n + x] += a * (1.0 - tz) + b * tz;
        }
    }
    out
}

/// The builder's mode whose ground is the size of the track's: Enduro
/// tracks are as big as Baja's, so they are laid out as Baja.
fn mode_for(files: &[(String, Vec<u8>)], ini: &scn::Ini) -> Option<Mode> {
    let env = ini.section("Environment")?;
    let width: usize = env.get("TerrainWidth")?.trim().parse().ok()?;
    let scale: f32 = env.get("TerrainWidthScale")?.trim().parse().ok()?;
    let has = |ext: &str| files.iter().any(|(n, _)| n.to_ascii_lowercase().ends_with(ext));
    let mode = Mode::ALL.into_iter().find(|m| m.terrain_width() == width && (m.scale() - scale).abs() < 0.01)?;
    // A race the route fits: laps on a course, or gates.
    match mode {
        Mode::Supercross | Mode::National if has(".tdf") => Some(mode),
        Mode::Baja | Mode::Enduro if has(".wpt") => Some(Mode::Baja),
        _ => None,
    }
}

/// The checkpoint gates in a `.wpt`, by number.
fn gates(text: &str) -> Vec<[f32; 2]> {
    let ini = scn::Ini::parse(text);
    let Some(w) = ini.section("Waypoints") else { return Vec::new() };
    let count: usize = w.get("NumWaypoints").and_then(|v| v.trim().parse().ok()).unwrap_or(0);
    (1..=count)
        .filter_map(|k| {
            let x = w.get(&format!("Point{k}X"))?.trim().parse().ok()?;
            let z = w.get(&format!("Point{k}Z"))?.trim().parse().ok()?;
            Some([x, z])
        })
        .collect()
}

/// A number from `name`, the same every time.
fn seed_of(name: &str) -> u32 {
    name.bytes().fold(2166136261u32, |h, b| (h ^ b.to_ascii_lowercase() as u32).wrapping_mul(16777619))
}

/// One INI section's text, by the `[Name]` lines.
fn sections(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = vec![(String::new(), String::new())];
    for line in text.split_inclusive('\n') {
        let t = line.trim();
        if t.starts_with('[') && t.contains(']') {
            let name = t[1..t.find(']').unwrap()].trim().to_string();
            out.push((name, String::new()));
        }
        out.last_mut().unwrap().1.push_str(line);
    }
    out
}

/// `key=x,y,z` moved to `place(x, y, z)`.
fn lifted(line: &str, key: &str, place: &dyn Fn(f32, f32, f32) -> [f32; 3]) -> Option<String> {
    let (k, v) = line.split_once('=')?;
    if !k.trim().eq_ignore_ascii_case(key) {
        return None;
    }
    let p: Vec<f32> = v.trim().split(',').filter_map(|s| s.trim().parse().ok()).collect();
    let [x, y, z] = p[..] else { return None };
    let [x, y, z] = place(x, y, z);
    Some(format!("{}={:.3},{:.3},{:.3}\r\n", k.trim(), x, y, z))
}

/// Scenery that hangs from or sits up on other scenery: wires, aerials,
/// the rails over a trestle, crates on a loading dock.
fn held_up(model: &str) -> bool {
    const WORDS: [&str; 8] = ["wire", "conn", "ant_", "twr-", "trestle", "_dock", "rrtrack", "track"];
    let m = model.to_ascii_lowercase();
    WORDS.iter().any(|w| m.contains(w))
}

/// Scenery that holds other scenery up: towers, poles, posts, trestles.
fn holds_up(model: &str) -> bool {
    const WORDS: [&str; 7] = ["tower", "pole", "post", "twr", "support", "trestle", "ramp"];
    let m = model.to_ascii_lowercase();
    WORDS.iter().any(|w| m.contains(w))
}

/// The ground plan of the buildings and other big solid scenery the
/// demo's tracks use: half its width and half its depth (feet), from the
/// Free Pack's stand-ins. A route through the gates goes round them.
const SOLID: &[(&str, f32, f32)] = &[
    ("aircraftq", 44.0, 58.0),
    ("billboard01", 21.0, 4.0),
    ("boxcarclosed", 5.0, 19.0),
    ("boxcaropen", 5.0, 23.0),
    ("cabin", 24.0, 18.0),
    ("caboose", 5.0, 14.0),
    ("cameraplatform01", 9.0, 6.0),
    ("cargowreck_body", 16.0, 43.0),
    ("cargowreck_lwing", 32.0, 9.0),
    ("cargowreck_nose", 9.0, 16.0),
    ("carport01", 12.0, 5.0),
    ("cattlecar", 5.0, 23.0),
    ("co_opbrick", 38.0, 19.0),
    ("coalcar", 4.0, 17.0),
    ("combine", 19.0, 29.0),
    ("container01", 15.0, 5.0),
    ("dc-3", 48.0, 34.0),
    ("duckblind", 7.0, 8.0),
    ("dumptruck", 9.0, 27.0),
    ("engine", 5.0, 24.0),
    ("farmhouse", 28.0, 18.0),
    ("feduptruck", 4.0, 10.0),
    ("fence_chlink", 17.0, 0.0),
    ("fence_chlinkshort", 9.0, 0.0),
    ("firetruck", 5.0, 15.0),
    ("garage01", 27.0, 27.0),
    ("garage02", 27.0, 27.0),
    ("gasstation", 19.0, 27.0),
    ("gianttire01", 29.0, 7.0),
    ("goalpost", 14.0, 0.0),
    ("greyhound", 5.0, 19.0),
    ("hanger", 64.0, 80.0),
    ("haywagon", 9.0, 21.0),
    ("hopper", 3.0, 8.0),
    ("ht_tower", 32.0, 15.0),
    ("ht_tower02", 22.0, 11.0),
    ("hut", 21.0, 24.0),
    ("loadingramp", 59.0, 22.0),
    ("lodge", 167.0, 60.0),
    ("main_office", 30.0, 49.0),
    ("miningbldg01", 19.0, 25.0),
    ("miningshack01", 18.0, 16.0),
    ("miningshed01", 8.0, 9.0),
    ("missilesilo", 122.0, 87.0),
    ("nattrailer01", 21.0, 16.0),
    ("observatorycone", 65.0, 66.0),
    ("office01", 21.0, 19.0),
    ("pit_trailer", 15.0, 20.0),
    ("pumpshack", 7.0, 9.0),
    ("pyramid", 124.0, 124.0),
    ("quonset", 24.0, 34.0),
    ("radiostation", 21.0, 30.0),
    ("rangerstation", 27.0, 16.0),
    ("rangerstationnosnow", 27.0, 16.0),
    ("retainwall01", 71.0, 2.0),
    ("rrt_elev", 6.0, 28.0),
    ("rrt_elevradius", 12.0, 28.0),
    ("ruin_ziggurat", 20.0, 20.0),
    ("schoolbus", 9.0, 15.0),
    ("scraper", 9.0, 32.0),
    ("semicab", 5.0, 11.0),
    ("semitrailer", 6.0, 31.0),
    ("silo", 94.0, 45.0),
    ("skijump", 13.0, 127.0),
    ("skiliftpost", 14.0, 5.0),
    ("station", 31.0, 36.0),
    ("support", 6.0, 15.0),
    ("switchyard", 40.0, 24.0),
    ("tent01", 17.0, 12.0),
    ("tent02", 17.0, 12.0),
    ("tm_head", 26.0, 24.0),
    ("tractor", 6.0, 9.0),
    ("trailer01", 7.0, 16.0),
    ("trailer02", 3.0, 12.0),
    ("trailer03", 6.0, 24.0),
    ("trailer04", 5.0, 13.0),
    ("trailer05", 6.0, 24.0),
    ("trailer06", 4.0, 16.0),
    ("trailer07", 9.0, 25.0),
    ("trailer08", 4.0, 14.0),
    ("trailer09", 5.0, 19.0),
    ("trailer10", 6.0, 14.0),
    ("trailer11", 5.0, 22.0),
    ("trailer12", 7.0, 26.0),
    ("trailer13", 9.0, 25.0),
    ("trailer14", 5.0, 17.0),
    ("trailer15", 7.0, 25.0),
    ("trailer16", 9.0, 25.0),
    ("trailer17", 5.0, 17.0),
    ("trailer18", 5.0, 17.0),
    ("watertank01", 19.0, 19.0),
    ("watertower", 9.0, 9.0),
    ("watertruck", 4.0, 10.0),
];

/// The half width and depth of `model` when it is a building or other
/// big solid scenery.
fn solid(model: &str) -> Option<(f32, f32)> {
    let m = model.to_ascii_lowercase();
    let m = m.strip_suffix(".slt").or_else(|| m.strip_suffix(".mcf")).unwrap_or(&m);
    SOLID.iter().find(|(n, _, _)| *n == m).map(|&(_, w, d)| (w, d))
}

/// The route `points` bent round the solid scenery in `scene`, so a
/// straight line from gate to gate doesn't run through a building.
fn round_solids(points: &mut Vec<Point>, scene: &scn::Scene) {
    let mut boxes: Vec<([f32; 2], [f32; 2], f32, f32)> = Vec::new();
    for m in &scene.models {
        let Some((w, d)) = solid(&m.model) else { continue };
        let f = [m.look[0], m.look[2]];
        let len = f[0].hypot(f[1]);
        let f = if len > 1e-3 { [f[0] / len, f[1] / len] } else { [0.0, 1.0] };
        let (sx, sz) = (m.scale[0].abs().max(0.1), m.scale[2].abs().max(0.1));
        boxes.push(([m.position[0], m.position[2]], f, w * sx, d * sz));
    }
    if boxes.is_empty() || points.len() < 2 {
        return;
    }
    // Every 15 ft, so a building can't slip between two points.
    let mut dense: Vec<Point> = Vec::new();
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let steps = ((b.x - a.x).hypot(b.z - a.z) / 15.0).ceil().max(1.0) as usize;
        for i in 0..steps {
            let t = i as f32 / steps as f32;
            dense.push(Point { x: a.x + (b.x - a.x) * t, z: a.z + (b.z - a.z) * t, width: a.width });
        }
    }
    dense.push(*points.last().unwrap());
    for round in 0..6 {
        // Smoothed a little so the bend is a curve (not after the last
        // push, which could undo it).
        if round > 0 {
            let old = dense.clone();
            for i in 1..dense.len() - 1 {
                dense[i].x = (old[i - 1].x + 2.0 * old[i].x + old[i + 1].x) / 4.0;
                dense[i].z = (old[i - 1].z + 2.0 * old[i].z + old[i + 1].z) / 4.0;
            }
        }
        let old = dense.clone();
        for (i, p) in dense.iter_mut().enumerate() {
            // The way the route runs here.
            let (a, b) = (old[i.saturating_sub(1)], old[(i + 1).min(old.len() - 1)]);
            let run = [b.x - a.x, b.z - a.z];
            for &(c, f, w, d) in &boxes {
                // In the building's own frame: x across it, z along it.
                let r = [f[1], -f[0]];
                let (dx, dz) = (p.x - c[0], p.z - c[1]);
                let (lx, lz) = (dx * r[0] + dz * r[1], dx * f[0] + dz * f[1]);
                let pad = p.width / 2.0 + 10.0;
                let (ex, ez) = (w + pad, d + pad);
                if lx.abs() >= ex || lz.abs() >= ez {
                    continue;
                }
                // Out to the side, across the way the route runs: round
                // the building, not back the way it came.
                let along_x = (run[0] * r[0] + run[1] * r[1]).abs() >= (run[0] * f[0] + run[1] * f[1]).abs();
                let side = |v: f32| if v >= 0.0 { 1.0 } else { -1.0 };
                let (lx, lz) = if along_x { (lx, ez * side(lz)) } else { (ex * side(lx), lz) };
                p.x = c[0] + lx * r[0] + lz * f[0];
                p.z = c[1] + lx * r[1] + lz * f[1];
            }
        }
    }
    // Back to points 40 ft or more apart, keeping the ends.
    let mut out: Vec<Point> = vec![dense[0]];
    for p in &dense[1..dense.len() - 1] {
        let q = out.last().unwrap();
        if (p.x - q.x).hypot(p.z - q.z) >= 40.0 {
            out.push(*p);
        }
    }
    out.push(*dense.last().unwrap());
    *points = out;
}

/// How far (feet) the route runs over a culvert's middle.
const CULVERT: f32 = 14.0;

/// Scenery the route runs over: a culvert.
fn passed_over(model: &str) -> bool {
    model.to_ascii_lowercase().contains("culvert")
}

/// Scenery the route runs under: a bridge, an overpass, a trestle.
fn passed_under(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    ["bridge", "overpass", "trestle", "tunnel"].iter().any(|w| m.contains(w))
}

/// Whether scenery named `model` is ridden through or under (a culvert,
/// a bridge, an arch, a start gate), so it may stand on the route.
fn ridden_through(model: &str) -> bool {
    const WORDS: [&str; 10] = ["culvert", "bridge", "arch", "gate", "start", "finish", "banner", "tunnel", "overpass", "trestle"];
    let m = model.to_ascii_lowercase();
    WORDS.iter().any(|w| m.contains(w))
}

/// The track in `files` (a demo track's archive, without ground) with
/// new ground under it: the archive's files, ready to pack. Its ground is
/// painted from `pictures` where it has them.
pub fn rebuild(files: &[(String, Vec<u8>)], pictures: &Pictures) -> Result<Vec<(String, Vec<u8>)>, String> {
    let (scn_name, text) = scene_of(files).ok_or("the track has no scene")?;
    let stem = scn_name[..scn_name.len() - 4].to_string();
    let ini = scn::Ini::parse(&text);
    let mode = mode_for(files, &ini).ok_or("the track isn't a size the builder makes")?;
    let info = ini.section("SceneInfo");
    let title = info.and_then(|s| s.get("SceneName")).unwrap_or(&stem).trim().trim_matches('"').to_string();
    let terrain_file = ini
        .section("Environment")
        .and_then(|s| s.get("TerrainFile"))
        .map(|s| s.trim().to_string())
        .unwrap_or(format!("{stem}.trn"));
    let scene = scn::Scene::parse(text.as_bytes()).map_err(|e| e.to_string())?;
    let get = |ext: &str| files.iter().find(|(n, _)| n.to_ascii_lowercase().ends_with(ext));

    let mut c = Course::new(mode);
    c.name = title.clone();
    c.land.seed = seed_of(&stem);
    // Enduro, laid out as Baja: green country, not desert.
    let event = info.and_then(|s| s.get("EventType")).unwrap_or("").trim().to_ascii_lowercase();
    if event.starts_with("enduro") {
        c.land = Course::new(Mode::Enduro).land;
        c.land.seed = seed_of(&stem);
        c.dress = Course::new(Mode::Enduro).dress;
    }
    c.jumps.clear();
    c.objects.clear();
    c.flights.clear();
    c.dress.start_area = false;
    c.dress.edge_bales = false;
    if mode == Mode::Supercross {
        c.dress.trees = 0.0;
    }
    if let Some(sky) = ini.section("Environment").and_then(|s| s.get("CubeFile")) {
        c.look.sky = sky.trim().to_string();
    }
    // The route: the course's own line, or one through the gates.
    let wpt = get(".wpt");
    if let Some((_, t)) = get(".tdf") {
        let def = tdf::parse(t).map_err(|e| e.to_string())?;
        let (points, closed) = copy::line_of(&def, mode);
        c.line.points = points;
        c.line.closed = closed || mode.laps();
        if let Some(s) = def.start_line() {
            if let Some(near) = c.path().nearest([s.position[0], s.position[2]]) {
                c.line.start = near.s;
            }
        }
    } else if let Some((_, w)) = wpt {
        let g = gates(&String::from_utf8_lossy(w));
        let width = mode.track_width();
        let mut points: Vec<Point> = Vec::new();
        // A run up to the first gate (the scenes' own start positions are
        // placeholders the gates override), then gate to gate.
        if let [a, b, ..] = g[..] {
            let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
            let d = dx.hypot(dz).max(1.0);
            points.push(Point { x: a[0] - dx / d * 150.0, z: a[1] - dz / d * 150.0, width });
        }
        // Gate to gate nearly straight (the scenery between, a culvert or
        // a bridge, stands on the straight line), rounding at the gates.
        for (k, p) in g.iter().enumerate() {
            if let Some(q) = k.checked_sub(1).map(|j| g[j]) {
                let d = (p[0] - q[0]).hypot(p[1] - q[1]);
                let steps = (d / 80.0).floor() as usize;
                for i in 1..steps {
                    let t = i as f32 / steps as f32;
                    if (0.18..=0.82).contains(&t) {
                        points.push(Point { x: q[0] + (p[0] - q[0]) * t, z: q[1] + (p[1] - q[1]) * t, width });
                    }
                }
            }
            points.push(Point { x: p[0], z: p[1], width });
        }
        round_solids(&mut points, &scene);
        c.line.points = points;
        c.line.closed = false;
        c.line.start = 0.0;
    }
    if c.line.points.len() < 2 {
        return Err("the track has no route to rebuild".into());
    }
    let seed = c.land.seed;
    auto::fill_jumps(&mut c, seed);
    // The ground fitted to the scenery.
    let models: Vec<[f32; 3]> = scene.models.iter().map(|m| m.position).collect();
    let base = fitted_ground(&c, &models);
    let names: Vec<&str> = scene.models.iter().map(|m| m.model.as_str()).collect();
    // The scenery on the route itself.
    let path = c.path();
    let on_route = |x: f32, z: f32| {
        path.nearest([x, z]).filter(|near| near.side.abs() < path.width[near.i] / 2.0)
    };
    let anchors: Vec<(f32, f32)> = models
        .iter()
        .zip(&names)
        .filter(|(p, name)| p[1].abs() >= 0.01 && ridden_through(name) && !passed_over(name) && !passed_under(name))
        .filter_map(|(p, _)| on_route(p[0], p[2]).map(|near| (near.s, p[1])))
        // A culvert is under the route: the route goes over its top.
        .chain(models.iter().zip(&names).filter(|(p, name)| p[1].abs() >= 0.01 && passed_over(name)).filter_map(|(p, _)| on_route(p[0], p[2]).map(|near| (near.s, p[1] + CULVERT))))
        .collect();
    c.ground = Some(std::sync::Arc::new(conform(&c, &base, &anchors)));
    let baked = crate::env::write_with(&c, pictures);
    let made = rs2::Archive::parse(&baked.env).map_err(|e| e.to_string())?;
    let n = mode.samples();
    let scale = mode.scale();
    let base_at = |x: f32, z: f32| {
        let i = ((x / scale).round().clamp(0.0, (n - 1) as f32)) as usize;
        let k = ((z / scale).round().clamp(0.0, (n - 1) as f32)) as usize;
        base[k * n + i]
    };
    // Where scenery on the new route goes: anything ridden through stays,
    // anything else (hay bales the original route went round) is set
    // beside it.
    let beside = |model: &str, x: f32, z: f32| {
        let near = on_route(x, z);
        let (x, z) = match near {
            Some(near) if !ridden_through(model) && solid(model).is_none() => {
                let d = path.dir[near.i];
                let q = path.pts[near.i];
                let out = path.width[near.i] / 2.0 + 8.0;
                let sign = if near.side >= 0.0 { 1.0 } else { -1.0 };
                (q[0] - d[1] * out * sign, q[1] + d[0] * out * sign)
            }
            _ => (x, z),
        };
        (x, z, near.is_some())
    };
    // The height of what isn't on the scene's model list (animations):
    // on the new ground unless it stood well above the old.
    let loose = |x: f32, y: f32, z: f32, ground: f32| {
        let lift = y - base_at(x, z);
        if y.abs() < 0.01 || lift < GROUNDED { ground } else { ground + lift }
    };
    // The scenery's heights on the new ground. The fitted ground is only
    // near the old one, so scenery doesn't keep its height over it: what
    // stands on the ground (hay bales, tents, signs, buildings) stands on
    // the new ground, what sits on other scenery (stacked crates, a bale
    // on a bale) keeps its height over that, and what hangs from or sits
    // up on a tower, post or trestle (wires, aerials, the rails over a
    // trestle) keeps its height over that. Lowest first, so what holds
    // something up is placed before it.
    let mut order: Vec<usize> = (0..models.len()).collect();
    order.sort_by(|&a, &b| models[a][1].total_cmp(&models[b][1]));
    let mut placed: Vec<Option<[f32; 3]>> = vec![None; models.len()];
    for (at, &j) in order.iter().enumerate() {
        let [x, y, z] = models[j];
        let name = names[j];
        let (nx, nz, on) = beside(name, x, z);
        let ground = baked.field.height(nx, nz);
        let up = held_up(name);
        // A trestle stands on its own legs; what's over it is held up.
        let alone = name.to_ascii_lowercase().contains("trestle");
        let support = order[..at]
            .iter()
            .filter(|&&i| y - models[i][1] > STACKED)
            .filter(|&&i| if up { holds_up(names[i]) } else { true })
            .map(|&i| (i, (models[i][0] - x).hypot(models[i][2] - z)))
            .filter(|&(_, d)| !alone && d < if up { 80.0 } else { 4.0 })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let ny = if on && passed_over(name) {
            ground - CULVERT
        } else if on && passed_under(name) {
            loose(x, y, z, ground)
        } else if y.abs() < 0.01 || on {
            ground
        } else if let Some((i, _)) = support {
            (placed[i].map_or(ground, |p| p[1]) + y - models[i][1]).max(ground)
        } else if up {
            loose(x, y, z, ground)
        } else {
            ground
        };
        placed[j] = Some([nx, ny, nz]);
    }
    let place = |model: &str, x: f32, y: f32, z: f32| -> [f32; 3] {
        if let Some(j) = (0..models.len()).find(|&j| models[j] == [x, y, z]) {
            if let Some(p) = placed[j] {
                return p;
            }
        }
        let (x, z, on) = beside(model, x, z);
        let ground = baked.field.height(x, z);
        [x, if on { ground } else { loose(x, y, z, ground) }, z]
    };

    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    let mut made_scene = String::new();
    for e in made.entries() {
        let l = e.name.to_ascii_lowercase();
        let data = made.data(e).to_vec();
        if l.ends_with(".trn") {
            out.push((terrain_file.clone(), data));
        } else if l.ends_with(".gmt") {
            out.push((format!("{stem}.gmt"), data));
        } else if l.ends_with(".esb") {
            out.push((format!("{stem}.esb"), data));
        } else if l.ends_with(".wtr") {
            out.push((format!("{stem}.wtr"), data));
        } else if l.ends_with(".tdf") {
            let name = if mode.laps() { format!("{stem}.tdf") } else { format!("{stem}01.tdf") };
            out.push((name, data));
        } else if l.ends_with(".wpt") && wpt.is_none() {
            out.push((format!("{stem}01.wpt"), data));
        } else if l.ends_with(".scn") {
            made_scene = String::from_utf8_lossy(&data).into_owned();
        }
        // The builder's previews and course file stay out: the track
        // keeps its own pictures and can't be opened in the builder.
    }
    // The original's scene, on the new ground: its scenery lifted with
    // the carving, the new ground's zones, the ground lit by OpenMCM2,
    // one race of gates.
    let zones: String = sections(&made_scene)
        .into_iter()
        .filter(|(n, _)| n.to_ascii_lowercase().starts_with("terrainzone"))
        .map(|(_, t)| t)
        .collect();
    let mut s = String::new();
    let mut zones_done = false;
    for (name, body) in sections(&text) {
        let l = name.to_ascii_lowercase();
        if l.starts_with("terrainzone") {
            if !zones_done {
                s += &zones;
                zones_done = true;
            }
            continue;
        }
        let moves = l.starts_with("model") || l.starts_with("animation") || l.starts_with("procedural");
        let model = body
            .lines()
            .filter_map(|t| t.split_once('='))
            .find(|(k, _)| matches!(k.trim().to_ascii_lowercase().as_str(), "slt" | "mcf"))
            .map_or(String::new(), |(_, v)| v.trim().to_string());
        let here = |x: f32, y: f32, z: f32| place(&model, x, y, z);
        for line in body.split_inclusive('\n') {
            let key = line.split_once('=').map(|(k, _)| k.trim().to_ascii_lowercase());
            match (l.as_str(), key.as_deref()) {
                ("sceneinfo", Some("scenename")) => s += &format!("SceneName=\"{title} {LABEL}\"\r\n"),
                ("sceneinfo", Some("numberofwaypointraces")) => s += "NumberOfWaypointRaces=1\r\n",
                ("sceneinfo", Some("waypointracename1")) => {
                    let v = line.split_once('=').map_or("", |(_, v)| v).trim().trim_matches('"');
                    s += &format!("WaypointRaceName1=\"{v} {LABEL}\"\r\n");
                }
                ("sceneinfo", Some("lightground")) => {}
                _ if moves => match lifted(line, "Position", &here).or_else(|| lifted(line, "Offset", &here)) {
                    Some(t) => s += &t,
                    None => s += line,
                },
                _ => s += line,
            }
        }
        if l == "sceneinfo" {
            s += "LightGround=1\r\n";
            if !mode.laps() && !body.to_ascii_lowercase().contains("numberofwaypointraces") {
                s += "NumberOfWaypointRaces=1\r\n";
            }
        }
    }
    if !zones_done {
        s += &zones;
    }
    out.push((scn_name.to_string(), s.into_bytes()));
    // The track's own course gates and pictures, the pictures without the
    // demo's "Track unavailable" line.
    for (name, data) in files {
        let l = name.to_ascii_lowercase();
        if l.ends_with(".wpt") {
            out.push((name.clone(), data.clone()));
        } else if l.ends_with(".tga") {
            let large = files.iter().find(|(n, _)| {
                l.ends_with("s.tga") && n.to_ascii_lowercase() == format!("{}.tga", &l[..l.len() - 5])
            });
            let (from, size) = match large {
                Some((_, big)) => (big, crate::preview::SMALL),
                None => (data, crate::preview::LARGE),
            };
            out.push((name.clone(), cleaned(from, size).unwrap_or_else(|| data.clone())));
        }
    }
    Ok(out)
}

/// The rows of the demo's track pictures above the "Track unavailable in
/// Trial Version" line, out of 220.
const CLEAR_ROWS: u32 = 194;

/// A demo track picture (the large one) without its bottom line of text,
/// the rest stretched a little to fill, at `size`, as a 16-bit TGA.
fn cleaned(tga: &[u8], size: (usize, usize)) -> Option<Vec<u8>> {
    let img = mm2_formats::tga::parse(tga).ok()?;
    if (img.width as usize, img.height as usize) != crate::preview::LARGE {
        return None;
    }
    let (w, h) = size;
    let keep = CLEAR_ROWS as f32 * img.height as f32 / 220.0;
    let mut rgb = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        for x in 0..w {
            let fx = ((x as f32 + 0.5) / w as f32 * img.width as f32 - 0.5).clamp(0.0, img.width as f32 - 1.0);
            let fy = ((y as f32 + 0.5) / h as f32 * keep - 0.5).clamp(0.0, keep - 1.0);
            let (x0, y0) = (fx.floor() as u32, fy.floor() as u32);
            let (x1, y1) = ((x0 + 1).min(img.width - 1), (y0 + 1).min(img.height - 1));
            let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
            for c in 0..3 {
                let p = |x: u32, y: u32| img.pixel(x, y)[c] as f32;
                let a = p(x0, y0) * (1.0 - tx) + p(x1, y0) * tx;
                let b = p(x0, y1) * (1.0 - tx) + p(x1, y1) * tx;
                rgb.push((a * (1.0 - ty) + b * ty).round() as u8);
            }
        }
    }
    Some(crate::preview::tga16(&rgb, size))
}

/// A demo track's large picture without its "Track unavailable" line,
/// for the event list before the track is rebuilt.
pub fn cleaned_picture(tga: &[u8]) -> Option<Vec<u8>> {
    cleaned(tga, crate::preview::LARGE)
}

/// [`rebuild`] for an archive's bytes (`file_name` its name, for the
/// game's scrambled archives), packed into a new archive.
pub fn rebuild_env(bytes: &[u8], file_name: &str, pictures: &Pictures) -> Result<Vec<u8>, String> {
    let files = copy::open(bytes, file_name)?;
    let out = rebuild(&files, pictures)?;
    let refs: Vec<(&str, &[u8])> = out.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
    Ok(rs2::build(&refs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_go_round_buildings() {
        let text = "[SceneInfo]\r\nSceneName=t\r\n[Environment]\r\nTerrainFile=a.trn\r\n[StaticModels]\r\nNumberOfStaticModels=1\r\n[Model1]\r\nSLT=Pyramid.slt\r\nPosition=500,0,0\r\n";
        let scene = scn::Scene::parse(text.as_bytes()).unwrap();
        let mut points: Vec<Point> = [0.0, 1000.0].iter().map(|&x| Point { x, z: 0.0, width: 30.0 }).collect();
        round_solids(&mut points, &scene);
        assert!(points.len() > 2);
        // Nowhere inside the pyramid's 248 ft square.
        for p in &points {
            assert!((p.x - 500.0).abs() >= 124.0 || p.z.abs() >= 124.0, "{p:?}");
        }
        assert_eq!((points[0].x, points.last().unwrap().x), (0.0, 1000.0));
    }

    #[test]
    fn reads_gates() {
        let g = gates("[Waypoints]\r\nNumWaypoints=2\r\n\r\nPoint1X=10.5\r\nPoint1Z=20\r\nPoint1LookX=1\r\nPoint1LookZ=0\r\nPoint2X=30\r\nPoint2Z=40\r\n");
        assert_eq!(g, vec![[10.5, 20.0], [30.0, 40.0]]);
    }

    #[test]
    fn lifts_positions() {
        let place = |x: f32, y: f32, z: f32| [x, y + 5.0, z];
        let t = lifted("Position=1.0,2.0,3.0\r\n", "Position", &place).unwrap();
        assert_eq!(t, "Position=1.000,7.000,3.000\r\n");
        assert!(lifted("LookVector=1,0,0", "Position", &place).is_none());
        assert!(ridden_through("Culvert01.slt") && ridden_through("StartGate.slt") && !ridden_through("HayBale.slt"));
    }
}
