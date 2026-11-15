// Ray-traced light shafts (rt.rs): for each pixel, steps through the air
// from the camera to what it sees, each asking the scene whether the sun
// reaches that bit of air (past hills, trees, leaves, riders), so the lit
// air glows and the shaded air does not: beams through the trees and past
// whatever stands in the sun, strongest looking toward it. In place of the
// screen-space shafts (post.wgsl `shafts`), which needed the sun on
// screen. At half the scene's size, blended over time (`settle`); the
// composite stretches it over the screen.

enable wgpu_ray_query;

struct Params {
    inverse_view_proj: mat4x4<f32>,
    camera: vec4<f32>,
    to_sun: vec4<f32>,
    // The scene's size in pixels, the frame's number, and a pixel's width
    // a ft away.
    size: vec4<f32>,
    previous: mat4x4<f32>,
    history: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var depth: texture_depth_2d;
@group(0) @binding(2) var scene: acceleration_structure;
@group(0) @binding(3) var<storage, read> cutouts: array<vec4<f32>>;
@group(0) @binding(4) var masks: texture_2d_array<f32>;
@group(0) @binding(5) var mask_sampler: sampler;
@group(0) @binding(7) var out: texture_storage_2d<rgba16float, write>;

// How far the steps go, ft, and how many there are.
const REACH: f32 = 600.0;
const STEPS: u32 = 2u;
// How far along a ray toward the sun leaves stop it, ft.
const CUTOUT_REACH: f32 = 300.0;

fn noise(p: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(p, vec2<f32>(0.06711056, 0.00583715))));
}

// Whether the sun reaches `p`: past what is solid, and the leaves and
// fences where their masks are solid (blurred, as the beams are soft).
fn sunlit(p: vec3<f32>) -> f32 {
    var rq: ray_query;
    rayQueryInitialize(&rq, scene, RayDesc(RAY_FLAG_TERMINATE_ON_FIRST_HIT, 0xffu, 0.0, 6000.0, p, params.to_sun.xyz));
    var through = 1.0;
    while (rayQueryProceed(&rq)) {
        let c = rayQueryGetCandidateIntersection(&rq);
        if (c.kind != RAY_QUERY_INTERSECTION_TRIANGLE || c.t > CUTOUT_REACH) {
            continue;
        }
        let r = (c.instance_custom_data + c.primitive_index) * 2u;
        let a = cutouts[r];
        let b = cutouts[r + 1u];
        let w = c.barycentrics;
        let uv = a.xy * (1.0 - w.x - w.y) + a.zw * w.x + b.xy * w.y;
        through *= 1.0 - textureSampleLevel(masks, mask_sampler, uv, i32(b.z), 3.0).r;
        if (through < 0.1) {
            rayQueryConfirmIntersection(&rq);
        }
    }
    if (rayQueryGetCommittedIntersection(&rq).kind != RAY_QUERY_INTERSECTION_NONE) {
        return 0.0;
    }
    return through;
}

// How strongly air scatters light toward the eye at `cos` from the sun
// (Henyey-Greenstein): mostly forward, so the beams show looking into the
// light, and a little always.
fn phase(cos: f32) -> f32 {
    let g = 0.6;
    let forward = (1.0 - g * g) / pow(1.0 + g * g - 2.0 * g * cos, 1.5);
    return 0.25 + forward;
}

// Which of a 2x2 block's full-size pixels a half-size pixel stands for:
// the nearest of them, or on every other one (a checkerboard) the
// farthest. The same each frame, so it settles; and where near and far
// meet (leaves against the sky) both are there for the full-size pixels
// to pick theirs from (post.wgsl `traced_shafts`).
fn sample_at(half: vec2<i32>) -> vec2<i32> {
    let most = vec2<i32>(params.size.xy) - 1;
    let far = ((half.x + half.y) & 1) == 1;
    var best = min(half * 2, most);
    var depth_best = textureLoad(depth, best, 0);
    for (var j = 1; j < 4; j++) {
        let q = min(half * 2 + vec2<i32>(j & 1, j >> 1u), most);
        let d = textureLoad(depth, q, 0);
        if ((far && d > depth_best) || (!far && d < depth_best)) {
            best = q;
            depth_best = d;
        }
    }
    return best;
}

// Where the full-size pixel `p` looks: the point it sees (at the far
// plane for the sky) and whether it is sky.
fn seen(p: vec2<i32>) -> vec4<f32> {
    let d = textureLoad(depth, p, 0);
    let ndc = vec2<f32>((f32(p.x) + 0.5) / params.size.x * 2.0 - 1.0, 1.0 - (f32(p.y) + 0.5) / params.size.y * 2.0);
    let h = params.inverse_view_proj * vec4<f32>(ndc, d, 1.0);
    return vec4<f32>(h.xyz / h.w, select(0.0, 1.0, d >= 1.0));
}

// Worked out at half the scene's size each way, a quarter of the rays:
// the haze is soft. Out: x how much sunlight the air sends toward the
// eye, y how far the pixel's surface is.
@compute @workgroup_size(8, 8)
fn scatter(@builtin(global_invocation_id) id: vec3<u32>) {
    let half = vec2<i32>(id.xy);
    if (any(half >= vec2<i32>(textureDimensions(out)))) {
        return;
    }
    let p = sample_at(half);
    let at = seen(p);
    let toward = at.xyz - params.camera.xyz;
    let dir = normalize(toward);
    let dist = select(length(toward), 1e9, at.w > 0.5);
    let length_ = min(dist, REACH);
    // Where along its stretch each step falls: a fixed start for the pixel,
    // moved on by the golden ratio each frame, so the frames' steps cover
    // the stretch evenly and their blend settles fast (random ones each
    // frame leave a grain that crawls).
    let jitter = fract(noise(vec2<f32>(id.xy)) + params.size.z * 0.618034);
    var lit = 0.0;
    for (var i = 0u; i < STEPS; i++) {
        // Evenly along the view, each standing for as much air, so the
        // shade round the eye takes only its own share.
        let k = (f32(i) + jitter) / f32(STEPS);
        lit += sunlit(params.camera.xyz + dir * length_ * k);
    }
    // The sunlit share of the air looked through, by how far, and as it
    // scatters toward the eye; sky as far off as the steps reach.
    let air = lit / f32(STEPS) * length_ / REACH * phase(dot(dir, params.to_sun.xyz));
    textureStore(out, half, vec4<f32>(air, length(toward), 0.0, 0.0));
}

// ---- Over time: each frame's few rays blended with what the frames
// before saw at the same place, at the same half size. ----

@group(0) @binding(8) var raw: texture_2d<f32>;
@group(0) @binding(9) var before: texture_2d<f32>;
@group(0) @binding(10) var history: texture_storage_2d<rgba16float, write>;
@group(0) @binding(11) var shown: texture_storage_2d<rgba16float, write>;

// The most frames a pixel's history gathers (its z counts them), each new
// frame one more among them.
const MOST_FRAMES: f32 = 20.0;
// And held still, where only its grain changes from frame to frame.
const MOST_FRAMES_STILL: f32 = 48.0;
// How far past what is round about now the history may be, held still:
// enough for the grain, not for a shadow that comes or goes (it is
// pulled most of the way at once).
const STILL_SLACK: f32 = 0.25;

@compute @workgroup_size(8, 8)
fn settle(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = vec2<i32>(textureDimensions(raw));
    let half = vec2<i32>(id.xy);
    if (any(half >= size)) {
        return;
    }
    let now = textureLoad(raw, half, 0);
    var out = now;
    if (params.history.x > 0.5) {
        // Where this place was on the half-size screen a frame ago.
        let world = seen(sample_at(half)).xyz;
        let was = params.previous * vec4<f32>(world, 1.0);
        if (was.w > 0.0) {
            let uv = vec2<f32>(was.x / was.w * 0.5 + 0.5, 0.5 - was.y / was.w * 0.5);
            // The history between the four pixels round where it was,
            // those that looked as far as this one does.
            let then = length(params.history.yzw - world);
            let at = uv * vec2<f32>(size) - 0.5;
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
                if (abs(old.y - then) > 0.05 * then + 1.0) {
                    continue;
                }
                let w = mix(1.0 - f.x, f.x, f32(o.x)) * mix(1.0 - f.y, f.y, f32(o.y)) + 1e-3;
                sum += old.x * w;
                frames += old.z * w;
                weight += w;
            }
            if (weight > 0.0) {
                // Within what is round about now, so a moving edge leaves
                // no trail; clamped, the history counts for fewer frames.
                var low = now.x;
                var high = now.x;
                for (var y = -1; y <= 1; y++) {
                    for (var x = -1; x <= 1; x++) {
                        let t = textureLoad(raw, clamp(half + vec2<i32>(x, y), vec2<i32>(0), size - 1), 0).x;
                        low = min(low, t);
                        high = max(high, t);
                    }
                }
                let old_x = sum / weight;
                // Held still (less than half a pixel's move), the history
                // may stray a little past what is round about now, so the
                // few steps' grain averages away rather than pushing the
                // limits round each frame.
                let still = length(uv * vec2<f32>(size) - (vec2<f32>(half) + 0.5)) < 0.5;
                let loose = select(0.0, STILL_SLACK * max(high, 1e-3), still);
                let kept = clamp(old_x, low - loose, high + loose);
                var n = min(frames / weight + 1.0, select(MOST_FRAMES, MOST_FRAMES_STILL, still));
                if (kept != old_x) {
                    n = min(n, 8.0);
                }
                out.x = mix(kept, now.x, 1.0 / (n + 1.0));
                out.z = n;
            }
        }
    }
    textureStore(history, half, out);
    textureStore(shown, half, out);
}
