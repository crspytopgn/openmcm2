fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// `n` bent by the bumps a texture's brightness `height` suggests, as it
// changes across the screen (bump mapping without tangents, Mikkelsen
// 2010).
fn bump(n: vec3<f32>, world: vec3<f32>, height: f32, strength: f32) -> vec3<f32> {
    let dp_dx = dpdx(world);
    let dp_dy = dpdy(world);
    let dh_dx = dpdx(height) * strength;
    let dh_dy = dpdy(height) * strength;
    let r1 = cross(dp_dy, n);
    let r2 = cross(n, dp_dx);
    let det = dot(dp_dx, r1);
    let grad = sign(det) * (dh_dx * r1 + dh_dy * r2);
    let bent = abs(det) * n - grad;
    if (dot(bent, bent) < 1e-12) {
        return n;
    }
    return normalize(bent);
}

// The game clamps its light at full; let it run a little past, rolling off.
fn soft_clamp(c: vec3<f32>) -> vec3<f32> {
    let over = max(c - vec3<f32>(0.9), vec3<f32>(0.0));
    return min(c, vec3<f32>(0.9)) + over / (vec3<f32>(1.0) + over * 2.5);
}

// How much of the sun reaches `world` (1 all, 0 none), from the cascade
// its distance from the camera falls in, filtered over 3×3 texels; seen
// from `lift` ft off the surface, so what lies flat on it (painted ruts,
// planks) casts nothing.
fn sun_visible(world: vec3<f32>, n: vec3<f32>, dist: f32, lift: f32) -> f32 {
    if (enh.flags.y < 0.5 || dist > enh.splits.w) {
        return 1.0;
    }
    var c = 0u;
    if (dist > enh.splits.x) {
        c = 1u;
    }
    if (dist > enh.splits.y) {
        c = 2u;
    }
    let texel = enh.texel[c];
    // Off the surface by a texel or two, so it does not shadow itself.
    let lifted = world + n * (texel * 1.5 + lift) + enh.to_sun.xyz * texel;
    let p = enh.cascades[c] * vec4<f32>(lifted, 1.0);
    let uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    if (any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || p.z > 1.0) {
        return 1.0;
    }
    let step = 1.0 / 2048.0;
    if (enh.texel.w > 0.5) {
        return mix(soft_sun(uv, p.z, c, texel), 1.0, smoothstep(enh.splits.w * 0.9, enh.splits.w, dist));
    }
    var sum = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            sum += textureSampleCompareLevel(sun_map, sun_sampler, uv + vec2<f32>(f32(x), f32(y)) * step, c, p.z);
        }
    }
    let seen = sum / 9.0;
    // Fade out over the last tenth of the last cascade.
    let fade = smoothstep(enh.splits.w * 0.9, enh.splits.w, dist);
    return mix(seen, 1.0, fade);
}

const POISSON: array<vec2<f32>, 16> = array<vec2<f32>, 16>(
    vec2<f32>(-0.94201624, -0.39906216), vec2<f32>(0.94558609, -0.76890725),
    vec2<f32>(-0.09418410, -0.92938870), vec2<f32>(0.34495938, 0.29387760),
    vec2<f32>(-0.91588581, 0.45771432), vec2<f32>(-0.81544232, -0.87912464),
    vec2<f32>(-0.38277543, 0.27676845), vec2<f32>(0.97484398, 0.75648379),
    vec2<f32>(0.44323325, -0.97511554), vec2<f32>(0.53742981, -0.47373420),
    vec2<f32>(-0.26496911, -0.41893023), vec2<f32>(0.79197514, 0.19090188),
    vec2<f32>(-0.24188840, 0.99706507), vec2<f32>(-0.81409955, 0.91437590),
    vec2<f32>(0.19984126, 0.78641367), vec2<f32>(0.14383161, -0.14100790),
);

// Soft sun shadows (percentage-closer soft shadows, Fernando 2005): how far
// the shadow lies from what casts it sets how soft its edge is, as the
// sun's disc blurs a real one: sharp where a tyre meets the ground, softer
// under a tree's crown.
fn soft_sun(uv: vec2<f32>, z: f32, c: u32, texel: f32) -> f32 {
    let side = 2048.0;
    // The cascade's depth, ft (enhance::cascades: 2 radii plus the reach
    // toward the sun).
    let depth_ft = side * texel + 1500.0;
    // A turn of the taps of its own for each pixel, so the few taps blur
    // into a smooth edge rather than repeat.
    let spin = fract(sin(dot(uv * side, vec2<f32>(12.9898, 78.233))) * 43758.547) * 6.2832;
    let turn = mat2x2<f32>(cos(spin), sin(spin), -sin(spin), cos(spin));
    // What lies between it and the sun, within 6 texels.
    var blockers = 0.0;
    var count = 0.0;
    for (var i = 0; i < 16; i++) {
        let at = uv + turn * POISSON[i] * 6.0 / side;
        let t = clamp(vec2<i32>(at * side), vec2<i32>(0), vec2<i32>(2047));
        let d = textureLoad(sun_map, t, c, 0);
        if (d < z) {
            blockers += d;
            count += 1.0;
        }
    }
    if (count < 0.5) {
        return 1.0;
    }
    let gap_ft = (z - blockers / count) * depth_ft;
    // The sun's disc, about half a degree across.
    let radius = clamp(gap_ft * 0.022 / texel, 1.0, 8.0);
    var sum = 0.0;
    for (var i = 0; i < 16; i++) {
        sum += textureSampleCompareLevel(sun_map, sun_sampler, uv + turn * POISSON[i] * radius / side, c, z);
    }
    return sum / 16.0;
}

// The enhanced fog: a haze that thickens with distance and never quite
// hides the land, in place of the game's fog wall.
fn haze_amount(dist: f32) -> f32 {
    return 1.0 - exp(-max(dist - enh.haze.x, 0.0) * enh.haze.y);
}

fn apply_haze(c: vec3<f32>, world: vec3<f32>, dist: f32) -> vec3<f32> {
    if (enh.flags.z > 0.5) {
        return mix(c, enh.fog.rgb, haze_amount(dist));
    }
    let range = max(globals.fog_range.y - globals.fog_range.x, 1.0);
    let fog = clamp((dist - globals.fog_range.x) / range, 0.0, 1.0);
    return mix(c, globals.fog.rgb, fog);
}

// A stadium's floodlights (enh.surfaces.w): four banks high in the
// corners, in place of the open sky's one sun. x the soft light they add
// on a surface facing `n`, y their glints off it seen from `v`, for a
// surface `shine` 0 (matte) to 1 (gloss).
fn lamps(n: vec3<f32>, v: vec3<f32>, shine: f32) -> vec2<f32> {
    // At night the stadium's own lamps light it instead (stadium_light).
    if (enh.surfaces.w < 0.5 || enh.stadium.y > 0.0) {
        return vec2<f32>(0.0);
    }
    var soft = 0.0;
    var glint = 0.0;
    let power = mix(12.0, 90.0, shine);
    for (var i = 0; i < 4; i++) {
        let sx = select(-1.0, 1.0, (i & 1) == 1);
        let sz = select(-1.0, 1.0, (i & 2) == 2);
        let l = normalize(vec3<f32>(sx * 0.8, 1.0, sz * 0.8));
        soft += max(dot(n, l), 0.0);
        let h = normalize(l + v);
        glint += pow(max(dot(n, h), 0.0), power) * (power + 2.0) / 8.0;
    }
    return vec2<f32>(soft * 0.12, glint * 0.03);
}

// The bikes' headlights' light at `world` facing `n`: none without ray
// tracing (enhanced_rt.wgsl's takes its place).
fn headlight_at(world: vec3<f32>, n: vec3<f32>, dist: f32) -> vec3<f32> {
    // A stadium's own lamps at night, shadows and all.
    let stadium = stadium_light(world, n);
    // Without ray tracing, only the light poles' light, baked over the
    // ground once (flood.rs): as it falls on the ground where a surface
    // faces up, and for what stands up from it, as much as reaches there,
    // less on faces turned down.
    if (enh.flood.z <= 0.0 || enh.night.x <= 0.0) {
        return stadium;
    }
    let uv = (world.xz - enh.flood.xy) / enh.flood.zw;
    if (any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0))) {
        return vec3<f32>(0.0);
    }
    let m = textureSampleLevel(flood_map, flood_sampler, uv, 0.0);
    let standing = vec3<f32>(0.95, 0.97, 1.0) / 0.97 * m.a * (0.15 + 0.3 * clamp(n.y + 0.5, 0.0, 1.0));
    let c = mix(standing, m.rgb, smoothstep(0.6, 0.9, n.y));
    return stadium + min(c, vec3<f32>(2.5)) * enh.night.x;
}

// The colour of a stadium's lamps: the near-white of metal halide.
const STADIUM_LAMP: vec3<f32> = vec3<f32>(1.0, 0.97, 0.92);
// The square distance (ft²) the lamps' strengths are given at (night.rs
// FALLOFF).
const STADIUM_FALLOFF: f32 = 10000.0;

// How much of shadow group `g`'s lamps reach `world` (1 all), from its map
// in the cascades' layer `g`, softly: the banks are broad, so the riders'
// shadows have soft edges.
fn stadium_seen(world: vec3<f32>, n: vec3<f32>, g: u32) -> f32 {
    if (f32(g) >= enh.stadium.w) {
        return 1.0;
    }
    let texel = enh.stadium_texels[g];
    let p = enh.stadium_views[g] * vec4<f32>(world + n * (texel * 2.0 + 0.15), 1.0);
    if (p.w <= 0.0) {
        return 1.0;
    }
    let q = p.xyz / p.w;
    let uv = vec2<f32>(q.x * 0.5 + 0.5, 0.5 - q.y * 0.5);
    if (any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || q.z > 1.0) {
        return 1.0;
    }
    let spin = fract(sin(dot(world.xz, vec2<f32>(12.9898, 78.233))) * 43758.547) * 6.2832;
    let turn = mat2x2<f32>(cos(spin), sin(spin), -sin(spin), cos(spin));
    // About 0.4 ft round, however fine the map.
    let radius = clamp(0.4 / texel, 1.0, 4.0) / 2048.0;
    var sum = 0.0;
    for (var i = 0; i < 8; i++) {
        sum += textureSampleCompareLevel(sun_map, sun_sampler, uv + turn * POISSON[i * 2] * radius, g, q.z);
    }
    return sum / 8.0;
}

// A stadium's own lamps' light on `world` facing `n` at night (night.rs):
// each lamp's beam, fading to its edge and with the square of how far it
// is, as squarely as it meets the surface, and only where its shadow group
// sees the surface.
fn stadium_light(world: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    if (enh.stadium.y <= 0.0) {
        return vec3<f32>(0.0);
    }
    let seen = vec3<f32>(stadium_seen(world, n, 0u), stadium_seen(world, n, 1u), stadium_seen(world, n, 2u));
    var sum = 0.0;
    for (var i = 0u; i < u32(enh.stadium.x); i++) {
        let lamp = enh.stadium_lamps[i];
        let way = enh.stadium_ways[i];
        let g = u32(floor(way.w / 10.0));
        let edge = way.w - f32(g) * 10.0;
        let to = lamp.xyz - world;
        let d2 = max(dot(to, to), 1.0);
        let l = to * inverseSqrt(d2);
        let full = 1.0 - (1.0 - edge) * 0.35;
        let cone = smoothstep(edge, full, dot(way.xyz, -l));
        // A little wrapped, as the light off the floor and the stands
        // softens where it grazes.
        let facing = clamp((dot(n, l) + 0.1) / 1.1, 0.0, 1.0);
        sum += lamp.w * cone * facing * STADIUM_FALLOFF / (d2 + STADIUM_FALLOFF) * seen[min(g, 2u)];
    }
    return STADIUM_LAMP * sum * enh.stadium.y;
}

// The same, softly, for what stands by the track: here as it is (the baked
// light is already smooth); ray traced (enhanced_rt.wgsl), spread wide.
fn headlight_soft(world: vec3<f32>, n: vec3<f32>, dist: f32) -> vec3<f32> {
    return headlight_at(world, n, dist);
}

// Whether the sun reaches `p`, a point in the air, past what is solid: -1
// here, for not known (the shadow maps are read per pixel); the ray-traced
// scene's (enhanced_rt.wgsl) answers when it is drawn.
fn sun_point(p: vec3<f32>) -> f32 {
    return -1.0;
}

// What a reflection from `origin` along `dir` shows: here only the sky,
// `sky`; the ray-traced scene's (enhanced_rt.wgsl) takes its place when it
// is drawn.
fn reflection(origin: vec3<f32>, dir: vec3<f32>, sky: vec3<f32>) -> vec3<f32> {
    return sky;
}

// How much of the sky's light reaches `world` past the hills round it
// (skylight.rs), 1 all: baked for the ground, and for what stands above it
// more of the sky the higher it is.
fn sky_seen(world: vec3<f32>) -> f32 {
    if (enh.sky_light.z <= 0.0) {
        return 1.0;
    }
    let uv = (vec2<f32>(world.x, -world.z) / enh.sky_light.y + 0.5) / enh.sky_light.x;
    if (any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0))) {
        return 1.0;
    }
    let m = textureSampleLevel(sky_map, flood_sampler, uv, 0.0);
    let ground = enh.sky_height.x + m.g * enh.sky_height.y;
    // Squared: a hollow that hides a fifth of the sky shows it, open
    // ground (nearly all of it) hardly changes.
    let seen = mix(m.r * m.r, 1.0, smoothstep(2.0, 30.0, world.y - ground));
    return mix(1.0, seen, enh.sky_light.z);
}
