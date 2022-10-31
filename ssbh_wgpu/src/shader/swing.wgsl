struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

struct CameraTransforms {
    model_view_matrix: mat4x4<f32>,
    mvp_matrix: mat4x4<f32>,
    camera_pos: vec4<f32>,
};

struct WorldTransforms {
    transforms: array<mat4x4<f32>, 512>
};

struct PerBone {
    bone_index: vec4<i32>,
    transform: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: CameraTransforms;

@group(1) @binding(0)
var<uniform> world_transforms: WorldTransforms;

@group(2) @binding(0)
var<uniform> per_bone: PerBone;

// TODO: Is it easier to make this part of skeleton.wgsl?
// TODO: Add a second transform for the bone transform?

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    // TODO: Include the bone index in the per bone bind group?
    // TODO: Use a consistent naming convention like PerScene, PerSkel, PerObject etc.
    var out: VertexOutput;
    out.clip_position = camera.mvp_matrix * world_transforms.transforms[per_bone.bone_index.x] * per_bone.transform * vec4(in.position.xyz, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4(1.0);
}