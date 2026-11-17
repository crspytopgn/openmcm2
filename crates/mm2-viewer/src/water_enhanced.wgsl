// What the enhanced drawing adds to a river's water (water.wgsl, put
// after it by water.rs, which renames its plain `mirrored` and `fogged`):
// the sky's own light all round (ibl.rs) in the ripples, blurred as they
// roughen, and the enhanced haze over it.

#include "enhanced.wgsl"

@group(1) @binding(0) var<uniform> enh: Enh;
@group(1) @binding(1) var sun_map: texture_depth_2d_array;
@group(1) @binding(2) var sun_sampler: sampler_comparison;
@group(1) @binding(6) var rut_map: texture_2d<f32>;
@group(1) @binding(7) var rut_sampler: sampler;

#include "enhanced_fns.wgsl"

fn mirrored(r: vec3<f32>, rough: f32) -> vec3<f32> {
    if (enh.ibl.x < 0.5) {
        return plain_mirrored(r, rough);
    }
    // Below the horizon a ripple shows the far bank, darker than the sky.
    let blur = 0.04 + 0.12 * rough;
    let up = vec3<f32>(r.x, max(r.y, 0.02), r.z);
    let sky = sky_reflected(normalize(up), blur);
    // As bright at the horizon as the plain water's sky, which matches
    // the haze the scene fades into; the cube gives its colour and how it
    // changes overhead and round toward the sun.
    let horizon = sky_reflected(normalize(vec3<f32>(r.x, 0.02, r.z)), blur);
    let k = clamp(luminance(plain_mirrored(vec3<f32>(r.x, 0.0, r.z), rough)) / max(luminance(horizon), 1e-3), 0.5, 4.0);
    return sky * k * mix(0.7, 1.0, smoothstep(-0.05, 0.1, r.y));
}

fn fogged(colour: vec3<f32>, alpha: f32, world: vec3<f32>) -> vec4<f32> {
    let dist = length(world - globals.camera.xyz);
    return vec4<f32>(apply_haze(colour, world, dist), alpha * (1.0 - haze_at(dist) * 0.5));
}
