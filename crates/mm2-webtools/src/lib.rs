//! The file readers behind the site's Tools page (`site/tools`), built for
//! the browser with `tools/build-web-tools.sh`.
//!
//! The page hands these functions the bytes of a file the player picked
//! (from their own game folder, CD image or the Free Pack) and gets back
//! plain data: archive listings, pictures as RGBA, models and motions as
//! JSON in the page's own right-handed coordinates (the game's `z`
//! flipped, as the engine draws it), terrain heights and a colour map, and
//! sounds as plain 16-bit WAV. Nothing leaves the player's computer.

use glam::{Mat4, Vec3};
use mm2_formats::{anim, cub, esb, faoe, rs2, scn, slb, slt, tdf, tga, trn, wav, Container};
use serde_json::{json, Value};
use wasm_bindgen::prelude::*;

fn err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

/// What kind of container `bytes` is: "RS2", "FAOE" or "unknown".
#[wasm_bindgen]
pub fn container(bytes: &[u8]) -> String {
    Container::detect(bytes).to_string()
}

/// An opened `.RES`, `.ENV`, `.DAT` or `.TEX` archive.
#[wasm_bindgen]
pub struct Archive {
    bytes: Vec<u8>,
    entries: Vec<(String, usize, usize)>,
}

#[wasm_bindgen]
impl Archive {
    /// Opens an archive; `name` is its file name, which keys the cipher
    /// of a FAOE one.
    #[wasm_bindgen(constructor)]
    pub fn new(name: &str, bytes: Vec<u8>) -> Result<Archive, JsError> {
        let bytes = match Container::detect(&bytes) {
            Container::Rs2 => bytes,
            Container::Faoe => faoe::decode(&bytes, name).map_err(err)?,
            Container::Unknown => return Err(JsError::new("not an archive")),
        };
        let entries = {
            let ar = rs2::Archive::parse(&bytes).map_err(err)?;
            let base = bytes.as_ptr() as usize;
            ar.entries()
                .iter()
                .map(|e| {
                    let data = ar.data(e);
                    (e.name.clone(), data.as_ptr() as usize - base, data.len())
                })
                .collect()
        };
        Ok(Archive { bytes, entries })
    }

    /// The entries' names, in the archive's order.
    pub fn names(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.0.clone()).collect()
    }

    pub fn sizes(&self) -> Vec<u32> {
        self.entries.iter().map(|e| e.2 as u32).collect()
    }

    /// Entry `i`'s bytes.
    pub fn at(&self, i: usize) -> Vec<u8> {
        self.entries.get(i).map(|e| self.bytes[e.1..e.1 + e.2].to_vec()).unwrap_or_default()
    }

    /// The entry called `name` (any case, folders ignored), if there is one.
    pub fn find(&self, name: &str) -> Option<usize> {
        let bare = |n: &str| n.rsplit(['\\', '/']).next().unwrap_or(n).trim().to_ascii_lowercase();
        let want = bare(name);
        self.entries.iter().position(|e| bare(&e.0) == want)
    }
}

/// A picture as top-down RGBA.
#[wasm_bindgen]
pub struct Picture {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

#[wasm_bindgen]
impl Picture {
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.width
    }
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn rgba(&self) -> Vec<u8> {
        self.rgba.clone()
    }
}

/// A model texture (the game's own format under a `.tga` name, or a real
/// Targa), top row first, with the magenta key made clear.
#[wasm_bindgen]
pub fn texture(bytes: &[u8]) -> Result<Picture, JsError> {
    match mm2_formats::texture::parse(bytes) {
        Ok(t) => Ok(Picture { width: t.size, height: t.size, rgba: mm2_formats::hires::export_rgba(&t) }),
        Err(e) => targa(bytes).map_err(|_| err(e)),
    }
}

/// A real Targa file (the menus' art, the `BIKES` and `RIDERS` paints).
#[wasm_bindgen]
pub fn targa(bytes: &[u8]) -> Result<Picture, JsError> {
    let i = tga::parse(bytes).map_err(err)?;
    Ok(Picture { width: i.width, height: i.height, rgba: i.rgba })
}

/// A sky box's six faces side by side in one picture (+z, −x, −z, +x, up,
/// down), each `side` pixels square.
#[wasm_bindgen]
pub fn sky(bytes: &[u8]) -> Result<Picture, JsError> {
    let cube = cub::Cube::parse(bytes).map_err(err)?;
    let side = (cube.tile_size() * cub::TILES as u32).clamp(64, 1024);
    let s = side as usize;
    let mut rgba = vec![0u8; s * 6 * s * 4];
    for face in 0..cube.faces.len().min(6) {
        let f = cube.face_rgba(face, side);
        for y in 0..s {
            let d = (y * s * 6 + face * s) * 4;
            rgba[d..d + s * 4].copy_from_slice(&f[y * s * 4..(y + 1) * s * 4]);
        }
    }
    Ok(Picture { width: side * 6, height: side, rgba })
}

/// An uncompressed 24-bit Targa from top-down RGBA, laid out as the game's
/// own `BIKES\BIKE01.TGA` (rows bottom-up).
#[wasm_bindgen]
pub fn encode_tga(width: u16, height: u16, rgba: &[u8]) -> Vec<u8> {
    let mut out = vec![0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&height.to_le_bytes());
    out.extend_from_slice(&[24, 0]);
    let row = width as usize * 4;
    for y in (0..height as usize).rev() {
        for p in rgba[y * row..(y + 1) * row].chunks_exact(4) {
            out.extend_from_slice(&[p[2], p[1], p[0]]);
        }
    }
    out
}

/// A game sound as a plain 16-bit PCM WAV that any program plays.
#[wasm_bindgen]
pub fn sound(bytes: &[u8]) -> Result<Vec<u8>, JsError> {
    let s = wav::parse(bytes).map_err(err)?;
    let data: Vec<u8> = s
        .samples
        .iter()
        .flat_map(|v| ((v.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes())
        .collect();
    let block = s.channels as u32 * 2;
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&s.channels.to_le_bytes());
    out.extend_from_slice(&s.rate.to_le_bytes());
    out.extend_from_slice(&(s.rate * block).to_le_bytes());
    out.extend_from_slice(&(block as u16).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&data);
    Ok(out)
}

/// The game's `z` points the other way from the page's.
const FLIP: Mat4 = Mat4::from_cols(
    glam::Vec4::X,
    glam::Vec4::Y,
    glam::Vec4::NEG_Z,
    glam::Vec4::W,
);

fn flip_point(p: [f32; 3]) -> [f32; 3] {
    [p[0], p[1], -p[2]]
}

/// The game's orientation from look and up vectors: +z along look, +y up.
fn orientation(look: [f32; 3], up: [f32; 3], scale: Vec3, position: [f32; 3]) -> Mat4 {
    let look = Vec3::from(look).normalize_or(Vec3::Z);
    let up = Vec3::from(up).normalize_or(Vec3::Y);
    let right = up.cross(look).normalize_or(Vec3::X);
    let up = look.cross(right);
    Mat4::from_cols(
        (right * scale.x).extend(0.0),
        (up * scale.y).extend(0.0),
        (look * scale.z).extend(0.0),
        Vec3::from(position).extend(1.0),
    )
}

/// A game-space matrix as the page's: position, rotation and scale.
fn trs(m: Mat4) -> Value {
    let (s, q, t) = (FLIP * m * FLIP).to_scale_rotation_translation();
    json!({ "t": [t.x, t.y, t.z], "q": [q.x, q.y, q.z, q.w], "s": [s.x, s.y, s.z] })
}

fn round(v: f32) -> f32 {
    (v * 1e5).round() / 1e5
}

/// A model (`.slb`, or the `.slt` text form when `name` ends so) as JSON:
/// its frames as a tree of nodes with their rest transforms, the texture
/// of each material, and its most detailed LOD's triangles, split by the
/// frame they hang from and their material, in that frame's own space.
#[wasm_bindgen]
pub fn model(bytes: &[u8], name: &str) -> Result<String, JsError> {
    let m = if name.to_ascii_lowercase().ends_with(".slt") {
        slt::parse(bytes).map_err(err)?
    } else {
        slb::parse(bytes).map_err(err)?
    };
    Ok(model_json(&m).to_string())
}

fn model_json(m: &slb::Model) -> Value {
    let world: Vec<Mat4> = m.world_matrices().iter().map(Mat4::from_cols_array).collect();
    let inverse: Vec<Mat4> = world.iter().map(|w| w.inverse()).collect();
    let frames: Vec<Value> = m
        .frames
        .iter()
        .map(|f| {
            let mut v = trs(Mat4::from_cols_array(&f.matrix));
            v["name"] = json!(f.name);
            v["parent"] = json!(f.parent.filter(|&p| p < m.frames.len()));
            v
        })
        .collect();
    let materials: Vec<Value> = m.materials.iter().map(|mat| json!(mat.texture)).collect();
    let mut meshes: Vec<Value> = Vec::new();
    let mut bounds = ([f32::MAX; 3], [f32::MIN; 3]);
    if let Some(lod) = m.lods.first() {
        for s in &lod.surfaces {
            let material = s.materials.first().copied().unwrap_or(u32::MAX);
            // Each face goes with the frame of its first corner.
            let mut by_frame: std::collections::BTreeMap<usize, Vec<[u16; 3]>> = Default::default();
            for f in &s.faces {
                by_frame.entry(s.frame_of(f[0] as usize)).or_default().push(*f);
            }
            for (frame, faces) in by_frame {
                let inv = inverse.get(frame).copied().unwrap_or(Mat4::IDENTITY);
                let mut remap = vec![u32::MAX; s.positions.len()];
                let (mut p, mut n, mut uv, mut c, mut idx) = (vec![], vec![], vec![], vec![], vec![]);
                for f in &faces {
                    // Flipping z turns the winding round too.
                    for &v in &[f[0], f[2], f[1]] {
                        let v = v as usize;
                        if remap[v] == u32::MAX {
                            remap[v] = (p.len() / 3) as u32;
                            let wp = Vec3::from(s.positions[v]);
                            for k in 0..3 {
                                bounds.0[k] = bounds.0[k].min(flip_point(s.positions[v])[k]);
                                bounds.1[k] = bounds.1[k].max(flip_point(s.positions[v])[k]);
                            }
                            let lp = flip_point(inv.transform_point3(wp).into());
                            let ln = flip_point(
                                inv.transform_vector3(Vec3::from(s.normals.get(v).copied().unwrap_or([0.0, 1.0, 0.0])))
                                    .normalize_or(Vec3::Y)
                                    .into(),
                            );
                            p.extend(lp.map(round));
                            n.extend(ln);
                            let t = s.uvs.get(v).copied().unwrap_or([0.0, 0.0]);
                            uv.extend([round(t[0]), round(t[1])]);
                            let col = s.colors[v];
                            c.extend([col[2], col[1], col[0], col[3]]);
                        }
                        idx.push(remap[v]);
                    }
                }
                meshes.push(json!({
                    "frame": frame, "material": material,
                    "p": p, "n": n, "uv": uv, "c": c, "i": idx,
                }));
            }
        }
    }
    json!({ "frames": frames, "materials": materials, "meshes": meshes, "min": bounds.0, "max": bounds.1 })
}

/// An `.mcf` (an animated model and its motions) as JSON.
#[wasm_bindgen]
pub fn mcf(bytes: &[u8]) -> Result<String, JsError> {
    let m = anim::Mcf::parse(bytes).map_err(err)?;
    let motions: Vec<Value> = m
        .motions
        .iter()
        .map(|x| json!({ "name": x.name, "file": x.file, "fps": x.fps, "frames": x.frame_count, "looping": x.looping }))
        .collect();
    Ok(json!({ "model": m.model, "motions": motions }).to_string())
}

/// A `.mot` motion as JSON: for each model frame it moves, its position
/// and turn on every animation frame, in the page's coordinates.
#[wasm_bindgen]
pub fn motion(bytes: &[u8]) -> Result<String, JsError> {
    let m = anim::Motion::parse(bytes).map_err(err)?;
    Ok(motion_json(&m).to_string())
}

fn motion_json(m: &anim::Motion) -> Value {
    let mut tracks: std::collections::BTreeMap<usize, (Vec<f32>, Vec<f32>)> = Default::default();
    for keys in &m.frames {
        for k in keys {
            let local = orientation(k.look, k.up, Vec3::ONE, k.position);
            let (_, q, t) = (FLIP * local * FLIP).to_scale_rotation_translation();
            let e = tracks.entry(k.frame).or_default();
            e.0.extend([t.x, t.y, t.z].map(round));
            e.1.extend([q.x, q.y, q.z, q.w].map(round));
        }
    }
    let tracks: Vec<Value> = tracks.into_iter().map(|(f, (t, q))| json!({ "frame": f, "t": t, "q": q })).collect();
    json!({ "count": m.frames.len(), "tracks": tracks })
}

/// The Free Pack rider's and bike's own motions (`free_motions.bin`, made
/// by `tools/freepack/free_motions.py`) as JSON, each as [`motion`] gives
/// one, with its name, frames a second, looping and whether it's the
/// rider's. Key frames count in that tool's `ORDER` and `BIKE` lists.
#[wasm_bindgen]
pub fn free_motions(data: &[u8]) -> Result<String, JsError> {
    let bad = || JsError::new("not a free motions file");
    if data.get(..4) != Some(b"FPMO") {
        return Err(bad());
    }
    let mut at = 4usize;
    let mut take = |n: usize| -> Result<&[u8], JsError> {
        let b = data.get(at..at + n).ok_or_else(bad)?;
        at += n;
        Ok(b)
    };
    let count = u16::from_le_bytes([take(1)?[0], take(1)?[0]]);
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let len = take(1)?[0] as usize;
        let name = String::from_utf8_lossy(take(len)?).into_owned();
        let f = take(4)?;
        let fps = f32::from_le_bytes([f[0], f[1], f[2], f[3]]);
        let (looping, rider) = (take(1)?[0] != 0, take(1)?[0] != 0);
        let n = take(2)?;
        let frames = u16::from_le_bytes([n[0], n[1]]) as usize;
        let keys = take(1)?[0] as usize;
        let mut motion = anim::Motion { frames: Vec::with_capacity(frames) };
        for _ in 0..frames {
            let mut list = Vec::with_capacity(keys);
            for _ in 0..keys {
                let b = take(19)?;
                let q = |i: usize| i16::from_le_bytes([b[1 + 2 * i], b[2 + 2 * i]]) as f32;
                let axis = |i: usize| [q(i) / 30000.0, q(i + 1) / 30000.0, q(i + 2) / 30000.0];
                list.push(anim::Key {
                    frame: b[0] as usize,
                    look: axis(0),
                    up: axis(3),
                    position: [q(6) / 1000.0, q(7) / 1000.0, q(8) / 1000.0],
                });
            }
            motion.frames.push(list);
        }
        let mut v = motion_json(&motion);
        v["name"] = json!(name);
        v["fps"] = json!(fps);
        v["looping"] = json!(looping);
        v["rider"] = json!(rider);
        out.push(v);
    }
    Ok(Value::Array(out).to_string())
}

fn placement(model: &str, position: [f32; 3], look: [f32; 3], up: [f32; 3], scale: [f32; 3]) -> Value {
    let mut v = trs(orientation(look, up, Vec3::from(scale), position));
    v["model"] = json!(model);
    v
}

/// A track's `.scn` as JSON: its models and animated models with their
/// places, start, sky, fog and lights.
#[wasm_bindgen]
pub fn scene(bytes: &[u8]) -> Result<String, JsError> {
    let s = scn::Scene::parse(bytes).map_err(err)?;
    let models: Vec<Value> =
        s.models.iter().map(|p| placement(&p.model, p.position, p.look, p.up, p.scale)).collect();
    let animations: Vec<Value> = s
        .animations
        .iter()
        .map(|a| {
            let mut v = placement(&a.model, a.offset, a.look, a.up, a.scale);
            v["motion"] = json!(a.motion);
            v
        })
        .collect();
    let procedural: Vec<Value> = s
        .procedural
        .iter()
        .map(|p| placement(&p.model, p.offset, [0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [1.0; 3]))
        .collect();
    let sun = s
        .lights
        .iter()
        .find_map(|l| l.direction.filter(|d| d[1] < -1e-3).map(|d| (d, l.color)))
        .map(|(d, c)| json!({ "dir": flip_point(d), "color": c }));
    Ok(json!({
        "name": s.name,
        "terrain": s.terrain_file,
        "start": flip_point(s.start_position),
        "startDir": flip_point(s.start_direction),
        "sky": s.cube_file,
        "fog": s.fog_color,
        "sun": sun,
        "stadium": s.stadium,
        "resources": s.resource_files,
        "models": models,
        "animations": animations,
        "procedural": procedural,
    })
    .to_string())
}

/// A track's trees and bushes (`.esb`) as JSON: each species' model and
/// sizes, and every plant's place, size and species, in the page's
/// coordinates. `span` is the terrain's side in world units.
#[wasm_bindgen]
pub fn eco(bytes: &[u8], span: f32) -> Result<String, JsError> {
    let e = esb::parse(bytes).map_err(err)?;
    let unit = span / 65536.0;
    let species: Vec<Value> = e.species.iter().map(|s| json!({ "slot": s.slot, "name": s.name })).collect();
    let mut plants = Vec::with_capacity(e.objects.len() * 6);
    for o in &e.objects {
        let Some(s) = e.species(o.species) else { continue };
        let p = o.position.map(|p| p as f32 * unit);
        plants.extend([
            o.species as f32,
            round(p[0]),
            round(p[1]),
            round(-p[2]),
            round(s.height_of(o.height)),
            round(s.width_of(o.width)),
        ]);
    }
    Ok(json!({ "species": species, "plants": plants }).to_string())
}

/// A track's racing line (`.tdf`) as JSON points in the page's coordinates.
#[wasm_bindgen]
pub fn track_line(bytes: &[u8]) -> Result<String, JsError> {
    let t = tdf::parse(bytes).map_err(err)?;
    let nodes: Vec<Vec<[f32; 3]>> =
        t.nodes.iter().map(|n| n.sections.iter().map(|s| flip_point(s.center)).collect()).collect();
    Ok(json!({ "nodes": nodes }).to_string())
}

/// A track's ground, ready to draw.
#[wasm_bindgen]
pub struct Ground {
    /// Samples a side of `heights`.
    side: u32,
    /// World units between two samples.
    spacing: f32,
    heights: Vec<f32>,
    map_side: u32,
    map: Vec<u8>,
    surfaces: Vec<u8>,
}

#[wasm_bindgen]
impl Ground {
    #[wasm_bindgen(getter)]
    pub fn side(&self) -> u32 {
        self.side
    }
    #[wasm_bindgen(getter)]
    pub fn spacing(&self) -> f32 {
        self.spacing
    }
    /// Heights in world units, rows along the game's +z (the page's −z).
    pub fn heights(&self) -> Vec<f32> {
        self.heights.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn map_side(&self) -> u32 {
        self.map_side
    }
    /// The ground's colour from above, RGBA, rows along the game's +z.
    pub fn map(&self) -> Vec<u8> {
        self.map.clone()
    }
    /// Each sample's ground type (the scene's terrain zone).
    pub fn surfaces(&self) -> Vec<u8> {
        self.surfaces.clone()
    }
}

/// A track's `.trn` terrain: every `step`th height sample, and its
/// textures drawn into one map of at most `map_max` pixels a side.
#[wasm_bindgen]
pub fn terrain(bytes: &[u8], step: u32, map_max: u32) -> Result<Ground, JsError> {
    let t = trn::parse(bytes).map_err(err)?;
    let n = t.size;
    let step = step.max(1) as usize;
    let side = (n - 1) / step + 1;
    let mut heights = Vec::with_capacity(side * side);
    let mut surfaces = Vec::with_capacity(side * side);
    for z in 0..side {
        for x in 0..side {
            let (sx, sz) = ((x * step).min(n - 1), (z * step).min(n - 1));
            heights.push(t.height(sx, sz) * t.scale);
            surfaces.push(t.surfaces.get(sz * n + sx).copied().unwrap_or(0));
        }
    }
    // Texels a terrain unit: as many as fit the map, a power of two.
    let units = (n - 1).max(1);
    let mut per = 1usize;
    while units * per * 2 <= map_max as usize && per < 16 {
        per *= 2;
    }
    let map_side = units * per;
    let mut map = vec![0u8; map_side * map_side * 4];
    for tile in &t.tiles {
        let Some(tex) = t.textures.get(tile.texture as usize) else { continue };
        let want = tile.size as usize * per;
        // The mip level nearest the size it's drawn at.
        let mut level = 0;
        while level + 1 < tex.levels.len() && (tex.size as usize >> (level + 1)) >= want {
            level += 1;
        }
        let src_side = (tex.size as usize >> level).max(1);
        let rgba = tex.rgba8(level);
        for v in 0..want {
            for u in 0..want {
                let (x, z) = (tile.x as usize * per + u, tile.z as usize * per + v);
                if x >= map_side || z >= map_side {
                    continue;
                }
                let s = ((v * src_side / want) * src_side + u * src_side / want) * 4;
                let d = (z * map_side + x) * 4;
                map[d..d + 3].copy_from_slice(&rgba[s..s + 3]);
                map[d + 3] = 255;
            }
        }
    }
    Ok(Ground {
        side: side as u32,
        spacing: step as f32 * t.scale,
        heights,
        map_side: map_side as u32,
        map,
        surfaces,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targa_round_trip() {
        let rgba = [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 9, 9, 9, 255];
        let tga = encode_tga(2, 2, &rgba);
        let back = tga::parse(&tga).unwrap();
        assert_eq!((back.width, back.height), (2, 2));
        assert_eq!(back.rgba, rgba);
    }

    #[test]
    fn flips_only_z() {
        let v = trs(Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0)));
        assert_eq!(v["t"], json!([1.0, 2.0, -3.0]));
    }
}
