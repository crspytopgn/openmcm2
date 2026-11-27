//! The enhanced drawing's photo materials (`assets/ground`, CC0 from
//! ambientCG): dirt, packed dirt, grass, rock and sand for the ground, and
//! wood, metal, concrete and straw for the objects; each a colour with its
//! roughness and a normal map with its height. The shaders lay only their
//! detail over the game's own painted colours.

/// Each material's colour and normal files, under `assets/ground`.
const FILES: [(&str, &str); COUNT] = [
    ("dirt_albedo.webp", "dirt_normal.webp"),
    ("packed_albedo.webp", "packed_normal.webp"),
    ("grass_albedo.webp", "grass_normal.webp"),
    ("rock_albedo.webp", "rock_normal.webp"),
    ("sand_albedo.webp", "sand_normal.webp"),
    ("wood_albedo.webp", "wood_normal.webp"),
    ("metal_albedo.webp", "metal_normal.webp"),
    ("concrete_albedo.webp", "concrete_normal.webp"),
    ("straw_albedo.webp", "straw_normal.webp"),
];
pub const COUNT: usize = 9;
const SIDE: u32 = 512;

/// One material file's bytes: built into the desktop game, but in a
/// browser read from the page's store (`web/index.html` puts the ground
/// pictures there before the game starts, and only for the enhanced look),
/// so the browser build doesn't carry their 9.5 MB. The classic look
/// never fetches them at all.
fn file_bytes(name: &str) -> Option<std::borrow::Cow<'static, [u8]>> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::borrow::Cow;
        Some(Cow::Borrowed(match name {
            "dirt_albedo.webp" => include_bytes!("../assets/ground/dirt_albedo.webp"),
            "dirt_normal.webp" => include_bytes!("../assets/ground/dirt_normal.webp"),
            "packed_albedo.webp" => include_bytes!("../assets/ground/packed_albedo.webp"),
            "packed_normal.webp" => include_bytes!("../assets/ground/packed_normal.webp"),
            "grass_albedo.webp" => include_bytes!("../assets/ground/grass_albedo.webp"),
            "grass_normal.webp" => include_bytes!("../assets/ground/grass_normal.webp"),
            "rock_albedo.webp" => include_bytes!("../assets/ground/rock_albedo.webp"),
            "rock_normal.webp" => include_bytes!("../assets/ground/rock_normal.webp"),
            "sand_albedo.webp" => include_bytes!("../assets/ground/sand_albedo.webp"),
            "sand_normal.webp" => include_bytes!("../assets/ground/sand_normal.webp"),
            "wood_albedo.webp" => include_bytes!("../assets/ground/wood_albedo.webp"),
            "wood_normal.webp" => include_bytes!("../assets/ground/wood_normal.webp"),
            "metal_albedo.webp" => include_bytes!("../assets/ground/metal_albedo.webp"),
            "metal_normal.webp" => include_bytes!("../assets/ground/metal_normal.webp"),
            "concrete_albedo.webp" => include_bytes!("../assets/ground/concrete_albedo.webp"),
            "concrete_normal.webp" => include_bytes!("../assets/ground/concrete_normal.webp"),
            "straw_albedo.webp" => include_bytes!("../assets/ground/straw_albedo.webp"),
            "straw_normal.webp" => include_bytes!("../assets/ground/straw_normal.webp"),
            _ => return None,
        }))
    }
    #[cfg(target_arch = "wasm32")]
    {
        crate::fs::read(format!("/assets/ground/{name}")).ok().map(std::borrow::Cow::Owned)
    }
}

/// The objects' materials' layers.
const WOOD: u32 = 5;
const METAL: u32 = 6;
const CONCRETE: u32 = 7;
const STRAW: u32 = 8;

/// The tracks whose objects get the photo materials: every track (it began
/// as a trial on Farm). Textures `surface` doesn't know stay as they are.
pub fn surfaced(_track_file: &str) -> bool {
    true
}

/// Whether an object texture is a sponsor's banner or board, lit from
/// behind at night (the enhanced drawing's night races): the boards along
/// the National tracks and the finish banner, each texture all banner.
pub fn backlit(texture: &str) -> bool {
    let stem = texture.to_ascii_lowercase();
    matches!(stem.split('.').next().unwrap_or_default(), "signs_08" | "signs2_08" | "signs3_08" | "finishbanner_08")
}

/// The material an object texture is mostly made of, picked by eye from
/// the game's textures; None for those left as they are (people, bikes,
/// signs, banners, tyres, plants and textures mixing many materials).
pub fn surface(texture: &str) -> Option<u32> {
    let stem = texture.to_ascii_lowercase();
    let stem = stem.split('.').next().unwrap_or_default();
    Some(match stem {
        // Farm.
        "barn_mapint_08" | "barn_mapwalls_08" | "boxes_06" | "fence_picket_08" | "fh_mapa_08" | "haywagon_08"
        | "lightpole_08" | "loadingramp_08" | "pumpshack_08" | "rrt_08" | "supports_08" | "tower_08"
        | "windmill_pump01_08" => WOOD,
        "barn_maproof01_08" | "barn_mapslide_07" | "blind_08" | "boxcars_08" | "caboose_08" | "cattlecars_08"
        | "cattleguard_08" | "combine3_08" | "engine02_08" | "fence_08" | "flatbed_08" | "fx_map_08" | "hopper_08"
        | "silo_map_08" | "tractor_08" | "trc-old_map_08" | "tressel02_08" => METAL,
        "brickbldg01_08" | "culvert01_08" | "gastation02_08" | "irrigcanal_08" | "ms_map_08" | "support_08" => CONCRETE,
        "haybail3_08" => STRAW,
        // The other tracks: cabins, bridges, sheds, fences and poles.
        "bridge_old_08" | "cabin_08" | "hut_mapa" | "miningshack01_08" | "pedbrdg_wood_08" | "rs_map_08"
        | "rsmapnosnow_08" | "splitrail02_07" | "telephonepole_07" | "walls01_08" | "outhouse_07"
        | "picnictable01_07" | "station_08" | "skilodgemap_08" | "cargo_map_08" | "still_07" => WOOD,
        // Hangars, towers, tanks, trailers, machines and vehicles.
        "ant-cel_map_08" | "antenna01" | "bridge_map_08" | "cargowreck01_08" | "cargowreck_inside01_08"
        | "cargoplane01" | "cargoplane_inside01_08" | "carport01" | "container_08" | "hanger_08" | "ht_map"
        | "ht_map03" | "mainoffice_08" | "mtlarchmap_08" | "pedbrdg_metal_08" | "pit_bridge_08" | "quonset_08"
        | "rail01_07" | "twr-d_map_08" | "water_tank_08" | "scraper01_08" | "dumptruk_08" | "semitrailer_08"
        | "machines01_08" | "sy_map_08" | "op_map_08" | "firetruck_08" | "schoolbus_08" | "semicab_08"
        | "semilog_08" | "watertruck_08" | "pickupmap_08" | "4runner_blue_08" | "4runner_brown_08"
        | "4runner_green_08" | "4runner_white_08" | "jeep_08" | "jeep_green_08" | "jeep_grey_08"
        | "chevelleblue_08" | "chevellered_08" | "300zx_08" | "toyotamap_08" | "famt_map_08" | "bobcat_map_08"
        | "cop_08" | "cop_navy_08" | "airplanetractor01_08" | "plane02_08" | "beech18rev02_08"
        | "helinewsbody_08" => METAL,
        // Stone, concrete and brick.
        "floor01_08" | "helipad_08" | "rrt_elev_08" | "rrt_culvert_08" | "sl-bridge_map_08" | "sl-patio_map_08"
        | "pyramid_mapa_08" | "ruin_mapa_08" | "steps01_06" | "radiostation01_08" => CONCRETE,
        _ => return None,
    })
}

pub struct Materials {
    pub albedo: wgpu::TextureView,
    pub normal: wgpu::TextureView,
    /// Each material's average colour (linear), which the shader divides
    /// out so that only its detail is laid over the painted colour.
    pub average: [[f32; 4]; COUNT],
}

/// A material's picture decoded to RGBA8, or None when its file is
/// missing or won't decode (in a browser, when the page didn't fetch it):
/// the caller then stands in a flat picture, so the game still runs.
fn decode(webp: &[u8]) -> Option<Vec<u8>> {
    // Lossless WebP: the same pixels as the PNGs they were, in 16% less.
    let mut decoder = image_webp::WebPDecoder::new(std::io::Cursor::new(webp)).ok()?;
    if decoder.dimensions() != (SIDE, SIDE) {
        return None;
    }
    let mut buf = vec![0; decoder.output_buffer_size()?];
    decoder.read_image(&mut buf).ok()?;
    if decoder.has_alpha() {
        return Some(buf);
    }
    // Stored without the alpha it had, all opaque.
    Some(buf.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect())
}

/// A flat picture standing in for a missing material: mid-grey and
/// middling rough for a colour picture, flat and pointing up for a
/// normal map. Only its detail is laid over the painted colour, so the
/// ground still looks like the game painted it.
fn flat(albedo: bool) -> Vec<u8> {
    let px: [u8; 4] = if albedo { [128, 128, 128, 128] } else { [128, 128, 255, 128] };
    px.repeat((SIDE * SIDE) as usize)
}

/// Halves an RGBA8 image `side` texels square, linear in sRGB colour for
/// `srgb` (only RGB; alpha is always linear).
/// A ground material's colour picture (RGB, roughness in A), by name:
/// dirt, packed, grass, rock or sand.
pub(crate) fn albedo(name: &str) -> Option<std::borrow::Cow<'static, [u8]>> {
    let i = ["dirt", "packed", "grass", "rock", "sand"].iter().position(|n| *n == name)?;
    file_bytes(FILES[i].0)
}

pub(crate) fn halve(level: &[u8], side: u32, srgb: bool) -> Vec<u8> {
    let half = (side / 2).max(1);
    // Each byte's value once, not four times a texel (the same sums).
    let linear: [f32; 256] = std::array::from_fn(|v| (v as f32 / 255.0).powf(2.2));
    let plain: [f32; 256] = std::array::from_fn(|v| v as f32 / 255.0);
    let to = |v: u8, c: usize| if srgb && c < 3 { linear[v as usize] } else { plain[v as usize] };
    let from = |v: f32, c: usize| {
        let v = if srgb && c < 3 { v.powf(1.0 / 2.2) } else { v };
        (v * 255.0).round().clamp(0.0, 255.0) as u8
    };
    let mut out = vec![0u8; (half * half * 4) as usize];
    for y in 0..half {
        for x in 0..half {
            for c in 0..4 {
                let mut sum = 0.0;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (sx, sy) = ((2 * x + dx).min(side - 1), (2 * y + dy).min(side - 1));
                    sum += to(level[((sy * side + sx) * 4) as usize + c], c);
                }
                out[((y * half + x) * 4) as usize + c] = from(sum / 4.0, c);
            }
        }
    }
    out
}

fn array(device: &wgpu::Device, queue: &wgpu::Queue, label: &str, layers: &[Vec<u8>], format: wgpu::TextureFormat) -> wgpu::TextureView {
    let srgb = format.is_srgb();
    let mips = SIDE.ilog2() + 1;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: SIDE,
            height: SIDE,
            depth_or_array_layers: crate::renderer::array_layers(layers.len() as u32, SIDE, SIDE),
        },
        mip_level_count: mips,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (layer, top) in layers.iter().enumerate() {
        let mut level = top.clone();
        let mut side = SIDE;
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
                    bytes_per_row: Some(side * 4),
                    rows_per_image: Some(side),
                },
                wgpu::Extent3d {
                    width: side,
                    height: side,
                    depth_or_array_layers: 1,
                },
            );
            if side > 1 {
                level = halve(&level, side, srgb);
                side /= 2;
            }
        }
    }
    texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    })
}

impl Materials {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Materials {
        // A file the page didn't fetch (or that won't decode) stands in
        // flat, rather than stopping the game: its detail only is laid
        // over the painted colour.
        let load = |name: &str, albedo: bool| {
            file_bytes(name).and_then(|b| decode(&b)).unwrap_or_else(|| {
                eprintln!("ground material {name}: missing, standing in flat");
                flat(albedo)
            })
        };
        let albedo: Vec<Vec<u8>> = FILES.iter().map(|m| load(m.0, true)).collect();
        let normal: Vec<Vec<u8>> = FILES.iter().map(|m| load(m.1, false)).collect();
        let average = std::array::from_fn(|i| {
            let mut sum = [0.0f64; 3];
            for p in albedo[i].chunks_exact(4) {
                for c in 0..3 {
                    sum[c] += (p[c] as f64 / 255.0).powf(2.2);
                }
            }
            let n = (SIDE * SIDE) as f64;
            [(sum[0] / n) as f32, (sum[1] / n) as f32, (sum[2] / n) as f32, 1.0]
        });
        Materials {
            albedo: array(device, queue, "ground colour", &albedo, wgpu::TextureFormat::Rgba8UnormSrgb),
            normal: array(device, queue, "ground normals", &normal, wgpu::TextureFormat::Rgba8Unorm),
            average,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_material_decodes_to_rgba() {
        for (albedo, normal) in FILES {
            for m in [albedo, normal] {
                let bytes = file_bytes(m).expect("built-in material");
                assert_eq!(decode(&bytes).expect("built-in material").len(), (SIDE * SIDE * 4) as usize);
            }
        }
    }

    #[test]
    fn a_flat_picture_stands_in_for_a_missing_material() {
        assert_eq!(flat(true).len(), (SIDE * SIDE * 4) as usize);
        assert_eq!(flat(false).len(), (SIDE * SIDE * 4) as usize);
        // Flat normal points up, flat colour is mid-grey.
        assert_eq!(&flat(false)[..4], &[128, 128, 255, 128]);
        assert_eq!(decode(&[]), None);
    }
}
