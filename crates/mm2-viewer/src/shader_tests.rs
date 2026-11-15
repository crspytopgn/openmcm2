//! Every shader the game makes, put together as the game puts it together
//! and compiled for each kind of graphics card: Metal (Macs), Vulkan and
//! DirectX. The cards compile them only as the game runs, so a shader
//! that only one of them refuses would otherwise first show on a player's
//! screen. The ray-traced ones only for Vulkan, the one place they run.

/// The shaders drawn with the enhanced drawing's shared declarations
/// (crate::enhance::full_source).
const ENHANCED: [(&str, &str); 8] = [
    ("crowd", include_str!("crowd.wgsl")),
    ("dust", include_str!("dust.wgsl")),
    ("grass", include_str!("grass.wgsl")),
    ("objects", include_str!("objects.wgsl")),
    ("rain", include_str!("rain.wgsl")),
    ("terrain", include_str!("terrain.wgsl")),
    ("sky", include_str!("sky.wgsl")),
    ("real sky", include_str!("sky_real.wgsl")),
];

fn module(name: &str, source: &str) -> (naga::Module, naga::valid::ModuleInfo) {
    let module = naga::front::wgsl::parse_str(source).unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(source)));
    let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
        .validate(&module)
        .unwrap_or_else(|e| panic!("{name}: {e:?}"));
    (module, info)
}

fn vulkan(name: &str, module: &naga::Module, info: &naga::valid::ModuleInfo) {
    naga::back::spv::write_vec(module, info, &naga::back::spv::Options::default(), None).unwrap_or_else(|e| panic!("{name} for Vulkan: {e}"));
}

fn metal(name: &str, module: &naga::Module, info: &naga::valid::ModuleInfo) {
    // Metal 2.4: every Mac the game runs on (macOS 12 and later) has it.
    let options = naga::back::msl::Options {
        lang_version: (2, 4),
        ..Default::default()
    };
    naga::back::msl::write_string(module, info, &options, &naga::back::msl::PipelineOptions::default())
        .unwrap_or_else(|e| panic!("{name} for Metal: {e}"));
}

fn directx(name: &str, module: &naga::Module, info: &naga::valid::ModuleInfo) {
    let mut out = String::new();
    let options = naga::back::hlsl::Options::default();
    let pipeline = naga::back::hlsl::PipelineOptions::default();
    naga::back::hlsl::Writer::new(&mut out, &options, &pipeline)
        .write(module, info, None)
        .unwrap_or_else(|e| panic!("{name} for DirectX: {e}"));
}

/// Without ray tracing: for every card.
#[test]
fn the_shaders_compile_for_every_card() {
    let mut shaders: Vec<(String, String)> = ENHANCED.iter().map(|(n, s)| (n.to_string(), crate::enhance::full_source(s, false))).collect();
    // The post pass reads a multisampled depth or a plain one (post.rs).
    for depth in ["texture_depth_2d", "texture_depth_multisampled_2d"] {
        shaders.push((format!("post, {depth}"), include_str!("post.wgsl").replace("DEPTH_TEXTURE", depth)));
    }
    shaders.push(("ruts".into(), include_str!("ruts.wgsl").into()));
    shaders.push(("floodlights' bake".into(), crate::flood::hills_source()));
    for (name, source) in &shaders {
        let (module, info) = module(name, source);
        vulkan(name, &module, &info);
        metal(name, &module, &info);
        directx(name, &module, &info);
    }
}

/// With ray tracing: for Vulkan.
#[test]
fn the_ray_traced_shaders_compile() {
    let mut shaders: Vec<(String, String)> = ENHANCED.iter().map(|(n, s)| (format!("{n}, ray traced"), crate::enhance::full_source(s, true))).collect();
    shaders.push(("shadows".into(), crate::rt::shadow_source()));
    for (name, source) in [
        ("pose", include_str!("rt_pose.wgsl")),
        ("shafts", include_str!("rt_volume.wgsl")),
        ("blend", include_str!("rt_temporal.wgsl")),
        ("blur", include_str!("rt_blur.wgsl")),
    ] {
        shaders.push((name.into(), source.into()));
    }
    for (name, source) in &shaders {
        let (module, info) = module(name, source);
        vulkan(name, &module, &info);
    }
}
