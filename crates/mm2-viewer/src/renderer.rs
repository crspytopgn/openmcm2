//! Draws a track's terrain and objects with wgpu.

use crate::hires::Image;
use crate::objects::{ObjectMesh, ObjectRenderer};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use mm2_formats::trn::Terrain;
use wgpu::util::DeviceExt;

/// With a stencil, which the player's shadow uses so that it darkens
/// each pixel once.
pub(crate) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;

pub const SKY: wgpu::Color = wgpu::Color {
    r: 0.36,
    g: 0.5,
    b: 0.74,
    a: 1.0,
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    height: f32,
    normal: [i8; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct Globals {
    pub view_proj: [[f32; 4]; 4],
    pub camera: [f32; 4],
    pub grid: [f32; 4],
    /// The fog's colour (linear) and its start and end, ft; the end is
    /// the far plane too.
    pub fog: [f32; 4],
    pub fog_range: [f32; 4],
    /// The scene's lights, [`SceneLights`].
    pub ambient: [f32; 4],
    pub sun: [f32; 4],
    pub to_sun: [f32; 4],
    /// The player's shadow: from the drawing to its mask's texture, and
    /// whether it falls (x).
    pub shadow_view: [[f32; 4]; 4],
    pub shadow_on: [f32; 4],
}

/// The ground's blocks are this many cells a side.
const BLOCK: u32 = 32;
/// The shadow's mask, as 0x4defa0 makes it: 128 texels a side.
const SHADOW_SIZE: u32 = 128;
/// Half the side of the square the shadow's camera sees round the bike
/// and rider, ft, and how far back toward the sun it stands.
const SHADOW_RADIUS: f32 = 4.5;
const SHADOW_REACH: f32 = 100.0;

/// The scene's two lights as the game lights each vertex with them
/// (0x4a0e40, from `[Lights]` via 0x4f0300): the ambient colour, plus the
/// sun's colour times how squarely the vertex faces it, each 0..=255 and
/// clamped there. Colours are 0..1 of the game's 0..255.
#[derive(Debug, Clone, Copy)]
pub struct SceneLights {
    pub ambient: [f32; 3],
    pub sun: [f32; 3],
    /// The way to the sun, in viewer coordinates.
    pub to_sun: Vec3,
    /// How much the terrain is lit by them in the original look, 0 to 1.
    /// The game's own tracks have their light painted into the ground's
    /// textures, so 0; the Free Pack's tracks and the Course Builder's
    /// don't (`LightGround=1` in the scene), so their hills are shaded
    /// as they face the sun.
    pub light_ground: f32,
}

impl SceneLights {
    /// From a scene's `[Lights]`. The game's lighting takes one ambient
    /// light and one other; a scene without lights gets the game's
    /// defaults (0x4f0300): grey 90 ambient and a 200 grey sun placed at
    /// infinity towards (−2000, 4000, 1252).
    pub fn from_scene(lights: &[mm2_formats::scn::Light]) -> SceneLights {
        use mm2_formats::scn::LightKind;
        let colour = |c: [f32; 3]| c.map(|v| v.clamp(0.0, 255.0) / 255.0);
        let ambient = lights
            .iter()
            .find(|l| l.kind == LightKind::Ambient)
            .map_or([90.0 / 255.0; 3], |l| colour(l.color));
        let sun = lights.iter().find_map(|l| {
            // A directional light (type 4) shines along its look vector;
            // a point at infinity (type 2) from where it is, towards each
            // object, which the track's size makes one direction.
            let to = match l.kind {
                LightKind::Directional => -Vec3::from(l.direction?),
                LightKind::Ambient => return None,
                _ => Vec3::from(l.position?),
            };
            Some((colour(l.color), to))
        });
        let (sun, to) = sun.unwrap_or(([200.0 / 255.0; 3], Vec3::new(-2000.0, 4000.0, 1252.0)));
        SceneLights {
            ambient,
            sun,
            to_sun: from_game(to).normalize_or(Vec3::Y),
            light_ground: 0.0,
        }
    }
}

impl Default for SceneLights {
    /// The port's own light, for views without a scene (the menus).
    fn default() -> SceneLights {
        SceneLights {
            ambient: [0.5; 3],
            sun: [0.6; 3],
            to_sun: Vec3::new(0.4, 0.8, 0.3).normalize(),
            light_ground: 0.0,
        }
    }
}

impl SceneLights {
    pub(crate) fn globals(&self) -> ([f32; 4], [f32; 4], [f32; 4]) {
        let [a, b, c] = self.ambient;
        let [r, g, bl] = self.sun;
        (
            [a, b, c, 0.0],
            [r, g, bl, 0.0],
            self.to_sun.extend(0.0).into(),
        )
    }
}

pub struct TerrainRenderer {
    pipeline: wgpu::RenderPipeline,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    /// The ground's blocks: their bounds (drawing coordinates) and indices.
    blocks: Vec<(Vec3, Vec3, std::ops::Range<u32>)>,
    /// The ground's heights, to tell what a hill hides (crate::cull).
    heightfield: crate::cull::Heightfield,
    /// The runs of indices in view this frame.
    runs: Vec<std::ops::Range<u32>>,
    /// The blocks in view by distance, kept between frames for its room.
    nearest: Vec<(f32, std::ops::Range<u32>, bool)>,
    /// The same runs for the enhanced drawing, each with whether its
    /// blocks reach under the smooth ground round the eye, where the
    /// game's mesh gives way to it (terrain.wgsl fs_enhanced); the rest
    /// are drawn with a shader that never discards, so the GPU can test
    /// their depth before shading them.
    open_runs: Vec<(std::ops::Range<u32>, bool)>,
    globals: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// The terrain's textures, tile map and sampler, kept to rebind with
    /// a detail texture.
    terrain_views: (wgpu::TextureView, wgpu::TextureView, wgpu::Sampler),
    /// The player's shadow's mask, as the sun sees the bike and rider.
    shadow_mask: wgpu::TextureView,
    detail: wgpu::TextureView,
    /// The free pack's fine ground (fine_ground.rs), or none.
    ground: crate::fine_ground::Views,
    grid: [f32; 4],
    depth: Option<(wgpu::TextureView, u32, u32)>,
    /// The depth texture, destroyed when the next takes its place.
    depth_held: crate::gpu_free::Held,
    objects: Option<ObjectRenderer>,
    sky: Option<crate::sky::SkyRenderer>,
    clear: wgpu::Color,
    /// The fog: its colour, and where it starts and ends (the far plane);
    /// a scene without a sky box has none (0x4e2d70).
    fog: [f32; 3],
    fog_range: (f32, f32),
    /// The original's trees at the Ecosystem detail ([`Self::set_ecosystem`]).
    trees: crate::objects::TreeBand,
    lights: SceneLights,
    /// The dirt the bikes throw up.
    pub particles: crate::particles::Particles,
    /// The Sky and Particles switches (F8, F9): off, the sky is the
    /// clear colour and no dirt is drawn.
    pub sky_on: bool,
    pub particles_on: bool,
    /// The Shadows switch (F10).
    pub shadows_on: bool,
    shadow: Option<([usize; 2], Vec3, Vec3)>,
    shadow_casters: Option<[usize; 2]>,
    /// The models that cast no sun shadow, and whether the objects know.
    shadowless: (Vec<usize>, bool),
    /// A desert track, for the enhanced drawing's weather.
    desert: bool,
    /// A stadium track (Supercross).
    stadium: bool,
    /// The scene's own sky box, shrunk, for the sky's light all round
    /// (crate::ibl); none for a stadium.
    env_painted: Option<crate::ibl::Source>,
    /// What the sky's light was last baked from and for, its harmonics,
    /// and its roughest level.
    env_baked: Option<(bool, crate::ibl::Light, [[f32; 4]; 9])>,
    /// In a replay, the distance (ft) to the rider the camera follows: the
    /// enhanced drawing then blurs what is out of focus and what the
    /// camera swings past.
    pub cinematic: Option<f32>,
    /// The last frame's view and projection, for that motion blur.
    last_view_proj: Option<Mat4>,
    /// Temporal anti-aliasing: frames drawn, for each one's shift, and
    /// the last one's view unshifted, the camera's place and way.
    taa_frame: u32,
    taa_last: Option<(Mat4, Vec3, Vec3)>,
    /// What the scene is drawn in: the target's format, or HDR with the
    /// enhanced drawing; and the samples a pixel.
    scene_format: wgpu::TextureFormat,
    samples: u32,
    /// The last frame's size.
    size: (u32, u32),
    /// The enhanced drawing ([`crate::graphics`]), when on.
    enhanced: Option<Enhanced>,
    /// Each bike's headlight this frame: where it is and the way it
    /// shines (drawing coordinates).
    headlights: Vec<(Vec3, Vec3)>,
    /// The track's light poles' lamp heads (drawing coordinates).
    pole_lamps: Vec<(Vec3, Vec3)>,
    /// Their light over the ground at night, baked the first night frame
    /// (crate::flood).
    flood: Option<crate::flood::FloodMap>,
    /// The sky's light baked over the land (crate::skylight): samples a
    /// side, ft between them, the lowest height and the range over them.
    sky_light: Option<[f32; 4]>,
    /// Each part of the frame timed on the GPU, when asked (gpu_time).
    gpu_timer: Option<crate::gpu_time::GpuTimer>,
    /// A stadium's own lights at night (crate::night): those found on its
    /// roof or ringed round it, and those its `.lights` file lists, which
    /// light any track at night.
    night_found: Option<crate::night::StadiumLights>,
    night_listed: Option<crate::night::StadiumLights>,
    /// A domed stadium's lamp banks painted on its roof (ObjectMesh::floodlights).
    night_banks: Vec<(Vec3, f32, f32)>,
    /// Rivers' water (crate::water), on tracks that have them.
    water: Option<crate::water::WaterRenderer>,
}

/// A stadium's lamps at night where its track lists none: its roof's
/// painted banks, or a ring round the floor.
fn found_lamps(banks: &[(Vec3, f32, f32)], floor: &crate::night::Floor) -> Option<crate::night::StadiumLights> {
    let lamps = crate::night::from_banks(banks, floor);
    crate::night::StadiumLights::new(if lamps.is_empty() { crate::night::ring(floor) } else { lamps }, floor)
}

/// The terrain renderer's part of the enhanced drawing.
struct Enhanced {
    shared: crate::enhance::Shared,
    terrain: (wgpu::RenderPipeline, wgpu::RenderPipeline),
    /// The terrain's own group for it, the same as the game's drawing's.
    layout: wgpu::BindGroupLayout,
    group: wgpu::BindGroup,
    grass: Option<crate::grass::GrassRenderer>,
    smooth: Option<crate::smooth::SmoothGround>,
    crowd: Option<crate::crowd::Crowd>,
    /// Each bike's place last seen on the ground (drawing coordinates),
    /// for the crowd.
    bikes: Vec<Option<Vec3>>,
    ruts: Option<crate::ruts::RutPainter>,
    dust: Option<crate::dust::DustRenderer>,
    rain: Option<crate::rain::RainRenderer>,
    /// How dirty each bike and its rider are, 0 to 1, and where the bike
    /// was last frame (drawing coordinates).
    mud: Vec<(f32, Option<Vec3>)>,
    /// Dirt building up on each bike and rider through the race, and the
    /// ground's colours it takes (the "grime" switch, in place of `mud`).
    grime: Option<(crate::grime::Grime, crate::grime::GroundColours)>,
    post: crate::post::Post,
    /// The scene for the ray-traced shadows, when they are drawn, and the
    /// terrain's pipeline for their depth pass.
    rt: Option<crate::rt::Scene>,
    rt_depth: Option<wgpu::RenderPipeline>,
}

/// Marks the end of the part `$label` of the frame, between passes, when
/// the parts are timed (gpu_time).
macro_rules! mark {
    ($timer:expr, $encoder:expr, $label:expr) => {
        if let Some(t) = $timer.as_mut() {
            t.mark($encoder, $label);
        }
    };
}

/// The same inside a render pass.
macro_rules! mark_pass {
    ($timer:expr, $pass:expr, $label:expr) => {
        if let Some(t) = $timer.as_mut() {
            t.mark_pass($pass, $label);
        }
    };
}

impl TerrainRenderer {
    /// Prints how long each part of the frame took on the GPU, when timed
    /// (`MM2_GPU_TIMES`).
    pub fn report_gpu_times(&mut self, device: &wgpu::Device) {
        if let Some(t) = &mut self.gpu_timer {
            t.report(device);
        }
    }

    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        terrain: &Terrain,
        textures: &[Image],
        objects: &ObjectMesh,
        format: wgpu::TextureFormat,
    ) -> Self {
        let n = terrain.size;
        let vertices: Vec<Vertex> = terrain
            .heights
            .iter()
            .zip(&terrain.normals)
            .map(|(&height, n)| Vertex {
                height,
                normal: [n[0], n[1], n[2], 0],
            })
            .collect();
        // In blocks of BLOCK cells a side, each block's triangles together,
        // so the blocks out of view can be left out (crate::cull).
        let mut indices = Vec::with_capacity((n - 1) * (n - 1) * 6);
        let mut blocks = Vec::new();
        let cells = n as u32 - 1;
        for bz in (0..cells).step_by(BLOCK as usize) {
            for bx in (0..cells).step_by(BLOCK as usize) {
                let start = indices.len() as u32;
                let (mut low, mut high) = (f32::MAX, f32::MIN);
                for z in bz..(bz + BLOCK).min(cells) {
                    for x in bx..(bx + BLOCK).min(cells) {
                        let i = z * n as u32 + x;
                        let below = i + n as u32;
                        for v in [i, i + 1, below, below + 1] {
                            low = low.min(terrain.heights[v as usize]);
                            high = high.max(terrain.heights[v as usize]);
                        }
                        // Split as the ground is sampled (ride::ground).
                        if (x ^ z) & 1 == 0 {
                            indices.extend_from_slice(&[i, below, below + 1, i, below + 1, i + 1]);
                        } else {
                            indices.extend_from_slice(&[i, below, i + 1, i + 1, below, below + 1]);
                        }
                    }
                }
                let s = terrain.scale;
                let far = (bx + BLOCK).min(cells) as f32;
                let near = (bz + BLOCK).min(cells) as f32;
                blocks.push((
                    Vec3::new(bx as f32 * s, low * s, -near * s),
                    Vec3::new(far * s, high * s, -(bz as f32) * s),
                    start..indices.len() as u32,
                ));
            }
        }
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // Every tile is the same size; pack textures into one array.
        let tile_size = terrain.tiles.first().map_or(64, |t| t.size);
        let tiles_per_side = (n as u32 - 1).div_ceil(tile_size).max(1);
        let mut tile_map = vec![0xffffu16; (tiles_per_side * tiles_per_side) as usize];
        for t in &terrain.tiles {
            let (tx, tz) = (t.x / tile_size, t.z / tile_size);
            if tx < tiles_per_side && tz < tiles_per_side {
                tile_map[(tz * tiles_per_side + tx) as usize] = t.texture;
            }
        }
        let images = textures;
        // The painted ground's colour, tile by tile, for the dirt that
        // builds up on the bikes (crate::grime), before the tiles are
        // packed into layers.
        let ground_colours = crate::graphics::made().is_some_and(|e| e.grime).then(|| {
            let average: Vec<Vec3> = images.iter().map(crate::grime::average_colour).collect();
            crate::grime::GroundColours {
                cells: tile_map.iter().map(|&t| average.get(t as usize).copied().unwrap_or(crate::grime::DIRT)).collect(),
                side: tiles_per_side,
                span: tile_size as f32 * terrain.scale,
            }
        });
        // The same picture once, and four to a layer past what the GPU
        // takes (a browser may take only 256).
        // (MM2_MAX_LAYERS=256 tries a browser's limit on a desktop.)
        let most = std::env::var("MM2_MAX_LAYERS").ok().and_then(|n| n.parse().ok()).unwrap_or(u32::MAX);
        let fitted = fit_layers(images, &mut tile_map, device.limits().max_texture_array_layers.min(most));
        let textures = upload_textures(device, queue, &fitted);
        let tile_map = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("tile map"),
                size: wgpu::Extent3d {
                    width: tiles_per_side,
                    height: tiles_per_side,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R16Uint,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            bytemuck::cast_slice(&tile_map),
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("terrain sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: crate::graphics::anisotropy(),
            ..Default::default()
        });
        let grid = [
            n as f32,
            terrain.scale,
            tile_size as f32,
            tiles_per_side as f32,
        ];
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let upgrades = crate::graphics::made();
        let (scene_format, samples) = match upgrades {
            Some(e) => (crate::graphics::HDR_FORMAT, crate::graphics::samples(e.msaa)),
            None => (format, 1),
        };
        let target_format = format;
        let format = scene_format;
        let shader = crate::enhance::shader(device, "terrain", include_str!("terrain.wgsl"));
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("terrain"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32, 1 => Snorm8x4],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: samples,
                ..Default::default()
            },
            multiview_mask: None,
            cache: None,
        });
        let objects_pole_lamps = objects.pole_lamps.clone();
        // The stadium lights at night, should this be a stadium: its
        // roof's painted lamps, or a ring round its floor.
        let night_banks = if upgrades.is_some() { objects.floodlights(FLOODLIGHTS_ABOVE) } else { Vec::new() };
        let night_found = upgrades.and_then(|_| found_lamps(&night_banks, &crate::night::Floor::guess(terrain)));
        let flood = upgrades.and_then(|_| crate::flood::FloodMap::new(device, terrain, &objects_pole_lamps));
        let mut rt = (upgrades.is_some() && crate::graphics::ray_traced()).then(|| {
            let geometry = objects.rt_geometry(device.limits().max_texture_array_layers);
            // The domed stadium's floodlights, high under its roof.
            let floodlights = objects.floodlights(FLOODLIGHTS_ABOVE);
            if !floodlights.is_empty() {
                eprintln!("ray tracing: {} floodlight banks", floodlights.len());
            }
            crate::rt::Scene::new(device, queue, terrain, images, &geometry, &floodlights)
        });
        let mut sky_light = None;
        let enhanced = upgrades.map(|_| {
            let mut shared = crate::enhance::Shared::new(device, queue, upgrades.is_some_and(|e| e.ruts || e.wear), rt.as_ref());
            if upgrades.is_some_and(|e| e.skylight) {
                let baked = crate::skylight::SkyLight::bake(terrain);
                shared.set_sky(device, queue, &baked, rt.as_ref());
                sky_light = Some([baked.side as f32, baked.spacing, baked.low, baked.range]);
            }
            use crate::enhance::{sampler, texture, uniform};
            use wgpu::{ShaderStages as S, TextureSampleType as T, TextureViewDimension as D};
            let float = T::Float { filterable: true };
            let group_layout = crate::enhance::layout(
                device,
                "terrain enhanced",
                &[
                    (0, S::VERTEX_FRAGMENT, uniform()),
                    // The grass reads these as it is placed, too.
                    (1, S::VERTEX_FRAGMENT, texture(D::D2Array, float)),
                    (2, S::VERTEX_FRAGMENT, sampler()),
                    (3, S::VERTEX_FRAGMENT, texture(D::D2, T::Uint)),
                    (4, S::FRAGMENT, texture(D::D2, float)),
                    (5, S::FRAGMENT, sampler()),
                    (6, S::FRAGMENT, texture(D::D2, float)),
                    (7, S::FRAGMENT, sampler()),
                    (8, S::FRAGMENT, texture(D::D2, T::Uint)),
                    (9, S::FRAGMENT, texture(D::D2Array, float)),
                    (10, S::FRAGMENT, sampler()),
                    (11, S::FRAGMENT, uniform()),
                ],
            );
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("terrain enhanced"),
                bind_group_layouts: &[Some(&group_layout), Some(&shared.layout)],
                immediate_size: 0,
            });
            let terrain_with = |fs: &str| device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("terrain enhanced"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32, 1 => Snorm8x4],
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(format.into())],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: samples,
                    ..Default::default()
                },
                multiview_mask: None,
                cache: None,
            });
            // The game's mesh where the smooth ground round the eye may lie
            // over it (it discards there), and the rest, which never does:
            // fs_smooth is the same ground with no discard.
            let terrain = (terrain_with("fs_enhanced"), terrain_with("fs_smooth"));
            (shared, terrain, group_layout)
        });
        let terrain_views = (
            textures.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            }),
            tile_map.create_view(&Default::default()),
            sampler,
        );
        // No detail texture until the scene gives one: clear, so the
        // ground shows as it is.
        let none = mm2_formats::tga::Image {
            width: 1,
            height: 1,
            rgba: vec![0; 4],
        };
        let detail = detail_view(device, queue, &none);
        let shadow_mask = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("shadow mask"),
                size: wgpu::Extent3d {
                    width: SHADOW_SIZE,
                    height: SHADOW_SIZE,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: crate::objects::SHADOW_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let ground = crate::fine_ground::Views::none(device, queue);
        let bind_group = terrain_bind_group(device, &pipeline.get_bind_group_layout(0), &globals, &terrain_views, &detail, &shadow_mask, &ground);
        let enhanced = enhanced.map(|(shared, terrain_pipeline, layout)| Enhanced {
            post: crate::post::Post::new(device, samples, target_format),
            group: terrain_bind_group(device, &layout, &globals, &terrain_views, &detail, &shadow_mask, &ground),
            ruts: upgrades.is_some_and(|e| e.ruts || e.wear).then(|| {
                // Over the whole terrain: x from 0, z up to 0.
                let size = (n as f32 - 1.0) * terrain.scale;
                let wear = upgrades.is_some_and(|e| !e.ruts);
                crate::ruts::RutPainter::new(device, glam::Vec2::new(0.0, -size), size, wear)
            }),
            mud: Vec::new(),
            grime: ground_colours.map(|g| (crate::grime::Grime::new(upgrades.map_or(0.0, |u| u.mud_start)), g)),
            crowd: None,
            bikes: Vec::new(),
            rain: upgrades.is_some_and(|e| e.weather == crate::graphics::Weather::Rain).then(|| {
                crate::rain::RainRenderer::new(device, &layout, &shared.layout, format, samples, DEPTH_FORMAT)
            }),
            dust: upgrades.is_some_and(|e| e.dust || e.roost).then(|| {
                crate::dust::DustRenderer::new(device, queue, &layout, &shared.layout, format, samples, DEPTH_FORMAT, &shared.buffer)
            }),
            grass: upgrades.is_some_and(|e| e.grass).then(|| {
                crate::grass::GrassRenderer::new(device, queue, terrain, &layout, &shared.layout, format, samples, DEPTH_FORMAT)
            }),
            smooth: upgrades.is_some_and(|e| e.smooth).then(|| {
                crate::smooth::SmoothGround::new(device, queue, terrain, &shader, &layout, &shared.layout, format, samples, DEPTH_FORMAT)
            }),
            rt_depth: rt.is_some().then(|| {
                let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("rt terrain depth"),
                    bind_group_layouts: &[Some(&layout), Some(&shared.layout)],
                    immediate_size: 0,
                });
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("rt terrain depth"),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        compilation_options: Default::default(),
                        buffers: &[Some(wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<Vertex>() as u64,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![0 => Float32, 1 => Snorm8x4],
                        })],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some("fs_rt_depth"),
                        compilation_options: Default::default(),
                        targets: &[],
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: crate::rt::DEPTH_FORMAT,
                        depth_write_enabled: Some(true),
                        depth_compare: Some(wgpu::CompareFunction::Less),
                        stencil: Default::default(),
                        bias: Default::default(),
                    }),
                    multisample: Default::default(),
                    multiview_mask: None,
                    cache: None,
                })
            }),
            terrain: terrain_pipeline,
            shared,
            layout,
            rt: None,
        });
        let objects = ObjectRenderer::new(
            device,
            queue,
            objects,
            &globals,
            format,
            DEPTH_FORMAT,
            samples,
            enhanced.as_ref().map(|e| &e.shared.layout),
        );
        if let (Some(rt), Some((vertices, nodes))) = (&mut rt, objects.as_ref().and_then(|o| o.posing())) {
            rt.attach(device, vertices, nodes);
        }
        let mut enhanced = enhanced;
        if let Some(e) = &mut enhanced {
            if let Some(rt) = &rt {
                e.shared.set_traced(device, rt);
            }
            e.rt = rt;
        }
        TerrainRenderer {
            pipeline,
            vertices,
            indices,
            blocks,
            heightfield: crate::cull::Heightfield::new(&terrain.heights, n, terrain.scale),
            runs: Vec::new(),
            nearest: Vec::new(),
            open_runs: Vec::new(),
            globals,
            bind_group,
            terrain_views,
            shadow_mask,
            ground,
            detail,
            grid,
            depth: None,
            depth_held: Default::default(),
            objects,
            sky: None,
            clear: SKY,
            fog: [0.62, 0.74, 0.88],
            fog_range: NO_FOG,
            trees: crate::objects::tree_band(crate::profile::GraphicsOptions::default().terrain),
            lights: SceneLights::default(),
            particles: crate::particles::Particles::new(),
            sky_on: true,
            particles_on: true,
            shadows_on: true,
            shadow: None,
            shadow_casters: None,
            shadowless: (Vec::new(), true),
            desert: false,
            stadium: false,
            env_painted: None,
            env_baked: None,
            cinematic: None,
            last_view_proj: None,
            taa_frame: 0,
            taa_last: None,
            gpu_timer: crate::gpu_time::GpuTimer::new(device, queue),
            scene_format,
            samples,
            size: (0, 0),
            enhanced,
            headlights: Vec::new(),
            flood,
            sky_light,
            pole_lamps: objects_pole_lamps,
            night_found,
            night_listed: None,
            night_banks,
            water: None,
        }
    }

    /// The track's floor and the lamps its `.lights` file lists, if any
    /// (crate::night): at night those light it, stadium or not.
    pub fn set_night(&mut self, listed: Option<Vec<crate::night::Lamp>>, floor: crate::night::Floor) {
        if self.enhanced.is_none() {
            return;
        }
        self.night_found = found_lamps(&self.night_banks, &floor);
        self.night_listed = listed.and_then(|l| crate::night::StadiumLights::new(l, &floor)).map(|n| crate::night::StadiumLights { listed: true, ..n });
    }

    /// Lays the detail texture over the terrain (0x50b140, 0x50b990): each
    /// texel is blended to the detail's colour by the detail's alpha, the
    /// way the game's second texture stage does (BLENDTEXTUREALPHA). The
    /// detail spans one terrain texture tile.
    pub fn set_detail(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, image: &mm2_formats::tga::Image) {
        self.detail = detail_view(device, queue, image);
        self.rebind(device);
    }

    /// Lays a free track's ground materials over its baked tiles as fine
    /// detail (fine_ground.rs).
    pub fn set_ground_map(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, map: &crate::fine_ground::GroundMap) {
        self.ground = crate::fine_ground::Views::new(device, queue, map);
        self.rebind(device);
    }

    fn rebind(&mut self, device: &wgpu::Device) {
        if let Some(e) = &mut self.enhanced {
            e.group = terrain_bind_group(device, &e.layout, &self.globals, &self.terrain_views, &self.detail, &self.shadow_mask, &self.ground);
        }
        self.bind_group = terrain_bind_group(
            device,
            &self.pipeline.get_bind_group_layout(0),
            &self.globals,
            &self.terrain_views,
            &self.detail,
            &self.shadow_mask,
            &self.ground,
        );
    }

    /// Draws `cube` as the sky.
    pub fn set_sky(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        cube: &mm2_formats::cub::Cube,
        images: &crate::hires::SkyImages,
        _format: wgpu::TextureFormat,
    ) {
        self.env_painted = Some(crate::ibl::shrink(images.side, &images.faces, 64));
        self.env_baked = None;
        self.sky = Some(crate::sky::SkyRenderer::new(
            device,
            queue,
            cube,
            images,
            self.scene_format,
            self.samples,
            self.enhanced.as_ref().map(|e| &e.shared.layout),
        ));
    }

    /// Draws the bikes' dirt from `sheet`, the scene's particle texture.
    pub fn set_dirt(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sheet: &mm2_formats::tga::Image,
        _format: wgpu::TextureFormat,
    ) {
        self.particles.upload(device, queue, sheet, self.scene_format, self.samples);
    }

    /// Fills what neither ground nor sky covers with the scene's fog
    /// colour (0–255 sRGB), as the game clears to it.
    pub fn set_fog(&mut self, rgb: [f32; 3]) {
        let linear = |c: f32| ((c / 255.0) as f64).powf(2.2);
        self.clear = wgpu::Color {
            r: linear(rgb[0]),
            g: linear(rgb[1]),
            b: linear(rgb[2]),
            a: 1.0,
        };
        self.fog = rgb.map(|c| linear(c) as f32);
    }

    /// Marks the track a desert one: the enhanced drawing's rain is
    /// overcast there.
    pub fn set_desert(&mut self, desert: bool) {
        self.desert = desert;
    }

    /// A stadium track: at night its own floodlights keep the floor lit.
    pub fn set_stadium(&mut self, stadium: bool) {
        self.stadium = stadium;
    }

    /// Lights the terrain and objects with the scene's lights.
    pub fn set_lights(&mut self, lights: SceneLights) {
        self.lights = lights;
    }

    /// The fog's reach at the Visibility detail `quality`, 0 to 9, for the
    /// scene's `[Fog]` Visibility and Haziness (0x463d00, 0x463700, the Fog
    /// made at 0x4e2d70 with 768, 256 and 128 ft): the far plane is
    /// `256 + 512 × (visibility − (9 − quality) / 10)`, at least 256, and
    /// the fog starts `1 − haziness` of the way from 128 ft out to it.
    pub fn set_visibility(&mut self, quality: u32, visibility: f32, haziness: f32) {
        self.fog_range = fog_range(quality, visibility, haziness);
    }

    /// The Ecosystem detail, 0 to 9: how near the trees turn to their
    /// billboards and how far those reach, as the original (Enhanced keeps
    /// its own, [`crate::objects::TreeBand::ENHANCED`]).
    pub fn set_ecosystem(&mut self, level: u32) {
        self.trees = crate::objects::tree_band(level);
    }

    /// The last frame's width over its height.
    pub fn aspect(&self) -> f32 {
        match self.size {
            (0, _) => 4.0 / 3.0,
            (w, h) => w as f32 / h.max(1) as f32,
        }
    }

    /// Moves animated object `index` (see [`ObjectMesh::sources`]) to
    /// `place`, in game coordinates.
    pub fn set_object_place(&mut self, index: usize, place: glam::Mat4) {
        if let Some(objects) = &mut self.objects {
            objects.set_place(index, place);
        }
    }

    /// Animated object `index`, for a craft the player takes over
    /// (crate::drive).
    pub fn animated_mut(&mut self, index: usize) -> Option<&mut crate::objects::AnimatedModel> {
        self.objects.as_mut()?.animated_mut(index)
    }

    /// The bikes' headlights: each one's place and the way it faces, game
    /// coordinates; lit at night with ray tracing.
    pub fn set_headlights(&mut self, lamps: &[(Vec3, Vec3)]) {
        self.headlights = lamps.iter().map(|(at, way)| (from_game(*at), from_game(*way).normalize_or(Vec3::Z))).collect();
    }

    /// Animated objects that cast no sun shadow (the signs over riders).
    pub fn set_shadowless(&mut self, models: &[usize]) {
        if self.shadowless.0 != models {
            self.shadowless = (models.to_vec(), false);
        }
    }

    /// The player's shadow: the objects that cast it, the middle of
    /// them and the way the light travels, game coordinates.
    pub fn set_shadow(&mut self, casters: [usize; 2], centre: Vec3, light: Vec3) {
        self.shadow = Some((casters, from_game(centre), from_game(light)));
    }

    /// Where animated object `index`'s frame `name` is at `time`, in game
    /// coordinates.
    pub fn object_frame(&self, index: usize, name: &str, time: f32) -> Option<Vec3> {
        self.objects.as_ref()?.frame_at(index, name, time)
    }

    /// Animated object `index`'s frame called `name`.
    pub fn object_frame_index(&self, index: usize, name: &str) -> Option<usize> {
        self.objects.as_ref()?.frame_index(index, name)
    }

    /// A copy of animated object `index`, to pose apart from the drawing.
    pub fn object_model(&self, index: usize) -> Option<crate::objects::AnimatedModel> {
        self.objects.as_ref()?.model(index)
    }

    /// Where animated object `index`'s frames are at `time`, game
    /// coordinates.
    pub fn object_frames(&self, index: usize, frames: &[usize], time: f32) -> Vec<Option<glam::Mat4>> {
        match &self.objects {
            Some(o) => o.frames_at(index, frames, time),
            None => vec![None; frames.len()],
        }
    }

    /// Leaves animated object `index` out of the camera's picture, its
    /// shadow kept, or draws it again.
    pub fn set_object_unseen(&mut self, index: usize, unseen: bool) {
        if let Some(objects) = &mut self.objects {
            objects.set_unseen(index, unseen);
        }
    }

    /// Draws animated object `index` see-through, or solid.
    pub fn set_object_ghost(&mut self, index: usize, ghost: bool) {
        if let Some(objects) = &mut self.objects {
            objects.set_ghost(index, ghost);
        }
    }

    /// Plays motion `name` on animated object `index` from `start` seconds.
    pub fn set_motion(&mut self, index: usize, name: &str, start: f32) {
        if let Some(objects) = &mut self.objects {
            objects.set_motion(index, name, start);
        }
    }

    /// Plays motion `name` on animated object `index` from `start`, fading
    /// in from the pose before over `span` seconds.
    pub fn fade_motion(&mut self, index: usize, name: &str, start: f32, span: f32) {
        if let Some(objects) = &mut self.objects {
            objects.fade_to(index, name, start, span);
        }
    }

    /// Turns animated object `index`'s frame `name` by `angle` about its
    /// own x axis: a bike's wheel.
    pub fn set_spin(&mut self, index: usize, name: &str, angle: f32) {
        if let Some(o) = &mut self.objects {
            o.set_spin(index, name, angle);
        }
    }

    /// Turns frame `name` of animated object `index` by `angle` about its
    /// own `axis` (a propeller or rotor, crate::drive).
    pub fn set_turn(&mut self, index: usize, name: &str, axis: Vec3, angle: f32) {
        if let Some(o) = &mut self.objects {
            o.set_turn(index, name, axis, angle);
        }
    }

    /// Slides frame `name` of animated object `index` by `by` (its
    /// parent's space), on top of its motion.
    pub fn set_slide(&mut self, index: usize, name: &str, by: Vec3) {
        if let Some(o) = &mut self.objects {
            o.set_slide(index, name, by);
        }
    }

    /// Mixes motions on animated object `index`, each at a phase and weight.
    pub fn set_blend(&mut self, index: usize, mix: &[(&str, f32, f32, u8)]) {
        if let Some(o) = &mut self.objects {
            o.set_blend(index, mix);
        }
    }

    /// Where each bike's tyres touch the ground (game coordinates) and
    /// how it kicks up dust there, for the enhanced drawing's ruts and
    /// dust; `dt` is the step, s.
    pub fn tyres(&mut self, feed: &[([Option<Vec3>; 2], crate::dust::Kick)], dt: f32) {
        let Some(e) = self.enhanced.as_mut() else { return };
        let bikes: Vec<[Option<Vec3>; 2]> = feed.iter().map(|f| f.0).collect();
        let bikes = &bikes[..];
        if let Some(ruts) = &mut e.ruts {
            ruts.tyres(bikes);
        }
        e.bikes.resize(bikes.len(), None);
        for (seen, tyres) in e.bikes.iter_mut().zip(bikes) {
            if let Some(p) = tyres[0].or(tyres[1]) {
                *seen = Some(from_game(p));
            }
        }
        let upgrades = crate::graphics::enhanced();
        let wet = upgrades.is_some_and(|u| u.weather == crate::graphics::Weather::Rain && !self.desert);
        if let (Some(dust), Some(u)) = (&mut e.dust, crate::graphics::showing().or(upgrades)) {
            let budget = crate::dust::Budget {
                dust: u.dust,
                roost: u.roost,
                amount: u.dust_amount,
            };
            dust.step(feed, dt, wet, budget);
        }
        // Dirt builds up very gradually through the race, taking the
        // ground's colour (crate::grime).
        if let Some((grime, colours)) = &mut e.grime {
            let drawn: Vec<([Option<Vec3>; 2], crate::dust::Kick)> = feed.iter().map(|(t, k)| (t.map(|p| p.map(from_game)), *k)).collect();
            grime.step(&drawn, dt, wet, |at| colours.at(at));
        } else if upgrades.is_some_and(|u| u.mud) && dt > 0.0 {
            // Dirt builds up as a bike runs on the ground: slowly in the
            // dry, fast on wet ground.
            let start = upgrades.map_or(0.0, |u| u.mud_start);
            e.mud.resize(bikes.len(), (start, None));
            for (m, tyres) in e.mud.iter_mut().zip(bikes) {
                let here = tyres[1].or(tyres[0]).map(from_game);
                if let (Some(a), Some(b)) = (m.1, here) {
                    let speed = a.distance(b) / dt;
                    if speed < 200.0 {
                        let rate = if wet { 0.02 } else { 0.006 };
                        m.0 = (m.0 + rate * (speed / 40.0).min(1.5) * dt).min(1.0);
                    }
                }
                m.1 = here;
            }
        }
    }

    /// The enhanced drawing's spectators: where each stands (game
    /// coordinates) and which way it looks.
    pub fn set_crowd(&mut self, device: &wgpu::Device, spots: &[(Vec3, Vec3)]) {
        let Some(e) = self.enhanced.as_mut() else { return };
        e.crowd = (!spots.is_empty()).then(|| {
            let samples = crate::graphics::enhanced().map_or(1, |u| crate::graphics::samples(u.msaa));
            crate::crowd::Crowd::new(device, spots, &e.layout, &e.shared.layout, crate::graphics::HDR_FORMAT, samples, DEPTH_FORMAT)
        });
        eprintln!("{} spectators", spots.len().div_ceil(spots.len().div_ceil(crate::crowd::MOST).max(1)));
    }

    /// The track's rivers, drawn over the scene in both drawings.
    pub fn set_water(&mut self, device: &wgpu::Device, rivers: &[mm2_course::nature::Water]) {
        let shared = self.enhanced.as_ref().map(|e| &e.shared.layout);
        self.water = crate::water::WaterRenderer::new(device, rivers, &self.globals, self.scene_format, self.samples, DEPTH_FORMAT, shared);
    }

    /// Draws bike `bike`'s models (its bike and its rider) as dirty as it
    /// has got.
    pub fn show_mud(&mut self, bike: usize, models: [usize; 2]) {
        let start = crate::graphics::enhanced().map_or(0.0, |u| u.mud_start);
        if let Some((grime, _)) = self.enhanced.as_ref().and_then(|e| e.grime.as_ref()) {
            let (a, b) = grime.coat(bike).packed();
            if let Some(objects) = &mut self.objects {
                for m in models {
                    objects.set_mud(m, a);
                    objects.set_coat(m, b);
                }
            }
            return;
        }
        let mud = self.enhanced.as_ref().and_then(|e| e.mud.get(bike)).map_or(start, |m| m.0);
        if let Some(objects) = &mut self.objects {
            for m in models {
                objects.set_mud(m, mud);
            }
        }
    }

    /// Holds animated object `index` on its first frame until `time`.
    pub fn set_motion_start(&mut self, index: usize, time: f32) {
        if let Some(objects) = &mut self.objects {
            objects.set_motion_start(index, time);
        }
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        (width, height): (u32, u32),
        camera: &crate::camera::Camera,
        time: f32,
    ) -> wgpu::CommandBuffer {
        self.size = (width, height);
        // The enhanced drawing as it shows now: none while F3 shows the
        // original look on it.
        let showing = self.enhanced.as_ref().and(crate::graphics::showing()).map(|mut e| {
            // No rain in the desert: overcast instead.
            if self.desert && e.weather == crate::graphics::Weather::Rain {
                e.weather = crate::graphics::Weather::Overcast;
            }
            e
        });
        if self.enhanced.is_none()
            && self
                .depth
                .as_ref()
                .is_none_or(|d| (d.1, d.2) != (width, height))
        {
            let depth = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("depth"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            // The last one's memory back at once, in a browser too
            // (crate::gpu_free): a browser's picture changes size while racing.
            let mut held = crate::gpu_free::Held::default();
            let depth = held.texture(depth);
            self.depth = Some((depth.create_view(&Default::default()), width, height));
            self.depth_held = held;
        }
        let aspect = width as f32 / height.max(1) as f32;
        // The size the enhanced scene is drawn at, before it is stretched
        // over the screen.
        let scene = showing.map_or((width, height), |e| {
            let size = e.scene_size((width, height));
            if !e.dynamic {
                return size;
            }
            // Dynamic resolution: smaller while frames come late, bigger
            // (up to the screen's own) while they keep time.
            // In a browser, no further than 1080p's worth (Enhanced::for_browser),
            // so Auto's Low (720p's worth) can grow to it with time to spare.
            let room = width as f32 / size.0.max(1) as f32;
            let room = if cfg!(target_arch = "wasm32") {
                let most = (crate::graphics::BROWSER_PIXELS as f32 / (size.0.max(1) * size.1.max(1)) as f32).sqrt();
                room.min(most.max(1.0))
            } else {
                room
            };
            crate::dynres::set_room(room);
            crate::dynres::scaled(size, (width, height))
        });
        // In a browser with FSR: the picture's share (dynres::Picture) taken
        // by the scene alone, FSR stretching it over the window's pixels.
        let scene = match showing {
            Some(e) if e.fsr && cfg!(target_arch = "wasm32") && crate::dynres::picture() < 1.0 => {
                let k = crate::dynres::picture();
                (((scene.0 as f32 * k).round() as u32).max(1), ((scene.1 as f32 * k).round() as u32).max(1))
            }
            _ => scene,
        };
        if let Some(e) = &mut self.enhanced {
            // FSR's targets made while it would be drawn.
            e.post.fsr_wanted = showing.is_some_and(|s| s.fsr) && (scene != (width, height) || showing.is_some_and(|s| s.taa)) && scene.0 <= width;
        }
        let (start, end) = self.fog_range;
        // The enhanced haze: half as thick as the game's fog at its end,
        // and the view reaching on till it has all but hidden the land.
        let haze = (showing.is_some_and(|e| e.distance) && self.fog_range != NO_FOG).then(|| {
            let span = (end - start).max(1.0);
            let far = (start + 7.0 * span).clamp(end, 20_000.0);
            // Enhanced+: the haze begins nearer and is thicker, so the air
            // between the rider and the hills shows (the far plane stays).
            if showing.is_some_and(|e| e.nextgen) {
                (start * 0.5, std::f32::consts::LN_2 / span * 1.5, far)
            } else {
                (start, std::f32::consts::LN_2 / span, far)
            }
        });
        let far = haze.map_or(end, |h| h.2);
        let plain = Mat4::perspective_rh(camera.fovy(aspect), aspect, 1.0, far);
        // Temporal anti-aliasing: each frame drawn a different fraction of
        // a pixel aside (the Halton points by 2 and 3), the steps blended
        // into one (crate::post).
        let taa = showing.is_some_and(|e| e.taa);
        let mut shift = [0.0; 2];
        let proj = if taa {
            self.taa_frame = self.taa_frame.wrapping_add(1);
            let i = self.taa_frame % 16 + 1;
            let (x, y) = (halton(i, 2) - 0.5, halton(i, 3) - 0.5);
            // Up in the projection is down on the screen.
            shift = [x, -y];
            Mat4::from_translation(Vec3::new(2.0 * x / scene.0 as f32, 2.0 * y / scene.1 as f32, 0.0)) * plain
        } else {
            plain
        };
        // The shadow (0x4df8c0): a camera at the sun, 80000 ft away and so
        // as good as parallel, fitted round the casters, their middle and
        // a radius about them; what it sees is laid over the ground.
        // The enhanced drawing's own shadows take its place.
        let sun_shadows = showing.is_some_and(|e| e.shadows) && self.objects.is_some();
        // Ray traced, the cascades are not drawn; the hills cast too, so
        // they need no objects.
        let traced = showing.is_some_and(|e| e.shadows) && self.enhanced.as_ref().is_some_and(|e| e.rt.is_some());
        let sun_shadows = sun_shadows || traced;
        let shadow = self.shadow.filter(|_| self.shadows_on && self.objects.is_some() && !sun_shadows);
        let shadow_view = shadow.map_or(Mat4::IDENTITY, |(_, centre, light)| {
            let to_sun = (-light).normalize_or(Vec3::Y);
            let up = if to_sun.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y };
            let view = Mat4::look_at_rh(centre + to_sun * SHADOW_REACH, centre, up);
            let r = SHADOW_RADIUS;
            Mat4::orthographic_rh(-r, r, -r, r, 0.0, SHADOW_REACH * 2.0) * view
        });
        // The enhanced drawing's hour and weather light the scene in
        // place of its own lights, and its real sky's colour is the haze's.
        let mut day = showing.map(|e| crate::enhance::daylight(&self.lights, e.time, e.weather));
        // At night a stadium's own lamps light it (crate::night), or any
        // track's that lists them; not ray traced, where the traced
        // floodlights do.
        let night_lamps = match (showing, &day) {
            (Some(e), Some(d)) if e.night_lights && d.night > 0.0 && !traced => {
                self.night_listed.as_ref().or(self.night_found.as_ref().filter(|_| self.stadium)).cloned()
            }
            _ => None,
        };
        if let (Some(d), Some(e), true) = (day.as_mut(), showing, night_lamps.is_some()) {
            // Under the roof no moon, and only the light thrown back off
            // the floor and the stands round about: a little of the day's,
            // a touch warm.
            let lit = crate::enhance::daylight(&self.lights, None, e.weather);
            let ambient = Vec3::from(lit.lights.ambient).max_element().max(0.25);
            d.lights.ambient = (Vec3::new(1.0, 0.97, 0.93) * ambient * NIGHT_BOUNCE).into();
            d.lights.sun = [0.0; 3];
            d.lights.to_sun = Vec3::Y;
        } else if let Some(d) = day.as_mut().filter(|d| self.stadium && d.night > 0.0) {
            // In a stadium at night the floodlights light the floor as the
            // day would: the scene's own lights, under the night sky.
            if let Some(e) = showing {
                let lit = crate::enhance::daylight(&self.lights, None, e.weather);
                d.lights = lit.lights;
                d.night = 0.0;
            }
        }
        let real_sky = showing.is_some_and(|e| e.clouds && e.sky) && self.sky.is_some() && self.sky_on;
        let lights = day.as_ref().map_or(self.lights, |d| d.lights);
        let trees = if showing.is_some() { crate::objects::TreeBand::ENHANCED } else { self.trees };
        let globals = Globals {
            view_proj: (proj * camera.view()).to_cols_array_2d(),
            camera: camera.position.extend(1.0).into(),
            grid: self.grid,
            fog: [self.fog[0], self.fog[1], self.fog[2], 1.0],
            fog_range: [start, end, trees.edge, trees.band],
            ambient: lights.globals().0,
            sun: lights.globals().1,
            to_sun: lights.globals().2,
            shadow_view: shadow_view.to_cols_array_2d(),
            shadow_on: [shadow.is_some() as u32 as f32, lights.light_ground, trees.reach.min(1e9), 0.0],
        };
        queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));
        let seen = crate::cull::Frustum::new(plain * camera.view());
        // The last frame's view for temporal anti-aliasing, and whether
        // the camera cut (a jump or a sharp turn): then it starts over.
        let taa_frame = taa.then(|| {
            let now = plain * camera.view();
            let forward = camera.forward();
            let last = self.taa_last.filter(|l| l.1.distance(camera.position) < 25.0 && l.2.dot(forward) > 0.85);
            crate::post::Taa {
                inverse: now.inverse(),
                before: last.map_or(now, |l| l.0),
                camera: camera.position,
                shift,
                fresh: last.is_none(),
            }
        });
        self.taa_last = taa.then(|| (plain * camera.view(), camera.position, camera.forward()));
        // Nearest first: the ground's shading is the dearest part of an
        // enhanced frame, and a block hidden behind a nearer hill then
        // fails the depth test before it is shaded. The ground never lies
        // over itself at one depth, so the picture is the same.
        let eye = camera.position;
        // Which blocks the smooth patch round the eye lies over, with a
        // cell to spare: only those can have pixels fs_enhanced discards.
        let patch = self.enhanced.as_ref().and_then(|e| e.smooth.as_ref()).map(|g| g.uniform(eye));
        let scale = self.grid[1];
        let under = |lo: Vec3, hi: Vec3| {
            patch.is_some_and(|[px, pz, cells, _]| {
                let (x0, x1) = (lo.x / scale, hi.x / scale);
                let (z0, z1) = (-hi.z / scale, -lo.z / scale);
                x1 >= px - 1.0 && x0 <= px + cells + 1.0 && z1 >= pz - 1.0 && z0 <= pz + cells + 1.0
            })
        };
        self.nearest.clear();
        // What the ground's hills hide from this camera (crate::cull),
        // unless left out for testing (MM2_SKIP=occlusion).
        let occluder = (!crate::gpu_time::skip("occlusion")).then_some(&self.heightfield);
        self.nearest.extend(
            self.blocks
                .iter()
                .filter(|b| seen.sees(b.0, b.1) && occluder.is_none_or(|o| !o.hides(eye, b.0, b.1)))
                .map(|b| (eye.clamp(b.0, b.1).distance_squared(eye), b.2.clone(), under(b.0, b.1))),
        );
        self.nearest.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.start.cmp(&b.1.start)));
        crate::cull::runs(self.nearest.iter().map(|(_, r, _)| r.clone()), &mut self.runs);
        self.open_runs.clear();
        for (_, r, cut) in &self.nearest {
            match self.open_runs.last_mut() {
                Some((last, was)) if last.end == r.start && was == cut => last.end = r.end,
                _ => self.open_runs.push((r.clone(), *cut)),
            }
        }
        let side = self.enhanced.as_ref().map_or(crate::enhance::SHADOW_SIDE, |e| e.shared.side);
        let (cascades, texels) = crate::enhance::cascades(camera.view(), camera.fovy(aspect), aspect, lights.to_sun, side);
        // The dust clouds as the sun sees them, round those near the eye.
        let dust_view = self
            .enhanced
            .as_ref()
            // Not on Low: there the puffs are lit without it.
            .filter(|_| showing.is_some_and(|e| e.dust && e.dust_amount >= 0.6))
            .and_then(|e| e.dust.as_ref()?.bounds(camera.position))
            .map(|(low, high)| crate::dust::sun_view(low, high, lights.to_sun));
        // The light poles' light baked the first night frame: traced
        // through the ray-traced scene, else over the hills alone.
        if let (Some(flood), Some(e)) = (self.flood.as_mut().filter(|f| !f.baked() && day.as_ref().is_some_and(|d| d.night > 0.0)), self.enhanced.as_mut()) {
            match e.rt.as_mut() {
                Some(rt) => {
                    // The still parts built first, if this is the first frame.
                    if !rt.ready() {
                        let mut build = device.create_command_encoder(&Default::default());
                        rt.update(&mut build, None);
                        queue.submit([build.finish()]);
                    }
                    rt.bake_flood(device, queue, flood);
                    e.shared.set_flood(device, flood.map(), Some(rt));
                }
                None => {
                    flood.bake_hills(device, queue);
                    e.shared.set_flood(device, flood.map(), None);
                }
            }
        }
        // The sky's light all round, baked again when the light changes.
        if let (Some(e), Some(on), Some(d)) = (self.enhanced.as_mut(), showing, day.as_ref()) {
            if on.pbr {
                let painted = !real_sky && self.sky.is_some() && self.env_painted.is_some();
                let light = crate::ibl::Light {
                    to_sun: lights.to_sun,
                    sun: Vec3::from(lights.sun),
                    ambient: Vec3::from(lights.ambient),
                    zenith: d.zenith,
                    horizon: d.horizon,
                    fog: Vec3::from(self.fog),
                    cover: d.cover,
                    night: d.night,
                };
                let fresh = self.env_baked.as_ref().is_some_and(|(p, l, _)| *p == painted && l.near(&light));
                if !fresh {
                    let source = match (&self.env_painted, painted, self.sky.is_some()) {
                        (Some(s), true, _) => s.clone(),
                        (_, _, true) => crate::ibl::Source::Real,
                        _ => crate::ibl::Source::Indoors,
                    };
                    let baked = crate::ibl::bake(&source, &light, crate::ibl::SIDE);
                    let view = crate::ibl::upload(device, queue, &baked);
                    e.shared.set_env(device, view, e.rt.as_ref());
                    self.env_baked = Some((painted, light, baked.sh));
                }
            }
        }
        if let (Some(e), Some(on)) = (&self.enhanced, showing) {
            let flag = |b: bool| b as u32 as f32;
            let (h_start, density, _) = haze.unwrap_or((0.0, 0.0, far));
            let enh = crate::enhance::EnhGlobals {
                cascades: cascades.map(|c| c.to_cols_array_2d()),
                splits: crate::enhance::splits(),
                flags: [flag(on.lighting), flag(sun_shadows && night_lamps.is_none()), flag(haze.is_some()), flag(on.sky)],
                haze: [h_start, density, far, on.detail],
                sun: lights.globals().1,
                to_sun: lights.globals().2,
                fog: match (&day, real_sky) {
                    (Some(d), true) => d.horizon.extend(1.0).into(),
                    // The haze in a dark arena is dark.
                    _ if night_lamps.is_some() => [self.fog[0] * NIGHT_HAZE, self.fog[1] * NIGHT_HAZE, self.fog[2] * NIGHT_HAZE, 1.0],
                    _ => [self.fog[0], self.fog[1], self.fog[2], 1.0],
                },
                texel: [texels[0], texels[1], texels[2], flag(on.soft_shadows)],
                world: day.as_ref().map_or([time, 0.0, 0.0, 0.0], |d| [time, d.wet, d.rain, d.cover]),
                ruts: e.ruts.as_ref().filter(|_| on.ruts || on.wear).map_or([0.0; 4], |r| r.uniform()),
                more: [flag(on.ground), flag(on.grass), flag(real_sky), flag(on.bikes)],
                material_average: e.shared.material_average,
                // The clouds drift 30 ft a second.
                sky: [aspect, flag(on.mud), time * 30.0, 1.0],
                zenith: day.as_ref().map_or([0.0; 4], |d| d.zenith.extend(1.0).into()),
                horizon: day.as_ref().map_or([0.0; 4], |d| d.horizon.extend(1.0).into()),
                smooth: e.smooth.as_ref().filter(|_| on.smooth).map_or([0.0; 4], |g| g.uniform(camera.position)),
                surfaces: [flag(on.surfaces), flag(on.trees), flag(on.round), flag(on.floodlights && self.sky.is_none())],
                // y: the dirt building up (grime), z how strongly the air is
                // lit, w the wind in the grass and trees (enhanced_fns.wgsl
                // `air_light`, `windy_day`).
                traced: [flag(traced), flag(e.grime.is_some()), if on.lit_air { if on.nextgen { 1.0 } else { LIT_AIR_SUBTLE } } else { 0.0 }, flag(on.wind)],
                view_proj: (proj * camera.view()).to_cols_array_2d(),
                ambient: lights.globals().0,
                // z: the Enhanced+ preset's stronger look (enhanced_fns.wgsl
                // `next_gen`): lusher grass, firmer ground detail; w the photo
                // dirt up close (`close_up`).
                terrain: [self.grid[1], self.grid[2] * self.grid[3], flag(on.nextgen), flag(on.close_up)],
                night: {
                    let (a, s, t) = self.lights.globals();
                    let own = Vec3::new(a[0], a[1], a[2]) + Vec3::new(s[0], s[1], s[2]) * t[1].max(0.0);
                    [day.as_ref().map_or(0.0, |d| d.night), own.x, own.y, own.z]
                },
                headlights: [
                    flag(self.enhanced.as_ref().is_some_and(|e| e.rt.is_some()) && !self.headlights.is_empty()),
                    if day.as_ref().is_some_and(|d| d.night > 0.0) { (self.headlights.len() + self.pole_lamps.len()).min(crate::enhance::LAMPS) as f32 } else { 0.0 },
                    // z: how many of the lamps are the bikes (the first),
                    // day or night, for the grass they push aside.
                    self.headlights.len().min(crate::enhance::LAMPS) as f32,
                    0.0,
                ],
                lamps: {
                    let mut l = [[0.0; 4]; crate::enhance::LAMPS];
                    let n = day.as_ref().map_or(0.0, |d| d.night);
                    // w: how strongly it glows, and how big it is (the poles'
                    // heads bigger), as the strength plus 10 for a pole.
                    let all = self.headlights.iter().map(|(at, _)| (*at, n)).chain(self.pole_lamps.iter().map(|(at, _)| (*at, n + 10.0)));
                    for (out, (at, w)) in l.iter_mut().zip(all) {
                        *out = [at.x, at.y, at.z, w];
                    }
                    l
                },
                dust_view: dust_view.map_or(Mat4::IDENTITY, |(m, _)| m).to_cols_array_2d(),
                dust: dust_view.map_or([0.0; 4], |(_, least)| [1.0, least, 0.0, 0.0]),
                flood: self.flood.as_ref().map_or([0.0; 4], |f| f.area()),
                sky_light: {
                    let [side, spacing, ..] = self.sky_light.unwrap_or([1.0, 1.0, 0.0, 1.0]);
                    let strength = if on.skylight && self.sky_light.is_some() { 1.0 } else { 0.0 };
                    [side, spacing, strength, flag(on.ssr && !traced && day.as_ref().is_some_and(|d| d.wet > 0.0))]
                },
                sky_height: {
                    let [.., low, range] = self.sky_light.unwrap_or([1.0, 1.0, 0.0, 1.0]);
                    [low, range, 0.0, 0.0]
                },
                stadium: night_lamps.as_ref().map_or([0.0; 4], |n| {
                    let night = day.as_ref().map_or(0.0, |d| d.night);
                    [n.lamps.len() as f32, night, n.lowest, if sun_shadows { crate::night::GROUPS as f32 } else { 0.0 }]
                }),
                stadium_views: night_lamps.as_ref().map_or([Mat4::IDENTITY; crate::night::GROUPS], |n| n.views).map(|m| m.to_cols_array_2d()),
                // w: the lamps are a track's own list, their heads to glow.
                stadium_texels: night_lamps.as_ref().map_or([1.0; 4], |n| [n.texels[0], n.texels[1], n.texels[2], flag(n.listed)]),
                stadium_lamps: night_lamps.as_ref().map_or([[0.0; 4]; crate::night::MOST_LAMPS], |n| n.uniform().0),
                stadium_ways: night_lamps.as_ref().map_or([[0.0; 4]; crate::night::MOST_LAMPS], |n| n.uniform().1),
                ibl: {
                    // w: the sunlit air in the haze, under an open sky.
                    let air = flag(on.air && self.sky.is_some());
                    match (&self.env_baked, on.pbr) {
                        (Some(_), true) => [1.0, (crate::ibl::LEVELS - 1) as f32, GAME_SKY_LIGHT, air],
                        _ => [0.0, 0.0, 0.0, air],
                    }
                },
                sh: self.env_baked.as_ref().map_or([[0.0; 4]; 9], |b| b.2),
            };
            queue.write_buffer(&e.shared.buffer, 0, bytemuck::bytes_of(&enh));
        }
        if let Some(sky) = &self.sky {
            sky.update(queue, proj, camera.view());
        }
        if let Some(objects) = &mut self.objects {
            objects.extras_shown = showing.is_some_and(|e| e.scenery);
            objects.trees = trees;
            objects.update(queue, time);
            if !self.shadowless.1 {
                objects.set_shadowless(device, &self.shadowless.0);
                self.shadowless.1 = true;
            }
            objects.cull(0, &seen, camera.position, occluder);
            if let Some((casters, ..)) = shadow {
                if self.shadow_casters != Some(casters) {
                    objects.set_casters(device, &casters);
                    self.shadow_casters = Some(casters);
                }
                objects.set_shadow_view(queue, shadow_view);
            }
            if sun_shadows && !traced {
                // At night under a stadium's lamps, each layer is a group
                // of lamps' view instead.
                let cascades = night_lamps.as_ref().map_or(cascades, |n| n.views);
                objects.set_cascades(queue, &cascades);
                // The sun's cascades hold only what can shade what is seen
                // (enhance::cascade_reach); a stadium's lamps' views all.
                let reach = match &night_lamps {
                    Some(_) => cascades,
                    None => crate::enhance::cascade_reach(camera.view(), camera.fovy(aspect), aspect, &cascades, &texels),
                };
                for (i, (tight, wide)) in reach.iter().zip(&cascades).enumerate() {
                    // The cascade's own stretch of ground, from the sun's
                    // height: none under a stadium's lamps, whose views
                    // are not bands of distance.
                    let band = night_lamps.as_ref().map_or_else(
                        || {
                            let splits = crate::enhance::splits();
                            Some((if i == 0 { 0.0 } else { splits[i - 1] }, splits[i], lights.to_sun.y))
                        },
                        |_| None,
                    );
                    objects.cull_cascade(1 + i, &crate::cull::Frustum::new(*tight), &crate::cull::Frustum::new(*wide), camera.position, band, None);
                }
            }
        }
        {
            // A particle faces the camera, each corner lit by the scene's
            // lights along its own normal, out to its corner and a quarter
            // toward the camera (0x4bec20's +0x2024, by 0x49fa50).
            let view = camera.view();
            let inverse = view.inverse();
            let [right, up, back] = [inverse.x_axis, inverse.y_axis, inverse.z_axis].map(|a| a.truncate());
            let l = &lights;
            let light = |x: f32, y: f32| {
                let n = (right * x + up * y + back * 0.25).normalize_or(back);
                let lit = Vec3::from(l.ambient) + Vec3::from(l.sun) * n.dot(l.to_sun).max(0.0);
                lit.min(Vec3::ONE)
            };
            self.particles.set_light([light(1.0, 1.0), light(-1.0, 1.0), light(1.0, -1.0), light(-1.0, -1.0)]);
            // In the haze, the dirt fades over about the same distance.
            let range = match haze {
                Some(_) => (start, start + 2.5 * (end - start)),
                None => self.fog_range,
            };
            // The enhanced dust clouds stand in for the tyres' dust trail.
            self.particles.hide_trail = showing.is_some_and(|e| e.dust);
            // And its flying dirt for the spinning tyre's chunks.
            self.particles.hide_chunks = showing.is_some_and(|e| e.roost);
            self.particles.prepare(queue, proj, view, (self.fog, range));
        }

        let mut encoder = device.create_command_encoder(&Default::default());
        if let Some(t) = &mut self.gpu_timer {
            t.begin(device, &mut encoder);
        }
        if let (Some(objects), Some(_)) = (&self.objects, shadow) {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow mask"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.shadow_mask,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            objects.draw_casters(&mut pass);
        }
        mark!(self.gpu_timer, &mut encoder, "old shadow");
        if let Some(e) = &mut self.enhanced {
            if let Some(ruts) = &mut e.ruts {
                ruts.paint(device, queue, &mut encoder, &e.shared.ruts.0);
            }
            if let Some(smooth) = &mut e.smooth {
                smooth.cull(camera.position, &seen);
            }
            if let Some(crowd) = &mut e.crowd {
                let bikes: Vec<Vec3> = e.bikes.iter().flatten().copied().collect();
                crowd.watch(queue, &bikes);
                crowd.cull(&seen);
            }
            if let Some(dust) = &mut e.dust {
                dust.prepare(queue, camera.position, showing.is_some_and(|e| e.dust), showing.is_some_and(|e| e.roost));
                if dust_view.is_some() {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("dust shadows"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &e.shared.dust,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    dust.draw_shadow(&mut pass, &e.group);
                }
            }
        }
        mark!(self.gpu_timer, &mut encoder, "ruts, dust shadow");
        // The ray-traced shadows: the moving models posed, the nearest
        // surface of each pixel drawn, and its shadow traced, before the
        // scene that reads it.
        if let Some(e) = self.enhanced.as_mut().filter(|e| traced && e.rt.is_some()) {
            let rt = e.rt.as_mut().unwrap();
            if rt.resize(device, scene) {
                e.shared.set_traced(device, rt);
                e.post.set_traced_shafts(rt.shafts());
            }
            rt.update(&mut encoder, self.gpu_timer.as_mut());
            mark!(self.gpu_timer, &mut encoder, "rt: build");
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("rt depth"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: rt.depth().unwrap(),
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                if let Some(pipeline) = &e.rt_depth {
                    pass.set_pipeline(pipeline);
                    pass.set_bind_group(0, &e.group, &[]);
                    pass.set_bind_group(1, &e.shared.group, &[]);
                    pass.set_vertex_buffer(0, self.vertices.slice(..));
                    pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
                    for run in &self.runs {
                        pass.draw_indexed(run.clone(), 0, 0..1);
                    }
                }
                // And the smooth ground round the eye, where it is drawn.
                if let Some(smooth) = e.smooth.as_ref().filter(|_| showing.is_some_and(|s| s.smooth)) {
                    smooth.draw_depth(&mut pass, &e.group, &e.shared.group);
                }
                if let Some(objects) = &self.objects {
                    objects.draw_camera_depth(queue, &mut pass, proj * camera.view());
                }
            }
            mark!(self.gpu_timer, &mut encoder, "rt: depth pass");
            // Indoors (a stadium, with no sky box) only the moving models
            // cast, as with the cascades.
            // Lit by floodlights, everything casts; with only the stadium's
            // one light (as with the shadow maps) only the moving models,
            // or the roof would shade the whole floor.
            let indoors = self.sky.is_none();
            let mask = if indoors && !rt.has_floodlights() { crate::rt::MOVING } else { 0xff };
            // The light shafts, outdoors only (indoors the "sun" is the
            // stadium's lights), fainter under cloud.
            let shafts = showing.is_some_and(|s| s.shafts) && self.sky.is_some() && self.sky_on;
            e.post.shaft_colour = lights.sun;
            e.post.shaft_strength = if shafts { SHAFT_STRENGTH * (1.0 - day.as_ref().map_or(0.0, |d| d.cover * 0.8)) } else { 0.0 };
            let night = day.as_ref().map_or(0.0, |d| d.night);
            let lamps: &[(Vec3, Vec3)] = if night > 0.0 { &self.headlights } else { &[] };
            let poles: &[(Vec3, Vec3)] = if night > 0.0 { &self.pole_lamps } else { &[] };
            rt.trace(queue, &mut encoder, proj * camera.view(), camera.position, lights.to_sun, mask, camera.fovy(aspect), shafts, indoors, lamps, poles, night, self.gpu_timer.as_mut());
        }
        if let (Some(objects), Some(e), true) = (&self.objects, &self.enhanced, sun_shadows && !traced) {
            // The cascades side by side in the one map (enhance.rs), a
            // tile each, drawn in a single pass: one clear for the three,
            // and each cascade kept to its own tile by its viewport and
            // scissor, so nothing it draws spills into the next tile.
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sun shadows"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &e.shared.shadow_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            for cascade in 0..crate::enhance::CASCADES {
                let x = cascade as f32 * side as f32;
                pass.set_viewport(x, 0.0, side as f32, side as f32, 0.0, 1.0);
                pass.set_scissor_rect(cascade as u32 * side, 0, side, side);
                // Indoors (a stadium, with no sky box) only the moving
                // models cast.
                if !crate::gpu_time::skip("cascades") {
                    objects.draw_sun_depth(&mut pass, cascade, self.sky.is_none());
                }
                mark_pass!(self.gpu_timer, &mut pass, ["cascade 0", "cascade 1", "cascade 2", "cascade 3"][cascade.min(3)]);
            }
            drop(pass);
        }
        mark!(self.gpu_timer, &mut encoder, "sun cascades");
        {
            let (colour, resolve, depth, store) = match &mut self.enhanced {
                Some(e) => {
                    let (colour, resolve, depth) = e.post.scene(device, scene, (width, height));
                    (colour, resolve, depth, wgpu::StoreOp::Store)
                }
                None => (target, None, &self.depth.as_ref().unwrap().0, wgpu::StoreOp::Discard),
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("terrain"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: colour,
                    depth_slice: None,
                    resolve_target: resolve,
                    ops: wgpu::Operations {
                        // A night sky over an open stadium lit by its lamps.
                        load: wgpu::LoadOp::Clear(if night_lamps.is_some() { NIGHT_SKY } else { self.clear }),
                        // Anti-aliased, only the resolved picture is read
                        // after: the samples themselves need not be kept
                        // (on a tiled GPU, such as Apple's, they then never
                        // leave the chip, a large saving at 4K).
                        store: if resolve.is_some() { wgpu::StoreOp::Discard } else { wgpu::StoreOp::Store },
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store,
                    }),
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: wgpu::StoreOp::Discard,
                    }),
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let shared = self.enhanced.as_ref().filter(|_| showing.is_some()).map(|e| (&e.shared.group, &e.terrain, &e.group));
            let grass = self
                .enhanced
                .as_ref()
                .filter(|_| showing.is_some_and(|e| e.grass))
                .and_then(|e| Some((e.grass.as_ref()?, &e.group, &e.shared.group)));
            pass.set_vertex_buffer(0, self.vertices.slice(..));
            pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
            let draw_ground = !crate::gpu_time::skip("terrain");
            match shared {
                Some((group, (cut, open), own)) => {
                    pass.set_bind_group(0, own, &[]);
                    pass.set_bind_group(1, group, &[]);
                    let mut bound = None;
                    for (run, under) in self.open_runs.iter().filter(|_| draw_ground) {
                        if bound != Some(*under) {
                            pass.set_pipeline(if *under { cut } else { open });
                            bound = Some(*under);
                        }
                        pass.draw_indexed(run.clone(), 0, 0..1);
                    }
                }
                None => {
                    pass.set_pipeline(&self.pipeline);
                    pass.set_bind_group(0, &self.bind_group, &[]);
                    for run in self.runs.iter().filter(|_| draw_ground) {
                        pass.draw_indexed(run.clone(), 0, 0..1);
                    }
                }
            }
            mark_pass!(self.gpu_timer, &mut pass, "scene: terrain");
            if let Some((crowd, own, group)) = self
                .enhanced
                .as_ref()
                .filter(|_| showing.is_some_and(|e| e.scenery))
                .and_then(|e| Some((e.crowd.as_ref()?, &e.group, &e.shared.group)))
            {
                crowd.draw(&mut pass, own, group);
            }
            mark_pass!(self.gpu_timer, &mut pass, "scene: crowd");
            if let Some((smooth, own, group)) = self
                .enhanced
                .as_ref()
                .filter(|_| showing.is_some_and(|e| e.smooth))
                .and_then(|e| Some((e.smooth.as_ref()?, &e.group, &e.shared.group)))
            {
                if !crate::gpu_time::skip("smooth") {
                    smooth.draw(&mut pass, own, group);
                }
            }
            mark_pass!(self.gpu_timer, &mut pass, "scene: smooth hills");
            if let Some((grass, own, group)) = grass.filter(|_| !crate::gpu_time::skip("grass")) {
                grass.draw(&mut pass, own, group, &seen, eye);
            }
            mark_pass!(self.gpu_timer, &mut pass, "scene: grass");
            if let Some(objects) = self.objects.as_ref().filter(|_| !crate::gpu_time::skip("objects")) {
                match shared {
                    Some((group, ..)) => objects.draw_enhanced(&mut pass, group),
                    None => objects.draw(&mut pass),
                }
            }
            mark_pass!(self.gpu_timer, &mut pass, "scene: objects");
            // The sky last of the solid scene, squeezed to the far depth so
            // it only colours what nothing nearer covered (each sample, as
            // before): it was drawn first, under it all, and most of it then
            // hidden. What is laid over it after (water, dust, particles) is
            // laid over the same colours.
            if let Some(sky) = self.sky.as_ref().filter(|_| self.sky_on) {
                let (w, h) = if self.enhanced.is_some() { scene } else { (width, height) };
                pass.set_viewport(0.0, 0.0, w as f32, h as f32, 1.0, 1.0);
                match shared {
                    Some((group, ..)) => sky.draw_enhanced(&mut pass, group, real_sky),
                    None => sky.draw(&mut pass),
                }
                pass.set_viewport(0.0, 0.0, w as f32, h as f32, 0.0, 1.0);
            }
            mark_pass!(self.gpu_timer, &mut pass, "scene: sky");
            if let Some(water) = &self.water {
                water.set_time(queue, time, day.as_ref().map_or(1.0, |d| 1.0 - d.night));
                water.draw(&mut pass, shared.map(|s| s.0));
            }
            if let Some((dust, own, group)) = self
                .enhanced
                .as_ref()
                .filter(|_| showing.is_some_and(|e| e.dust || e.roost))
                .and_then(|e| Some((e.dust.as_ref()?, &e.group, &e.shared.group)))
            {
                dust.draw(&mut pass, own, group);
            }
            mark_pass!(self.gpu_timer, &mut pass, "scene: dust");
            if self.particles_on {
                self.particles.draw(&mut pass);
            }
            mark_pass!(self.gpu_timer, &mut pass, "scene: particles");
            if let Some((rain, own, group)) = self
                .enhanced
                .as_ref()
                // (With temporal anti-aliasing the last step draws it, or
                // its streaks would blend away.)
                .filter(|_| !taa && showing.is_some_and(|e| e.weather == crate::graphics::Weather::Rain))
                .and_then(|e| Some((e.rain.as_ref()?, &e.group, &e.shared.group)))
            {
                rain.draw(&mut pass, own, group);
            }
        }
        mark!(self.gpu_timer, &mut encoder, "scene: rain, resolve");
        // The moving models' motion on screen, for the anti-aliasing.
        if let (Some(e), Some(objects), Some(t)) = (&self.enhanced, &self.objects, taa_frame) {
            if let Some((motion, depth)) = e.post.motion() {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("motion"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: motion,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: depth,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Discard,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                objects.draw_motion(queue, &mut pass, plain * camera.view(), t.before, camera.position, scene);
            }
        }
        mark!(self.gpu_timer, &mut encoder, "taa motion");
        if let Some(e) = &self.enhanced {
            // The sun's place on the screen, for its shafts: strongest
            // looking toward it, fading as it leaves the screen, and none
            // indoors.
            let sun = showing.filter(|_| self.sky.is_some() && self.sky_on).and_then(|_| {
                let clip = plain * camera.view() * (camera.position + lights.to_sun * 10_000.0).extend(1.0);
                if clip.w <= 0.0 {
                    return None;
                }
                let ndc = clip.truncate() / clip.w;
                let (x, y) = (ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
                let off = (x - x.clamp(0.0, 1.0)).abs().max((y - y.clamp(0.0, 1.0)).abs());
                let strength = 1.6 * (1.0 - (off / 0.4).clamp(0.0, 1.0)) * (1.0 - day.as_ref().map_or(0.0, |d| d.cover * 0.8));
                (strength > 0.0).then(|| ([x, y, strength], lights.sun))
            });
            let view_proj = plain * camera.view();
            let cinematic = self.cinematic.filter(|_| showing.is_some_and(|e| e.cinematic)).map(|focus| {
                let last = self.last_view_proj.unwrap_or(view_proj);
                (focus, last * view_proj.inverse())
            });
            self.last_view_proj = Some(view_proj);
            // The rain over the finished picture, with the anti-aliasing.
            let rain = day.as_ref().filter(|d| taa && e.rain.is_some() && d.rain > 0.0 && showing.is_some_and(|s| s.weather == crate::graphics::Weather::Rain)).map(|d| {
                let a = lights.globals().0;
                let c = d.horizon * 0.9 + Vec3::new(a[0], a[1], a[2]) * 0.3;
                ([time, d.rain, aspect, camera.position.x], c.into())
            });
            // The contact shadows by day, as the sun's own show; the
            // reflections as wet as the ground is.
            let screen_fx = crate::post::Screen {
                view: camera.view(),
                to_sun: lights.to_sun,
                contact: if showing.is_some_and(|s| s.shadows) {
                    let high = ((lights.to_sun.y - 0.03) / 0.15).clamp(0.0, 1.0);
                    0.5 * high * day.as_ref().map_or(1.0, |d| (1.0 - d.night) * (1.0 - d.cover * 0.7))
                } else {
                    0.0
                },
                wet: day.as_ref().map_or(0.0, |d| d.wet),
            };
            e.post.finish(queue, &mut encoder, target, proj, showing, sun, cinematic, (width, height), taa_frame, rain, screen_fx, self.gpu_timer.as_mut());
        }
        if let Some(t) = &mut self.gpu_timer {
            t.end(&mut encoder);
        }
        encoder.finish()
    }
}

/// The `i`th point of the Halton sequence by `base`, 0 to 1.
fn halton(mut i: u32, base: u32) -> f32 {
    let (mut f, mut r) = (1.0, 0.0);
    while i > 0 {
        f /= base as f32;
        r += f * (i % base) as f32;
        i /= base;
    }
    r
}

/// Uploads every terrain texture with its mips into one texture array.
fn terrain_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    globals: &wgpu::Buffer,
    (textures, tile_map, sampler): &(wgpu::TextureView, wgpu::TextureView, wgpu::Sampler),
    detail: &wgpu::TextureView,
    shadow_mask: &wgpu::TextureView,
    ground: &crate::fine_ground::Views,
) -> wgpu::BindGroup {
    let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("shadow sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let detail_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("detail sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        anisotropy_clamp: crate::graphics::anisotropy(),
        ..Default::default()
    });
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("terrain"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(textures),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(tile_map),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(detail),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::Sampler(&detail_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::TextureView(shadow_mask),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::Sampler(&shadow_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 8,
                resource: wgpu::BindingResource::TextureView(&ground.map),
            },
            wgpu::BindGroupEntry {
                binding: 9,
                resource: wgpu::BindingResource::TextureView(&ground.pictures),
            },
            wgpu::BindGroupEntry {
                binding: 10,
                resource: wgpu::BindingResource::Sampler(&ground.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 11,
                resource: ground.info.as_entire_binding(),
            },
        ],
    })
}

/// The detail texture with its mips, as the game's own 0..255 values (not
/// sRGB), since it blends in the game's colour space.
fn detail_view(device: &wgpu::Device, queue: &wgpu::Queue, image: &mm2_formats::tga::Image) -> wgpu::TextureView {
    let (w, h) = (image.width.max(1), image.height.max(1));
    let mips = w.min(h).ilog2() + 1;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("detail"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: mips,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let mut level = image.rgba.clone();
    let (mut lw, mut lh) = (w, h);
    for mip in 0..mips {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: mip,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &level,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * lw),
                rows_per_image: Some(lh),
            },
            wgpu::Extent3d {
                width: lw,
                height: lh,
                depth_or_array_layers: 1,
            },
        );
        // A box filter down to the next level.
        let (nw, nh) = ((lw / 2).max(1), (lh / 2).max(1));
        let mut next = vec![0u8; (4 * nw * nh) as usize];
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let at = |dx: u32, dy: u32| {
                        let (sx, sy) = ((2 * x + dx).min(lw - 1), (2 * y + dy).min(lh - 1));
                        level[(4 * (sy * lw + sx) + c) as usize] as u32
                    };
                    next[(4 * (y * nw + x) + c) as usize] = ((at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1) + 2) / 4) as u8;
                }
            }
        }
        level = next;
        (lw, lh) = (nw, nh);
    }
    texture.create_view(&Default::default())
}

fn upload_textures(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    textures: &[Image],
) -> wgpu::Texture {
    let size = textures.iter().map(|t| t.size).max().unwrap_or(1);
    let layers = array_layers(textures.len().max(1) as u32, size, size);
    let mip_count = size.trailing_zeros() + 1;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("terrain textures"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: layers,
        },
        mip_level_count: mip_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (layer, tex) in textures.iter().enumerate() {
        // Smaller textures fill only the top mips; they are not used by tiles.
        let skip = (size / tex.size).trailing_zeros();
        for (level, data) in tex.levels.iter().enumerate() {
            let side = tex.size >> level;
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: level as u32 + skip,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                data,
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
        }
    }
    texture
}

/// A tile map value for a tile in a layer of four ([`fit_layers`]): the
/// top bit set, the quarter in the next two, the layer in the rest.
pub const PACKED: u16 = 0x8000;

/// The terrain textures to upload for the tiles in `tile_map`, which is
/// changed to match: each picture once (some tracks repeat the same one
/// under many names), and when there are still more than the GPU takes
/// in one array, four to a layer twice the size, each in a quarter
/// ([`PACKED`]; the shaders' `tile_layer`). A layer of four blurs into its
/// neighbours a little in the farthest mips, so it is only for those.
pub fn fit_layers(images: &[Image], tile_map: &mut [u16], most: u32) -> Vec<Image> {
    use std::collections::HashMap;
    let mut seen: HashMap<(u32, &[u8]), u16> = HashMap::new();
    let mut unique: Vec<Image> = Vec::new();
    let mut remap = vec![0xffffu16; images.len()];
    for (i, img) in images.iter().enumerate() {
        let Some(top) = img.levels.first() else { continue };
        remap[i] = *seen.entry((img.size, &top[..])).or_insert_with(|| {
            unique.push(img.clone());
            (unique.len() - 1) as u16
        });
    }
    for t in tile_map.iter_mut() {
        if let Some(&n) = remap.get(*t as usize) {
            *t = n;
        }
    }
    let size = unique.iter().map(|t| t.size).max().unwrap_or(1);
    if array_layers(unique.len().max(1) as u32, size, size) <= most {
        return unique;
    }
    eprintln!("{} terrain textures, more than the {most} this GPU takes in one array: four to a layer", unique.len());
    let fits = (most.saturating_sub(1).min(0x1fff) * 4) as usize;
    if unique.len() > fits {
        eprintln!("{} terrain textures still left out", unique.len() - fits);
    }
    // Every tile at the full size with its mips, as a box filter makes them.
    let full = |img: &Image| -> Vec<Vec<u8>> {
        if img.size == size {
            return img.levels.clone();
        }
        let k = (size / img.size.max(1)) as usize;
        let (from, side) = (&img.levels[0], img.size as usize);
        let mut top = vec![0u8; (size * size * 4) as usize];
        for y in 0..size as usize {
            for x in 0..size as usize {
                let at = ((y / k) * side + x / k) * 4;
                top[(y * size as usize + x) * 4..][..4].copy_from_slice(&from[at..at + 4]);
            }
        }
        let mut levels = vec![top];
        while (size >> (levels.len() - 1)) > 1 {
            let side = (size >> (levels.len() - 1)) as usize;
            levels.push(box_half(levels.last().unwrap(), side));
        }
        levels
    };
    let mut packed = Vec::new();
    for four in unique[..unique.len().min(fits)].chunks(4) {
        let quarters: Vec<Vec<Vec<u8>>> = four.iter().map(full).collect();
        let big = size * 2;
        let mut levels = Vec::new();
        for level in 0..quarters[0].len() {
            let side = (size >> level).max(1) as usize;
            let row = side * 2;
            let mut out = vec![0u8; row * row * 4];
            for (q, tile) in quarters.iter().enumerate() {
                let (qx, qy) = ((q & 1) * side, (q >> 1) * side);
                for y in 0..side {
                    let to = ((qy + y) * row + qx) * 4;
                    out[to..to + side * 4].copy_from_slice(&tile[level][y * side * 4..(y + 1) * side * 4]);
                }
            }
            levels.push(out);
        }
        // The one more mip a layer twice the size has.
        let last = levels.last().unwrap().clone();
        levels.push(box_half(&last, 2));
        packed.push(Image { size: big, levels });
    }
    for t in tile_map.iter_mut() {
        if *t == 0xffff {
            continue;
        }
        let n = *t as usize;
        *t = if n < fits { PACKED | (((n % 4) as u16) << 13) | (n / 4) as u16 } else { 0xffff };
    }
    packed
}

/// An RGBA8 image `side` square halved by a box filter.
fn box_half(level: &[u8], side: usize) -> Vec<u8> {
    let half = (side / 2).max(1);
    let mut out = vec![0u8; half * half * 4];
    for y in 0..half {
        for x in 0..half {
            for c in 0..4 {
                let at = |dx: usize, dy: usize| level[(((2 * y + dy).min(side - 1)) * side + (2 * x + dx).min(side - 1)) * 4 + c] as u32;
                out[(y * half + x) * 4 + c] = ((at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1) + 2) / 4) as u8;
            }
        }
    }
    out
}

/// The layer count to give a 2D array texture of `layers` images,
/// `width` by `height`. wgpu's OpenGL backend takes any square array
/// whose layer count is a multiple of six for a cube map (or cube map
/// array), and then a `D2Array` view of it samples nothing usable: the
/// sky showed the land's textures and some tracks' ground went black
/// (openmcm2 issue #15). One unused extra layer keeps it a plain array.
pub fn array_layers(layers: u32, width: u32, height: u32) -> u32 {
    if width == height && layers > 1 && layers % 6 == 0 {
        layers + 1
    } else {
        layers
    }
}

/// Converts game coordinates (left-handed, +z north) to the viewer's
/// right-handed world by flipping z, so the world is not mirrored.
/// No fog: the camera's own far plane (0x42f800's 100000).
const NO_FOG: (f32, f32) = (100_000.0, 100_000.0);

/// The fog's start and end, ft, as [`TerrainRenderer::set_visibility`].
pub fn fog_range(quality: u32, visibility: f32, haziness: f32) -> (f32, f32) {
    const FAR: f32 = 768.0;
    const NEAR: f32 = 256.0;
    const CLEAR: f32 = 128.0;
    let mut cut = (9 - quality.min(9)) as f32 * 0.1;
    if visibility - cut < 0.0 {
        cut = visibility;
    }
    let end = (FAR - NEAR) * (visibility - cut) + NEAR;
    let start = (1.0 - haziness) * (end - CLEAR);
    (start, end)
}

/// How high a stadium's lamps hang at least, ft, to be its floodlights.
const FLOODLIGHTS_ABOVE: f32 = 300.0;
/// Under a stadium's lamps at night, the light thrown back round about
/// against the day's ambient, and the haze against the day's.
const NIGHT_BOUNCE: f32 = 0.3;
const NIGHT_HAZE: f32 = 0.12;
/// How thick the lit air is on the usual presets, of Enhanced+'s: a hint of
/// light and shade in the air, not a haze.
const LIT_AIR_SUBTLE: f32 = 0.5;
const NIGHT_SKY: wgpu::Color = wgpu::Color { r: 0.006, g: 0.008, b: 0.016, a: 1.0 };

/// How strongly the ray-traced light shafts show (crate::rt).
const SHAFT_STRENGTH: f32 = 0.6;

/// How much of the sky's light all round (crate::ibl) the game's own
/// models take in place of their old even ambient, 0 to 1: gently, so
/// they keep their look.
const GAME_SKY_LIGHT: f32 = 0.5;

pub fn from_game(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.y, -v.z)
}

/// A camera that starts above the middle of the terrain, looking along it.
pub fn start_camera(terrain: &Terrain) -> crate::camera::Camera {
    let mid = (terrain.size / 2) as f32;
    let ground = terrain.height(terrain.size / 2, terrain.size / 2);
    let s = terrain.scale;
    crate::camera::Camera {
        position: from_game(Vec3::new(mid, ground + 60.0, mid - 160.0) * s),
        yaw: -std::f32::consts::FRAC_PI_2,
        pitch: -0.45,
        fov: crate::camera::FOV,
        roll: 0.0,
    }
}

#[cfg(test)]
mod fit_tests {
    use super::*;

    fn image(size: u32, shade: u8) -> Image {
        let mut levels = Vec::new();
        let mut side = size;
        loop {
            levels.push(vec![shade; (side * side * 4) as usize]);
            if side == 1 {
                break;
            }
            side /= 2;
        }
        Image { size, levels }
    }

    #[test]
    fn the_same_picture_is_uploaded_once() {
        let images: Vec<Image> = (0..6).map(|i| image(8, (i % 2) as u8)).collect();
        let mut map = vec![0, 1, 2, 3, 4, 5, 0xffff];
        let out = fit_layers(&images, &mut map, 256);
        assert_eq!(out.len(), 2);
        assert_eq!(map, [0, 1, 0, 1, 0, 1, 0xffff]);
    }

    #[test]
    fn too_many_go_four_to_a_layer() {
        let images: Vec<Image> = (0..10).map(|i| image(8, i as u8)).collect();
        let mut map: Vec<u16> = (0..10).collect();
        let out = fit_layers(&images, &mut map, 4);
        assert_eq!(out.len(), 3);
        assert!(out.iter().all(|i| i.size == 16 && i.levels.len() == 5));
        // Tile 5 is the second quarter of the second layer.
        assert_eq!(map[5], PACKED | (1 << 13) | 1);
        // Its texels sit in the top right quarter of that layer.
        let top = &out[1].levels[0];
        assert_eq!(top[(0 * 16 + 8) * 4], 5);
        assert_eq!(top[(8 * 16 + 0) * 4], 6);
    }
}
