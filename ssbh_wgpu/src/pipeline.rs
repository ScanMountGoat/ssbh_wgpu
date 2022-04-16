use ssbh_data::matl_data::{BlendFactor, BlendStateData, MatlEntryData};

pub fn create_pipeline(
    device: &wgpu::Device,
    render_pipeline_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    surface_format: wgpu::TextureFormat,
    material: Option<&MatlEntryData>,
    depth_write: bool,
    depth_test: bool,
) -> wgpu::RenderPipeline {
    // Pipeline state takes most of its settings from the material.
    // The mesh object is just used for depth settings.
    // If matl parameters are not present, use fallback values.
    let rasterizer_state_data = material.and_then(|m| m.rasterizer_states.first().map(|p| &p.data));
    let blend_state_data = material.and_then(|m| m.blend_states.first().map(|p| &p.data));

    // TODO: Some of these values should come from wgsl_to_wgpu
    // TODO: Get entry points from wgsl shader.
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(render_pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: "vs_main",
            buffers: &[
                // Assume tightly packed elements with no additional padding or alignment.
                wgpu::VertexBufferLayout {
                    array_stride: crate::shader::model::VertexInput0::SIZE_IN_BYTES,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &crate::shader::model::VertexInput0::VERTEX_ATTRIBUTES,
                },
                wgpu::VertexBufferLayout {
                    array_stride: crate::shader::model::VertexInput1::SIZE_IN_BYTES,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &crate::shader::model::VertexInput1::VERTEX_ATTRIBUTES,
                },
            ],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: "fs_main",
            // TODO: Automatically create a target for each fragment output?
            targets: &[wgpu::ColorTargetState {
                format: surface_format,
                blend: blend_state_data.map(blend_state),
                write_mask: wgpu::ColorWrites::ALL,
            }],
        }),
        // TODO: RasterizerState settings.
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: rasterizer_state_data.and_then(|r| match r.cull_mode {
                ssbh_data::matl_data::CullMode::Back => Some(wgpu::Face::Back),
                ssbh_data::matl_data::CullMode::Front => Some(wgpu::Face::Front),
                ssbh_data::matl_data::CullMode::Disabled => None,
            }),
            // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
            polygon_mode: wgpu::PolygonMode::Fill, // TODO: set by rasterizer state
            conservative: false,
            unclipped_depth: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: crate::DEPTH_FORMAT,
            depth_write_enabled: depth_write,
            depth_compare: if depth_test {
                wgpu::CompareFunction::LessEqual
            } else {
                wgpu::CompareFunction::Always
            },
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            // TODO: This wont look correct without multisampling?
            // alpha_to_coverage_enabled: blend_state_data.map(|b| b.alpha_sample_to_coverage).unwrap_or(false),
            alpha_to_coverage_enabled: false,
        },
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
        alpha: wgpu::BlendComponent::REPLACE,
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
