struct VertexInput {
    @location(0) position: vec4<f32>,
    @location(1) normal: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) position: vec4<f32>,
    @location(1) normal: vec4<f32>,
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

struct BoneColors {
    // The world transform of each bone.
    // This is used for parenting objects to bones.
    colors: array<vec4<f32>, 512>
};

struct PerBone {
    // index, parent_index, _, _
    indices: vec4<i32>
};

// TODO: Bind groups should be ordered by how frequently they change for performance.
@group(0) @binding(0)
var<uniform> camera: CameraTransforms;

@group(1) @binding(0)
var<uniform> world_transforms: WorldTransforms;

@group(1) @binding(1)
var<uniform> bone_colors: BoneColors;

// TODO: Just use instancing?
@group(2) @binding(0)
var<uniform> per_bone: PerBone;

@vertex
fn vs_axes(in: VertexInput) -> VertexOutput {
    let bone_index = per_bone.indices.x;
    var out: VertexOutput;
    if bone_index >= 0 && bone_index < 512 {
        let position = vec4(in.position.xyz, 1.0);
        out.clip_position = camera.mvp_matrix * world_transforms.transforms[bone_index] * position;
        out.position = vec4(in.position.xyz, 1.0);
        // Use the normal as the color.
        out.normal = vec4(in.position.xyz, 0.0);
    }

    return out;
}

@vertex
fn vs_bone(in: VertexInput) -> VertexOutput {
    let bone_index = per_bone.indices.x;
    var out: VertexOutput;
    if bone_index >= 0 && bone_index < 512 {
        var world_transform = world_transforms.transforms[per_bone.indices.x];
        // Quick fix to disable bone scale for some skeletons like Kirby.
        world_transform[0] = vec4(normalize(world_transform[0].xyz), world_transform[0].w);
        world_transform[1] = vec4(normalize(world_transform[1].xyz), world_transform[1].w);
        world_transform[2] = vec4(normalize(world_transform[2].xyz), world_transform[2].w);

        let bone_pos = world_transform * vec4(0.0, 0.0, 0.0, 1.0);

        // Keep a constant size in pixels on screen.
        let scale_factor = distance(bone_pos.xyz, camera.camera_pos.xyz) * 0.0025;
        let position = vec4(in.position.xyz * scale_factor, 1.0);

        out.clip_position = camera.mvp_matrix * world_transform * position;
        out.position = in.position;
        out.normal = world_transform * vec4(in.normal.xyz, 0.0);
    }
    return out;
}

@vertex
fn vs_joint(in: VertexInput) -> VertexOutput {
    let bone_index = per_bone.indices.x;
    var out: VertexOutput;
    if bone_index >= 0 && bone_index < 512 {
        let bone_pos = world_transforms.transforms[bone_index] * vec4(0.0, 0.0, 0.0, 1.0);

        // Keep a constant size in pixels on screen.
        // Only scale the ends of the joint without affecting the height.
        let scale_factor = distance(bone_pos.xyz, camera.camera_pos.xyz) * 0.005;
        let position = vec4(in.position.xyz * vec3(scale_factor, 1.0, scale_factor), 1.0);

        out.clip_position = camera.mvp_matrix * world_transforms.transforms[bone_index] * position;
        out.position = in.position;
        out.normal = world_transforms.transforms[bone_index] * vec4(in.normal.xyz, 0.0);
    }
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let viewVector = normalize(camera.camera_pos.xyz - in.position.xyz);
    let shading = mix(0.5, 1.0, dot(viewVector, normalize(in.normal.xyz)));
    var color = vec3(0.0);
    let bone_index = per_bone.indices.x;
    if bone_index >= 0 && bone_index < 512 {
        color = bone_colors.colors[bone_index].xyz * shading;
    }
    return vec4(pow(color, vec3(2.2)), 1.0);
}

@fragment
fn fs_axes(in: VertexOutput) -> @location(0) vec4<f32> {
    // Use the normals as vertex color.
    return vec4(in.normal.xyz, 1.0);
}