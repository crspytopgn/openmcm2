// The picture extras' last step over the race (polish.rs): smoother
// edges, then sharpening, from the scene drawn into a picture of its own.

struct Params {
    // 1 / width, 1 / height, edge smoothing on (1) or off, sharpening 0 to 1.
    v: vec4<f32>,
    // x: how far out sharpening reads, pixels.
    reach: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var scene: texture_2d<f32>;
@group(0) @binding(2) var smp: sampler;

struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_full(@builtin(vertex_index) i: u32) -> Out {
    // One triangle over the whole screen.
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: Out;
    o.pos = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2<f32>(p.x, 1.0 - p.y);
    return o;
}

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.299, 0.587, 0.114));
}

fn at(uv: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(scene, smp, uv, 0.0).rgb;
}

// Edge smoothing: where the brightness steps sharply between neighbours,
// the pixel is blended along the edge rather than across it.
fn smooth_edges(uv: vec2<f32>, px: vec2<f32>) -> vec3<f32> {
    let c = at(uv);
    let n = luma(at(uv + vec2<f32>(0.0, -px.y)));
    let s = luma(at(uv + vec2<f32>(0.0, px.y)));
    let w = luma(at(uv + vec2<f32>(-px.x, 0.0)));
    let e = luma(at(uv + vec2<f32>(px.x, 0.0)));
    let m = luma(c);
    let lo = min(m, min(min(n, s), min(w, e)));
    let hi = max(m, max(max(n, s), max(w, e)));
    let range = hi - lo;
    // Flat areas and faint steps stay as they are.
    if (range < max(0.04, hi * 0.12)) {
        return c;
    }
    let nw = luma(at(uv + vec2<f32>(-px.x, -px.y)));
    let ne = luma(at(uv + vec2<f32>(px.x, -px.y)));
    let sw = luma(at(uv + vec2<f32>(-px.x, px.y)));
    let se = luma(at(uv + vec2<f32>(px.x, px.y)));
    // Which way the edge runs: across it the brightness changes most.
    var dir = vec2<f32>(-((nw + ne) - (sw + se)), (nw + sw) - (ne + se));
    let reduce = max((nw + ne + sw + se) * 0.03125, 1.0 / 128.0);
    let scale = 1.0 / (min(abs(dir.x), abs(dir.y)) + reduce);
    dir = clamp(dir * scale, vec2<f32>(-8.0), vec2<f32>(8.0)) * px;
    let near = 0.5 * (at(uv + dir * (1.0 / 3.0 - 0.5)) + at(uv + dir * (2.0 / 3.0 - 0.5)));
    let far = near * 0.5 + 0.25 * (at(uv - dir * 0.5) + at(uv + dir * 0.5));
    let lf = luma(far);
    if (lf < lo || lf > hi) {
        return near;
    }
    return far;
}

@fragment
fn fs_polish(in: Out) -> @location(0) vec4<f32> {
    let px = params.v.xy;
    let smooth_on = params.v.z > 0.5;
    let amount = params.v.w;
    var c: vec3<f32>;
    if (smooth_on) {
        c = smooth_edges(in.uv, px);
    } else {
        c = at(in.uv);
    }
    if (amount > 0.0) {
        // Sharpening that holds back where the neighbours already differ
        // a lot, so edges don't ring and noise isn't raised.
        // The game's textures are stretched over many pixels, so their
        // detail is a few pixels wide: the neighbours are read that far
        // out (more on a bigger window), between pixels.
        let r = px * params.reach.x;
        let n = at(in.uv + vec2<f32>(0.0, -r.y));
        let s = at(in.uv + vec2<f32>(0.0, r.y));
        let w = at(in.uv + vec2<f32>(-r.x, 0.0));
        let e = at(in.uv + vec2<f32>(r.x, 0.0));
        let lo = min(c, min(min(n, s), min(w, e)));
        let hi = max(c, max(max(n, s), max(w, e)));
        let room = min(lo, vec3<f32>(1.0) - hi) / max(hi, vec3<f32>(1e-4));
        let k = sqrt(clamp(room, vec3<f32>(0.0), vec3<f32>(1.0))) * (-0.2 * amount);
        c = clamp((c + (n + s + w + e) * k) / (1.0 + 4.0 * k), vec3<f32>(0.0), vec3<f32>(1.0));
    }
    // Read as shown (sRGB-coded), written to the window's sRGB view,
    // which codes it again: so back to linear here.
    let lin = select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
    return vec4<f32>(lin, 1.0);
}
