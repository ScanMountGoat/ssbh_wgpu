use crate::{
    animation::AnimationTransforms,
    renderer::INVERTED_STENCIL_MASK_STATE,
    shape::{sphere_indices, sphere_vertices, IndexedMeshBuffers},
    DeviceBufferExt,
};
use glam::Vec4Swizzles;
use ssbh_data::{hlpb_data::HlpbData, skel_data::SkelData};

// TODO: Create a shared outline renderer for outlining bones, joints, meshes, etc.
// It's too difficult to get a fixed outline width using two meshes and culling.
pub struct BonePipelines {
    pub bone_pipeline: wgpu::RenderPipeline,
    pub joint_pipeline: wgpu::RenderPipeline,
    pub bone_axes_pipeline: wgpu::RenderPipeline,
}

impl BonePipelines {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        // TODO: Move this to bone rendering?
        let bone_pipeline = skeleton_pipeline(
            device,
            "vs_bone",
            "fs_main",
            wgpu::Face::Back,
            surface_format,
        );
        let joint_pipeline = skeleton_pipeline(
            device,
            "vs_joint",
            "fs_main",
            wgpu::Face::Back,
            surface_format,
        );
        let bone_axes_pipeline = bone_axes_pipeline(device, surface_format);

        Self {
            bone_pipeline,
            joint_pipeline,
            bone_axes_pipeline,
        }
    }
}

pub struct BoneBuffers {
    pub bone_buffers: IndexedMeshBuffers,
    pub joint_buffers: IndexedMeshBuffers,
    pub axes_buffers: IndexedMeshBuffers,
}

impl BoneBuffers {
    pub fn new(device: &wgpu::Device) -> Self {
        // TODO: Create these from shapes instead.
        let bone_buffers = IndexedMeshBuffers {
            vertex_buffer: bone_vertex_buffer(device),
            index_buffer: bone_index_buffer(device),
            index_count: bone_index_count() as u32,
        };

        let joint_buffers = IndexedMeshBuffers {
            vertex_buffer: joint_vertex_buffer(device),
            index_buffer: joint_index_buffer(device),
            index_count: joint_index_count() as u32,
        };

        let axes_buffers = IndexedMeshBuffers {
            vertex_buffer: axes_vertex_buffer(device),
            index_buffer: axes_index_buffer(device),
            index_count: bone_axes_index_count() as u32,
        };

        Self {
            bone_buffers,
            joint_buffers,
            axes_buffers,
        }
    }
}

pub fn joint_transforms(skel: &SkelData, anim_transforms: &AnimationTransforms) -> Vec<glam::Mat4> {
    let mut joint_transforms: Vec<_> = skel
        .bones
        .iter()
        .enumerate()
        .map(|(i, bone)| {
            // TODO: Add an option to show the bone's actual rotation?
            // TODO: The bones wont be connected and should use a different model for rendering.
            let pos = anim_transforms.world_transforms[i].col(3).xyz();
            let parent_pos = bone
                .parent_index
                .and_then(|parent_index| anim_transforms.world_transforms.get(parent_index))
                .map(|transform| transform.col(3).xyz())
                .unwrap_or(pos);

            let scale = pos.distance(parent_pos);

            // Assume an inverted pyramid with up as the Y-axis.
            // 1. Scale the pyramid along the Y-axis to have the appropriate length.
            // 2. Rotate the pyramid to point to its parent.
            // 3. Translate the bone to its world position.
            let rotation =
                glam::Quat::from_rotation_arc(glam::Vec3::Y, (parent_pos - pos).normalize());
            glam::Mat4::from_translation(pos)
                * glam::Mat4::from_quat(rotation)
                * glam::Mat4::from_scale(glam::vec3(1.0, scale, 1.0))
        })
        .collect();
    joint_transforms.resize(crate::animation::MAX_BONE_COUNT, glam::Mat4::IDENTITY);
    joint_transforms
}

pub fn bone_axes_index_count() -> usize {
    bone_axes_indices().len()
}

pub fn bone_index_count() -> usize {
    sphere_indices(8, 8, crate::shape::SphereRange::Full).len()
}

pub fn joint_index_count() -> usize {
    pyramid_indices().len()
}

pub fn bone_colors_buffer(
    device: &wgpu::Device,
    skel: Option<&SkelData>,
    hlpb: Option<&HlpbData>,
) -> wgpu::Buffer {
    device.create_buffer_from_data(
        "Bone Colors Buffer",
        &bone_colors(skel, hlpb),
        wgpu::BufferUsages::UNIFORM,
    )
}

pub fn bone_vertex_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer_from_data(
        "Bone Vertex Buffer",
        &sphere_vertices(8, 8, crate::shape::SphereRange::Full),
        wgpu::BufferUsages::VERTEX,
    )
}

pub fn bone_index_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_index_buffer(
        "Bone Index Buffer",
        &sphere_indices(8, 8, crate::shape::SphereRange::Full),
    )
}

pub fn joint_vertex_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer_from_data(
        "Bone Joint Vertex Buffer",
        &pyramid(),
        wgpu::BufferUsages::VERTEX,
    )
}

pub fn joint_index_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_index_buffer("Joint Index Buffer", &pyramid_indices())
}

pub fn axes_vertex_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer_from_data(
        "Bone Axes Vertex Buffer",
        &[
            // Use the normals to store colors.
            // X+
            [0f32, 0f32, 0f32, 1.0],
            [1f32, 0f32, 0f32, 1.0],
            [1f32, 0f32, 0f32, 1.0],
            [1f32, 0f32, 0f32, 1.0],
            // Y+
            [0f32, 0f32, 0f32, 1.0],
            [0f32, 1f32, 0f32, 1.0],
            [0f32, 1f32, 0f32, 1.0],
            [0f32, 1f32, 0f32, 1.0],
            // Z+
            [0f32, 0f32, 0f32, 1.0],
            [0f32, 0f32, 1f32, 1.0],
            [0f32, 0f32, 1f32, 1.0],
            [0f32, 0f32, 1f32, 1.0],
        ],
        wgpu::BufferUsages::VERTEX,
    )
}

pub fn axes_index_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_index_buffer("Bone Axes Index Buffer", &bone_axes_indices())
}

fn bone_axes_indices() -> Vec<u32> {
    vec![0, 1, 2, 3, 4, 5]
}

fn pyramid() -> Vec<[f32; 4]> {
    // Pos0 Nrm0 Pos1 Nrm1 ...
    vec![
        [0.000000, 1.000000, 0.000000, 1.0000],
        [-0.1400, 0.9901, 0.0000, 1.0000],
        [-0.707107, 0.900000, -0.707107, 1.0000],
        [-0.7863, -0.6178, 0.0000, 1.0000],
        [0.707107, 0.900000, -0.707107, 1.0000],
        [0.7863, -0.6178, 0.0000, 1.0000],
        [0.707107, 0.900000, 0.707107, 1.0000],
        [0.0000, -0.6178, 0.7863, 1.0000],
        [-0.707107, 0.900000, 0.707107, 1.0000],
        [-0.7863, -0.6178, 0.0000, 1.0000],
        [0.000000, 0.000000, 0.000000, 1.0000],
        [-0.7863, -0.6178, 0.0000, 1.0000],
        [0.707107, 0.900000, -0.707107, 1.0000],
        [0.0000, 0.9901, -0.1400, 1.0000],
        [0.707107, 0.900000, -0.707107, 1.0000],
        [0.0000, -0.6178, -0.7863, 1.0000],
        [0.707107, 0.900000, -0.707107, 1.0000],
        [0.1400, 0.9901, 0.0000, 1.0000],
        [0.000000, 1.000000, 0.000000, 1.0000],
        [0.0000, 0.9901, -0.1400, 1.0000],
        [0.000000, 1.000000, 0.000000, 1.0000],
        [0.1400, 0.9901, 0.0000, 1.0000],
        [0.000000, 1.000000, 0.000000, 1.0000],
        [0.0000, 0.9901, 0.1400, 1.0000],
        [-0.707107, 0.900000, -0.707107, 1.0000],
        [0.0000, 0.9901, -0.1400, 1.0000],
        [-0.707107, 0.900000, -0.707107, 1.0000],
        [0.0000, -0.6178, -0.7863, 1.0000],
        [-0.707107, 0.900000, -0.707107, 1.0000],
        [-0.1400, 0.9901, 0.0000, 1.0000],
        [0.000000, 0.000000, 0.000000, 1.0000],
        [0.0000, -0.6178, -0.7863, 1.0000],
        [0.000000, 0.000000, 0.000000, 1.0000],
        [0.7863, -0.6178, 0.0000, 1.0000],
        [0.000000, 0.000000, 0.000000, 1.0000],
        [0.0000, -0.6178, 0.7863, 1.0000],
        [0.707107, 0.900000, 0.707107, 1.0000],
        [0.1400, 0.9901, 0.0000, 1.0000],
        [0.707107, 0.900000, 0.707107, 1.0000],
        [0.7863, -0.6178, 0.0000, 1.0000],
        [0.707107, 0.900000, 0.707107, 1.0000],
        [0.0000, 0.9901, 0.1400, 1.0000],
        [-0.707107, 0.900000, 0.707107, 1.0000],
        [0.0000, 0.9901, 0.1400, 1.0000],
        [-0.707107, 0.900000, 0.707107, 1.0000],
        [0.0000, -0.6178, 0.7863, 1.0000],
        [-0.707107, 0.900000, 0.707107, 1.0000],
        [-0.1400, 0.9901, 0.0000, 1.0000],
    ]
}

fn pyramid_indices() -> Vec<u32> {
    // An inverted pyramid with a pyramid base.
    vec![
        9, 6, 12, 13, 7, 15, 10, 18, 8, 2, 19, 16, 11, 21, 20, 3, 22, 17, 0, 14, 23, 4, 1, 5,
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
                    if bone.name == constraint.target_bone_name {
                        colors[i] = helper_color;
                    }
                }
            }
        }
    }
    colors
}

// TODO: Create a separate pipeline for the outline that sets stencil.
fn skeleton_pipeline(
    device: &wgpu::Device,
    vertex_entry: &str,
    fragment_entry: &str,
    cull_face: wgpu::Face,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let module = crate::shader::skeleton::create_shader_module(device);
    let layout = crate::shader::skeleton::create_pipeline_layout(device);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Skeleton Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some(vertex_entry),
            buffers: &[crate::shader::skeleton::VertexInput::vertex_buffer_layout(
                wgpu::VertexStepMode::Vertex,
            )],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some(fragment_entry),
            targets: &[Some(surface_format.into())],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            cull_mode: Some(cull_face),
            ..Default::default()
        },
        depth_stencil: Some(INVERTED_STENCIL_MASK_STATE),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

fn bone_axes_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = crate::shader::skeleton::create_shader_module(device);
    let layout = crate::shader::skeleton::create_pipeline_layout(device);
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Bone Axes Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_axes"),
            buffers: &[crate::shader::skeleton::VertexInput::vertex_buffer_layout(
                wgpu::VertexStepMode::Vertex,
            )],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_axes"),
            targets: &[Some(surface_format.into())],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            polygon_mode: wgpu::PolygonMode::Line,
            topology: wgpu::PrimitiveTopology::LineList,
            ..Default::default()
        }, // TODO: Just disable the depth?
        depth_stencil: Some(wgpu::DepthStencilState {
            format: crate::renderer::DEPTH_STENCIL_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}
