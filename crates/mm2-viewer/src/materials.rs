//! The enhanced drawing's photo materials (`assets/ground`, CC0 from
//! ambientCG): dirt, packed dirt, grass, rock and sand for the ground, and
//! wood, metal, concrete and straw for the objects; each a colour with its
//! roughness and a normal map with its height. The shaders lay only their
//! detail over the game's own painted colours.

/// Each material's colour (RGB) and roughness (A), and its normal (RGB)
/// and height (A).
const MATERIALS: [(&[u8], &[u8]); COUNT] = [
    (include_bytes!("../assets/ground/dirt_albedo.png"), include_bytes!("../assets/ground/dirt_normal.png")),
    (include_bytes!("../assets/ground/packed_albedo.png"), include_bytes!("../assets/ground/packed_normal.png")),
    (include_bytes!("../assets/ground/grass_albedo.png"), include_bytes!("../assets/ground/grass_normal.png")),
    (include_bytes!("../assets/ground/rock_albedo.png"), include_bytes!("../assets/ground/rock_normal.png")),
    (include_bytes!("../assets/ground/sand_albedo.png"), include_bytes!("../assets/ground/sand_normal.png")),
    (include_bytes!("../assets/ground/wood_albedo.png"), include_bytes!("../assets/ground/wood_normal.png")),
    (include_bytes!("../assets/ground/metal_albedo.png"), include_bytes!("../assets/ground/metal_normal.png")),
    (include_bytes!("../assets/ground/concrete_albedo.png"), include_bytes!("../assets/ground/concrete_normal.png")),
    (include_bytes!("../assets/ground/straw_albedo.png"), include_bytes!("../assets/ground/straw_normal.png")),
];
pub const COUNT: usize = 9;
const SIDE: u32 = 512;

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

fn decode(png_bytes: &[u8]) -> Vec<u8> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().expect("built-in material");
    let mut buf = vec![0; reader.output_buffer_size().expect("built-in material")];
    let info = reader.next_frame(&mut buf).expect("built-in material");
    assert_eq!((info.width, info.height, info.color_type), (SIDE, SIDE, png::ColorType::Rgba));
    buf.truncate(info.buffer_size());
    buf
}

/// Halves an RGBA8 image `side` texels square, linear in sRGB colour for
/// `srgb` (only RGB; alpha is always linear).
fn halve(level: &[u8], side: u32, srgb: bool) -> Vec<u8> {
    let half = (side / 2).max(1);
    let to = |v: u8, c: usize| if srgb && c < 3 { (v as f32 / 255.0).powf(2.2) } else { v as f32 / 255.0 };
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
        let albedo: Vec<Vec<u8>> = MATERIALS.iter().map(|m| decode(m.0)).collect();
        let normal: Vec<Vec<u8>> = MATERIALS.iter().map(|m| decode(m.1)).collect();
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
