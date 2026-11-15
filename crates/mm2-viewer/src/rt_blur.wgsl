// The ray-traced shadows' blur (rt.rs): the rays' grain (rt_shadow.wgsl)
// evened out over the shadow's soft edge, as wide as the sun's disc makes
// it there, among pixels about as far away.

struct Params {
    inverse_view_proj: mat4x4<f32>,
    camera: vec4<f32>,
    to_sun: vec4<f32>,
    // The scene's size in pixels, the frame's number, and a pixel's width
    // a ft away.
    size: vec4<f32>,
};

// As rt_shadow.wgsl's.
const SUN_RADIUS: f32 = 0.0047;

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var raw: texture_2d<f32>;
@group(0) @binding(2) var soft: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8)
fn blur(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = vec2<i32>(params.size.xy);
    let p = vec2<i32>(id.xy);
    if (p.x >= size.x || p.y >= size.y) {
        return;
    }
    let here = textureLoad(raw, p, 0);
    if (here.y > 50000.0) {
        textureStore(soft, p, here);
        return;
    }
    // What shades round about, and so how wide the soft edge is, pixels.
    var shade = 0.0;
    var shaded = 0.0;
    var low = here.x;
    var high = here.x;
    for (var y = -2; y <= 2; y++) {
        for (var x = -2; x <= 2; x++) {
            let t = textureLoad(raw, clamp(p + vec2<i32>(x, y) * 3, vec2<i32>(0), size - 1), 0);
            if (t.z > 0.0) {
                shade += t.z;
                shaded += 1.0;
            }
            low = min(low, t.x);
            high = max(high, t.x);
        }
    }
    // All lit or all shaded round about (most of the screen): no edge to
    // soften, and no grain left to even out after the blend over time.
    if (high - low < 0.02) {
        textureStore(soft, p, vec4<f32>(here.x, here.y, here.z, 0.0));
        return;
    }
    let pixel = here.y * params.size.w;
    let edge = 2.0 * (shade / max(shaded, 1.0)) * SUN_RADIUS / max(pixel, 1e-4);
    // At least a pixel or so to even out the two rays; at most 8.
    let radius = clamp(edge * 0.5, 1.0, 8.0);
    var sum = 0.0;
    var weight = 0.0;
    let tolerance = here.y * 0.03 + 0.2;
    for (var y = -3; y <= 3; y++) {
        for (var x = -3; x <= 3; x++) {
            let q = clamp(p + vec2<i32>(round(vec2<f32>(f32(x), f32(y)) * radius / 3.0)), vec2<i32>(0), size - 1);
            let t = textureLoad(raw, q, 0);
            let w = exp(-abs(t.y - here.y) / tolerance) * exp(-f32(x * x + y * y) / 8.0);
            sum += t.x * w;
            weight += w;
        }
    }
    textureStore(soft, p, vec4<f32>(sum / max(weight, 1e-5), here.y, here.z, 0.0));
}
