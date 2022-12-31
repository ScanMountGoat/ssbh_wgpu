use ssbh_data::matl_data::{BlendFactor, BlendStateData, MatlEntryData};

use crate::renderer::INVERTED_STENCIL_MASK_STATE;

// Create some helper structs to simplify the function signatures.
pub struct PipelineData {
    pub surface_format: wgpu::TextureFormat,
    pub layout: wgpu::PipelineLayout,
    pub shader: wgpu::ShaderModule,
}

impl PipelineData {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = crate::shader::model::create_shader_module(device);
        let layout = crate::shader::model::create_pipeline_layout(device);
        Self {
            surface_format,
            layout,
            shader,
        }
    }
}

// Uniquely identify pipelines assuming a shared WGSL source.
// Depth state is set per mesh rather than per material.
// This means we can't always have one pipeline per material.
// In practice, there will usually be one pipeline per material.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct PipelineKey {
    enable_depth_write: bool,
    enable_depth_test: bool,
    blend: Option<wgpu::BlendState>,
    cull_mode: Option<wgpu::Face>,
    polygon_mode: wgpu::PolygonMode,
    alpha_to_coverage_enabled: bool,
}

impl PipelineKey {
    pub fn new(
        disable_depth_write: bool,
        disable_depth_test: bool,
        material: Option<&MatlEntryData>,
    ) -> Self {
        // Pipeline state takes most of its settings from the material.
        // The mesh object is just used for depth settings.
        // If matl parameters are not present, use fallback values.
        let rasterizer_state_data =
            material.and_then(|m| m.rasterizer_states.first().map(|p| &p.data));
        let blend_state_data = material.and_then(|m| m.blend_states.first().map(|p| &p.data));

        Self {
            enable_depth_write: !disable_depth_write,
            enable_depth_test: !disable_depth_test,
            cull_mode: rasterizer_state_data.and_then(|r| match r.cull_mode {
                ssbh_data::matl_data::CullMode::Back => Some(wgpu::Face::Back),
                ssbh_data::matl_data::CullMode::Front => Some(wgpu::Face::Front),
                ssbh_data::matl_data::CullMode::Disabled => None,
            }),
            polygon_mode: wgpu::PolygonMode::Fill, // TODO: set by rasterizer state
            blend: blend_state_data.map(blend_state),
            alpha_to_coverage_enabled: blend_state_data
                .map(|b| b.alpha_sample_to_coverage)
                .unwrap_or(false),
        }
    }

    pub fn with_material(&self, material: Option<&MatlEntryData>) -> Self {
        Self::new(!self.enable_depth_write, !self.enable_depth_test, material)
    }
}

pub fn create_pipeline(
    device: &wgpu::Device,
    pipeline_data: &PipelineData,
    pipeline_key: &PipelineKey,
) -> wgpu::RenderPipeline {
    // TODO: Some of these values should come from wgsl_to_wgpu
    // TODO: Get entry points from wgsl shader.
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&pipeline_data.layout),
        vertex: wgpu::VertexState {
            module: &pipeline_data.shader,
            entry_point: "vs_main",
            buffers: &[
                crate::shader::model::VertexInput0::vertex_buffer_layout(
                    wgpu::VertexStepMode::Vertex,
                ),
                crate::shader::model::VertexInput1::vertex_buffer_layout(
                    wgpu::VertexStepMode::Vertex,
                ),
            ],
        },
        fragment: Some(wgpu::FragmentState {
            module: &pipeline_data.shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: pipeline_data.surface_format,
                blend: pipeline_key.blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        // TODO: RasterizerState settings.
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: pipeline_key.cull_mode,
            // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
            polygon_mode: wgpu::PolygonMode::Fill, // TODO: set by rasterizer state
            conservative: false,
            unclipped_depth: false,
        },
        depth_stencil: Some(depth_stencil_state(
            pipeline_key.enable_depth_write,
            pipeline_key.enable_depth_test,
        )),
        multisample: wgpu::MultisampleState {
            // TODO: This wont look correct without multisampling?
            alpha_to_coverage_enabled: pipeline_key.alpha_to_coverage_enabled,
            ..Default::default()
        },
        multiview: None,
    })
}

pub fn create_depth_pipeline(device: &wgpu::Device) -> wgpu::RenderPipeline {
    let shader = crate::shader::model::create_shader_module(device);

    // We only need the per frame light transforms.
    let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[
            &crate::shader::model::bind_groups::BindGroup0::get_bind_group_layout(device),
        ],
        push_constant_ranges: &[],
    });

    // TODO: Some of these values should come from wgsl_to_wgpu
    // TODO: Get entry points from wgsl shader.
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline Depth"),
        layout: Some(&render_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_depth",
            buffers: &[
                crate::shader::model::VertexInput0::vertex_buffer_layout(
                    wgpu::VertexStepMode::Vertex,
                ),
                crate::shader::model::VertexInput1::vertex_buffer_layout(
                    wgpu::VertexStepMode::Vertex,
                ),
            ],
        },
        fragment: None,
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(depth_stencil_state(true, true)),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    })
}

pub fn create_invalid_shader_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    create_model_pipeline_from_entry(
        device,
        surface_format,
        "vs_main_invalid",
        "fs_invalid_shader",
        "Model Invalid Shader",
    )
}

pub fn create_selected_material_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    create_model_pipeline_from_entry(
        device,
        surface_format,
        "vs_main",
        "fs_selected_material",
        "Model Selected Material",
    )
}

pub fn create_invalid_attributes_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    create_model_pipeline_from_entry(
        device,
        surface_format,
        "vs_main_invalid",
        "fs_invalid_attributes",
        "Model Invalid Attributes",
    )
}

pub fn create_debug_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    create_model_pipeline_from_entry(device, surface_format, "vs_main", "fs_debug", "Model Debug")
}

pub fn create_silhouette_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = crate::shader::model::create_shader_module(device);
    let render_pipeline_layout = crate::shader::model::create_pipeline_layout(device);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Model Silhouette"),
        layout: Some(&render_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[
                crate::shader::model::VertexInput0::vertex_buffer_layout(
                    wgpu::VertexStepMode::Vertex,
                ),
                crate::shader::model::VertexInput1::vertex_buffer_layout(
                    wgpu::VertexStepMode::Vertex,
                ),
            ],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_solid",
            targets: &[Some(surface_format.into())],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(INVERTED_STENCIL_MASK_STATE),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    })
}

pub fn create_wireframe_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = crate::shader::model::create_shader_module(device);
    let render_pipeline_layout = crate::shader::model::create_pipeline_layout(device);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Model Wireframe"),
        layout: Some(&render_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[
                crate::shader::model::VertexInput0::vertex_buffer_layout(
                    wgpu::VertexStepMode::Vertex,
                ),
                crate::shader::model::VertexInput1::vertex_buffer_layout(
                    wgpu::VertexStepMode::Vertex,
                ),
            ],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_solid",
            targets: &[Some(surface_format.into())],
        }),
        primitive: wgpu::PrimitiveState {
            polygon_mode: wgpu::PolygonMode::Line,
            ..Default::default()
        },
        depth_stencil: Some(depth_stencil_state(true, true)),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    })
}

pub fn create_model_pipeline_from_entry(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
    vertex_entry: &str,
    entry_point: &str,
    label: &str,
) -> wgpu::RenderPipeline {
    let shader = crate::shader::model::create_shader_module(device);
    let render_pipeline_layout = crate::shader::model::create_pipeline_layout(device);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&render_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: vertex_entry,
            buffers: &[
                crate::shader::model::VertexInput0::vertex_buffer_layout(
                    wgpu::VertexStepMode::Vertex,
                ),
                crate::shader::model::VertexInput1::vertex_buffer_layout(
                    wgpu::VertexStepMode::Vertex,
                ),
            ],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point,
            targets: &[Some(surface_format.into())],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(depth_stencil_state(true, true)),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    })
}

// TODO: Move this to lib.rs?
pub fn depth_stencil_state(depth_write: bool, depth_test: bool) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: crate::renderer::DEPTH_FORMAT,
        depth_write_enabled: depth_write,
        depth_compare: if depth_test {
            wgpu::CompareFunction::LessEqual
        } else {
            wgpu::CompareFunction::Always
        },
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    }
}

pub fn create_uv_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = crate::shader::model::create_shader_module(device);
    let render_pipeline_layout = crate::shader::model::create_pipeline_layout(device);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Model UV"),
        layout: Some(&render_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_uv",
            buffers: &[
                crate::shader::model::VertexInput0::vertex_buffer_layout(
                    wgpu::VertexStepMode::Vertex,
                ),
                crate::shader::model::VertexInput1::vertex_buffer_layout(
                    wgpu::VertexStepMode::Vertex,
                ),
            ],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_uv",
            targets: &[Some(surface_format.into())],
        }),
        primitive: wgpu::PrimitiveState {
            // Use wireframe rendering to show UV edges.
            polygon_mode: wgpu::PolygonMode::Line,
            ..Default::default()
        },
        depth_stencil: Some(depth_stencil_state(true, true)),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    })
}

// TODO: These can be easily unit tested.
fn blend_state(blend_state: &BlendStateData) -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: blend_factor(blend_state.source_color),
            dst_factor: blend_factor(blend_state.destination_color),
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        },
    }
}

fn blend_factor(factor: BlendFactor) -> wgpu::BlendFactor {
    match factor {
        BlendFactor::Zero => wgpu::BlendFactor::Zero,
        BlendFactor::One => wgpu::BlendFactor::One,
        BlendFactor::SourceAlpha => wgpu::BlendFactor::SrcAlpha,
        BlendFactor::DestinationAlpha => wgpu::BlendFactor::DstAlpha,
        BlendFactor::SourceColor => wgpu::BlendFactor::Src,
        BlendFactor::DestinationColor => wgpu::BlendFactor::Dst,
        BlendFactor::OneMinusSourceAlpha => wgpu::BlendFactor::OneMinusSrcAlpha,
        BlendFactor::OneMinusDestinationAlpha => wgpu::BlendFactor::OneMinusDstAlpha,
        BlendFactor::OneMinusSourceColor => wgpu::BlendFactor::OneMinusSrc,
        BlendFactor::OneMinusDestinationColor => wgpu::BlendFactor::OneMinusDst,
        BlendFactor::SourceAlphaSaturate => wgpu::BlendFactor::SrcAlphaSaturated,
    }
}

// TODO: Add some tests?
#[cfg(test)]
mod tests {}
