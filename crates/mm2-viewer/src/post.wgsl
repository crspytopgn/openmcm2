// The enhanced drawing's last steps (post.rs): ambient occlusion, bloom,
// and tone mapping the HDR scene onto the screen. None of it the game's.

struct Post {
    // From the screen back to the camera's space.
    inv_proj: mat4x4<f32>,
    // Width, height, and one over each.
    size: vec4<f32>,
    // x ambient occlusion, y bloom, z tone mapping: 1 on.
    flags: vec4<f32>,
    // x occlusion radius (ft), y its strength, z bloom strength, w how
    // far (px per ft at 1 ft) the radius spreads on the screen.
    params: vec4<f32>,
    // The sun's place on the screen (0 to 1) and its shafts' strength (0
    // for none), and its colour.
    sun: vec4<f32>,
    sun_colour: vec4<f32>,
    // From this frame's screen to the last frame's, for motion blur.
    reproject: mat4x4<f32>,
    // Replays' camera look: x the distance in focus (ft), y the blur's
    // size out of focus (px), z the motion blur's strength, w 1 on.
    cinematic: vec4<f32>,
    // x 1 when the scene is drawn smaller than the screen and stretched
    // over it.
    scaled: vec4<f32>,
    // Temporal anti-aliasing (fs_taa): from the screen into the world and
    // the world into the last frame's screen, unshifted; the camera; x 1
    // with a history to blend with, y how much the result is sharpened,
    // zw how far this frame is shifted, pixels.
    taa_inverse: mat4x4<f32>,
    taa_before: mat4x4<f32>,
    taa_camera: vec4<f32>,
    taa: vec4<f32>,
    // The size it draws at (the screen's) and one over each.
    taa_out: vec4<f32>,
    // The rain, drawn here when temporal anti-aliasing is on (which would
    // blend its streaks away in the scene): the time, how hard it rains
    // (0 for none here), the screen's shape and the camera's x; its colour.
    rain: vec4<f32>,
    rain_colour: vec4<f32>,
};

@group(0) @binding(0) var<uniform> post: Post;
@group(0) @binding(1) var depth_tex: DEPTH_TEXTURE;
@group(0) @binding(2) var hdr_tex: texture_2d<f32>;
@group(0) @binding(3) var ao_tex: texture_2d<f32>;
@group(0) @binding(4) var src_tex: texture_2d<f32>;
@group(0) @binding(5) var smooth_sampler: sampler;
@group(0) @binding(6) var bloom_tex: texture_2d<f32>;
// The ray-traced light shafts (rt_volume.wgsl), when drawn (scaled.y).
@group(0) @binding(7) var shaft_tex: texture_2d<f32>;
// The moving models' motion on screen (objects.wgsl fs_motion).
@group(0) @binding(8) var motion_tex: texture_2d<f32>;

struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// One triangle over the whole screen.
@vertex
fn vs_full(@builtin(vertex_index) i: u32) -> Out {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var out: Out;
    out.position = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(p.x, 1.0 - p.y);
    return out;
}

fn depth_at(p: vec2<i32>) -> f32 {
    let size = vec2<i32>(post.size.xy);
    return textureLoad(depth_tex, clamp(p, vec2<i32>(0), size - 1), 0);
}

// Where pixel `p` shows, in the camera's space (it looks down -z).
fn view_at(p: vec2<i32>) -> vec3<f32> {
    let uv = (vec2<f32>(p) + 0.5) * post.size.zw;
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth_at(p), 1.0);
    let v = post.inv_proj * ndc;
    return v.xyz / v.w;
}

// Ambient occlusion (the Alchemy estimate, McGuire 2011): how much of the
// open sky round each pixel the nearby surfaces hide.
@fragment
fn fs_ao(in: Out) -> @location(0) vec4<f32> {
    // Worked out at half the scene's size, then blurred and stretched.
    let p = vec2<i32>(in.position.xy) * 2;
    if (depth_at(p) >= 1.0) {
        return vec4<f32>(1.0);
    }
    let c = view_at(p);
    // The surface's normal, from the nearer neighbours each way.
    let l = view_at(p - vec2<i32>(1, 0));
    let r = view_at(p + vec2<i32>(1, 0));
    let u = view_at(p - vec2<i32>(0, 1));
    let d = view_at(p + vec2<i32>(0, 1));
    let dx = select(c - l, r - c, abs(r.z - c.z) < abs(c.z - l.z));
    let dy = select(c - u, d - c, abs(d.z - c.z) < abs(c.z - u.z));
    var n = normalize(cross(dx, dy));
    if (dot(n, c) > 0.0) {
        n = -n;
    }
    let radius = post.params.x;
    let reach = clamp(radius * post.params.w / max(-c.z, 0.1), 2.0, 80.0);
    // A different turn each pixel; the blur after evens it out.
    let noise = fract(52.9829189 * fract(dot(in.position.xy, vec2<f32>(0.06711056, 0.00583715))));
    var sum = 0.0;
    let count = 12;
    for (var i = 0; i < count; i++) {
        let t = (f32(i) + 0.5) / f32(count);
        let angle = (f32(i) * 2.39996 + noise * 6.2832);
        let q = p + vec2<i32>(vec2<f32>(cos(angle), sin(angle)) * reach * sqrt(t));
        let v = view_at(q) - c;
        let vv = dot(v, v);
        let near = 1.0 - smoothstep(radius * radius * 0.5, radius * radius, vv);
        sum += max(dot(v, n) - 0.01 * -c.z, 0.0) / (vv + 0.05) * near;
    }
    let ao = max(1.0 - 2.0 * post.params.y * sum / f32(count), 0.0);
    return vec4<f32>(ao, ao, ao, 1.0);
}

// The occlusion blurred over 4×4 pixels, in four filtered taps.
@fragment
fn fs_ao_blur(in: Out) -> @location(0) vec4<f32> {
    let t = post.size.zw;
    var sum = 0.0;
    sum += textureSampleLevel(ao_tex, smooth_sampler, in.uv + vec2<f32>(-1.0, -1.0) * t, 0.0).r;
    sum += textureSampleLevel(ao_tex, smooth_sampler, in.uv + vec2<f32>(1.0, -1.0) * t, 0.0).r;
    sum += textureSampleLevel(ao_tex, smooth_sampler, in.uv + vec2<f32>(-1.0, 1.0) * t, 0.0).r;
    sum += textureSampleLevel(ao_tex, smooth_sampler, in.uv + vec2<f32>(1.0, 1.0) * t, 0.0).r;
    let ao = sum * 0.25;
    return vec4<f32>(ao, ao, ao, 1.0);
}

// `c` with any channel that is not a number, or is infinite, as 0, and
// the rest held to what a half float keeps. One such pixel in the scene
// (a bright glint past a half float's range, say) would otherwise spread
// through every step of the bloom and black out a block of the screen
// round it.
fn finite(c: vec3<f32>) -> vec3<f32> {
    let bad = (bitcast<vec3<u32>>(c) & vec3<u32>(0x7f800000u)) == vec3<u32>(0x7f800000u);
    return select(clamp(c, vec3<f32>(0.0), vec3<f32>(60000.0)), vec3<f32>(0.0), bad);
}

// Bloom's first step: half size, keeping only what is brighter than
// white, softly.
@fragment
fn fs_bloom_first(in: Out) -> @location(0) vec4<f32> {
    let c = finite(textureSampleLevel(hdr_tex, smooth_sampler, in.uv, 0.0).rgb);
    let bright = max(max(c.r, c.g), c.b);
    let knee = 0.35;
    let soft = clamp(bright - 1.0 + knee, 0.0, 2.0 * knee);
    let keep = max(soft * soft / (4.0 * knee + 1e-4), bright - 1.0) / max(bright, 1e-4);
    return vec4<f32>(c * keep, 1.0);
}

// Each next step down: half size again, from four filtered taps.
@fragment
fn fs_bloom_down(in: Out) -> @location(0) vec4<f32> {
    let t = 1.0 / vec2<f32>(textureDimensions(src_tex));
    var c = vec3<f32>(0.0);
    c += textureSampleLevel(src_tex, smooth_sampler, in.uv + vec2<f32>(-1.0, -1.0) * t, 0.0).rgb;
    c += textureSampleLevel(src_tex, smooth_sampler, in.uv + vec2<f32>(1.0, -1.0) * t, 0.0).rgb;
    c += textureSampleLevel(src_tex, smooth_sampler, in.uv + vec2<f32>(-1.0, 1.0) * t, 0.0).rgb;
    c += textureSampleLevel(src_tex, smooth_sampler, in.uv + vec2<f32>(1.0, 1.0) * t, 0.0).rgb;
    return vec4<f32>(c * 0.25, 1.0);
}

// Back up: the smaller step spread with a tent and added on.
@fragment
fn fs_bloom_up(in: Out) -> @location(0) vec4<f32> {
    let t = 1.0 / vec2<f32>(textureDimensions(src_tex));
    var c = textureSampleLevel(src_tex, smooth_sampler, in.uv, 0.0).rgb * 4.0;
    c += textureSampleLevel(src_tex, smooth_sampler, in.uv + vec2<f32>(-t.x, 0.0), 0.0).rgb * 2.0;
    c += textureSampleLevel(src_tex, smooth_sampler, in.uv + vec2<f32>(t.x, 0.0), 0.0).rgb * 2.0;
    c += textureSampleLevel(src_tex, smooth_sampler, in.uv + vec2<f32>(0.0, -t.y), 0.0).rgb * 2.0;
    c += textureSampleLevel(src_tex, smooth_sampler, in.uv + vec2<f32>(0.0, t.y), 0.0).rgb * 2.0;
    c += textureSampleLevel(src_tex, smooth_sampler, in.uv + vec2<f32>(-t.x, -t.y), 0.0).rgb;
    c += textureSampleLevel(src_tex, smooth_sampler, in.uv + vec2<f32>(t.x, -t.y), 0.0).rgb;
    c += textureSampleLevel(src_tex, smooth_sampler, in.uv + vec2<f32>(-t.x, t.y), 0.0).rgb;
    c += textureSampleLevel(src_tex, smooth_sampler, in.uv + vec2<f32>(t.x, t.y), 0.0).rgb;
    return vec4<f32>(c / 16.0, 1.0);
}

// Khronos PBR Neutral tone mapping: colours up to about 0.8 stay as they
// are, so the game's own colours keep their look, and only what is
// brighter rolls off toward white.
fn neutral(colour: vec3<f32>) -> vec3<f32> {
    let start = 0.8 - 0.04;
    let desaturation = 0.15;
    // (Its toe, which darkens the darkest colours, is left out.)
    var c = colour;
    let peak = max(c.r, max(c.g, c.b));
    if (peak < start) {
        return c;
    }
    let d = 1.0 - start;
    let new_peak = 1.0 - d * d / (peak + d - start);
    c *= new_peak / peak;
    let g = 1.0 - 1.0 / (desaturation * (peak - new_peak) + 1.0);
    return mix(c, vec3<f32>(new_peak), g);
}

// The bolder look: richer colour, more contrast round the middle grey
// (worked in the screen's gamma, as the eye sees it), and the corners a
// little darker.
fn grade(linear: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    var c = pow(max(linear, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2));
    let grey = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    c = mix(vec3<f32>(grey), c, 1.1);
    // An S-curve: contrast 1.15 about 0.45.
    c = clamp((c - 0.45) * 1.08 + 0.45, vec3<f32>(0.0), vec3<f32>(1.0));
    c = mix(c, c * c * (3.0 - 2.0 * c), 0.15);
    // Warm the lights a touch and cool the shadows.
    c += vec3<f32>(0.015, 0.005, -0.015) * (grey - 0.4);
    let d = uv - 0.5;
    c *= 1.0 - 0.28 * smoothstep(0.25, 0.75, dot(d, d) * 2.0);
    return pow(max(c, vec3<f32>(0.0)), vec3<f32>(2.2));
}

// Shafts of sunlight (light scattered in the air, after Mitchell 2007):
// marching toward the sun on the screen, the open sky on the way shines
// along the line, and whatever stands in front of it, hills, trees,
// riders, leaves its shadow in the light.
fn shafts(uv: vec2<f32>, position: vec2<f32>) -> vec3<f32> {
    let steps = 40;
    let reach = 0.85;
    let delta = (post.sun.xy - uv) * reach / f32(steps);
    // A different start each pixel, so the steps don't band.
    let jitter = fract(52.9829189 * fract(dot(position, vec2<f32>(0.06711056, 0.00583715))));
    var q = uv + delta * jitter;
    var decay = 1.0;
    var sum = 0.0;
    for (var i = 0; i < steps; i++) {
        q += delta;
        if (any(q < vec2<f32>(0.0)) || any(q > vec2<f32>(1.0))) {
            break;
        }
        let d = depth_at(vec2<i32>(q * post.size.xy));
        // Only the sky lights the air; brighter near the sun.
        let open = select(0.0, 1.0, d >= 1.0);
        let near = 1.0 - smoothstep(0.0, 0.3, length((q - post.sun.xy) * vec2<f32>(post.size.x * post.size.w, 1.0)));
        sum += open * near * near * decay;
        decay *= 0.965;
    }
    return post.sun_colour.rgb * sum / f32(steps) * post.sun.z;
}

// The replay camera's look: what is nearer or further than the rider in
// focus is soft, as through a lens, and the world smears along the way
// the camera swings (from where each pixel was on the last frame).
fn cinematic(uv: vec2<f32>, p: vec2<i32>) -> vec3<f32> {
    let d = depth_at(p);
    let z = -view_at(p).z;
    let focus = post.cinematic.x;
    // The circle of confusion, px; the sky as far as anything.
    let far = select(z, 10000.0, d >= 1.0);
    let coc = clamp(abs(far - focus) / max(far, 1.0) * post.cinematic.y * clamp(sqrt(focus / 12.0), 0.5, 1.5), 0.0, post.cinematic.y * 1.5);
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, min(d, 0.99999), 1.0);
    let before = post.reproject * ndc;
    let was = vec2<f32>(before.x / before.w * 0.5 + 0.5, 0.5 - before.y / before.w * 0.5);
    var motion = (uv - was) * post.cinematic.z;
    let longest = 0.04;
    if (length(motion) > longest) {
        motion = normalize(motion) * longest;
    }
    let jitter = fract(52.9829189 * fract(dot(vec2<f32>(p), vec2<f32>(0.06711056, 0.00583715))));
    var sum = vec3<f32>(0.0);
    var weight = 0.0;
    let taps = 16;
    for (var i = 0; i < taps; i++) {
        let t = (f32(i) + jitter) / f32(taps);
        let angle = f32(i) * 2.39996;
        let disc = vec2<f32>(cos(angle), sin(angle)) * sqrt(t) * coc * post.size.zw;
        let q = uv + disc + motion * (t - 0.5);
        // Nearer, sharper things do not bleed onto what is behind them.
        let qz = -view_at(vec2<i32>(q * post.size.xy)).z;
        let w = select(1.0, 0.2, qz < z * 0.8 && coc > 1.5);
        sum += textureSampleLevel(hdr_tex, smooth_sampler, q, 0.0).rgb * w;
        weight += w;
    }
    return sum / max(weight, 1e-4);
}

fn rain_hash(x: f32) -> f32 {
    return fract(sin(x * 91.345) * 47453.5453);
}

// How much of the pixel at `uv` the rain's streaks cover: as rain.wgsl
// draws them into the scene, here over the finished picture.
fn rain_cover(uv: vec2<f32>) -> f32 {
    let time = post.rain.x;
    var cover = 0.0;
    for (var k = 0; k < 3; k++) {
        let layer = f32(k);
        let columns = mix(70.0, 220.0, layer / 2.0);
        let speed = mix(2.6, 1.3, layer / 2.0);
        let length_ = mix(0.09, 0.035, layer / 2.0);
        var q = vec2<f32>(uv.x * post.rain.z, uv.y);
        q.x += q.y * 0.12 + post.rain.w * 0.002 * (3.0 - layer);
        let col = floor(q.x * columns);
        let x = fract(q.x * columns) - 0.5;
        let r = rain_hash(col + layer * 17.0);
        let y = fract(q.y - time * speed * (0.8 + r * 0.4) + r * 7.0);
        let line = smoothstep(0.08, 0.0, abs(x) * (1.0 + layer)) * smoothstep(0.0, length_ * 0.3, y) * smoothstep(length_, length_ * 0.5, y);
        let some = step(rain_hash(col * 1.7 + floor(q.y - time * speed + r * 7.0) + layer), 0.55);
        cover += line * some * mix(0.35, 0.18, layer / 2.0);
    }
    return clamp(cover * post.rain.y, 0.0, 0.5);
}

// The ray-traced light shafts at `uv` (scene pixel `p`), stretched from
// their half size: between the four round about, those that looked as far
// as this pixel does counting most, so leaves take the leaves' and the sky
// between them the sky's (rt_volume.wgsl `sample_at`).
fn traced_shafts(uv: vec2<f32>, p: vec2<i32>) -> f32 {
    let size = vec2<i32>(textureDimensions(shaft_tex));
    let dist = length(view_at(p));
    let at = uv * vec2<f32>(size) - 0.5;
    let base = vec2<i32>(floor(at));
    let f = at - floor(at);
    var sum = 0.0;
    var weight = 0.0;
    for (var j = 0; j < 4; j++) {
        let o = vec2<i32>(j & 1, j >> 1u);
        let t = textureLoad(shaft_tex, clamp(base + o, vec2<i32>(0), size - 1), 0);
        let apart = abs(t.y - dist) / max(dist, 1.0);
        let w = (mix(1.0 - f.x, f.x, f32(o.x)) * mix(1.0 - f.y, f.y, f32(o.y)) + 0.01) / (0.02 + apart);
        sum += t.x * w;
        weight += w;
    }
    return sum / weight;
}

@fragment
fn fs_composite(in: Out) -> @location(0) vec4<f32> {
    var p = vec2<i32>(in.position.xy);
    var c: vec3<f32>;
    if (post.scaled.x > 0.5) {
        p = vec2<i32>(in.uv * post.size.xy);
        c = textureSampleLevel(hdr_tex, smooth_sampler, in.uv, 0.0).rgb;
    } else {
        c = textureLoad(hdr_tex, p, 0).rgb;
    }
    // After temporal anti-aliasing (which softens a little), sharpened
    // back: away from the pixels round about, within them, so no halo.
    if (post.taa.y > 0.0) {
        let t = 1.0 / vec2<f32>(textureDimensions(hdr_tex));
        let n = textureSampleLevel(hdr_tex, smooth_sampler, in.uv - vec2<f32>(0.0, t.y), 0.0).rgb;
        let s = textureSampleLevel(hdr_tex, smooth_sampler, in.uv + vec2<f32>(0.0, t.y), 0.0).rgb;
        let w = textureSampleLevel(hdr_tex, smooth_sampler, in.uv - vec2<f32>(t.x, 0.0), 0.0).rgb;
        let e = textureSampleLevel(hdr_tex, smooth_sampler, in.uv + vec2<f32>(t.x, 0.0), 0.0).rgb;
        let lo = min(min(min(n, s), min(w, e)), c);
        let hi = max(max(max(n, s), max(w, e)), c);
        c = clamp(c + (c - (n + s + w + e) * 0.25) * post.taa.y, lo, hi);
    }
    if (post.cinematic.w > 0.5) {
        c = cinematic(in.uv, p);
    }
    c = finite(c);
    if (post.flags.x > 0.5) {
        c *= textureSampleLevel(ao_tex, smooth_sampler, in.uv, 0.0).r;
    }
    if (post.rain.y > 0.0) {
        let a = rain_cover(in.uv);
        c = c * (1.0 - a) + post.rain_colour.rgb * a;
    }
    if (post.scaled.y > 0.0) {
        // Ray traced: the sunlit air, as strong as scaled.y says (with
        // temporal anti-aliasing, already in its blend).
        if (post.taa.y <= 0.0) {
            c += post.sun_colour.rgb * traced_shafts(in.uv, p) * post.scaled.y;
        }
    } else if (post.sun.z > 0.0) {
        c += shafts(in.uv, in.position.xy);
    }
    if (post.flags.y > 0.5) {
        c += textureSampleLevel(bloom_tex, smooth_sampler, in.uv, 0.0).rgb * post.params.z;
    }
    if (post.flags.z > 0.5) {
        c = neutral(c);
    }
    if (post.flags.w > 0.5) {
        c = grade(c, in.uv);
    }
    return vec4<f32>(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// ---- Temporal anti-aliasing. ----
//
// Each frame is drawn a different fraction of a pixel aside (renderer.rs),
// and blended here into the history of the ones before: each pixel's
// surface found where it was a frame ago (by the camera's move, or on a
// moving model by its own), the history read there. Kept within what the
// pixels round about show now, so what moves or comes into view leaves no
// trail. Fine texture and thin edges, a different few pixels each frame
// alone, so hold still.

// How many frames of history a pixel keeps: still, and moving fast (the
// pixels a frame moves at that), where each read of it softens it and the
// motion hides what it would smooth.
// (As frames gathered, the output's alpha counting them: each new frame
// is one more among them, so a place just come into view shows at once
// and one that holds still goes on settling.)
const TAA_FRAMES: f32 = 32.0;
const TAA_FRAMES_MOVING: f32 = 3.0;
const TAA_FAST: f32 = 12.0;
// How far from what is round about now the history may stray standing
// still, in its spreads: cut-out leaves against the sky change much from
// one shifted frame to the next, and a tight limit there would clip the
// history differently each frame and flutter.
const TAA_CLIP_STILL: f32 = 2.5;

// Bright pixels count for less in the blend, so one does not flash.
fn tame(c: vec3<f32>) -> vec3<f32> {
    return c / (1.0 + max(c.r, max(c.g, c.b)));
}
fn untame(c: vec3<f32>) -> vec3<f32> {
    return c / max(1.0 - max(c.r, max(c.g, c.b)), 1e-4);
}
fn to_ycocg(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(0.25 * c.r + 0.5 * c.g + 0.25 * c.b, 0.5 * c.r - 0.5 * c.b, -0.25 * c.r + 0.5 * c.g - 0.25 * c.b);
}
fn from_ycocg(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(c.x + c.y - c.z, c.x + c.z, c.x - c.y - c.z);
}

// The history at `uv`, Catmull-Rom from five bilinear taps: sharper than
// bilinear, so it does not soften as it is read again each frame.
fn history_at(uv: vec2<f32>) -> vec3<f32> {
    let size = vec2<f32>(textureDimensions(src_tex));
    let p = uv * size;
    let c = floor(p - 0.5) + 0.5;
    let f = p - c;
    let w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
    let w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
    let w2 = f * (0.5 + f * (2.0 - 1.5 * f));
    let w3 = f * f * (-0.5 + 0.5 * f);
    let w12 = w1 + w2;
    let t0 = (c - 1.0) / size;
    let t3 = (c + 2.0) / size;
    let t12 = (c + w2 / w12) / size;
    let r = textureSampleLevel(src_tex, smooth_sampler, vec2<f32>(t12.x, t0.y), 0.0).rgb * (w12.x * w0.y)
        + textureSampleLevel(src_tex, smooth_sampler, vec2<f32>(t0.x, t12.y), 0.0).rgb * (w0.x * w12.y)
        + textureSampleLevel(src_tex, smooth_sampler, vec2<f32>(t12.x, t12.y), 0.0).rgb * (w12.x * w12.y)
        + textureSampleLevel(src_tex, smooth_sampler, vec2<f32>(t3.x, t12.y), 0.0).rgb * (w3.x * w12.y)
        + textureSampleLevel(src_tex, smooth_sampler, vec2<f32>(t12.x, t3.y), 0.0).rgb * (w12.x * w3.y);
    let w = w12.x * w0.y + w0.x * w12.y + w12.x * w12.y + w3.x * w12.y + w12.x * w3.y;
    return max(r / w, vec3<f32>(0.0));
}

@fragment
fn fs_taa(in: Out) -> @location(0) vec4<f32> {
    // Drawn at the screen's size (taa_out); the scene's own pixels may be
    // fewer (size). Where this pixel is on the scene, in its pixels.
    let uv = in.position.xy * post.taa_out.zw;
    let size = vec2<i32>(post.size.xy);
    let at_scene = uv * post.size.xy;
    let centre = clamp(vec2<i32>(floor(at_scene)), vec2<i32>(0), size - 1);
    // The ray-traced light shafts join the scene here, before the blend,
    // so they settle with it (the last step adds them without it): soft,
    // the same for the pixels round about.
    var shaft = vec3<f32>(0.0);
    if (post.scaled.y > 0.0) {
        shaft = post.sun_colour.rgb * traced_shafts(uv, centre) * post.scaled.y;
    }
    let own = finite(textureLoad(hdr_tex, centre, 0).rgb) + shaft;
    // The scene's pixels round about, and the nearest of them: a thin edge
    // in front is found where it was with what is in front. Each pixel's
    // sample this frame stands where the frame's shift put it; weighed by
    // how near that is to this pixel's middle (a Gaussian near the
    // Blackman-Harris window), they rebuild this pixel's sample, and where
    // the nearest is far (drawn smaller than the screen, only some frames
    // have a sample near) the history counts for more.
    var m1 = vec3<f32>(0.0);
    var m2 = vec3<f32>(0.0);
    var nearest = 2.0;
    var at = centre;
    var filtered = vec3<f32>(0.0);
    var filtered_weight = 0.0;
    var closest = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let q = clamp(centre + vec2<i32>(x, y), vec2<i32>(0), size - 1);
            let s = to_ycocg(tame(finite(textureLoad(hdr_tex, q, 0).rgb) + shaft));
            m1 += s;
            m2 += s * s;
            let off = vec2<f32>(centre + vec2<i32>(x, y)) + 0.5 - post.taa.zw - at_scene;
            let w = exp(-2.29 * dot(off, off));
            filtered += s * w;
            filtered_weight += w;
            closest = max(closest, w);
            let d = textureLoad(depth_tex, q, 0);
            if (d < nearest) {
                nearest = d;
                at = q;
            }
        }
    }
    let settled = untame(from_ycocg(filtered / max(filtered_weight, 1e-4)));
    if (post.taa.x < 0.5) {
        return vec4<f32>(settled, 1.0);
    }
    m1 /= 9.0;
    let sigma = sqrt(max(m2 / 9.0 - m1 * m1, vec3<f32>(0.0)));

    // Where it was a frame ago: by the camera's move, or on a moving model
    // (its motion drawn there, at the same distance as what shows) its own.
    let h = post.taa_inverse * vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, nearest, 1.0);
    let world = h.xyz / h.w;
    var was = uv;
    let m = textureLoad(motion_tex, at, 0);
    let dist = length(world - post.taa_camera.xyz);
    if (m.w > 0.5 && abs(m.z - dist) < 0.03 * dist + 0.5) {
        was = uv + m.xy * post.size.zw;
    } else {
        let c = post.taa_before * vec4<f32>(world, 1.0);
        if (c.w <= 0.0) {
            return vec4<f32>(settled, 1.0);
        }
        was = vec2<f32>(c.x / c.w * 0.5 + 0.5, 0.5 - c.y / c.w * 0.5);
    }
    if (any(was < vec2<f32>(0.0)) || any(was > vec2<f32>(1.0))) {
        return vec4<f32>(settled, 1.0);
    }
    var hist = to_ycocg(tame(history_at(was)));
    // How far it moved, in the scene's pixels.
    let moved = length((was - uv) * post.size.xy);
    let fast = clamp(moved / TAA_FAST, 0.0, 1.0);
    // The rebuilt sample where the picture holds still (its edges settle
    // smoothly); moving, where it would only soften, the nearest pixel's
    // own (drawn at the screen's size; smaller, the rebuilt one always).
    let upscaled = post.taa_out.x > post.size.x + 0.5;
    let now = select(mix(settled, own, clamp(moved / 2.0, 0.0, 1.0)), settled, upscaled);
    // Clipped toward the middle of what is round about now, not clamped
    // corner by corner, so it takes on no tint; wider standing still,
    // where nothing can trail and a tight clip only makes edges flutter.
    let d = hist - m1;
    let range = max(mix(TAA_CLIP_STILL, 1.0, fast) * sigma, vec3<f32>(1e-4));
    let k = max(max(abs(d.x) / range.x, abs(d.y) / range.y), abs(d.z) / range.z);
    if (k > 1.0) {
        hist = m1 + d / k;
    }
    // Less history the farther it moved: far fewer as soon as it moves at
    // all, as each frame the history is read between pixels, and many such
    // reads would wash fine leaves out.
    let frames = textureSampleLevel(src_tex, smooth_sampler, was, 0.0).a;
    let most = 1.0 / mix(1.0 / TAA_FRAMES, 1.0 / TAA_FRAMES_MOVING, sqrt(fast));
    let n = min(frames + 1.0, most);
    // This frame's share, less where its nearest sample is far.
    let share = select(1.0, mix(0.25, 1.0, closest), upscaled) / (n + 1.0);
    let c = mix(hist, to_ycocg(tame(now)), share);
    return vec4<f32>(max(untame(from_ycocg(c)), vec3<f32>(0.0)), n);
}
