//! The enhanced ground's photo materials (`assets/ground`, CC0 from
//! ambientCG): dirt, packed dirt, grass, rock and sand, each a colour with
//! its roughness and a normal map with its height, laid over the game's
//! painted terrain by the shader.

/// Each material's colour (RGB) and roughness (A), and its normal (RGB)
/// and height (A).
const MATERIALS: [(&[u8], &[u8]); COUNT] = [
    (include_bytes!("../assets/ground/dirt_albedo.png"), include_bytes!("../assets/ground/dirt_normal.png")),
    (include_bytes!("../assets/ground/packed_albedo.png"), include_bytes!("../assets/ground/packed_normal.png")),
    (include_bytes!("../assets/ground/grass_albedo.png"), include_bytes!("../assets/ground/grass_normal.png")),
    (include_bytes!("../assets/ground/rock_albedo.png"), include_bytes!("../assets/ground/rock_normal.png")),
    (include_bytes!("../assets/ground/sand_albedo.png"), include_bytes!("../assets/ground/sand_normal.png")),
];
pub const COUNT: usize = 5;
const SIDE: u32 = 512;

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
            depth_or_array_layers: layers.len() as u32,
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
