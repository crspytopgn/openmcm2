// How much of a pole's bank of lamps at `at` the ground at `world`
// (facing `n`) sees, traced (flood.wgsl, after rt_shadow.wgsl): rays to
// points across the bank, turned by `turn`, past everything that does not
// move (the riders' shadows are traced each frame instead), leaves letting
// light through as the sun's rays do; the hills from half a foot up, as
// the sun's rays see them.
fn flood_visible(world: vec3<f32>, n: vec3<f32>, at: vec3<f32>, turn: f32) -> f32 {
    let start = world + n * 0.1;
    let to = normalize(at - start);
    let side = normalize(cross(to, select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(to.y) > 0.99)));
    let up = cross(side, to);
    let count = u32(flood.grid.w);
    var sum = 0.0;
    for (var k = 0u; k < count; k++) {
        let a = turn + f32(k) * 2.3999632;
        let r = sqrt((f32(k) + 0.5) / f32(count)) * FLOOD_BANK;
        let aim = at + (side * cos(a) + up * sin(a)) * r;
        let path = aim - start;
        let d = length(path);
        let dir = path / d;
        ray_reach = max(d - FLOOD_SHORT, 0.5);
        let things = ray(start, dir, 0xffu & ~(TERRAIN | MOVING), 0.0, 0u);
        var hills = 1.0;
        if (things.light > 0.0) {
            let lifted = start + vec3<f32>(0.0, TERRAIN_LIFT, 0.0);
            let lp = aim - lifted;
            let ld = length(lp);
            ray_reach = max(ld - FLOOD_SHORT, 0.5);
            hills = ray(lifted, lp / ld, TERRAIN, 0.0, RAY_FLAG_CULL_NO_OPAQUE).light;
        }
        sum += things.light * hills;
    }
    ray_reach = RAY_REACH;
    return sum / f32(max(count, 1u));
}
