struct Globals {
    view_proj: mat4x4<f32>,
    camera: vec4<f32>,
    // x: samples per side, y: world units per terrain unit,
    // z: terrain units per texture tile, w: tiles per side
    grid: vec4<f32>,
    // The fog's colour, and its start and end in x and y.
    fog: vec4<f32>,
    fog_range: vec4<f32>,
    // The scene's ambient and sun colours (0..1 of the game's 0..255) and
    // the way to the sun.
    ambient: vec4<f32>,
    sun: vec4<f32>,
    to_sun: vec4<f32>,
    // The player's shadow: from the drawing to its mask, and whether it
    // falls (x).
    shadow_view: mat4x4<f32>,
    shadow_on: vec4<f32>,
};


@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var textures: texture_2d_array<f32>;
@group(0) @binding(2) var tex_sampler: sampler;
@group(0) @binding(3) var tile_map: texture_2d<u32>;
// The scene's detail texture (GrayNoise.tga): a colour, and how much of
// it to lay over each texel in its alpha.
@group(0) @binding(4) var detail: texture_2d<f32>;
@group(0) @binding(5) var detail_sampler: sampler;
// The player's shadow's mask: the bike and rider as the sun sees them.
@group(0) @binding(6) var shadow_mask: texture_2d<f32>;
@group(0) @binding(7) var shadow_sampler: sampler;
// The free pack's fine ground (fine_ground.rs): per terrain sample its
// two ground materials and the second's share, their full-size pictures,
// and per material one over the feet a repeat covers (x) and whether it
// has a picture (y). ground_info.on: x 1 when on, y the baked tiles' feet
// per texel, z the pictures' side.
struct GroundInfo {
    on: vec4<f32>,
    repeats: array<vec4<f32>, 12>,
};
@group(0) @binding(8) var ground_map: texture_2d<u32>;
@group(0) @binding(9) var ground_pictures: texture_2d_array<f32>;
@group(0) @binding(10) var ground_sampler: sampler;
@group(0) @binding(11) var<uniform> ground_info: GroundInfo;

// How much of the player's shadow falls on `world`: the mask laid along
// the light (0x4df8c0's texture matrix), shrunk to half its size as
// 0x4e0520 does, and drawn half dark (0x4df4d0: alpha 0x80).
fn shadow_at(world: vec3<f32>) -> f32 {
    let p = globals.shadow_view * vec4<f32>(world, 1.0);
    let uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    let step = 0.5 / 128.0;
    var m = 0.0;
    m += textureSampleLevel(shadow_mask, shadow_sampler, uv + vec2<f32>(-step, -step), 0.0).r;
    m += textureSampleLevel(shadow_mask, shadow_sampler, uv + vec2<f32>(step, -step), 0.0).r;
    m += textureSampleLevel(shadow_mask, shadow_sampler, uv + vec2<f32>(-step, step), 0.0).r;
    m += textureSampleLevel(shadow_mask, shadow_sampler, uv + vec2<f32>(step, step), 0.0).r;
    let inside = all(uv > vec2<f32>(0.0)) && all(uv < vec2<f32>(1.0));
    return select(0.0, 0.25 * m * 0.5 * globals.shadow_on.x, inside);
}

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) terrain_xz: vec2<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) world: vec3<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) index: u32,
    @location(0) height: f32,
    @location(1) normal: vec4<f32>,
) -> VertexOut {
    let size = u32(globals.grid.x);
    let xz = vec2<f32>(f32(index % size), f32(index / size));
    // z is flipped: the game is left-handed, the viewer right-handed.
    let world = vec3<f32>(xz.x, height, -xz.y) * globals.grid.y;
    var out: VertexOut;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.terrain_xz = xz;
    out.normal = normalize(normal.xyz * vec3<f32>(1.0, 1.0, -1.0));
    out.world = world;
    return out;
}

/// How far into a tile, in tiles, it fades into its neighbour.
const BAND: f32 = 0.08;

/// Tile `cell`'s texture at tile coordinate `t`, which may lie just
/// outside it (the sampler clamps to its edge).
fn tile(t: vec2<f32>, cell: vec2<i32>, ddx: vec2<f32>, ddy: vec2<f32>) -> vec3<f32> {
    let tiles = i32(globals.grid.w);
    let c = clamp(cell, vec2<i32>(0), vec2<i32>(tiles - 1));
    let layer = textureLoad(tile_map, c, 0).r;
    if (layer == 0xffffu) {
        return vec3<f32>(0.45, 0.42, 0.36);
    }
    let at = tile_layer(layer, t - vec2<f32>(c));
    let k = select(1.0, 0.5, layer >= 0x8000u);
    return textureSampleGrad(textures, tex_sampler, at.xy, u32(at.z), ddx * k, ddy * k).rgb;
}

// A tile map value's layer, and where `uv` (0 to 1 over the tile) falls in
// it: four tiles to a layer (renderer::fit_layers) when the top bit is set.
fn tile_layer(raw: u32, uv: vec2<f32>) -> vec3<f32> {
    if (raw < 0x8000u) {
        return vec3<f32>(uv, f32(raw));
    }
    let q = (raw >> 13u) & 3u;
    let edge = 1.0 / f32(textureDimensions(textures).x);
    let inside = clamp(uv, vec2<f32>(edge), vec2<f32>(1.0 - edge));
    return vec3<f32>((inside + vec2<f32>(f32(q & 1u), f32(q >> 1u))) * 0.5, f32(raw & 0x1fffu));
}

// How much material `m`'s picture brightens or darkens the baked colour
// at `feet`: its full-size texel over the same spot blurred to the baked
// tiles' texel size, so only the finer detail is added.
fn fine_ratio(m: u32, feet: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>) -> vec3<f32> {
    let r = ground_info.repeats[m];
    if (r.y == 0.0) {
        return vec3<f32>(1.0);
    }
    let uv = feet * r.x;
    let fine = textureSampleGrad(ground_pictures, ground_sampler, uv, m, dx * r.x, dy * r.x).rgb;
    let baked = log2(max(ground_info.on.y * ground_info.on.z * r.x, 1.0));
    let coarse = textureSampleLevel(ground_pictures, ground_sampler, uv, m, baked).rgb;
    return clamp(fine / max(coarse, vec3<f32>(0.01)), vec3<f32>(0.35), vec3<f32>(2.2));
}

// The free pack's fine detail over the baked colour at terrain sample
// coordinate `xz`, 1 where there is none.
fn fine_ground(xz: vec2<f32>) -> vec3<f32> {
    let feet = xz * globals.grid.y;
    let dx = dpdx(feet);
    let dy = dpdy(feet);
    if (ground_info.on.x == 0.0) {
        return vec3<f32>(1.0);
    }
    // The four samples round the pixel, each giving its two materials
    // their shares; the two with the most between them are drawn.
    let size = vec2<i32>(textureDimensions(ground_map));
    let i0 = vec2<i32>(floor(xz));
    let f = fract(xz);
    var w = array<f32, 12>(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for (var k = 0; k < 4; k++) {
        let o = vec2<i32>(k & 1, k >> 1u);
        let c = textureLoad(ground_map, clamp(i0 + o, vec2<i32>(0), size - 1), 0);
        let bw = select(1.0 - f.x, f.x, o.x == 1) * select(1.0 - f.y, f.y, o.y == 1);
        let share = f32(c.b) / 255.0;
        w[min(c.r, 11u)] += bw * (1.0 - share);
        w[min(c.g, 11u)] += bw * share;
    }
    var a = 0u;
    for (var m = 1u; m < 12u; m++) {
        if (w[m] > w[a]) {
            a = m;
        }
    }
    var b = select(0u, 1u, a == 0u);
    for (var m = 0u; m < 12u; m++) {
        if (m != a && w[m] > w[b]) {
            b = m;
        }
    }
    let wb = w[b] / max(w[a] + w[b], 1e-4);
    var ratio = fine_ratio(a, feet, dx, dy) * (1.0 - wb);
    if (wb > 0.02) {
        ratio += fine_ratio(b, feet, dx, dy) * wb;
    } else {
        ratio += vec3<f32>(wb);
    }
    return ratio;
}

// The ground's colour at a pixel, as the game shows it, before its
// shadow and fog.
fn ground(in: VertexOut) -> vec3<f32> {
    // Continuous coordinates in tiles, so mip selection has no seams.
    let t = in.terrain_xz / globals.grid.z;
    let ddx = dpdx(t);
    let ddy = dpdy(t);
    let cell = vec2<i32>(floor(t));
    // Each tile is its own texture, so their edges need not match (the
    // upscaled ones least of all): near an edge, fade into the next tile
    // so no line shows. Halfway across the band at the edge itself, both
    // sides take half of each, so the colour runs on across it.
    let f = fract(t);
    let towards = vec2<i32>(select(-1, 1, f.x >= 0.5), select(-1, 1, f.y >= 0.5));
    let edge = min(f, 1.0 - f);
    let blend = 0.5 * (1.0 - smoothstep(vec2<f32>(0.0), vec2<f32>(BAND), edge));
    // Most of a tile lies outside the bands (blend 0 there), where the
    // neighbours' share is nothing and so they are not looked up: the
    // colour is the same to the bit.
    let own = tile(t, cell, ddx, ddy);
    var row = own;
    if (blend.x > 0.0) {
        row = mix(own, tile(t, cell + vec2<i32>(towards.x, 0), ddx, ddy), blend.x);
    }
    var mixed_tiles = row;
    if (blend.y > 0.0) {
        var other = tile(t, cell + vec2<i32>(0, towards.y), ddx, ddy);
        if (blend.x > 0.0) {
            other = mix(other, tile(t, cell + towards, ddx, ddy), blend.x);
        }
        mixed_tiles = mix(row, other, blend.y);
    }
    let base = mixed_tiles * fine_ground(in.terrain_xz);
    // The detail texture, once over each tile, blended over the texel in
    // the game's own colour space (its second texture stage,
    // BLENDTEXTUREALPHA).
    // 0x50b990 maps it as the tile's own texture coordinates.
    let d = textureSampleGrad(detail, detail_sampler, 0.03125 + 0.9375 * f, 0.9375 * ddx, 0.9375 * ddy);
    let mixed = mix(pow(base, vec3<f32>(1.0 / 2.2)), d.rgb, d.a);
    return pow(mixed, vec3<f32>(2.2));
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let detailed = ground(in);
    // The game never lights the terrain: its textures have the light
    // painted in. 0x47fed0 could light it as the objects, but only when
    // 0x50b140 is asked to (+0xc34), and the scene loader (0x4e3f14)
    // never asks.
    var light = 1.0 - shadow_at(in.world);
    // Ground with no light painted into it (the Free Pack's and the
    // Course Builder's tracks): shaded softly as it faces the sun, level
    // ground as it was, so hills read as hills.
    if (globals.shadow_on.y > 0.0) {
        let n = normalize(in.normal);
        let lum = vec3<f32>(0.3, 0.59, 0.11);
        let amb = dot(globals.ambient.rgb, lum);
        let sun = dot(globals.sun.rgb, lum);
        let level = amb + sun * max(globals.to_sun.y, 0.25);
        let here = amb + sun * max(dot(n, globals.to_sun.xyz), 0.0);
        light *= clamp(mix(1.0, here / max(level, 0.05), 0.75 * globals.shadow_on.y), 0.55, 1.2);
    }
    let sky = globals.fog.rgb;
    let dist = length(in.world - globals.camera.xyz);
    let range = max(globals.fog_range.y - globals.fog_range.x, 1.0);
    let fog = clamp((dist - globals.fog_range.x) / range, 0.0, 1.0);
    return vec4<f32>(mix(detailed * light, sky, fog), 1.0);
}

// ---- The enhanced drawing (graphics.rs), none of it the game's. ----

#include "enhanced.wgsl"

@group(1) @binding(0) var<uniform> enh: Enh;
@group(1) @binding(1) var sun_map: texture_depth_2d_array;
@group(1) @binding(2) var sun_sampler: sampler_comparison;
// The photo materials (materials.rs): colour and roughness, normal and
// height; dirt, packed dirt, grass, rock, sand.
@group(1) @binding(3) var material_colour: texture_2d_array<f32>;
@group(1) @binding(4) var material_normal: texture_2d_array<f32>;
@group(1) @binding(5) var material_sampler: sampler;
// How deep the tyres have rutted the ground.
@group(1) @binding(6) var rut_map: texture_2d<f32>;
@group(1) @binding(7) var rut_sampler: sampler;

#include "enhanced_fns.wgsl"

// The game's light, ambient plus the sun as squarely as `n` faces it.
fn game_light(n: vec3<f32>, sun: f32) -> vec3<f32> {
    return globals.ambient.rgb + globals.sun.rgb * max(dot(n, globals.to_sun.xyz), 0.0) * sun;
}

const DIRT: i32 = 0;
const PACKED: i32 = 1;
const GRASS: i32 = 2;
const ROCK: i32 = 3;
const SAND: i32 = 4;

// How many ft each material's texture spans.
fn material_scale(m: i32) -> f32 {
    switch m {
        case 0: { return 5.0; }
        case 1: { return 7.0; }
        case 2: { return 4.0; }
        case 3: { return 14.0; }
        default: { return 8.0; }
    }
}

struct Sample {
    colour: vec3<f32>,
    rough: f32,
    // The normal in the texture's own frame (z out of it), and the height.
    normal: vec3<f32>,
    height: f32,
};

// Material `m` at `uv` (in its texture's own repeats), with the
// derivatives taken before any branch.
fn sample_material(m: i32, uv: vec2<f32>, ddx: vec2<f32>, ddy: vec2<f32>) -> Sample {
    let c = textureSampleGrad(material_colour, material_sampler, uv, m, ddx, ddy);
    let n = textureSampleGrad(material_normal, material_sampler, uv, m, ddx, ddy);
    var out: Sample;
    out.colour = c.rgb / max(enh.material_average[m].rgb, vec3<f32>(0.02));
    out.rough = c.a;
    out.normal = normalize(n.xyz * 2.0 - 1.0);
    out.height = n.a;
    return out;
}

// Material `m` on the level, seen from above, twice at different sizes
// and turns so its repeats do not show; `dx`, `dy` are the derivatives of
// the level's x, z, taken before any branch.
// Far off (`single` 1), where its repeats are too small to see, the
// second look is left out: each is the texture's own on average.
fn material_level(m: i32, world: vec3<f32>, dx: vec2<f32>, dy: vec2<f32>, single: f32) -> Sample {
    let s = material_scale(m);
    let uv = world.xz / s;
    let a = sample_material(m, uv, dx / s, dy / s);
    if (single >= 1.0) {
        return a;
    }
    let turn = mat2x2<f32>(0.8 * 0.37, 0.6 * 0.37, -0.6 * 0.37, 0.8 * 0.37);
    let uv2 = turn * uv + 0.31;
    let b = sample_material(m, uv2, turn * (dx / s), turn * (dy / s));
    var out: Sample;
    out.colour = mix(a.colour * mix(vec3<f32>(1.0), b.colour, 0.45), a.colour, single);
    out.rough = mix(mix(a.rough, b.rough, 0.3), a.rough, single);
    out.normal = normalize(mix(vec3<f32>(a.normal.xy + b.normal.xy * 0.5, a.normal.z), a.normal, single));
    out.height = mix(mix(a.height, b.height, 0.35), a.height, single);
    return out;
}

// The ground's own sorts by its painted colour (in the game's 0..1) and
// how steep it is: how much is grass, rock, sand and packed dirt (the
// rest is loose dirt).
fn sorts(painted: vec3<f32>, up: f32) -> vec4<f32> {
    let c = pow(painted, vec3<f32>(1.0 / 2.2));
    let lum = dot(c, vec3<f32>(0.3, 0.55, 0.15));
    let hi = max(c.r, max(c.g, c.b));
    let lo = min(c.r, min(c.g, c.b));
    let sat = (hi - lo) / max(hi, 0.01);
    let grass = smoothstep(0.0, 0.05, c.g - c.r * 0.93) * smoothstep(0.0, 0.03, c.g - c.b);
    let grey = 1.0 - smoothstep(0.12, 0.25, sat);
    let steep = smoothstep(0.62, 0.42, up);
    let rock = max(grey * smoothstep(0.25, 0.4, lum) * 0.8, steep) * (1.0 - grass * 0.7);
    let sand = smoothstep(0.55, 0.7, lum) * smoothstep(0.1, 0.25, c.r - c.b) * (1.0 - rock);
    let packed = smoothstep(0.38, 0.52, lum) * (1.0 - sand) * (1.0 - grass) * (1.0 - rock);
    return vec4<f32>(grass, rock, sand, packed);
}

// The game's sun and sky light on a surface facing `n` (0..1 of 255 each),
// with `sun` of the sun reaching it.
fn real_light(n: vec3<f32>, sun: f32) -> vec3<f32> {
    let sky = mix(0.95, 1.3, n.y * 0.5 + 0.5);
    return globals.ambient.rgb * sky + globals.sun.rgb * max(dot(n, globals.to_sun.xyz), 0.0) * sun;
}

// The same where only `seen` of the sky's light gets past the hills.
fn real_light_seen(n: vec3<f32>, sun: f32, seen: f32) -> vec3<f32> {
    let sky = mix(0.95, 1.3, n.y * 0.5 + 0.5) * seen;
    return globals.ambient.rgb * sky + globals.sun.rgb * max(dot(n, globals.to_sun.xyz), 0.0) * sun;
}

// How much still water the ground drawn last holds, for the screen's
// reflections (post.wgsl fs_ssr): 0 dry.
var<private> water_here: f32 = 0.0;

// How far down a rut is, 0 to 1, from the ruts' map (ruts.rs): sunk
// once a few tyres have run there, deeper as more do.
fn rut_shape(v: f32) -> f32 {
    return smoothstep(0.16, 0.3, v) * 0.6 + smoothstep(0.3, 1.0, v) * 0.4;
}

// The new ground: the painted colour with its light taken out, the photo
// materials' detail laid over it by what each part of the ground is, and
// then lit by the sun as it really falls, bumps, ruts and shadows and all.
fn new_ground(in: VertexOut, painted: vec3<f32>, n: vec3<f32>, dist: f32) -> vec3<f32> {
    let w = sorts(painted, n.y);
    // Blend by weight and by height, so grass grows between the stones
    // rather than fading into them.
    var weights = array<f32, 5>(
        max(1.0 - w.x - w.y - w.z - w.w, 0.0),
        w.w,
        w.x,
        w.y,
        w.z,
    );
    // How near the detail shows (see below), and where only one look of
    // each material is needed.
    let detail = enh.haze.w;
    let reach = 0.5 + detail;
    let near = 1.0 - smoothstep(180.0 * reach, 450.0 * reach, dist);
    let single = smoothstep(80.0, 120.0, dist);
    // Only the materials that are here are looked up, and only where their
    // detail shows; past it, only their sheen and the puddles' hollows
    // would use them, and an average does.
    let dx = dpdx(in.world.xz);
    let dy = dpdy(in.world.xz);
    var level: array<Sample, 5>;
    for (var m = 0; m < 5; m++) {
        if (near > 0.0 && weights[m] > 0.0) {
            level[m] = material_level(m, in.world, dx, dy, single);
        } else {
            level[m] = Sample(vec3<f32>(1.0), 0.6, vec3<f32>(0.0, 0.0, 1.0), 0.4);
        }
    }
    // Rock is seen from three sides, so cliffs are not smeared.
    let rs = material_scale(ROCK);
    let dzy = vec2<f32>(dpdx(in.world.z), dpdx(in.world.y)) / rs;
    let dzy2 = vec2<f32>(dpdy(in.world.z), dpdy(in.world.y)) / rs;
    let dxy = dpdx(in.world.xy) / rs;
    let dxy2 = dpdy(in.world.xy) / rs;
    var side_x: Sample;
    var side_z: Sample;
    if (weights[ROCK] > 0.0 && near > 0.0) {
        side_x = sample_material(ROCK, in.world.zy / rs, dzy, dzy2);
        side_z = sample_material(ROCK, in.world.xy / rs, dxy, dxy2);
    } else {
        side_x = level[ROCK];
        side_z = level[ROCK];
    }
    var best = 0.0;
    for (var m = 0; m < 5; m++) {
        weights[m] = weights[m] * (0.35 + level[m].height);
        best = max(best, weights[m]);
    }
    var total = 0.0;
    for (var m = 0; m < 5; m++) {
        weights[m] = max(weights[m] - best * 0.55, 0.0);
        total += weights[m];
    }
    var colour = vec3<f32>(0.0);
    var normal = vec3<f32>(0.0);
    var rough = 0.0;
    var surface = 0.0;
    for (var m = 0; m < 5; m++) {
        if (weights[m] <= 0.0) {
            continue;
        }
        let k = weights[m] / max(total, 1e-4);
        var s = level[m];
        if (m == ROCK) {
            let t = abs(n);
            let tw = t * t * t / dot(t * t, t);
            s.colour = s.colour * tw.y + side_x.colour * tw.x + side_z.colour * tw.z;
            s.normal = normalize(s.normal * tw.y + side_x.normal * tw.x + side_z.normal * tw.z);
        }
        colour += s.colour * k;
        normal += s.normal * k;
        rough += s.rough * k;
        surface += level[m].height * k;
    }
    // The detail fades with distance, where the painted colour suffices;
    // mostly its light and dark, a little of its own colour.
    // `detail=` (enh.haze.w, 0 to 1) sets how strongly, and how far.
    colour = mix(vec3<f32>(luminance(colour)), colour, 0.35);
    // Gently: the painted ground stays what the eye reads.
    colour = mix(vec3<f32>(1.0), colour, near * (0.35 + 0.5 * detail));
    // Further off, where the painted texture is stretched soft, the main
    // material again at six times its size: only its light and dark,
    // faintly, so the middle distance has grain without new colour.
    var main = 0;
    for (var m = 1; m < 5; m++) {
        if (weights[m] > weights[main]) {
            main = m;
        }
    }
    let big = material_scale(main) * 6.0;
    let grain = textureSampleGrad(material_colour, material_sampler, in.world.xz / big, main, dx / big, dy / big).rgb
        / max(enh.material_average[main].rgb, vec3<f32>(0.02));
    let middle = smoothstep(40.0, 160.0, dist) * (1.0 - smoothstep(700.0, 1600.0, dist));
    colour *= mix(1.0, luminance(grain), middle * 0.9 * detail);
    // The normal from the texture's frame to the world's: across is +x,
    // up the texture is -z.
    let t = normalize(vec3<f32>(1.0, 0.0, 0.0) - n * n.x);
    let b = cross(t, n);
    let nm = normalize(mix(vec3<f32>(0.0, 0.0, 1.0), normalize(normal), near * (0.4 + 0.5 * detail)));
    var bent = normalize(t * nm.x + b * nm.y + n * nm.z);
    // The ruts: dark, damp grooves where the tyres have run.
    var rut = 0.0;
    var worn = 0.0;
    var lip = 0.0;
    if (enh.ruts.w > 1.5) {
        // The gentle wear: where the tyres have run, again and again, the
        // ground packs a little darker and smoother, a soft band with no
        // grooves, blurred over a couple of feet.
        let ruv = (in.world.xz - enh.ruts.xy) / enh.ruts.z;
        let r = 1.0 / enh.ruts.z;
        var sum = 0.0;
        for (var i = 0; i < 9; i++) {
            let o = vec2<f32>(f32(i % 3) - 1.0, f32(i / 3) - 1.0) * 1.4 * r;
            sum += textureSampleLevel(rut_map, rut_sampler, ruv + o, 0.0).r;
        }
        worn = smoothstep(0.0, 0.05, sum / 9.0);
    } else if (enh.ruts.w > 0.5) {
        // Tyre marks that build up into ruts: each pass of a tyre adds a
        // little to the map (ruts.rs), so a line ridden once is a faint,
        // packed mark, and the racing line ridden lap after lap sinks
        // into a dark, damp rut with loose dirt pushed up either side and
        // a gently packed band round it all. Close by only the shape and
        // the dirt's grain; far off just the band, as the map's texels
        // would shimmer.
        let ruv = (in.world.xz - enh.ruts.xy) / enh.ruts.z;
        let e = 1.5 / 8192.0;
        let c = textureSampleLevel(rut_map, rut_sampler, ruv, 0.0).r;
        let px = textureSampleLevel(rut_map, rut_sampler, ruv + vec2<f32>(e, 0.0), 0.0).r;
        let nx = textureSampleLevel(rut_map, rut_sampler, ruv - vec2<f32>(e, 0.0), 0.0).r;
        let pz = textureSampleLevel(rut_map, rut_sampler, ruv + vec2<f32>(0.0, e), 0.0).r;
        let nz = textureSampleLevel(rut_map, rut_sampler, ruv - vec2<f32>(0.0, e), 0.0).r;
        var ring = 0.0;
        for (var i = 0; i < 4; i++) {
            let o = vec2<f32>(f32(i % 2) * 2.0 - 1.0, f32(i / 2) * 2.0 - 1.0) * 2.4 * e;
            ring += textureSampleLevel(rut_map, rut_sampler, ruv + o, 0.0).r;
        }
        let blur = (px + nx + pz + nz + ring) / 8.0;
        let close = 1.0 - smoothstep(50.0 * reach, 160.0 * reach, dist);
        // Read as a contour of the smooth map, so a rut has a crisp, true
        // edge however coarse the map's texels; broken up by the dirt's
        // own grain, so no line is drawn clean.
        let rough_edge = 0.75 + 0.5 * clamp(surface, 0.0, 1.0);
        let here = mix(blur, c, close) * mix(1.0, rough_edge, close);
        let mark = smoothstep(0.02, 0.07, here);
        rut = smoothstep(0.16, 0.3, here);
        let deep = smoothstep(0.3, 1.0, here);
        // Loose dirt pushed out just beside a rut.
        lip = smoothstep(0.06, 0.16, blur) * (1.0 - smoothstep(0.08, 0.18, here)) * close;
        worn = smoothstep(0.0, 0.3, blur) * (0.15 + 0.15 * (1.0 - close)) + mark * 0.12 * (1.0 - rut) + deep * 0.25;
        // The rut's walls, from its shape round about.
        let hx = rut_shape(px) - rut_shape(nx);
        let hz = rut_shape(pz) - rut_shape(nz);
        bent = normalize(bent + vec3<f32>(hx, 0.0, hz) * 1.2 * close * near);
    }
    // Take out the light the painter put in: as the scene's sun would
    // have lit the land's shape, half of it.
    let flat_light = globals.ambient.rgb + globals.sun.rgb * max(globals.to_sun.y, 0.0);
    let baked = mix(vec3<f32>(1.0), max(real_light(n, 1.0), vec3<f32>(0.05)) / max(flat_light, vec3<f32>(0.05)), 0.5);
    let albedo_game = pow(painted, vec3<f32>(1.0 / 2.2)) / baked;
    var albedo = pow(max(albedo_game, vec3<f32>(0.0)), vec3<f32>(2.2)) * colour;
    albedo *= 1.0 - rut * 0.16;
    albedo *= mix(vec3<f32>(1.0), vec3<f32>(0.55, 0.52, 0.5), worn);
    // Loose dirt thrown up beside a rut: drier, a little lighter.
    albedo *= 1.0 + lip * vec3<f32>(0.2, 0.19, 0.16);
    // The wet ground is darker.
    albedo *= 1.0 - enh.world.y * 0.35;
    // In the wet, puddles in the hollows of flat ground and in the ruts:
    // still water, darker below, the sky in it.
    var puddle = 0.0;
    if (enh.world.y > 0.0) {
        let swell = 0.5 + 0.3 * sin(in.world.x * 0.071 + sin(in.world.z * 0.05) * 2.0) * sin(in.world.z * 0.093);
        let low = 1.0 - smoothstep(0.32, 0.42, surface * 0.6 + swell * 0.5);
        puddle = clamp(max(low * smoothstep(0.95, 0.99, n.y), smoothstep(0.3, 0.7, rut)) * enh.world.y, 0.0, 1.0);
        bent = normalize(mix(bent, n, puddle));
        albedo *= 1.0 - puddle * 0.4;
    }
    let sun = sun_visible(in.world, n, dist, 0.4);
    // At night, against the track's own light, so the ground goes dark.
    let against = mix(flat_light, enh.night.yzw, enh.night.x);
    let light = real_light_seen(bent, sun, sky_seen(in.world)) / max(against, vec3<f32>(0.05));
    var lit = albedo * pow(max(light, vec3<f32>(0.0)), vec3<f32>(2.2)) * 1.05;
    // The bikes' headlights, as they light the ground's own colour.
    lit += albedo * headlight_at(in.world, n, dist);
    if (puddle > 0.0) {
        let eye = normalize(globals.camera.xyz - in.world);
        let r = reflect(-eye, n);
        var sky = mix(enh.horizon.rgb, enh.zenith.rgb, clamp(r.y, 0.0, 1.0));
        let fresnel = 0.02 + 0.98 * pow(1.0 - max(dot(eye, n), 0.0), 5.0);
        // Ray traced, what stands round the water shows in it too.
        if (fresnel * puddle > 0.02) {
            sky = reflection(in.world + n * 0.1, r, sky);
        }
        lit = mix(lit, sky, fresnel * puddle * 0.85);
    }
    // For the screen's reflections: the puddles' still water, and the rest
    // of the wet ground a dim mirror.
    if (enh.sky_light.w > 0.5 && enh.world.y > 0.0) {
        water_here = clamp(puddle * 0.85 + enh.world.y * 0.2 * (1.0 - puddle), 0.0, 1.0) * (1.0 - haze_amount(dist));
    }
    // Ray traced, the rest of the wet ground is a dim, rippled mirror
    // too, as water's glancing angles make it: the far track shines with
    // the sky and what stands by it, the ground at the wheels hardly.
    if (enh.world.y > 0.0 && enh.traced.x > 0.5) {
        let eye = normalize(globals.camera.xyz - in.world);
        let rn = normalize(mix(bent, n, 0.8));
        let r = reflect(-eye, rn);
        let fresnel = 0.02 + 0.98 * pow(1.0 - max(dot(eye, rn), 0.0), 5.0);
        // Dirt is no lake: at most a sixth of what it shows is reflected.
        let wet = min(fresnel * enh.world.y * (1.0 - puddle) * 0.35, 0.16);
        if (wet > 0.03 && r.y > 0.0) {
            let sky = mix(enh.horizon.rgb, enh.zenith.rgb, r.y);
            lit = mix(lit, reflection(in.world + n * 0.1, r, sky), wet);
        }
    }
    // A sheen off the smoother and the wet ground.
    let v = normalize(globals.camera.xyz - in.world);
    let h = normalize(globals.to_sun.xyz + v);
    let r = clamp(mix(rough, 0.15, enh.world.y) - rut * 0.1 - worn * 0.25, 0.08, 1.0);
    let power = 2.0 / (r * r * r * r) - 2.0;
    let glint = pow(max(dot(bent, h), 0.0), power) * (power + 2.0) / 8.0 * 0.04 * sun * max(dot(bent, globals.to_sun.xyz), 0.0);
    lit += globals.sun.rgb * glint;
    // Under a stadium's floodlights the packed dirt catches their glints.
    let lamp = lamps(bent, v, clamp(1.0 - r, 0.0, 1.0) * 0.5 + worn * 0.2);
    lit += (albedo * lamp.x + albedo * lamp.y * 1.5) * vec3<f32>(1.0, 0.97, 0.92);
    return lit;
}

@fragment
fn fs_enhanced(in: VertexOut) -> @location(0) vec4<f32> {
    // Under the smooth ground round the eye (smooth.rs), the game's own
    // mesh gives way to it, but only SMOOTH_OVERLAP in from the patch's
    // edge: meshes meeting edge to edge leave pixels neither covers, and
    // the haze showed through as a bright line.
    let p = in.terrain_xz - enh.fine_ground.xy;
    let inside = vec2<f32>(SMOOTH_OVERLAP);
    if (enh.fine_ground.w > 0.5 && all(p > inside) && all(p < vec2<f32>(enh.fine_ground.z) - inside)) {
        discard;
    }
    return enhanced_ground(in);
}

// The ray-traced shadows' depth pass (rt.rs): the game's mesh where it is
// drawn, giving way to the smooth ground as fs_enhanced does, so the depth
// is of the very ground the scene shows.
@fragment
fn fs_rt_depth(in: VertexOut) {
    let p = in.terrain_xz - enh.fine_ground.xy;
    let inside = vec2<f32>(SMOOTH_OVERLAP);
    if (enh.fine_ground.w > 0.5 && all(p > inside) && all(p < vec2<f32>(enh.fine_ground.z) - inside)) {
        discard;
    }
}

@fragment
fn fs_smooth(in: VertexOut) -> @location(0) vec4<f32> {
    return enhanced_ground(in);
}

fn enhanced_ground(in: VertexOut) -> vec4<f32> {
    let colour = ground(in);
    let n = normalize(in.normal);
    let dist = length(in.world - globals.camera.xyz);
    // The new ground lights itself: none of the plain light below is
    // needed, and its shadow lookups were most of a pixel's cost.
    if (enh.more.x > 0.5) {
        let lit = new_ground(in, colour, n, dist);
        return vec4<f32>(apply_haze(lit, in.world, dist), select(1.0, 2.0 + water_here, water_here > 0.0));
    }
    let bumped = bump(n, in.world, luminance(colour), 1.2);
    // The textures have the land's light painted in, so keep it: only the
    // small bumps the texture suggests, and the objects' shadows, change
    // it, as they change the light the game would give the ground here.
    var light = vec3<f32>(1.0 - shadow_at(in.world));
    // At night against the track's own light, so the ground goes dark.
    let plain = max(mix(game_light(n, 1.0), enh.night.yzw, enh.night.x), vec3<f32>(0.05));
    if (enh.flags.y > 0.5) {
        let sun = sun_visible(in.world, n, dist, 0.4);
        light = game_light(n, sun) / plain;
    }
    if (enh.flags.x > 0.5) {
        let sun = select(1.0, sun_visible(in.world, n, dist, 0.4), enh.flags.y > 0.5);
        light = game_light(bumped, sun) / plain;
    }
    var lit = colour * pow(max(light, vec3<f32>(0.0)), vec3<f32>(2.2));
    lit += colour * headlight_at(in.world, n, dist);
    // Water marked past 2 in the alpha, which nothing else writes there.
    return vec4<f32>(apply_haze(lit, in.world, dist), select(1.0, 2.0 + water_here, water_here > 0.0));
}

// ---- The smooth ground round the eye (smooth.rs), only drawn. ----

// The terrain's heights (terrain units) and the game's normals.
@group(2) @binding(0) var heights: texture_2d<f32>;
@group(2) @binding(1) var normals: texture_2d<f32>;

// Smoothing moves the ground at most this far from the game's, ft.
const SMOOTH_MOST: f32 = 0.35;
// Cells over which the smoothing fades in from the patch's edge.
const SMOOTH_FADE: f32 = 6.0;
// Cells the game's mesh carries on under the patch's edge (fs_enhanced);
// the smoothing there is a hundredth of a foot at most.
const SMOOTH_OVERLAP: f32 = 0.5;
// Each cell is cut into this many a side (smooth.rs SPLIT).
const SMOOTH_SPLIT: u32 = 4u;

fn height_cell(i: vec2<i32>) -> f32 {
    let last = i32(globals.grid.x) - 1;
    return textureLoad(heights, clamp(i, vec2<i32>(0), vec2<i32>(last)), 0).r;
}

fn normal_cell(i: vec2<i32>) -> vec3<f32> {
    let last = i32(globals.grid.x) - 1;
    return textureLoad(normals, clamp(i, vec2<i32>(0), vec2<i32>(last)), 0).xyz;
}

// The game's ground at terrain point `t`: its triangles, split as the
// physics samples them (ride::ground), and their normals blended alike.
fn game_ground(t: vec2<f32>) -> vec4<f32> {
    let i = vec2<i32>(floor(t));
    let f = t - vec2<f32>(i);
    let a = height_cell(i);
    let b = height_cell(i + vec2<i32>(1, 0));
    let d = height_cell(i + vec2<i32>(0, 1));
    let e = height_cell(i + vec2<i32>(1, 1));
    let na = normal_cell(i);
    let nb = normal_cell(i + vec2<i32>(1, 0));
    let nd = normal_cell(i + vec2<i32>(0, 1));
    let ne = normal_cell(i + vec2<i32>(1, 1));
    // Weights of a, b, d, e.
    var w = vec4<f32>(0.0);
    if (((i.x ^ i.y) & 1) == 0) {
        if (f.y >= f.x) {
            w = vec4<f32>(1.0 - f.y, 0.0, f.y - f.x, f.x);
        } else {
            w = vec4<f32>(1.0 - f.x, f.x - f.y, 0.0, f.y);
        }
    } else if (f.x + f.y <= 1.0) {
        w = vec4<f32>(1.0 - f.x - f.y, f.x, f.y, 0.0);
    } else {
        w = vec4<f32>(0.0, 1.0 - f.y, 1.0 - f.x, f.x + f.y - 1.0);
    }
    let h = dot(w, vec4<f32>(a, b, d, e));
    let n = na * w.x + nb * w.y + nd * w.z + ne * w.w;
    return vec4<f32>(n, h);
}

fn catmull(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    return p1 + 0.5 * t * (p2 - p0 + t * (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3 + t * (3.0 * (p1 - p2) + p3 - p0)));
}

// A smooth surface through the same heights.
fn smooth_height(t: vec2<f32>) -> f32 {
    let i = vec2<i32>(floor(t));
    let f = t - vec2<f32>(i);
    var row = vec4<f32>(0.0);
    for (var k = 0; k < 4; k++) {
        let z = i.y - 1 + k;
        row[k] = catmull(
            height_cell(vec2<i32>(i.x - 1, z)), height_cell(vec2<i32>(i.x, z)),
            height_cell(vec2<i32>(i.x + 1, z)), height_cell(vec2<i32>(i.x + 2, z)), f.x);
    }
    return catmull(row.x, row.y, row.z, row.w, f.y);
}

// How far the smooth ground lies off the game's at `t`, terrain units:
// kept within SMOOTH_MOST, and none at the patch's edge.
fn smooth_offset(t: vec2<f32>, base: f32) -> f32 {
    let most = SMOOTH_MOST / globals.grid.y;
    let p = t - enh.fine_ground.xy;
    let edge = min(min(p.x, p.y), min(enh.fine_ground.z - p.x, enh.fine_ground.z - p.y));
    return clamp(smooth_height(t) - base, -most, most) * smoothstep(0.0, SMOOTH_FADE, edge);
}

@vertex
fn vs_smooth(@builtin(vertex_index) index: u32) -> VertexOut {
    // The patch: enh.fine_ground.z cells a side from enh.fine_ground.xy, each cut
    // into SMOOTH_SPLIT a side.
    let split = SMOOTH_SPLIT;
    let side = u32(enh.fine_ground.z) * split + 1u;
    let t = enh.fine_ground.xy + vec2<f32>(f32(index % side), f32(index / side)) / f32(split);
    let game = game_ground(t);
    let off = smooth_offset(t, game.w);
    // Tilted by how the smoothing slopes.
    let step = 0.25;
    let gx = (smooth_offset(t + vec2<f32>(step, 0.0), game_ground(t + vec2<f32>(step, 0.0)).w) - off) / step;
    let gz = (smooth_offset(t + vec2<f32>(0.0, step), game_ground(t + vec2<f32>(0.0, step)).w) - off) / step;
    var n = normalize(game.xyz);
    n = normalize(n + vec3<f32>(-gx, 0.0, -gz) * n.y);
    let world = vec3<f32>(t.x, game.w + off, -t.y) * globals.grid.y;
    var out: VertexOut;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.terrain_xz = t;
    out.normal = normalize(n * vec3<f32>(1.0, 1.0, -1.0));
    out.world = world;
    return out;
}
