// This should be identical to the Buffer0 struct in model.wgsl.
struct VertexInput0 {
    position0: vec4<f32>,
    normal0: vec4<f32>,
    tangent0: vec4<f32>,
};

struct VertexWeight {
    bone_indices: vec4<i32>,
    weights: vec4<f32>,
};

struct Vertices {
    vertices: array<VertexInput0>
};

struct AdjData {
    adjacency: array<i32>
};

@group(0) @binding(0) var<storage, read_write> vertices: Vertices;
@group(0) @binding(1) var<storage, read> adj_data: AdjData;

// TODO: Can this be done in the skinning compute pass?
// A single shader would require synchronization to ensure all writes to position0 finish.
@compute
@workgroup_size(256)
fn main(@builtin(global_invocation_id) global_invocation_id: vec3<u32>) {
    let vertexCount = arrayLength(&vertices.vertices);
    let index = global_invocation_id.x;
    if (index >= vertexCount) {
        return;
    }

    var in = vertices.vertices[index];

    // Average normals over adjacent faces to calculate smooth normals.
    // This reduces shading artifacts in animations with heavy deformations.
    var renormal = vec3(0.0);
    let start = i32(index) * 18;

    // Loop over up to 9 adjacent faces.
    let vertexCount = i32(vertexCount);
    for (var i = 0; i < 9; i = i + 1) {
        let v0 = i32(index);
        let v1 = adj_data.adjacency[start + i*2 + 0];
        let v2 = adj_data.adjacency[start + i*2 + 1];

        if ((v0 >= 0 && v0 < vertexCount) && (v1 >= 0 && v1 < vertexCount) && (v2 >= 0 && v2 < vertexCount)) {
            let u = vertices.vertices[v1].position0 - vertices.vertices[v0].position0;
            let v = vertices.vertices[v2].position0 - vertices.vertices[v0].position0;
            renormal = renormal + cross(u.xyz, v.xyz);
        }
    }

    var out: VertexInput0;
    out.position0 = in.position0;
    out.normal0 = vec4(normalize(renormal), 0.0);
    // TODO: Do we need to recompute tangents?
    out.tangent0 = in.tangent0;
    vertices.vertices[index] = out;
}