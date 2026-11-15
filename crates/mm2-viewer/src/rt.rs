//! Ray-traced sun shadows and reflections (`--enhanced=raytracing`), on a GPU that can
//! trace rays (Vulkan with ray queries; [`crate::graphics::ray_traced`]).
//!
//! In place of the three shadow cascades, each pixel asks the scene
//! itself whether the sun is in sight: the hills, every still object and
//! the moving models (bikes, riders, people, gates) are built into
//! acceleration structures, the moving ones again each frame as they are
//! posed. So the shadows are as sharp close up as far off, reach to the
//! haze, and the hills cast them too. Cut-out texels (leaves, fences) let
//! the sun through, as they do in the cascades.
//!
//! The same scene gives the reflections: still water and glossy paint
//! show what a ray from them meets, its texture lit by the sun where the
//! sun reaches it, and the sky where it meets nothing.

use mm2_formats::trn::Terrain;
use wgpu::util::DeviceExt;

/// What each instance is, for a ray to pick which it sees: indoors only
/// the moving models cast, as with the cascades.
pub const TERRAIN: u8 = 1;
pub const STILL: u8 = 2;
pub const MOVING: u8 = 4;

/// Texels a side of a cut-out texture's mask.
pub const MASK_SIDE: u32 = 256;
/// Texels a side of each texture's colours, for the reflections.
pub const COLOUR_SIDE: u32 = 128;
/// A triangle with no texture: its vertex colours only.
pub const NO_COLOUR: u32 = u32::MAX;
/// Texels a side of each terrain tile in the ground's colour map.
const GROUND_TEXELS: u32 = 64;

/// The objects as the rays see them ([`crate::objects::ObjectMesh::rt_geometry`]).
#[derive(Default)]
pub struct Geometry {
    /// Every vertex at rest (drawing coordinates); the still ones are
    /// where they are drawn.
    pub positions: Vec<[f32; 3]>,
    /// The still triangles: solid, and cut out (each with a record).
    pub still: [Vec<u32>; 2],
    /// The moving triangles, by their vertex in [`Geometry::sources`]:
    /// solid, and cut out.
    pub moving: [Vec<u32>; 2],
    /// The object vertices the moving triangles are made of.
    pub sources: Vec<u32>,
    /// Each cut-out triangle's texture coordinates and mask: the still
    /// ones, then the moving ones. Two vec4s: (u0, v0, u1, v1), (u2, v2,
    /// layer, the mask's texels a ft on it).
    pub cutouts: Vec<[f32; 8]>,
    /// The cut-out textures' masks, [`MASK_SIDE`] a side, 255 solid; each
    /// is made smaller and smaller for its mips.
    pub masks: Vec<Vec<u8>>,
    /// Each triangle's object vertices and colour layer, as a reflection
    /// meets it: still solid, still cut out, moving solid, moving cut out.
    pub hits: [Vec<[u32; 4]>; 4],
    /// The textures' colours, [`COLOUR_SIDE`] a side, RGBA.
    pub colours: Vec<Vec<u8>>,
}

/// A triangle geometry's size, and the buffers it is built from.
struct Part {
    size: wgpu::BlasTriangleGeometrySizeDescriptor,
    indices: wgpu::Buffer,
    /// Which of the geometry's parts it is: 0 solid, 1 cut out.
    which: usize,
}

/// An acceleration structure of up to two parts (solid, cut out) over one
/// vertex buffer.
struct Built {
    blas: wgpu::Blas,
    vertices: wgpu::Buffer,
    parts: Vec<Part>,
}

impl Built {
    fn new(device: &wgpu::Device, label: &str, vertices: wgpu::Buffer, vertex_count: u32, parts: [&[u32]; 2], moving: bool) -> Option<Built> {
        let parts: Vec<Part> = parts
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.is_empty())
            .map(|(i, p)| Part {
                size: wgpu::BlasTriangleGeometrySizeDescriptor {
                    vertex_format: wgpu::VertexFormat::Float32x3,
                    vertex_count,
                    index_format: Some(wgpu::IndexFormat::Uint32),
                    index_count: Some(p.len() as u32),
                    flags: if i == 0 {
                        wgpu::AccelerationStructureGeometryFlags::OPAQUE
                    } else {
                        wgpu::AccelerationStructureGeometryFlags::empty()
                    },
                },
                indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents: bytemuck::cast_slice(p),
                    usage: wgpu::BufferUsages::BLAS_INPUT | wgpu::BufferUsages::STORAGE,
                }),
                which: i,
            })
            .collect();
        if parts.is_empty() {
            return None;
        }
        let blas = device.create_blas(
            &wgpu::CreateBlasDescriptor {
                label: Some(label),
                flags: if moving {
                    wgpu::AccelerationStructureFlags::PREFER_FAST_BUILD | wgpu::AccelerationStructureFlags::ALLOW_UPDATE
                } else {
                    wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE
                },
                update_mode: if moving {
                    wgpu::AccelerationStructureUpdateMode::PreferUpdate
                } else {
                    wgpu::AccelerationStructureUpdateMode::Build
                },
            },
            wgpu::BlasGeometrySizeDescriptors::Triangles {
                descriptors: parts.iter().map(|p| p.size.clone()).collect(),
            },
        );
        Some(Built { blas, vertices, parts })
    }

    fn entry(&self) -> wgpu::BlasBuildEntry<'_> {
        wgpu::BlasBuildEntry {
            blas: &self.blas,
            geometry: wgpu::BlasGeometries::TriangleGeometries(
                self.parts
                    .iter()
                    .map(|p| wgpu::BlasTriangleGeometry {
                        size: &p.size,
                        vertex_buffer: &self.vertices,
                        first_vertex: 0,
                        vertex_stride: 12,
                        index_buffer: Some(&p.indices),
                        first_index: Some(0),
                        transform_buffer: None,
                        transform_buffer_offset: None,
                    })
                    .collect(),
            ),
        }
    }
}

/// The moving models, posed each frame from the objects' own vertices and
/// nodes by a compute pass, then built again.
struct Moving {
    built: Built,
    /// Where each posed vertex was the frame before, for the blend over
    /// time to follow the moving models.
    previous: wgpu::Buffer,
    sources: wgpu::Buffer,
    count: u32,
    pipeline: wgpu::ComputePipeline,
    group: Option<wgpu::BindGroup>,
}

/// The textures the shadows are traced in, at the scene's size.
struct Targets {
    /// The headlights' light at each pixel (rt_shadow.wgsl).
    lamps: wgpu::TextureView,
    size: (u32, u32),
    /// The nearest surface's depth for each pixel, drawn first.
    depth: wgpu::TextureView,
    /// The rays' light, blurred.
    soft: wgpu::TextureView,
    trace_groups: [wgpu::BindGroup; 2],
    /// Each 2 x 2 block's first pixel traced ahead (rt_shadow.wgsl
    /// `probe`), at half size.
    probe_groups: [wgpu::BindGroup; 2],
    probe_size: (u32, u32),
    /// Blending in the frames before, and blurring what that gives: one of
    /// each for either of the two histories being the one written.
    blend_groups: [wgpu::BindGroup; 2],
    blur_groups: [wgpu::BindGroup; 2],
    /// The light shafts (rt_volume.wgsl), at half size: traced, then
    /// blended over time into either history and what shows.
    shaft_group: wgpu::BindGroup,
    shaft_settle_groups: [wgpu::BindGroup; 2],
    shaft_size: (u32, u32),
    shafts: wgpu::TextureView,
}

/// The per-pixel passes: the rays, the frames before blended in, then
/// the blur.
struct Passes {
    params: wgpu::Buffer,
    trace: wgpu::ComputePipeline,
    probe: wgpu::ComputePipeline,
    /// The light poles' light over the ground, baked once (crate::flood).
    bake: wgpu::ComputePipeline,
    blend: wgpu::ComputePipeline,
    blur: wgpu::ComputePipeline,
    shafts: wgpu::ComputePipeline,
    settle: wgpu::ComputePipeline,
    /// The frame before's view and projection and camera, while the
    /// history holds it.
    before: Option<(glam::Mat4, glam::Vec3)>,
    targets: Option<Targets>,
    /// Before the first frame, for the shared group.
    blank: wgpu::TextureView,
    frame: u32,
}

/// The passes' settings (rt_shadow.wgsl Params).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    inverse_view_proj: [[f32; 4]; 4],
    camera: [f32; 4],
    to_sun: [f32; 4],
    size: [f32; 4],
    /// The frame before's (rt_temporal.wgsl): its view and projection,
    /// and whether there is one and where its camera was.
    previous: [[f32; 4]; 4],
    history: [f32; 4],
    /// x how many floodlights light the shadows in place of the sun (0
    /// outdoors); y how many headlights, z how much it is night.
    lights: [f32; 4],
}

/// How steeply a light pole's lamps point down, against how far toward
/// the track it faces (1 to 1 is 45 degrees).
const POLE_DOWN: f32 = 1.2;

/// The way a light pole placed facing `facing` shines: down onto the
/// track, which lies behind the way it faces.
pub fn pole_way(facing: glam::Vec3) -> glam::Vec3 {
    (glam::Vec3::NEG_Y * POLE_DOWN - facing).normalize()
}

/// The shadows' shader: rt_shadow.wgsl with the lamps' beams
/// (lamp_light.wgsl) and the poles' baked light (flood.wgsl, traced by
/// flood_rt.wgsl).
pub fn shadow_source() -> String {
    let shadow = include_str!("rt_shadow.wgsl").replacen("enable wgpu_ray_query;", &format!("enable wgpu_ray_query;\n{}", include_str!("lamp_light.wgsl")), 1);
    format!("{shadow}\n{}\n{}", include_str!("flood.wgsl"), include_str!("flood_rt.wgsl"))
}

/// The poles' baked light as the trace reads it (crate::flood): blanks
/// till it is baked.
struct FloodBinds {
    heights: wgpu::Buffer,
    uniform: wgpu::Buffer,
    map: wgpu::TextureView,
}

/// The most headlights traced.
const MOST_HEADLIGHTS: usize = 80;

/// What the scene's shaders read the traced shadows from.
pub const SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// The depth pass's depth.
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// What the reflections read of the scene.
struct Reflections {
    /// Where each instance's parts start (eight words), then each
    /// triangle's object vertices and colour layer.
    hits: wgpu::Buffer,
    colours: wgpu::TextureView,
    /// The ground's colour over the whole terrain.
    ground: wgpu::TextureView,
    /// The objects' vertices and nodes ([`Scene::attach`]), or stand-ins
    /// when there are no objects.
    vertices: wgpu::Buffer,
    nodes: wgpu::Buffer,
}

/// The scene as the rays see it.
pub struct Scene {
    passes: Passes,
    /// A domed stadium's floodlights (rt_shadow.wgsl), two vec4s each:
    /// (middle, spread), (how many lamps, ...); and how many.
    floodlights: wgpu::Buffer,
    floodlight_count: u32,
    /// The bikes' headlights this frame, two vec4s each: (place, 0), (way,
    /// 0).
    headlights: wgpu::Buffer,
    reflections: Reflections,
    pub tlas: wgpu::Tlas,
    terrain: Built,
    still: Option<Built>,
    moving: Option<Moving>,
    /// The cut-out triangles' records and the masks, for the shaders.
    pub cutouts: wgpu::Buffer,
    pub masks: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    /// Whether the still parts are built.
    built: bool,
    flood: FloodBinds,
}

/// The instance transform that leaves things where they are.
const IDENTITY: [f32; 12] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];

impl Scene {
    /// The scene of `terrain` (`ground` its tiles' textures) and the
    /// objects' `geometry`.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        terrain: &Terrain,
        ground: &[crate::hires::Image],
        geometry: &Geometry,
        floodlights: &[(glam::Vec3, f32, f32)],
    ) -> Scene {
        eprintln!(
            "ray tracing: {} still triangles ({} cut out), {} moving ({} cut out), {} masks",
            (geometry.still[0].len() + geometry.still[1].len()) / 3,
            geometry.still[1].len() / 3,
            (geometry.moving[0].len() + geometry.moving[1].len()) / 3,
            geometry.moving[1].len() / 3,
            geometry.masks.len()
        );
        let ground = ground_map(device, queue, terrain, ground);
        // The terrain as it is drawn (renderer.rs): x along a row, the
        // height up, rows along -z, split as the ground is sampled.
        let n = terrain.size;
        let s = terrain.scale;
        let points: Vec<[f32; 3]> = (0..n * n)
            .map(|i| [(i % n) as f32 * s, terrain.heights[i] * s, -((i / n) as f32) * s])
            .collect();
        let mut indices = Vec::with_capacity((n - 1) * (n - 1) * 6);
        let n = n as u32;
        for z in 0..n - 1 {
            for x in 0..n - 1 {
                let i = z * n + x;
                let below = i + n;
                if (x ^ z) & 1 == 0 {
                    indices.extend_from_slice(&[i, below, below + 1, i, below + 1, i + 1]);
                } else {
                    indices.extend_from_slice(&[i, below, i + 1, i + 1, below, below + 1]);
                }
            }
        }
        let input = |label: &str, data: &[[f32; 3]], extra: wgpu::BufferUsages| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(data),
                usage: wgpu::BufferUsages::BLAS_INPUT | extra,
            })
        };
        let terrain = Built::new(
            device,
            "rt terrain",
            input("rt terrain", &points, wgpu::BufferUsages::STORAGE),
            points.len() as u32,
            [&indices, &[]],
            false,
        )
        .expect("a terrain has triangles");
        let still = (!geometry.positions.is_empty())
            .then(|| {
                Built::new(
                    device,
                    "rt still",
                    input("rt still", &geometry.positions, wgpu::BufferUsages::empty()),
                    geometry.positions.len() as u32,
                    [&geometry.still[0], &geometry.still[1]],
                    false,
                )
            })
            .flatten();
        let moving = (!geometry.sources.is_empty())
            .then(|| {
                let count = geometry.sources.len() as u32;
                let vertices = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("rt moving"),
                    size: count as u64 * 12,
                    usage: wgpu::BufferUsages::BLAS_INPUT | wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                });
                let previous = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("rt moving before"),
                    size: count as u64 * 12,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let built = Built::new(device, "rt moving", vertices, count, [&geometry.moving[0], &geometry.moving[1]], true)?;
                let sources = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("rt moving sources"),
                    contents: bytemuck::cast_slice(&geometry.sources),
                    usage: wgpu::BufferUsages::STORAGE,
                });
                let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("rt pose"),
                    source: wgpu::ShaderSource::Wgsl(include_str!("rt_pose.wgsl").into()),
                });
                let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("rt pose"),
                    layout: None,
                    module: &shader,
                    entry_point: Some("main"),
                    compilation_options: Default::default(),
                    cache: None,
                });
                Some(Moving {
                    built,
                    previous,
                    sources,
                    count,
                    pipeline,
                    group: None,
                })
            })
            .flatten();
        let mut tlas = device.create_tlas(&wgpu::CreateTlasDescriptor {
            label: Some("rt scene"),
            max_instances: 3,
            flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
            update_mode: wgpu::AccelerationStructureUpdateMode::Build,
        });
        // The instances one after another, so each one's index is known:
        // the terrain, the still objects, the moving ones.
        let still_at = 1;
        let moving_at = still_at + still.is_some() as usize;
        tlas[0] = Some(wgpu::TlasInstance::new(&terrain.blas, IDENTITY, 0, TERRAIN));
        if let Some(b) = &still {
            tlas[still_at] = Some(wgpu::TlasInstance::new(&b.blas, IDENTITY, 0, STILL));
        }
        if let Some(m) = &moving {
            // The moving cut-outs' records follow the still ones'.
            let first = geometry.still[1].len() as u32 / 3;
            tlas[moving_at] = Some(wgpu::TlasInstance::new(&m.built.blas, IDENTITY, first, MOVING));
        }
        let mut records = geometry.cutouts.clone();
        if records.is_empty() {
            records.push([0.0; 8]);
        }
        let cutouts = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("rt cutouts"),
            contents: bytemuck::cast_slice(&records),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let masks = layers(device, queue, "rt masks", MASK_SIDE, 1, wgpu::TextureFormat::R8Unorm, &geometry.masks);
        let colours = layers(device, queue, "rt colours", COLOUR_SIDE, 4, wgpu::TextureFormat::Rgba8UnormSrgb, &geometry.colours);
        // Where each instance's parts start among the hits: instance i's
        // geometry g at head[2 i + g], as its parts were built (an empty
        // part is left out, so the next takes its place).
        let mut first = [0u32; 4];
        let mut at = 2;
        for (f, h) in first.iter_mut().zip(&geometry.hits) {
            *f = at;
            at += h.len() as u32;
        }
        // And in its last word, which instance moves.
        let mut head = [0u32; 8];
        head[7] = if moving.is_some() { moving_at as u32 } else { u32::MAX };
        for (instance, built, base) in [(still_at, still.as_ref(), 0), (moving_at, moving.as_ref().map(|m| &m.built), 2)] {
            for (g, part) in built.map_or(&[][..], |b| &b.parts[..]).iter().enumerate() {
                head[2 * instance + g] = first[base + part.which];
            }
        }
        let mut rows: Vec<[u32; 4]> = vec![[head[0], head[1], head[2], head[3]], [head[4], head[5], head[6], head[7]]];
        for h in &geometry.hits {
            rows.extend_from_slice(h);
        }
        let blank = |label: &str| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: &[0; 64],
                usage: wgpu::BufferUsages::STORAGE,
            })
        };
        let reflections = Reflections {
            hits: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("rt hits"),
                contents: bytemuck::cast_slice(&rows),
                usage: wgpu::BufferUsages::STORAGE,
            }),
            colours,
            ground,
            vertices: blank("rt no vertices"),
            nodes: blank("rt no nodes"),
        };
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("rt masks"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let module = |label: &str, source: &str| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            })
        };
        let compute = |label: &str, module: &wgpu::ShaderModule, entry: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: None,
                module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let volume = module("rt shafts", include_str!("rt_volume.wgsl"));
        let shadow = module("rt trace", &shadow_source());
        let passes = Passes {
            params: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("rt shadows"),
                size: std::mem::size_of::<Params>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            trace: compute("rt trace", &shadow, "trace"),
            probe: compute("rt probe", &shadow, "probe"),
            bake: compute("flood bake", &shadow, "bake"),
            blend: compute("rt blend", &module("rt blend", include_str!("rt_temporal.wgsl")), "blend"),
            blur: compute("rt blur", &module("rt blur", include_str!("rt_blur.wgsl")), "blur"),
            shafts: compute("rt shafts", &volume, "scatter"),
            settle: compute("rt shafts settle", &volume, "settle"),
            before: None,
            targets: None,
            blank: shadow_texture(device, (1, 1)),
            frame: 0,
        };
        let mut lamps: Vec<[f32; 8]> = floodlights.iter().map(|(at, spread, n)| [at.x, at.y, at.z, *spread, *n, 0.0, 0.0, 0.0]).collect();
        let floodlight_count = lamps.len() as u32;
        if lamps.is_empty() {
            lamps.push([0.0; 8]);
        }
        let floodlights = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("rt floodlights"),
            contents: bytemuck::cast_slice(&lamps),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let headlights = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rt headlights"),
            size: (MOST_HEADLIGHTS * 32) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Scene {
            headlights,
            floodlights,
            floodlight_count,
            passes,
            reflections,
            tlas,
            terrain,
            still,
            moving,
            cutouts,
            masks,
            sampler,
            built: false,
            flood: FloodBinds {
                heights: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("rt no flood"),
                    size: 16,
                    usage: wgpu::BufferUsages::STORAGE,
                    mapped_at_creation: false,
                }),
                uniform: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("rt no flood"),
                    size: 48,
                    usage: wgpu::BufferUsages::UNIFORM,
                    mapped_at_creation: false,
                }),
                map: shadow_texture(device, (1, 1)),
            },
        }
    }

    /// Whether the still parts are built, so rays can be traced through
    /// them outside the frame (the floodlights' bake).
    pub fn ready(&self) -> bool {
        self.built
    }

    /// Bakes the light poles' light over the ground (crate::flood), traced
    /// through all that does not move, and reads it from now on.
    pub fn bake_flood(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, flood: &mut crate::flood::FloodMap) {
        let p = &self.passes;
        let mut entries = vec![
            entry(2, wgpu::BindingResource::AccelerationStructure(&self.tlas)),
            entry(3, self.cutouts.as_entire_binding()),
            entry(4, wgpu::BindingResource::TextureView(&self.masks)),
            entry(5, wgpu::BindingResource::Sampler(&self.sampler)),
            entry(7, self.reflections.hits.as_entire_binding()),
            entry(8, self.reflections.vertices.as_entire_binding()),
            entry(9, self.reflections.nodes.as_entire_binding()),
        ];
        entries.extend(flood.entries());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("flood bake"),
            layout: &p.bake.get_bind_group_layout(0),
            entries: &entries,
        });
        flood.run(device, queue, &p.bake, &group);
        self.flood = FloodBinds {
            heights: flood.heights().clone(),
            uniform: flood.uniform().clone(),
            map: flood.map().clone(),
        };
        // The trace's groups made again with it.
        self.passes.targets = None;
    }

    /// Poses the moving models from the objects' `vertices` (made with
    /// STORAGE) and `nodes`.
    pub fn attach(&mut self, device: &wgpu::Device, vertices: &wgpu::Buffer, nodes: &wgpu::Buffer) {
        self.reflections.vertices = vertices.clone();
        self.reflections.nodes = nodes.clone();
        if let Some(m) = &mut self.moving {
            m.group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("rt pose"),
                layout: &m.pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: vertices.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: nodes.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: m.sources.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: m.built.vertices.as_entire_binding(),
                    },
                ],
            }));
        }
    }

    /// What the scene's shaders read for the shadows and reflections, from
    /// binding 8 of the shared group on (enhance::ray_traced).
    pub fn bindings(&self) -> Vec<wgpu::BindingResource<'_>> {
        use wgpu::BindingResource as R;
        let r = &self.reflections;
        let terrain = &self.terrain.parts[0].indices;
        vec![
            R::TextureView(self.shadows()),
            R::AccelerationStructure(&self.tlas),
            self.cutouts.as_entire_binding(),
            R::TextureView(&self.masks),
            R::Sampler(&self.sampler),
            r.vertices.as_entire_binding(),
            r.nodes.as_entire_binding(),
            r.hits.as_entire_binding(),
            R::TextureView(&r.colours),
            R::TextureView(&r.ground),
            self.terrain.vertices.as_entire_binding(),
            terrain.as_entire_binding(),
            R::TextureView(self.passes.targets.as_ref().map_or(&self.passes.blank, |t| &t.lamps)),
        ]
    }

    /// Whether a domed stadium's floodlights were found to light it.
    pub fn has_floodlights(&self) -> bool {
        self.floodlight_count > 0
    }

    /// The traced shadows, for the scene's shaders.
    pub fn shadows(&self) -> &wgpu::TextureView {
        self.passes.targets.as_ref().map_or(&self.passes.blank, |t| &t.soft)
    }

    /// Makes the textures for a scene of `size`, if they are not that
    /// size already. Returns whether they are new, so the shared group
    /// must be made again with [`Scene::shadows`].
    pub fn resize(&mut self, device: &wgpu::Device, size: (u32, u32)) -> bool {
        if self.passes.targets.as_ref().is_some_and(|t| t.size == size) {
            return false;
        }
        let depth = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("rt depth"),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let raw = shadow_texture(device, size);
        let soft = shadow_texture(device, size);
        let lamps = shadow_texture(device, size);
        // Where each pixel's surface was on screen a frame ago, for those on
        // moving models: the pixels' move, and how far it was then.
        let motion = shadow_texture(device, size);
        let blank = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rt no moving"),
            size: 64,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let (before, parts) = match &self.moving {
            Some(m) => {
                let first = &m.built.parts[0].indices;
                (m.previous.clone(), [first.clone(), m.built.parts.get(1).map_or(first.clone(), |p| p.indices.clone())])
            }
            None => (blank.clone(), [blank.clone(), blank.clone()]),
        };
        let p = &self.passes;
        let histories = [shadow_texture(device, size), shadow_texture(device, size)];
        let probe_size = (size.0.div_ceil(2), size.1.div_ceil(2));
        let probes = shadow_texture(device, probe_size);
        let probe_lamps = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("rt probe lamps"),
                size: wgpu::Extent3d {
                    width: probe_size.0,
                    height: probe_size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba32Uint,
                usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        // With the frame before's settled shadows, as the trace's.
        let probe_groups = [0usize, 1].map(|w| device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rt probe"),
            layout: &p.probe.get_bind_group_layout(0),
            entries: &[
                entry(0, p.params.as_entire_binding()),
                entry(1, wgpu::BindingResource::TextureView(&depth)),
                entry(2, wgpu::BindingResource::AccelerationStructure(&self.tlas)),
                entry(3, self.cutouts.as_entire_binding()),
                entry(4, wgpu::BindingResource::TextureView(&self.masks)),
                entry(5, wgpu::BindingResource::Sampler(&self.sampler)),
                entry(7, self.reflections.hits.as_entire_binding()),
                entry(8, self.reflections.vertices.as_entire_binding()),
                entry(9, self.reflections.nodes.as_entire_binding()),
                entry(15, self.headlights.as_entire_binding()),
                entry(17, wgpu::BindingResource::TextureView(&histories[1 - w])),
                entry(18, wgpu::BindingResource::TextureView(&probes)),
                entry(20, wgpu::BindingResource::TextureView(&probe_lamps)),
                entry(30, self.flood.heights.as_entire_binding()),
                entry(31, self.flood.uniform.as_entire_binding()),
                entry(34, wgpu::BindingResource::TextureView(&self.flood.map)),
            ],
        }));
        // Tracing the frame that writes history `w`, with the other (the
        // frame before's settled shadows) to tell how many rays a pixel
        // needs.
        let trace_groups = [0usize, 1].map(|w| device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rt trace"),
            layout: &p.trace.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: p.params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&depth),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::AccelerationStructure(&self.tlas),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.cutouts.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&self.masks),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&raw),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: self.reflections.hits.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: self.reflections.vertices.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: self.reflections.nodes.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: before.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: parts[0].as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: parts[1].as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 13,
                    resource: wgpu::BindingResource::TextureView(&motion),
                },
                wgpu::BindGroupEntry {
                    binding: 14,
                    resource: self.floodlights.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 15,
                    resource: self.headlights.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 16,
                    resource: wgpu::BindingResource::TextureView(&lamps),
                },
                wgpu::BindGroupEntry {
                    binding: 17,
                    resource: wgpu::BindingResource::TextureView(&histories[1 - w]),
                },
                entry(19, wgpu::BindingResource::TextureView(&probes)),
                entry(21, wgpu::BindingResource::TextureView(&probe_lamps)),
                entry(30, self.flood.heights.as_entire_binding()),
                entry(31, self.flood.uniform.as_entire_binding()),
                entry(34, wgpu::BindingResource::TextureView(&self.flood.map)),
            ],
        }));
        // Writing history `w`, reading the other; for the shadows and the
        // shafts alike.
        let blend = |raw: &wgpu::TextureView, histories: &[wgpu::TextureView; 2], w: usize| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("rt blend"),
                layout: &p.blend.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: p.params.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(raw),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&histories[1 - w]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&depth),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(&histories[w]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::TextureView(&motion),
                    },
                ],
            })
        };
        let blur = |histories: &[wgpu::TextureView; 2], soft: &wgpu::TextureView, w: usize| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("rt blur"),
                layout: &p.blur.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: p.params.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&histories[w]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(soft),
                    },
                ],
            })
        };
        let blend_groups = [0, 1].map(|w| blend(&raw, &histories, w));
        let blur_groups = [0, 1].map(|w| blur(&histories, &soft, w));
        let shaft_size = (size.0.div_ceil(2), size.1.div_ceil(2));
        let shaft_raw = shadow_texture(device, shaft_size);
        let shaft_histories = [shadow_texture(device, shaft_size), shadow_texture(device, shaft_size)];
        let shafts = shadow_texture(device, shaft_size);
        let shaft_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rt shafts"),
            layout: &p.shafts.get_bind_group_layout(0),
            entries: &[
                entry(0, p.params.as_entire_binding()),
                entry(1, wgpu::BindingResource::TextureView(&depth)),
                entry(2, wgpu::BindingResource::AccelerationStructure(&self.tlas)),
                entry(3, self.cutouts.as_entire_binding()),
                entry(4, wgpu::BindingResource::TextureView(&self.masks)),
                entry(5, wgpu::BindingResource::Sampler(&self.sampler)),
                entry(7, wgpu::BindingResource::TextureView(&shaft_raw)),
            ],
        });
        // Writing history `w`, reading the other.
        let shaft_settle_groups = [0, 1].map(|w| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("rt shafts settle"),
                layout: &p.settle.get_bind_group_layout(0),
                entries: &[
                    entry(0, p.params.as_entire_binding()),
                    entry(1, wgpu::BindingResource::TextureView(&depth)),
                    entry(8, wgpu::BindingResource::TextureView(&shaft_raw)),
                    entry(9, wgpu::BindingResource::TextureView(&shaft_histories[1 - w])),
                    entry(10, wgpu::BindingResource::TextureView(&shaft_histories[w])),
                    entry(11, wgpu::BindingResource::TextureView(&shafts)),
                ],
            })
        });
        self.passes.targets = Some(Targets {
            lamps,
            shaft_group,
            shaft_settle_groups,
            shaft_size,
            shafts,
            size,
            depth,
            soft,
            trace_groups,
            probe_groups,
            probe_size,
            blend_groups,
            blur_groups,
        });
        // New textures hold no frame before.
        self.passes.before = None;
        true
    }

    /// The ray-traced light shafts (their red: the sunlight the air sends
    /// toward the eye), once [`Scene::resize`]d.
    pub fn shafts(&self) -> Option<&wgpu::TextureView> {
        self.passes.targets.as_ref().map(|t| &t.shafts)
    }

    /// The depth pass's target, once [`Scene::resize`]d.
    pub fn depth(&self) -> Option<&wgpu::TextureView> {
        self.passes.targets.as_ref().map(|t| &t.depth)
    }

    /// Traces the shadows of what the depth pass drew, seen through
    /// `view_proj` (with a field of view `fovy`) from `camera`, lit from
    /// `to_sun`; `mask` picks what casts.
    #[allow(clippy::too_many_arguments)]
    pub fn trace(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view_proj: glam::Mat4,
        camera: glam::Vec3,
        to_sun: glam::Vec3,
        mask: u8,
        fovy: f32,
        shafts: bool,
        indoors: bool,
        headlights: &[(glam::Vec3, glam::Vec3)],
        poles: &[(glam::Vec3, glam::Vec3)],
        night: f32,
        mut timer: Option<&mut crate::gpu_time::GpuTimer>,
    ) {
        // w 0 for a headlight, 1 for a light pole (shining down).
        let lamps: Vec<[f32; 8]> = headlights
            .iter()
            .map(|(at, way)| [at.x, at.y, at.z, 0.0, way.x, way.y, way.z, 0.0])
            .chain(poles.iter().map(|(at, facing)| {
                // Down onto the track, which lies behind the way the pole
                // was placed facing.
                let way = pole_way(*facing);
                [at.x, at.y, at.z, 1.0, way.x, way.y, way.z, 0.0]
            }))
            .take(MOST_HEADLIGHTS)
            .collect();
        if !lamps.is_empty() {
            queue.write_buffer(&self.headlights, 0, bytemuck::cast_slice(&lamps));
        }
        let p = &mut self.passes;
        let Some(t) = &p.targets else { return };
        p.frame = p.frame.wrapping_add(1);
        let (previous, history) = match p.before {
            Some((m, eye)) => (m, [1.0, eye.x, eye.y, eye.z]),
            None => (glam::Mat4::IDENTITY, [0.0; 4]),
        };
        let params = Params {
            inverse_view_proj: view_proj.inverse().to_cols_array_2d(),
            camera: camera.extend(1.0).into(),
            to_sun: to_sun.extend(mask as f32).into(),
            size: [t.size.0 as f32, t.size.1 as f32, (p.frame % 64) as f32, 2.0 * (fovy / 2.0).tan() / t.size.1 as f32],
            previous: previous.to_cols_array_2d(),
            history,
            lights: [if indoors { self.floodlight_count as f32 } else { 0.0 }, lamps.len() as f32, night, 0.0],
        };
        p.before = Some((view_proj, camera));
        let w = (p.frame % 2) as usize;
        queue.write_buffer(&p.params, 0, bytemuck::bytes_of(&params));
        let groups = (t.size.0.div_ceil(8), t.size.1.div_ceil(8));
        // Each step in a pass of its own, so each can be timed.
        let mut step = |encoder: &mut wgpu::CommandEncoder, pipeline: &wgpu::ComputePipeline, group: &wgpu::BindGroup, size: (u32, u32), label: &'static str| {
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some(label), timestamp_writes: None });
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, group, &[]);
                pass.dispatch_workgroups(size.0, size.1, 1);
            }
            if let Some(timer) = timer.as_mut() {
                timer.mark(encoder, label);
            }
        };
        step(encoder, &p.probe, &t.probe_groups[w], (t.probe_size.0.div_ceil(8), t.probe_size.1.div_ceil(8)), "rt: shadow probes");
        step(encoder, &p.trace, &t.trace_groups[w], groups, "rt: shadow rays");
        if shafts {
            let half = (t.shaft_size.0.div_ceil(8), t.shaft_size.1.div_ceil(8));
            step(encoder, &p.shafts, &t.shaft_group, half, "rt: shaft rays");
            step(encoder, &p.settle, &t.shaft_settle_groups[w], half, "rt: shaft blend");
        }
        step(encoder, &p.blend, &t.blend_groups[w], groups, "rt: shadow blend");
        step(encoder, &p.blur, &t.blur_groups[w], groups, "rt: shadow blur");

    }

    /// Builds what has changed: the still parts the first time, the moving
    /// models every time, and the scene over them.
    pub fn update(&mut self, encoder: &mut wgpu::CommandEncoder, mut timer: Option<&mut crate::gpu_time::GpuTimer>) {
        let mut entries = Vec::new();
        if !self.built {
            entries.push(self.terrain.entry());
            if let Some(b) = &self.still {
                entries.push(b.entry());
            }
        }
        let moving = match &self.moving {
            Some(Moving { group: Some(group), pipeline, count, built, previous, .. }) => {
                // Kept before they are posed again.
                encoder.copy_buffer_to_buffer(&built.vertices, 0, previous, 0, *count as u64 * 12);
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("rt pose"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, group, &[]);
                pass.dispatch_workgroups(count.div_ceil(64), 1, 1);
                drop(pass);
                if let Some(t) = timer.as_mut() {
                    t.mark(encoder, "rt: pose");
                }
                entries.push(built.entry());
                true
            }
            _ => false,
        };
        if !self.built || moving {
            encoder.build_acceleration_structures(entries.iter(), std::iter::once(&self.tlas));
        }
        self.built = true;
    }
}

/// A texture array of `data`'s layers, each `side` texels a side of
/// `bytes` bytes, with its mips.
fn layers(device: &wgpu::Device, queue: &wgpu::Queue, label: &str, side: u32, bytes: u32, format: wgpu::TextureFormat, data: &[Vec<u8>]) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: data.len().max(1) as u32,
        },
        mip_level_count: side.ilog2() + 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (layer, top) in data.iter().enumerate() {
        let mut level = top.clone();
        let mut size = side;
        for mip in 0..texture.mip_level_count() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: mip,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: layer as u32 },
                    aspect: wgpu::TextureAspect::All,
                },
                &level,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(size * bytes),
                    rows_per_image: Some(size),
                },
                wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
            );
            (level, size) = (halve(&level, size, bytes), (size / 2).max(1));
        }
    }
    texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    })
}

/// The ground's colour, [`GROUND_TEXELS`] a tile, over the whole terrain:
/// each tile's texture as the game paints it (renderer.rs's tile map).
fn ground_map(device: &wgpu::Device, queue: &wgpu::Queue, terrain: &Terrain, textures: &[crate::hires::Image]) -> wgpu::TextureView {
    let tile = terrain.tiles.first().map_or(64, |t| t.size);
    let tiles = (terrain.size as u32 - 1).div_ceil(tile).max(1);
    let side = tiles * GROUND_TEXELS;
    // Where there is no tile, the game's own grey-brown.
    let mut rgba = [115u8, 107, 92, 255].repeat((side * side) as usize);
    for t in &terrain.tiles {
        let (tx, tz) = (t.x / tile, t.z / tile);
        let Some(img) = textures.get(t.texture as usize) else { continue };
        let Some(top) = img.levels.first() else { continue };
        if tx >= tiles || tz >= tiles {
            continue;
        }
        let small = colours_sized(top, img.size, GROUND_TEXELS);
        for y in 0..GROUND_TEXELS {
            let row = ((tz * GROUND_TEXELS + y) * side + tx * GROUND_TEXELS) as usize * 4;
            let from = (y * GROUND_TEXELS) as usize * 4;
            rgba[row..row + GROUND_TEXELS as usize * 4].copy_from_slice(&small[from..from + GROUND_TEXELS as usize * 4]);
        }
    }
    use wgpu::util::DeviceExt as _;
    device
        .create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("rt ground"),
                size: wgpu::Extent3d {
                    width: side,
                    height: side,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &rgba,
        )
        .create_view(&Default::default())
}

/// A texture's colours `rgba` (`side` a side), sampled to [`COLOUR_SIDE`].
pub fn colours(rgba: &[u8], side: u32) -> Vec<u8> {
    colours_sized(rgba, side, COLOUR_SIDE)
}

/// `rgba` (`side` a side) sampled to `to` a side, each texel the average
/// of those it covers, the clear ones left out so a cut-out's edge keeps
/// its colour.
fn colours_sized(rgba: &[u8], side: u32, to: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((to * to * 4) as usize);
    for y in 0..to {
        for x in 0..to {
            let (x0, y0) = (x * side / to, y * side / to);
            let (x1, y1) = ((((x + 1) * side).div_ceil(to)).max(x0 + 1).min(side), (((y + 1) * side).div_ceil(to)).max(y0 + 1).min(side));
            let (mut sum, mut count) = ([0u32; 3], 0u32);
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let p = &rgba[((sy * side + sx) * 4) as usize..][..4];
                    if p[3] >= 128 || count == 0 && sy + 1 == y1 && sx + 1 == x1 {
                        for c in 0..3 {
                            sum[c] += p[c] as u32;
                        }
                        count += 1;
                    }
                }
            }
            let c = count.max(1);
            out.extend_from_slice(&[(sum[0] / c) as u8, (sum[1] / c) as u8, (sum[2] / c) as u8, 255]);
        }
    }
    out
}

/// A bind group entry.
fn entry(binding: u32, resource: wgpu::BindingResource<'_>) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry { binding, resource }
}

/// A texture the shadows are traced in, `size` pixels.
fn shadow_texture(device: &wgpu::Device, size: (u32, u32)) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("rt shadows"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SHADOW_FORMAT,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

/// The mask of a cut-out texture `rgba` (`side` a side, top row first):
/// its alpha, sampled to [`MASK_SIDE`] a side.
pub fn mask(rgba: &[u8], side: u32) -> Vec<u8> {
    let mut out = vec![0u8; (MASK_SIDE * MASK_SIDE) as usize];
    for y in 0..MASK_SIDE {
        for x in 0..MASK_SIDE {
            // As many of the texture's texels as fall in this one.
            let (x0, x1) = (x * side / MASK_SIDE, ((x + 1) * side).div_ceil(MASK_SIDE).max(x * side / MASK_SIDE + 1));
            let (y0, y1) = (y * side / MASK_SIDE, ((y + 1) * side).div_ceil(MASK_SIDE).max(y * side / MASK_SIDE + 1));
            let (mut sum, mut count) = (0u32, 0u32);
            for sy in y0..y1.min(side) {
                for sx in x0..x1.min(side) {
                    sum += rgba[((sy * side + sx) * 4 + 3) as usize] as u32;
                    count += 1;
                }
            }
            out[(y * MASK_SIDE + x) as usize] = (sum / count.max(1)) as u8;
        }
    }
    out
}

/// How much of a cut-out mask a triangle covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cover {
    /// Only clear texels: it casts nothing.
    Clear,
    /// Only solid ones: it casts like any solid triangle.
    Solid,
    /// Some of each: the rays look at the mask where they meet it.
    Mixed,
}

/// Cells a side of [`Pooled`]'s grid.
const POOL: u32 = 64;

/// A mask's lowest and highest alpha in each cell of a [`POOL`] grid, to
/// tell quickly how much of it a triangle covers.
pub struct Pooled(Vec<(u8, u8)>);

impl Pooled {
    pub fn new(mask: &[u8]) -> Pooled {
        let k = MASK_SIDE / POOL;
        let mut cells = vec![(255u8, 0u8); (POOL * POOL) as usize];
        for y in 0..MASK_SIDE {
            for x in 0..MASK_SIDE {
                let a = mask[(y * MASK_SIDE + x) as usize];
                let c = &mut cells[((y / k) * POOL + x / k) as usize];
                *c = (c.0.min(a), c.1.max(a));
            }
        }
        Pooled(cells)
    }

    /// How much of the mask the triangle with texture coordinates `uv`
    /// covers: every cell it touches looked at, so a thin solid strand in
    /// a clear cell still counts.
    pub fn cover(&self, uv: [[f32; 2]; 3]) -> Cover {
        let p = uv.map(|t| glam::Vec2::new(t[0], t[1]) * POOL as f32);
        let low = p[0].min(p[1]).min(p[2]).floor();
        let high = p[0].max(p[1]).max(p[2]).ceil();
        if (high - low).max_element() > POOL as f32 || !low.is_finite() || !high.is_finite() {
            return Cover::Mixed;
        }
        let twice = (p[1] - p[0]).perp_dot(p[2] - p[0]);
        if twice.abs() < 1e-6 {
            return Cover::Mixed;
        }
        // Each edge's distance, in cells, signed inward.
        let edge = |a: glam::Vec2, b: glam::Vec2, q: glam::Vec2| (b - a).perp_dot(q - a) * twice.signum() / (b - a).length().max(1e-6);
        let (mut solid, mut clear) = (false, false);
        for y in low.y as i32..high.y as i32 {
            for x in low.x as i32..high.x as i32 {
                let centre = glam::Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                // Within half a diagonal of the triangle: it may touch the cell.
                if [edge(p[0], p[1], centre), edge(p[1], p[2], centre), edge(p[2], p[0], centre)].iter().any(|&d| d < -0.71) {
                    continue;
                }
                let at = (y.rem_euclid(POOL as i32) as u32 * POOL + x.rem_euclid(POOL as i32) as u32) as usize;
                let (min, max) = self.0[at];
                solid |= max >= 128;
                clear |= min < 128;
                if solid && clear {
                    return Cover::Mixed;
                }
            }
        }
        match (solid, clear) {
            (true, false) => Cover::Solid,
            (false, _) => Cover::Clear,
            _ => Cover::Mixed,
        }
    }
}

/// An image `side` a side of `bytes` channels, half as big: each texel
/// the average of four.
fn halve(m: &[u8], side: u32, bytes: u32) -> Vec<u8> {
    let half = (side / 2).max(1);
    let at = |x: u32, y: u32, c: u32| m[((y.min(side - 1) * side + x.min(side - 1)) * bytes + c) as usize] as u32;
    let mut out = Vec::with_capacity((half * half * bytes) as usize);
    for y in 0..half {
        for x in 0..half {
            for c in 0..bytes {
                out.push(((at(2 * x, 2 * y, c) + at(2 * x + 1, 2 * y, c) + at(2 * x, 2 * y + 1, c) + at(2 * x + 1, 2 * y + 1, c) + 2) / 4) as u8);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn mask_keeps_the_cut_out() {
        // A 2×2 texture: solid on the left, clear on the right.
        let rgba = [0, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0, 255, 0, 0, 0, 0];
        let m = super::mask(&rgba, 2);
        let side = super::MASK_SIDE as usize;
        assert_eq!(m[0], 255);
        assert_eq!(m[side - 1], 0);
        assert_eq!(m[(side - 1) * side], 255);
        assert_eq!(super::halve(&[255, 0, 255, 0], 2, 1), [128]);
        let pooled = super::Pooled::new(&m);
        assert_eq!(pooled.cover([[0.0, 0.0], [0.3, 0.0], [0.0, 0.3]]), super::Cover::Solid);
        assert_eq!(pooled.cover([[0.7, 0.0], [1.0, 0.0], [1.0, 0.3]]), super::Cover::Clear);
        assert_eq!(pooled.cover([[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]), super::Cover::Mixed);
    }
}
