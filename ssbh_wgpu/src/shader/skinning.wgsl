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

struct VertexWeights {
    vertices: array<VertexWeight>
};

// The in game buffer is vec4[4096] with the first vec4 containing a u32 bone count.
// This allows at most 511 bones with 2 matrices per bone.
// TODO: What two matrices are stored per bone?
// Remove the length field to improve compatibility.
// This gives a more generous alignment without exceeding 65536 bytes.
struct AnimatedWorldTransforms {
    // bone_world.inv() * animated_bone_world
    transforms: array<mat4x4<f32>, 512>,
    // Inverse transpose of above to use for normals and tangents.
    transforms_inv_transpose: array<mat4x4<f32>, 512>,
};

struct WorldTransforms {
    // The world transform of each bone.
    // This is used for parenting objects to bones.
    transforms: array<mat4x4<f32>, 512>
};

struct MeshObjectInfo {
    // TODO: Alignment?
    // Just use X for now.
    parent_index: vec4<i32>
};

@group(0) @binding(0) var<storage, read> src : Vertices;
@group(0) @binding(1) var<storage, read> vertex_weights : VertexWeights;
@group(0) @binding(2) var<storage, read_write> dst : Vertices;

@group(1) @binding(0) var<uniform> transforms: AnimatedWorldTransforms;
@group(1) @binding(1) var<uniform> world_transforms: WorldTransforms;

@group(2) @binding(0) var<uniform> mesh_object_info: MeshObjectInfo;

@compute
@workgroup_size(256)
fn main(@builtin(global_invocation_id) global_invocation_id: vec3<u32>) {
    let total = arrayLength(&src.vertices);
    let index = global_invocation_id.x;
    if (index >= total) {
        return;
    }

    var vertex = src.vertices[index];
    let influence = vertex_weights.vertices[index];
    
    // Some mesh objects are parented to a bone and don't use skinning.
    // This transform is currently applied in the vertex shader.
    // TODO: Should vertices with no influences be handled differently.
    // The in game normals appear to be slightly different compared to skinning.
    var position = vertex.position0.xyz;
    var normal = vertex.normal0.xyz;
    var tangent = vertex.tangent0.xyz;

    // Apply parent transforms.
    // Assume the object won't also have vertex weights.
    // The application of vertex weights "resets" the vectors.
    let parent_index = mesh_object_info.parent_index.x;
    if (parent_index >= 0 && parent_index < 512) {
        position = (world_transforms.transforms[parent_index] * vec4(position, 1.0)).xyz;
        normal = (world_transforms.transforms[parent_index] * vec4(normal, 0.0)).xyz;
        tangent = (world_transforms.transforms[parent_index] * vec4(tangent, 0.0)).xyz;
    }

    // Disabling skinning if the first influence is unused.
    if (influence.bone_indices.x >= 0) {
        position = vec3(0.0);
        normal = vec3(0.0);
        tangent = vec3(0.0);

        for (var i = 0; i < 4; i = i + 1) {
            // Only 511 influences are supported in game.
            let bone_index = influence.bone_indices[i];
            if (bone_index >= 0 && bone_index < 511) {
                position = position + (transforms.transforms[bone_index] * vec4(vertex.position0.xyz, 1.0) * influence.weights[i]).xyz;
                normal = normal + (transforms.transforms_inv_transpose[bone_index] * vec4(vertex.normal0.xyz, 0.0) * influence.weights[i]).xyz;
                tangent = tangent + (transforms.transforms_inv_transpose[bone_index] * vec4(vertex.tangent0.xyz, 0.0) * influence.weights[i]).xyz;
            }
        }
    }

    var out: VertexInput0;
    out.position0 = vec4(position, 1.0);
    out.normal0 = vec4(normalize(normal), 0.0);
    out.tangent0 = vec4(normalize(tangent), vertex.tangent0.w);
    dst.vertices[index] = out;
}