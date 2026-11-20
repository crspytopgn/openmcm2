// Dust clouds behind the bikes for the enhanced drawing (dust.rs), none of
// them the game's: soft, lumpy puffs the colour of the ground they rose
// from, wispy at the edges and thicker in clumps, lit as balls of dust
// (bright toward the sun, darker beneath), that billow, drift and thin
// out, shadowed as the ground is.

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

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var textures: texture_2d_array<f32>;
@group(0) @binding(2) var tex_sampler: sampler;
@group(0) @binding(3) var tile_map: texture_2d<u32>;

#include "enhanced.wgsl"

@group(1) @binding(0) var<uniform> enh: Enh;
@group(1) @binding(1) var sun_map: texture_depth_2d_array;
@group(1) @binding(2) var sun_sampler: sampler_comparison;

#include "enhanced_fns.wgsl"

// The baked pictures of lumpy dust (dust.rs puff_atlas): red how thick,
// green the lumps' height, blue and alpha their slope.
@group(2) @binding(0) var puffs: texture_2d_array<f32>;
@group(2) @binding(1) var puff_sampler: sampler;

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) centre: vec3<f32>,
    @location(2) @interpolate(flat) look: vec4<f32>,
    @location(3) @interpolate(flat) colour: vec3<f32>,
    // Across the puff, in the world: with the way to the eye, its surface.
    @location(4) side: vec3<f32>,
    @location(5) @interpolate(flat) to_eye: vec3<f32>,
    // 0 for a puff, 1 for a drop of spray, 2 for a clod.
    @location(6) @interpolate(flat) kind: f32,
    // The puff's own across and up, in the world.
    @location(7) @interpolate(flat) right: vec3<f32>,
    // How much of the sun reaches this corner: past what is solid at the
    // puff's top, then through the dust to it. Asked at the corners and
    // blended across, not for every pixel.
    @location(8) sun: f32,
    // The two pictures and turns its shape is read with (x, y the
    // pictures; z, w the turns), and how much of the second.
    @location(9) @interpolate(flat) shape: vec4<f32>,
    @location(10) @interpolate(flat) blend: f32,
};

// The painted ground's colour (linear) under drawing point `xz`, blurred.
fn ground_colour(xz: vec2<f32>) -> vec3<f32> {
    let t = vec2<f32>(xz.x, -xz.y) / globals.grid.y;
    let tiles = i32(globals.grid.w);
    let tc = t / globals.grid.z;
    let cell = clamp(vec2<i32>(floor(tc)), vec2<i32>(0), vec2<i32>(tiles - 1));
    let layer = textureLoad(tile_map, cell, 0).r;
    if (layer == 0xffffu) {
        return vec3<f32>(0.35, 0.28, 0.2);
    }
    let at = tile_layer(layer, tc - vec2<f32>(cell));
    return textureSampleLevel(textures, tex_sampler, at.xy, u32(at.z), 4.0 + select(0.0, 1.0, layer >= 0x8000u)).rgb;
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


@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @location(0) place: vec4<f32>, @location(1) look: vec4<f32>, @location(2) home: vec4<f32>) -> Out {
    let corner = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    )[vi];
    let kind = home.z;
    let to_eye = normalize(globals.camera.xyz - place.xyz);
    let right = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), to_eye));
    let up = cross(to_eye, right);
    // Brought toward the eye by its size and shrunk to match, so it looks
    // the same but the ground in front of it does not cut it off flat.
    let d = length(globals.camera.xyz - place.xyz);
    let pull = select(min(place.w, d * 0.5), 0.0, kind > 0.5);
    var side = right * corner.x + up * corner.y;
    var world = place.xyz + to_eye * pull + side * place.w * (d - pull) / max(d, 1e-3);
    if (kind > 0.5) {
        // A drop or a clod is drawn streaked along the way it flies, as
        // in a blink: a drop much more than a clod.
        let v = look.yzw - to_eye * dot(look.yzw, to_eye);
        let along = select(up, normalize(v), length(v) > 1e-3);
        let across = normalize(cross(along, to_eye));
        // (A drop as at a 50th of a second, grit and clods a 120th.)
        let reach = place.w + length(v) * select(0.02, 0.008, kind > 1.5);
        side = across * corner.x + along * corner.y;
        world = place.xyz + across * corner.x * place.w + along * corner.y * reach;
    }
    var out: Out;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.uv = corner;
    out.centre = place.xyz;
    out.look = look;
    // Lighter than the ground and a little paler, as fine dust is, but
    // keeping its warmth.
    let g = ground_colour(home.xy);
    let grey = vec3<f32>(dot(g, vec3<f32>(0.3, 0.59, 0.11)));
    out.colour = mix(grey, g, 0.8) * 2.3 + vec3<f32>(0.06, 0.05, 0.035);
    if (kind > 2.5) {
        // Grit: the ground's own dirt, a little lighter for being dry and
        // broken up, as a cloud of it is.
        out.colour = mix(grey, g, 0.9) * 1.25 + vec3<f32>(0.02, 0.016, 0.01);
    } else if (kind > 1.5) {
        // A clod: the ground's dirt, a shade darker and damper from just
        // under its top.
        out.colour = mix(grey, g, 0.95) * 0.85;
    } else if (kind > 0.5) {
        // Muddy water: the ground's colour, a little darker.
        out.colour = g * 0.7;
    }
    out.side = side;
    out.to_eye = to_eye;
    out.kind = kind;
    out.right = right;
    // Its shape: a puff turns slowly one way as it churns, blended with a
    // second picture turning the other, so it rolls rather than spins; a
    // clod tumbles (dust.rs works out its turn).
    let s = select(look.y, fract(home.w * 7.31), kind > 0.5);
    let n = 8.0;
    if (kind > 1.5 && kind < 2.5) {
        out.shape = vec4<f32>(floor(home.w), floor(home.w), fract(home.w) * 6.2831853, 0.0);
        out.blend = 0.0;
    } else {
        let age = look.z;
        out.shape = vec4<f32>(floor(s * n), floor(fract(s * 5.3) * n), s * 6.2831853 + age * 0.22, s * 11.0 - age * 0.17);
        out.blend = 0.5 + 0.45 * sin(age * 0.6 + s * 6.2831853);
    }
    // The sun past what is solid at the puff's top (the ray-traced
    // scene's, else the shadow maps'), so the bike's own shadow does not
    // black out the dust right behind it; then through the dust between
    // this corner and the sun, its own included, so a cloud is bright on
    // the sun's side and dim beneath and behind.
    var sun = 1.0;
    if (kind < 1.5 || kind > 2.5) {
        let top = place.xyz + vec3<f32>(0.0, place.w * 0.5, 0.0);
        sun = sun_point(top);
        if (sun < 0.0) {
            sun = sun_solid(top, vec3<f32>(0.0, 1.0, 0.0), d, 0.0);
        }
        if (kind < 0.5) {
            // At the corner's point on the ball's sunward side: a cloud is
            // lit on its face to the sun and dark under and behind.
            sun *= dust_through(place.xyz + (side * 0.6 + enh.to_sun.xyz * 0.5) * place.w);
        }
    } else {
        sun = sun_point(place.xyz);
        if (sun < 0.0) {
            sun = sun_solid(place.xyz, vec3<f32>(0.0, 1.0, 0.0), d, 0.0);
        }
    }
    out.sun = sun;
    return out;
}

// ---- The dust as the sun sees it, for its shadows (enhanced_dust.wgsl). ----

struct ShadowOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // How thick it is, and how deep in the map.
    @location(1) @interpolate(flat) thick: f32,
    @location(2) @interpolate(flat) depth: f32,
};

@vertex
fn vs_shadow(@builtin(vertex_index) vi: u32, @location(0) place: vec4<f32>, @location(1) look: vec4<f32>, @location(2) home: vec4<f32>) -> ShadowOut {
    let corner = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    )[vi];
    let l = enh.to_sun.xyz;
    let right = normalize(cross(select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(l.y) > 0.99), l));
    let up = cross(l, right);
    // A little smaller than it is drawn: its wispy rim casts nothing.
    let world = place.xyz + (right * corner.x + up * corner.y) * place.w * 0.85;
    var out: ShadowOut;
    out.clip = enh.dust_view * vec4<f32>(world, 1.0);
    out.uv = corner;
    // Drops and clods cast nothing here.
    out.thick = select(look.x, 0.0, home.z > 0.5);
    out.depth = (enh.dust_view * vec4<f32>(place.xyz, 1.0)).z;
    return out;
}

@fragment
fn fs_shadow(in: ShadowOut) -> @location(0) vec4<f32> {
    let r2 = dot(in.uv, in.uv);
    if (r2 > 1.0 || in.thick <= 0.0) {
        discard;
    }
    // Through a ball: thickest across its middle.
    let tau = in.thick * 1.4 * (1.0 - r2);
    return vec4<f32>(tau, tau * in.depth, tau * in.depth * in.depth, 0.0);
}

// The baked picture `layer` at `uv` (-1 to 1 across) turned by `turn`:
// how thick, the lumps' height, and their slope turned back to the puff's
// own across and up.
// `dx` and `dy` are how uv changes a pixel across and down, for the
// picture's level of detail.
fn picture(uv: vec2<f32>, layer: f32, turn: f32, dx: vec2<f32>, dy: vec2<f32>) -> vec4<f32> {
    let c = cos(turn);
    let s = sin(turn);
    let q = vec2<f32>(c * uv.x - s * uv.y, s * uv.x + c * uv.y);
    // Up the puff is up the picture, down its rows.
    let t = textureSampleGrad(puffs, puff_sampler, vec2<f32>(q.x * 0.5 + 0.5, 0.5 - q.y * 0.5), i32(layer), dx * 0.5, dy * 0.5);
    let sl = (t.zw - 0.5) * 8.0;
    return vec4<f32>(t.x, t.y, c * sl.x + s * sl.y, -s * sl.x + c * sl.y);
}

@fragment
fn fs_main(in: Out) -> @location(0) vec4<f32> {
    // Asked first, while every pixel still runs.
    let dx = dpdx(in.uv);
    let dy = dpdy(in.uv);
    let dist = length(in.centre - globals.camera.xyz);
    if (in.kind > 0.5 && in.kind < 1.5) {
        // A drop: a soft streak, the sky's light caught in it.
        let r = length(in.uv);
        if (r > 1.0) {
            discard;
        }
        let a = (1.0 - smoothstep(0.3, 1.0, r)) * in.look.x * smoothstep(1.5, 4.0, dist);
        let lit = globals.ambient.rgb * 1.1 + globals.sun.rgb * 0.5;
        let c = in.colour * pow(soft_clamp(lit), vec3<f32>(2.2)) + enh.horizon.rgb * 0.03;
        return vec4<f32>(apply_haze(c, in.centre, dist) * a, a);
    }
    let l = globals.to_sun.xyz;
    let up = cross(in.to_eye, in.right);
    if (in.kind > 2.5) {
        // Grit: a small soft streak, lit by the sun and the sky; too fine
        // to be seen alone up close, so it thins out near the eye.
        let r = length(in.uv);
        if (r > 1.0) {
            discard;
        }
        let a = (1.0 - smoothstep(0.35, 1.0, r)) * in.look.x * smoothstep(1.2, 3.5, dist);
        let lit = globals.ambient.rgb * 1.05 + globals.sun.rgb * in.sun * 0.75;
        let c = in.colour * pow(soft_clamp(lit), vec3<f32>(2.2));
        return vec4<f32>(apply_haze(c, in.centre, dist) * a, a);
    }
    if (in.kind > 1.5) {
        // A clod: a solid lump cut from the picture's thickest part, lit
        // as a little rough stone, a hard edge softened by a pixel.
        let p = picture(in.uv * 0.8, in.shape.x, in.shape.z, dx * 0.8, dy * 0.8);
        let edge = (abs(dx.x) + abs(dx.y) + abs(dy.x) + abs(dy.y)) * 0.6 + 0.02;
        let a = smoothstep(0.45 - edge, 0.45 + edge, p.x) * in.look.x * smoothstep(1.5, 5.0, dist);
        if (a < 0.01) {
            discard;
        }
        let r = min(length(in.uv), 1.0);
        let ball = normalize(in.side * r + in.to_eye * sqrt(max(1.0 - r * r, 0.05)));
        let sn = normalize(ball - (in.right * p.z + up * p.w) * 0.6);
        let lit = globals.ambient.rgb * mix(0.7, 1.15, sn.y * 0.5 + 0.5) + globals.sun.rgb * in.sun * max(dot(sn, l), 0.0);
        let c = in.colour * mix(0.75, 1.1, p.y) * pow(soft_clamp(lit), vec3<f32>(2.2));
        return vec4<f32>(apply_haze(c, in.centre, dist) * a, a);
    }
    // Lumps of dust from two baked pictures turning against each other as
    // it ages: wispy at the rim, clumped and thicker inside.
    let p = mix(picture(in.uv, in.shape.x, in.shape.z, dx, dy), picture(in.uv * 0.92, in.shape.y, in.shape.w, dx * 0.92, dy * 0.92), in.blend);
    // A touch thicker than the picture: two blended pictures thin each
    // other's clumps.
    let alpha = min(p.x * 1.15, 1.0) * in.look.x;
    if (alpha < 0.004) {
        discard;
    }
    // Lit as a ball: its surface faces out from the middle and toward the
    // eye, tilted by each lump, so the lumps are bright on the sun's side
    // and shaded on the other; the light wraps round a little, as through
    // dust.
    let r = min(length(in.uv), 1.0);
    let ball = normalize(in.side * r + in.to_eye * sqrt(max(1.0 - r * r, 0.05)));
    let sn = normalize(ball - (in.right * p.z + up * p.w) * 0.35);
    let wrap = clamp((dot(sn, l) + 0.3) / 1.3, 0.0, 1.0);
    // How deep in the puff this is: its middle is thicker than its rim.
    let body = 1.0 - smoothstep(0.2, 0.98, r);
    // Light coming through the thin rim when the sun is behind it.
    let behind = pow(max(dot(-in.to_eye, l), 0.0), 3.0) * (1.6 - body * 0.8);
    let sky = mix(0.9, 1.3, sn.y * 0.5 + 0.5);
    let lit = globals.ambient.rgb * sky + globals.sun.rgb * in.sun * (wrap + behind);
    // Its thick clumps shade themselves a little.
    let shade = mix(1.0, 0.72, body * smoothstep(0.4, 0.8, p.y));
    let c = in.colour * shade * pow(soft_clamp(lit), vec3<f32>(2.2));
    let hazed = apply_haze(c, in.centre, dist);
    // Thin near the eye, so a puff never fills the screen or hides the
    // rider from the chase camera.
    let near = smoothstep(4.0, 16.0, dist);
    let a = alpha * near;
    return vec4<f32>(hazed * a, a);
}
