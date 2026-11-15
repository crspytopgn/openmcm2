// How much of a pole's lamps at `at` the ground at `world` sees, without
// ray tracing (flood.wgsl): stepping along the way to it over the
// terrain's heights, softly dimmed where the hills come close to the line
// and dark where they cross it. What stands on the ground casts nothing
// here; the hills do.
fn flood_visible(world: vec3<f32>, n: vec3<f32>, at: vec3<f32>, turn: f32) -> f32 {
    let start = world + vec3<f32>(0.0, 0.5, 0.0);
    let path = at - start;
    let d = length(path);
    let steps = clamp(u32(d / flood.grid.y), 4u, 160u);
    var clear = 1.0;
    for (var i = 1u; i < steps; i++) {
        let t = f32(i) / f32(steps);
        if (t * d > d - FLOOD_SHORT) {
            break;
        }
        let p = start + path * t;
        // How far above the hills the line runs there, against how wide
        // the bank looks from here at that distance.
        let above = p.y - ground_height(p.x, p.z);
        let soft = FLOOD_BANK * t + 0.5;
        clear = min(clear, smoothstep(-soft, soft, above));
        if (clear <= 0.0) {
            break;
        }
    }
    return clear;
}
