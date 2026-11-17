//! The free pack's fine ground: its tracks bake their ground into the
//! usual 256 px tiles (about 0.75 ft a texel), and keep beside them a map
//! of which two ground materials cover each terrain sample (`<track>.gmt`,
//! written by `mm2-tools freetracks`). The terrain shader lays those
//! materials' full-size pictures over the baked colour, adding only the
//! detail finer than a baked texel, so the ground stays the colour it was
//! baked and turns crisp close up. The game's own tracks have no map and
//! are drawn as they always were.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// The side of each material's picture on the GPU.
pub const SIDE: u32 = 1024;
/// At most this many materials (the shader's array).
pub const MAX: usize = 12;

/// A free track's ground map, with its materials' pictures.
pub struct GroundMap {
    /// Samples per side.
    pub size: u32,
    /// Per sample: first material, second, the second's share (0..255),
    /// spare.
    pub cells: Vec<u8>,
    /// The baked tiles' feet per texel.
    pub feet_per_texel: f32,
    /// Per material: one over the feet one repeat covers.
    pub repeats: Vec<f32>,
    /// Per material: its picture, SIDE square, sRGB RGBA8, or none to add
    /// no detail.
    pub layers: Vec<Option<Arc<Vec<u8>>>>,
}

/// The ground uniform: x 1 when on, y the baked feet per texel, z the
/// picture's side; per material x one over its repeat in feet, y 1 when
/// it has a picture.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Info {
    pub on: [f32; 4],
    pub repeats: [[f32; 4]; MAX],
}

impl GroundMap {
    /// Reads a `.gmt`, loading its materials from the pack's `ground`
    /// folder. None when the map is unreadable or the pack is not in use.
    pub fn parse(data: &[u8]) -> Option<GroundMap> {
        let u32_at = |o: usize| Some(u32::from_le_bytes(data.get(o..o + 4)?.try_into().ok()?));
        let f32_at = |o: usize| Some(f32::from_le_bytes(data.get(o..o + 4)?.try_into().ok()?));
        // MM2_NO_FINE_GROUND: draw the baked tiles alone, to compare.
        if std::env::var_os("MM2_NO_FINE_GROUND").is_some() || data.get(..4)? != b"GMT1" {
            return None;
        }
        let size = u32_at(4)?;
        let count = (u32_at(8)? as usize).min(MAX);
        // The pack's pictures when it is in use, else the game's own photo
        // materials, so builder tracks are crisp with either art.
        let dir = crate::freepack::root().map(|r| r.join("ground"));
        let mut repeats = Vec::new();
        let mut layers = Vec::new();
        let mut at = 12;
        for _ in 0..count {
            let name = data.get(at..at + 16)?;
            let name =
                String::from_utf8_lossy(&name[..name.iter().position(|&b| b == 0).unwrap_or(16)])
                    .into_owned();
            repeats.push(1.0 / f32_at(at + 16)?.max(1.0));
            layers.push(picture(dir.as_deref(), &name));
            at += 20;
        }
        let feet_per_texel = f32_at(at)?;
        at += 4;
        let cells = data.get(at..at + (size * size * 4) as usize)?.to_vec();
        layers.iter().any(Option::is_some).then_some(GroundMap {
            size,
            cells,
            feet_per_texel,
            repeats,
            layers,
        })
    }

    pub fn info(&self) -> Info {
        let mut info = Info {
            on: [1.0, self.feet_per_texel, SIDE as f32, 0.0],
            repeats: [[0.0; 4]; MAX],
        };
        for (m, r) in self.repeats.iter().enumerate() {
            info.repeats[m] = [
                *r,
                if self.layers[m].is_some() { 1.0 } else { 0.0 },
                0.0,
                0.0,
            ];
        }
        info
    }
}

/// The game's own photo material standing in for a pack ground picture.
fn stand_in(name: &str) -> Option<&'static str> {
    Some(match name {
        "trackdirt" | "sxclay" => "packed",
        "loosedirt" | "mud" | "forestfloor" => "dirt",
        "grass" | "drygrass" => "grass",
        "desertsand" => "sand",
        "rock" | "gravel" => "rock",
        _ => return None,
    })
}

/// A ground picture as RGBA8, square: the pack's `<name>.png`, or the
/// game's own photo material standing in for it.
fn load(dir: Option<&std::path::Path>, name: &str) -> Option<(u32, Vec<u8>)> {
    if let Some(dir) = dir {
        if let Ok(bytes) = crate::fs::read(&dir.join(format!("{name}.png"))) {
            if let Ok((w, h, rgba)) = crate::hires::decode_png(&bytes) {
                if w == h && w.is_power_of_two() {
                    return Some((w, rgba));
                }
                eprintln!("free pack ground {name}: {w}x{h}, wanted a square power of two");
            }
        }
    }
    let (w, h, mut rgba) = crate::hires::decode_png(crate::materials::albedo(stand_in(name)?)?).ok()?;
    if w != h {
        return None;
    }
    // Its alpha is roughness, not cover.
    for px in rgba.chunks_exact_mut(4) {
        px[3] = 255;
    }
    Some((w, rgba))
}

/// Doubles a square RGBA8 picture, bilinear and wrapping.
fn double(rgba: &[u8], side: u32) -> Vec<u8> {
    let s = side as i64;
    let at = |x: i64, y: i64, c: usize| rgba[((y.rem_euclid(s) * s + x.rem_euclid(s)) * 4) as usize + c] as f32;
    let mut out = Vec::with_capacity(rgba.len() * 4);
    for j in 0..s * 2 {
        for i in 0..s * 2 {
            let (u, v) = (i as f32 / 2.0 - 0.25, j as f32 / 2.0 - 0.25);
            let (x, y) = (u.floor() as i64, v.floor() as i64);
            let (fx, fy) = (u - u.floor(), v - v.floor());
            for c in 0..4 {
                let a = at(x, y, c) * (1.0 - fx) + at(x + 1, y, c) * fx;
                let b = at(x, y + 1, c) * (1.0 - fx) + at(x + 1, y + 1, c) * fx;
                out.push((a * (1.0 - fy) + b * fy).round() as u8);
            }
        }
    }
    out
}

/// A ground material's picture at SIDE, kept for the next track.
fn picture(dir: Option<&std::path::Path>, name: &str) -> Option<Arc<Vec<u8>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Arc<Vec<u8>>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let key = format!("{}|{name}", dir.is_some());
    if let Some(hit) = cache.lock().ok()?.get(&key) {
        return hit.clone();
    }
    let loaded = (|| {
        let (mut side, mut rgba) = load(dir, name)?;
        while side > SIDE {
            rgba = crate::materials::halve(&rgba, side, true);
            side /= 2;
        }
        while side < SIDE {
            rgba = double(&rgba, side);
            side *= 2;
        }
        Some(Arc::new(rgba))
    })();
    if let Ok(mut c) = cache.lock() {
        c.insert(key, loaded.clone());
    }
    loaded
}

/// The pictures the Course Builder paints its ground from: the pack's
/// when it is in use, else the game's own photo materials.
pub fn builder_pictures() -> mm2_course::env::Pictures {
    static PICS: OnceLock<mm2_course::env::Pictures> = OnceLock::new();
    PICS.get_or_init(|| {
        let dir = crate::freepack::root().map(|r| r.join("ground"));
        let mut out = mm2_course::env::Pictures::default();
        for (name, _) in mm2_course::env::GROUND {
            let Some((mut side, mut rgba)) = load(dir.as_deref(), name) else { continue };
            while side > 512 {
                rgba = crate::materials::halve(&rgba, side, true);
                side /= 2;
            }
            let rgb = rgba
                .chunks_exact(4)
                .map(|p| [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0])
                .collect();
            out.by_name.insert(
                name.to_string(),
                Arc::new(mm2_course::env::Picture { side: side as usize, rgb }),
            );
        }
        out
    })
    .clone()
}

/// The GPU side: the map, the pictures, their sampler and the uniform.
pub struct Views {
    pub map: wgpu::TextureView,
    pub pictures: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub info: wgpu::Buffer,
}

impl Views {
    /// None drawn: a one-sample map and a blank picture, switched off.
    pub fn none(device: &wgpu::Device, queue: &wgpu::Queue) -> Views {
        let info = Info {
            on: [0.0; 4],
            repeats: [[0.0; 4]; MAX],
        };
        Views::build(device, queue, 1, &[0; 4], &[None], &info)
    }

    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, map: &GroundMap) -> Views {
        Views::build(
            device,
            queue,
            map.size,
            &map.cells,
            &map.layers,
            &map.info(),
        )
    }

    fn build(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: u32,
        cells: &[u8],
        layers: &[Option<Arc<Vec<u8>>>],
        info: &Info,
    ) -> Views {
        use wgpu::util::DeviceExt;
        let map = device
            .create_texture_with_data(
                queue,
                &wgpu::TextureDescriptor {
                    label: Some("ground map"),
                    size: wgpu::Extent3d {
                        width: size,
                        height: size,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Uint,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                cells,
            )
            .create_view(&Default::default());
        // Blank layers are a single grey texel's worth of SIDE: cheap to
        // skip, since the shader never samples a layer without a picture.
        let side = if layers.iter().any(Option::is_some) {
            SIDE
        } else {
            1
        };
        let count = crate::renderer::array_layers(layers.len().max(2) as u32, side, side);
        let mips = side.ilog2() + 1;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ground pictures"),
            size: wgpu::Extent3d {
                width: side,
                height: side,
                depth_or_array_layers: count,
            },
            mip_level_count: mips,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (layer, top) in layers.iter().enumerate() {
            let Some(top) = top else { continue };
            let mut level = top.as_ref().clone();
            let mut s = side;
            for mip in 0..mips {
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: mip,
                        origin: wgpu::Origin3d {
                            x: 0,
                            y: 0,
                            z: layer as u32,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &level,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(s * 4),
                        rows_per_image: Some(s),
                    },
                    wgpu::Extent3d {
                        width: s,
                        height: s,
                        depth_or_array_layers: 1,
                    },
                );
                if s > 1 {
                    level = crate::materials::halve(&level, s, true);
                    s /= 2;
                }
            }
        }
        let pictures = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ground pictures"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: crate::graphics::anisotropy(),
            ..Default::default()
        });
        let info = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ground info"),
            contents: bytemuck::bytes_of(info),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        Views {
            map,
            pictures,
            sampler,
            info,
        }
    }
}
