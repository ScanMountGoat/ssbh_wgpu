// This should be identical to the Buffer0 struct in model.wgsl.
struct VertexInput0 {
    position0: vec4<f32>;
    normal0: vec4<f32>;
    tangent0: vec4<f32>;
};

struct VertexWeight {
    bone_indices: vec4<i32>;
    weights: vec4<f32>;
};

struct Vertices {
    vertices: array<VertexInput0>;
};

struct VertexWeights {
    vertices: array<VertexWeight>;
};

// The in game buffer is vec4[4096] with the first vec4 containing a u32 bone count.
// This allows at most 511 bones with 2 matrices per bone.
// TODO: What two matrices are stored per bone?
// Remove the length field to improve compatibility.
// This gives a more generous alignment without exceeding 65536 bytes.
struct Transforms {
    transforms: array<mat4x4<f32>, 512>;
    transforms_inv_transpose: array<mat4x4<f32>, 512>;
};

[[group(0), binding(0)]] var<storage, read> src : Vertices;
[[group(0), binding(1)]] var<storage, read> vertex_weights : VertexWeights;
[[group(0), binding(2)]] var<storage, read_write> dst : Vertices;
[[group(0), binding(3)]] var<uniform> transforms: Transforms;

[[stage(compute), workgroup_size(256)]]
fn main([[builtin(global_invocation_id)]] global_invocation_id: vec3<u32>) {
    let total = arrayLength(&src.vertices);
    let index = global_invocation_id.x;
    if (index >= total) {
        return;
    }

    // TODO: Should vertices with no influences be handled differently.
    // The in game normals appear to be slightly different compared to skinning.
    var vertex = src.vertices[index];
    let influence = vertex_weights.vertices[index];
    
    var position = vec3<f32>(0.0);
    var normal = vec3<f32>(0.0);
    var tangent = vec3<f32>(0.0);

    // TODO: Index vector4 in loop?
    // TODO: Restrict to 511 bones like in game?
    if (influence.bone_indices.x >= 0 && influence.bone_indices.x < 512) {
        position = position + (transforms.transforms[influence.bone_indices.x] * vec4<f32>(vertex.position0.xyz, 1.0) * influence.weights.x).xyz;
        normal = normal + (transforms.transforms_inv_transpose[influence.bone_indices.x] * vec4<f32>(vertex.normal0.xyz, 1.0) * influence.weights.x).xyz;
        tangent = tangent + (transforms.transforms_inv_transpose[influence.bone_indices.x] * vec4<f32>(vertex.tangent0.xyz, 1.0) * influence.weights.x).xyz;
    }
    
    if (influence.bone_indices.y >= 0 && influence.bone_indices.y < 512) {
        position = position + (transforms.transforms[influence.bone_indices.y] * vec4<f32>(vertex.position0.xyz, 1.0) * influence.weights.y).xyz;
        normal = normal + (transforms.transforms_inv_transpose[influence.bone_indices.y] * vec4<f32>(vertex.normal0.xyz, 1.0) * influence.weights.y).xyz;
        tangent = tangent + (transforms.transforms_inv_transpose[influence.bone_indices.y] * vec4<f32>(vertex.tangent0.xyz, 1.0) * influence.weights.y).xyz;
    }

    if (influence.bone_indices.z >= 0 && influence.bone_indices.z < 512) {
        position = position + (transforms.transforms[influence.bone_indices.z] * vec4<f32>(vertex.position0.xyz, 1.0) * influence.weights.z).xyz;
        normal = normal + (transforms.transforms_inv_transpose[influence.bone_indices.z] * vec4<f32>(vertex.normal0.xyz, 1.0) * influence.weights.z).xyz;
        tangent = tangent + (transforms.transforms_inv_transpose[influence.bone_indices.z] * vec4<f32>(vertex.tangent0.xyz, 1.0) * influence.weights.z).xyz;
    }

    if (influence.bone_indices.w >= 0 && influence.bone_indices.w < 512) {
        position = position + (transforms.transforms[influence.bone_indices.w] * vec4<f32>(vertex.position0.xyz, 1.0) * influence.weights.w).xyz;
        normal = normal + (transforms.transforms_inv_transpose[influence.bone_indices.w] * vec4<f32>(vertex.normal0.xyz, 1.0) * influence.weights.w).xyz;
        tangent = tangent + (transforms.transforms_inv_transpose[influence.bone_indices.w] * vec4<f32>(vertex.tangent0.xyz, 1.0) * influence.weights.w).xyz;
    }
    // TODO: Transform each vertex by the parent transform
    // TODO: Transform each vertex by the animation transform
    // TODO: sync to make sure writes happen?
    // TODO: Recalculate normals and tangents for renormal?

    var out: VertexInput0;
    out.position0 = vec4<f32>(position, 1.0);
    out.normal0 = vec4<f32>(normal, 0.0);
    out.tangent0 = vec4<f32>(tangent, vertex.tangent0.w);
    dst.vertices[index] = out;
}