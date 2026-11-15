// The light poles' light over the ground, baked once (flood.rs): for each
// texel of a map over the poles' reach, the light of every pole that
// reaches it, by its beam (lamp_light.wgsl) and how much of its bank of
// lamps nothing hides (`flood_visible`: traced, flood_rt.wgsl; or past the
// hills alone, flood_hills.wgsl). Out: rgb the light on the ground, a how
// much light reaches there whichever way a surface faces (for what stands
// on it).

struct Flood {
    // The map's corner (x, z, drawing coordinates), ft a texel, and w 1
    // once baked.
    corner: vec4<f32>,
    // Texels across and down, the first row of this run, the rows in it.
    size: vec4<f32>,
    // The terrain: samples a side, ft a sample; how many poles, and how
    // many rays to each pole's bank.
    grid: vec4<f32>,
};

// The terrain's heights, ft, a row of samples at a time (rows along -z).
@group(0) @binding(30) var<storage, read> flood_heights: array<f32>;
@group(0) @binding(31) var<uniform> flood: Flood;
// Each pole's lamps: (where, 1), (the way they shine, 0).
@group(0) @binding(32) var<storage, read> flood_poles: array<vec4<f32>>;
@group(0) @binding(33) var flood_out: texture_storage_2d<rgba16float, write>;

// How far across a pole's bank of lamps its rays spread, ft.
const FLOOD_BANK: f32 = 3.0;
// How far short of a lamp a ray stops, ft: past the pole's own head.
const FLOOD_SHORT: f32 = 3.0;

fn flood_height_at(col: i32, row: i32) -> f32 {
    let n = i32(flood.grid.x);
    let c = clamp(col, 0, n - 1);
    let r = clamp(row, 0, n - 1);
    return flood_heights[r * n + c];
}

// The terrain's height at (x, z), ft, between its samples.
fn ground_height(x: f32, z: f32) -> f32 {
    let g = vec2<f32>(x, -z) / flood.grid.y;
    let base = vec2<i32>(floor(g));
    let f = g - floor(g);
    let a = mix(flood_height_at(base.x, base.y), flood_height_at(base.x + 1, base.y), f.x);
    let b = mix(flood_height_at(base.x, base.y + 1), flood_height_at(base.x + 1, base.y + 1), f.x);
    return mix(a, b, f.y);
}

// The terrain's way up at (x, z).
fn ground_normal(x: f32, z: f32) -> vec3<f32> {
    let e = flood.grid.y;
    let dx = ground_height(x + e, z) - ground_height(x - e, z);
    let dz = ground_height(x, z + e) - ground_height(x, z - e);
    return normalize(vec3<f32>(-dx, 2.0 * e, -dz));
}

// A turn for each texel, so the rays to a bank are spread differently
// from texel to texel and no pattern shows.
fn flood_turn(p: vec2<u32>) -> f32 {
    return fract(52.9829189 * fract(dot(vec2<f32>(p), vec2<f32>(0.06711056, 0.00583715)))) * 6.2831853;
}

@compute @workgroup_size(8, 8)
fn bake(@builtin(global_invocation_id) id: vec3<u32>) {
    let p = vec2<u32>(id.x, id.y + u32(flood.size.z));
    if (p.x >= u32(flood.size.x) || p.y >= u32(flood.size.y) || id.y >= u32(flood.size.w)) {
        return;
    }
    let x = flood.corner.x + (f32(p.x) + 0.5) * flood.corner.z;
    let z = flood.corner.y + (f32(p.y) + 0.5) * flood.corner.z;
    let world = vec3<f32>(x, ground_height(x, z), z);
    let n = ground_normal(x, z);
    var light = vec3<f32>(0.0);
    var reaching = 0.0;
    let poles = u32(flood.grid.z);
    for (var j = 0u; j < poles; j++) {
        let at = flood_poles[j * 2u].xyz;
        let c = lamp_beam(at, flood_poles[j * 2u + 1u].xyz, true, world);
        if (c.g < LAMP_FAINT * 0.25) {
            continue;
        }
        let facing = dot(n, normalize(at - world));
        let seen = flood_visible(world, n, at, flood_turn(p) + f32(j) * 1.7);
        light += c * max(facing, 0.0) * seen;
        reaching += c.g * seen;
    }
    textureStore(flood_out, vec2<i32>(p), vec4<f32>(light, reaching));
}
