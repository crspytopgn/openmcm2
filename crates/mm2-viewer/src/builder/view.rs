//! The Course Builder's own quick drawing of the land: a lit height grid
//! coloured by surface, with faint 100 ft squares for scale and the
//! brush's ring on the ground. It is rebuilt in a few milliseconds when
//! anything changes, so points and brushes update as they are dragged;
//! Ride it shows the track in the game's full picture.

use glam::{Mat4, Vec3};
use mm2_course::bake::Field;
use mm2_course::Surface;
use wgpu::util::DeviceExt;

const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    sun: [f32; 4],
    /// The brush: x, z (drawing space), radius, shown (0 or 1).
    ring: [f32; 4],
    ring_color: [f32; 4],
    /// Fog colour and how thick.
    fog: [f32; 4],
}

const SHADER: &str = r#"
struct U {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    sun: vec4<f32>,
    ring: vec4<f32>,
    ring_color: vec4<f32>,
    fog: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: U;

struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
};

@vertex
fn vs(@location(0) p: vec3<f32>, @location(1) n: vec3<f32>, @location(2) c: vec3<f32>) -> Out {
    var o: Out;
    o.position = u.view_proj * vec4<f32>(p, 1.0);
    o.world = p;
    o.normal = n;
    o.color = c;
    return o;
}

@fragment
fn fs(i: Out) -> @location(0) vec4<f32> {
    let n = normalize(i.normal);
    let light = 0.30 + 0.95 * max(dot(n, normalize(u.sun.xyz)), 0.0);
    var c = i.color * light;
    // Faint 100 ft squares, for scale.
    let g = i.world.xz / 100.0;
    let w = fwidth(g);
    let f = abs(fract(g - 0.5) - 0.5) / max(w, vec2<f32>(1e-4));
    let line = 1.0 - min(min(f.x, f.y), 1.0);
    c = mix(c, c * 0.8, line * 0.35);
    // The brush's ring and a light tint inside it.
    if (u.ring.w > 0.5) {
        let d = distance(i.world.xz, u.ring.xy);
        let e = abs(d - u.ring.z) / max(fwidth(d), 1e-4);
        let rim = 1.0 - clamp(e - 0.75, 0.0, 1.0);
        let inside = select(0.0, 0.12, d < u.ring.z);
        c = mix(c, u.ring_color.rgb, max(rim, inside));
    }
    // Haze with distance.
    let dist = distance(i.world, u.eye.xyz);
    let haze = 1.0 - exp(-dist * u.fog.w);
    c = mix(c, u.fog.rgb, clamp(haze, 0.0, 0.6));
    return vec4<f32>(c, 1.0);
}
"#;

fn linear(c: [f32; 3]) -> [f32; 3] {
    c.map(|v| v.max(0.0).powf(2.2))
}

/// The colour the builder shows a surface in (a little brighter than the
/// baked texture, so lines and paint read clearly from above).
pub fn surface_colour(s: Surface) -> [f32; 3] {
    match s {
        Surface::Dirt => [0.56, 0.41, 0.28],
        Surface::Grass => [0.33, 0.45, 0.20],
        Surface::Sand => [0.80, 0.70, 0.50],
        Surface::Mud => [0.30, 0.22, 0.16],
        Surface::Rock => [0.60, 0.58, 0.54],
    }
}

pub struct View {
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    group: wgpu::BindGroup,
    vertices: Option<wgpu::Buffer>,
    indices: Option<(wgpu::Buffer, u32)>,
    /// The grid's side the indices were made for.
    side: usize,
    depth: Option<(wgpu::TextureView, (u32, u32))>,
}

impl View {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> View {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("builder land"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("builder uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("builder"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("builder"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("builder"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("builder land"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        View { pipeline, uniforms, group, vertices: None, indices: None, side: 0, depth: None }
    }

    /// Takes a new bake of the land.
    /// Draws `field`; `ground` gives a copied track's own colour at a
    /// sample (where it was not painted).
    pub fn set_field(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, field: &Field, ground: Option<&dyn Fn(usize) -> Option<[f32; 3]>>) {
        let n = field.n;
        let h = |x: usize, z: usize| field.heights[z.min(n - 1) * n + x.min(n - 1)];
        let mut vertices = Vec::with_capacity(n * n);
        for z in 0..n {
            for x in 0..n {
                let (dx, dz) = (h(x + 1, z) - h(x.saturating_sub(1), z), h(x, z + 1) - h(x, z.saturating_sub(1)));
                let span = 2.0 * field.step;
                // Drawing space is the game's with z turned round.
                let normal = Vec3::new(-dx / span, 1.0, dz / span).normalize();
                let k = z * n + x;
                let mut c = ground.and_then(|g| g(k)).unwrap_or_else(|| surface_colour(Surface::from_zone(field.surfaces[k])));
                // The racing line, a shade darker.
                let t = field.track[k];
                if t < 1.0 {
                    c = c.map(|v| v * 0.8);
                }
                vertices.push(Vertex {
                    position: [x as f32 * field.step, field.heights[k], -(z as f32) * field.step],
                    normal: normal.to_array(),
                    color: linear(c),
                });
            }
        }
        let bytes: &[u8] = bytemuck::cast_slice(&vertices);
        match &self.vertices {
            Some(b) if b.size() == bytes.len() as u64 => queue.write_buffer(b, 0, bytes),
            _ => {
                self.vertices = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("builder land"),
                    contents: bytes,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                }))
            }
        }
        if self.side != n || self.indices.is_none() {
            let mut idx: Vec<u32> = Vec::with_capacity((n - 1) * (n - 1) * 6);
            for z in 0..n - 1 {
                for x in 0..n - 1 {
                    let a = (z * n + x) as u32;
                    let (b, c, d) = (a + 1, a + n as u32, a + n as u32 + 1);
                    idx.extend_from_slice(&[a, c, b, b, c, d]);
                }
            }
            let buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("builder land indices"),
                contents: bytemuck::cast_slice(&idx),
                usage: wgpu::BufferUsages::INDEX,
            });
            self.indices = Some((buf, idx.len() as u32));
            self.side = n;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        size: (u32, u32),
        view_proj: Mat4,
        eye: Vec3,
        ring: Option<([f32; 2], f32, [f32; 3])>,
        sky: [f32; 3],
    ) -> wgpu::CommandBuffer {
        if self.depth.as_ref().is_none_or(|(_, s)| *s != size) {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("builder depth"),
                size: wgpu::Extent3d { width: size.0.max(1), height: size.1.max(1), depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            self.depth = Some((tex.create_view(&Default::default()), size));
        }
        let sun = Vec3::new(0.45, 0.8, 0.35).normalize();
        let (rp, rr, rc, on) = match ring {
            Some((p, r, c)) => ([p[0], -p[1]], r, c, 1.0),
            None => ([0.0; 2], 0.0, [1.0; 3], 0.0),
        };
        let sky_lin = linear(sky);
        let u = Uniforms {
            view_proj: view_proj.to_cols_array_2d(),
            eye: [eye.x, eye.y, eye.z, 0.0],
            sun: [sun.x, sun.y, sun.z, 0.0],
            ring: [rp[0], rp[1], rr, on],
            ring_color: [linear(rc)[0], linear(rc)[1], linear(rc)[2], 1.0],
            fog: [sky_lin[0], sky_lin[1], sky_lin[2], 0.00004],
        };
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&u));
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("builder land"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: sky_lin[0] as f64, g: sky_lin[1] as f64, b: sky_lin[2] as f64, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth.as_ref().unwrap().0,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let (Some(v), Some((i, count))) = (&self.vertices, &self.indices) {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.group, &[]);
                pass.set_vertex_buffer(0, v.slice(..));
                pass.set_index_buffer(i.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..*count, 0, 0..1);
            }
        }
        encoder.finish()
    }
}
