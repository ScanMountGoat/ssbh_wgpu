use ssbh_data::skel_data::SkelData;
use wgpu::util::DeviceExt;

use crate::{swing::Sphere, uniform_buffer};

// TODO: Create a separate structs for the shared and non shared data.
pub struct SwingRenderData {
    pub pipeline: wgpu::RenderPipeline,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub bind_group1: crate::shader::swing::bind_groups::BindGroup1,
    // TODO: How much of the rendering code can be shared between shape types?
    pub spheres: Vec<crate::shader::swing::bind_groups::BindGroup2>,
}

impl SwingRenderData {
    pub fn new(
        device: &wgpu::Device,
        bone_world_transforms: &wgpu::Buffer,
        spheres: &[Sphere],
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
                    format: crate::RGBA_COLOR_FORMAT.into(),
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
            primitive: wgpu::PrimitiveState::default(),
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

        let spheres = spheres
            .iter()
            .map(|s| {
                let buffer2 = uniform_buffer(
                    device,
                    "Swing Buffer2",
                    &[crate::shader::swing::PerBone {
                        bone_index: [skel
                            .and_then(|skel| {
                                skel.bones
                                    .iter()
                                    .position(|b| b.name.eq_ignore_ascii_case(&s.bone_name))
                                    .map(|i| i as i32)
                            })
                            .unwrap_or(-1); 4],
                        center: [s.cx, s.cy, s.cz, 0.0],
                        radius: [s.radius; 4],
                    }],
                );

                crate::shader::swing::bind_groups::BindGroup2::from_bindings(
                    device,
                    crate::shader::swing::bind_groups::BindGroupLayout2 {
                        per_bone: buffer2.as_entire_buffer_binding(),
                    },
                )
            })
            .collect();

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            bind_group1,
            spheres,
        }
    }
}
