// This should be identical to the Buffer0 struct in model.wgsl.
struct VertexInput0 {
    [[location(0)]] position0: vec4<f32>;
    [[location(1)]] normal0: vec4<f32>;
    [[location(2)]] tangent0: vec4<f32>;
};

struct Vertices {
  vertices: array<VertexInput0>;
};

[[group(0), binding(0)]] var<storage, read> src : Vertices;
[[group(0), binding(1)]] var<storage, read_write> dst : Vertices;

[[stage(compute), workgroup_size(256)]]
fn main([[builtin(global_invocation_id)]] global_invocation_id: vec3<u32>) {
    let total = arrayLength(&src.vertices);
    let index = global_invocation_id.x;
    if (index >= total) {
        return;
    }

    // TODO: Transform each vertex by the parent transform
    // TODO: Transform each vertex by the animation transform
    // TODO: sync to make sure writes happen?
    // TODO: Recalculate normals and tangents for renormal?
    dst.vertices[index] = src.vertices[index];
}