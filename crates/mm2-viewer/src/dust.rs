//! Dust clouds behind the bikes for the enhanced drawing
//! ([`crate::graphics`]), none of them the game's: soft, lumpy puffs the
//! colour of the ground, thrown up behind each bike's rear tyre as it runs
//! on ground where the game raises dust. A spinning tyre or a bike pulling
//! hard throws up more, and a landing throws out a ring. They swell, spread
//! low, drift with the wind and thin away (`dust.wgsl`). They stand in for
//! the game's own dust trail; its dirt chunks still fly. On wet ground the
//! tyres throw up spray instead: drops of muddy water that fall back.
//!
//! The puffs are also drawn from the sun into a shadow map of the dust
//! (enhanced_dust.wgsl): how thick it is along each of the sun's rays and
//! how deep into it, so the clouds shade themselves and what lies past
//! them.
//!
//! With `roost` on, the rear tyres also throw dirt as a real bike does: a
//! fan of clods flung back and up as it pulls hard or spins, and out to
//! the side as it slides through a turn, falling back to land, some in a
//! little puff of their own. They stand in for the game's dirt chunks.
//!
//! Cheap by design: each puff's shape is read from a few baked pictures
//! of lumpy dust ([`puff_atlas`]), turned and blended as it churns, rather
//! than worked out from noise for every pixel, and its sunlight is asked
//! once at each corner, not at every pixel. How many puffs and clods fly
//! goes with the graphics preset ([`Budget::amount`]): fewer and larger on
//! Low, and no hanging haze there.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

/// Puffs in the air at most, at an amount of 1 ([`Budget::amount`]).
const MOST: usize = 2400;
/// Room in the buffer: for the most at the largest amount.
const ROOM: usize = 3200;
/// The baked pictures of dust: how many, and their side, texels.
const LAYERS: u32 = 8;
const SIDE: u32 = 128;
/// How far from the eye the dust casts shadows, ft.
const SHADOWED: f32 = 250.0;

/// The dust's shadow map's view for the box from `low` to `high`, lit from
/// `to_sun`: from the drawing to the map, looking along the light, and the
/// least spread of a cloud's depth in it (a ft and a half).
pub fn sun_view(low: Vec3, high: Vec3, to_sun: Vec3) -> (glam::Mat4, f32) {
    use glam::Mat4;
    let up = if to_sun.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y };
    let light = Mat4::look_at_rh(Vec3::ZERO, -to_sun, up);
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for i in 0..8 {
        let corner = Vec3::new(
            if i & 1 == 0 { low.x } else { high.x },
            if i & 2 == 0 { low.y } else { high.y },
            if i & 4 == 0 { low.z } else { high.z },
        );
        let p = light.transform_point3(corner);
        lo = lo.min(p);
        hi = hi.max(p);
    }
    // Looking along -z: the nearest to the sun has the highest z.
    let (near, far) = (-hi.z - 2.0, -lo.z + 2.0);
    let proj = Mat4::orthographic_rh(lo.x - 1.0, hi.x + 1.0, lo.y - 1.0, hi.y + 1.0, near, far);
    (proj * light, 1.5 / (far - near).max(1.0))
}

/// The wind the dust drifts on, ft/s, drawing coordinates.
const WIND: Vec3 = Vec3::new(2.0, 0.0, 1.2);

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Instance {
    /// Where, drawing coordinates, and how big (half its side), ft.
    place: [f32; 4],
    /// How thick, its seed, how far it has churned and its size again; a
    /// drop's thickness and velocity.
    look: [f32; 4],
    /// Where it rose from (x, z), for the ground's colour; 0 for a puff,
    /// 1 for a drop of spray, 2 for a clod; and a clod's picture and turn
    /// (the whole part and the fraction of a turn).
    home: [f32; 4],
}

/// How a bike's tyres kick up dust this step.
#[derive(Clone, Copy, Debug, Default)]
pub struct Kick {
    /// The ground under a tyre raises dust (the surface's GenerateDust).
    pub dusty: bool,
    /// How much the rear tyre spins, 0 to 1 (1 less its traction).
    pub spin: f32,
    /// The ground under the rear tyre is dirt that flies (GenerateDirt).
    pub dirt: bool,
}

/// What the dust and dirt may do, from the graphics settings.
#[derive(Clone, Copy, Debug)]
pub struct Budget {
    /// Dust clouds (and spray on wet ground).
    pub dust: bool,
    /// Flying dirt off the rear tyres.
    pub roost: bool,
    /// How much of either flies, about 1 (High): 0.5 on Low, 0.75 on
    /// Medium, 1.25 on Ultra and Max.
    pub amount: f32,
}

impl Default for Budget {
    fn default() -> Self {
        Budget { dust: true, roost: false, amount: 1.0 }
    }
}

/// What is kept of each bike from the step before.
#[derive(Clone, Copy, Default)]
struct Track {
    /// The rear tyre, drawing coordinates.
    last: Option<Vec3>,
    /// Its speed, ft/s.
    speed: f32,
    /// Seconds off the ground.
    air: f32,
    /// Puffs owed, a fraction carried over; clods; and the thin haze
    /// that hangs over a dry track.
    owed: f32,
    clods: f32,
    haze: f32,
    /// How hard it pulls away, smoothed (0 to 1.5), and the way it ran
    /// (level, unit) for how fast it turns.
    pull: f32,
    heading: Vec3,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Puff,
    /// A drop of spray: small, and falls.
    Drop,
    /// A clod of dirt: small, solid, tumbles and falls.
    Clod,
}

struct Puff {
    at: Vec3,
    home: Vec3,
    velocity: Vec3,
    age: f32,
    life: f32,
    seed: f32,
    thick: f32,
    kind: Kind,
    /// The size it is thrown at and how much it swells, ft.
    size: f32,
    swell: f32,
    /// A clod that lands on dry, dusty ground raises a little puff.
    dusty: bool,
}

pub struct DustRenderer {
    pipeline: wgpu::RenderPipeline,
    /// The puffs drawn from the sun into the dust's shadow map, with only
    /// the enhanced drawing's settings (the shared group holds the map).
    shadow: wgpu::RenderPipeline,
    shadow_group: wgpu::BindGroup,
    instances: wgpu::Buffer,
    /// The baked pictures of dust, for this pipeline's own group.
    atlas_group: wgpu::BindGroup,
    count: u32,
    puffs: std::collections::VecDeque<Puff>,
    /// The most puffs at once, by the amount.
    most: usize,
    bikes: Vec<Track>,
    random: u32,
}

impl DustRenderer {
    /// With the terrain's own group's layout and the enhanced drawing's
    /// shared one.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        terrain_layout: &wgpu::BindGroupLayout,
        shared_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
        samples: u32,
        depth_format: wgpu::TextureFormat,
        settings: &wgpu::Buffer,
    ) -> DustRenderer {
        let atlas_layout = crate::enhance::layout(
            device,
            "dust pictures",
            &[
                (
                    0,
                    wgpu::ShaderStages::FRAGMENT,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                ),
                (1, wgpu::ShaderStages::FRAGMENT, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)),
            ],
        );
        let atlas_group = {
            use wgpu::util::DeviceExt;
            let mips = SIDE.trailing_zeros() - 2;
            let texture = device.create_texture_with_data(
                queue,
                &wgpu::TextureDescriptor {
                    label: Some("dust pictures"),
                    size: wgpu::Extent3d {
                        width: SIDE,
                        height: SIDE,
                        depth_or_array_layers: LAYERS,
                    },
                    mip_level_count: mips,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                &puff_atlas(SIDE as usize, LAYERS as usize, mips as usize),
            );
            let view = texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("dust pictures"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                ..Default::default()
            });
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("dust pictures"),
                layout: &atlas_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            })
        };
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("dust"),
            bind_group_layouts: &[Some(terrain_layout), Some(shared_layout), Some(&atlas_layout)],
            immediate_size: 0,
        });
        let shader = crate::enhance::shader(device, "dust", include_str!("dust.wgsl"));
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("dust"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: Some(false),
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
        let settings_layout = crate::enhance::layout(
            device,
            "dust shadows",
            &[(0, wgpu::ShaderStages::VERTEX_FRAGMENT, crate::enhance::uniform())],
        );
        let shadow_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("dust shadows"),
            layout: &settings_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: settings.as_entire_binding(),
            }],
        });
        let shadow_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("dust shadows"),
            bind_group_layouts: &[Some(terrain_layout), Some(&settings_layout)],
            immediate_size: 0,
        });
        let shadow = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("dust shadows"),
            layout: Some(&shadow_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_shadow"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_shadow"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: crate::enhance::DUST_FORMAT,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("dust"),
            size: (ROOM * std::mem::size_of::<Instance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        DustRenderer {
            pipeline,
            shadow,
            shadow_group,
            instances,
            atlas_group,
            count: 0,
            puffs: std::collections::VecDeque::new(),
            most: MOST,
            bikes: Vec::new(),
            random: 0x9e37_79b9,
        }
    }

    fn random(&mut self) -> f32 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 17;
        self.random ^= self.random << 5;
        (self.random >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Each bike's tyres on the ground this step (game coordinates) and
    /// how they kick up dust, the step, s, whether the ground is wet, and
    /// what may fly: the rear ones throw up dust (or spray) and dirt as
    /// they run, and what is in the air moves on.
    pub fn step(&mut self, feed: &[([Option<Vec3>; 2], Kick)], dt: f32, wet: bool, budget: Budget) {
        if dt <= 0.0 {
            return;
        }
        let amount = budget.amount.clamp(0.25, 1.5);
        self.most = ((MOST as f32 * amount) as usize).min(ROOM);
        self.bikes.resize(feed.len(), Track::default());
        for (i, (tyres, kick)) in feed.iter().enumerate() {
            let rear = tyres[1].map(crate::renderer::from_game);
            let mut track = self.bikes[i];
            match (track.last, rear) {
                (Some(a), Some(b)) => {
                    let velocity = (b - a) / dt;
                    let speed = velocity.length();
                    if speed < 200.0 {
                        // How hard it pulls away, smoothed over about a
                        // tenth of a second (the tyre's place jitters).
                        let pull = ((speed - track.speed) / dt / 20.0).clamp(0.0, 1.5);
                        let k = 1.0 - (-dt / 0.1).exp();
                        track.pull += (pull - track.pull) * k;
                        if budget.dust {
                            if wet {
                                self.spray(&mut track, b, velocity, dt);
                            } else if kick.dusty {
                                self.dust(&mut track, b, velocity, *kick, dt, amount);
                            }
                        }
                        if budget.roost && kick.dirt {
                            self.roost(&mut track, b, velocity, *kick, dt, amount, wet);
                        }
                    }
                    track.speed = speed;
                    track.air = 0.0;
                }
                (None, Some(b)) => {
                    // Down again after a jump: a ring of dust thrown out
                    // round the tyre, by how long it flew and how fast.
                    if budget.dust && !wet && kick.dusty && track.air > 0.3 && track.speed > 12.0 {
                        let n = (((track.air * 10.0).min(12.0) + track.speed * 0.15) * amount.sqrt()) as usize;
                        for k in 0..n {
                            let (r1, r2, r3, seed) = (self.random(), self.random(), self.random(), self.random());
                            let angle = (k as f32 + r1) / n as f32 * std::f32::consts::TAU;
                            let out = Vec3::new(angle.cos(), 0.0, angle.sin());
                            self.add(Puff {
                                at: b + out * 0.8 + Vec3::Y * 0.2,
                                home: b,
                                velocity: out * (6.0 + 8.0 * r2) + Vec3::Y * (1.0 + 2.0 * r3),
                                age: 0.0,
                                life: 1.6 + 1.4 * r3,
                                seed,
                                thick: 0.32 + 0.2 * r2,
                                kind: Kind::Puff,
                                size: 0.5 + 0.4 * r1,
                                swell: (1.4 + 1.2 * r2) / amount.sqrt(),
                                dusty: false,
                            });
                        }
                    }
                    track.air = 0.0;
                    track.pull = 0.0;
                }
                (_, None) => track.air += dt,
            }
            track.last = rear;
            self.bikes[i] = track;
        }
        let mut landed: Vec<(Vec3, f32)> = Vec::new();
        for p in &mut self.puffs {
            p.age += dt;
            match p.kind {
                Kind::Drop => {
                    // A drop falls, a little slowed by the air.
                    p.velocity = p.velocity * (-dt * 0.6).exp() - Vec3::Y * 32.0 * dt;
                    p.at += p.velocity * dt;
                }
                Kind::Clod => {
                    // A clod falls as a stone does, and lands about as low
                    // as the tyre that threw it (it stops at the ground
                    // before that where the ground rises, hidden by it).
                    p.velocity = p.velocity * (-dt * 0.25).exp() - Vec3::Y * 32.0 * dt;
                    p.at += p.velocity * dt;
                    if p.velocity.y < 0.0 && p.at.y < p.home.y - 0.1 {
                        if p.dusty {
                            landed.push((p.at, p.size));
                        }
                        p.age = p.life;
                    }
                }
                Kind::Puff => {
                    // Slowed hard by the air toward the wind, so it hangs
                    // low rather than rising as smoke does.
                    let drag = (-dt * 2.2).exp();
                    p.velocity = WIND + (p.velocity - WIND) * drag + Vec3::Y * 0.3 * dt;
                    p.at += p.velocity * dt;
                }
            }
        }
        self.puffs.retain(|p| p.age < p.life);
        // The bigger clods landing on dry ground raise a little dust.
        for (at, size) in landed {
            let (r1, r2, seed) = (self.random(), self.random(), self.random());
            if r1 > 0.25 + size * 2.0 {
                continue;
            }
            self.add(Puff {
                at: at + Vec3::Y * 0.1,
                home: at,
                velocity: Vec3::new(r2 - 0.5, 1.2, seed - 0.5) * 2.0,
                age: 0.0,
                life: 0.9 + 0.8 * r2,
                seed,
                thick: 0.3,
                kind: Kind::Puff,
                size: 0.15 + size,
                swell: 0.6 + 0.6 * r2,
                dusty: false,
            });
        }
    }

    fn add(&mut self, puff: Puff) {
        while self.puffs.len() >= self.most {
            self.puffs.pop_front();
        }
        self.puffs.push_back(puff);
    }

    /// A rear tyre running at `b` on dusty ground: a little dust always,
    /// more and thicker as it spins or the bike pulls away, and a thin
    /// haze that hangs over the track long after (not on Low).
    fn dust(&mut self, track: &mut Track, b: Vec3, velocity: Vec3, kick: Kick, dt: f32, amount: f32) {
        let speed = velocity.length();
        if speed < 4.0 {
            return;
        }
        let pull = ((speed - track.speed) / dt / 20.0).clamp(0.0, 1.5);
        let spin = (kick.spin * 4.0).clamp(0.0, 1.5);
        let hard = pull.max(spin);
        // Fewer puffs on a lower preset, each a little bigger, so the
        // cloud looks much the same.
        track.owed += (3.0 + speed * 0.18) * (1.0 + 1.5 * hard) * dt * amount;
        let bigger = 1.0 / amount.sqrt();
        let back = -velocity / speed;
        let side = back.cross(Vec3::Y).normalize_or(Vec3::X);
        while track.owed >= 1.0 {
            track.owed -= 1.0;
            let (r1, r2, r3, r4, seed) = (self.random(), self.random(), self.random(), self.random(), self.random());
            // Kicked up and back off the tyre, fanning out to the sides; a
            // spinning tyre throws it further back.
            let thrown = velocity * (0.05 + 0.1 * r1) + back * speed * (0.06 + 0.12 * hard * r3) + side * (r2 - 0.5) * (6.0 + speed * 0.12) + Vec3::Y * (1.5 + 3.0 * r3 + 2.0 * hard);
            self.add(Puff {
                at: b + back * 0.8 + side * (r4 - 0.5) * 0.8 + Vec3::Y * 0.3,
                home: b,
                velocity: thrown,
                age: 0.0,
                life: 2.2 + 2.0 * r4,
                seed,
                thick: (0.5 + speed / 60.0 * 0.25 + hard * 0.2).min(0.9),
                kind: Kind::Puff,
                size: (0.4 + 0.3 * r1) * bigger,
                swell: (1.2 + speed * 0.02 + 0.8 * hard) * (0.7 + 0.6 * r2) * bigger,
                dusty: false,
            });
        }
        // The fine dust that hangs in the air over a dry track and drifts
        // off on the wind, thin and wide: a few big, faint puffs.
        if amount < 0.6 || speed < 12.0 {
            return;
        }
        track.haze += (speed / 40.0).min(1.5) * (1.0 + hard) * 0.9 * dt * amount;
        while track.haze >= 1.0 {
            track.haze -= 1.0;
            let (r1, r2, r3, seed) = (self.random(), self.random(), self.random(), self.random());
            self.add(Puff {
                at: b + back * 3.0 + side * (r1 - 0.5) * 2.0 + Vec3::Y * 0.6,
                home: b,
                velocity: velocity * 0.05 + side * (r2 - 0.5) * 3.0 + Vec3::Y * 0.8,
                age: 0.0,
                life: 7.0 + 5.0 * r3,
                seed,
                thick: 0.07 + 0.05 * r2,
                kind: Kind::Puff,
                size: 1.5,
                swell: 6.0 + 5.0 * r1,
                dusty: false,
            });
        }
    }

    /// A rear tyre on dirt at `b`: the roost. Pulling hard or spinning, it
    /// throws a fan of clods back and up (further the harder); sliding
    /// through a turn, out to the side away from it. Wet, the clods are
    /// mud.
    #[allow(clippy::too_many_arguments)]
    fn roost(&mut self, track: &mut Track, b: Vec3, velocity: Vec3, kick: Kick, dt: f32, amount: f32, wet: bool) {
        let speed = velocity.length();
        let level = Vec3::new(velocity.x, 0.0, velocity.z);
        let heading = level.normalize_or_zero();
        // How fast it turns, and so how hard the tyre slides out.
        let turning = if track.heading != Vec3::ZERO && heading != Vec3::ZERO {
            track.heading.cross(heading).y.clamp(-1.0, 1.0).asin() / dt
        } else {
            0.0
        };
        if heading != Vec3::ZERO {
            track.heading = heading;
        }
        if speed < 3.0 {
            return;
        }
        let spin = (kick.spin * 4.0).clamp(0.0, 1.5);
        let slide = ((level.length() * turning.abs() - 30.0) / 45.0).clamp(0.0, 1.0);
        let hard = track.pull.max(spin).max(slide * 0.8);
        if hard < 0.15 {
            return;
        }
        track.clods += (45.0 + 250.0 * hard) * (speed / 15.0 + 0.3).min(1.0) * dt * amount;
        let back = -velocity / speed;
        let side = back.cross(Vec3::Y).normalize_or(Vec3::X);
        // Out of the turn: the tyre steps out that way.
        let out = side * -turning.signum() * slide;
        while track.clods >= 1.0 {
            track.clods -= 1.0;
            let (r1, r2, r3, r4, seed) = (self.random(), self.random(), self.random(), self.random(), self.random());
            let eject = (8.0 + speed * 0.4 + 16.0 * hard) * (0.5 + 0.6 * r1);
            // Up at between 10 and 38 degrees, spreading to the sides: a
            // low fan, seldom higher than the rider's head.
            let rise = 0.18 + 0.48 * r2;
            let fling = back * rise.cos() + Vec3::Y * rise.sin() + side * (r3 - 0.5) * 0.45 + out * 0.9;
            self.add(Puff {
                at: b + back * 0.9 + side * (r4 - 0.5) * 0.4 + Vec3::Y * 0.35,
                home: b,
                velocity: velocity * 0.85 + fling * eject,
                age: 0.0,
                life: 1.8,
                seed,
                thick: if wet { 0.95 } else { 0.9 },
                kind: Kind::Clod,
                // Mostly crumbs, a few lumps.
                size: 0.03 + 0.11 * r4 * r4 * r1,
                swell: 0.0,
                dusty: !wet,
            });
        }
    }

    /// A rear tyre running at `b` on wet ground: a rooster tail of muddy
    /// drops, and now and then a wisp of mist.
    fn spray(&mut self, track: &mut Track, b: Vec3, velocity: Vec3, dt: f32) {
        let speed = velocity.length();
        if speed < 4.0 {
            return;
        }
        track.owed += (40.0 + speed * 3.0) * dt;
        let back = -velocity / speed;
        let side = back.cross(Vec3::Y).normalize_or(Vec3::X);
        while track.owed >= 1.0 {
            track.owed -= 1.0;
            let (r1, r2, r3, r4, seed) = (self.random(), self.random(), self.random(), self.random(), self.random());
            // Flung up off the back of the tyre in a rooster tail, fanning
            // out to the sides.
            let thrown = velocity * (0.2 + 0.2 * r1) + back * speed * (0.1 + 0.15 * r3) + side * (r2 - 0.5) * (4.0 + speed * 0.2) + Vec3::Y * (5.0 + 9.0 * r4 + speed * 0.1);
            self.add(Puff {
                at: b + back * 0.6 + Vec3::Y * 0.4,
                home: b,
                velocity: thrown,
                age: 0.0,
                life: 0.5 + 0.5 * r1,
                seed,
                thick: 0.6 + 0.3 * r2,
                kind: Kind::Drop,
                size: 0.0,
                swell: 0.0,
                dusty: false,
            });
            if seed < 0.06 {
                self.add(Puff {
                    at: b + back * 0.8 + Vec3::Y * 0.3,
                    home: b,
                    velocity: velocity * 0.1 + back * speed * 0.05 + Vec3::Y * 2.0,
                    age: 0.0,
                    life: 1.0 + r3,
                    seed: r4,
                    thick: 0.22,
                    kind: Kind::Puff,
                    size: 1.0,
                    swell: 4.5,
                    dusty: false,
                });
            }
        }
    }

    /// Puts the puffs in the buffer, furthest first, for the eye at `eye`:
    /// only the dust, only the dirt, or both.
    pub fn prepare(&mut self, queue: &wgpu::Queue, eye: Vec3, dust: bool, roost: bool) {
        let mut order: Vec<(f32, &Puff)> = self
            .puffs
            .iter()
            .filter(|p| if p.kind == Kind::Clod { roost } else { dust })
            .map(|p| (p.at.distance_squared(eye), p))
            .collect();
        order.sort_by(|a, b| b.0.total_cmp(&a.0));
        let instances: Vec<Instance> = order
            .iter()
            .map(|(_, p)| {
                let t = p.age / p.life;
                match p.kind {
                    Kind::Drop => {
                        let size = 0.04 + 0.07 * p.seed;
                        return Instance {
                            place: [p.at.x, p.at.y, p.at.z, size],
                            look: [p.thick * (1.0 - t * t), p.velocity.x, p.velocity.y, p.velocity.z],
                            home: [p.home.x, p.home.z, 1.0, 0.0],
                        };
                    }
                    Kind::Clod => {
                        // Tumbling: a picture of its own and a turn that
                        // goes on as it flies.
                        let picture = (p.seed * LAYERS as f32).floor().min(LAYERS as f32 - 1.0);
                        let turn = (p.seed * 13.7 + p.age * (1.5 + 3.0 * p.seed)).fract();
                        return Instance {
                            place: [p.at.x, p.at.y, p.at.z, p.size],
                            look: [p.thick * (1.0 - (t * 4.0 - 3.0).max(0.0)), p.velocity.x, p.velocity.y, p.velocity.z],
                            home: [p.home.x, p.home.z, 2.0, picture + turn.min(0.999)],
                        };
                    }
                    Kind::Puff => {}
                }
                // Swells fast then slowly; fades in quickly and out slowly,
                // thinning as it spreads.
                let size = p.size + p.swell * (1.0 - (-p.age * 1.3).exp()) + p.age * 0.25;
                let thick = p.thick * (t / 0.05).min(1.0) * (1.0 - t).powf(1.6);
                // Raised by about half its size, so it billows up off the
                // ground rather than being cut off flat by it.
                Instance {
                    place: [p.at.x, p.at.y + size * 0.45, p.at.z, size],
                    look: [thick, p.seed, p.age, size],
                    home: [p.home.x, p.home.z, 0.0, 0.0],
                }
            })
            .collect();
        self.count = instances.len() as u32;
        if !instances.is_empty() {
            queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&instances));
        }
    }

    /// The box round the puffs within [`SHADOWED`] ft of `eye` (drawing
    /// coordinates), if any: what the dust's shadow map spans.
    pub fn bounds(&self, eye: Vec3) -> Option<(Vec3, Vec3)> {
        let mut out: Option<(Vec3, Vec3)> = None;
        for p in self.puffs.iter().filter(|p| p.kind == Kind::Puff && p.at.distance_squared(eye) < SHADOWED * SHADOWED) {
            // As prepare draws it: swollen, and raised by half its size.
            let size = p.size + p.swell + p.age * 0.25;
            let r = Vec3::splat(size * 1.2);
            let c = p.at + Vec3::Y * size * 0.45;
            out = Some(out.map_or((c - r, c + r), |(lo, hi)| (lo.min(c - r), hi.max(c + r))));
        }
        out
    }

    /// Draws the puffs from the sun into the dust's shadow map.
    pub fn draw_shadow(&self, pass: &mut wgpu::RenderPass<'_>, terrain: &wgpu::BindGroup) {
        if self.count == 0 {
            return;
        }
        pass.set_pipeline(&self.shadow);
        pass.set_bind_group(0, terrain, &[]);
        pass.set_bind_group(1, &self.shadow_group, &[]);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        pass.draw(0..6, 0..self.count);
    }

    /// Draws the dust, with the terrain's own group and the shared one.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, terrain: &wgpu::BindGroup, shared: &wgpu::BindGroup) {
        if self.count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, terrain, &[]);
        pass.set_bind_group(1, shared, &[]);
        pass.set_bind_group(2, &self.atlas_group, &[]);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        pass.draw(0..6, 0..self.count);
    }
}

/// Gradient noise at `p` (Perlin's 2002 fade), 0 to 1 about a half, as
/// dust.wgsl had it worked out for every pixel.
fn noise(p: [f32; 2]) -> f32 {
    fn hash(x: f32, y: f32) -> f32 {
        // An integer hash of the lattice point: the same everywhere.
        let mut h = (x as i32 as u32).wrapping_mul(0x8da6_b343) ^ (y as i32 as u32).wrapping_mul(0xd816_3841);
        h ^= h >> 15;
        h = h.wrapping_mul(0x2c1b_3c6d);
        h ^= h >> 12;
        (h & 0xffff) as f32 / 65536.0
    }
    let (ix, iy) = (p[0].floor(), p[1].floor());
    let (fx, fy) = (p[0] - ix, p[1] - iy);
    let slope = |cx: f32, cy: f32, dx: f32, dy: f32| {
        let a = hash(cx, cy) * std::f32::consts::TAU;
        a.cos() * dx + a.sin() * dy
    };
    let fade = |f: f32| f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let (ux, uy) = (fade(fx), fade(fy));
    let a = slope(ix, iy, fx, fy);
    let b = slope(ix + 1.0, iy, fx - 1.0, fy);
    let c = slope(ix, iy + 1.0, fx, fy - 1.0);
    let d = slope(ix + 1.0, iy + 1.0, fx - 1.0, fy - 1.0);
    let ab = a + (b - a) * ux;
    let cd = c + (d - c) * ux;
    (0.5 + 1.1 * (ab + (cd - ab) * uy)).clamp(0.0, 1.0)
}

/// Lumps of dust at `q`: four layers of noise, each turned from the last
/// so no grid lines up.
fn lumps(q: [f32; 2]) -> f32 {
    let turn = |v: [f32; 2]| [0.8 * v[0] - 0.6 * v[1], 0.6 * v[0] + 0.8 * v[1]];
    let q2 = turn(q);
    let q3 = turn(q2);
    let q4 = turn(q3);
    noise(q) * 0.5 + noise([q2[0] * 2.1 + 3.1, q2[1] * 2.1 + 3.1]) * 0.27 + noise([q3[0] * 4.3 + 7.7, q3[1] * 4.3 + 7.7]) * 0.15 + noise([q4[0] * 8.9 + 1.3, q4[1] * 8.9 + 1.3]) * 0.08
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The baked pictures of lumpy dust the puffs and clods are drawn from:
/// `layers` of them `side` texels a side, each with `mips` levels, layer
/// by layer. Red is how thick the dust is there (wispy at the rim,
/// clumped inside, nothing past it), green the lumps' height, and blue
/// and alpha how they slope across and up (a half for flat), so a puff is
/// lit lump by lump.
pub fn puff_atlas(side: usize, layers: usize, mips: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for layer in 0..layers {
        // Each its own patch of the noise.
        let off = [layer as f32 * 17.31 + 3.7, layer as f32 * 9.13 + 1.9];
        let at = |x: usize, y: usize| {
            let u = (x as f32 + 0.5) / side as f32 * 2.0 - 1.0;
            let v = (y as f32 + 0.5) / side as f32 * 2.0 - 1.0;
            (u, v, lumps([u * 1.6 + off[0], v * 1.6 + off[1]]))
        };
        let n: Vec<f32> = (0..side * side).map(|i| at(i % side, i / side).2).collect();
        // The lumps' height a step in q apart (q = uv × 1.6).
        let step = 2.0 * 1.6 * 2.0 / side as f32;
        let mut level: Vec<[f32; 4]> = Vec::with_capacity(side * side);
        for y in 0..side {
            for x in 0..side {
                let (u, v, h) = at(x, y);
                let r = (u * u + v * v).sqrt() + (h - 0.5) * 0.9;
                let body = 1.0 - smoothstep(0.2, 0.98, r);
                let clumps = 0.45 + 1.05 * smoothstep(0.3, 0.72, h);
                let thick = if r > 1.0 { 0.0 } else { (body * clumps).clamp(0.0, 1.0) };
                let get = |x: usize, y: usize| n[y.min(side - 1) * side + x.min(side - 1)];
                let sx = (get(x + 1, y) - get(x.saturating_sub(1), y)) / step;
                // Up the picture is down its rows.
                let sy = (get(x, y.saturating_sub(1)) - get(x, y + 1)) / step;
                level.push([thick, h, (0.5 + sx / 8.0).clamp(0.0, 1.0), (0.5 + sy / 8.0).clamp(0.0, 1.0)]);
            }
        }
        let mut s = side;
        for m in 0..mips {
            out.extend(level.iter().flat_map(|t| t.map(|c| (c * 255.0).round() as u8)));
            if m + 1 == mips {
                break;
            }
            let half = s / 2;
            let mut next = Vec::with_capacity(half * half);
            for y in 0..half {
                for x in 0..half {
                    let mut sum = [0.0; 4];
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let t = level[(y * 2 + dy) * s + x * 2 + dx];
                        for c in 0..4 {
                            sum[c] += t[c] * 0.25;
                        }
                    }
                    next.push(sum);
                }
            }
            level = next;
            s = half;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pictures_are_round_puffs() {
        let side = 32;
        let atlas = puff_atlas(side, 2, 3);
        assert_eq!(atlas.len(), 2 * (32 * 32 + 16 * 16 + 8 * 8) * 4);
        // Nothing in the corners, something in the middle.
        assert_eq!(atlas[0], 0);
        let middle = (side / 2 * side + side / 2) * 4;
        assert!(atlas[middle] > 60, "{}", atlas[middle]);
    }
}
