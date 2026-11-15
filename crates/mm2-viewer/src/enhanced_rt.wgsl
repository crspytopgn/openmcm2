// The ray-traced shadows (rt.rs), put in by enhance::ray_traced in place
// of the shadow maps' sun_visible (kept as sun_mapped): traced for each
// pixel before the scene is drawn (rt_shadow.wgsl), and read back here.

// How much of the sun reaches `world`, `dist` ft from the camera: what
// was traced round its place on screen, blended between the four pixels
// nearest it as a texture is filtered, but only those whose surface is
// about as far away, so an edge (a leaf, a rider against the hills, grass
// over the ground) takes its own side's. Blended, not picked, so the
// shadow's edges hold still as the camera moves by a fraction of a pixel.
fn sun_visible(world: vec3<f32>, n: vec3<f32>, dist: f32, lift: f32) -> f32 {
    if (enh.flags.y < 0.5) {
        return 1.0;
    }
    if (enh.traced.x < 0.5) {
        return sun_mapped(world, n, dist, lift);
    }
    let c = enh.view_proj * vec4<f32>(world, 1.0);
    if (c.w <= 0.0) {
        return 1.0;
    }
    let size = vec2<i32>(textureDimensions(rt_shadows));
    let uv = vec2<f32>(c.x / c.w * 0.5 + 0.5, 0.5 - c.y / c.w * 0.5);
    let at = uv * vec2<f32>(size) - 0.5;
    let base = vec2<i32>(floor(at));
    let f = fract(at);
    let tolerance = dist * 0.03 + 0.3;
    var sum = 0.0;
    var weight = 0.0;
    for (var k = 0; k < 4; k++) {
        let o = vec2<i32>(k & 1, k >> 1u);
        let t = textureLoad(rt_shadows, clamp(base + o, vec2<i32>(0), size - 1), 0);
        let w = select(1.0 - f.x, f.x, o.x == 1) * select(1.0 - f.y, f.y, o.y == 1);
        if (abs(t.y - dist) < tolerance) {
            sum += t.x * w;
            weight += w;
        }
    }
    if (weight > 1e-4) {
        return sum / weight;
    }
    // None of them: the pixel round about whose surface is nearest as far.
    let p = clamp(vec2<i32>(uv * vec2<f32>(size)), vec2<i32>(0), size - 1);
    var best = textureLoad(rt_shadows, p, 0);
    var gap = abs(best.y - dist);
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let t = textureLoad(rt_shadows, clamp(p + vec2<i32>(x, y), vec2<i32>(0), size - 1), 0);
            let g = abs(t.y - dist);
            if (g < gap) {
                best = t;
                gap = g;
            }
        }
    }
    return best.x;
}

// ---- Reflections: what a ray meets, from the scene the shadows are
// traced in (rt.rs). ----

// How far a reflection looks, ft.
const RT_REACH: f32 = 6000.0;
// The object vertex words (objects.rs Vertex): position, normal, uv,
// colour, node.
const RT_WORDS: u32 = 10u;
const RT_NO_COLOUR: u32 = 0xffffffffu;

// Whether a cut-out triangle the ray meets is solid where it meets it.
fn rt_solid(c: RayIntersection) -> bool {
    let r = (c.instance_custom_data + c.primitive_index) * 2u;
    let a = rt_cutouts[r];
    let b = rt_cutouts[r + 1u];
    let w = c.barycentrics;
    let uv = a.xy * (1.0 - w.x - w.y) + a.zw * w.x + b.xy * w.y;
    return textureSampleLevel(rt_masks, rt_sampler, uv, i32(b.z), 0.0).r >= 0.5;
}

fn rt_point(i: u32) -> vec3<f32> {
    return vec3<f32>(rt_points[i * 3u], rt_points[i * 3u + 1u], rt_points[i * 3u + 2u]);
}

fn rt_word(i: u32, k: u32) -> f32 {
    return bitcast<f32>(rt_vertices[i * RT_WORDS + k]);
}

// Whether the sun reaches `p`: past solid things only, to be quick.
fn rt_sunlit(p: vec3<f32>) -> f32 {
    var rq: ray_query;
    rayQueryInitialize(&rq, rt_scene, RayDesc(RAY_FLAG_TERMINATE_ON_FIRST_HIT | RAY_FLAG_CULL_NO_OPAQUE, 0xffu, 0.0, RT_REACH, p, enh.to_sun.xyz));
    rayQueryProceed(&rq);
    return select(1.0, 0.0, rayQueryGetCommittedIntersection(&rq).kind != RAY_QUERY_INTERSECTION_NONE);
}

// What a reflection from `origin` along `dir` shows: what it meets, as
// the sun and the sky light it, hazed by how far it is; or `sky` when it
// meets nothing.
fn reflection(origin: vec3<f32>, dir: vec3<f32>, sky: vec3<f32>) -> vec3<f32> {
    if (enh.traced.x < 0.5) {
        return sky;
    }
    var rq: ray_query;
    rayQueryInitialize(&rq, rt_scene, RayDesc(RAY_FLAG_NONE, 0xffu, 0.0, RT_REACH, origin, dir));
    while (rayQueryProceed(&rq)) {
        let c = rayQueryGetCandidateIntersection(&rq);
        if (c.kind == RAY_QUERY_INTERSECTION_TRIANGLE && rt_solid(c)) {
            rayQueryConfirmIntersection(&rq);
        }
    }
    let hit = rayQueryGetCommittedIntersection(&rq);
    if (hit.kind == RAY_QUERY_INTERSECTION_NONE) {
        return sky;
    }
    let p = origin + dir * hit.t;
    let w = hit.barycentrics;
    let weights = vec3<f32>(1.0 - w.x - w.y, w.x, w.y);
    let to_sun = enh.to_sun.xyz;
    let flat_light = enh.ambient.rgb + enh.sun.rgb * max(to_sun.y, 0.0);
    var colour: vec3<f32>;
    if (hit.instance_index == 0u) {
        // The ground: its painted colour, which has the light of a level
        // sun in it, lit again as it faces the sun.
        let k = hit.primitive_index * 3u;
        let a = rt_point(rt_terrain[k]);
        let b = rt_point(rt_terrain[k + 1u]);
        let c = rt_point(rt_terrain[k + 2u]);
        var n = normalize(cross(b - a, c - a));
        if (n.y < 0.0) {
            n = -n;
        }
        let uv = vec2<f32>(p.x, -p.z) / enh.terrain.x / enh.terrain.y;
        let painted = textureSampleLevel(rt_ground, rt_sampler, uv, 0.0).rgb;
        let sun = rt_sunlit(p + n * 0.2);
        let light = (enh.ambient.rgb + enh.sun.rgb * max(dot(n, to_sun), 0.0) * sun) / max(flat_light, vec3<f32>(0.05));
        colour = painted * pow(clamp(light, vec3<f32>(0.0), vec3<f32>(1.6)), vec3<f32>(2.2));
    } else {
        // An object: its texture and vertex colours, lit as the game
        // lights it (objects.wgsl), turned with its part if it moves.
        let head = rt_hits[hit.instance_index / 2u];
        let base = select(select(head.x, head.y, hit.geometry_index == 1u), select(head.z, head.w, hit.geometry_index == 1u), (hit.instance_index & 1u) == 1u);
        let tri = rt_hits[base + hit.primitive_index];
        var n = vec3<f32>(0.0);
        var uv = vec2<f32>(0.0);
        var tint = vec4<f32>(0.0);
        for (var k = 0u; k < 3u; k++) {
            let i = tri[k];
            n += vec3<f32>(rt_word(i, 3u), rt_word(i, 4u), rt_word(i, 5u)) * weights[k];
            uv += vec2<f32>(rt_word(i, 6u), rt_word(i, 7u)) * weights[k];
            tint += unpack4x8unorm(rt_vertices[i * RT_WORDS + 8u]) * weights[k];
        }
        let node = rt_vertices[tri.x * RT_WORDS + 9u];
        if (hit.instance_index == rt_hits[1].w) {
            n = (rt_nodes[node & 0x3fffffffu] * vec4<f32>(n, 0.0)).xyz;
        }
        n = normalize(n + vec3<f32>(0.0, 1e-5, 0.0));
        if (dot(n, dir) > 0.0) {
            n = -n;
        }
        var albedo = tint.rgb;
        if (tri.w != RT_NO_COLOUR) {
            // Blurred by how far: a reflection is seldom sharp.
            let lod = clamp(log2(max(hit.t, 1.0) / 6.0), 0.0, 7.0);
            albedo *= textureSampleLevel(rt_colours, rt_sampler, uv, i32(tri.w), lod).rgb;
        }
        if ((node >> 31u) != 0u) {
            colour = albedo;
        } else {
            let sun = rt_sunlit(p + n * 0.1);
            let game = min(enh.ambient.rgb + enh.sun.rgb * max(dot(n, to_sun), 0.0) * sun, vec3<f32>(1.0));
            colour = albedo * pow(game, vec3<f32>(2.2));
        }
    }
    if (enh.flags.z > 0.5) {
        colour = mix(colour, enh.fog.rgb, haze_amount(hit.t));
    }
    return colour;
}

// Whether the sun reaches `p`, a point in the air (a puff of dust), past
// what is solid: asked of the scene itself, since the traced shadows are
// only of the surfaces on screen.
fn sun_point(p: vec3<f32>) -> f32 {
    if (enh.traced.x < 0.5) {
        return -1.0;
    }
    return rt_sunlit(p);
}

// The lamps' light at `world` softly, for what stands by the track: the mean
// of the traced light over a wide patch round it (5 x 5 pixels 3 apart),
// of those about as far away, so a thin rail's or post's pixels, each traced alone
// and easily lit or shaded by chance, are evened out.
fn headlight_soft(world: vec3<f32>, n: vec3<f32>, dist: f32) -> vec3<f32> {
    if (enh.headlights.x < 0.5) {
        return vec3<f32>(0.0);
    }
    let c = enh.view_proj * vec4<f32>(world, 1.0);
    if (c.w <= 0.0) {
        return vec3<f32>(0.0);
    }
    let size = vec2<i32>(textureDimensions(rt_lamps));
    let uv = vec2<f32>(c.x / c.w * 0.5 + 0.5, 0.5 - c.y / c.w * 0.5);
    let at = vec2<i32>(uv * vec2<f32>(size));
    let tolerance = dist * 0.15 + 1.0;
    var sum = vec3<f32>(0.0);
    var weight = 0.0;
    for (var y = -2; y <= 2; y++) {
        for (var x = -2; x <= 2; x++) {
            let q = clamp(at + vec2<i32>(x, y) * 3, vec2<i32>(0), size - 1);
            if (abs(textureLoad(rt_shadows, q, 0).y - dist) < tolerance) {
                sum += textureLoad(rt_lamps, q, 0).rgb;
                weight += 1.0;
            }
        }
    }
    return sum / max(weight, 1.0);
}

// The bikes' headlights' light at `world` facing `n` (rt_shadow.wgsl
// traced it, shadows and all, for each pixel): read as the shadows are,
// from the pixels round about whose surface is about as far away.
fn headlight_at(world: vec3<f32>, n: vec3<f32>, dist: f32) -> vec3<f32> {
    if (enh.headlights.x < 0.5) {
        return vec3<f32>(0.0);
    }
    let c = enh.view_proj * vec4<f32>(world, 1.0);
    if (c.w <= 0.0) {
        return vec3<f32>(0.0);
    }
    let size = vec2<i32>(textureDimensions(rt_lamps));
    let uv = vec2<f32>(c.x / c.w * 0.5 + 0.5, 0.5 - c.y / c.w * 0.5);
    let at = uv * vec2<f32>(size) - 0.5;
    let base = vec2<i32>(floor(at));
    let f = fract(at);
    let tolerance = dist * 0.03 + 0.3;
    var sum = vec3<f32>(0.0);
    var weight = 0.0;
    for (var k = 0; k < 4; k++) {
        let o = vec2<i32>(k & 1, k >> 1u);
        let q = clamp(base + o, vec2<i32>(0), size - 1);
        let t = textureLoad(rt_lamps, q, 0);
        let w = select(1.0 - f.x, f.x, o.x == 1) * select(1.0 - f.y, f.y, o.y == 1);
        if (abs(textureLoad(rt_shadows, q, 0).y - dist) < tolerance) {
            sum += t.rgb * w;
            weight += w;
        }
    }
    return sum / max(weight, 1e-4);
}
