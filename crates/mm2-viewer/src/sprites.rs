//! 2D drawing for the front end and the in-race display: textured
//! rectangles on the game's 640×480 screen, scaled to fit the window with
//! the picture kept at 4:3.

use mm2_formats::tga::Image;
use wgpu::util::DeviceExt;

/// The game's screen size; everything 2D is placed on it.
pub const SCREEN: (f32, f32) = (640.0, 480.0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureId(usize);

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

struct Texture {
    texture: wgpu::Texture,
    group: wgpu::BindGroup,
    width: u32,
    height: u32,
    /// Its size in 640x480 pixels, where it holds more pixels than it
    /// covers (text drawn at the window's resolution).
    logical: (u32, u32),
}

pub struct Sprites {
    pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    textures: Vec<Texture>,
    /// This frame's rectangles, by texture.
    /// The third field is whether the quad stands at a place in the 3D
    /// view (a rider's label), with [`widescreen`](Self::widescreen).
    quads: Vec<(TextureId, [Vertex; 4], bool)>,
    /// Stretch 640×480 over the whole window, as the game's race display
    /// does, rather than keep it 4:3 (the menus).
    pub stretch: bool,
    /// Window pixels per 640x480 pixel, for drawing text at the window's
    /// own resolution.
    pub scale: f32,
    /// With `stretch`, on a window wider than 4:3: keep each piece its
    /// shape and move it to the edge it is nearest, rather than stretch
    /// it (an enhanced-graphics upgrade, not the game's).
    pub widescreen: bool,
    /// Whether the quads drawn now are placed in the 3D view, so only
    /// their place is stretched, not their shape.
    pub in_view: bool,
}

const SHADER: &str = r#"
struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(@location(0) position: vec2<f32>, @location(1) uv: vec2<f32>,
           @location(2) color: vec4<f32>) -> Out {
    var out: Out;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.uv = uv;
    // Tints are the game's own sRGB colours; the target blends in linear.
    let c = color.rgb;
    let lin = select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
    out.color = vec4<f32>(lin, color.a);
    return out;
}

@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;

// Sharp bilinear: a magnified texel stays a solid square, blended with
// its neighbour only across the last screen pixel at its edge, so the
// game's 640x480 art and text keep hard pixels at any window size without
// the uneven rows of plain nearest. Minified images (the hi-res pack)
// sample plainly.
@fragment
fn fs_main(in: Out) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(image));
    let texel = in.uv * size;
    let per_pixel = max(fwidth(texel), vec2<f32>(1e-6));
    let edge = max(vec2<f32>(0.5) - 0.5 * per_pixel, vec2<f32>(0.0));
    let d = fract(texel) - 0.5;
    let f = (d - clamp(d, -edge, edge)) / per_pixel + 0.5;
    let uv = select((floor(texel) + f) / size, in.uv, per_pixel >= vec2<f32>(1.0));
    return textureSampleGrad(image, image_sampler, uv, dpdx(in.uv), dpdy(in.uv)) * in.color;
}
"#;

impl Sprites {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sprites"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprites"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sprites"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Sprites {
            pipeline,
            sampler,
            textures: Vec::new(),
            quads: Vec::new(),
            stretch: false,
            scale: 1.0,
            widescreen: false,
            in_view: false,
        }
    }

    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        image: &Image,
    ) -> TextureId {
        self.upload_covering(device, queue, image, (image.width, image.height))
    }

    /// Uploads `image` to cover `logical` 640x480 pixels.
    pub fn upload_covering(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        image: &Image,
        logical: (u32, u32),
    ) -> TextureId {
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("sprite"),
                size: wgpu::Extent3d {
                    width: image.width.max(1),
                    height: image.height.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            if image.rgba.is_empty() {
                &[0; 4]
            } else {
                &image.rgba
            },
        );
        let view = texture.create_view(&Default::default());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprite"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.textures.push(Texture {
            texture,
            group,
            width: image.width,
            height: image.height,
            logical,
        });
        TextureId(self.textures.len() - 1)
    }

    /// New pixels for a texture of the same size (a movie's next frame).
    pub fn update(&mut self, queue: &wgpu::Queue, id: TextureId, rgba: &[u8]) {
        let t = &self.textures[id.0];
        if rgba.len() != (t.width * t.height * 4) as usize {
            return;
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &t.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(t.width * 4),
                rows_per_image: Some(t.height),
            },
            wgpu::Extent3d {
                width: t.width,
                height: t.height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Drops the quads queued since the last render.
    pub fn discard(&mut self) {
        self.quads.clear();
    }

    pub fn size(&self, id: TextureId) -> (u32, u32) {
        self.textures[id.0].logical
    }

    /// Draws the whole of `id` with its top left at `x`, `y`.
    pub fn image(&mut self, id: TextureId, x: f32, y: f32) {
        let (w, h) = self.size(id);
        self.quad(
            id,
            [0.0, 0.0, 1.0, 1.0],
            [x, y, w as f32, h as f32],
            [1.0; 4],
        );
    }

    /// Draws the part `uv` (left, top, right, bottom, 0 to 1) of `id` into
    /// `rect` (x, y, width, height on the 640×480 screen), tinted.
    pub fn quad(&mut self, id: TextureId, uv: [f32; 4], rect: [f32; 4], color: [f32; 4]) {
        let [x, y, w, h] = rect;
        let v = |px: f32, py: f32, u: f32, t: f32| Vertex {
            position: [px, py],
            uv: [u, t],
            color,
        };
        self.quads.push((
            id,
            [
                v(x, y, uv[0], uv[1]),
                v(x + w, y, uv[2], uv[1]),
                v(x + w, y + h, uv[2], uv[3]),
                v(x, y + h, uv[0], uv[3]),
            ],
            self.in_view,
        ));
    }

    /// Draws the whole of `id` stretched over four corners (clockwise from
    /// the top left), for shapes that are not upright rectangles.
    pub fn corners(&mut self, id: TextureId, corners: [[f32; 2]; 4], color: [f32; 4]) {
        let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        self.quads.push((
            id,
            std::array::from_fn(|i| Vertex {
                position: corners[i],
                uv: uv[i],
                color,
            }),
            self.in_view,
        ));
    }

    /// Draws this frame's rectangles over `target` and clears the list.
    /// With `clear`, the window is first filled with black.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        (width, height): (u32, u32),
        clear: bool,
    ) {
        // Fit 640×480 into the window, centred, or stretch it.
        let (sx, sy) = if self.stretch {
            (width as f32 / SCREEN.0, height as f32 / SCREEN.1)
        } else {
            let scale = (width as f32 / SCREEN.0).min(height as f32 / SCREEN.1);
            (scale, scale)
        };
        let (ox, oy) = (
            (width as f32 - SCREEN.0 * sx) / 2.0,
            (height as f32 - SCREEN.1 * sy) / 2.0,
        );
        let ndc = |p: [f32; 2]| {
            [
                (ox + p[0] * sx) / width as f32 * 2.0 - 1.0,
                1.0 - (oy + p[1] * sy) / height as f32 * 2.0,
            ]
        };
        let mut vertices = Vec::with_capacity(self.quads.len() * 4);
        let mut indices: Vec<u32> = Vec::with_capacity(self.quads.len() * 6);
        let mut batches: Vec<(TextureId, std::ops::Range<u32>)> = Vec::new();
        // Widescreen: one scale for both ways, and each piece moved to the
        // edge it is nearest, by the third of the screen its middle is in.
        let wide = (self.stretch && self.widescreen && width * 3 > height * 4)
            .then(|| (height as f32 / SCREEN.1, width as f32 / SCREEN.0));
        for (id, quad, in_view) in &self.quads {
            let base = vertices.len() as u32;
            let shift = wide.map(|(s, stretched)| {
                let middle = quad.iter().map(|v| v.position[0]).sum::<f32>() / 4.0;
                let anchor = if *in_view {
                    // Stays over its rider: its corner at its stretched place.
                    quad[0].position[0]
                } else if middle < SCREEN.0 / 3.0 {
                    0.0
                } else if middle > SCREEN.0 * 2.0 / 3.0 {
                    SCREEN.0
                } else {
                    SCREEN.0 / 2.0
                };
                (anchor, anchor * stretched, s)
            });
            for v in quad {
                let position = match shift {
                    Some((anchor, to, s)) => [
                        (to + (v.position[0] - anchor) * s) / width as f32 * 2.0 - 1.0,
                        1.0 - v.position[1] * s / height as f32 * 2.0,
                    ],
                    None => ndc(v.position),
                };
                vertices.push(Vertex { position, ..*v });
            }
            let start = indices.len() as u32;
            indices.extend([0, 1, 2, 0, 2, 3].map(|i| base + i));
            match batches.last_mut() {
                Some((last, range)) if last == id => range.end = indices.len() as u32,
                _ => batches.push((*id, start..indices.len() as u32)),
            }
        }
        self.quads.clear();
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sprites"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: if clear {
                        wgpu::LoadOp::Clear(wgpu::Color::BLACK)
                    } else {
                        wgpu::LoadOp::Load
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if indices.is_empty() {
            return;
        }
        let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sprite vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sprite indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, vb.slice(..));
        pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
        for (id, range) in batches {
            pass.set_bind_group(0, &self.textures[id.0].group, &[]);
            pass.draw_indexed(range, 0, 0..1);
        }
    }

    /// Where a window position falls on the 640×480 screen.
    pub fn to_screen((width, height): (u32, u32), x: f64, y: f64) -> (f32, f32) {
        Self::to_screen_as((width, height), x, y, false)
    }

    /// The same, for sprites drawn with [`stretch`](Self::stretch) on (the
    /// race's HUD and the replay player), which fill the window.
    pub fn to_screen_as((width, height): (u32, u32), x: f64, y: f64, stretch: bool) -> (f32, f32) {
        if stretch {
            return (
                x as f32 * SCREEN.0 / width.max(1) as f32,
                y as f32 * SCREEN.1 / height.max(1) as f32,
            );
        }
        let scale = (width as f32 / SCREEN.0).min(height as f32 / SCREEN.1);
        let (sw, sh) = (SCREEN.0 * scale, SCREEN.1 * scale);
        let (ox, oy) = ((width as f32 - sw) / 2.0, (height as f32 - sh) / 2.0);
        ((x as f32 - ox) / scale, (y as f32 - oy) / scale)
    }
}
