struct VertexInput {
    [[location(0)]] position: vec3<f32>;
    [[location(1)]] normal: vec3<f32>;
};

struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(0)]] position: vec3<f32>;
    [[location(1)]] normal: vec3<f32>;
};

struct CameraTransforms {
    model_view_matrix: mat4x4<f32>;
    mvp_matrix: mat4x4<f32>;
    camera_pos: vec4<f32>;
};

struct WorldTransforms {
    transforms: array<mat4x4<f32>, 512>;
};

struct BoneColors {
    // The world transform of each bone.
    // This is used for parenting objects to bones.
    colors: array<vec4<f32>, 512>;
};

struct PerBone {
    // index, parent_index, _, _
    indices: vec4<i32>;
};

// TODO: Bind groups should be ordered by how frequently they change for performance.
[[group(0), binding(0)]]
var<uniform> camera: CameraTransforms;

[[group(1), binding(0)]]
var<uniform> world_transforms: WorldTransforms;

[[group(1), binding(1)]]
var<uniform> bone_colors: BoneColors;

// TODO: Just use instancing?
[[group(2), binding(0)]]
var<uniform> per_bone: PerBone;

[[stage(vertex)]]
fn vs_bone(
    in: VertexInput,
) -> VertexOutput {
    let bone_index = per_bone.indices.x;
    var out: VertexOutput;
    if (bone_index >= 0 && bone_index < 512) {
        let bone_pos = world_transforms.transforms[per_bone.indices.x] * vec4<f32>(0.0, 0.0, 0.0, 1.0);

        // Keep a constant size in pixels on screen.
        let scale_factor = distance(bone_pos.xyz, camera.camera_pos.xyz) * 0.0025;
        let position = vec4<f32>(in.position.xyz * scale_factor, 1.0);

        out.clip_position = camera.mvp_matrix * world_transforms.transforms[per_bone.indices.x] * position;
        out.position = in.position;
        out.normal = (world_transforms.transforms[per_bone.indices.x] * vec4<f32>(in.normal, 0.0)).xyz;
    }
    return out;
}

[[stage(vertex)]]
fn vs_joint(
    in: VertexInput,
) -> VertexOutput {
    let bone_index = per_bone.indices.x;
    var out: VertexOutput;
    if (bone_index >= 0 && bone_index < 512) {
        let bone_pos = world_transforms.transforms[bone_index] * vec4<f32>(0.0, 0.0, 0.0, 1.0);

        // Keep a constant size in pixels on screen.
        // Only scale the ends of the joint without affecting the height.
        let scale_factor = distance(bone_pos.xyz, camera.camera_pos.xyz) * 0.005;
        let position = vec4<f32>(in.position.xyz * vec3<f32>(scale_factor, 1.0, scale_factor), 1.0);

        out.clip_position = camera.mvp_matrix * world_transforms.transforms[bone_index] * position;
        out.position = in.position;
        out.normal = (world_transforms.transforms[bone_index] * vec4<f32>(in.normal, 0.0)).xyz;
    }
    return out;
}

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    let viewVector = normalize(camera.camera_pos.xyz - in.position.xyz);
    let shading = mix(0.5, 1.0, dot(viewVector, normalize(in.normal)));
    var color = vec3<f32>(0.0);
    let bone_index = per_bone.indices.x;
    if (bone_index >= 0 && bone_index < 512) {
        color = bone_colors.colors[bone_index].xyz * shading;
    }
    return vec4<f32>(pow(color, vec3<f32>(2.2)), 1.0);
}