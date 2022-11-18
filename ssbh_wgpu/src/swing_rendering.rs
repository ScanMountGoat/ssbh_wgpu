use glam::Vec4Swizzles;
use prc::hash40::Hash40;
use ssbh_data::skel_data::SkelData;

use crate::{
    shape::{capsule_mesh_buffers, plane_mesh_buffers, sphere_mesh_buffers, IndexedMeshBuffers},
    swing::*,
    DeviceExt2,
};

// TODO: Create a separate structs for the shared and non shared data.
// TODO: How to support animation for shapes with two endpoints?
pub struct SwingRenderData {
    pub pipeline: wgpu::RenderPipeline,
    pub sphere_buffers: IndexedMeshBuffers,
    pub capsule_buffers: IndexedMeshBuffers,
    pub plane_buffers: IndexedMeshBuffers,
    pub bind_group1: crate::shader::swing::bind_groups::BindGroup1,
    pub spheres: Vec<crate::shader::swing::bind_groups::BindGroup2>,
    pub ovals: Vec<crate::shader::swing::bind_groups::BindGroup2>,
    pub ellipsoids: Vec<crate::shader::swing::bind_groups::BindGroup2>,
    pub capsules: Vec<crate::shader::swing::bind_groups::BindGroup2>,
    pub planes: Vec<crate::shader::swing::bind_groups::BindGroup2>,
}

// TODO: Add a &[glam::Mat4] bone world transforms parameter.
// TODO: Just recreate this every frame while animating for now.
// TODO: Figure out which objects don't need to be recreated every frame.
impl SwingRenderData {
    pub fn new(
        device: &wgpu::Device,
        bone_world_transforms_buffer: &wgpu::Buffer,
        swing_prc: Option<&SwingPrc>,
        skel: Option<&SkelData>,
    ) -> Self {
        // TODO: Share shape drawing code with skeleton rendering?
        let shader = crate::shader::swing::create_shader_module(device);
        let layout = crate::shader::swing::create_pipeline_layout(device);
        // TODO: Get the stride using encase.
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 32,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &crate::shader::swing::VertexInput::VERTEX_ATTRIBUTES,
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: crate::RGBA_COLOR_FORMAT,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let sphere_buffers = sphere_mesh_buffers(device);
        let capsule_buffers = capsule_mesh_buffers(device);
        let plane_buffers = plane_mesh_buffers(device);

        let bind_group1 = crate::shader::swing::bind_groups::BindGroup1::from_bindings(
            device,
            crate::shader::swing::bind_groups::BindGroupLayout1 {
                world_transforms: bone_world_transforms_buffer.as_entire_buffer_binding(),
            },
        );

        let spheres = swing_prc
            .map(|swing_prc| sphere_bind_groups(device, &swing_prc.spheres, skel))
            .unwrap_or_default();

        let ovals = Vec::new();
        let ellipsoids = Vec::new();

        let capsules = swing_prc
            .map(|swing_prc| capsule_bind_groups(device, &swing_prc.capsules, skel))
            .unwrap_or_default();

        let planes = swing_prc
            .map(|swing_prc| plane_bind_groups(device, &swing_prc.planes, skel))
            .unwrap_or_default();

        Self {
            pipeline,
            sphere_buffers,
            capsule_buffers,
            plane_buffers,
            bind_group1,
            spheres,
            ovals,
            ellipsoids,
            capsules,
            planes,
        }
    }
}

fn sphere_bind_groups(
    device: &wgpu::Device,
    spheres: &[Sphere],
    skel: Option<&SkelData>,
) -> Vec<crate::shader::swing::bind_groups::BindGroup2> {
    spheres
        .iter()
        .map(|s| {
            let buffer2 = device.create_uniform_buffer(
                "Swing Buffer2",
                &[crate::shader::swing::PerShape {
                    bone_indices: [bone_index(skel, s.bonename), -1, -1, -1],
                    start_transform: (glam::Mat4::from_translation(glam::Vec3::new(
                        s.cx, s.cy, s.cz,
                    )) * glam::Mat4::from_scale(glam::Vec3::splat(s.radius)))
                    .to_cols_array_2d(),
                    end_transform: glam::Mat4::IDENTITY.to_cols_array_2d(),
                    color: [0.0, 0.0, 1.0, 1.0],
                }],
            );

            crate::shader::swing::bind_groups::BindGroup2::from_bindings(
                device,
                crate::shader::swing::bind_groups::BindGroupLayout2 {
                    per_shape: buffer2.as_entire_buffer_binding(),
                },
            )
        })
        .collect()
}

fn capsule_bind_groups(
    device: &wgpu::Device,
    capsules: &[Capsule],
    skel: Option<&SkelData>,
) -> Vec<crate::shader::swing::bind_groups::BindGroup2> {
    capsules
        .iter()
        .map(|c| {
            let transform = if let Some(skel) = skel {
                // The capsule needs to be transformed to span the two bones.
                // TODO: Modify the vertex generation instead to avoid changing the cap shape.
                // TODO: This needs to support animation similar to joint transforms for bone display.
                // TODO: find a simpler way to do this.
                let start_i = bone_position(Some(&skel), c.start_bonename).unwrap();
                let end_i = bone_position(Some(&skel), c.end_bonename).unwrap();

                let start_bone_pos = glam::Mat4::from_cols_array_2d(
                    &skel
                        .calculate_world_transform(&skel.bones[start_i])
                        .unwrap(),
                )
                .col(3)
                .xyz();

                let _start_offset =
                    glam::Vec3::new(c.start_offset_x, c.start_offset_y, c.start_offset_z);

                let end_bone_pos = glam::Mat4::from_cols_array_2d(
                    &skel.calculate_world_transform(&skel.bones[end_i]).unwrap(),
                )
                .col(3)
                .xyz();

                let _end_offset = glam::Vec3::new(c.end_offset_x, c.end_offset_y, c.end_offset_z);

                // Assume the shape is along the Z-axis and has unit dimensions.
                // 1. Scale the shape along the Z-axis to have the appropriate length.
                // 2. Rotate the shape to point to both bones.
                // 3. Translate the bone in between the two bones.
                let direction = end_bone_pos - start_bone_pos;

                // TODO: Don't assume the length of the cylinder for the capsule?
                // TODO: How to include the offsets?
                let scale = 1.0 / direction.length();

                let rotation = glam::Quat::from_rotation_arc(glam::Vec3::Z, direction.normalize());
                glam::Mat4::from_translation((end_bone_pos + start_bone_pos) / 2.0)
                    * glam::Mat4::from_quat(rotation)
                    * glam::Mat4::from_scale(glam::Vec3::new(1.0, 1.0, scale))
            } else {
                glam::Mat4::IDENTITY
            };

            // TODO: Use a single transform for each bone?
            let buffer2 = device.create_uniform_buffer(
                "Swing Buffer2",
                &[crate::shader::swing::PerShape {
                    bone_indices: [-1; 4],
                    start_transform: transform.to_cols_array_2d(),
                    end_transform: glam::Mat4::IDENTITY.to_cols_array_2d(),
                    color: [1.0, 0.0, 0.0, 1.0],
                }],
            );

            crate::shader::swing::bind_groups::BindGroup2::from_bindings(
                device,
                crate::shader::swing::bind_groups::BindGroupLayout2 {
                    per_shape: buffer2.as_entire_buffer_binding(),
                },
            )
        })
        .collect()
}

fn plane_bind_groups(
    device: &wgpu::Device,
    planes: &[Plane],
    skel: Option<&SkelData>,
) -> Vec<crate::shader::swing::bind_groups::BindGroup2> {
    planes
        .iter()
        .map(|p| {
            // Assume the plane points in the direction of the positive Z-axis.
            // Rotate the plane to point in the direction (nx, ny, nz).
            // TODO: Does this correctly match the in game behavior?
            let buffer2 = device.create_uniform_buffer(
                "Swing Buffer2",
                &[crate::shader::swing::PerShape {
                    bone_indices: [bone_index(skel, p.bonename), -1, -1, -1],
                    start_transform: glam::Mat4::from_quat(glam::Quat::from_rotation_arc(
                        glam::Vec3::Z,
                        glam::Vec3::new(p.nx, p.ny, p.nz),
                    ))
                    .to_cols_array_2d(),
                    end_transform: glam::Mat4::IDENTITY.to_cols_array_2d(),
                    color: [0.0, 1.0, 0.0, 1.0],
                }],
            );

            crate::shader::swing::bind_groups::BindGroup2::from_bindings(
                device,
                crate::shader::swing::bind_groups::BindGroupLayout2 {
                    per_shape: buffer2.as_entire_buffer_binding(),
                },
            )
        })
        .collect()
}

fn bone_position(skel: Option<&SkelData>, name: Hash40) -> Option<usize> {
    skel.and_then(|skel| {
        skel.bones
            .iter()
            .position(|b| prc::hash40::hash40(&b.name.to_lowercase()) == name)
    })
}

fn bone_index(skel: Option<&SkelData>, name: Hash40) -> i32 {
    bone_position(skel, name).map(|i| i as i32).unwrap_or(-1)
}

// TODO: Is it worth making a SwingRenderer type?
pub fn draw_swing_collisions<'a>(
    render_data: &'a SwingRenderData,
    pass: &mut wgpu::RenderPass<'a>,
    swing_camera_bind_group: &'a crate::shader::swing::bind_groups::BindGroup0,
) {
    pass.set_pipeline(&render_data.pipeline);

    // Just draw most shapes as spheres for now.
    // TODO: Create vertex and index buffers for each shape.
    // TODO: Not all bind groups need to be set more than once.
    // TODO: Allow toggling rendering of certain shapes or shape types.
    draw_shapes(
        pass,
        &render_data.sphere_buffers,
        &render_data.spheres,
        &render_data.bind_group1,
        swing_camera_bind_group,
    );

    draw_shapes(
        pass,
        &render_data.sphere_buffers,
        &render_data.ovals,
        &render_data.bind_group1,
        swing_camera_bind_group,
    );

    draw_shapes(
        pass,
        &render_data.sphere_buffers,
        &render_data.ellipsoids,
        &render_data.bind_group1,
        swing_camera_bind_group,
    );

    draw_shapes(
        pass,
        &render_data.capsule_buffers,
        &render_data.capsules,
        &render_data.bind_group1,
        swing_camera_bind_group,
    );

    draw_shapes(
        pass,
        &render_data.plane_buffers,
        &render_data.planes,
        &render_data.bind_group1,
        swing_camera_bind_group,
    );
}

fn draw_shapes<'a>(
    pass: &mut wgpu::RenderPass<'a>,
    buffers: &'a IndexedMeshBuffers,
    shapes: &'a [crate::shader::swing::bind_groups::BindGroup2],
    bind_group1: &'a crate::shader::swing::bind_groups::BindGroup1,
    swing_camera_bind_group: &'a crate::shader::swing::bind_groups::BindGroup0,
) {
    buffers.set(pass);
    for bind_group2 in shapes {
        crate::shader::swing::bind_groups::set_bind_groups(
            pass,
            crate::shader::swing::bind_groups::BindGroups {
                bind_group0: swing_camera_bind_group,
                bind_group1,
                bind_group2,
            },
        );
        pass.draw_indexed(0..buffers.index_count, 0, 0..1);
    }
}
