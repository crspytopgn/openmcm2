struct Globals {
    view_proj: mat4x4<f32>,
    camera: vec4<f32>,
    grid: vec4<f32>,
    // The fog's colour, and its start and end in x and y.
    fog: vec4<f32>,
    fog_range: vec4<f32>,
    // The scene's ambient and sun colours (0..1 of the game's 0..255) and
    // the way to the sun.
    ambient: vec4<f32>,
    sun: vec4<f32>,
    to_sun: vec4<f32>,
};

// The game's vertex light (0x4a0e40): ambient plus the sun's colour times
// how squarely the vertex faces it, clamped; as linear light, since it
// scales the texture's sRGB values.
fn scene_light(facing: f32) -> vec3<f32> {
    let c = min(globals.ambient.rgb + globals.sun.rgb * max(facing, 0.0), vec3<f32>(1.0));
    return pow(c, vec3<f32>(2.2));
}

@group(0) @binding(0) var<uniform> globals: Globals;
// Transforms for the parts of animated models; 0 is the identity. A
// texture of four texels (columns) per node, not a storage buffer, since
// older graphics cards' OpenGL has no storage buffers in vertex shaders.
@group(0) @binding(1) var nodes: texture_2d<f32>;

// Node `n`'s transform, from row-wrapped texels (NODE_ROW texels a row).
const NODE_ROW: u32 = 1024u;
fn node_matrix(n: u32) -> mat4x4<f32> {
    let i = n * 4u;
    let at = vec2<i32>(i32(i % NODE_ROW), i32(i / NODE_ROW));
    return mat4x4<f32>(
        textureLoad(nodes, at, 0),
        textureLoad(nodes, at + vec2<i32>(1, 0), 0),
        textureLoad(nodes, at + vec2<i32>(2, 0), 0),
        textureLoad(nodes, at + vec2<i32>(3, 0), 0),
    );
}
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var tex_sampler: sampler;

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) world: vec3<f32>,
    // 1 for a see-through model (a ghost race's ghost).
    @location(4) @interpolate(flat) ghost: f32,
    // The vertex's light.
    @location(5) light: vec3<f32>,
    // How it is lit: UNLIT, PLANT or neither.
    @location(6) @interpolate(flat) mode: u32,
    // 1 for a part of a moving model (a bike or a rider, a gate, ...).
    @location(7) @interpolate(flat) moving: f32,
    // Where on the model at rest, and how dirty the model is, 0 to 1.
    @location(8) rest: vec3<f32>,
    @location(9) @interpolate(flat) mud: f32,
    // Which way it faces on the model at rest.
    @location(10) rest_normal: vec3<f32>,
    // The enhanced drawing's: a plant's height above its foot, ft.
    @location(11) plant_up: f32,
};

// How a vertex is lit, from the top bits of its node (objects.rs).
const UNLIT: u32 = 2u;
const PLANT: u32 = 1u;

fn vertex_light(mode: u32, normal: vec3<f32>) -> vec3<f32> {
    if (mode >= UNLIT) {
        return vec3<f32>(1.0);
    }
    let n = normalize(normal);
    if (mode == PLANT) {
        // A plant: lit with the sun level, and only ambient underneath
        // (0x4577d0, -0.95 at 0x55763c).
        if (n.y <= -0.95) {
            return scene_light(0.0);
        }
        let level = normalize(vec3<f32>(globals.to_sun.x, 0.0, globals.to_sun.z) + vec3<f32>(1e-6, 0.0, 0.0));
        return scene_light(dot(n, level));
    }
    return scene_light(dot(n, globals.to_sun.xyz));
}

// The eco-system's trees give way to their billboards here, ft, over a
// dithered band (objects.rs TREE_FAR, TREE_BAND).
const TREE_FAR: f32 = 450.0;
const TREE_BAND: f32 = 60.0;

// Where a far tree's billboard corner is: its quad turned round the
// upright to face the camera, from the corner as stored facing along z
// (`position`) and its offset from the foot (`off`).
fn billboard(position: vec3<f32>, off: vec2<f32>) -> vec3<f32> {
    let foot = position - vec3<f32>(off.x, off.y, 0.0);
    let f = billboard_facing(foot);
    return foot + vec3<f32>(f.z, 0.0, -f.x) * off.x + vec3<f32>(0.0, off.y, 0.0);
}

// The level way from a billboard's foot to the camera.
fn billboard_facing(foot: vec3<f32>) -> vec3<f32> {
    let to = globals.camera.xyz - foot;
    return normalize(vec3<f32>(to.x, 0.0, to.z) + vec3<f32>(1e-4, 0.0, 0.0));
}

// The sun on a far tree's billboard: it stands in the middle of the tree
// the rays see, so they find its own leaves round it; the leaves the
// billboard stands for are the outer ones, mostly lit.
fn billboard_sun(sun: f32) -> f32 {
    return select(sun, mix(sun, 1.0, 0.6), surface.z > 1.5);
}

// Whether a fragment of a tree, at `world` on screen pixel `pixel`, is
// left to its other look: the full tree past the band, the billboard
// short of it, each by a dither through it.
fn tree_hidden(world: vec3<f32>, pixel: vec2<f32>) -> bool {
    let kind = surface.z;
    if (kind < 0.5) {
        return false;
    }
    let dither = fract(52.9829189 * fract(dot(floor(pixel), vec2<f32>(0.06711056, 0.00583715)))) - 0.5;
    let edge = TREE_FAR + dither * TREE_BAND;
    let d = length(world - globals.camera.xyz);
    if (kind < 1.5) {
        return d > edge;
    }
    return d < edge;
}

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) node: u32,
) -> VertexOut {
    return game_vertex(position, normal, uv, color, node);
}

fn game_vertex(position: vec3<f32>, normal: vec3<f32>, uv: vec2<f32>, color: vec4<f32>, node: u32) -> VertexOut {
    let m = node_matrix(node & 0x3fffffffu);
    var world = (m * vec4<f32>(position, 1.0)).xyz;
    var n = (m * vec4<f32>(normal, 0.0)).xyz;
    if (surface.z > 1.5) {
        // A far tree's billboard, lit as its leaves would be: much as
        // facing the sun, a little toward the eye.
        world = billboard(position, normal.xy);
        let level = normalize(vec3<f32>(globals.to_sun.x, 0.0, globals.to_sun.z) + vec3<f32>(1e-6, 0.0, 0.0));
        n = normalize(level + billboard_facing(world) * 0.5 + vec3<f32>(0.0, 0.3, 0.0));
    }
    var out: VertexOut;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.normal = n;
    out.uv = uv;
    out.color = color;
    out.world = world;
    out.ghost = m[0][3];
    out.light = vertex_light(node >> 30u, out.normal);
    out.mode = node >> 30u;
    // The enhanced drawing's dressing hangs on a node too, but is still.
    out.moving = select(0.0, 1.0, (node & 0x3fffffffu) != 0u && m[2][3] < 0.5);
    out.rest = position;
    out.rest_normal = normal;
    out.mud = m[1][3];
    out.plant_up = 0.0;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let texel = textureSample(tex, tex_sampler, in.uv) * in.color;
    if (texel.a < 0.5 || tree_hidden(in.world, in.clip.xy)) {
        discard;
    }
    // A ghost shows the scene through every other pixel.
    if (in.ghost > 0.5 && ((u32(in.clip.x) + u32(in.clip.y)) & 1u) == 0u) {
        discard;
    }
    let light = in.light;
    let sky = globals.fog.rgb;
    let dist = length(in.world - globals.camera.xyz);
    let range = max(globals.fog_range.y - globals.fog_range.x, 1.0);
    let fog = clamp((dist - globals.fog_range.x) / range, 0.0, 1.0);
    return vec4<f32>(mix(texel.rgb * light, sky, fog), 1.0);
}

// The player's shadow (ProjectedShadow, 0x4defa0): the casters drawn
// from the sun into its mask, as seen along the light.
struct Shadow {
    view_proj: mat4x4<f32>,
};
@group(2) @binding(0) var<uniform> shadow: Shadow;

struct CasterOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) world: vec3<f32>,
};

@vertex
fn vs_caster(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) node: u32,
) -> CasterOut {
    let m = node_matrix(node & 0x3fffffffu);
    var world = (m * vec4<f32>(position, 1.0)).xyz;
    if (surface.z > 1.5) {
        world = billboard(position, normal.xy);
    }
    var out: CasterOut;
    out.clip = shadow.view_proj * vec4<f32>(world, 1.0);
    out.uv = uv;
    out.color = color;
    out.world = world;
    return out;
}

@fragment
fn fs_caster(in: CasterOut) -> @location(0) vec4<f32> {
    let texel = textureSample(tex, tex_sampler, in.uv) * in.color;
    if (texel.a < 0.5) {
        discard;
    }
    return vec4<f32>(1.0);
}

// The sun's depth in each shadow cascade, for the enhanced drawing:
// cut-out texels (leaves, fences) cast no shadow.
@fragment
fn fs_depth(in: CasterOut) {
    let texel = textureSample(tex, tex_sampler, in.uv) * in.color;
    if (texel.a < 0.5 || tree_hidden(in.world, in.clip.xy)) {
        discard;
    }
}

// The moving models' motion on screen, for temporal anti-aliasing
// (post.rs): where each of their pixels was a frame ago, in pixels from
// where it is, and how far it is from the camera.
struct MotionViews {
    now: mat4x4<f32>,
    before: mat4x4<f32>,
    camera: vec4<f32>,
    size: vec4<f32>,
};
@group(2) @binding(1) var<uniform> motion_views: MotionViews;
@group(2) @binding(2) var<storage, read> nodes_before: array<mat4x4<f32>>;

struct MotionOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) now: vec4<f32>,
    @location(3) before: vec4<f32>,
    @location(4) world: vec3<f32>,
};

@vertex
fn vs_motion(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) node: u32,
) -> MotionOut {
    let n = node & 0x3fffffffu;
    let world = (node_matrix(n) * vec4<f32>(position, 1.0)).xyz;
    let was = (nodes_before[n] * vec4<f32>(position, 1.0)).xyz;
    var out: MotionOut;
    // Drawn where the scene is (shifted as it is), measured unshifted.
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.now = motion_views.now * vec4<f32>(world, 1.0);
    out.before = motion_views.before * vec4<f32>(was, 1.0);
    out.uv = uv;
    out.color = color;
    out.world = world;
    return out;
}

@fragment
fn fs_motion(in: MotionOut) -> @location(0) vec4<f32> {
    let texel = textureSample(tex, tex_sampler, in.uv) * in.color;
    if (texel.a < 0.5) {
        discard;
    }
    if (in.before.w <= 0.0 || in.now.w <= 0.0) {
        return vec4<f32>(0.0);
    }
    let moved = (in.before.xy / in.before.w - in.now.xy / in.now.w) * vec2<f32>(0.5, -0.5) * motion_views.size.xy;
    return vec4<f32>(moved, length(in.world - motion_views.camera.xyz), 1.0);
}

// ---- The enhanced drawing (graphics.rs), none of it the game's. ----

#include "enhanced.wgsl"

@group(3) @binding(0) var<uniform> enh: Enh;
@group(3) @binding(1) var sun_map: texture_depth_2d_array;
@group(3) @binding(2) var sun_sampler: sampler_comparison;
// The photo materials (materials.rs), and the one this draw's texture is
// mostly made of (its layer, or -1 for none).
@group(3) @binding(3) var material_colour: texture_2d_array<f32>;
@group(3) @binding(4) var material_normal: texture_2d_array<f32>;
@group(3) @binding(5) var material_sampler: sampler;
// x the photo material's layer (-1 none), y 1 for a banner lit from
// behind at night (materials::backlit), z which drawing of a tree it is
// (0 none).
@group(1) @binding(2) var<uniform> surface: vec4<f32>;

// How brightly a back-lit banner glows at night, by its own colours.
const BACKLIT: f32 = 0.9;

#include "enhanced_fns.wgsl"

// The wind's way and its gusts, as the grass sways (grass.wgsl).
const WIND: vec3<f32> = vec3<f32>(0.7, 0.0, 0.4);
fn gust(time: f32, xz: vec2<f32>) -> f32 {
    return sin(time * 1.6 + xz.x * 0.21 + xz.y * 0.17) * 0.5 + sin(time * 2.7 + xz.x * 0.5) * 0.25;
}

// The game's vertex shader, with the enhanced drawing's extras: plants
// sway in the wind, a little more the taller they stand, their leaves
// fluttering; moving models shade as round, from their smoothed normals.
@vertex
fn vs_enhanced(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) node: u32,
    @location(5) smooth_normal: vec3<f32>,
    @location(6) sway: vec2<f32>,
) -> VertexOut {
    var out = game_vertex(position, normal, uv, color, node);
    if (enh.surfaces.y > 0.5 && out.mode == PLANT && sway.x > 0.0) {
        let time = enh.world.x;
        let up = sway.x;
        // A trunk bends from its foot: the tops of a 30 ft tree move about
        // half a foot; stronger in the rain's wind.
        let windy = 1.0 + enh.world.y * 0.8;
        let bend = min(up * up * 0.0016, 1.6) * windy;
        let g = gust(time * 0.6, out.world.xz * 0.2) + sin(time * 0.9 + sway.y) * 0.35;
        // The leaves flutter on their own, faster and smaller.
        let leaf = sin(time * 6.0 + dot(out.world, vec3<f32>(1.7, 2.3, 1.1)) + sway.y) * 0.1 * smoothstep(0.5, 4.0, up);
        let moved = out.world + WIND * g * bend + vec3<f32>(leaf, leaf * 0.5, leaf * 0.7) * windy;
        out.world = moved;
        out.clip = globals.view_proj * vec4<f32>(moved, 1.0);
        out.plant_up = up;
    }
    if (enh.surfaces.z > 0.5 && dot(smooth_normal, smooth_normal) > 0.5) {
        let m = node_matrix(node & 0x3fffffffu);
        out.normal = (m * vec4<f32>(smooth_normal, 0.0)).xyz;
        out.rest_normal = smooth_normal;
    }
    return out;
}

// How many ft each object material's texture spans: wood, metal,
// concrete, straw.
fn surface_scale(m: i32) -> f32 {
    switch m {
        case 5: { return 3.0; }
        case 6: { return 4.0; }
        case 7: { return 6.0; }
        default: { return 2.5; }
    }
}

struct Surface {
    // The material's colour over its average, and its normal on the model
    // at rest.
    detail: vec3<f32>,
    normal: vec3<f32>,
};

// Material `m` laid over the model at rest from three sides, blended by
// which way it faces, so it needs no texture coordinates of its own and
// holds still on a moving model. Each side's normal is turned onto the
// side it is laid on (Golus 2017, whiteout blend).
fn surface_at(m: i32, p: vec3<f32>, rest_normal: vec3<f32>) -> Surface {
    let s = surface_scale(m);
    let n = normalize(rest_normal);
    var w = pow(abs(n), vec3<f32>(4.0));
    w /= w.x + w.y + w.z;
    let uv_x = p.zy / s;
    let uv_y = p.xz / s;
    let uv_z = p.xy / s;
    let cx = textureSample(material_colour, material_sampler, uv_x, m).rgb;
    let cy = textureSample(material_colour, material_sampler, uv_y, m).rgb;
    let cz = textureSample(material_colour, material_sampler, uv_z, m).rgb;
    let tx = textureSample(material_normal, material_sampler, uv_x, m).xyz * 2.0 - 1.0;
    let ty = textureSample(material_normal, material_sampler, uv_y, m).xyz * 2.0 - 1.0;
    let tz = textureSample(material_normal, material_sampler, uv_z, m).xyz * 2.0 - 1.0;
    let nx = vec3<f32>(tx.xy + n.zy, abs(tx.z) * n.x);
    let ny = vec3<f32>(ty.xy + n.xz, abs(ty.z) * n.y);
    let nz = vec3<f32>(tz.xy + n.xy, abs(tz.z) * n.z);
    var out: Surface;
    out.detail = (cx * w.x + cy * w.y + cz * w.z) / max(enh.material_average[m].rgb, vec3<f32>(0.02));
    out.normal = normalize(nx.zyx * w.x + ny.xzy * w.y + nz.xyz * w.z);
    return out;
}

fn hash3(p: vec3<f32>) -> f32 {
    let q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yzx + 33.33);
    return fract((r.x + r.y) * r.z);
}

fn noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(mix(hash3(i), hash3(i + vec3<f32>(1.0, 0.0, 0.0)), u.x), mix(hash3(i + vec3<f32>(0.0, 1.0, 0.0)), hash3(i + vec3<f32>(1.0, 1.0, 0.0)), u.x), u.y);
    let b = mix(mix(hash3(i + vec3<f32>(0.0, 0.0, 1.0)), hash3(i + vec3<f32>(1.0, 0.0, 1.0)), u.x), mix(hash3(i + vec3<f32>(0.0, 1.0, 1.0)), hash3(i + vec3<f32>(1.0, 1.0, 1.0)), u.x), u.y);
    return mix(a, b, u.z);
}

@fragment
fn fs_enhanced(in: VertexOut) -> @location(0) vec4<f32> {
    let texel = textureSample(tex, tex_sampler, in.uv) * in.color;
    // Alpha to coverage: a sharp edge, but smoothed over a pixel.
    // A far tree's picture is denser than the tree's own leaves seen as
    // far: its leaves a little thinner.
    let cut = select(0.5, 0.62, surface.z > 1.5);
    let edge = (texel.a - cut) / max(fwidth(texel.a), 1e-4) + 0.5;
    // A face seen from behind (leaves, flags) is lit on the side the eye
    // sees. Told by the eye's side of it, not by the triangle's turn: the
    // flip of z from the game's coordinates turns every triangle round.
    var n = normalize(in.normal);
    if (dot(n, globals.camera.xyz - in.world) < 0.0) {
        n = -n;
    }
    // Before any discard: it looks at the pixels around.
    // The bikes and riders, remastered, smooth: their textures' marks are
    // paint and print, not bumps.
    let polished = enh.more.w > 0.5 && in.moving > 0.5;
    // Only a touch of bump from the texture's brightness: more reads as an
    // over-sharpened picture.
    var strength = select(select(0.2, 0.45, in.mode == PLANT), 0.08, polished);
    // The trial track's objects: the grain of the photo material each is
    // made of, its colour taken out so the game's paint stays, and its
    // bumps in place of the texture's brightness (which sparkle on fine
    // paint).
    var grain = vec3<f32>(1.0);
    var bend = vec3<f32>(0.0);
    let m = i32(surface.x);
    if (enh.surfaces.x > 0.5 && m >= 0) {
        let s = surface_at(m, in.rest, in.rest_normal);
        let dist = length(in.world - globals.camera.xyz);
        // Gently, and fading out where the texture's own paint suffices.
        let near = 1.0 - smoothstep(60.0, 220.0, dist);
        // Only its light and dark, softened.
        let lum = clamp(luminance(s.detail), 0.45, 1.8);
        grain = vec3<f32>(mix(1.0, lum, near));
        // How the bumps turn the normal, on the model at rest; laid on
        // the drawn normal as is (the still models' rest is where they
        // stand), on the side the eye sees.
        bend = (s.normal - normalize(in.rest_normal)) * near * 1.6;
        strength *= 1.0 - 0.8 * near;
    }
    let bumped = normalize(bump(n, in.world, luminance(texel.rgb), strength) + bend * sign(dot(n, normalize(in.normal))));
    if (edge <= 0.0 || (texel.a < 0.5 && enh.flags.x < 0.5) || tree_hidden(in.world, in.clip.xy)) {
        discard;
    }
    if (in.ghost > 0.5 && ((u32(in.clip.x) + u32(in.clip.y)) & 1u) == 0u) {
        discard;
    }
    let dist = length(in.world - globals.camera.xyz);
    var light = in.light;
    // What stands still by the track: not a tree, not a rider or bike.
    let roadside = in.moving < 0.5 && surface.z < 0.5;
    if (enh.flags.x > 0.5 && in.mode < UNLIT) {
        var l = globals.to_sun.xyz;
        if (in.mode == PLANT) {
            l = normalize(vec3<f32>(l.x, 0.0, l.z) + vec3<f32>(1e-6, 0.0, 0.0));
        }
        var lit = billboard_sun(sun_visible(in.world, n, dist, 0.0));
        var facing = max(dot(bumped, l), 0.0);
        // What stands still by the track (fences, signs, poles, tents; not
        // the trees, nor the riders) softly: its shadows half as deep, the
        // light wrapping round it, so a rail or post a pixel across
        // doesn't flicker between lit and not as it slides past.
        // At night only: by day the sun's shadows ground them.
        let soft = select(0.0, enh.night.x, roadside);
        lit = mix(lit, mix(lit, 1.0, 0.5), soft);
        facing = mix(facing, dot(bumped, l) * 0.5 + 0.5, soft);
        // Ambient a little brighter from the sky above than the ground below.
        let sky = mix(0.95, 1.15, bumped.y * 0.5 + 0.5);
        let game = globals.ambient.rgb * sky * sky_seen(in.world) + globals.sun.rgb * facing * lit;
        // A soft highlight where the sun glints off it.
        let v = normalize(globals.camera.xyz - in.world);
        let h = normalize(l + v);
        let glint = pow(max(dot(bumped, h), 0.0), 16.0) * 0.08 * lit * select(1.0, 0.0, in.mode == PLANT);
        light = pow(soft_clamp(game), vec3<f32>(2.2)) + globals.sun.rgb * glint;
        if (enh.surfaces.y > 0.5 && in.mode == PLANT) {
            // Leaves glow a little where the sun shines through them,
            // seen from the shade side of the tree, toward the sun.
            let through = pow(max(dot(-v, globals.to_sun.xyz), 0.0), 3.0);
            light += globals.sun.rgb * vec3<f32>(0.9, 1.0, 0.6) * through * 0.5 * lit;
        }
    } else if (in.mode < UNLIT) {
        // The game's own light, only shadowed.
        let lit = billboard_sun(sun_visible(in.world, n, dist, 0.0));
        let game = min(globals.ambient.rgb + globals.sun.rgb * max(dot(n, globals.to_sun.xyz), 0.0) * lit, vec3<f32>(1.0));
        light = pow(game, vec3<f32>(2.2));
    }
    // Mud caked on as it rides: thickest low down, on the wheels, the
    // boots and the underside, in splats that spread as it gets dirtier;
    // dark and glossy when wet, pale and dusty when dry.
    var albedo = texel.rgb * grain;
    var caked = 0.0;
    if (enh.sky.y > 0.5 && in.mud > 0.0 && in.moving > 0.5) {
        let low = 1.0 - smoothstep(0.3, 3.2, in.rest.y);
        let under = smoothstep(0.2, -0.8, n.y) * 0.3;
        let reach = in.mud * (0.1 + 0.9 * low + under);
        let splat = noise3(in.rest * 1.6) * 0.55 + noise3(in.rest * 5.0 + 4.0) * 0.3 + noise3(in.rest * 14.0 + 9.0) * 0.15;
        let over = reach * 1.2 - (1.0 - splat);
        let specks = smoothstep(0.8, 0.84, noise3(in.rest * 24.0)) * smoothstep(0.05, 0.4, reach);
        caked = max(smoothstep(0.0, 0.05, over), specks) * 0.9;
        // Darker at the clumps' edges, where they stand off the paint.
        let rim = 1.0 - 0.35 * (1.0 - smoothstep(0.05, 0.2, over)) * step(0.0, over);
        let wet = enh.world.y;
        let dirt = mix(vec3<f32>(0.3, 0.19, 0.1), vec3<f32>(0.07, 0.045, 0.025), wet) * rim;
        // The paint's detail still shows faintly through a thin coat.
        albedo = mix(albedo, dirt * (0.7 + 0.6 * luminance(texel.rgb)) * (0.85 + 0.3 * splat), caked);
    }
    // A banner lit from behind shows only its own glow at night, nothing
    // else's light on it: steady.
    let glowing = surface.y > 0.5 && enh.night.x > 0.0;
    if (glowing) {
        light *= 1.0 - enh.night.x;
    }
    var colour = albedo * light;
    if (in.mode < UNLIT && in.mode != PLANT) {
        // A stadium's floodlights: soft light from the four corners, and
        // their glints off paint and plastic.
        let v = normalize(globals.camera.xyz - in.world);
        let shine = select(0.2, 0.75, polished) * (1.0 - caked);
        let lamp = lamps(bumped, v, shine);
        colour += (albedo * lamp.x + vec3<f32>(lamp.y * select(0.3, 1.0, polished))) * vec3<f32>(1.0, 0.97, 0.92);
    }
    if (in.mode < UNLIT && !glowing) {
        // What stands by the track the lamps' light round about it, softly.
        if (roadside) {
            colour += albedo * headlight_soft(in.world, n, dist);
        } else {
            colour += albedo * headlight_at(in.world, n, dist);
        }
    }
    // The headlights (on the moving models) and the light poles' heads
    // glow at night.
    for (var i = 0u; i < u32(enh.headlights.y); i++) {
        let lamp = enh.lamps[i];
        let pole = lamp.w >= 10.0;
        if (pole == (in.moving > 0.5)) {
            continue;
        }
        let r = select(0.3, 2.2, pole);
        var near = 1.0 - smoothstep(r * 0.25, r, length(in.world - lamp.xyz));
        // A pole's lamps face down: the glow on the underside of its head.
        if (pole) {
            near *= smoothstep(lamp.y + 0.6, lamp.y - 0.8, in.world.y) * smoothstep(0.3, -0.5, n.y);
        }
        colour += vec3<f32>(1.0, 0.95, 0.85) * near * select(4.0, 6.0, pole) * (lamp.w - select(0.0, 10.0, pole));
    }
    if (caked > 0.0 && enh.world.y > 0.3 && in.mode < UNLIT) {
        // Wet mud shines.
        let v = normalize(globals.camera.xyz - in.world);
        let h = normalize(globals.to_sun.xyz + v);
        colour += globals.sun.rgb * pow(max(dot(bumped, h), 0.0), 40.0) * 0.25 * caked * enh.world.y * sun_visible(in.world, n, dist, 0.0);
    }
    if (polished && in.mode < UNLIT) {
        // Shiny plastic and paint: the brighter, purer colours gloss over,
        // with the sky caught in them at a glancing angle and a hard
        // highlight from the sun.
        let v = normalize(globals.camera.xyz - in.world);
        let sat = max(texel.r, max(texel.g, texel.b)) - min(texel.r, min(texel.g, texel.b));
        let gloss = (smoothstep(0.2, 0.5, sat) * 0.8 + 0.1) * (1.0 - caked);
        let nv = abs(dot(bumped, v));
        let fresnel = 0.04 + 0.96 * pow(1.0 - nv, 5.0);
        let r = reflect(-v, bumped);
        let sky = mix(enh.horizon.rgb, enh.zenith.rgb, clamp(r.y, 0.0, 1.0));
        let ground = enh.horizon.rgb * 0.25;
        var env = select(ground, sky, r.y > -0.05);
        // Ray traced, what is round about shows in the paint.
        if (fresnel * gloss > 0.02) {
            env = reflection(in.world + bumped * 0.05, r, env);
        }
        let lit = sun_visible(in.world, n, dist, 0.0);
        let h = normalize(globals.to_sun.xyz + v);
        let coat = pow(max(dot(bumped, h), 0.0), 80.0) * 0.5 * lit;
        colour = mix(colour, env * 0.6, fresnel * gloss * 0.25) + globals.sun.rgb * coat * gloss;
    }
    // A sponsor's banner lit from behind at night: its own colours glow
    // through it, a little through the dirt on it.
    if (surface.y > 0.5 && enh.night.x > 0.0) {
        colour += texel.rgb * mix(1.0, 0.4, caked) * BACKLIT * enh.night.x;
    }
    return vec4<f32>(apply_haze(colour, in.world, dist), select(1.0, clamp(edge, 0.0, 1.0), enh.flags.x > 0.5));
}
