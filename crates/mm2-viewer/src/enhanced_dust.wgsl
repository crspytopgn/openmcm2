// The dust clouds' shadows (dust.rs), put in by enhance::shader after the
// sun's own: the dust as the sun sees it, how thick it is along each of
// its rays and how deep into it that thickness lies (its mean and spread),
// so what is past a cloud is shaded by it and what is in front is not.

// How much of the sun gets through the dust to `world`, 0 to 1.
fn dust_through(world: vec3<f32>) -> f32 {
    if (enh.dust.x < 0.5) {
        return 1.0;
    }
    let p = enh.dust_view * vec4<f32>(world, 1.0);
    let uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    if (any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0))) {
        return 1.0;
    }
    let m = textureSampleLevel(dust_map, dust_sampler, uv, 0.0);
    if (m.r < 0.01) {
        return 1.0;
    }
    let mean = m.g / m.r;
    // At least a puff's depth, so a lone puff shades its far side softly.
    let spread = sqrt(max(m.b / m.r - mean * mean, 0.0)) + enh.dust.y;
    let past = smoothstep(-1.6, 1.6, (p.z - mean) / spread);
    return exp(-m.r * past);
}

// How much of the sun reaches `world`: past what is solid, then the dust.
fn sun_visible(world: vec3<f32>, n: vec3<f32>, dist: f32, lift: f32) -> f32 {
    return sun_solid(world, n, dist, lift) * dust_through(world);
}
