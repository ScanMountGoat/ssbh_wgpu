use prc::hash40::Hash40;
use ssbh_data::skel_data::SkelData;
use wgpu::util::DeviceExt;

use crate::{swing::SwingPrc, DeviceExt2};

// TODO: Create a separate structs for the shared and non shared data.
pub struct SwingRenderData {
    pub pipeline: wgpu::RenderPipeline,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub bind_group1: crate::shader::swing::bind_groups::BindGroup1,
    // TODO: How to select the buffer type for each shape?
    // TODO: Make this Vec<(ShapeType, BindGroup2)> so we can swap buffers for rendering?
    pub shapes: Vec<crate::shader::swing::bind_groups::BindGroup2>,
}

// TODO: Add rendering for other types.

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
                    array_stride: 24,
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

        // TODO: Dedicated module for creating shape primitives.
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Swing Vertex Buffer"),
            contents: bytemuck::cast_slice(&crate::bone_rendering::sphere()),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Swing Index Buffer"),
            contents: bytemuck::cast_slice(&crate::bone_rendering::sphere_indices()),
            usage: wgpu::BufferUsages::INDEX,
        });

        let bind_group1 = crate::shader::swing::bind_groups::BindGroup1::from_bindings(
            device,
            crate::shader::swing::bind_groups::BindGroupLayout1 {
                world_transforms: bone_world_transforms.as_entire_buffer_binding(),
            },
        );

        // TODO: These should be split into separate vectors for each shape type.
        // Just draw everything as spheres for now.
        let shapes = swing_prc
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
                    .chain(swing_prc.capsules.iter().map(|c| {
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
                    }))
                    .chain(swing_prc.planes.iter().map(|p| {
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
                    }))
                    .collect()
            })
            .unwrap_or_default();

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            bind_group1,
            shapes,
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
