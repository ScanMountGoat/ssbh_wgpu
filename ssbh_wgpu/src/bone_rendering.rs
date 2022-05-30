use glam::Vec4Swizzles;
use ssbh_data::{hlpb_data::HlpbData, skel_data::SkelData};

use crate::animation::AnimationTransforms;
use wgpu::util::DeviceExt;

pub fn joint_transforms(skel: &SkelData, anim_transforms: &AnimationTransforms) -> Vec<glam::Mat4> {
    let mut joint_transforms: Vec<_> = skel
        .bones
        .iter()
        .enumerate()
        .map(|(i, bone)| {
            let pos = anim_transforms.world_transforms[i].col(3).xyz();
            let mut parent_pos = pos;
            if let Some(parent_index) = bone.parent_index {
                parent_pos = anim_transforms.world_transforms[parent_index].col(3).xyz();
            }
            let scale = pos.distance(parent_pos);

            // Assume an inverted pyramid with up as the Y-axis.
            // 1. Scale the pyramid along the Y-axis to have the appropriate length.
            // 2. Rotate the pyramid to point to its parent.
            // 3. Translate the bone to its world position.
            let rotation =
                glam::Quat::from_rotation_arc(glam::Vec3::Y, (parent_pos - pos).normalize());
            glam::Mat4::from_translation(pos)
                * glam::Mat4::from_quat(rotation)
                * glam::Mat4::from_scale(glam::Vec3::new(1.0, scale, 1.0))
        })
        .collect();
    joint_transforms.resize(crate::animation::MAX_BONE_COUNT, glam::Mat4::IDENTITY);
    joint_transforms
}

pub fn bone_index_count() -> usize {
    sphere_indices().len()
}

pub fn joint_index_count() -> usize {
    pyramid_indices().len()
}

pub fn bone_colors_buffer(
    device: &wgpu::Device,
    skel: Option<&SkelData>,
    hlpb: Option<&HlpbData>,
) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Bone Colors Buffer"),
        contents: bytemuck::cast_slice(&bone_colors(skel, hlpb)),
        usage: wgpu::BufferUsages::UNIFORM,
    })
}

pub fn bone_vertex_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Bone Vertex Buffer"),
        contents: bytemuck::cast_slice(&sphere()),
        usage: wgpu::BufferUsages::VERTEX,
    })
}

pub fn bone_vertex_buffer_outer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Bone Vertex Buffer Outer"),
        contents: bytemuck::cast_slice(&sphere_outer()),
        usage: wgpu::BufferUsages::VERTEX,
    })
}

pub fn bone_index_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Bone Index Buffer"),
        contents: bytemuck::cast_slice(&sphere_indices()),
        usage: wgpu::BufferUsages::INDEX,
    })
}

pub fn joint_vertex_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Bone Joint Vertex Buffer"),
        contents: bytemuck::cast_slice(&pyramid()),
        usage: wgpu::BufferUsages::VERTEX,
    })
}

pub fn joint_vertex_buffer_outer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Bone Joint Vertex Buffer Outer"),
        contents: bytemuck::cast_slice(&pyramid_outer()),
        usage: wgpu::BufferUsages::VERTEX,
    })
}

pub fn joint_index_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Joint Index Buffer"),
        contents: bytemuck::cast_slice(&pyramid_indices()),
        usage: wgpu::BufferUsages::INDEX,
    })
}

fn pyramid() -> Vec<[f32; 3]> {
    // Pos0 Nrm0 Pos1 Nrm1 ...
    vec![
        [0.000000, 1.000000, 0.000000],
        [-0.1400, 0.9901, 0.0000],
        [-0.707107, 0.900000, -0.707107],
        [-0.7863, -0.6178, 0.0000],
        [0.707107, 0.900000, -0.707107],
        [0.7863, -0.6178, 0.0000],
        [0.707107, 0.900000, 0.707107],
        [0.0000, -0.6178, 0.7863],
        [-0.707107, 0.900000, 0.707107],
        [-0.7863, -0.6178, 0.0000],
        [0.000000, 0.000000, 0.000000],
        [-0.7863, -0.6178, 0.0000],
        [0.707107, 0.900000, -0.707107],
        [0.0000, 0.9901, -0.1400],
        [0.707107, 0.900000, -0.707107],
        [0.0000, -0.6178, -0.7863],
        [0.707107, 0.900000, -0.707107],
        [0.1400, 0.9901, 0.0000],
        [0.000000, 1.000000, 0.000000],
        [0.0000, 0.9901, -0.1400],
        [0.000000, 1.000000, 0.000000],
        [0.1400, 0.9901, 0.0000],
        [0.000000, 1.000000, 0.000000],
        [0.0000, 0.9901, 0.1400],
        [-0.707107, 0.900000, -0.707107],
        [0.0000, 0.9901, -0.1400],
        [-0.707107, 0.900000, -0.707107],
        [0.0000, -0.6178, -0.7863],
        [-0.707107, 0.900000, -0.707107],
        [-0.1400, 0.9901, 0.0000],
        [0.000000, 0.000000, 0.000000],
        [0.0000, -0.6178, -0.7863],
        [0.000000, 0.000000, 0.000000],
        [0.7863, -0.6178, 0.0000],
        [0.000000, 0.000000, 0.000000],
        [0.0000, -0.6178, 0.7863],
        [0.707107, 0.900000, 0.707107],
        [0.1400, 0.9901, 0.0000],
        [0.707107, 0.900000, 0.707107],
        [0.7863, -0.6178, 0.0000],
        [0.707107, 0.900000, 0.707107],
        [0.0000, 0.9901, 0.1400],
        [-0.707107, 0.900000, 0.707107],
        [0.0000, 0.9901, 0.1400],
        [-0.707107, 0.900000, 0.707107],
        [0.0000, -0.6178, 0.7863],
        [-0.707107, 0.900000, 0.707107],
        [-0.1400, 0.9901, 0.0000],
    ]
}

fn pyramid_outer() -> Vec<[f32; 3]> {
    // TODO: Scale positions along the normal direction?
    let scale = 1.25;
    pyramid()
        .iter()
        .map(|v| [v[0] * scale, v[1], v[2] * scale])
        .collect()
}

fn pyramid_indices() -> Vec<u32> {
    // An inverted pyramid with a pyramid base.
    vec![
        9, 6, 12, 13, 7, 15, 10, 18, 8, 2, 19, 16, 11, 21, 20, 3, 22, 17, 0, 14, 23, 4, 1, 5,
    ]
}

fn sphere() -> Vec<[f32; 3]> {
    // Pos0 Nrm0 Pos1 Nrm1 ...
    vec![
        [0.000000, 0.923880, -0.382683],
        [0.000000, 0.923880, -0.382683],
        [0.000000, 0.707107, -0.707107],
        [0.000000, 0.707107, -0.707107],
        [0.000000, 0.382683, -0.923880],
        [0.000000, 0.382683, -0.923880],
        [0.000000, -0.000000, -1.000000],
        [0.000000, -0.000000, -1.000000],
        [0.000000, -0.382683, -0.923880],
        [0.000000, -0.382683, -0.923880],
        [0.000000, -0.707107, -0.707107],
        [0.000000, -0.707107, -0.707107],
        [0.000000, -0.923880, -0.382683],
        [0.000000, -0.923880, -0.382683],
        [0.270598, 0.923880, -0.270598],
        [0.270598, 0.923880, -0.270598],
        [0.500000, 0.707107, -0.500000],
        [0.500000, 0.707107, -0.500000],
        [0.653282, 0.382683, -0.653281],
        [0.653282, 0.382683, -0.653281],
        [0.707107, -0.000000, -0.707107],
        [0.707107, -0.000000, -0.707107],
        [0.653282, -0.382683, -0.653282],
        [0.653282, -0.382683, -0.653282],
        [0.500000, -0.707107, -0.500000],
        [0.500000, -0.707107, -0.500000],
        [0.270598, -0.923880, -0.270598],
        [0.270598, -0.923880, -0.270598],
        [0.382684, 0.923880, 0.000000],
        [0.382684, 0.923880, 0.000000],
        [0.707107, 0.707107, 0.000000],
        [0.707107, 0.707107, 0.000000],
        [0.923880, 0.382683, 0.000000],
        [0.923880, 0.382683, 0.000000],
        [1.000000, -0.000000, 0.000000],
        [1.000000, -0.000000, 0.000000],
        [0.923880, -0.382683, 0.000000],
        [0.923880, -0.382683, 0.000000],
        [0.707107, -0.707107, 0.000000],
        [0.707107, -0.707107, 0.000000],
        [0.382684, -0.923880, 0.000000],
        [0.382684, -0.923880, 0.000000],
        [0.270598, 0.923880, 0.270598],
        [0.270598, 0.923880, 0.270598],
        [0.500000, 0.707107, 0.500000],
        [0.500000, 0.707107, 0.500000],
        [0.653282, 0.382683, 0.653282],
        [0.653282, 0.382683, 0.653282],
        [0.707107, -0.000000, 0.707107],
        [0.707107, -0.000000, 0.707107],
        [0.653282, -0.382683, 0.653282],
        [0.653282, -0.382683, 0.653282],
        [0.500000, -0.707107, 0.500000],
        [0.500000, -0.707107, 0.500000],
        [0.270598, -0.923880, 0.270598],
        [0.270598, -0.923880, 0.270598],
        [0.000000, 0.923880, 0.382684],
        [0.000000, 0.923880, 0.382684],
        [0.000000, 0.707107, 0.707107],
        [0.000000, 0.707107, 0.707107],
        [0.000000, 0.382683, 0.923880],
        [0.000000, 0.382683, 0.923880],
        [0.000000, -0.000000, 1.000000],
        [0.000000, -0.000000, 1.000000],
        [0.000000, -0.382683, 0.923880],
        [0.000000, -0.382683, 0.923880],
        [0.000000, -0.707107, 0.707107],
        [0.000000, -0.707107, 0.707107],
        [0.000000, -0.923880, 0.382684],
        [0.000000, -0.923880, 0.382684],
        [-0.000000, 1.000000, 0.000000],
        [-0.000000, 1.000000, 0.000000],
        [-0.270598, 0.923880, 0.270598],
        [-0.270598, 0.923880, 0.270598],
        [-0.500000, 0.707107, 0.500000],
        [-0.500000, 0.707107, 0.500000],
        [-0.653281, 0.382683, 0.653282],
        [-0.653281, 0.382683, 0.653282],
        [-0.707107, -0.000000, 0.707107],
        [-0.707107, -0.000000, 0.707107],
        [-0.653282, -0.382683, 0.653282],
        [-0.653282, -0.382683, 0.653282],
        [-0.500000, -0.707107, 0.500000],
        [-0.500000, -0.707107, 0.500000],
        [-0.270598, -0.923880, 0.270598],
        [-0.270598, -0.923880, 0.270598],
        [-0.382684, 0.923880, 0.000000],
        [-0.382684, 0.923880, 0.000000],
        [-0.707107, 0.707107, 0.000000],
        [-0.707107, 0.707107, 0.000000],
        [-0.923879, 0.382683, 0.000000],
        [-0.923879, 0.382683, 0.000000],
        [-1.000000, -0.000000, 0.000000],
        [-1.000000, -0.000000, 0.000000],
        [-0.923880, -0.382683, 0.000000],
        [-0.923880, -0.382683, 0.000000],
        [-0.707107, -0.707107, 0.000000],
        [-0.707107, -0.707107, 0.000000],
        [-0.382684, -0.923880, 0.000000],
        [-0.382684, -0.923880, 0.000000],
        [0.000000, -1.000000, 0.000000],
        [0.000000, -1.000000, 0.000000],
        [-0.270598, 0.923880, -0.270598],
        [-0.270598, 0.923880, -0.270598],
        [-0.500000, 0.707107, -0.500000],
        [-0.500000, 0.707107, -0.500000],
        [-0.653281, 0.382683, -0.653281],
        [-0.653281, 0.382683, -0.653281],
        [-0.707107, -0.000000, -0.707107],
        [-0.707107, -0.000000, -0.707107],
        [-0.653282, -0.382683, -0.653281],
        [-0.653282, -0.382683, -0.653281],
        [-0.500000, -0.707107, -0.500000],
        [-0.500000, -0.707107, -0.500000],
        [-0.270598, -0.923880, -0.270598],
        [-0.270598, -0.923880, -0.270598],
    ]
}

fn sphere_outer() -> Vec<[f32; 3]> {
    let scale = 1.25;
    sphere()
        .iter()
        .map(|v| [v[0] * scale, v[1] * scale, v[2] * scale])
        .collect()
}

fn sphere_indices() -> Vec<u32> {
    vec![
        5, 13, 6, 3, 11, 4, 1, 9, 2, 0, 35, 7, 50, 6, 13, 4, 12, 5, 2, 10, 3, 0, 8, 1, 7, 35, 14,
        50, 13, 20, 11, 19, 12, 9, 17, 10, 7, 15, 8, 12, 20, 13, 11, 17, 18, 8, 16, 9, 19, 25, 26,
        16, 24, 17, 14, 22, 15, 19, 27, 20, 18, 24, 25, 15, 23, 16, 14, 35, 21, 50, 20, 27, 26, 34,
        27, 24, 32, 25, 22, 30, 23, 21, 35, 28, 50, 27, 34, 25, 33, 26, 23, 31, 24, 21, 29, 22, 31,
        40, 32, 29, 38, 30, 28, 35, 36, 50, 34, 42, 32, 41, 33, 30, 39, 31, 28, 37, 29, 34, 41, 42,
        50, 42, 49, 40, 48, 41, 38, 46, 39, 36, 44, 37, 42, 48, 49, 40, 46, 47, 38, 44, 45, 36, 35,
        43, 45, 54, 46, 43, 52, 44, 48, 57, 49, 46, 55, 47, 44, 53, 45, 43, 35, 51, 50, 49, 57, 47,
        56, 48, 56, 6, 57, 54, 4, 55, 52, 2, 53, 51, 35, 0, 50, 57, 6, 55, 5, 56, 54, 2, 3, 52, 0,
        1, 5, 12, 13, 3, 10, 11, 1, 8, 9, 4, 11, 12, 2, 9, 10, 0, 7, 8, 11, 18, 19, 9, 16, 17, 7,
        14, 15, 12, 19, 20, 11, 10, 17, 8, 15, 16, 19, 18, 25, 16, 23, 24, 14, 21, 22, 19, 26, 27,
        18, 17, 24, 15, 22, 23, 26, 33, 34, 24, 31, 32, 22, 29, 30, 25, 32, 33, 23, 30, 31, 21, 28,
        29, 31, 39, 40, 29, 37, 38, 32, 40, 41, 30, 38, 39, 28, 36, 37, 34, 33, 41, 40, 47, 48, 38,
        45, 46, 36, 43, 44, 42, 41, 48, 40, 39, 46, 38, 37, 44, 45, 53, 54, 43, 51, 52, 48, 56, 57,
        46, 54, 55, 44, 52, 53, 47, 55, 56, 56, 5, 6, 54, 3, 4, 52, 1, 2, 55, 4, 5, 54, 53, 2, 52,
        51, 0,
    ]
}

fn bone_colors(skel: Option<&SkelData>, hlpb: Option<&HlpbData>) -> Vec<[f32; 4]> {
    // Match the color scheme used for the Blender addon.
    let helper_color = [0.3, 0.0, 0.6, 1.0];
    let default_color = [0.65, 0.65, 0.65, 1.0];

    let mut colors = vec![[0.0; 4]; crate::animation::MAX_BONE_COUNT];
    if let Some(skel) = skel {
        for (i, bone) in skel.bones.iter().enumerate() {
            colors[i] = default_color;

            // TODO: Check for swing bones.

            // Color helper bones using a different color.
            if let Some(hlpb) = hlpb {
                for constraint in &hlpb.aim_constraints {
                    if bone.name == constraint.target_bone_name2 {
                        colors[i] = helper_color;
                    }
                }

                for constraint in &hlpb.orient_constraints {
                    if bone.name == constraint.driver_bone_name {
                        colors[i] = helper_color;
                    }
                }
            }
        }
    }
    colors
}
