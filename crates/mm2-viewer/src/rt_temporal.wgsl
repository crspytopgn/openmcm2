// The ray-traced shadows over time (rt.rs): each frame's two rays a pixel
// (rt_shadow.wgsl) blended with what the frames before saw at the same
// place, so the shadows' fine detail (a rider's arms, the gaps between
// railway wagons) holds still instead of flickering. Where the place was
// not in view, or something else stood there, the new frame's own.

struct Params {
    inverse_view_proj: mat4x4<f32>,
    camera: vec4<f32>,
    to_sun: vec4<f32>,
    size: vec4<f32>,
    // The frame before's view and projection, and x 1 when the history
    // holds a frame before, yzw where its camera was.
    previous: mat4x4<f32>,
    history: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var raw: texture_2d<f32>;
@group(0) @binding(2) var before: texture_2d<f32>;
@group(0) @binding(3) var depth: texture_depth_2d;
@group(0) @binding(4) var blended: texture_storage_2d<rgba16float, write>;
// For a pixel on a moving model (rt_shadow.wgsl): where it was on screen a
// frame ago, in pixels from here, how far from that frame's camera, and
// whether it is known.
@group(0) @binding(5) var motion: texture_2d<f32>;

// The most frames a pixel's history gathers (its w counts them): each new
// frame is one more among them, so the rays' grain on leaves and soft
// edges settles the longer a place is seen, and a place just come into
// view shows its own frame at once.
const MOST_FRAMES: f32 = 24.0;
// And held still, where only its grain changes from frame to frame.
const MOST_FRAMES_STILL: f32 = 48.0;
// How far past what is round about now the history may be, held still:
// enough for the grain, not for a shadow that comes or goes (it is
// pulled most of the way at once).
const STILL_SLACK: f32 = 0.25;

@compute @workgroup_size(8, 8)
fn blend(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = vec2<i32>(params.size.xy);
    let p = vec2<i32>(id.xy);
    if (p.x >= size.x || p.y >= size.y) {
        return;
    }
    let now = textureLoad(raw, p, 0);
    if (now.y > 50000.0 || params.history.x < 0.5) {
        textureStore(blended, p, now);
        return;
    }
    // Where this pixel's surface was on the screen a frame ago.
    let d = textureLoad(depth, p, 0);
    let ndc = vec2<f32>((f32(p.x) + 0.5) / params.size.x * 2.0 - 1.0, 1.0 - (f32(p.y) + 0.5) / params.size.y * 2.0);
    let h = params.inverse_view_proj * vec4<f32>(ndc, d, 1.0);
    let world = h.xyz / h.w;
    let was = params.previous * vec4<f32>(world, 1.0);
    if (was.w <= 0.0) {
        textureStore(blended, p, now);
        return;
    }
    var uv = vec2<f32>(was.x / was.w * 0.5 + 0.5, 0.5 - was.y / was.w * 0.5);
    var then = length(params.history.yzw - world);
    // On a moving model, where it moved to, not where the camera's move
    // alone would put it.
    if (now.w > 0.5) {
        let m = textureLoad(motion, p, 0);
        if (m.w < 0.5) {
            textureStore(blended, p, now);
            return;
        }
        uv = ((vec2<f32>(p) + 0.5) + m.xy) / params.size.xy;
        then = m.z;
    }
    // The history between the four pixels round where it was, each by how
    // near it is, and only those that saw this surface: something else
    // stood there, if it was not as far from the camera of the frame
    // before as this place is. (Not the one nearest pixel: a fraction of a
    // pixel's shift, as temporal anti-aliasing gives each frame, would
    // pick another leaf's.)
    let at = uv * params.size.xy - 0.5;
    let base = vec2<i32>(floor(at));
    let f = at - floor(at);
    var sum = 0.0;
    var frames = 0.0;
    var weight = 0.0;
    for (var j = 0; j < 4; j++) {
        let o = vec2<i32>(j & 1, j >> 1u);
        let q = base + o;
        if (any(q < vec2<i32>(0)) || any(q >= size)) {
            continue;
        }
        let old = textureLoad(before, q, 0);
        if (abs(old.y - then) > 0.04 * now.y + 0.4) {
            continue;
        }
        let w = mix(1.0 - f.x, f.x, f32(o.x)) * mix(1.0 - f.y, f.y, f32(o.y));
        sum += old.x * w;
        frames += old.w * w;
        weight += w;
    }
    if (weight < 0.01) {
        textureStore(blended, p, now);
        return;
    }
    let old_x = sum / weight;
    // Kept only within what the pixels round about see now, so a shadow
    // that moves leaves no trail.
    var low = now.x;
    var high = now.x;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let t = textureLoad(raw, clamp(p + vec2<i32>(x, y), vec2<i32>(0), size - 1), 0).x;
            low = min(low, t);
            high = max(high, t);
        }
    }
    // Held still (less than half a pixel's move), the history may stray a
    // little past what is round about now, so the rays' grain averages
    // away rather than pushing the limits round each frame.
    let still = length(uv * params.size.xy - (vec2<f32>(p) + 0.5)) < 0.5;
    let loose = select(0.0, STILL_SLACK, still);
    let kept = clamp(old_x, low - loose, high + loose);
    // Clamped, the history was wrong there (a shadow moved): it counts
    // for fewer frames, so the new ones take over.
    var n = min(frames / weight + 1.0, select(MOST_FRAMES, MOST_FRAMES_STILL, still));
    if (kept != old_x) {
        n = min(n, 5.0);
    }
    textureStore(blended, p, vec4<f32>(mix(kept, now.x, 1.0 / (n + 1.0)), now.y, now.z, n));
}
