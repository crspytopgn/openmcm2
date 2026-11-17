//! Editing a copy of a track that already exists: the game's own, a free
//! pack track or one downloaded. The copy keeps the track's ground, its
//! pictures, its scene and its objects; the builder adds jumps, shapes and
//! paints the ground, places objects and can redraw the line. The bake
//! then writes the track again with the changed ground, leaving the
//! original file as it was.

use crate::path::Path;
use crate::{Course, Field, Mode, Point, Surface};
use mm2_formats::{faoe, rs2, tdf, trn, Container};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Which track a copy was made from (kept in the `.course` file; the
/// track itself is read again when the course is opened).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Base {
    /// The track's file, as it was opened.
    pub file: String,
    /// Which of its courses (Baja and Enduro tracks have several).
    pub course: usize,
}

/// The track a copy was made from, read in.
pub struct Source {
    /// The archive, its files by name.
    pub files: Vec<(String, Vec<u8>)>,
    /// Samples a side, ft between them, and the heights (ft) and zones.
    pub n: usize,
    pub scale: f32,
    pub heights: Vec<f32>,
    pub surfaces: Vec<u8>,
    /// The terrain's pictures and which 64-sample tile each covers.
    pub textures: Vec<(u32, Vec<u16>)>,
    pub tiles: Vec<Option<u16>>,
    /// The course file (`.tdf`) and gate file (`.wpt`) of the course
    /// copied, by name.
    pub tdf: String,
    pub wpt: Option<String>,
    /// The line as read, to tell whether it was redrawn.
    pub line: Vec<Point>,
    pub start: f32,
    /// The ground's colour at each sample, from its pictures.
    pub colours: Vec<[u8; 3]>,
}

impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Source({} files, {} samples)", self.files.len(), self.n)
    }
}

impl PartialEq for Source {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Source {
    /// The ground's height at (x, z) ft, between samples.
    pub fn height(&self, x: f32, z: f32) -> f32 {
        let n = self.n;
        let fx = (x / self.scale).clamp(0.0, (n - 1) as f32);
        let fz = (z / self.scale).clamp(0.0, (n - 1) as f32);
        let (x0, z0) = (fx.floor() as usize, fz.floor() as usize);
        let (x1, z1) = ((x0 + 1).min(n - 1), (z0 + 1).min(n - 1));
        let (tx, tz) = (fx - x0 as f32, fz - z0 as f32);
        let h = |x: usize, z: usize| self.heights[z * n + x];
        let a = h(x0, z0) * (1.0 - tx) + h(x1, z0) * tx;
        let b = h(x0, z1) * (1.0 - tx) + h(x1, z1) * tx;
        a * (1.0 - tz) + b * tz
    }

    /// The ground's colour at (x, z), 0 to 1.
    pub fn colour(&self, x: f32, z: f32) -> [f32; 3] {
        let n = self.n;
        let ix = ((x / self.scale).round().max(0.0) as usize).min(n - 1);
        let iz = ((z / self.scale).round().max(0.0) as usize).min(n - 1);
        self.colours[iz * n + ix].map(|v| v as f32 / 255.0)
    }

    pub fn surface(&self, x: f32, z: f32) -> u8 {
        let n = self.n;
        let ix = ((x / self.scale).round().max(0.0) as usize).min(n - 1);
        let iz = ((z / self.scale).round().max(0.0) as usize).min(n - 1);
        self.surfaces[iz * n + ix]
    }
}

/// Opens the archive in an `.ENV` file's bytes (the game's own are
/// scrambled, keyed on the file's name).
fn open(bytes: &[u8], file_name: &str) -> Result<Vec<(String, Vec<u8>)>, String> {
    let plain = match Container::detect(bytes) {
        Container::Faoe => faoe::decode(bytes, file_name).map_err(|e| e.to_string())?,
        _ => bytes.to_vec(),
    };
    let ar = rs2::Archive::parse(&plain).map_err(|e| e.to_string())?;
    Ok(ar.entries().iter().map(|e| (e.name.clone(), ar.data(e).to_vec())).collect())
}

/// The mode a track is for, from its scene's `EventType`.
pub fn mode_of(files: &[(String, Vec<u8>)]) -> Option<Mode> {
    let (_, scn) = files.iter().find(|(n, _)| n.to_ascii_lowercase().ends_with(".scn"))?;
    let text = String::from_utf8_lossy(scn).to_ascii_lowercase();
    let line = text.lines().find(|l| l.trim_start().starts_with("eventtype"))?;
    let v = line.split_once('=')?.1.trim();
    Some(match v {
        v if v.starts_with("supercross") || v.starts_with("sx") => Mode::Supercross,
        v if v.starts_with("national") => Mode::National,
        v if v.starts_with("baja") => Mode::Baja,
        v if v.starts_with("enduro") => Mode::Enduro,
        _ => return None,
    })
}

/// Reads the track at `file` (its bytes in `bytes`) for a copy of its
/// `course`th course.
pub fn read(bytes: &[u8], file: &str, mode: Mode, course: usize) -> Result<Source, String> {
    let name = std::path::Path::new(file).file_name().map_or(file.to_string(), |n| n.to_string_lossy().to_string());
    let files = open(bytes, &name)?;
    let (_, t) = files
        .iter()
        .find(|(n, _)| n.to_ascii_lowercase().ends_with(".trn"))
        .ok_or("the track has no ground (.trn)")?;
    let t = trn::parse(t).map_err(|e| e.to_string())?;
    if t.size != mode.samples() || (t.scale - mode.scale()).abs() > 0.01 {
        return Err(format!(
            "its ground is {} samples {} ft apart, not the {} the builder makes for {}",
            t.size,
            t.scale,
            mode.samples(),
            mode.name()
        ));
    }
    let heights: Vec<f32> = t.heights.iter().map(|h| h * t.scale).collect();
    // The pictures as the writer takes them: RGB555, the largest level.
    let textures: Vec<(u32, Vec<u16>)> = t
        .textures
        .iter()
        .map(|x| {
            let px = match x.format {
                trn::PixelFormat::Rgb555 => x.levels[0].clone(),
                _ => x
                    .rgba8(0)
                    .chunks_exact(4)
                    .map(|c| (c[0] as u16 >> 3) << 10 | (c[1] as u16 >> 3) << 5 | c[2] as u16 >> 3)
                    .collect(),
            };
            (x.size, px)
        })
        .collect();
    let tiles_side = ((t.size - 1) / 16).div_ceil(4);
    let mut tiles = vec![None; tiles_side * tiles_side];
    for tile in &t.tiles {
        // Tiles of other sizes cover several 64-sample squares.
        let span = (tile.size / 64).max(1) as usize;
        for dz in 0..span {
            for dx in 0..span {
                let (x, z) = (tile.x as usize / 64 + dx, tile.z as usize / 64 + dz);
                if x < tiles_side && z < tiles_side {
                    tiles[z * tiles_side + x] = Some(tile.texture);
                }
            }
        }
    }
    // The course: `<stem>.tdf` for lap tracks; Baja and Enduro number
    // theirs, `<stem>01.tdf` on.
    let mut tdfs: Vec<&String> = files.iter().map(|(n, _)| n).filter(|n| n.to_ascii_lowercase().ends_with(".tdf")).collect();
    tdfs.sort_by_key(|n| n.to_ascii_lowercase());
    let tdf_name = tdfs.get(course).or(tdfs.first()).map(|s| s.to_string()).ok_or("the track has no course (.tdf)")?;
    let wpt = {
        let stem = &tdf_name[..tdf_name.len() - 4];
        files.iter().map(|(n, _)| n).find(|n| n.eq_ignore_ascii_case(&format!("{stem}.wpt"))).cloned()
    };
    let def = tdf::parse(&files.iter().find(|(n, _)| *n == tdf_name).unwrap().1).map_err(|e| e.to_string())?;
    let (line, closed) = line_of(&def, mode);
    let colours = colours(t.size, &textures, &tiles);
    let mut src = Source {
        files,
        n: t.size,
        scale: t.scale,
        heights,
        surfaces: t.surfaces,
        textures,
        tiles,
        tdf: tdf_name,
        wpt,
        line,
        start: 0.0,
        colours,
    };
    // Where the start line crosses the line.
    if let Some(s) = def.start_line() {
        let path = Path::new(&src.line, closed);
        if let Some(near) = path.nearest([s.position[0], s.position[2]]) {
            src.start = near.s;
        }
    }
    Ok(src)
}

/// Each sample's colour from the tile picture over it.
fn colours(n: usize, textures: &[(u32, Vec<u16>)], tiles: &[Option<u16>]) -> Vec<[u8; 3]> {
    let side = ((n - 1) / 16).div_ceil(4);
    let mut out = vec![[120u8, 110, 90]; n * n];
    for (k, c) in out.iter_mut().enumerate() {
        let (x, z) = (k % n, k / n);
        let (tx, tz) = ((x / 64).min(side - 1), (z / 64).min(side - 1));
        let Some(Some(t)) = tiles.get(tz * side + tx) else { continue };
        let Some((size, px)) = textures.get(*t as usize) else { continue };
        let size = *size as usize;
        let (u, v) = ((x - tx * 64) * size / 64, (z - tz * 64) * size / 64);
        let p = px[v.min(size - 1) * size + u.min(size - 1)];
        let ch = |shift: u16| {
            let v = (p >> shift & 31) as u8;
            v << 3 | v >> 2
        };
        *c = [ch(10), ch(5), ch(0)];
    }
    out
}

/// The line through the course's cross-sections: its first node, in
/// driving order, with the course's width at each.
fn line_of(def: &tdf::TrackDef, mode: Mode) -> (Vec<Point>, bool) {
    // The lap: the longest node that leads back into itself (Supercross
    // tracks start on a chute of their own that joins it).
    let looping = def
        .nodes
        .iter()
        .enumerate()
        .filter(|(i, n)| n.links.contains(&(*i as u32)))
        .max_by_key(|(_, n)| n.sections.len());
    let Some((ix, node)) = looping.or(def.nodes.iter().enumerate().max_by_key(|(_, n)| n.sections.len())) else {
        return (Vec::new(), true);
    };
    let mut pts: Vec<Point> = Vec::new();
    for s in &node.sections {
        let [a, b] = s.edges;
        let w = ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
        let width = if w > 4.0 { w.clamp(10.0, 120.0) } else { mode.track_width() };
        let p = Point { x: s.center[0], z: s.center[2], width };
        // Skip points closer than a few feet: they make kinks.
        if pts.last().is_none_or(|q| (q.x - p.x).hypot(q.z - p.z) > 8.0) {
            pts.push(p);
        }
    }
    // A course that comes round to its first point is a loop.
    let closed = node.links.contains(&(ix as u32)) || pts.len() > 2 && {
        let (a, b) = (pts[0], pts[pts.len() - 1]);
        (a.x - b.x).hypot(a.z - b.z) < 40.0
    };
    if closed && pts.len() > 2 {
        let (a, b) = (pts[0], pts[pts.len() - 1]);
        if (a.x - b.x).hypot(a.z - b.z) < 20.0 {
            pts.pop();
        }
    }
    (pts, closed)
}

/// A new course: a copy of the track in `bytes` (from `file`), named
/// `name`. Its mode comes from the track's scene unless given.
pub fn import(bytes: &[u8], file: &str, mode: Option<Mode>, course: usize, name: &str) -> Result<Course, String> {
    let name_of = std::path::Path::new(file).file_name().map_or(file.to_string(), |n| n.to_string_lossy().to_string());
    let mode = match mode {
        Some(m) => m,
        None => mode_of(&open(bytes, &name_of)?).ok_or("can't tell what kind of race the track is for")?,
    };
    let src = read(bytes, file, mode, course)?;
    let mut c = Course::new(mode);
    c.name = name.to_string();
    c.line.points = src.line.clone();
    c.line.closed = true;
    c.line.start = src.start;
    c.line.berms = 0.0;
    c.jumps.clear();
    c.objects.clear();
    c.dress.edge_bales = false;
    c.dress.trees = 0.0;
    c.dress.start_area = false;
    c.base = Some(Base { file: file.to_string(), course });
    c.source = Some(Arc::new(src));
    Ok(c)
}

/// Reads a copy's track again, for a course opened from its file.
pub fn attach(c: &mut Course, bytes: &[u8]) -> Result<(), String> {
    let base = c.base.clone().ok_or("not a copy")?;
    c.source = Some(Arc::new(read(bytes, &base.file, c.mode, base.course)?));
    Ok(())
}

/// Whether the line is still the track's own.
pub fn same_line(c: &Course, src: &Source) -> bool {
    c.line.points == src.line && (c.line.start - src.start).abs() < 0.5 && c.line.closed
}

/// The copy's ground pictures with the builder's paint laid on: each tile
/// the paint touches gets its own picture, the paint blended in softly
/// over the track's own, so painting changes the look as well as the grip.
fn repainted(
    c: &Course,
    src: &Source,
    field: &Field,
    ready: &[Option<crate::env::Ready>],
) -> (Vec<(u32, Vec<u16>)>, Vec<Option<u16>>) {
    let mut textures = src.textures.clone();
    let mut tiles = src.tiles.clone();
    if c.paint.is_empty() {
        return (textures, tiles);
    }
    let n = field.n;
    let step = field.step;
    // Which samples are painted, with what, and how far in from the rim.
    let mut painted: Vec<Option<(Surface, f32)>> = vec![None; n * n];
    for p in &c.paint {
        let r = p.radius.max(step);
        for d in &p.dabs {
            crate::bake::each_within(n, step, *d, r, |k, fall| {
                let w = (fall * 3.0).min(1.0);
                let keep = match painted[k] {
                    Some((s, old)) if s == p.surface => old.max(w),
                    _ => w,
                };
                painted[k] = Some((p.surface, keep));
            });
        }
    }
    let side = (n - 1) / 64;
    for tz in 0..side {
        for tx in 0..side {
            let touched = (0..=64).any(|j| {
                (0..=64).any(|i| painted[(tz * 64 + j).min(n - 1) * n + (tx * 64 + i).min(n - 1)].is_some())
            });
            if !touched {
                continue;
            }
            let slot = tz * side + tx;
            let Some(Some(t)) = tiles.get(slot).copied() else { continue };
            let Some((size, px)) = textures.get(t as usize).cloned() else { continue };
            let s = size as usize;
            if px.len() != s * s {
                continue;
            }
            let mut out = px.clone();
            let per = 64.0 / s as f32;
            for j in 0..s {
                for i in 0..s {
                    let (fx, fz) = (tx as f32 * 64.0 + (i as f32 + 0.5) * per, tz as f32 * 64.0 + (j as f32 + 0.5) * per);
                    let (x0, z0) = ((fx.floor() as usize).min(n - 2), (fz.floor() as usize).min(n - 2));
                    let (ax, az) = (fx - x0 as f32, fz - z0 as f32);
                    let mut col = [0.0f32; 3];
                    let mut cover = 0.0;
                    for (dx, dz, w) in [(0, 0, (1.0 - ax) * (1.0 - az)), (1, 0, ax * (1.0 - az)), (0, 1, (1.0 - ax) * az), (1, 1, ax * az)] {
                        let Some((surface, keep)) = painted[(z0 + dz) * n + x0 + dx] else { continue };
                        let m = crate::env::material(surface);
                        let pc = match ready.get(m).and_then(Option::as_ref) {
                            Some(r) => r.at(fx * step, fz * step),
                            None => crate::env::target(m),
                        };
                        for q in 0..3 {
                            col[q] += pc[q] * w * keep;
                        }
                        cover += w * keep;
                    }
                    if cover <= 0.0 {
                        continue;
                    }
                    let k = j * s + i;
                    let old = out[k];
                    let o = [((old >> 10) & 31) as f32 / 31.0, ((old >> 5) & 31) as f32 / 31.0, (old & 31) as f32 / 31.0];
                    let q5 = |v: f32| ((v.clamp(0.0, 1.0) * 31.0).round() as u16) & 31;
                    let mut c3 = [0.0; 3];
                    for q in 0..3 {
                        // col already carries the cover's weight.
                        c3[q] = o[q] * (1.0 - cover) + col[q];
                    }
                    out[k] = q5(c3[0]) << 10 | q5(c3[1]) << 5 | q5(c3[2]);
                }
            }
            tiles[slot] = Some(textures.len() as u16);
            textures.push((size, out));
        }
    }
    (textures, tiles)
}

/// The copy's files: the track's own with the new ground, the new name,
/// the objects raised or lowered with the ground and the builder's added,
/// and a new course and gates if the line was redrawn.
#[allow(clippy::too_many_arguments)]
pub fn files(
    c: &Course,
    src: &Source,
    field: &Field,
    ready: &[Option<crate::env::Ready>],
    added: &[crate::env::Placed],
    tdf: impl FnOnce() -> Vec<u8>,
    wpt: impl FnOnce() -> String,
    start: ([f32; 2], [f32; 2]),
) -> Result<Vec<(String, Vec<u8>)>, String> {
    let (textures, tiles) = repainted(c, src, field, ready);
    let terrain = trn::Build {
        size: field.n,
        heights: &field.heights,
        surfaces: &field.surfaces,
        scale: src.scale,
        textures: &textures,
        tiles: &tiles,
    }
    .write()
    .map_err(|e| e.to_string())?;
    let redrawn = !same_line(c, src);
    let laps = c.mode.laps();
    let (tdf_stem, _) = src.tdf.split_at(src.tdf.len() - 4);
    // Only the copied course goes along, as the first.
    let new_tdf = if laps { src.tdf.clone() } else { format!("{}01.tdf", &tdf_stem[..tdf_stem.len().saturating_sub(2)]) };
    let new_wpt = format!("{}.wpt", &new_tdf[..new_tdf.len() - 4]);
    let mut out = Vec::new();
    let mut tdf = Some(tdf);
    let mut wpt = Some(wpt);
    for (name, data) in &src.files {
        let l = name.to_ascii_lowercase();
        if l.ends_with(".trn") {
            out.push((name.clone(), terrain.clone()));
        } else if l.ends_with(".scn") {
            out.push((name.clone(), scene(c, src, field, added, redrawn, start, data).into_bytes()));
        } else if l.ends_with(".tdf") {
            if *name == src.tdf {
                let data = if redrawn { (tdf.take().unwrap())() } else { data.clone() };
                out.push((new_tdf.clone(), data));
            }
        } else if l.ends_with(".wpt") {
            if Some(name) == src.wpt.as_ref() && !redrawn {
                out.push((new_wpt.clone(), data.clone()));
            }
        } else if l.ends_with(".course") {
            // The builder's own, written fresh below.
        } else {
            out.push((name.clone(), data.clone()));
        }
    }
    if !laps && (redrawn || src.wpt.is_none()) {
        out.push((new_wpt, (wpt.take().unwrap())().into_bytes()));
    }
    Ok(out)
}

/// The track's scene with the copy's name, its objects kept on the ground
/// as it moved, the added ones after them, and only the copied course.
fn scene(
    c: &Course,
    src: &Source,
    field: &Field,
    added: &[crate::env::Placed],
    redrawn: bool,
    start: ([f32; 2], [f32; 2]),
    data: &[u8],
) -> String {
    let text = String::from_utf8_lossy(data);
    let mut out = String::new();
    let mut section = String::new();
    let mut models = 0usize;
    let mut counted = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            section = t.trim_matches(|c| c == '[' || c == ']').to_ascii_lowercase();
        }
        let (key, value) = t.split_once('=').map_or((String::new(), ""), |(k, v)| (k.trim().to_ascii_lowercase(), v.trim()));
        let course_ix = c.base.as_ref().map_or(0, |b| b.course) + 1;
        // Zones 2 to 5 become the builder's paint (the game's tracks
        // keep their ground in zone 1).
        if ["terrainzone2", "terrainzone3", "terrainzone4", "terrainzone5"].contains(&section.as_str()) {
            continue;
        }
        match (section.as_str(), key.as_str()) {
            ("sceneinfo", "scenename") => {
                out += &format!("SceneName=\"{}\"\r\n", c.name.replace('"', "'"));
                continue;
            }
            ("sceneinfo", "numberofwaypointraces") => {
                out += "NumberOfWaypointRaces=1\r\n";
                continue;
            }
            ("sceneinfo", k) if k.starts_with("waypointracename") => {
                if k == format!("waypointracename{course_ix}") {
                    out += &format!("WaypointRaceName1={value}\r\n");
                }
                continue;
            }
            ("sceneinfo", "defaultstartposition") if redrawn => {
                let (p, _) = start;
                out += &format!("DefaultStartPosition={:.1},{:.1},{:.1}\r\n", p[0], field.height(p[0], p[1]) + 4.0, p[1]);
                continue;
            }
            ("sceneinfo", "defaultstartdirection") if redrawn => {
                let (_, d) = start;
                out += &format!("DefaultStartDirection={:.3},0.0,{:.3}\r\n", d[0], d[1]);
                continue;
            }
            ("staticmodels", "numberofstaticmodels") => {
                models = value.parse().unwrap_or(0);
                counted = true;
                out += &format!("NumberOfStaticModels={}\r\n", models + added.len());
                continue;
            }
            (s, "position") if s.starts_with("model") => {
                // Moved up or down with the ground under it.
                let v: Vec<f32> = value.split(',').filter_map(|v| v.trim().parse().ok()).collect();
                if let [x, y, z] = v[..] {
                    let dy = field.height(x, z) - src.height(x, z);
                    out += &format!("Position={x:.2},{:.2},{z:.2}\r\n", y + dy);
                    continue;
                }
            }
            _ => {}
        }
        out += line;
        out += "\r\n";
    }
    for (k, surface) in crate::Surface::ALL.iter().enumerate().skip(1) {
        out += &crate::env::zone_section(k, *surface);
    }
    if !counted && !added.is_empty() {
        out += &format!("\r\n[StaticModels]\r\nNumberOfStaticModels={}\r\n", added.len());
    }
    for (k, m) in added.iter().enumerate() {
        out += &crate::env::model_section(models + k + 1, m);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A copy of one of the game's tracks, when the game's files are here.
    fn game_track(rel: &str) -> Option<(Vec<u8>, String)> {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../GAME/TERAFORM").join(rel);
        Some((std::fs::read(&p).ok()?, p.to_string_lossy().to_string()))
    }

    #[test]
    fn a_copy_keeps_the_ground_and_takes_a_jump() {
        let Some((bytes, file)) = game_track("NATIONAL/NAT01.ENV") else {
            eprintln!("no game files: skipped");
            return;
        };
        let mut c = import(&bytes, &file, None, 0, "Test copy").unwrap();
        assert_eq!(c.mode, Mode::National);
        assert!(c.line.points.len() > 30);
        let src = c.source.clone().unwrap();
        let at = c.path().length / 2.0;
        let j = crate::kit::Kind::Tabletop.make(c.mode, at);
        c.jumps.push(j);
        let baked = crate::env::write(&c);
        let ar = rs2::Archive::parse(&baked.env).unwrap();
        let t = trn::parse(ar.get("Nat01.trn").unwrap()).unwrap();
        let path = c.path();
        let mid = path.at(at + j.length / 2.0);
        let h = |x: f32, z: f32| {
            let (ix, iz) = ((x / t.scale).round() as usize, (z / t.scale).round() as usize);
            t.heights[iz * t.size + ix] * t.scale
        };
        let rise = h(mid[0], mid[1]) - src.height(mid[0], mid[1]);
        assert!((rise - j.height).abs() < 1.5, "rose {rise}, jump {}", j.height);
        // Away from the track, the ground is as it was.
        let away = h(200.0, 200.0) - src.height(200.0, 200.0);
        assert!(away.abs() < 0.05, "{away}");
        // The line was kept, so the course file is the track's own.
        let tdf_new = ar.get("Nat01.tdf").unwrap();
        assert_eq!(tdf_new, src.files.iter().find(|(n, _)| n == "Nat01.tdf").unwrap().1.as_slice());
        let scn = String::from_utf8_lossy(ar.get("Nat01.scn").unwrap()).to_string();
        assert!(scn.contains("SceneName=\"Test copy\""));
    }
}
