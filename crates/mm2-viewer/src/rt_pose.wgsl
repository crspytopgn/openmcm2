// The moving models posed for the rays (rt.rs): each of their vertices
// moved by its node, as objects.wgsl's vs_main moves it.

// The objects' vertices (objects.rs Vertex, 10 words: position, normal,
// uv, colour, node).
@group(0) @binding(0) var<storage, read> vertices: array<u32>;
@group(0) @binding(1) var<storage, read> nodes: array<mat4x4<f32>>;
// Which object vertex each posed one is.
@group(0) @binding(2) var<storage, read> sources: array<u32>;
@group(0) @binding(3) var<storage, read_write> posed: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if (i >= arrayLength(&sources)) {
        return;
    }
    let s = sources[i] * 10u;
    let p = vec3<f32>(bitcast<f32>(vertices[s]), bitcast<f32>(vertices[s + 1u]), bitcast<f32>(vertices[s + 2u]));
    let m = nodes[vertices[s + 9u] & 0x3fffffffu];
    let w = (m * vec4<f32>(p, 1.0)).xyz;
    posed[i * 3u] = w.x;
    posed[i * 3u + 1u] = w.y;
    posed[i * 3u + 2u] = w.z;
}
