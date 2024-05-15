struct VertexInput {
    @location(0) position: vec4<f32>,
    @location(1) normal: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

struct CameraTransforms {
    model_view_matrix: mat4x4<f32>,
    projection_matrix: mat4x4<f32>,
    mvp_matrix: mat4x4<f32>,
    mvp_inv_matrix: mat4x4<f32>,
    camera_pos: vec4<f32>,
    screen_dimensions: vec4<f32>, // width, height, scale, _
};

struct WorldTransforms {
    transforms: array<mat4x4<f32>, 512>
};

// Swing collisions can use two bones like capsules.
// Some shapes like spheres will use only one bone.
struct PerShape {
    bone_indices: vec4<i32>, // start, -1, -1, -1
    start_transform: mat4x4<f32>,
    color: vec4<f32>
}

@group(0) @binding(0)
var<uniform> camera: CameraTransforms;

@group(1) @binding(0)
var<uniform> world_transforms: WorldTransforms;

@group(2) @binding(0)
var<uniform> per_shape: PerShape;

// TODO: Is it easier to make this part of skeleton.wgsl?
// TODO: Add a second transform for the bone transform?

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    // TODO: Use a consistent naming convention like PerScene, PerSkel, PerObject etc.
    // Assume the vertex buffer is centered on the origin with unit size.
    var out: VertexOutput;

    var world_position = per_shape.start_transform * vec4(in.position.xyz, 1.0);
    if per_shape.bone_indices.x >= 0 && per_shape.bone_indices.x < 512 {
        world_position = world_transforms.transforms[per_shape.bone_indices.x] * world_position;
    }

    out.clip_position = camera.mvp_matrix * world_position;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Premultiplied alpha.
    let alpha = 0.15;
    return vec4(per_shape.color.rgb * alpha, alpha);
}