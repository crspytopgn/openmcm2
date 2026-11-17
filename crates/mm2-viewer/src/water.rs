//! Rivers' water, from a track's `.wtr` (made by the Course Builder,
//! mm2_course::nature): a ribbon down each river at its water's height,
//! drawn after the solid scene and seen through at its edges
//! (`water.wgsl`). Only drawn: the river's bed is in the ground, muddy,
//! and the bikes ride it as the game rides any mud. The splashes and the
//! sound of the water are in main.rs.

use bytemuck::{Pod, Zeroable};
use mm2_course::nature::Water;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    pos: [f32; 3],
    along: [f32; 2],
    flow: [f32; 2],
}

pub struct WaterRenderer {
    pipeline: wgpu::RenderPipeline,
    /// With the enhanced drawing's shared parts as group 1: the sky's own
    /// light in the ripples, and its haze (water_enhanced.wgsl).
    enhanced: Option<wgpu::RenderPipeline>,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
    uniform: wgpu::Buffer,
    group: wgpu::BindGroup,
}

/// How far past the river's edge the ribbon reaches, of its half width:
/// under the banks, so no gap shows.
const OVER: f32 = 1.14;

impl WaterRenderer {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device: &wgpu::Device,
        rivers: &[Water],
        globals: &wgpu::Buffer,
        format: wgpu::TextureFormat,
        samples: u32,
        depth: wgpu::TextureFormat,
        shared: Option<&wgpu::BindGroupLayout>,
    ) -> Option<WaterRenderer> {
        use wgpu::util::DeviceExt;
        let mut vertices = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        for r in rivers {
            let p = &r.points;
            if p.len() < 2 {
                continue;
            }
            let half = r.width / 2.0 * OVER;
            let first = vertices.len() as u32;
            let mut s = 0.0;
            for i in 0..p.len() {
                if i > 0 {
                    s += ((p[i][0] - p[i - 1][0]).powi(2) + (p[i][2] - p[i - 1][2]).powi(2)).sqrt();
                }
                let (a, b) = (p[i.saturating_sub(1)], p[(i + 1).min(p.len() - 1)]);
                let (dx, dz) = (b[0] - a[0], b[2] - a[2]);
                let l = (dx * dx + dz * dz).sqrt().max(1e-4);
                let (dx, dz) = (dx / l, dz / l);
                // Sideways, in the game's coordinates.
                let (sx, sz) = (-dz, dx);
                // In the drawing's, z turned round.
                let flow = [dx * half, -dz * half];
                for k in [-1.0f32, 1.0] {
                    let (x, z) = (p[i][0] + sx * half * k, p[i][2] + sz * half * k);
                    vertices.push(Vertex { pos: [x, p[i][1] + 0.04, -z], along: [s, k], flow });
                }
                if i > 0 {
                    let a = first + (i as u32 - 1) * 2;
                    indices.extend_from_slice(&[a, a + 1, a + 2, a + 1, a + 3, a + 2]);
                }
            }
        }
        if indices.is_empty() {
            return None;
        }
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("water"),
            source: wgpu::ShaderSource::Wgsl(include_str!("water.wgsl").into()),
        });
        let entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("water"), entries: &[entry(0), entry(1)] });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor { label: Some("water"), size: 16, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("water"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }, wgpu::BindGroupEntry { binding: 1, resource: uniform.as_entire_binding() }],
        });
        let make = |shader: &wgpu::ShaderModule, layouts: &[Option<&wgpu::BindGroupLayout>]| {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("water"), bind_group_layouts: layouts, immediate_size: 0 });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("water"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x2],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState { count: samples, ..Default::default() },
            multiview_mask: None,
            cache: None,
        })
        };
        let pipeline = make(&shader, &[Some(&layout)]);
        let enhanced = shared.map(|shared| {
            make(&crate::enhance::shader(device, "water enhanced", &enhanced_source()), &[Some(&layout), Some(shared)])
        });
        Some(WaterRenderer {
            pipeline,
            enhanced,
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("water"), contents: bytemuck::cast_slice(&vertices), usage: wgpu::BufferUsages::VERTEX }),
            indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("water"), contents: bytemuck::cast_slice(&indices), usage: wgpu::BufferUsages::INDEX }),
            count: indices.len() as u32,
            uniform,
            group,
        })
    }

    /// This frame's time, s, and how bright the day is (0 at night).
    pub fn set_time(&self, queue: &wgpu::Queue, time: f32, day: f32) {
        queue.write_buffer(&self.uniform, 0, bytemuck::cast_slice(&[time, day, 0.0, 0.0]));
    }

    /// Draws the rivers; with the enhanced drawing's shared group, as it
    /// lights them.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, shared: Option<&wgpu::BindGroup>) {
        match (&self.enhanced, shared) {
            (Some(pipeline), Some(group)) => {
                pass.set_pipeline(pipeline);
                pass.set_bind_group(1, group, &[]);
            }
            _ => pass.set_pipeline(&self.pipeline),
        }
        pass.set_bind_group(0, &self.group, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.count, 0, 0..1);
    }
}

/// The enhanced drawing's water shader before its shared declarations
/// go in: the plain one with its sky and fog renamed, and the enhanced
/// ones after it.
pub fn enhanced_source() -> String {
    include_str!("water.wgsl").replacen("fn mirrored(", "fn plain_mirrored(", 1).replacen("fn fogged(", "fn plain_fogged(", 1)
        + include_str!("water_enhanced.wgsl")
}

/// The water's height under `(x, z)` (game coordinates, ft), if it is in
/// a river, and how far from its middle (0) to its edge (1).
pub fn level_at(rivers: &[Water], x: f32, z: f32) -> Option<(f32, f32)> {
    let mut best: Option<(f32, f32)> = None;
    for r in rivers {
        let hw = r.width / 2.0;
        for w in r.points.windows(2) {
            let (a, b) = (w[0], w[1]);
            let (dx, dz) = (b[0] - a[0], b[2] - a[2]);
            let l2 = (dx * dx + dz * dz).max(1e-6);
            let t = (((x - a[0]) * dx + (z - a[2]) * dz) / l2).clamp(0.0, 1.0);
            let (px, pz) = (a[0] + dx * t, a[2] + dz * t);
            let d = ((x - px).powi(2) + (z - pz).powi(2)).sqrt();
            if d < hw && best.is_none_or(|b| d / hw < b.1) {
                best = Some((a[1] + (b[1] - a[1]) * t, d / hw));
            }
        }
    }
    best
}
