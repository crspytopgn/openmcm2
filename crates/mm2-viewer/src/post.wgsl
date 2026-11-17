// The enhanced drawing's last steps (post.rs): ambient occlusion, bloom,
// and tone mapping the HDR scene onto the screen. None of it the game's.

struct Post {
    // From the screen back to the camera's space.
    inv_proj: mat4x4<f32>,
    // Width, height, and one over each.
    size: vec4<f32>,
    // x ambient occlusion, y bloom, z tone mapping: 1 on, 2 the
    // film-like curve (filmic).
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
    // From the camera's space to the screen.
    proj: mat4x4<f32>,
    // The way to the sun in the camera's space, and w how dark the
    // contact shadows are (0 for none).
    view_sun: vec4<f32>,
    // The way up in the camera's space, and w how wet the ground is for
    // its reflections (0 for none).
    view_up: vec4<f32>,
    // x 1 for the horizon-based occlusion (GTAO) in place of the plain one.
    extra: vec4<f32>,
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
// The reflections in wet ground (fs_ssr): what they show, premultiplied
// by how much.
@group(0) @binding(9) var ssr_tex: texture_2d<f32>;

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

// The surface's normal at `p` (camera's space), from the nearer neighbours
// each way, so an edge's far side does not bend it.
fn normal_at(p: vec2<i32>, c: vec3<f32>) -> vec3<f32> {
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
    return n;
}

// Where `v` (camera's space) falls on the scene's screen, in pixels, and
// its depth there.
fn screen_of(v: vec3<f32>) -> vec3<f32> {
    let clip = post.proj * vec4<f32>(v, 1.0);
    let ndc = clip.xyz / clip.w;
    return vec3<f32>((ndc.x * 0.5 + 0.5) * post.size.x, (0.5 - ndc.y * 0.5) * post.size.y, ndc.z);
}

// Ambient occlusion (the Alchemy estimate, McGuire 2011): how much of the
// open sky round each pixel the nearby surfaces hide.
fn alchemy(p: vec2<i32>, c: vec3<f32>, n: vec3<f32>, noise: f32) -> f32 {
    let radius = post.params.x;
    let reach = clamp(radius * post.params.w / max(-c.z, 0.1), 2.0, 80.0);
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
    return max(1.0 - 2.0 * post.params.y * sum / f32(count), 0.0);
}

// Ambient occlusion from the horizons (GTAO, Jimenez et al. 2016): across
// a few slices of the view round each pixel, how high the surfaces round
// it rise each way, and so how much of the sky's light, as a surface
// facing `n` takes it, they leave.
fn gtao(p: vec2<i32>, c: vec3<f32>, n: vec3<f32>, noise: f32, jitter: f32) -> f32 {
    let radius = post.params.x;
    let reach = clamp(radius * post.params.w / max(-c.z, 0.1), 3.0, 64.0);
    let eye = normalize(-c);
    let slices = 3;
    let steps = 6;
    var seen = 0.0;
    for (var s = 0; s < slices; s++) {
        let phi = (f32(s) + noise) * 3.14159 / f32(slices);
        let way = vec2<f32>(cos(phi), sin(phi));
        // The slice's way in the camera's space (the screen's y is down).
        let dir = vec3<f32>(way.x, -way.y, 0.0);
        let ortho = dir - dot(dir, eye) * eye;
        let axis = normalize(cross(ortho, eye));
        let projected = n - axis * dot(n, axis);
        let length_n = max(length(projected), 1e-4);
        let cos_n = clamp(dot(projected, eye) / length_n, -1.0, 1.0);
        let angle_n = sign(dot(ortho, projected)) * acos(cos_n);
        var h = vec2<f32>(-1.0);
        for (var i = 0; i < steps; i++) {
            let t = (f32(i) + jitter) / f32(steps);
            let off = way * max(reach * t * t, 1.0 + f32(i));
            for (var side = 0; side < 2; side++) {
                let q = p + vec2<i32>(select(-off, off, side == 1));
                let v = view_at(q) - c;
                let len = length(v);
                // Past the radius a surface counts for less and less, so
                // a far wall does not shade the ground before it.
                let fade = clamp(len * len / (radius * radius * 2.0), 0.0, 1.0);
                let rise = mix(dot(v, eye) / max(len, 1e-4), -1.0, fade);
                h[side] = max(h[side], rise);
            }
        }
        let h0 = angle_n + clamp(-acos(h.x) - angle_n, -1.5708, 1.5708);
        let h1 = angle_n + clamp(acos(h.y) - angle_n, -1.5708, 1.5708);
        let sin_n = sin(angle_n);
        let arc0 = (cos_n + 2.0 * h0 * sin_n - cos(2.0 * h0 - angle_n)) * 0.25;
        let arc1 = (cos_n + 2.0 * h1 * sin_n - cos(2.0 * h1 - angle_n)) * 0.25;
        seen += length_n * (arc0 + arc1);
    }
    // The depth's own small errors read as a little shade everywhere:
    // most of the sky open counts as open.
    let visible = clamp(seen / f32(slices) / 0.85, 0.0, 1.0);
    // As strong as the plain one, softened toward full light: a crease is
    // darker, open ground no darker than it was.
    return mix(1.0, visible, 0.55 * post.params.y / 1.5);
}

// Contact shadows: from `c` a short way toward the sun, across the
// screen's depth, whether anything just in front stands in the way: the
// fine shadow where a tyre or a boot meets the ground, which the shadow
// maps are too coarse for. 1 lit, 0 in its shadow.
fn contact(c: vec3<f32>, n: vec3<f32>, jitter: f32) -> f32 {
    let far = -c.z;
    if (post.view_sun.w <= 0.0 || far > 90.0 || dot(n, post.view_sun.xyz) <= 0.0) {
        return 1.0;
    }
    // Longer further off, where a pixel covers more.
    let length_ft = clamp(far * 0.03, 0.8, 2.5);
    let steps = 12;
    let start = c + n * (0.02 + far * 0.002);
    var shade = 0.0;
    for (var i = 0; i < steps; i++) {
        let t = (f32(i) + jitter) / f32(steps);
        let at = start + post.view_sun.xyz * length_ft * t;
        let q = screen_of(at);
        if (any(q.xy < vec2<f32>(0.0)) || any(q.xy >= post.size.xy)) {
            break;
        }
        let scene = view_at(vec2<i32>(q.xy));
        // In front of the ray, by a little, but not so much it is
        // something far in front (a rider before a wall).
        let gap = scene.z - at.z;
        let thick = 0.6 + far * 0.01;
        if (gap > 0.03 + far * 0.002 && gap < thick) {
            shade = max(shade, 1.0 - t);
        }
    }
    // Fading out with distance and toward the end of the ray.
    return 1.0 - shade * (1.0 - smoothstep(50.0, 90.0, far));
}

@fragment
fn fs_ao(in: Out) -> @location(0) vec4<f32> {
    // Worked out at half the scene's size, then blurred and stretched.
    let p = vec2<i32>(in.position.xy) * 2;
    if (depth_at(p) >= 1.0) {
        return vec4<f32>(1.0);
    }
    let c = view_at(p);
    let n = normal_at(p, c);
    // Lifted off the surface a little, so its own bumps hide nothing.
    let lifted = c + n * (0.03 + -c.z * 0.004);
    // A different turn each pixel; the blur after evens it out.
    let noise = fract(52.9829189 * fract(dot(in.position.xy, vec2<f32>(0.06711056, 0.00583715))));
    let jitter = fract(noise * 7.31 + 0.5);
    var ao = 1.0;
    if (post.flags.x > 0.5) {
        if (post.extra.x > 0.5) {
            ao = gtao(p, lifted, n, noise, jitter);
        } else {
            ao = alchemy(p, c, n, noise);
        }
    }
    let lit = contact(c, n, jitter);
    return vec4<f32>(ao, lit, 0.0, 1.0);
}

// The occlusion blurred over 4×4 pixels, in four filtered taps.
@fragment
fn fs_ao_blur(in: Out) -> @location(0) vec4<f32> {
    let t = post.size.zw;
    var sum = vec2<f32>(0.0);
    sum += textureSampleLevel(ao_tex, smooth_sampler, in.uv + vec2<f32>(-1.0, -1.0) * t, 0.0).rg;
    sum += textureSampleLevel(ao_tex, smooth_sampler, in.uv + vec2<f32>(1.0, -1.0) * t, 0.0).rg;
    sum += textureSampleLevel(ao_tex, smooth_sampler, in.uv + vec2<f32>(-1.0, 1.0) * t, 0.0).rg;
    sum += textureSampleLevel(ao_tex, smooth_sampler, in.uv + vec2<f32>(1.0, 1.0) * t, 0.0).rg;
    let ao = sum * 0.25;
    return vec4<f32>(ao, 0.0, 1.0);
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

// The film-like curve (AgX, after Sobotka, in Wrensch's fitted form):
// colours go through a log encoding and an S-curve in a slightly
// narrowed colour space, so bright light rolls off softly toward white
// and shadows gain a gentle toe, as on film. The log range is set so the
// game's middle tones land close to where they were (0.18 shows at about
// 0.5, white at 0.87), not darker as the curve's own range would.
fn agx_curve(x: vec3<f32>) -> vec3<f32> {
    let x2 = x * x;
    let x4 = x2 * x2;
    return 15.5 * x4 * x2 - 40.14 * x4 * x + 31.96 * x4 - 6.868 * x2 * x + 0.4298 * x2 + 0.1191 * x - 0.00232;
}

fn filmic(colour: vec3<f32>) -> vec3<f32> {
    let inset = mat3x3<f32>(
        0.842479062253094, 0.0423282422610123, 0.0423756549057051,
        0.0784335999999992, 0.878468636469772, 0.0784336,
        0.0792237451477643, 0.0791661274605434, 0.879142973793104,
    );
    let outset = mat3x3<f32>(
        1.19687900512017, -0.0528968517574562, -0.0529716355144438,
        -0.0980208811401368, 1.15190312990417, -0.0980434501171241,
        -0.0990297440797205, -0.0989611768448433, 1.15107367264116,
    );
    let lo = -10.0;
    let hi = 2.3;
    var c = inset * max(colour, vec3<f32>(1e-6));
    c = (clamp(log2(c), vec3<f32>(lo), vec3<f32>(hi)) - lo) / (hi - lo);
    c = outset * agx_curve(c);
    return pow(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(2.2));
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
    if (post.view_up.w > 0.0) {
        let r = textureSampleLevel(ssr_tex, smooth_sampler, in.uv, 0.0);
        c = c * (1.0 - r.a) + r.rgb;
    }
    if (post.flags.x > 0.5 || post.view_sun.w > 0.0) {
        let a = textureSampleLevel(ao_tex, smooth_sampler, in.uv, 0.0).rg;
        if (post.flags.x > 0.5) {
            c *= a.r;
        }
        c *= mix(1.0, a.g, post.view_sun.w);
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
    if (post.flags.z > 1.5) {
        c = filmic(c);
    } else if (post.flags.z > 0.5) {
        c = neutral(c);
    }
    if (post.flags.w > 0.5) {
        c = grade(c, in.uv);
    }
    return vec4<f32>(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// Reflections in wet ground (screen space): where the scene marks its
// ground wet (the ground's alpha, terrain.wgsl: 2 and up, 3 for still
// water), the ray off the water followed across the screen's depth
// to what it meets, and that pixel's colour shown, as strongly as the
// water's angle reflects. Worked out at half size. Premultiplied: what it
// shows times how much, and how much.
@fragment
fn fs_ssr(in: Out) -> @location(0) vec4<f32> {
    let p = vec2<i32>(in.position.xy) * 2;
    if (depth_at(p) >= 1.0) {
        return vec4<f32>(0.0);
    }
    let water = clamp(textureLoad(hdr_tex, p, 0).a - 2.0, 0.0, 1.0);
    if (water < 0.02) {
        return vec4<f32>(0.0);
    }
    let c = view_at(p);
    let n = normal_at(p, c);
    let up = post.view_up.xyz;
    // Only level ground holds water; its normal is the way up, not the
    // depth's own, so the reflection does not shiver.
    if (dot(n, up) < 0.9) {
        return vec4<f32>(0.0);
    }
    let eye = normalize(c);
    let r = reflect(eye, up);
    let fresnel = 0.02 + 0.98 * pow(1.0 - max(dot(-eye, up), 0.0), 5.0);
    // A little over what the ground's own sky in the water takes (its
    // fresnel, terrain.wgsl), so what stands round it shows.
    let amount = fresnel * water * post.view_up.w * 1.5;
    if (amount < 0.01) {
        return vec4<f32>(0.0);
    }
    let far = -c.z;
    // Steps that grow, near the water fine, far off coarse.
    let steps = 24;
    let reach = clamp(far * 5.0, 60.0, 600.0);
    let noise = fract(52.9829189 * fract(dot(in.position.xy, vec2<f32>(0.06711056, 0.00583715))));
    var before = 0.0;
    var hit = -1.0;
    for (var i = 1; i <= steps; i++) {
        let t = (f32(i) - 0.5 + noise) / f32(steps);
        let d = reach * t * t;
        let at = c + r * d;
        if (at.z > -0.5) {
            break;
        }
        let q = screen_of(at);
        if (any(q.xy < vec2<f32>(0.0)) || any(q.xy >= post.size.xy)) {
            break;
        }
        let scene = view_at(vec2<i32>(q.xy));
        let gap = scene.z - at.z;
        // Behind what the screen shows there, and not far behind (else it
        // passed behind something, which the screen cannot show).
        if (gap > 0.0 && gap < max(d - before, 1.0) * 1.5 + 0.5) {
            // Narrowed down between the last step and this one.
            var lo = before;
            var hi = d;
            for (var k = 0; k < 4; k++) {
                let mid = (lo + hi) * 0.5;
                let m = c + r * mid;
                let mq = screen_of(m);
                if (view_at(vec2<i32>(mq.xy)).z - m.z > 0.0) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            hit = hi;
            break;
        }
        before = d;
    }
    if (hit < 0.0) {
        return vec4<f32>(0.0);
    }
    let q = screen_of(c + r * hit);
    // What is there must itself not be the sky, and fades toward the
    // screen's edges and the end of the ray, where what it would show
    // leaves the screen.
    if (depth_at(vec2<i32>(q.xy)) >= 1.0) {
        return vec4<f32>(0.0);
    }
    let uv = q.xy * post.size.zw;
    let edge = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));
    let fade = smoothstep(0.0, 0.08, edge) * (1.0 - smoothstep(0.6, 1.0, hit / reach));
    let shown = finite(textureLoad(hdr_tex, vec2<i32>(q.xy), 0).rgb);
    let a = clamp(amount * fade, 0.0, 0.85);
    return vec4<f32>(shown * a, a);
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
