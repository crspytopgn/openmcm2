//! The enhanced drawing's targets and last steps ([`crate::graphics`]):
//! the scene is drawn in HDR with anti-aliasing, then ambient occlusion and
//! bloom are worked out and the result tone mapped onto the screen.

use crate::graphics::HDR_FORMAT;
use crate::renderer::DEPTH_FORMAT;
use bytemuck::{Pod, Zeroable};
use glam::Mat4;

const AO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;
/// The moving models' motion on screen (objects.rs fs_motion): pixels
/// back to where each was a frame ago, how far it is, and 1 where drawn.
pub const MOTION_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub const MOTION_DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// What temporal anti-aliasing needs of a frame: the camera's view
/// unshifted, from the screen back into the world, the last frame's into
/// its screen, and where the camera is; `fresh` to start over (a cut).
#[derive(Clone, Copy)]
pub struct Taa {
    pub inverse: Mat4,
    pub before: Mat4,
    pub camera: glam::Vec3,
    /// How far this frame is drawn aside, pixels right and down.
    pub shift: [f32; 2],
    pub fresh: bool,
}
/// How much the scene is sharpened after temporal anti-aliasing.
const TAA_SHARPEN: f32 = 0.8;
/// Halvings in the bloom's chain.
const BLOOM_STEPS: usize = 6;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PostGlobals {
    inv_proj: [[f32; 4]; 4],
    size: [f32; 4],
    flags: [f32; 4],
    params: [f32; 4],
    /// The sun's place on the screen (0 to 1) and how strong its shafts
    /// are (0 for none), and its colour.
    sun: [f32; 4],
    sun_colour: [f32; 4],
    /// From this frame's screen to the last frame's, for motion blur.
    reproject: [[f32; 4]; 4],
    /// Replays' camera look: x the distance in focus (ft), y the blur's
    /// size out of focus (px), z the motion blur's strength, w 1 on.
    cinematic: [f32; 4],
    /// x 1 when the scene is drawn smaller than the screen and stretched
    /// over it; y how strong the ray-traced light shafts are (0 for the
    /// screen's own).
    scaled: [f32; 4],
    /// Temporal anti-aliasing (Taa): from the screen into the world, and
    /// the world into the last frame's screen, unshifted; the camera; x 1
    /// with a history to blend with, y how much the result is sharpened.
    taa_inverse: [[f32; 4]; 4],
    taa_before: [[f32; 4]; 4],
    taa_camera: [f32; 4],
    taa: [f32; 4],
    /// The size temporal anti-aliasing draws at (the screen's, so it
    /// rebuilds a scene drawn smaller at the screen's own sharpness), and
    /// one over each.
    taa_out: [f32; 4],
    /// The rain when drawn in the last step (with temporal anti-aliasing):
    /// the time, how hard it rains, the screen's shape, the camera's x;
    /// and its colour.
    rain: [f32; 4],
    rain_colour: [f32; 4],
}

/// What the scene is drawn into, at one size.
struct Targets {
    size: (u32, u32),
    /// The screen's size: temporal anti-aliasing's outputs are made at it.
    output: (u32, u32),
    /// The multisampled colour, when anti-aliasing, resolved into `hdr`.
    msaa: Option<wgpu::TextureView>,
    hdr: wgpu::TextureView,
    depth: wgpu::TextureView,
    ao_group: wgpu::BindGroup,
    ao: wgpu::TextureView,
    blur_group: wgpu::BindGroup,
    blurred: wgpu::TextureView,
    bloom: Vec<wgpu::TextureView>,
    /// Bloom's first step, then each step down (from the one above), then
    /// each step up (from the one below).
    bloom_first: wgpu::BindGroup,
    bloom_down: Vec<wgpu::BindGroup>,
    bloom_up: Vec<wgpu::BindGroup>,
    composite: wgpu::BindGroup,
    /// The moving models' motion and its depth (objects.rs draw_motion).
    motion: wgpu::TextureView,
    motion_depth: wgpu::TextureView,
    /// Temporal anti-aliasing's two outputs, written in turn, each the
    /// history of the next: each one's blend (from the other), and bloom
    /// and the last step reading it.
    taa: [wgpu::BindGroup; 2],
    taa_bloom: [wgpu::BindGroup; 2],
    taa_composite: [wgpu::BindGroup; 2],
    history: [wgpu::TextureView; 2],
}

pub struct Post {
    samples: u32,
    /// The ray-traced light shafts (crate::rt) and how strong they show,
    /// when drawn; a blank stand-in for the group.
    traced_shafts: Option<wgpu::TextureView>,
    pub shaft_strength: f32,
    /// The sun's colour for them: theirs whether or not the sun is on
    /// screen (the screen's own shafts know it only while it is).
    pub shaft_colour: [f32; 3],
    blank: wgpu::TextureView,
    globals: wgpu::Buffer,
    sampler: wgpu::Sampler,
    ao: wgpu::RenderPipeline,
    ao_blur: wgpu::RenderPipeline,
    bloom_first: wgpu::RenderPipeline,
    bloom_down: wgpu::RenderPipeline,
    bloom_up: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    taa: wgpu::RenderPipeline,
    /// Which of the outputs is written next, and whether the other holds
    /// the last frame.
    taa_next: std::cell::Cell<usize>,
    taa_valid: std::cell::Cell<bool>,
    targets: Option<Targets>,
}

fn pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    entry: &str,
    format: wgpu::TextureFormat,
    blend: Option<wgpu::BlendState>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(entry),
        layout: None,
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_full"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(entry),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}

impl Post {
    /// For a scene drawn with `samples` per pixel, shown on a `format`
    /// target.
    pub fn new(device: &wgpu::Device, samples: u32, format: wgpu::TextureFormat) -> Post {
        let depth_type = if samples > 1 {
            "texture_depth_multisampled_2d"
        } else {
            "texture_depth_2d"
        };
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("post"),
            source: wgpu::ShaderSource::Wgsl(include_str!("post.wgsl").replace("DEPTH_TEXTURE", depth_type).into()),
        });
        let add = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::REPLACE,
        };
        let blank = Self::texture(device, "no shafts", (1, 1), HDR_FORMAT, 1, wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT)
            .create_view(&Default::default());
        Post {
            traced_shafts: None,
            shaft_strength: 0.0,
            shaft_colour: [0.0; 3],
            blank,
            samples,
            globals: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("post"),
                size: std::mem::size_of::<PostGlobals>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("post"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            ao: pipeline(device, &shader, "fs_ao", AO_FORMAT, None),
            ao_blur: pipeline(device, &shader, "fs_ao_blur", AO_FORMAT, None),
            bloom_first: pipeline(device, &shader, "fs_bloom_first", HDR_FORMAT, None),
            bloom_down: pipeline(device, &shader, "fs_bloom_down", HDR_FORMAT, None),
            bloom_up: pipeline(device, &shader, "fs_bloom_up", HDR_FORMAT, Some(add)),
            composite: pipeline(device, &shader, "fs_composite", format, None),
            taa: pipeline(device, &shader, "fs_taa", HDR_FORMAT, None),
            taa_next: std::cell::Cell::new(0),
            taa_valid: std::cell::Cell::new(false),
            targets: None,
        }
    }

    fn texture(
        device: &wgpu::Device,
        label: &str,
        (width, height): (u32, u32),
        format: wgpu::TextureFormat,
        samples: u32,
        usage: wgpu::TextureUsages,
    ) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    }

    fn group(&self, device: &wgpu::Device, pipeline: &wgpu::RenderPipeline, views: &[(u32, &wgpu::TextureView)], with_globals: bool, with_sampler: bool) -> wgpu::BindGroup {
        let mut entries: Vec<wgpu::BindGroupEntry> = views
            .iter()
            .map(|(binding, view)| wgpu::BindGroupEntry {
                binding: *binding,
                resource: wgpu::BindingResource::TextureView(view),
            })
            .collect();
        if with_globals {
            entries.push(wgpu::BindGroupEntry {
                binding: 0,
                resource: self.globals.as_entire_binding(),
            });
        }
        if with_sampler {
            entries.push(wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::Sampler(&self.sampler),
            });
        }
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("post"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &entries,
        })
    }

    /// Makes the targets for `size`, unless they are made.
    fn ready(&mut self, device: &wgpu::Device, size: (u32, u32), output: (u32, u32)) {
        if self.targets.as_ref().is_some_and(|t| t.size == size && t.output == output) {
            return;
        }
        let draw = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let view = |t: wgpu::Texture| t.create_view(&Default::default());
        let msaa = (self.samples > 1).then(|| {
            view(Self::texture(device, "scene msaa", size, HDR_FORMAT, self.samples, wgpu::TextureUsages::RENDER_ATTACHMENT))
        });
        let hdr = view(Self::texture(device, "scene", size, HDR_FORMAT, 1, draw));
        let depth_texture = Self::texture(device, "scene depth", size, DEPTH_FORMAT, self.samples, draw);
        let depth = depth_texture.create_view(&Default::default());
        let depth_only = depth_texture.create_view(&wgpu::TextureViewDescriptor {
            aspect: wgpu::TextureAspect::DepthOnly,
            ..Default::default()
        });
        // The occlusion at half size.
        let half = (size.0.div_ceil(2), size.1.div_ceil(2));
        let ao = view(Self::texture(device, "ao", half, AO_FORMAT, 1, draw));
        let blurred = view(Self::texture(device, "ao blurred", half, AO_FORMAT, 1, draw));
        let mut bloom = Vec::new();
        let mut s = size;
        for _ in 0..BLOOM_STEPS {
            s = ((s.0 / 2).max(1), (s.1 / 2).max(1));
            bloom.push(view(Self::texture(device, "bloom", s, HDR_FORMAT, 1, draw)));
        }
        let ao_group = self.group(device, &self.ao, &[(1, &depth_only)], true, false);
        let blur_group = self.group(device, &self.ao_blur, &[(3, &ao)], true, true);
        let bloom_first = self.group(device, &self.bloom_first, &[(2, &hdr)], false, true);
        let bloom_down = (1..BLOOM_STEPS)
            .map(|i| self.group(device, &self.bloom_down, &[(4, &bloom[i - 1])], false, true))
            .collect();
        let bloom_up = (1..BLOOM_STEPS)
            .map(|i| self.group(device, &self.bloom_up, &[(4, &bloom[i])], false, true))
            .collect();
        let shafts = self.traced_shafts.clone().unwrap_or_else(|| self.blank.clone());
        let composite = self.group(device, &self.composite, &[(1, &depth_only), (2, &hdr), (3, &blurred), (6, &bloom[0]), (7, &shafts)], true, true);
        let motion = view(Self::texture(device, "motion", size, MOTION_FORMAT, 1, draw));
        let motion_depth = view(Self::texture(device, "motion depth", size, MOTION_DEPTH, 1, wgpu::TextureUsages::RENDER_ATTACHMENT));
        // At the screen's size: drawn smaller, the scene is rebuilt from the
        // frames' shifted samples at the screen's own sharpness, not
        // stretched.
        let history = [0, 1].map(|_| view(Self::texture(device, "taa", output, HDR_FORMAT, 1, draw)));
        let taa = [0, 1].map(|i| self.group(device, &self.taa, &[(1, &depth_only), (2, &hdr), (4, &history[1 - i]), (7, &shafts), (8, &motion)], true, true));
        let taa_bloom = [0, 1].map(|i| self.group(device, &self.bloom_first, &[(2, &history[i])], false, true));
        let taa_composite =
            [0, 1].map(|i| self.group(device, &self.composite, &[(1, &depth_only), (2, &history[i]), (3, &blurred), (6, &bloom[0]), (7, &shafts)], true, true));
        self.taa_valid.set(false);
        self.targets = Some(Targets {
            output,
            motion,
            motion_depth,
            taa,
            taa_bloom,
            taa_composite,
            history,
            size,
            msaa,
            hdr,
            depth,
            ao_group,
            ao,
            blur_group,
            blurred,
            bloom,
            bloom_first,
            bloom_down,
            bloom_up,
            composite,
        });
    }

    /// Shows the light shafts from `traced` (the ray-traced ones), or works
    /// them out on screen with none.
    pub fn set_traced_shafts(&mut self, traced: Option<&wgpu::TextureView>) {
        self.traced_shafts = traced.cloned();
        // Made again with them at the next frame.
        self.targets = None;
    }

    /// Where to draw the scene at `size`, on a screen `output` big: the
    /// colour, what it resolves into, and the depth.
    pub fn scene(&mut self, device: &wgpu::Device, size: (u32, u32), output: (u32, u32)) -> (&wgpu::TextureView, Option<&wgpu::TextureView>, &wgpu::TextureView) {
        self.ready(device, size, output);
        let t = self.targets.as_ref().unwrap();
        match &t.msaa {
            Some(msaa) => (msaa, Some(&t.hdr), &t.depth),
            None => (&t.hdr, None, &t.depth),
        }
    }

    /// Where the moving models' motion is drawn, and its depth, for
    /// temporal anti-aliasing (made with the scene's targets).
    pub fn motion(&self) -> Option<(&wgpu::TextureView, &wgpu::TextureView)> {
        self.targets.as_ref().map(|t| (&t.motion, &t.motion_depth))
    }

    /// Finishes the frame onto `target`: occlusion and bloom when on, and
    /// tone mapped unless the original look is showing.
    pub fn finish(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        proj: Mat4,
        showing: Option<crate::graphics::Enhanced>,
        sun: Option<([f32; 3], [f32; 3])>,
        cinematic: Option<(f32, Mat4)>,
        screen: (u32, u32),
        taa: Option<Taa>,
        rain: Option<([f32; 4], [f32; 3])>,
        mut timer: Option<&mut crate::gpu_time::GpuTimer>,
    ) {
        let Some(t) = &self.targets else { return };
        if taa.is_none_or(|a| a.fresh) {
            self.taa_valid.set(false);
        }
        let (w, h) = (t.size.0 as f32, t.size.1 as f32);
        let ao = showing.is_some_and(|e| e.ao);
        let bloom = showing.is_some_and(|e| e.bloom);
        let globals = PostGlobals {
            inv_proj: proj.inverse().to_cols_array_2d(),
            size: [w, h, 1.0 / w, 1.0 / h],
            flags: [ao as u32 as f32, bloom as u32 as f32, showing.is_some() as u32 as f32, showing.is_some_and(|e| e.grade) as u32 as f32],
            // A 3 ft reach, its strength, the bloom's, and how many pixels a foot
            // spans 1 ft away.
            params: [3.0, if showing.is_some_and(|e| e.grade) { 2.2 } else { 1.5 }, if showing.is_some_and(|e| e.grade) { 0.9 } else { 0.6 }, proj.y_axis.y * 0.5 * h],
            sun: sun.filter(|_| showing.is_some_and(|e| e.shafts)).map_or([0.0; 4], |(s, _)| [s[0], s[1], s[2], 0.0]),
            sun_colour: match sun {
                _ if self.traced_shafts.is_some() => [self.shaft_colour[0], self.shaft_colour[1], self.shaft_colour[2], 0.0],
                Some((_, c)) => [c[0], c[1], c[2], 0.0],
                None => [0.0; 4],
            },
            reproject: cinematic.map_or(Mat4::IDENTITY, |c| c.1).to_cols_array_2d(),
            cinematic: cinematic.map_or([0.0; 4], |(focus, _)| [focus, 0.006 * h, 0.5, 1.0]),
            scaled: [
                (t.size != screen) as u32 as f32,
                if self.traced_shafts.is_some() && showing.is_some_and(|e| e.shafts) { self.shaft_strength } else { 0.0 },
                0.0,
                0.0,
            ],
            taa_inverse: taa.map_or(Mat4::IDENTITY, |a| a.inverse).to_cols_array_2d(),
            taa_before: taa.map_or(Mat4::IDENTITY, |a| a.before).to_cols_array_2d(),
            taa_camera: taa.map_or([0.0; 4], |a| a.camera.extend(1.0).into()),
            rain: rain.map_or([0.0; 4], |r| r.0),
            rain_colour: rain.map_or([0.0; 4], |r| [r.1[0], r.1[1], r.1[2], 0.0]),
            taa_out: {
                let (w, h) = (t.output.0.max(1) as f32, t.output.1.max(1) as f32);
                [w, h, 1.0 / w, 1.0 / h]
            },
            taa: [self.taa_valid.get() as u32 as f32, if taa.is_some() { TAA_SHARPEN } else { 0.0 }, taa.map_or(0.0, |a| a.shift[0]), taa.map_or(0.0, |a| a.shift[1])],
        };
        queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));
        let mut mark = |encoder: &mut wgpu::CommandEncoder, label: &'static str| {
            if let Some(t) = timer.as_mut() {
                t.mark(encoder, label);
            }
        };
        let pass = |encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, load: bool, pipeline: &wgpu::RenderPipeline, group: &wgpu::BindGroup| {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("post"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if load { wgpu::LoadOp::Load } else { wgpu::LoadOp::Clear(wgpu::Color::BLACK) },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.draw(0..3, 0..1);
        };
        // Blended with the frames before, before the rest reads it.
        let (bloom_first, composite) = match taa {
            Some(_) => {
                let i = self.taa_next.get();
                pass(encoder, &t.history[i], false, &self.taa, &t.taa[i]);
                mark(encoder, "post: taa");
                self.taa_next.set(1 - i);
                self.taa_valid.set(true);
                (&t.taa_bloom[i], &t.taa_composite[i])
            }
            None => (&t.bloom_first, &t.composite),
        };
        if ao {
            pass(encoder, &t.ao, false, &self.ao, &t.ao_group);
            pass(encoder, &t.blurred, false, &self.ao_blur, &t.blur_group);
            mark(encoder, "post: ao");
        }
        if bloom {
            pass(encoder, &t.bloom[0], false, &self.bloom_first, bloom_first);
            for i in 1..BLOOM_STEPS {
                pass(encoder, &t.bloom[i], false, &self.bloom_down, &t.bloom_down[i - 1]);
            }
            for i in (1..BLOOM_STEPS).rev() {
                pass(encoder, &t.bloom[i - 1], true, &self.bloom_up, &t.bloom_up[i - 1]);
            }
        }
        mark(encoder, "post: bloom");
        pass(encoder, target, false, &self.composite, composite);
        mark(encoder, "post: composite");
    }
}
