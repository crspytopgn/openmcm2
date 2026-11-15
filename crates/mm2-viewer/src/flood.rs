//! The light poles' light over the ground at night, baked once a race
//! (flood.wgsl): a map over everywhere the poles reach, holding the light
//! every pole puts on the ground there, with the shadows of all that does
//! not move. The poles, the hills, the trees and the props never move, so
//! tracing that light anew each frame was wasted; each frame now only the
//! riders' shadows in it are traced (rt_shadow.wgsl), and without ray
//! tracing the ground reads the map as it is (enhanced_fns.wgsl).
//!
//! With ray tracing the bake traces rays to points across each pole's
//! bank of lamps, so its shadows are soft and the trees' leaves dapple
//! them; without, it steps over the terrain's heights, so the hills cast
//! but what stands on them does not.

use glam::Vec3;
use mm2_formats::trn::Terrain;
use wgpu::util::DeviceExt;

/// How far a pole's light reaches, ft (lamp_light.wgsl POLE_REACH).
const POLE_REACH: f32 = 260.0;
/// The most texels a side, and the finest a texel may be, ft.
const MOST_TEXELS: f32 = 2048.0;
const FINEST: f32 = 1.0;
/// Rays traced to each pole's bank of lamps from each texel.
const RAYS: f32 = 8.0;
/// Rows of texels baked in each submission: small enough that no one of
/// them keeps the graphics card busy long (the system resets a card that
/// stalls for two seconds).
const ROWS: u32 = 64;

/// What flood.wgsl's `Flood` holds.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    corner: [f32; 4],
    size: [f32; 4],
    grid: [f32; 4],
}

/// The bake's shader without ray tracing: the beams, the bake, the hills.
pub fn hills_source() -> String {
    format!("{}\n{}\n{}", include_str!("lamp_light.wgsl"), include_str!("flood.wgsl"), include_str!("flood_hills.wgsl"))
}

pub struct FloodMap {
    /// The map's corner (x, z, drawing coordinates) and ft a texel.
    corner: (f32, f32),
    texel: f32,
    size: (u32, u32),
    terrain: (f32, f32),
    pole_count: u32,
    heights: wgpu::Buffer,
    uniform: wgpu::Buffer,
    poles: wgpu::Buffer,
    map: wgpu::TextureView,
    baked: bool,
}

impl FloodMap {
    /// The map for the poles `poles` (each lamp head's place and the way
    /// the pole faces, drawing coordinates) over `terrain`; none without
    /// poles.
    pub fn new(device: &wgpu::Device, terrain: &Terrain, poles: &[(Vec3, Vec3)]) -> Option<FloodMap> {
        if poles.is_empty() {
            return None;
        }
        let s = terrain.scale;
        let side = (terrain.size - 1) as f32 * s;
        // Round the poles, as far as they reach, within the terrain (whose
        // rows run along -z).
        let (mut low, mut high) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for (at, _) in poles {
            low = low.min(*at - POLE_REACH);
            high = high.max(*at + POLE_REACH);
        }
        let (x0, x1) = (low.x.max(0.0), high.x.min(side));
        let (z0, z1) = (low.z.max(-side), high.z.min(0.0));
        if x1 <= x0 || z1 <= z0 {
            return None;
        }
        let texel = ((x1 - x0).max(z1 - z0) / MOST_TEXELS).max(FINEST);
        let size = (((x1 - x0) / texel).ceil() as u32, ((z1 - z0) / texel).ceil() as u32);
        let heights: Vec<f32> = terrain.heights.iter().map(|h| h * s).collect();
        let lamps: Vec<[f32; 4]> = poles
            .iter()
            .flat_map(|(at, facing)| {
                let way = crate::rt::pole_way(*facing);
                [[at.x, at.y, at.z, 1.0], [way.x, way.y, way.z, 0.0]]
            })
            .collect();
        let map = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("flood map"),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let flood = FloodMap {
            corner: (x0, z0),
            texel,
            size,
            terrain: (terrain.size as f32, s),
            pole_count: poles.len() as u32,
            heights: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("flood heights"),
                contents: bytemuck::cast_slice(&heights),
                usage: wgpu::BufferUsages::STORAGE,
            }),
            uniform: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("flood"),
                size: std::mem::size_of::<Uniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            poles: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("flood poles"),
                contents: bytemuck::cast_slice(&lamps),
                usage: wgpu::BufferUsages::STORAGE,
            }),
            map,
            baked: false,
        };
        Some(flood)
    }

    pub fn baked(&self) -> bool {
        self.baked
    }

    /// The map's corner (x, z) and the ft it spans across and down, for
    /// the scene's shaders; all 0 till it is baked.
    pub fn area(&self) -> [f32; 4] {
        if !self.baked {
            return [0.0; 4];
        }
        [self.corner.0, self.corner.1, self.size.0 as f32 * self.texel, self.size.1 as f32 * self.texel]
    }

    pub fn map(&self) -> &wgpu::TextureView {
        &self.map
    }

    pub fn heights(&self) -> &wgpu::Buffer {
        &self.heights
    }

    pub fn uniform(&self) -> &wgpu::Buffer {
        &self.uniform
    }

    fn write(&self, queue: &wgpu::Queue, first: u32, rows: u32) {
        let u = Uniform {
            corner: [self.corner.0, self.corner.1, self.texel, self.baked as u32 as f32],
            size: [self.size.0 as f32, self.size.1 as f32, first as f32, rows as f32],
            grid: [self.terrain.0, self.terrain.1, self.pole_count as f32, RAYS],
        };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
    }

    /// Bakes the map with `pipeline` (flood.wgsl's `bake`) and its `group`,
    /// [`ROWS`] rows at a time, waiting for each; then marks it baked.
    pub fn run(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, pipeline: &wgpu::ComputePipeline, group: &wgpu::BindGroup) {
        let start = std::time::Instant::now();
        let mut first = 0;
        while first < self.size.1 {
            let rows = ROWS.min(self.size.1 - first);
            self.write(queue, first, rows);
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("flood bake") });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("flood bake"), timestamp_writes: None });
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, group, &[]);
                pass.dispatch_workgroups(self.size.0.div_ceil(8), rows.div_ceil(8), 1);
            }
            queue.submit([encoder.finish()]);
            let _ = device.poll(wgpu::PollType::wait_indefinitely());
            first += rows;
        }
        self.baked = true;
        self.write(queue, 0, 0);
        eprintln!(
            "floodlights: {} poles baked over {}x{} texels of {:.1} ft in {:.0} ms",
            self.pole_count,
            self.size.0,
            self.size.1,
            self.texel,
            start.elapsed().as_secs_f64() * 1000.0
        );
    }

    /// Bakes the map without ray tracing: the hills alone cast.
    pub fn bake_hills(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let source = hills_source();
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("flood bake"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("flood bake"),
            layout: None,
            module: &module,
            entry_point: Some("bake"),
            compilation_options: Default::default(),
            cache: None,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("flood bake"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &self.entries(),
        });
        self.run(device, queue, &pipeline, &group);
    }

    /// The bake's own bindings (flood.wgsl), 30 to 33.
    pub fn entries(&self) -> Vec<wgpu::BindGroupEntry<'_>> {
        vec![
            wgpu::BindGroupEntry { binding: 30, resource: self.heights.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 31, resource: self.uniform.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 32, resource: self.poles.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 33, resource: wgpu::BindingResource::TextureView(&self.map) },
        ]
    }
}
