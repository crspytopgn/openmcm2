// A river's water (water.rs): a ribbon down the river's middle line, the
// light off its moving surface. Its own pass over the scene, lit by the
// scene's globals as the ground is, so the game's drawing and the
// enhanced one both show it.

struct Globals {
    view_proj: mat4x4<f32>,
    camera: vec4<f32>,
    grid: vec4<f32>,
    fog: vec4<f32>,
    fog_range: vec4<f32>,
    ambient: vec4<f32>,
    sun: vec4<f32>,
    to_sun: vec4<f32>,
    shadow_view: mat4x4<f32>,
    shadow_on: vec4<f32>,
};

struct Water {
    // x: seconds; y: how bright the sky is over the water (1 by day).
    time: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var<uniform> water: Water;

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    // Feet down the river, and across it from -1 to 1.
    @location(1) along: vec2<f32>,
    // The way it flows, in the drawing's xz.
    @location(2) flow: vec2<f32>,
};

@vertex
fn vs_main(@location(0) pos: vec3<f32>, @location(1) along: vec2<f32>, @location(2) flow: vec2<f32>) -> VertexOut {
    var out: VertexOut;
    out.clip = globals.view_proj * vec4<f32>(pos, 1.0);
    out.world = pos;
    out.along = along;
    out.flow = flow;
    return out;
}

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2<f32>(1.0, 0.0)), u.x), mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0, 1.0)), u.x), u.y);
}

// The surface's height in small ripples, carried down the river (`q` in
// river space: down, across, in ft).
fn ripples(q: vec2<f32>, t: f32) -> f32 {
    var h = 0.0;
    h += noise(vec2<f32>(q.x * 0.11 - t * 0.55, q.y * 0.18)) * 0.5;
    h += noise(vec2<f32>(q.x * 0.27 - t * 1.1, q.y * 0.35 + 3.0)) * 0.3;
    h += noise(vec2<f32>(q.x * 0.71 - t * 2.3, q.y * 0.8 + 7.0)) * 0.14;
    h += noise(vec2<f32>(q.x * 1.6 - t * 3.1 + q.y * 0.4, q.y * 1.7)) * 0.06;
    return h;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let t = water.time.x;
    let across = in.along.y;
    // Across in ft, near enough: the ribbon's own width is in the flow's
    // length (water.rs).
    let half = length(in.flow);
    let q = vec2<f32>(in.along.x, across * half);
    let e = 0.35;
    let h0 = ripples(q, t);
    let hx = ripples(q + vec2<f32>(e, 0.0), t);
    let hy = ripples(q + vec2<f32>(0.0, e), t);
    // Steeper where the water runs fast in the middle.
    let rough = 0.55 + 0.45 * (1.0 - across * across);
    let slope = vec2<f32>(hx - h0, hy - h0) / e * 0.9 * rough;
    // From river space back to the drawing's.
    let down = normalize(in.flow);
    let side = vec2<f32>(-down.y, down.x);
    let tilt = down * slope.x + side * slope.y;
    let n = normalize(vec3<f32>(-tilt.x, 1.0, -tilt.y));

    let to_eye = normalize(globals.camera.xyz - in.world);
    let cos_v = max(dot(n, to_eye), 0.0);
    let fresnel = (0.02 + 0.98 * pow(1.0 - cos_v, 5.0)) * 0.62;
    let to_sun = normalize(globals.to_sun.xyz);
    let sky = globals.fog.rgb * (1.05 + 0.25 * water.time.y);
    // The sky it shows: paler at the horizon.
    let r = reflect(-to_eye, n);
    let reflected = mix(sky * 0.82, sky * vec3<f32>(0.5, 0.58, 0.66), clamp(r.y * 1.6, 0.0, 1.0));
    // The water itself: dark and a little green where deep, the sandy
    // bed showing through at the edges.
    let deep = vec3<f32>(0.035, 0.06, 0.05);
    let shallow = vec3<f32>(0.19, 0.17, 0.11);
    let edge = smoothstep(0.45, 1.0, abs(across));
    let light = globals.ambient.rgb + globals.sun.rgb * max(to_sun.y, 0.0) * 0.6;
    let body = mix(deep, shallow, edge) * light;
    var colour = mix(body, reflected, fresnel);
    // The sun's glints.
    let glint = pow(max(dot(r, to_sun), 0.0), 220.0) * 3.5 + pow(max(dot(r, to_sun), 0.0), 24.0) * 0.12;
    colour += globals.sun.rgb * glint * water.time.y;
    // A little froth where it meets the banks.
    let froth = smoothstep(0.78, 0.97, abs(across)) * smoothstep(0.45, 0.75, noise(q * vec2<f32>(0.6, 1.2) - vec2<f32>(t * 1.4, 0.0)));
    colour = mix(colour, light * vec3<f32>(0.62, 0.63, 0.6), froth * 0.22);
    // Seen through at the very edge, so it meets the bank softly.
    let alpha = mix(0.88, 0.0, smoothstep(0.8, 1.0, abs(across)));
    let dist = length(in.world - globals.camera.xyz);
    let range = max(globals.fog_range.y - globals.fog_range.x, 1.0);
    let fog = clamp((dist - globals.fog_range.x) / range, 0.0, 1.0);
    return vec4<f32>(mix(colour, globals.fog.rgb, fog), alpha * (1.0 - fog * 0.5));
}
