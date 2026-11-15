// The ray-traced sun shadows (rt.rs): for each pixel of the scene, from
// where the depth pass put the nearest surface, rays toward points on the
// sun's disc. Out: x how much sun reaches it, y how far it is from the
// camera, z how far along the rays what shades it is (0 for nothing).

enable wgpu_ray_query;

struct Params {
    // From the screen (x, y -1 to 1, depth) to the drawing.
    inverse_view_proj: mat4x4<f32>,
    camera: vec4<f32>,
    // The way to the sun, and in w which instances cast (rt.rs).
    to_sun: vec4<f32>,
    // The scene's size in pixels, the frame's number, and a pixel's width
    // a ft away.
    size: vec4<f32>,
    // The frame before's view and projection, and x whether there is
    // one, yzw where its camera was (rt_temporal.wgsl).
    previous: mat4x4<f32>,
    history: vec4<f32>,
    // x how many floodlights light the shadows in place of the sun; y how
    // many headlights, z how much it is night.
    lights: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var depth: texture_depth_2d;
@group(0) @binding(2) var scene: acceleration_structure;
@group(0) @binding(3) var<storage, read> cutouts: array<vec4<f32>>;
@group(0) @binding(4) var masks: texture_2d_array<f32>;
@group(0) @binding(5) var mask_sampler: sampler;
@group(0) @binding(6) var traced: texture_storage_2d<rgba16float, write>;
// What the rays meet (rt.rs Reflections): the instances' parts and each
// triangle's object vertices, the objects' vertices (objects.rs Vertex,
// ten words) and the nodes that move them.
@group(0) @binding(7) var<storage, read> hits: array<vec4<u32>>;
@group(0) @binding(8) var<storage, read> vertices: array<u32>;
@group(0) @binding(9) var<storage, read> nodes: array<mat4x4<f32>>;
// The moving models' posed vertices the frame before (rt.rs Moving), their
// triangles (solid, cut out), and where each pixel on one was on screen
// then: its move in pixels, how far from that frame's camera, and 1.
@group(0) @binding(10) var<storage, read> before: array<f32>;
@group(0) @binding(11) var<storage, read> moving_solid: array<u32>;
@group(0) @binding(12) var<storage, read> moving_cut: array<u32>;
@group(0) @binding(13) var motion: texture_storage_2d<rgba16float, write>;
// A domed stadium's floodlights (rt.rs): (middle, spread), (lamps, ...).
@group(0) @binding(14) var<storage, read> floodlights: array<vec4<f32>>;
// The bikes' headlights: (place, 0), (way, 0); and their light at each
// pixel, for the scene's shaders.
@group(0) @binding(15) var<storage, read> headlights: array<vec4<f32>>;
@group(0) @binding(16) var lamps: texture_storage_2d<rgba16float, write>;
// The frame before's shadows as rt_temporal.wgsl settled them: x the
// light, w how many frames it has gathered there.
@group(0) @binding(17) var settled: texture_2d<f32>;
// Each 2 x 2 block's first pixel traced ahead of the rest (`probe`): x
// its sun's light (-1 where the floodlights light it), y how far it is
// from the camera, z how far off what shades it is, w 1 where it is the
// ground under the poles' baked light (0 off it, 0.5 the sky). Written by
// the probe pass, read by the trace.
// And which lamps (the first 64) it traced and saw (xy), and which it
// traced and found something in the way of (zw).
@group(0) @binding(18) var probe_out: texture_storage_2d<rgba16float, write>;
@group(0) @binding(19) var probes: texture_2d<f32>;
@group(0) @binding(20) var probe_lamps_out: texture_storage_2d<rgba32uint, write>;
@group(0) @binding(21) var probe_lamps: texture_2d<u32>;
// A pixel's shadow has settled with this many frames gathered.
const SETTLED: f32 = 3.0;

// The lamps' beams (LAMP, POLE, `lamp_beam`) are in lamp_light.wgsl,
// shared with the poles' baked light (flood.wgsl).
// The poles' light over the ground, baked (flood.wgsl, whose bindings and
// `ground_height` this shares).
@group(0) @binding(34) var flood_map: texture_2d<f32>;
// How far short of a headlight its shadow rays stop, ft: past the lamp's
// own bike (its number plate stands in front of it).
const OWN_BIKE: f32 = 4.5;

// How far the ray now being traced may go: to the sun, or short of the
// floodlight it goes to (its lamps are on the stadium's own panels).
var<private> ray_reach: f32 = 6000.0;

// How wide the sun looks, radians: the real one's quarter degree, so
// shadows soften the farther they fall and a gantry's lattice still shows.
const SUN_RADIUS: f32 = 0.0047;
// How far a ray looks for something in the way, ft: past the haze.
const RAY_REACH: f32 = 6000.0;
// How far along a ray cut-outs (leaves, fences) stop it, ft, and how far
// from the camera they cast at all: past that they would cast only a
// faint blur, and a forest's leaves are many to look through.
const CUTOUT_REACH: f32 = 300.0;
const CUTOUT_SHOWN: f32 = 250.0;
// The terrain's instance mask (rt.rs TERRAIN), and how far above a
// surface its rays toward the hills start, ft.
const TERRAIN: u32 = 1u;
// A floodlight bank's lamp, ft across, and how much deeper each bank's
// shadow goes than its share of the light.
const BANK_LAMP: f32 = 4.0;
const FAN_DEPTH: f32 = 2.0;
const MOVING: u32 = 4u;
const TERRAIN_LIFT: f32 = 0.5;

struct Ray {
    // How much light it brings from the sky, 0 to 1, and how far along it
    // what shades it is (0 for nothing).
    light: f32,
    shade: f32,
};

// A ray from `origin` along `dir`: none of the light if something solid
// is in the way; through a cut-out what its mask lets by, the mask as
// blurred as the ray's cone is wide where it meets it (`spread` ft at the
// start, widening as the sun's disc does), so foliage far up casts a
// soft dapple.
fn ray_from(origin: vec3<f32>, dir: vec3<f32>, mask: u32, spread: f32, flags: u32, start: f32, through_in: f32) -> Leg {
    // The nearest it meets, not the first found: whether that is a face
    // from behind (see `ray`) must not hang on how the moving models'
    // structure happened to be built this frame, or their shadows flicker.
    var rq: ray_query;
    rayQueryInitialize(&rq, scene, RayDesc(flags, mask, start, ray_reach, origin, dir));
    var through = through_in;
    var first = 0.0;
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
        // About as wide as the sun's disc is there, less a little: the
        // two rays and the blur after soften it the rest of the way.
        let wide = spread * 0.5 + c.t * SUN_RADIUS;
        let lod = log2(max(wide * b.w, 1.0));
        let solid = textureSampleLevel(masks, mask_sampler, uv, i32(b.z), lod).r;
        // Sharp where the mask is sharp, a fraction where it is blurred.
        let cover = select(smoothstep(0.0, 1.0, solid), step(0.5, solid), lod < 0.5);
        if (cover > 0.05 && (first == 0.0 || c.t < first)) {
            first = c.t;
        }
        through *= 1.0 - cover;
        if (through < 0.1) {
            rayQueryConfirmIntersection(&rq);
        }
    }
    var out: Leg;
    out.hit = rayQueryGetCommittedIntersection(&rq);
    out.through = through;
    out.first = first;
    return out;
}

// One stretch of a ray: what stopped it, if anything, how much light got
// through the cut-outs on the way, and how far along the first was.
struct Leg {
    hit: RayIntersection,
    through: f32,
    first: f32,
};

// Which way the object triangle a ray met faces outward, by its vertices'
// normals (turned with its part if it moves): the meshes' windings are
// not to be trusted, the normals the game lights them by are.
fn outward(h: RayIntersection) -> vec3<f32> {
    let head = hits[h.instance_index / 2u];
    let base = select(select(head.x, head.y, h.geometry_index == 1u), select(head.z, head.w, h.geometry_index == 1u), (h.instance_index & 1u) == 1u);
    let tri = hits[base + h.primitive_index];
    let w = vec3<f32>(1.0 - h.barycentrics.x - h.barycentrics.y, h.barycentrics.x, h.barycentrics.y);
    var n = vec3<f32>(0.0);
    for (var k = 0u; k < 3u; k++) {
        let i = tri[k] * 10u;
        n += vec3<f32>(bitcast<f32>(vertices[i + 3u]), bitcast<f32>(vertices[i + 4u]), bitcast<f32>(vertices[i + 5u])) * w[k];
    }
    if (h.instance_index == hits[1].w) {
        n = (nodes[vertices[tri.x * 10u + 9u] & 0x3fffffffu] * vec4<f32>(n, 0.0)).xyz;
    }
    return n;
}

// A ray from `origin` along `dir`. What it meets from behind (facing the
// way the ray goes) is the inside of the very surface it left: a low-poly
// curve whose flat faces stand proud of its smooth lighting where the sun
// grazes it, and would shade it in jagged steps. The ray goes on past
// that, a few times at most.
fn ray(origin: vec3<f32>, dir: vec3<f32>, mask: u32, spread: f32, flags: u32) -> Ray {
    var start = 0.0;
    var through = 1.0;
    var first = 0.0;
    var out = Ray(1.0, 0.0);
    for (var leg = 0u; leg < 4u; leg++) {
        let got = ray_from(origin, dir, mask, spread, flags, start, through);
        through = got.through;
        if (got.first > 0.0 && first == 0.0) {
            first = got.first;
        }
        if (got.hit.kind == RAY_QUERY_INTERSECTION_NONE) {
            out.light = through;
            out.shade = first;
            return out;
        }
        if (got.hit.instance_index != 0u && dot(outward(got.hit), dir) > 0.0) {
            start = got.hit.t + 0.01;
            continue;
        }
        out.light = 0.0;
        out.shade = got.hit.t;
        return out;
    }
    out.light = through;
    out.shade = first;
    return out;
}

// 1 if `world` (`dist` ft from the camera, `to_eye` toward it) is on a
// moving model, 0 if not: a ray from the camera to it, against the moving
// models alone, meets one there. Their frames before are not where the
// camera's own move says, so the blend over time leaves them out.
fn on_moving(world: vec3<f32>, to_eye: vec3<f32>, dist: f32, pixel: vec2<i32>) -> f32 {
    textureStore(motion, pixel, vec4<f32>(0.0));
    let mover = hits[1].w;
    if (mover == 0xffffffffu) {
        return 0.0;
    }
    var rq: ray_query;
    rayQueryInitialize(&rq, scene, RayDesc(RAY_FLAG_FORCE_OPAQUE, MOVING, max(dist - 1.0, 0.0), dist + 1.0, params.camera.xyz, -to_eye / max(dist, 1e-3)));
    rayQueryProceed(&rq);
    let hit = rayQueryGetCommittedIntersection(&rq);
    if (hit.kind == RAY_QUERY_INTERSECTION_NONE) {
        return 0.0;
    }
    // The same place on the triangle, where its vertices were a frame
    // ago, and where that was on screen.
    let w = vec3<f32>(1.0 - hit.barycentrics.x - hit.barycentrics.y, hit.barycentrics.x, hit.barycentrics.y);
    var was = vec3<f32>(0.0);
    for (var k = 0u; k < 3u; k++) {
        let at = hit.primitive_index * 3u + k;
        let i = select(moving_solid[at], moving_cut[at], hit.geometry_index == 1u);
        was += vec3<f32>(before[i * 3u], before[i * 3u + 1u], before[i * 3u + 2u]) * w[k];
    }
    let c = params.previous * vec4<f32>(was, 1.0);
    if (params.history.x > 0.5 && c.w > 0.0) {
        let uv = vec2<f32>(c.x / c.w * 0.5 + 0.5, 0.5 - c.y / c.w * 0.5);
        let then = uv * params.size.xy - (vec2<f32>(pixel) + 0.5);
        textureStore(motion, pixel, vec4<f32>(then, length(params.history.yzw - was), 1.0));
    }
    return 1.0;
}

// What the frames before settled on for the surface at `world` (`dist` ft
// from the camera): x its light, y how many frames it has gathered (0
// for none), z how far off what shades it is, read where it was on screen a frame ago from the pixels
// that saw it, as rt_temporal.wgsl reads it to blend.
fn settled_at(world: vec3<f32>, dist: f32) -> vec3<f32> {
    if (params.history.x < 0.5) {
        return vec3<f32>(0.0);
    }
    let was = params.previous * vec4<f32>(world, 1.0);
    if (was.w <= 0.0) {
        return vec3<f32>(0.0);
    }
    let size = vec2<i32>(params.size.xy);
    let then = length(params.history.yzw - world);
    let at = vec2<f32>(was.x / was.w * 0.5 + 0.5, 0.5 - was.y / was.w * 0.5) * params.size.xy - 0.5;
    let base = vec2<i32>(floor(at));
    let f = at - floor(at);
    var sum = vec3<f32>(0.0);
    var weight = 0.0;
    for (var j = 0; j < 4; j++) {
        let o = vec2<i32>(j & 1, j >> 1u);
        let q = base + o;
        if (any(q < vec2<i32>(0)) || any(q >= size)) {
            continue;
        }
        let old = textureLoad(settled, q, 0);
        if (abs(old.y - then) > 0.04 * dist + 0.4) {
            continue;
        }
        let w = mix(1.0 - f.x, f.x, f32(o.x)) * mix(1.0 - f.y, f.y, f32(o.y));
        sum += old.xwz * w;
        weight += w;
    }
    // Most of it must have seen this surface, or the blend will not
    // trust it either.
    if (weight < 0.5) {
        return vec3<f32>(0.0);
    }
    return sum / weight;
}

// The way out of the surface at pixel `p` (`world`), from its neighbours'
// depths: the nearer on each side, so an edge does not bend it.
fn surface_normal(p: vec2<i32>, world: vec3<f32>) -> vec3<f32> {
    let size = vec2<i32>(params.size.xy);
    var pts: array<vec3<f32>, 4>;
    let offs = array<vec2<i32>, 4>(vec2<i32>(-1, 0), vec2<i32>(1, 0), vec2<i32>(0, -1), vec2<i32>(0, 1));
    for (var k = 0; k < 4; k++) {
        let q = clamp(p + offs[k], vec2<i32>(0), size - 1);
        let d = textureLoad(depth, q, 0);
        let ndc = vec2<f32>((f32(q.x) + 0.5) / params.size.x * 2.0 - 1.0, 1.0 - (f32(q.y) + 0.5) / params.size.y * 2.0);
        let h = params.inverse_view_proj * vec4<f32>(ndc, d, 1.0);
        pts[k] = h.xyz / h.w;
    }
    let dx = select(world - pts[0], pts[1] - world, length(pts[1] - world) < length(world - pts[0]));
    let dy = select(world - pts[2], pts[3] - world, length(pts[3] - world) < length(world - pts[2]));
    var n = normalize(cross(dx, dy) + vec3<f32>(0.0, 1e-6, 0.0));
    if (dot(n, params.camera.xyz - world) < 0.0) {
        n = -n;
    }
    return n;
}

// The bikes' headlights' light reaching `world` (pixel `p`, `dist` ft
// from the camera): each spot's beam, falling off with the square of how
// far, as squarely as it meets the surface, where nothing stands between.
// A lamp the blocks round it all saw (`seen`) is taken as reaching it, one
// they all found blocked as blocked; only the rest are traced.
// `kind` says what the blocks round it were: 0 all off the ground, 1 all
// ground, 2 some of each.
fn headlit(world: vec3<f32>, p: vec2<i32>, dist: f32, seen_by: vec2<u32>, shut_by: vec2<u32>, kind: u32) -> Lit {
    var out = Lit(vec3<f32>(0.0), vec2<u32>(0u), vec2<u32>(0u), false);
    let count = u32(params.lights.y);
    if (count == 0u) {
        return out;
    }
    let n = surface_normal(p, world);
    let off = 0.04 + dist * 0.0006;
    // On the ground the poles' light is baked, shadows and all; only the
    // riders' shadows in it are traced here.
    let baked = baked_ground(world, n, dist);
    let on_ground = baked.w > 0.5;
    out.ground = on_ground;
    // The blocks' answers only from blocks of the same kind: on the ground
    // they looked only for riders, elsewhere for everything.
    let alike = select(kind == 0u, kind == 1u, on_ground);
    let seen = select(vec2<u32>(0u), seen_by, alike);
    let shut = select(vec2<u32>(0u), shut_by, alike);
    var unhidden = vec3<f32>(0.0);
    var hidden = vec3<f32>(0.0);
    // The four strongest poles, kept by insertion; the faint left out.
    var best = array<u32, 4>(0u, 0u, 0u, 0u);
    var strength = array<f32, 4>(0.0, 0.0, 0.0, 0.0);
    // Only the lamps that reach this pixel's tile (`cull_lamps`).
    let listed = atomicLoad(&tile_count);
    for (var t = 0u; t < listed; t++) {
        let j = tile_lamps[t];
        if (headlights[j * 2u].w < 0.5) {
            continue;
        }
        var w = lamp_light(j, world, n).w;
        var at_j = j;
        for (var k = 0u; k < 4u; k++) {
            if (w > strength[k]) {
                let ow = strength[k];
                let oj = best[k];
                strength[k] = w;
                best[k] = at_j;
                w = ow;
                at_j = oj;
            }
        }
    }
    // Then every bike's headlight (there are few, and they matter most)
    // and those poles, each traced in this one place: a ray query written
    // out more than once makes the shader too big to run many at a time.
    var sum = vec3<f32>(0.0);
    for (var i = 0u; i < listed + 4u; i++) {
        var j = 0u;
        var short = OWN_BIKE;
        if (i < listed) {
            j = tile_lamps[i];
            if (headlights[j * 2u].w > 0.5) {
                continue;
            }
        } else {
            if (strength[i - listed] <= 0.0) {
                break;
            }
            j = best[i - listed];
            short = 3.0;
        }
        let c = lamp_light(j, world, n);
        if (c.w <= 0.0) {
            continue;
        }
        // A pole over the ground: its light is baked, only a rider in the
        // way of it is looked for.
        let riders_only = on_ground && i >= listed;
        // As the blocks round it all found, or traced (and noted).
        let bit = 1u << (j & 31u);
        let word = min(j >> 5u, 1u);
        let noted = j < 64u;
        var clear = noted && (seen[word] & bit) != 0u;
        if (!clear && !(noted && (shut[word] & bit) != 0u)) {
            clear = lamp_clear(world, n, off, headlights[j * 2u].xyz, short, riders_only);
            if (noted) {
                if (clear) {
                    out.seen[word] |= bit;
                } else {
                    out.shut[word] |= bit;
                }
            }
        }
        if (riders_only) {
            unhidden += c.rgb;
            if (!clear) {
                hidden += c.rgb;
            }
            continue;
        }
        if (clear) {
            sum += c.rgb;
        }
    }
    if (on_ground) {
        // A rider's shadow takes away that pole's share, as much of it as
        // the baked light holds (what else hides the pole is in it).
        let held = clamp(dot(baked.rgb, vec3<f32>(1.0)) / max(dot(unhidden, vec3<f32>(1.0)), 1e-4), 0.0, 1.0);
        sum += max(baked.rgb - hidden * held, vec3<f32>(0.0));
    }
    out.light = min(sum, vec3<f32>(LAMP_MOST)) * params.lights.z;
    return out;
}

// Lamp `j`'s light at `world` (facing `n`) were nothing in the way, and in
// w how strong it is: 0 where it does not reach or is too faint to trace.
fn lamp_light(j: u32, world: vec3<f32>, n: vec3<f32>) -> vec4<f32> {
    let at = headlights[j * 2u].xyz;
    let beam = lamp_beam(at, headlights[j * 2u + 1u].xyz, headlights[j * 2u].w > 0.5, world);
    let facing = dot(n, normalize(at - world));
    if (facing <= 0.0) {
        return vec4<f32>(0.0);
    }
    let c = beam * facing;
    let w = max(c.r, max(c.g, c.b));
    return vec4<f32>(c, select(w, 0.0, w < LAMP_FAINT));
}

// Where `world` (facing `n`, `dist` ft away) is the ground under the
// poles' baked light: that light (rgb) and w 1; else w 0. Off the map no
// pole reaches, so the ground there has none.
fn baked_ground(world: vec3<f32>, n: vec3<f32>, dist: f32) -> vec4<f32> {
    if (flood.corner.w < 0.5 || n.y < 0.6) {
        return vec4<f32>(0.0);
    }
    if (abs(world.y - ground_height(world.x, world.z)) > 0.4 + dist * 0.003) {
        return vec4<f32>(0.0);
    }
    let uv = (world.xz - flood.corner.xy) / (flood.size.xy * flood.corner.z);
    if (any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0))) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    return vec4<f32>(textureSampleLevel(flood_map, mask_sampler, uv, 0.0).rgb, 1.0);
}

// The lamps' light at a place, and which of the first 64 were traced and
// reached it (`seen`) or were blocked (`shut`).
struct Lit {
    light: vec3<f32>,
    seen: vec2<u32>,
    shut: vec2<u32>,
    // Whether it was the ground under the poles' baked light.
    ground: bool,
};

// Whether nothing stands between `world` (facing `n`, `off` ft off it)
// and the lamp at `at`, the ray stopping `short` ft short of it (past a
// headlight's own bike, a pole's head): the objects from the surface, the
// hills from half a foot above it, as the sun's rays do (the smooth ground
// near the eye lies a few inches off the hills the rays see).
// With `riders_only`, only the moving models are looked for (all else is
// in the poles' baked light): one place a ray is traced from, as a ray
// query written out twice makes the shader too big to run many at once.
fn lamp_clear(world: vec3<f32>, n: vec3<f32>, off: f32, at: vec3<f32>, short: f32, riders_only: bool) -> bool {
    // Off the surface toward the camera and the lamp, as the sun's rays
    // start: the side the camera sees is always outside it. (Off along `n`,
    // which the depths round a pixel give, a post or trunk a few pixels
    // wide started rays inside itself, in black specks that flickered.)
    let start = world + normalize(params.camera.xyz - world) * off + normalize(at - world) * off;
    let d = length(at - start);
    var rq: ray_query;
    rayQueryInitialize(&rq, scene, RayDesc(RAY_FLAG_TERMINATE_ON_FIRST_HIT | RAY_FLAG_CULL_NO_OPAQUE, select(0xffu & ~TERRAIN, MOVING, riders_only), 0.0, max(d - short, 0.0), start, (at - start) / d));
    rayQueryProceed(&rq);
    if (rayQueryGetCommittedIntersection(&rq).kind != RAY_QUERY_INTERSECTION_NONE) {
        return false;
    }
    if (riders_only) {
        return true;
    }
    let lifted = start + vec3<f32>(0.0, TERRAIN_LIFT, 0.0);
    let h = length(at - lifted);
    var hq: ray_query;
    rayQueryInitialize(&hq, scene, RayDesc(RAY_FLAG_TERMINATE_ON_FIRST_HIT | RAY_FLAG_CULL_NO_OPAQUE, TERRAIN, 0.0, max(h - short, 0.0), lifted, (at - lifted) / h));
    rayQueryProceed(&hq);
    return rayQueryGetCommittedIntersection(&hq).kind == RAY_QUERY_INTERSECTION_NONE;
}

// How much a bank of `lamps` lamps `d` away counts: by its lamps and the
// square of how far, and that squared again, so the nearest bright bank
// casts a clear shadow and the others fainter ones.
fn bank_weight(lamps: f32, d: vec3<f32>) -> f32 {
    return lamps / max(dot(d, d), 1.0) * 1e5;
}

// How much of a domed stadium's floodlight reaches `origin`: one ray to
// each of the four banks that light it most, at a place across each turned
// by `turn`, their answers weighed by how strongly each lights it. The same
// banks every frame, so no choosing among them shimmers. Its x the light,
// y how far along the rays what shades it is (0 for nothing).
fn floodlit(origin: vec3<f32>, mask: u32, spread: f32, flags: u32, turn: f32) -> Ray {
    let count = u32(params.lights.x);
    // The four strongest, by insertion.
    var best = array<u32, 4>(0u, 0u, 0u, 0u);
    var weight = array<f32, 4>(0.0, 0.0, 0.0, 0.0);
    for (var j = 0u; j < count; j++) {
        var w = bank_weight(floodlights[j * 2u + 1u].x, floodlights[j * 2u].xyz - origin);
        var at = j;
        for (var k = 0u; k < 4u; k++) {
            if (w > weight[k]) {
                let ow = weight[k];
                let oa = best[k];
                weight[k] = w;
                best[k] = at;
                w = ow;
                at = oa;
            }
        }
    }
    var lit = 0.0;
    var total = 0.0;
    var shade = 0.0;
    var shaded = 0.0;
    for (var k = 0u; k < 4u; k++) {
        if (weight[k] <= 0.0) {
            continue;
        }
        let l = floodlights[best[k] * 2u];
        let to = normalize(l.xyz - origin);
        let side = normalize(cross(to, select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(to.y) > 0.99)));
        let up = cross(side, to);
        let a = turn + f32(k) * 2.3999632;
        // As a small bright lamp, not the whole panel the bank is painted
        // on, so its shadow is a clear one.
        let aim = l.xyz + (side * cos(a) + up * sin(a)) * BANK_LAMP;
        let path = aim - origin;
        // Short of the lamps and what holds them.
        ray_reach = max(length(path) - 30.0 - l.w, 1.0);
        let dir = normalize(path);
        let things = ray(origin, dir, mask & ~TERRAIN, spread, flags);
        var hills = Ray(1.0, 0.0);
        if ((mask & TERRAIN) != 0u && things.light > 0.0) {
            hills = ray(origin + vec3<f32>(0.0, TERRAIN_LIFT, 0.0), dir, TERRAIN, spread, RAY_FLAG_CULL_NO_OPAQUE);
        }
        lit += things.light * hills.light * weight[k];
        total += weight[k];
        let first = select(hills.shade, things.shade, things.shade > 0.0);
        if (first > 0.0) {
            shade += first;
            shaded += 1.0;
        }
    }
    ray_reach = RAY_REACH;
    // Each bank's share of the shade made deeper, so that one lamp's
    // shadow among four shows clearly and two together go dark: an arena's
    // fan of shadows.
    let blocked = 1.0 - lit / max(total, 1e-20);
    return Ray(clamp(1.0 - blocked * FAN_DEPTH, 0.0, 1.0), shade / max(shaded, 1.0));
}

// Interleaved gradient noise (Jimenez 2014): a different turn for each
// pixel that the blur after evens out.
fn noise(p: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(p, vec2<f32>(0.06711056, 0.00583715))));
}

// What the four probed blocks round pixel `p` (`dist` ft away) saw:
// whether all four are of its surface, the least and most sun they had,
// how far off what shaded them was, and whether all saw every lamp.
struct Near {
    same: bool,
    low: f32,
    high: f32,
    shade: f32,
    // The lamps all four saw, and all four found blocked.
    seen: vec2<u32>,
    shut: vec2<u32>,
    // 0 all four off the ground, 1 all on it, 2 some of each.
    kind: u32,
};

fn probes_near(p: vec2<i32>, dist: f32) -> Near {
    let last = vec2<i32>(textureDimensions(probes)) - 1;
    let q = p / 2;
    var out = Near(true, 2.0, -2.0, 0.0, vec2<u32>(0xffffffffu), vec2<u32>(0xffffffffu), 2u);
    var shaded = 0.0;
    var grounds = 0u;
    var others = 0u;
    for (var j = 0; j < 4; j++) {
        // The blocks either side of the pixel, that it lies between.
        let at = min(q + vec2<i32>(j & 1, j >> 1u), last);
        let s = textureLoad(probes, at, 0);
        if (abs(s.y - dist) > 0.04 * dist + 0.4) {
            out.same = false;
        }
        out.low = min(out.low, s.x);
        out.high = max(out.high, s.x);
        if (s.z > 0.0) {
            out.shade += s.z;
            shaded += 1.0;
        }
        let l = textureLoad(probe_lamps, at, 0);
        out.seen &= l.xy;
        out.shut &= l.zw;
        grounds += u32(s.w > 0.75);
        others += u32(s.w < 0.25);
    }
    if (grounds == 4u) {
        out.kind = 1u;
    } else if (others == 4u) {
        out.kind = 0u;
    }
    out.shade /= max(shaded, 1.0);
    if (!out.same) {
        out.seen = vec2<u32>(0u);
        out.shut = vec2<u32>(0u);
    }
    return out;
}

// The first pixel of each 2 x 2 block, traced ahead (as OpenMM2 shares its
// rays in a block): one ray to the sun, and the lamps lighting it. Where
// the four blocks round a pixel agree (all lit, all shaded, every lamp
// seen) and its own frames before say the same, the trace after takes
// their answer and traces nothing there; anywhere they differ (a shadow's
// edge, a dapple) it traces its own, so the edges stay as sharp.
fn probe_pixel(id: vec3<u32>) {
    let half = vec2<u32>(textureDimensions(probe_out));
    if (id.x >= half.x || id.y >= half.y) {
        return;
    }
    let p = vec2<i32>(id.xy * 2u);
    let d = textureLoad(depth, p, 0);
    if (d >= 1.0) {
        textureStore(probe_out, vec2<i32>(id.xy), vec4<f32>(1.0, 60000.0, 0.0, 0.5));
        textureStore(probe_lamps_out, vec2<i32>(id.xy), vec4<u32>(0u));
        return;
    }
    let ndc = vec2<f32>(
        (f32(p.x) + 0.5) / params.size.x * 2.0 - 1.0,
        1.0 - (f32(p.y) + 0.5) / params.size.y * 2.0,
    );
    let h = params.inverse_view_proj * vec4<f32>(ndc, d, 1.0);
    let world = h.xyz / h.w;
    let to_eye = params.camera.xyz - world;
    let dist = length(to_eye);
    let lit = headlit(world, p, dist, vec2<u32>(0u), vec2<u32>(0u), 2u);
    let kind = select(0.0, 1.0, lit.ground);
    textureStore(probe_lamps_out, vec2<i32>(id.xy), vec4<u32>(lit.seen, lit.shut));
    // Where the floodlights light it, or its shadow has settled on a
    // dapple of leaves (the pixels round it trace their own), no sun ray.
    let old = settled_at(world, dist);
    if (params.lights.x > 0.0 || (old.y >= SETTLED && old.x > 0.03 && old.x < 0.97)) {
        textureStore(probe_out, vec2<i32>(id.xy), vec4<f32>(-1.0, dist, 0.0, kind));
        return;
    }
    let to_sun = params.to_sun.xyz;
    let off = 0.04 + dist * 0.0006;
    let origin = world + to_sun * off + to_eye / max(dist, 1e-3) * off;
    let mask = u32(params.to_sun.w);
    let spread = dist * params.size.w;
    let flags = select(0u, RAY_FLAG_CULL_NO_OPAQUE, dist > CUTOUT_SHOWN);
    let side = normalize(cross(to_sun, select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(to_sun.y) > 0.99)));
    let up = cross(side, to_sun);
    let a = fract(noise(vec2<f32>(p)) + params.size.z * 0.618034) * 6.2831853;
    let dir = normalize(to_sun + (side * cos(a) + up * sin(a)) * 0.7 * SUN_RADIUS);
    let things = ray(origin, dir, mask & ~TERRAIN, spread, flags);
    var hills = Ray(1.0, 0.0);
    if ((mask & TERRAIN) != 0u && things.light > 0.0) {
        hills = ray(origin + vec3<f32>(0.0, TERRAIN_LIFT, 0.0), dir, TERRAIN, spread, RAY_FLAG_CULL_NO_OPAQUE);
    }
    let first = select(hills.shade, things.shade, things.shade > 0.0);
    textureStore(probe_out, vec2<i32>(id.xy), vec4<f32>(things.light * hills.light, dist, first, kind));
}

fn trace_pixel(id: vec3<u32>) {
    let size = vec2<u32>(params.size.xy);
    if (id.x >= size.x || id.y >= size.y) {
        return;
    }
    let d = textureLoad(depth, vec2<i32>(id.xy), 0);
    if (d >= 1.0) {
        textureStore(traced, vec2<i32>(id.xy), vec4<f32>(1.0, 60000.0, 0.0, 0.0));
        textureStore(motion, vec2<i32>(id.xy), vec4<f32>(0.0));
        textureStore(lamps, vec2<i32>(id.xy), vec4<f32>(0.0));
        return;
    }
    let ndc = vec2<f32>(
        (f32(id.x) + 0.5) / params.size.x * 2.0 - 1.0,
        1.0 - (f32(id.y) + 0.5) / params.size.y * 2.0,
    );
    let h = params.inverse_view_proj * vec4<f32>(ndc, d, 1.0);
    let world = h.xyz / h.w;
    let to_eye = params.camera.xyz - world;
    let dist = length(to_eye);
    let to_sun = params.to_sun.xyz;
    // Off the surface toward the sun and the camera, farther the farther
    // away: the depth's precision and the triangles' own flatness.
    let off = 0.04 + dist * 0.0006;
    let origin = world + to_sun * off + to_eye / max(dist, 1e-3) * off;
    let mask = u32(params.to_sun.w);
    let spread = dist * params.size.w;
    let flags = select(0u, RAY_FLAG_CULL_NO_OPAQUE, dist > CUTOUT_SHOWN);
    // Two points on the sun's disc at opposite sides, turned for each
    // pixel and frame; where they disagree (a shadow's edge), six more on
    // a spiral, so the soft edges are smooth and the rest costs two rays
    // (as OpenMM2's ray traced sun shadows do).
    let side = normalize(cross(to_sun, select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(to_sun.y) > 0.99)));
    let up = cross(side, to_sun);
    // Their turn: a fixed one for the pixel, moved on by the golden ratio
    // each frame, so the frames' rays cover the sun's disc evenly and their
    // blend settles fast.
    let turn = fract(noise(vec2<f32>(id.xy)) + params.size.z * 0.618034) * 6.2831853;
    let near = probes_near(vec2<i32>(id.xy), dist);
    textureStore(lamps, vec2<i32>(id.xy), vec4<f32>(headlit(world, vec2<i32>(id.xy), dist, near.seen, near.shut, near.kind).light, 0.0));
    if (params.lights.x > 0.0) {
        // Each pixel's points across the banks held still from frame to
        // frame: the banks are wide, their soft edges wider than the blur
        // evens out, and points that move each frame shimmer.
        let got = floodlit(origin, mask, spread, flags, noise(vec2<f32>(id.xy)) * 6.2831853);
        textureStore(traced, vec2<i32>(id.xy), vec4<f32>(got.light, dist, got.shade, on_moving(world, to_eye, dist, vec2<i32>(id.xy))));
        return;
    }
    var light = 0.0;
    var shade = 0.0;
    var shaded = 0.0;
    var rays = 2u;
    var first_light = -1.0;
    // How many rays: where the frames before have settled (rt_temporal's
    // count of them) on all lit or all shaded, one, to see it is still
    // so; where they have settled on a soft edge or a dapple of leaves,
    // the two, each frame's adding to the many gathered; and where the
    // picture is new or has just changed, all eight at once.
    let moving = on_moving(world, to_eye, dist, vec2<i32>(id.xy));
    let old = settled_at(world, dist);
    let known = moving < 0.5 && old.y >= SETTLED;
    let even = known && (old.x < 0.03 || old.x > 0.97);
    let dappled = known && !even;
    // Settled all lit or all shaded, and the blocks round it see the same
    // this frame: their answer, no rays.
    if (even && near.same && near.high - near.low < 0.03 && abs(near.low - old.x) < 0.25) {
        textureStore(traced, vec2<i32>(id.xy), vec4<f32>((near.low + near.high) * 0.5, dist, near.shade, moving));
        return;
    }
    // (Every pixel is answered from this frame's rays, its own or its
    // blocks': left out every other frame, in tiles or alone, a shadow
    // moving onto it, a rider's, came a frame late there, in a checkerboard.)
    for (var i = 0u; i < rays; i++) {
        var a = turn + f32(i) * 3.14159265;
        var r = 0.7 * SUN_RADIUS;
        if (i >= 2u) {
            let k = f32(i - 2u);
            a = turn + 1.5707963 + k * 2.3999632;
            r = sqrt((k + 0.5) / 6.0) * SUN_RADIUS;
        }
        let dir = normalize(to_sun + (side * cos(a) + up * sin(a)) * r);
        // The objects from the surface itself; the hills from a little
        // above it, as the hills drawn near the eye are smoothed a few
        // inches off the ones the rays see, and would shade themselves.
        let things = ray(origin, dir, mask & ~TERRAIN, spread, flags);
        var hills = Ray(1.0, 0.0);
        if ((mask & TERRAIN) != 0u && things.light > 0.0) {
            hills = ray(origin + vec3<f32>(0.0, TERRAIN_LIFT, 0.0), dir, TERRAIN, spread, RAY_FLAG_CULL_NO_OPAQUE);
        }
        let got = things.light * hills.light;
        light += got;
        let first = select(hills.shade, things.shade, things.shade > 0.0);
        if (first > 0.0) {
            shade += first;
            shaded += 1.0;
        }
        if (i == 0u) {
            first_light = got;
        }
        // Settled even, and this ray says the same: it is enough. Settled
        // on a dapple, one a frame too: the blend gathers the frames' rays
        // (each frame's at another place on the sun's disc).
        if (i == 0u && ((even && abs(got - old.x) < 0.25) || dappled)) {
            rays = 1u;
        }
        if (i == 1u && abs(got - first_light) > 0.25 && !dappled) {
            rays = 8u;
        }
    }
    textureStore(traced, vec2<i32>(id.xy), vec4<f32>(light / f32(rays), dist, shade / max(shaded, 1.0), moving));
}

// The lamps that can reach a workgroup's tile of pixels, found together
// before its pixels light themselves: the box round the tile's surfaces,
// and each lamp whose reach meets it. Each pixel then looks through these
// few, not every lamp on the track (a National's poles are many, and far
// most are out of reach).
var<workgroup> tile_low: array<atomic<u32>, 3>;
var<workgroup> tile_high: array<atomic<u32>, 3>;
var<workgroup> tile_count: atomic<u32>;
var<workgroup> tile_lamps: array<u32, 80>;

// A float as a u32 that sorts the same way, for the atomics; and back.
fn sortable(f: f32) -> u32 {
    let b = bitcast<u32>(f);
    return select(b | 0x80000000u, ~b, (b & 0x80000000u) != 0u);
}

fn unsortable(k: u32) -> f32 {
    return bitcast<f32>(select(~k, k & 0x7fffffffu, (k & 0x80000000u) != 0u));
}

// The surface at pixel `p`, and w 1 if there is one (not the sky, not off
// the edge).
fn surface_at(p: vec2<u32>) -> vec4<f32> {
    let size = vec2<u32>(params.size.xy);
    if (p.x >= size.x || p.y >= size.y) {
        return vec4<f32>(0.0);
    }
    let d = textureLoad(depth, vec2<i32>(p), 0);
    if (d >= 1.0) {
        return vec4<f32>(0.0);
    }
    let ndc = vec2<f32>((f32(p.x) + 0.5) / params.size.x * 2.0 - 1.0, 1.0 - (f32(p.y) + 0.5) / params.size.y * 2.0);
    let h = params.inverse_view_proj * vec4<f32>(ndc, d, 1.0);
    return vec4<f32>(h.xyz / h.w, 1.0);
}

// Every invocation of the workgroup calls this, with its pixel's surface.
fn cull_lamps(at: vec4<f32>, index: u32) {
    if (index == 0u) {
        for (var k = 0u; k < 3u; k++) {
            atomicStore(&tile_low[k], 0xffffffffu);
            atomicStore(&tile_high[k], 0u);
        }
        atomicStore(&tile_count, 0u);
    }
    workgroupBarrier();
    if (at.w > 0.5) {
        for (var k = 0u; k < 3u; k++) {
            atomicMin(&tile_low[k], sortable(at[k]));
            atomicMax(&tile_high[k], sortable(at[k]));
        }
    }
    workgroupBarrier();
    let count = min(u32(params.lights.y), 80u);
    let low = vec3<f32>(unsortable(atomicLoad(&tile_low[0])), unsortable(atomicLoad(&tile_low[1])), unsortable(atomicLoad(&tile_low[2])));
    let high = vec3<f32>(unsortable(atomicLoad(&tile_high[0])), unsortable(atomicLoad(&tile_high[1])), unsortable(atomicLoad(&tile_high[2])));
    if (all(low <= high)) {
        for (var j = index; j < count; j += 64u) {
            let lamp = headlights[j * 2u];
            let reach = select(LAMP_REACH, POLE_REACH, lamp.w > 0.5);
            if (length(lamp.xyz - clamp(lamp.xyz, low, high)) <= reach) {
                tile_lamps[atomicAdd(&tile_count, 1u)] = j;
            }
        }
    }
    workgroupBarrier();
}

@compute @workgroup_size(8, 8)
fn probe(@builtin(global_invocation_id) id: vec3<u32>, @builtin(local_invocation_index) index: u32) {
    let half = vec2<u32>(textureDimensions(probe_out));
    var at = vec4<f32>(0.0);
    if (id.x < half.x && id.y < half.y) {
        at = surface_at(id.xy * 2u);
    }
    cull_lamps(at, index);
    probe_pixel(id);
}

@compute @workgroup_size(8, 8)
fn trace(@builtin(global_invocation_id) id: vec3<u32>, @builtin(local_invocation_index) index: u32) {
    cull_lamps(surface_at(id.xy), index);
    trace_pixel(id);
}
