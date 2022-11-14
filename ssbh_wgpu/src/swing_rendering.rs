use prc::hash40::Hash40;
use ssbh_data::skel_data::SkelData;

use crate::{
    shape::{capsule_mesh_buffers, plane_mesh_buffers, sphere_mesh_buffers, IndexedMeshBuffers},
    swing::SwingPrc,
    DeviceExt2,
};

// TODO: Create a separate structs for the shared and non shared data.
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

impl SwingRenderData {
    pub fn new(
        device: &wgpu::Device,
        bone_world_transforms: &wgpu::Buffer,
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
                world_transforms: bone_world_transforms.as_entire_buffer_binding(),
            },
        );

        // Just draw most shapes as spheres for now.
        let spheres = swing_prc
            .map(|swing_prc| {
                swing_prc
                    .spheres
                    .iter()
                    .map(|s| {
                        let buffer2 = device.create_uniform_buffer(
                            "Swing Buffer2",
                            &[crate::shader::swing::PerShape {
                                bone_indices: [bone_index(skel, s.bonename), -1, -1, -1],
                                start_transform: (glam::Mat4::from_translation(glam::Vec3::new(
                                    s.cx, s.cy, s.cz,
                                )) * glam::Mat4::from_scale(glam::Vec3::splat(
                                    s.radius,
                                )))
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
            })
            .unwrap_or_default();

        let ovals = Vec::new();
        let ellipsoids = Vec::new();

        let capsules = swing_prc
            .map(|swing_prc| {
                swing_prc
                    .capsules
                    .iter()
                    .map(|c| {
                        let buffer2 = device.create_uniform_buffer(
                            "Swing Buffer2",
                            &[crate::shader::swing::PerShape {
                                bone_indices: [
                                    bone_index(skel, c.start_bonename),
                                    bone_index(skel, c.end_bonename),
                                    -1,
                                    -1,
                                ],
                                start_transform: (glam::Mat4::from_translation(glam::Vec3::new(
                                    c.start_offset_x,
                                    c.start_offset_y,
                                    c.start_offset_z,
                                )) * glam::Mat4::from_scale(glam::Vec3::splat(
                                    c.start_radius,
                                )))
                                .to_cols_array_2d(),
                                end_transform: (glam::Mat4::from_translation(glam::Vec3::new(
                                    c.end_offset_x,
                                    c.end_offset_y,
                                    c.end_offset_z,
                                )) * glam::Mat4::from_scale(glam::Vec3::splat(
                                    c.end_radius,
                                )))
                                .to_cols_array_2d(),
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
            })
            .unwrap_or_default();

        let planes = swing_prc
            .map(|swing_prc| {
                swing_prc
                    .planes
                    .iter()
                    .map(|p| {
                        // TODO: How to use the normal?
                        let buffer2 = device.create_uniform_buffer(
                            "Swing Buffer2",
                            &[crate::shader::swing::PerShape {
                                bone_indices: [bone_index(skel, p.bonename), -1, -1, -1],
                                start_transform: glam::Mat4::IDENTITY.to_cols_array_2d(),
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
            })
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

fn bone_index(skel: Option<&SkelData>, name: Hash40) -> i32 {
    skel.and_then(|skel| {
        skel.bones
            .iter()
            .position(|b| prc::hash40::hash40(&b.name.to_lowercase()) == name)
            .map(|i| i as i32)
    })
    .unwrap_or(-1)
}

// TODO: Is it worth making a SwingRenderer type?
pub fn draw_swing_collisions<'a>(
    render_data: &'a SwingRenderData,
    pass: &mut wgpu::RenderPass<'a>,
    swing_camera_bind_group: &'a crate::shader::swing::bind_groups::BindGroup0,
) {
    pass.set_pipeline(&render_data.pipeline);

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
