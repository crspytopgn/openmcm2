//! What the enhanced drawing ([`crate::graphics`]) shares between the
//! terrain, the objects and the sky: the sun's shadow map in three
//! cascades, and the settings each shader reads.

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4Swizzles};
use wgpu::util::DeviceExt as _;

/// The cascades' depth, as the sun sees the objects.
pub const SHADOW_DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Texels a side of each cascade.
pub const SHADOW_SIDE: u32 = 2048;

/// Texels a side of each cascade with the upgrades as set: twice
/// [`SHADOW_SIDE`] on Ultra (crate::graphics::Enhanced::shadow_side).
pub fn shadow_side() -> u32 {
    crate::graphics::enhanced().map_or(SHADOW_SIDE, |e| e.shadow_side.clamp(512, 8192))
}
pub const CASCADES: usize = 3;
/// How far each cascade reaches from the camera, ft; past the last the
/// shadows fade out.
const SPLITS: [f32; CASCADES] = [36.0, 140.0, 520.0];
/// With the detail reaching further (crate::graphics::Enhanced::far):
/// the nearest the same, the far ones further out, so hills and trees
/// keep their shadows well into the distance.
const FAR_SPLITS: [f32; CASCADES] = [36.0, 170.0, 950.0];

/// The cascades' reach with the upgrades as set.
fn reach() -> [f32; CASCADES] {
    if crate::graphics::enhanced().is_some_and(|e| e.far) {
        FAR_SPLITS
    } else {
        SPLITS
    }
}
/// How far beyond a cascade toward the sun a caster may stand, ft.
const SUNWARD: f32 = 1500.0;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct EnhGlobals {
    /// From the drawing to each cascade's map.
    pub cascades: [[[f32; 4]; 4]; CASCADES],
    /// Each cascade's reach, and in w where the shadows end.
    pub splits: [f32; 4],
    /// Which upgrades show: x lighting, y shadows, z haze, w sky.
    pub flags: [f32; 4],
    /// The haze: where it starts, how thick it is (per ft) and the far
    /// plane; w how strongly the new ground's detail shows.
    pub haze: [f32; 4],
    /// The sun's colour and the way to it, and the fog's colour (linear).
    pub sun: [f32; 4],
    pub to_sun: [f32; 4],
    pub fog: [f32; 4],
    /// Each cascade's texel, ft.
    pub texel: [f32; 4],
    /// The clock (s), how wet the ground is, how hard it rains, and how
    /// much of the sky is cloud, 0 to 1.
    pub world: [f32; 4],
    /// The ruts' map: where its corner is (x, z, drawing coordinates), how
    /// many ft it spans, and whether it is drawn.
    pub ruts: [f32; 4],
    /// Which of the bigger upgrades show: x the new ground, y grass, z the
    /// real sky, w the bikes' paint.
    pub more: [f32; 4],
    /// The photo materials' average colours (linear).
    pub material_average: [[f32; 4]; crate::materials::COUNT],
    /// The real sky: x the screen's width over its height, y whether the
    /// bikes get muddy, z cloud drift (ft), w the exposure.
    pub sky: [f32; 4],
    /// The sky's colour overhead and at the horizon, linear.
    pub zenith: [f32; 4],
    pub horizon: [f32; 4],
    /// The smooth ground's patch: its corner (terrain units), cells a
    /// side, and whether it is drawn.
    pub smooth: [f32; 4],
    /// x the objects' photo materials, y swaying trees, z round bikes and
    /// riders, w a stadium's floodlights.
    pub surfaces: [f32; 4],
    /// x whether the shadows are ray traced; y whether dirt builds up on
    /// the bikes and riders (crate::grime).
    pub traced: [f32; 4],
    /// From the drawing to the screen, for the ray-traced shadows.
    pub view_proj: [[f32; 4]; 4],
    /// The ambient light's colour, for what the reflections see.
    pub ambient: [f32; 4],
    /// From the drawing to the dust's shadow map (crate::dust), and x
    /// whether it is drawn, y the least spread of a cloud's depth in it.
    pub dust_view: [[f32; 4]; 4],
    pub dust: [f32; 4],
    /// The terrain for the reflections: ft a terrain unit, and terrain
    /// units the ground's colour map spans.
    pub terrain: [f32; 4],
    /// x how much it is night; yzw the track's own light on level ground.
    pub night: [f32; 4],
    /// x whether the bikes' headlights are traced, y how many glow.
    pub headlights: [f32; 4],
    /// Each glowing headlight: its place and how strongly (drawing
    /// coordinates).
    pub lamps: [[f32; 4]; LAMPS],
    /// The light poles' baked light (crate::flood): its map's corner (x,
    /// z) and the ft it spans across and down; 0 for none.
    pub flood: [f32; 4],
    /// The sky's light baked over the land (crate::skylight): samples a
    /// side, ft between them, how strongly it shows (0 off), and x whether
    /// the ground marks its water for the screen's reflections (w).
    pub sky_light: [f32; 4],
    /// The baked map's heights: the lowest (ft) and the range over them.
    pub sky_height: [f32; 4],
    /// A stadium's own lights at night (crate::night): x how many, y how
    /// much they light (0 off), z the lowest lamp's height (ft), w how
    /// many shadow groups are drawn into the cascades' layers.
    pub stadium: [f32; 4],
    /// From the drawing to each shadow group's map, and the ft a texel
    /// of each spans on the floor.
    pub stadium_views: [[[f32; 4]; 4]; crate::night::GROUPS],
    pub stadium_texels: [f32; 4],
    /// Each lamp: xyz where, w its power; xyz the way it points, w its
    /// beam's edge (a cosine) plus 10 times its shadow group.
    pub stadium_lamps: [[f32; 4]; crate::night::MOST_LAMPS],
    pub stadium_ways: [[f32; 4]; crate::night::MOST_LAMPS],
    /// The sky's light all round (crate::ibl): x whether the physically
    /// based materials take it, y the cube's roughest level, z how much of
    /// it the game's own models take (0 their old light), w unused.
    pub ibl: [f32; 4],
    /// Its irradiance as harmonics (crate::ibl::bake).
    pub sh: [[f32; 4]; 9],
}

/// What the shared group is made of, kept to make it again.
pub struct Parts {
    buffer: wgpu::Buffer,
    shadows: wgpu::TextureView,
    shadow_sampler: wgpu::Sampler,
    albedo: wgpu::TextureView,
    normal: wgpu::TextureView,
    repeat: wgpu::Sampler,
    ruts: wgpu::TextureView,
    clamp: wgpu::Sampler,
    /// The dust clouds' shadows, as the sun sees them.
    dust: wgpu::TextureView,
    /// The light poles' baked light (crate::flood), or a blank.
    flood: wgpu::TextureView,
    /// The sky's light over the land (crate::skylight), or a blank.
    sky: wgpu::TextureView,
    /// The sky's light all round (crate::ibl), or a blank cube.
    env: wgpu::TextureView,
    /// Its sampler: smooth between levels.
    env_sampler: wgpu::Sampler,
}

/// Texels a side of the dust's shadow map.
pub const DUST_SIDE: u32 = 512;
/// What the dust's shadow map holds: how thick, and how deep (its sum and
/// its square's), added up.
pub const DUST_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// The dust's shadow map's binding in the shared group, and its sampler's.
const DUST_BINDING: u32 = 30;
/// The light poles' baked light's binding, and its sampler's.
const FLOOD_BINDING: u32 = 32;
/// The sky's light over the land's binding (read with the flood's sampler).
const SKY_BINDING: u32 = 34;
/// The sky's light all round (crate::ibl), and its sampler's.
const ENV_BINDING: u32 = 36;

impl Parts {
    /// The shared group, with the ray-traced scene `traced` if drawn.
    fn group(&self, device: &wgpu::Device, layout: &wgpu::BindGroupLayout, traced: Option<&crate::rt::Scene>) -> wgpu::BindGroup {
        use wgpu::BindingResource as R;
        let mut resources = vec![
            self.buffer.as_entire_binding(),
            R::TextureView(&self.shadows),
            R::Sampler(&self.shadow_sampler),
            R::TextureView(&self.albedo),
            R::TextureView(&self.normal),
            R::Sampler(&self.repeat),
            R::TextureView(&self.ruts),
            R::Sampler(&self.clamp),
        ];
        resources.extend(traced.into_iter().flat_map(|t| t.bindings()));
        let mut entries: Vec<wgpu::BindGroupEntry> = resources
            .into_iter()
            .zip(0..)
            .map(|(resource, binding)| wgpu::BindGroupEntry { binding, resource })
            .collect();
        entries.push(wgpu::BindGroupEntry {
            binding: DUST_BINDING,
            resource: R::TextureView(&self.dust),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: DUST_BINDING + 1,
            resource: R::Sampler(&self.clamp),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: FLOOD_BINDING,
            resource: R::TextureView(&self.flood),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: FLOOD_BINDING + 1,
            resource: R::Sampler(&self.clamp),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: SKY_BINDING,
            resource: R::TextureView(&self.sky),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: ENV_BINDING,
            resource: R::TextureView(&self.env),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: ENV_BINDING + 1,
            resource: R::Sampler(&self.env_sampler),
        });
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("enhanced"),
            layout,
            entries: &entries,
        })
    }
}

/// The enhanced drawing's shared parts.
pub struct Shared {
    /// The dust's shadow map, to draw the puffs into from the sun.
    pub dust: wgpu::TextureView,
    parts: Parts,
    pub layout: wgpu::BindGroupLayout,
    pub buffer: wgpu::Buffer,
    pub group: wgpu::BindGroup,
    /// Each cascade's layer of the map, to draw the casters into.
    pub layers: Vec<wgpu::TextureView>,
    pub material_average: [[f32; 4]; crate::materials::COUNT],
    /// The ruts' map, to draw the tyres into: its view and texels a side.
    pub ruts: (wgpu::TextureView, u32),
    /// Texels a side of each cascade's layer.
    pub side: u32,
}

/// The most headlights that glow on the bikes.
pub const LAMPS: usize = 64;

/// Texels a side of the ruts' map, over the whole terrain.
pub const RUT_SIDE: u32 = 8192;
pub const RUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

impl Shared {
    /// Makes the group again for the ray-traced scene as it is now.
    pub fn set_traced(&mut self, device: &wgpu::Device, traced: &crate::rt::Scene) {
        self.group = self.parts.group(device, &self.layout, Some(traced));
    }

    /// Makes the group again with the light poles' baked light `map`
    /// (crate::flood), and the ray-traced scene `traced` if drawn.
    pub fn set_flood(&mut self, device: &wgpu::Device, map: &wgpu::TextureView, traced: Option<&crate::rt::Scene>) {
        self.parts.flood = map.clone();
        self.group = self.parts.group(device, &self.layout, traced);
    }

    /// Makes the group again with the sky's light all round `env`
    /// (crate::ibl), and the ray-traced scene `traced` if drawn.
    pub fn set_env(&mut self, device: &wgpu::Device, env: wgpu::TextureView, traced: Option<&crate::rt::Scene>) {
        self.parts.env = env;
        self.group = self.parts.group(device, &self.layout, traced);
    }

    /// Makes the group again with the sky's light over the land `sky`
    /// (crate::skylight), and the ray-traced scene `traced` if drawn.
    pub fn set_sky(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sky: &crate::skylight::SkyLight,
        traced: Option<&crate::rt::Scene>,
    ) {
        self.parts.sky = device
            .create_texture_with_data(
                queue,
                &wgpu::TextureDescriptor {
                    label: Some("sky light"),
                    size: wgpu::Extent3d {
                        width: sky.side,
                        height: sky.side,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                bytemuck::cast_slice(&sky.texels),
            )
            .create_view(&Default::default());
        self.group = self.parts.group(device, &self.layout, traced);
    }

    /// With `ruts`, a map of tyre ruts over the terrain; with `rt`, the
    /// scene for the ray-traced shadows.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, ruts: bool, rt: Option<&crate::rt::Scene>) -> Shared {
        let mut entries = vec![
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    // Also for the dust's corners (crate::dust).
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: texture(wgpu::TextureViewDimension::D2Array, wgpu::TextureSampleType::Float { filterable: true }),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: texture(wgpu::TextureViewDimension::D2Array, wgpu::TextureSampleType::Float { filterable: true }),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: sampler(),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: texture(wgpu::TextureViewDimension::D2, wgpu::TextureSampleType::Float { filterable: true }),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: sampler(),
                    count: None,
                },
        ];
        if rt.is_some() {
            use wgpu::{TextureSampleType as T, TextureViewDimension as D};
            let storage = wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            };
            let float = T::Float { filterable: true };
            // As rt::Scene::bindings gives them.
            let types = [
                texture(D::D2, T::Float { filterable: false }),
                wgpu::BindingType::AccelerationStructure { vertex_return: false },
                storage,
                texture(D::D2Array, float),
                sampler(),
                storage,
                storage,
                storage,
                texture(D::D2Array, float),
                texture(D::D2, float),
                storage,
                storage,
                texture(D::D2, T::Float { filterable: false }),
            ];
            entries.extend(types.into_iter().zip(8..).map(|(ty, binding)| wgpu::BindGroupLayoutEntry {
                binding,
                // The scene and the traced shadows also for the dust's
                // vertices, to ask whether each puff is in the sun.
                visibility: if binding == 8 || binding == 9 { wgpu::ShaderStages::VERTEX_FRAGMENT } else { wgpu::ShaderStages::FRAGMENT },
                ty,
                count: None,
            }));
        }
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: DUST_BINDING,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: texture(wgpu::TextureViewDimension::D2, wgpu::TextureSampleType::Float { filterable: true }),
            count: None,
        });
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: DUST_BINDING + 1,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: sampler(),
            count: None,
        });
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: FLOOD_BINDING,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: texture(wgpu::TextureViewDimension::D2, wgpu::TextureSampleType::Float { filterable: true }),
            count: None,
        });
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: FLOOD_BINDING + 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: sampler(),
            count: None,
        });
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: SKY_BINDING,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: texture(
                wgpu::TextureViewDimension::D2,
                wgpu::TextureSampleType::Float { filterable: true },
            ),
            count: None,
        });
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: ENV_BINDING,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: texture(wgpu::TextureViewDimension::Cube, wgpu::TextureSampleType::Float { filterable: true }),
            count: None,
        });
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: ENV_BINDING + 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: sampler(),
            count: None,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("enhanced"),
            entries: &entries,
        });
        let materials = crate::materials::Materials::new(device, queue);
        let rut_side = if ruts { RUT_SIDE } else { 1 };
        let rut_map = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ruts"),
            size: wgpu::Extent3d {
                width: rut_side,
                height: rut_side,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: RUT_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let rut_view = rut_map.create_view(&Default::default());
        let repeat = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ground materials"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 16,
            ..Default::default()
        });
        let clamp = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ruts"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("enhanced"),
            size: std::mem::size_of::<EnhGlobals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let side = shadow_side();
        let map = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sun shadows"),
            size: wgpu::Extent3d {
                width: side,
                height: side,
                depth_or_array_layers: CASCADES as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SHADOW_DEPTH,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let layers = (0..CASCADES as u32)
            .map(|layer| {
                map.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: layer,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sun shadows"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let dust = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("dust shadows"),
                size: wgpu::Extent3d {
                    width: DUST_SIDE,
                    height: DUST_SIDE,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DUST_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let flood = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("no flood"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let sky = device
            .create_texture_with_data(
                queue,
                &wgpu::TextureDescriptor {
                    label: Some("no sky light"),
                    size: wgpu::Extent3d {
                        width: 1,
                        height: 1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                &[255, 0, 0, 255],
            )
            .create_view(&Default::default());
        // A grey sky till the track's is baked.
        let env = crate::ibl::upload(
            device,
            queue,
            &crate::ibl::Baked {
                levels: vec![vec![[0.2; 3]; 6]],
                sh: [[0.0; 4]; 9],
            },
        );
        let env_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sky's light"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let parts = Parts {
            flood,
            sky,
            env,
            env_sampler,
            dust: dust.clone(),
            buffer: buffer.clone(),
            shadows: map.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            }),
            shadow_sampler: sampler,
            albedo: materials.albedo.clone(),
            normal: materials.normal.clone(),
            repeat,
            ruts: rut_view.clone(),
            clamp,
        };
        let group = parts.group(device, &layout, rt);
        Shared {
            side,
            dust,
            parts,
            layout,
            buffer,
            group,
            layers,
            material_average: materials.average,
            ruts: (rut_view, rut_side),
        }
    }
}

/// The cascades for a camera at `view` with `fovy` and `aspect`, lit from
/// `to_sun`, in maps `side` texels a side: each a box along the light round a slice of what the camera
/// sees, moved only in whole texels so the shadows' edges hold still as
/// the camera moves. Returns each one's matrix and its texel, ft.
pub fn cascades(view: Mat4, fovy: f32, aspect: f32, to_sun: Vec3, side: u32) -> ([Mat4; CASCADES], [f32; CASCADES]) {
    let inverse = view.inverse();
    let (ty, tx) = ((fovy / 2.0).tan(), (fovy / 2.0).tan() * aspect);
    let up = if to_sun.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y };
    let light = Mat4::look_at_rh(Vec3::ZERO, -to_sun, up);
    let mut near = 0.5;
    let mut out = [Mat4::IDENTITY; CASCADES];
    let mut texels = [0.0; CASCADES];
    for (i, &far) in reach().iter().enumerate() {
        // The slice's bounding sphere, so its size never changes as the
        // camera turns.
        let corners: Vec<Vec3> = [near, far]
            .iter()
            .flat_map(|&d| {
                [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
                    .map(|(sx, sy)| (inverse * glam::Vec4::new(sx * tx * d, sy * ty * d, -d, 1.0)).xyz())
            })
            .collect();
        let centre = corners.iter().copied().sum::<Vec3>() / corners.len() as f32;
        let radius = corners.iter().map(|c| c.distance(centre)).fold(0.0, f32::max);
        let radius = (radius * 16.0).ceil() / 16.0;
        let texel = 2.0 * radius / side as f32;
        // Snap the centre to the texel grid as the light sees it.
        let mut c = light.transform_point3(centre);
        c.x = (c.x / texel).round() * texel;
        c.y = (c.y / texel).round() * texel;
        let snapped = light.inverse().transform_point3(c);
        let eye = snapped + to_sun * (radius + SUNWARD);
        let look = Mat4::look_at_rh(eye, snapped, up);
        let proj = Mat4::orthographic_rh(-radius, radius, -radius, radius, 0.0, 2.0 * radius + SUNWARD);
        out[i] = proj * look;
        texels[i] = texel;
        near = far;
    }
    (out, texels)
}

/// What each sun cascade need hold, for leaving the rest of the scenery
/// out of it: each cascade's view, narrowed across to the part of its map
/// that is ever read. A cascade is only read for what the camera sees
/// within its reach (sun_visible picks it by the distance from the eye),
/// so only for what lies in the camera's view short of that far; round
/// that, as far as the soft shadows' search and blur and the lift off the
/// surface reach, and a little more. An object outside it shades nothing
/// that is drawn, so the picture is the same without it. For culling
/// only: the cascades are still drawn through their own views.
pub fn cascade_reach(view: Mat4, fovy: f32, aspect: f32, cascades: &[Mat4; CASCADES], texels: &[f32; CASCADES]) -> [Mat4; CASCADES] {
    let inverse = view.inverse();
    let (ty, tx) = ((fovy / 2.0).tan(), (fovy / 2.0).tan() * aspect);
    let mut out = *cascades;
    for (i, &far) in reach().iter().enumerate() {
        let c = cascades[i];
        let texel = texels[i];
        // The eye and the four corners of the view as far as the reach.
        let mut points = vec![inverse.transform_point3(Vec3::ZERO)];
        for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            points.push((inverse * glam::Vec4::new(sx * tx * far, sy * ty * far, -far, 1.0)).xyz());
        }
        let (mut lo, mut hi) = (glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN));
        for p in points {
            let q = c.project_point3(p).truncate();
            lo = lo.min(q);
            hi = hi.max(q);
        }
        // The map's side in ft is 2 / its view's x scale; the soft
        // shadows reach 6 + 8 texels of a 2048 map whatever its size, the
        // plain filter one, the lift two and a half and 0.4 ft.
        let side_ft = 2.0 / c.x_axis.truncate().length().max(1e-9);
        let margin_ft = 16.0 * side_ft / 2048.0 + 4.0 * texel + 1.0;
        let margin = 2.0 * margin_ft / side_ft;
        let (lo, hi) = ((lo - margin).max(glam::Vec2::splat(-1.0)), (hi + margin).min(glam::Vec2::splat(1.0)));
        if !(lo.x < hi.x && lo.y < hi.y) || !lo.is_finite() || !hi.is_finite() {
            continue;
        }
        // Maps lo..hi across onto -1..1, depth as it was.
        let scale = (2.0 / (hi - lo)).extend(1.0);
        let shift = (-(hi + lo) / (hi - lo)).extend(0.0);
        out[i] = Mat4::from_translation(shift) * Mat4::from_scale(scale) * c;
    }
    out
}

/// Each cascade's reach, and where the shadows end.
pub fn splits() -> [f32; 4] {
    let s = reach();
    [s[0], s[1], s[2], s[2]]
}

/// `source` with the enhanced drawing's shared declarations put in where
/// it asks for them, ray traced (`traced`) or not: what [`shader`] makes.
pub fn full_source(source: &str, traced: bool) -> String {
    let mut source = source
        .replace("#include \"enhanced.wgsl\"", include_str!("enhanced.wgsl"))
        .replace("#include \"enhanced_fns.wgsl\"", include_str!("enhanced_fns.wgsl"));
    if traced {
        source = ray_traced(&source);
    }
    source = dusty(&source);
    flooded(&source)
}

/// Makes a shader from `source`, with the enhanced drawing's shared
/// declarations put in where it asks for them.
pub fn shader(device: &wgpu::Device, label: &str, source: &str) -> wgpu::ShaderModule {
    let source = full_source(source, crate::graphics::ray_traced());
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    })
}

/// `source` with the sun's shadows ray traced (crate::rt), where it reads
/// the shadow maps: the traced shadows bound beside them in the same
/// group, and `sun_visible` reading them, the maps' own kept as
/// `sun_mapped`.
fn ray_traced(source: &str) -> String {
    let Some(line) = source.lines().find(|l| l.contains("var sun_map: texture_depth_2d_array")) else {
        return source.to_string();
    };
    let group = line
        .split("@group(")
        .nth(1)
        .and_then(|r| r.split(')').next())
        .unwrap_or("1");
    let declared = [
        "rt_shadows: texture_2d<f32>",
        "rt_scene: acceleration_structure",
        "<storage, read> rt_cutouts: array<vec4<f32>>",
        "rt_masks: texture_2d_array<f32>",
        "rt_sampler: sampler",
        "<storage, read> rt_vertices: array<u32>",
        "<storage, read> rt_nodes: array<mat4x4<f32>>",
        "<storage, read> rt_hits: array<vec4<u32>>",
        "rt_colours: texture_2d_array<f32>",
        "rt_ground: texture_2d<f32>",
        "<storage, read> rt_points: array<f32>",
        "<storage, read> rt_terrain: array<u32>",
        "rt_lamps: texture_2d<f32>",
    ];
    let mut bindings = line.to_string();
    for (d, binding) in declared.iter().zip(8..) {
        let (space, rest) = match d.strip_prefix('<') {
            Some(r) => {
                let (s, rest) = r.split_once("> ").unwrap();
                (format!("<{s}>"), rest)
            }
            None => (String::new(), *d),
        };
        bindings.push_str(&format!("\n@group({group}) @binding({binding}) var{space} {rest};"));
    }
    let source = source
        .replacen(line, &bindings, 1)
        .replacen("fn sun_visible(", "fn sun_mapped(", 1)
        .replacen("fn reflection(", "fn reflection_sky(", 1)
        .replacen("fn sun_point(", "fn sun_point_mapped(", 1)
        .replacen("fn headlight_at(", "fn headlight_none(", 1)
        .replacen("fn headlight_soft(", "fn headlight_soft_none(", 1);
    format!("enable wgpu_ray_query;\n{source}\n{}", include_str!("enhanced_rt.wgsl"))
}

/// `source` with the light poles' baked light (crate::flood) bound beside
/// the sun's shadows, for `headlight_at` without ray tracing.
fn flooded(source: &str) -> String {
    let Some(line) = source.lines().find(|l| l.contains("var sun_map: texture_depth_2d_array")) else {
        return source.to_string();
    };
    let group = line
        .split("@group(")
        .nth(1)
        .and_then(|r| r.split(')').next())
        .unwrap_or("1");
    let bindings = format!(
        "{line}\n@group({group}) @binding({FLOOD_BINDING}) var flood_map: texture_2d<f32>;\n@group({group}) @binding({}) var flood_sampler: sampler;\n@group({group}) @binding({SKY_BINDING}) var sky_map: texture_2d<f32>;\n@group({group}) @binding({ENV_BINDING}) var env_map: texture_cube<f32>;\n@group({group}) @binding({}) var env_sampler: sampler;",
        FLOOD_BINDING + 1,
        ENV_BINDING + 1
    );
    source.replacen(line, &bindings, 1)
}

/// `source` with the dust clouds' shadows over the sun's, where it reads
/// the sun's shadows: the dust's map bound beside them, and `sun_visible`
/// the sun's own (kept as `sun_solid`) through the dust.
fn dusty(source: &str) -> String {
    let Some(line) = source.lines().find(|l| l.contains("var sun_map: texture_depth_2d_array")) else {
        return source.to_string();
    };
    let group = line
        .split("@group(")
        .nth(1)
        .and_then(|r| r.split(')').next())
        .unwrap_or("1");
    let bindings = format!(
        "{line}\n@group({group}) @binding({DUST_BINDING}) var dust_map: texture_2d<f32>;\n@group({group}) @binding({}) var dust_sampler: sampler;",
        DUST_BINDING + 1
    );
    let source = source.replacen(line, &bindings, 1).replacen("fn sun_visible(", "fn sun_solid(", 1);
    format!("{source}\n{}", include_str!("enhanced_dust.wgsl"))
}

/// A bind group layout of `entries`: each binding, where it is seen, and
/// what it is. The enhanced pipelines name their layouts, since a layout
/// a pipeline works out for itself fits no other pipeline.
pub fn layout(device: &wgpu::Device, label: &str, entries: &[(u32, wgpu::ShaderStages, wgpu::BindingType)]) -> wgpu::BindGroupLayout {
    let entries: Vec<wgpu::BindGroupLayoutEntry> = entries
        .iter()
        .map(|&(binding, visibility, ty)| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty,
            count: None,
        })
        .collect();
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &entries,
    })
}

/// A bind group layout's entry.
pub fn entry(binding: u32, visibility: wgpu::ShaderStages, ty: wgpu::BindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry { binding, visibility, ty, count: None }
}

pub fn uniform() -> wgpu::BindingType {
    wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Uniform,
        has_dynamic_offset: false,
        min_binding_size: None,
    }
}

pub fn texture(dimension: wgpu::TextureViewDimension, sample_type: wgpu::TextureSampleType) -> wgpu::BindingType {
    wgpu::BindingType::Texture {
        sample_type,
        view_dimension: dimension,
        multisampled: false,
    }
}

pub fn sampler() -> wgpu::BindingType {
    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)
}

/// The enhanced drawing's light and sky for the hour and weather it is
/// asked for ([`crate::graphics::Enhanced`]): none of it the game's.
pub struct Daylight {
    pub lights: crate::renderer::SceneLights,
    /// The sky's colour overhead and at the horizon (linear).
    pub zenith: Vec3,
    pub horizon: Vec3,
    /// How much of the sky is cloud, how wet the ground is and how hard
    /// it rains, each 0 to 1.
    pub cover: f32,
    pub wet: f32,
    pub rain: f32,
    /// How much it is night, 0 to 1: the sun down and the moon up.
    pub night: f32,
}

/// The light and sky from the scene's own `lights` at `time` (an hour,
/// 0-24; at the scene's own sun when none) in `weather`.
pub fn daylight(lights: &crate::renderer::SceneLights, time: Option<f32>, weather: crate::graphics::Weather) -> Daylight {
    use crate::graphics::Weather;
    let mut l = *lights;
    let sun_strength = Vec3::from(lights.sun).max_element().max(0.3);
    if let Some(hour) = time {
        // Up at 6 and down at 18.6, highest at half past noon; round from
        // the east through the south as the scene's sun is at noon.
        let flat = Vec3::new(lights.to_sun.x, 0.0, lights.to_sun.z).normalize_or(Vec3::X);
        let day = ((hour - 6.0) / 12.6).clamp(-0.1, 1.1);
        let high = (day * std::f32::consts::PI).sin() * 62f32.to_radians();
        let round = (day - 0.5) * std::f32::consts::PI * 0.9;
        let (s, c) = round.sin_cos();
        let way = Vec3::new(flat.x * c - flat.z * s, 0.0, flat.x * s + flat.z * c);
        l.to_sun = (way * high.cos() + Vec3::Y * high.sin()).normalize();
        let up = l.to_sun.y;
        // Low, the sun is redder and weaker; the ambient bluer and dimmer.
        let warm = Vec3::new(1.0, 0.55, 0.32).lerp(Vec3::new(1.0, 0.96, 0.9), smoothstep(0.02, 0.45, up));
        let strength = sun_strength * smoothstep(-0.04, 0.18, up);
        l.sun = (warm * strength).into();
        let ambient = Vec3::from(lights.ambient).max_element().max(0.25);
        let tint = Vec3::new(0.75, 0.8, 1.0).lerp(Vec3::ONE, smoothstep(0.05, 0.4, up));
        l.ambient = (tint * ambient * (0.5 + 0.5 * smoothstep(-0.1, 0.3, up))).into();
    }
    // After dusk and before dawn the moon lights the scene: high and to
    // the side away from where the sun set, faint and blue, and the sky's
    // light round about fainter still.
    // The sky's colour goes by where the sun is, not the moon.
    let sun_up = l.to_sun.y;
    let night = smoothstep(-0.02, -0.15, sun_up);
    if night > 0.0 {
        let flat = Vec3::new(lights.to_sun.x, 0.0, lights.to_sun.z).normalize_or(Vec3::X);
        let moon = (Vec3::new(-flat.z, 0.0, flat.x) * 0.7 + Vec3::Y * 0.75).normalize();
        l.to_sun = moon.lerp(l.to_sun, 1.0 - night).normalize();
        // Without ray tracing there are no headlights: the moon brighter,
        // so the track still shows.
        let lift = if crate::graphics::ray_traced() { 1.0 } else { 2.5 };
        l.sun = (Vec3::from(l.sun).lerp(MOON * lift, night)).into();
        l.ambient = (Vec3::from(l.ambient).lerp(NIGHT_AMBIENT * lift, night)).into();
    }
    let (cover, wet, rain) = match weather {
        Weather::Clear => (0.42, 0.0, 0.0),
        Weather::Overcast => (0.85, 0.15, 0.0),
        Weather::Rain => (0.97, 1.0, 1.0),
    };
    // Under cloud, less sun and more of the sky's light round about.
    let dim = 1.0 - smoothstep(0.5, 1.0, cover) * 0.7;
    let sun = Vec3::from(l.sun);
    l.sun = (sun * dim).into();
    l.ambient = (Vec3::from(l.ambient) + sun * (1.0 - dim) * 0.35).into();
    let up = sun_up;
    let day = smoothstep(-0.08, 0.25, up);
    let dusk = smoothstep(0.35, 0.02, up) * smoothstep(-0.12, 0.0, up);
    let zenith = Vec3::new(0.02, 0.025, 0.06).lerp(Vec3::new(0.13, 0.27, 0.62), day);
    let horizon = Vec3::new(0.03, 0.035, 0.06).lerp(Vec3::new(0.58, 0.68, 0.82), day).lerp(Vec3::new(0.95, 0.5, 0.28), dusk * 0.8);
    let grey = Vec3::splat(0.5 * (0.08 + 0.92 * day)) * Vec3::new(0.95, 0.98, 1.05);
    let overcast = smoothstep(0.5, 1.0, cover);
    Daylight {
        lights: l,
        zenith: zenith.lerp(grey * 0.9, overcast),
        horizon: horizon.lerp(grey, overcast),
        cover,
        wet,
        rain,
        night,
    }
}

/// The moon's light and the night sky's, linear (0..1 of the game's).
const MOON: Vec3 = Vec3::new(0.09, 0.11, 0.17);
const NIGHT_AMBIENT: Vec3 = Vec3::new(0.035, 0.04, 0.065);

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
