use crate::{
    renderer::{DEPTH_FORMAT, MSAA_SAMPLE_COUNT},
    shape::IndexedMeshBuffers,
};

pub struct FloorGridRenderData {
    pipeline: wgpu::RenderPipeline,
    bind_group: crate::shader::floor_grid::bind_groups::BindGroup0,
    buffers: IndexedMeshBuffers,
}

impl FloorGridRenderData {
    pub fn new(
        device: &wgpu::Device,
        camera_buffer: &wgpu::Buffer,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = crate::shader::floor_grid::create_shader_module(device);
        let layout = crate::shader::floor_grid::create_pipeline_layout(device);

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    crate::shader::floor_grid::VertexInput::vertex_buffer_layout(
                        wgpu::VertexStepMode::Vertex,
                    ),
                ],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                // TODO: Why doesn't this blend properly from below?
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
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
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            // TODO: Create a constant for this?
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: MSAA_SAMPLE_COUNT,
                ..Default::default()
            },
            multiview: None,
            cache: None,
        });

        let bind_group = crate::shader::floor_grid::bind_groups::BindGroup0::from_bindings(
            device,
            crate::shader::floor_grid::bind_groups::BindGroupLayout0 {
                camera: camera_buffer.as_entire_buffer_binding(),
            },
        );

        // A quad on the XY-plane.
        let buffers = IndexedMeshBuffers::from_vertices(
            device,
            &[
                [1.0, 1.0, 0.0, 0.0],
                [-1.0, -1.0, 0.0, 0.0],
                [-1.0, 1.0, 0.0, 0.0],
                [-1.0, -1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0, 0.0],
                [1.0, -1.0, 0.0, 0.0],
            ],
            &[0, 1, 2, 3, 4, 5],
        );

        Self {
            pipeline,
            bind_group,
            buffers,
        }
    }

    // TODO: Split off more of the renderer like this?
    // TODO: Create a renderer module?
    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        pass.set_pipeline(&self.pipeline);

        crate::shader::floor_grid::set_bind_groups(pass, &self.bind_group);

        self.buffers.set(pass);

        pass.draw(0..6, 0..1);
    }
}
