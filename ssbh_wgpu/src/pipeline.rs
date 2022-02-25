use ssbh_data::{
    matl_data::{BlendFactor, BlendStateData, MatlEntryData},
    mesh_data::MeshObjectData,
};

// TODO: Create a function create_pipeline(mesh_object, material) -> RenderPipeline
// TODO: Could this be a method on the struct for holding mesh, matl, modl, etc?
pub fn create_pipeline(
    device: &wgpu::Device,
    render_pipeline_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    surface_format: wgpu::TextureFormat,
    mesh_object: &MeshObjectData,
    material: Option<&MatlEntryData>,
) -> wgpu::RenderPipeline {
    // Pipeline state takes most of its settings from the material.
    // The mesh object is just used for depth settings.
    // If matl parameters are not present, use fallback values.
    let _rasterizer_state_data = material
        .map(|m| m.rasterizer_states.first().map(|p| &p.data))
        .flatten();
    let blend_state_data = material
        .map(|m| m.blend_states.first().map(|p| &p.data))
        .flatten();

    // TODO: Some of these values should come from wgsl_to_wgpu
    // TODO: Get entry points from wgsl shader.
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(render_pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: "vs_main",
            // TODO: Generate this information from the structs themselves?
            buffers: &[
                wgpu::VertexBufferLayout {
                    array_stride: 3 * 16,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x4,
                        1 => Float32x4,
                        2 => Float32x4
                    ],
                },
                wgpu::VertexBufferLayout {
                    array_stride: 5 * 16,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    // TODO: Does having unique locations simplify validating this against the shader code?
                    attributes: &wgpu::vertex_attr_array![
                        3 => Float32x4,
                        4 => Float32x4,
                        5 => Float32x4,
                        6 => Float32x4,
                        7 => Float32x4
                    ],
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
        // TODO: Write tests for this and move it to its own module?
        // TODO: RasterizerState settings.
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back), // TODO: Set by blend state
            // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
            polygon_mode: wgpu::PolygonMode::Fill, // TODO: set by rasterizer state
            // Requires Features::DEPTH_CLAMPING
            // Requires Features::CONSERVATIVE_RASTERIZATION
            conservative: false,
            unclipped_depth: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float, // TODO: make this a constant?
            depth_write_enabled: !mesh_object.disable_depth_write,
            depth_compare: if mesh_object.disable_depth_test {
                wgpu::CompareFunction::Always
            } else {
                wgpu::CompareFunction::LessEqual
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
