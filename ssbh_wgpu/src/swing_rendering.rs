use wgpu::util::DeviceExt;

use crate::uniform_buffer;

// TODO: Create a separate structs for the shared and non shared data.
pub struct SwingRenderData {
    pub pipeline: wgpu::RenderPipeline,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub bind_group1: crate::shader::swing::bind_groups::BindGroup1,
    pub bind_group2: crate::shader::swing::bind_groups::BindGroup2,
}

// TODO: Separate module for getting data from swing.prc.
// spheres: cxyz, radius
// ovals:
// ellipsoids: cxyz, rxyz, sxyz
// capsules: start_offset_xyz, end_offset_xyz, start_radius, end_radius
// planes: nxyz, distance

impl SwingRenderData {
    pub fn new(device: &wgpu::Device, bone_world_transforms: &wgpu::Buffer) -> Self {
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
                targets: &[Some(crate::RGBA_COLOR_FORMAT.into())],
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

        // TODO: This should use the swing collision data.
        // TODO: Can all shapes share a pipeline?
        let buffer2 = uniform_buffer(
            device,
            "Swing Buffer2",
            &[crate::shader::swing::PerBone {
                bone_index: [12; 4],
                transform: glam::Mat4::IDENTITY.to_cols_array_2d(),
            }],
        );

        let bind_group2 = crate::shader::swing::bind_groups::BindGroup2::from_bindings(
            device,
            crate::shader::swing::bind_groups::BindGroupLayout2 {
                per_bone: buffer2.as_entire_buffer_binding(),
            },
        );

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            bind_group1,
            bind_group2,
        }
    }
}
