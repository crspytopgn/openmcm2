//! Grass for the enhanced drawing ([`crate::graphics`]): tufts on a grid
//! round the camera wherever the painted ground is grassy (`grass.wgsl`).
//! Only drawn: the bikes ride through it as the game has them ride over
//! the ground.
//!
//! The grid is cut into tiles ([`TILE`] tufts a side), and a tile wholly
//! outside the view, or wholly past where the grass reaches ([`END`]), is
//! never drawn: the instances of one tile follow each other, so the tiles
//! a camera sees are a few joined ranges ([`crate::cull::runs`]). Near
//! tufts keep all three cards; further off the shader sinks a card at a
//! time away over its own band of distances, so nothing pops, and far
//! tufts draw half the blades. What the picture shows is the same, bar
//! the far blades a few pixels tall.

use glam::{Vec2, Vec3};
use mm2_formats::trn::Terrain;

/// Tufts a side of the grid round the camera (`SIDE` in the shader).
const SIDE: u32 = 384;
/// With the detail reaching further (crate::graphics::Enhanced::far):
/// half as far again, the tufts as close together.
const FAR_SIDE: u32 = 576;
/// Tufts a side of one tile: 32 tufts are 27 ft, small enough that a
/// tile at the edge of the view is mostly in it or mostly out of it,
/// large enough that the tiles a camera sees join into few draws.
const TILE: u32 = 32;
/// Ft between tufts (STEP in the shader).
const STEP: f32 = 0.85;
/// Where the grass reaches, ft (END in the shader, grown with the grid).
const END: f32 = 160.0;
const FAR_END: f32 = 240.0;
/// How far past a tile's edge a tuft may stand: its own width and sway,
/// and a pixel of the anti-aliasing shift, so a tile is only left out
/// when none of its tufts could reach the screen.
const MARGIN: f32 = 2.0;
/// How much taller than the highest ground a tuft may stand (the
/// tallest tuft, 1.6 ft, with room for the wind to bow it).
const TUFT_TOP: f32 = 2.0;

pub struct GrassRenderer {
    pipeline: wgpu::RenderPipeline,
    heights: wgpu::BindGroup,
    side: u32,
    tiles: u32,
    end: f32,
    /// The ground's lowest and highest world heights, with room for a
    /// tuft above and a little below: every tile's box in height.
    y_lo: f32,
    y_hi: f32,
}

/// The grass shader for a grid `side` tufts across: its reach, and where
/// it sinks away, grown with it.
fn source(side: u32) -> String {
    let source = include_str!("grass.wgsl");
    if side == SIDE {
        return source.to_string();
    }
    let k = side as f32 / SIDE as f32;
    source
        .replace("const SIDE: u32 = 384u;", &format!("const SIDE: u32 = {side}u;"))
        .replace("const FADE: f32 = 90.0;", &format!("const FADE: f32 = {:.1};", 90.0 * k))
        .replace("const END: f32 = 160.0;", &format!("const END: f32 = {:.1};", 160.0 * k))
}

/// A tile's corner in the grid, in tufts, from its index (row-major).
fn tile_origin(tile: u32, tiles: u32) -> (u32, u32) {
    ((tile % tiles) * TILE, (tile / tiles) * TILE)
}

/// A tuft's place in the grid, in tufts, from its instance: the tile's
/// corner and its place inside the tile, the same mapping as the shader
/// (`grass.wgsl`), so one tile's instances follow each other.
#[cfg(test)]
fn cell_offset(instance: u32, side: u32) -> (u32, u32) {
    let tiles = side / TILE;
    let tile = instance / (TILE * TILE);
    let within = instance % (TILE * TILE);
    let (ox, oy) = tile_origin(tile, tiles);
    (ox + within % TILE, oy + within / TILE)
}

/// How much of card `card` (0, 1 or 2) is left at `dist` ft: the same
/// bands as the shader, where each card shrinks smoothly away instead
/// of popping off. Card 0 always stays.
#[cfg(test)]
fn card_fade(card: u32, dist: f32) -> f32 {
    let smooth = |a: f32, b: f32, x: f32| {
        let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    match card {
        1 => 1.0 - smooth(55.0, 80.0, dist),
        2 => 1.0 - smooth(30.0, 45.0, dist),
        _ => 1.0,
    }
}

/// The instance ranges a camera sees, joined where they meet: tiles
/// outside `frustum`, or wholly past `end` ft from `camera`, are left
/// out. A tile's box is its tufts' ground, `y_lo..y_hi` high and a
/// margin wider, so a tile is only left out when none of its tufts
/// could reach the screen.
fn visible_ranges(
    side: u32,
    tiles: u32,
    end: f32,
    y_lo: f32,
    y_hi: f32,
    frustum: &crate::cull::Frustum,
    camera: Vec3,
) -> Vec<std::ops::Range<u32>> {
    let centre = (Vec2::new(camera.x, camera.z) / STEP).floor();
    let mut ranges = Vec::new();
    for tile in 0..tiles * tiles {
        let (ox, oy) = tile_origin(tile, tiles);
        let lo = (centre + Vec2::new(ox as f32, oy as f32) - side as f32 / 2.0) * STEP - MARGIN;
        let hi = lo + (TILE as f32 + 1.0) * STEP + 2.0 * MARGIN;
        // Past where the grass reaches: the shader leaves these out too.
        let dx = (lo.x - camera.x).max(0.0).max(camera.x - hi.x);
        let dz = (lo.y - camera.z).max(0.0).max(camera.z - hi.y);
        if dx * dx + dz * dz > end * end {
            continue;
        }
        if !frustum.sees(Vec3::new(lo.x, y_lo, lo.y), Vec3::new(hi.x, y_hi, hi.y)) {
            continue;
        }
        ranges.push(tile * TILE * TILE..(tile + 1) * TILE * TILE);
    }
    let mut joined = Vec::new();
    crate::cull::runs(ranges.into_iter(), &mut joined);
    joined
}

impl GrassRenderer {
    /// With the terrain's own group's layout and the enhanced drawing's
    /// shared one, drawn into `format` with `samples` a pixel.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        terrain: &Terrain,
        terrain_layout: &wgpu::BindGroupLayout,
        shared_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
        samples: u32,
        depth_format: wgpu::TextureFormat,
    ) -> GrassRenderer {
        use wgpu::util::DeviceExt;
        let n = terrain.size as u32;
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("terrain heights"),
                size: wgpu::Extent3d {
                    width: n,
                    height: n,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            bytemuck::cast_slice(&terrain.heights),
        );
        let heights_layout = crate::enhance::layout(
            device,
            "grass heights",
            &[(
                0,
                wgpu::ShaderStages::VERTEX,
                crate::enhance::texture(
                    wgpu::TextureViewDimension::D2,
                    wgpu::TextureSampleType::Float { filterable: false },
                ),
            )],
        );
        let heights = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("grass heights"),
            layout: &heights_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&texture.create_view(&Default::default())),
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("grass"),
            bind_group_layouts: &[Some(terrain_layout), Some(shared_layout), Some(&heights_layout)],
            immediate_size: 0,
        });
        let side = if crate::graphics::enhanced().is_some_and(|e| e.far) { FAR_SIDE } else { SIDE };
        let shader = crate::enhance::shader(device, "grass", &source(side));
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("grass"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: samples,
                alpha_to_coverage_enabled: samples > 1,
                ..Default::default()
            },
            multiview_mask: None,
            cache: None,
        });
        let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
        for h in &terrain.heights {
            lo = lo.min(*h);
            hi = hi.max(*h);
        }
        if !lo.is_finite() {
            (lo, hi) = (0.0, 0.0);
        }
        let y_lo = lo * terrain.scale - 0.1;
        let y_hi = hi * terrain.scale + TUFT_TOP;
        GrassRenderer {
            pipeline,
            heights,
            side,
            tiles: side / TILE,
            end: if side == FAR_SIDE { FAR_END } else { END },
            y_lo,
            y_hi,
        }
    }

    /// Draws the grass the camera sees, with the terrain's own group and
    /// the shared one. Tiles outside the view are never drawn.
    pub fn draw(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        terrain: &wgpu::BindGroup,
        shared: &wgpu::BindGroup,
        frustum: &crate::cull::Frustum,
        camera: Vec3,
    ) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, terrain, &[]);
        pass.set_bind_group(1, shared, &[]);
        pass.set_bind_group(2, &self.heights, &[]);
        let side = self.side;
        for run in visible_ranges(side, self.tiles, self.end, self.y_lo, self.y_hi, frustum, camera) {
            pass.draw(0..18, run);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Mat4;

    #[test]
    fn the_far_grass_shader_reaches_further() {
        let far = super::source(super::FAR_SIDE);
        assert!(far.contains("const SIDE: u32 = 576u;") && far.contains("const END: f32 = 240.0;") && far.contains("const FADE: f32 = 135.0;"));
        assert_eq!(super::source(super::SIDE), include_str!("grass.wgsl"));
    }

    #[test]
    fn every_instance_has_its_own_cell() {
        // The tile mapping covers the grid exactly once: no tuft moves,
        // none doubles, none is lost, on either grid.
        for side in [SIDE, FAR_SIDE] {
            let mut seen = std::collections::HashSet::new();
            for instance in 0..side * side {
                let at = cell_offset(instance, side);
                assert!(at.0 < side && at.1 < side);
                assert!(seen.insert(at), "cell {at:?} twice on a {side} grid");
            }
            assert_eq!(seen.len() as u32, side * side);
        }
    }

    #[test]
    fn cards_fade_smoothly_not_all_at_once() {
        assert_eq!(card_fade(0, 200.0), 1.0);
        assert_eq!(card_fade(1, 0.0), 1.0);
        assert_eq!(card_fade(1, 55.0), 1.0);
        assert_eq!(card_fade(1, 80.0), 0.0);
        assert_eq!(card_fade(2, 30.0), 1.0);
        assert_eq!(card_fade(2, 45.0), 0.0);
        for dist in [0.0, 20.0, 35.0, 40.0, 60.0, 70.0, 90.0, 160.0] {
            assert!(card_fade(1, dist) >= card_fade(1, dist + 1.0));
            assert!(card_fade(2, dist) >= card_fade(2, dist + 1.0));
            assert!(card_fade(1, dist) >= card_fade(2, dist), "the second card outlasts the third at {dist} ft");
        }
    }

    #[test]
    fn far_tufts_cost_fewer_cards() {
        // Cards drawn per tuft within reach, counting each card by how
        // much of it is left: the far two-thirds of the disc draw about
        // a third of the cards they did.
        for (side, end) in [(SIDE, END), (FAR_SIDE, FAR_END)] {
            let (mut before, mut after, mut within) = (0u32, 0f32, 0u32);
            for oy in 0..side {
                for ox in 0..side {
                    let dx = (ox as f32 - side as f32 / 2.0 + 0.5) * STEP;
                    let dz = (oy as f32 - side as f32 / 2.0 + 0.5) * STEP;
                    let dist = dx.hypot(dz);
                    if dist > end {
                        continue;
                    }
                    within += 1;
                    before += 3;
                    after += 1.0 + card_fade(1, dist) + card_fade(2, dist);
                }
            }
            assert_eq!(within + (side * side - within), side * side);
            println!("{side} grid: {within} tufts within reach, {before} cards before, {after:.0} after ({:.0}%)", 100.0 * after / before as f32);
            assert!(after < 0.45 * before as f32, "far tufts still cost most of their cards on a {side} grid");
        }
    }

    #[test]
    fn a_camera_sees_a_wedge_not_the_whole_grid() {
        // A camera at the grid's middle looking down -z with a 57 degree
        // field: the tiles behind it and past the grass's reach are left
        // out, the tiles ahead of it are drawn.
        let proj = Mat4::perspective_rh(1.0, 16.0 / 9.0, 1.0, 1000.0);
        let view = Mat4::look_at_rh(Vec3::ZERO, -Vec3::Z, Vec3::Y);
        let frustum = crate::cull::Frustum::new(proj * view);
        let camera = Vec3::ZERO;
        for (side, end) in [(SIDE, END), (FAR_SIDE, FAR_END)] {
            let tiles = side / TILE;
            let ranges = visible_ranges(side, tiles, end, -50.0, 50.0, &frustum, camera);
            let drawn: u32 = ranges.iter().map(|r| r.end - r.start).sum();
            println!("{side} grid: {drawn} of {} tufts drawn ({:.0}%), {} draws", side * side, 100.0 * drawn as f32 / (side * side) as f32, ranges.len());
            assert!(drawn > 0 && drawn < (side * side) / 2, "a wedge should be well under half the grid on a {side} grid");
            assert!(ranges.len() <= (tiles * tiles) as usize);
            // The tile ahead of the camera, and the one behind it, tell
            // the tale: ahead is low rows (the grid's -z), behind high.
            let ahead = ((tiles / 2 - 2) * tiles + tiles / 2) * TILE * TILE;
            let behind = ((tiles / 2 + 1) * tiles + tiles / 2) * TILE * TILE;
            assert!(ranges.iter().any(|r| r.contains(&ahead)), "the tile ahead is not drawn");
            assert!(!ranges.iter().any(|r| r.contains(&behind)), "the tile behind is drawn");
        }
    }
}
