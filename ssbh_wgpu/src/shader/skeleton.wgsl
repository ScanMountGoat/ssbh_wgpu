struct VertexInput {
    [[location(0)]] position: vec3<f32>;
};

struct CameraTransforms {
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
fn vs_main(
    in: VertexInput,
) -> [[builtin(position)]] vec4<f32> {
    // TODO: Check the bounds.
    // Keep a constant size in pixels on screen.
    let bone_pos = world_transforms.transforms[per_bone.indices.x] * vec4<f32>(0.0, 0.0, 0.0, 1.0);
    let scale_factor = distance(bone_pos.xyz, camera.camera_pos.xyz) * 0.0025;

    // TODO: Find a better way to avoid scaling the pyramid height.
    // let position = vec4<f32>(in.position.xyz * scale_factor, 1.0);
    let position = vec4<f32>(in.position.xyz, 1.0);

    return camera.mvp_matrix * world_transforms.transforms[per_bone.indices.x] * position;
}

[[stage(fragment)]]
fn fs_main() -> [[location(0)]] vec4<f32> {
    // TODO: Check the bounds.
    let color = bone_colors.colors[per_bone.indices.x].xyz;
    return vec4<f32>(pow(color, vec3<f32>(2.2)), 1.0);
}