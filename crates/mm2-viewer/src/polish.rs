//! The picture extras (off unless turned on): smoother edges and
//! sharpening over the race, in either look. The race is drawn into a
//! picture of its own, and one pass (`polish.wgsl`) puts it on the window
//! smoothed and sharpened, before the race display goes over it.

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Params {
    v: [f32; 4],
}

pub struct Polish {
    pipeline: wgpu::RenderPipeline,
    params: wgpu::Buffer,
    sampler: wgpu::Sampler,
    /// The race's picture: its base format, drawn into through an sRGB
    /// view and read through the plain one, so the pass works on the
    /// colours as shown.
    picture: Option<wgpu::Texture>,
}

/// Whether either picture extra is on.
pub fn wanted() -> bool {
    let m = crate::tweaks::more();
    m.fxaa || m.sharpen > 0
}

impl Polish {
    /// For a window drawn through an sRGB view of `format`.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Polish {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("polish"),
            source: wgpu::ShaderSource::Wgsl(include_str!("polish.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("polish"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_full"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_polish"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: format.add_srgb_suffix(),
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Polish {
            pipeline,
            params: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("polish"),
                size: std::mem::size_of::<Params>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("polish"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                ..Default::default()
            }),
            picture: None,
        }
    }

    /// The sRGB view to draw the race into at `size`, for a window of
    /// `format`.
    pub fn target(&mut self, device: &wgpu::Device, format: wgpu::TextureFormat, size: (u32, u32)) -> wgpu::TextureView {
        let format = format.remove_srgb_suffix();
        let fresh = |t: &wgpu::Texture| t.width() == size.0 && t.height() == size.1 && t.format() == format;
        if !self.picture.as_ref().is_some_and(fresh) {
            self.picture = Some(device.create_texture(&wgpu::TextureDescriptor {
                label: Some("race picture"),
                size: wgpu::Extent3d {
                    width: size.0.max(1),
                    height: size.1.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[format.add_srgb_suffix()],
            }));
        }
        self.picture.as_ref().unwrap().create_view(&wgpu::TextureViewDescriptor {
            format: Some(format.add_srgb_suffix()),
            ..Default::default()
        })
    }

    /// Puts the race's picture onto `out` (the window's sRGB view).
    pub fn finish(&self, device: &wgpu::Device, queue: &wgpu::Queue, out: &wgpu::TextureView) -> Option<wgpu::CommandBuffer> {
        let picture = self.picture.as_ref()?;
        let m = crate::tweaks::more();
        let v = [
            1.0 / picture.width() as f32,
            1.0 / picture.height() as f32,
            if m.fxaa { 1.0 } else { 0.0 },
            m.sharpen.min(100) as f32 / 100.0,
        ];
        queue.write_buffer(&self.params, 0, bytemuck::bytes_of(&Params { v }));
        let view = picture.create_view(&Default::default());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("polish"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.params.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("polish"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: out,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3, 0..1);
        }
        Some(encoder.finish())
    }
}
