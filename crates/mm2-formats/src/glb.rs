//! Binary glTF (`.glb`) models, the free pack's format, read into the same
//! [`slb::Model`] the game's own models load into.
//!
//! Only what the free pack's stand-ins use is read: triangle meshes with
//! positions, normals and one set of texture coordinates, placed by the
//! node tree, and each material's base colour (a factor, and a PNG or JPEG
//! texture stored in the file, or in a file beside it that several models
//! share) with the rest of its metallic-roughness look (normal, metal and
//! roughness, occlusion and emissive pictures and factors). Everything is
//! baked into one frame.
//!
//! glTF is right-handed with +Y up; the game's axes are (x, y, -z), so z is
//! mirrored and the windings turned to match. Units stay as authored (the
//! free pack works in feet, like the game).

use crate::slb::{Frame, Lod, Material, Model, Pbr, Surface};
use crate::{Error, Result};
use serde_json::Value;

/// One texture stored in the file, as its encoded bytes.
#[derive(Debug, Clone)]
pub struct Image {
    /// The name the model's materials use for it.
    pub name: String,
    /// `image/png` or `image/jpeg`.
    pub mime: String,
    /// The encoded picture; empty for one kept in a file of its own.
    pub data: Vec<u8>,
    /// A picture kept in a file of its own: its path relative to the
    /// model's folder, for the caller to read. Shared pictures are named
    /// `fp:file:<file name>`, so models that share one share its texture.
    pub uri: Option<String>,
}

/// Reads a `.glb`. Materials name their textures `<prefix><n>`, the
/// returned images' names.
pub fn parse(data: &[u8], prefix: &str) -> Result<(Model, Vec<Image>)> {
    read(data, prefix, None)
}

/// Reads a `.glb` whose nodes are a model's moving parts, as the frames
/// `names` in that order (a bike's or rider's frame tree, so the game's
/// motions can drive it). Each named node becomes its frame, placed
/// against the nearest named node above it; parts under a node without
/// one of the names move with that nearest named node. A name no node
/// has is a still frame on the first.
pub fn parse_framed(data: &[u8], prefix: &str, names: &[&str]) -> Result<(Model, Vec<Image>)> {
    read(data, prefix, Some(names))
}

/// Each material's name, in order (the model's [`Material`]s keep only
/// their textures).
pub fn material_names(data: &[u8]) -> Result<Vec<String>> {
    let (gltf, _) = chunks(data)?;
    Ok(arr(&gltf["materials"]).iter().map(|m| m["name"].as_str().unwrap_or_default().to_string()).collect())
}

/// The JSON and the binary chunk.
fn chunks(data: &[u8]) -> Result<(Value, &[u8])> {
    if data.len() < 20 || &data[0..4] != b"glTF" {
        return Err(Error::BadMagic);
    }
    let mut json: Option<Value> = None;
    let mut bin: &[u8] = &[];
    let mut at = 12;
    while at + 8 <= data.len() {
        let len = u32::from_le_bytes(data[at..at + 4].try_into().unwrap()) as usize;
        let kind = &data[at + 4..at + 8];
        let body = data.get(at + 8..at + 8 + len).ok_or(Error::Truncated { what: "glb chunk", offset: at })?;
        match kind {
            b"JSON" => json = Some(serde_json::from_slice(body).map_err(|_| Error::Corrupt("glb json"))?),
            b"BIN\0" => bin = body,
            _ => {}
        }
        at += 8 + len.next_multiple_of(4);
    }
    Ok((json.ok_or(Error::Corrupt("glb has no json"))?, bin))
}

fn read(data: &[u8], prefix: &str, names: Option<&[&str]>) -> Result<(Model, Vec<Image>)> {
    let (gltf, bin) = chunks(data)?;
    let doc = Doc { gltf: &gltf, bin };

    let mut images: Vec<Image> = Vec::new();
    // Each glTF image's texture name, if it has a picture.
    let mut named: Vec<Option<String>> = Vec::new();
    for (i, img) in arr(&gltf["images"]).iter().enumerate() {
        let mut name = None;
        if let Some(view) = img["bufferView"].as_u64() {
            if let Some(bytes) = doc.view(view as usize) {
                name = Some(format!("{prefix}{i}"));
                images.push(Image {
                    name: format!("{prefix}{i}"),
                    mime: img["mimeType"].as_str().unwrap_or("image/png").to_string(),
                    data: bytes.to_vec(),
                    uri: None,
                });
            }
        } else if let Some(uri) = img["uri"].as_str().filter(|u| !u.is_empty() && !u.starts_with("data:")) {
            let file = uri.rsplit(['/', '\\']).next().unwrap_or(uri).to_ascii_lowercase();
            let jpeg = file.ends_with(".jpg") || file.ends_with(".jpeg");
            let shared = format!("fp:file:{file}");
            if !images.iter().any(|im| im.name == shared) {
                images.push(Image {
                    name: shared.clone(),
                    mime: img["mimeType"].as_str().unwrap_or(if jpeg { "image/jpeg" } else { "image/png" }).to_string(),
                    data: Vec::new(),
                    uri: Some(uri.to_string()),
                });
            }
            name = Some(shared);
        }
        named.push(name);
    }
    // A texture reference's picture name, if it has one.
    let picture = |info: &Value| {
        info["index"]
            .as_u64()
            .and_then(|t| gltf["textures"][t as usize]["source"].as_u64())
            .and_then(|i| named.get(i as usize).cloned().flatten())
    };
    let number = |v: &Value, default: f32| v.as_f64().map_or(default, |v| v as f32);
    // Each glTF material: its texture's name, its colour (BGRA) and the
    // rest of its physically based look.
    let materials: Vec<(String, [u8; 4], Pbr)> = arr(&gltf["materials"])
        .iter()
        .map(|m| {
            let pbr = &m["pbrMetallicRoughness"];
            let image = picture(&pbr["baseColorTexture"]);
            let f: Vec<f32> = arr(&pbr["baseColorFactor"]).iter().filter_map(|v| v.as_f64()).map(|v| v as f32).collect();
            let c = |k: usize| (srgb(f.get(k).copied().unwrap_or(1.0)) * 255.0).round().clamp(0.0, 255.0) as u8;
            let e: Vec<f32> = arr(&m["emissiveFactor"]).iter().filter_map(|v| v.as_f64()).map(|v| v as f32).collect();
            let strength = number(&m["extensions"]["KHR_materials_emissive_strength"]["emissiveStrength"], 1.0);
            let look = Pbr {
                normal: picture(&m["normalTexture"]).unwrap_or_default(),
                metal_rough: picture(&pbr["metallicRoughnessTexture"]).unwrap_or_default(),
                occlusion: picture(&m["occlusionTexture"]).unwrap_or_default(),
                emissive: picture(&m["emissiveTexture"]).unwrap_or_default(),
                metallic: number(&pbr["metallicFactor"], 1.0).clamp(0.0, 1.0),
                roughness: number(&pbr["roughnessFactor"], 1.0).clamp(0.0, 1.0),
                normal_scale: number(&m["normalTexture"]["scale"], 1.0),
                occlusion_strength: number(&m["occlusionTexture"]["strength"], 1.0).clamp(0.0, 1.0),
                emissive_factor: [0, 1, 2].map(|k| e.get(k).copied().unwrap_or(0.0) * strength),
                specular: number(&m["extensions"]["KHR_materials_specular"]["specularFactor"], 1.0).clamp(0.0, 1.0),
            };
            (image.unwrap_or_default(), [c(2), c(1), c(0), 255], look)
        })
        .collect();

    let frames = match names {
        Some(names) if !names.is_empty() => names
            .iter()
            .enumerate()
            .map(|(i, n)| Frame { name: n.to_string(), matrix: IDENTITY, parent: (i > 0).then_some(0) })
            .collect(),
        _ => vec![Frame { name: "root".into(), matrix: IDENTITY, parent: None }],
    };
    let mut model = Model {
        frames,
        materials: materials.iter().map(|(t, _, look)| Material { texture: t.clone(), pbr: Some(look.clone()) }).collect(),
        lods: vec![Lod { surfaces: Vec::new() }],
    };
    if model.materials.is_empty() {
        model.materials.push(Material { texture: String::new(), pbr: None });
    }
    let scene = gltf["scene"].as_u64().unwrap_or(0) as usize;
    let roots: Vec<usize> = match gltf["scenes"][scene]["nodes"].as_array() {
        Some(n) => n.iter().filter_map(|v| v.as_u64()).map(|v| v as usize).collect(),
        None => (0..arr(&gltf["nodes"]).len()).collect(),
    };
    // Each named frame's place in the model, in game axes.
    let mut placed: Vec<Option<[f32; 16]>> = vec![None; model.frames.len()];
    // (node, its world transform, depth, the frame it moves with)
    let mut stack: Vec<(usize, [f32; 16], usize, usize)> = roots.into_iter().map(|n| (n, IDENTITY, 0, 0)).collect();
    while let Some((n, parent, depth, above)) = stack.pop() {
        let node = &gltf["nodes"][n];
        if depth > 64 || node.is_null() {
            continue;
        }
        let world = mul(&parent, &local(node));
        let mut frame = above;
        if let Some(names) = names {
            let name = node["name"].as_str().unwrap_or_default();
            if let Some(k) = names.iter().position(|w| w.eq_ignore_ascii_case(name)).filter(|&k| placed[k].is_none()) {
                // glTF to game axes: mirror z on both sides.
                let game = mul(&mul(&MIRROR_Z, &world), &MIRROR_Z);
                placed[k] = Some(game);
                model.frames[k].parent = (k != 0).then_some(above);
                frame = k;
            }
        }
        if let Some(mesh) = node["mesh"].as_u64() {
            for prim in arr(&gltf["meshes"][mesh as usize]["primitives"]) {
                if let Some(s) = doc.primitive(prim, &world, &materials)? {
                    model.lods[0].surfaces.extend(split(s).into_iter().map(|mut s| {
                        s.groups = vec![(frame as u32, 0)];
                        s
                    }));
                }
            }
        }
        for c in arr(&node["children"]).iter().filter_map(|v| v.as_u64()) {
            stack.push((c as usize, world, depth + 1, frame));
        }
    }
    if names.is_some() {
        // Each frame against its parent: the parent's inverse times its own.
        for k in 0..model.frames.len() {
            let own = placed[k].unwrap_or_else(|| model.frames[k].parent.and_then(|p| placed[p]).unwrap_or(IDENTITY));
            let up = model.frames[k].parent.and_then(|p| placed[p]).unwrap_or(IDENTITY);
            model.frames[k].matrix = mul(&inverse(&up), &own);
        }
    }
    Ok((model, images))
}

fn arr(v: &Value) -> &[Value] {
    v.as_array().map_or(&[], |a| a.as_slice())
}

fn srgb(v: f32) -> f32 {
    // Factors are linear; vertex colours are display colours.
    v.clamp(0.0, 1.0).powf(1.0 / 2.2)
}

/// Column-major identity.
const IDENTITY: [f32; 16] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

/// Mirrors z: glTF's axes to the game's and back.
const MIRROR_Z: [f32; 16] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

/// The inverse of an affine transform, column-major.
fn inverse(m: &[f32; 16]) -> [f32; 16] {
    let a = |c: usize, r: usize| m[c * 4 + r];
    let det = a(0, 0) * (a(1, 1) * a(2, 2) - a(2, 1) * a(1, 2)) - a(1, 0) * (a(0, 1) * a(2, 2) - a(2, 1) * a(0, 2))
        + a(2, 0) * (a(0, 1) * a(1, 2) - a(1, 1) * a(0, 2));
    if det.abs() < 1e-12 {
        return IDENTITY;
    }
    let mut o = IDENTITY;
    // Inverse 3x3 (r, c) = cofactor (c, r) / det.
    for r in 0..3 {
        for c in 0..3 {
            let (r1, r2) = ((c + 1) % 3, (c + 2) % 3);
            let (c1, c2) = ((r + 1) % 3, (r + 2) % 3);
            o[c * 4 + r] = (a(c1, r1) * a(c2, r2) - a(c2, r1) * a(c1, r2)) / det;
        }
    }
    for r in 0..3 {
        o[12 + r] = -(0..3).map(|k| o[k * 4 + r] * m[12 + k]).sum::<f32>();
    }
    o
}

/// Column-major `a * b`.
fn mul(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut m = [0.0; 16];
    for c in 0..4 {
        for r in 0..4 {
            m[c * 4 + r] = (0..4).map(|k| a[k * 4 + r] * b[c * 4 + k]).sum();
        }
    }
    m
}

/// A node's transform, column-major.
fn local(node: &Value) -> [f32; 16] {
    let nums = |v: &Value| -> Vec<f32> { arr(v).iter().filter_map(|x| x.as_f64()).map(|x| x as f32).collect() };
    let m = nums(&node["matrix"]);
    if m.len() == 16 {
        return m.try_into().unwrap();
    }
    let t = nums(&node["translation"]);
    let r = nums(&node["rotation"]);
    let s = nums(&node["scale"]);
    let t = if t.len() == 3 { [t[0], t[1], t[2]] } else { [0.0; 3] };
    let [x, y, z, w] = if r.len() == 4 { [r[0], r[1], r[2], r[3]] } else { [0.0, 0.0, 0.0, 1.0] };
    let s = if s.len() == 3 { [s[0], s[1], s[2]] } else { [1.0; 3] };
    let rot = [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + z * w), 2.0 * (x * z - y * w)],
        [2.0 * (x * y - z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + x * w)],
        [2.0 * (x * z + y * w), 2.0 * (y * z - x * w), 1.0 - 2.0 * (x * x + y * y)],
    ];
    let mut m = IDENTITY;
    for c in 0..3 {
        for r in 0..3 {
            m[c * 4 + r] = rot[c][r] * s[c];
        }
    }
    m[12..15].copy_from_slice(&t);
    m
}

struct Doc<'a> {
    gltf: &'a Value,
    bin: &'a [u8],
}

impl Doc<'_> {
    fn view(&self, i: usize) -> Option<&[u8]> {
        let v = &self.gltf["bufferViews"][i];
        if v["buffer"].as_u64().unwrap_or(0) != 0 {
            return None;
        }
        let off = v["byteOffset"].as_u64().unwrap_or(0) as usize;
        let len = v["byteLength"].as_u64()? as usize;
        self.bin.get(off..off + len)
    }

    /// An accessor's values as f32s, `width` per element (normalised
    /// integers scaled to 0..1).
    fn floats(&self, i: usize, width: usize) -> Option<Vec<f32>> {
        let a = &self.gltf["accessors"][i];
        let count = a["count"].as_u64()? as usize;
        let kind = a["componentType"].as_u64()?;
        let normalized = a["normalized"].as_bool().unwrap_or(false);
        let size = match kind {
            5126 | 5125 => 4,
            5122 | 5123 => 2,
            _ => 1,
        };
        let view_index = a["bufferView"].as_u64()? as usize;
        let view = self.view(view_index)?;
        let stride = self.gltf["bufferViews"][view_index]["byteStride"].as_u64().map_or(size * width, |s| s as usize);
        let base = a["byteOffset"].as_u64().unwrap_or(0) as usize;
        let mut out = Vec::with_capacity(count * width);
        for e in 0..count {
            for k in 0..width {
                let at = base + e * stride + k * size;
                let b = view.get(at..at + size)?;
                out.push(match kind {
                    5126 => f32::from_le_bytes(b.try_into().ok()?),
                    5125 => u32::from_le_bytes(b.try_into().ok()?) as f32,
                    5123 => {
                        let v = u16::from_le_bytes(b.try_into().ok()?) as f32;
                        if normalized { v / 65535.0 } else { v }
                    }
                    5122 => {
                        let v = i16::from_le_bytes(b.try_into().ok()?) as f32;
                        if normalized { (v / 32767.0).max(-1.0) } else { v }
                    }
                    5121 => {
                        let v = b[0] as f32;
                        if normalized { v / 255.0 } else { v }
                    }
                    _ => {
                        let v = b[0] as i8 as f32;
                        if normalized { (v / 127.0).max(-1.0) } else { v }
                    }
                });
            }
        }
        Some(out)
    }

    fn indices(&self, i: usize) -> Option<Vec<u32>> {
        let a = &self.gltf["accessors"][i];
        let count = a["count"].as_u64()? as usize;
        let kind = a["componentType"].as_u64()?;
        let size = match kind {
            5125 => 4,
            5123 => 2,
            5121 => 1,
            _ => return None,
        };
        let view_index = a["bufferView"].as_u64()? as usize;
        let view = self.view(view_index)?;
        let base = a["byteOffset"].as_u64().unwrap_or(0) as usize;
        (0..count)
            .map(|e| {
                let at = base + e * size;
                let b = view.get(at..at + size)?;
                Some(match size {
                    4 => u32::from_le_bytes(b.try_into().ok()?),
                    2 => u16::from_le_bytes(b.try_into().ok()?) as u32,
                    _ => b[0] as u32,
                })
            })
            .collect()
    }

    /// One triangle primitive, placed by `world` and turned into game
    /// axes. Faces are u32 here; [`split`] makes them fit.
    fn primitive(&self, prim: &Value, world: &[f32; 16], materials: &[(String, [u8; 4], Pbr)]) -> Result<Option<Big>> {
        if prim["mode"].as_u64().unwrap_or(4) != 4 {
            return Ok(None);
        }
        let attr = &prim["attributes"];
        let Some(pos) = attr["POSITION"].as_u64().and_then(|a| self.floats(a as usize, 3)) else {
            return Err(Error::Corrupt("glb primitive without positions"));
        };
        let n = pos.len() / 3;
        let normals = attr["NORMAL"].as_u64().and_then(|a| self.floats(a as usize, 3)).filter(|v| v.len() == n * 3);
        let uvs = attr["TEXCOORD_0"].as_u64().and_then(|a| self.floats(a as usize, 2)).filter(|v| v.len() == n * 2);
        let index = match prim["indices"].as_u64() {
            Some(a) => self.indices(a as usize).ok_or(Error::Corrupt("glb indices"))?,
            None => (0..n as u32).collect(),
        };
        let material = prim["material"].as_u64().map_or(0, |m| m as u32);
        let color = materials.get(material as usize).map_or([255; 4], |m| m.1);
        let normal_matrix = normal_matrix(world);
        let positions: Vec<[f32; 3]> = pos
            .chunks_exact(3)
            .map(|p| {
                let q = transform(world, [p[0], p[1], p[2]], 1.0);
                [q[0], q[1], -q[2]]
            })
            .collect();
        let mut faces: Vec<[u32; 3]> = index
            .chunks_exact(3)
            .filter(|f| f.iter().all(|&i| (i as usize) < n))
            // Mirroring z turns the faces round; swap back.
            .map(|f| [f[0], f[2], f[1]])
            .collect();
        faces.retain(|f| f[0] != f[1] && f[1] != f[2] && f[0] != f[2]);
        let normals: Vec<[f32; 3]> = match normals {
            Some(nv) => nv
                .chunks_exact(3)
                .map(|v| {
                    let q = mat3(&normal_matrix, [v[0], v[1], v[2]]);
                    normalize([q[0], q[1], -q[2]])
                })
                .collect(),
            None => face_normals(&positions, &faces),
        };
        Ok(Some(Big {
            positions,
            normals,
            uvs: uvs.map_or(vec![[0.0; 2]; n], |u| u.chunks_exact(2).map(|u| [u[0], 1.0 - u[1]]).collect()),
            colors: vec![color; n],
            faces,
            material: if (material as usize) < materials.len().max(1) { material } else { 0 },
        }))
    }
}

/// A surface before it is split into pieces with u16 indices.
struct Big {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[u8; 4]>,
    faces: Vec<[u32; 3]>,
    material: u32,
}

/// Splits a surface into pieces of at most 65536 vertices.
fn split(big: Big) -> Vec<Surface> {
    let piece = |positions, normals, uvs, colors, faces| Surface {
        positions,
        normals,
        uvs,
        colors,
        faces,
        materials: vec![big.material],
        groups: vec![(0, 0)],
    };
    if big.positions.len() <= 65536 {
        let faces = big.faces.iter().map(|f| [f[0] as u16, f[1] as u16, f[2] as u16]).collect();
        return vec![piece(big.positions.clone(), big.normals.clone(), big.uvs.clone(), big.colors.clone(), faces)];
    }
    let mut out = Vec::new();
    let mut map = std::collections::HashMap::new();
    let (mut p, mut nm, mut uv, mut c, mut f) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for face in &big.faces {
        if p.len() + 3 > 65536 {
            out.push(piece(std::mem::take(&mut p), std::mem::take(&mut nm), std::mem::take(&mut uv), std::mem::take(&mut c), std::mem::take(&mut f)));
            map.clear();
        }
        let mut tri = [0u16; 3];
        for (k, &v) in face.iter().enumerate() {
            tri[k] = *map.entry(v).or_insert_with(|| {
                let v = v as usize;
                p.push(big.positions[v]);
                nm.push(big.normals[v]);
                uv.push(big.uvs[v]);
                c.push(big.colors[v]);
                (p.len() - 1) as u16
            });
        }
        f.push(tri);
    }
    if !f.is_empty() {
        out.push(piece(p, nm, uv, c, f));
    }
    out
}

fn transform(m: &[f32; 16], v: [f32; 3], w: f32) -> [f32; 3] {
    let mut o = [0.0; 3];
    for (r, out) in o.iter_mut().enumerate() {
        *out = m[r] * v[0] + m[4 + r] * v[1] + m[8 + r] * v[2] + m[12 + r] * w;
    }
    o
}

/// The inverse transpose of the upper 3x3, column-major.
fn normal_matrix(m: &[f32; 16]) -> [f32; 9] {
    let a = |c: usize, r: usize| m[c * 4 + r];
    // Rows of the 3x3 (r, c) = a(c, r).
    let (m00, m01, m02) = (a(0, 0), a(1, 0), a(2, 0));
    let (m10, m11, m12) = (a(0, 1), a(1, 1), a(2, 1));
    let (m20, m21, m22) = (a(0, 2), a(1, 2), a(2, 2));
    let cof = [
        m11 * m22 - m12 * m21,
        m12 * m20 - m10 * m22,
        m10 * m21 - m11 * m20,
        m02 * m21 - m01 * m22,
        m00 * m22 - m02 * m20,
        m01 * m20 - m00 * m21,
        m01 * m12 - m02 * m11,
        m02 * m10 - m00 * m12,
        m00 * m11 - m01 * m10,
    ];
    // The cofactor matrix is the inverse transpose up to a scale, which
    // the normalisation removes. Stored row-major: (r, c) = cof[r * 3 + c].
    let det = m00 * cof[0] + m01 * cof[1] + m02 * cof[2];
    let s = if det < 0.0 { -1.0 } else { 1.0 };
    cof.map(|v| v * s)
}

fn mat3(m: &[f32; 9], v: [f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|r| m[r * 3] * v[0] + m[r * 3 + 1] * v[1] + m[r * 3 + 2] * v[2])
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 0.0 { v.map(|x| x / l) } else { [0.0, 1.0, 0.0] }
}

fn face_normals(p: &[[f32; 3]], faces: &[[u32; 3]]) -> Vec<[f32; 3]> {
    let mut n = vec![[0.0f32; 3]; p.len()];
    for f in faces {
        let (a, b, c) = (p[f[0] as usize], p[f[1] as usize], p[f[2] as usize]);
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let x = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        for &i in f {
            for k in 0..3 {
                n[i as usize][k] += x[k];
            }
        }
    }
    n.into_iter().map(normalize).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-triangle glb, moved up 2 and back 3 by its node.
    fn triangle() -> Vec<u8> {
        with_material(r#"{"pbrMetallicRoughness":{"baseColorFactor":[1,0,0,1]}}"#, "")
    }

    /// The triangle with `material` as its one material and `images` (a
    /// JSON list's insides) beside it.
    fn with_material(material: &str, extra: &str) -> Vec<u8> {
        let mut bin = Vec::new();
        for v in [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, -1.0] {
            bin.extend_from_slice(&v.to_le_bytes());
        }
        for i in [0u16, 1, 2, 0] {
            bin.extend_from_slice(&i.to_le_bytes());
        }
        let json = r#"{"asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0]}],
            "nodes":[{"name":"Wheel","mesh":0,"translation":[0,2,-3]}],
            "meshes":[{"primitives":[{"attributes":{"POSITION":0},"indices":1,"material":0}]}],
            "materials":[MATERIAL],EXTRA
            "buffers":[{"byteLength":44}],
            "bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":36},{"buffer":0,"byteOffset":36,"byteLength":6}],
            "accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3"},
                         {"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}]}"#;
        let json = json.replace("MATERIAL", material).replace("EXTRA", extra);
        let mut json = json.as_bytes().to_vec();
        while json.len() % 4 != 0 {
            json.push(b' ');
        }
        let mut out = b"glTF".to_vec();
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&((12 + 8 + json.len() + 8 + bin.len()) as u32).to_le_bytes());
        out.extend_from_slice(&(json.len() as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&json);
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin);
        out
    }

    #[test]
    fn reads_a_triangle_in_game_axes() {
        let (model, images) = parse(&triangle(), "fp:t:").unwrap();
        assert!(images.is_empty());
        let s = &model.lods[0].surfaces[0];
        assert_eq!(s.positions, vec![[0.0, 2.0, 3.0], [1.0, 2.0, 3.0], [0.0, 2.0, 4.0]]);
        assert_eq!(s.faces, vec![[0, 2, 1]]);
        // The face points up in game axes too.
        assert!(s.normals[0][1] > 0.99, "{:?}", s.normals[0]);
        // Red, as BGRA.
        assert_eq!(s.colors[0], [0, 0, 255, 255]);
    }

    #[test]
    fn keeps_named_nodes_as_frames() {
        let (model, _) = parse_framed(&triangle(), "fp:t:", &["Bike", "Wheel"]).unwrap();
        assert_eq!(model.frames.len(), 2);
        assert_eq!(model.frames[1].parent, Some(0));
        // Placed in game axes, against the root.
        assert_eq!(&model.frames[1].matrix[12..15], &[0.0, 2.0, 3.0]);
        let s = &model.lods[0].surfaces[0];
        assert_eq!(s.groups, vec![(1, 0)]);
        // The vertices stay in model space, as the game's models have them.
        assert_eq!(s.positions[0], [0.0, 2.0, 3.0]);
        assert_eq!(material_names(&triangle()).unwrap(), vec![String::new()]);
    }

    #[test]
    fn reads_the_physically_based_look() {
        let glb = with_material(
            r#"{"pbrMetallicRoughness":{"baseColorTexture":{"index":0},"metallicRoughnessTexture":{"index":1},"roughnessFactor":0.5},
                "normalTexture":{"index":2,"scale":0.8},"occlusionTexture":{"index":1},"emissiveFactor":[1,0.5,0],
                "extensions":{"KHR_materials_specular":{"specularFactor":0.5}}}"#,
            r#""images":[{"uri":"textures/paint.png"},{"uri":"textures/orm.png"},{"uri":"textures/normal.png"}],
               "textures":[{"source":0},{"source":1},{"source":2}],"#,
        );
        let (model, images) = parse(&glb, "fp:t:").unwrap();
        assert_eq!(images.len(), 3);
        let m = &model.materials[0];
        assert_eq!(m.texture, "fp:file:paint.png");
        let pbr = m.pbr.as_ref().unwrap();
        assert_eq!(pbr.normal, "fp:file:normal.png");
        assert_eq!(pbr.metal_rough, "fp:file:orm.png");
        assert_eq!(pbr.occlusion, "fp:file:orm.png");
        assert_eq!(pbr.emissive, "");
        // glTF's defaults where a factor isn't given.
        assert_eq!((pbr.metallic, pbr.roughness), (1.0, 0.5));
        assert_eq!((pbr.normal_scale, pbr.occlusion_strength, pbr.specular), (0.8, 1.0, 0.5));
        assert_eq!(pbr.emissive_factor, [1.0, 0.5, 0.0]);
    }
}
