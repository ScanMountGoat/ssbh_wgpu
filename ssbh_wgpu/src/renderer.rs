use wgpu::{ComputePassDescriptor, ComputePipelineDescriptor};

use crate::{
    camera::create_camera_bind_group, rendermesh::RenderMesh, texture::load_texture_sampler_3d,
    CameraTransforms, RenderModel,
};

// TODO: Document the renderer.
// TODO: Add multiple animation "slots"?
// TODO: Should animations be a parameter or field?
pub struct SsbhRenderer {
    bloom_threshold_pipeline: wgpu::RenderPipeline,
    bloom_blur_pipeline: wgpu::RenderPipeline,
    bloom_combine_pipeline: wgpu::RenderPipeline,
    bloom_upscale_pipeline: wgpu::RenderPipeline,
    post_process_pipeline: wgpu::RenderPipeline,
    skinning_pipeline: wgpu::ComputePipeline,
    // Store camera state for efficiently updating it later.
    // This avoids exposing shader implementations like bind groups.
    camera_buffer: wgpu::Buffer,
    camera_bind_group: crate::shader::model::bind_groups::BindGroup0,
    pass_info: PassInfo,
}

impl SsbhRenderer {
    /// Initializes the renderer for the given dimensions.
    ///
    /// This is an expensive operation, so applications should create and reuse a single [SsbhRenderer].
    /// Use [SsbhRenderer::resize] and [SsbhRenderer::update_camera] for changing window sizes and user interaction.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        initial_width: u32,
        initial_height: u32,
    ) -> Self {
        let shader = crate::shader::post_process::create_shader_module(device);
        let pipeline_layout = crate::shader::post_process::create_pipeline_layout(device);
        let post_process_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[crate::RGBA_COLOR_FORMAT.into()], // formats can be made targets?
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        let shader = crate::shader::bloom_threshold::create_shader_module(device);
        let pipeline_layout = crate::shader::bloom_threshold::create_pipeline_layout(device);
        let bloom_threshold_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[crate::BLOOM_COLOR_FORMAT.into()], // formats can be made targets?
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        let shader = crate::shader::bloom_blur::create_shader_module(device);
        let pipeline_layout = crate::shader::bloom_blur::create_pipeline_layout(device);
        let bloom_blur_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                // TODO: Floating point target?
                module: &shader,
                entry_point: "fs_main",
                targets: &[crate::BLOOM_COLOR_FORMAT.into()], // formats can be made targets?
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let shader = crate::shader::bloom_combine::create_shader_module(device);
        let pipeline_layout = crate::shader::bloom_combine::create_pipeline_layout(device);
        let bloom_combine_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[crate::RGBA_COLOR_FORMAT.into()],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        let shader = crate::shader::bloom_upscale::create_shader_module(device);
        let pipeline_layout = crate::shader::bloom_upscale::create_pipeline_layout(device);
        let bloom_upscale_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[crate::RGBA_COLOR_FORMAT.into()],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        let module = crate::shader::skinning::create_shader_module(device);
        let pipeline_layout = crate::shader::skinning::create_pipeline_layout(device);
        // TODO: Better support compute shaders in wgsl_to_wgpu.
        let skinning_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Vertex Skinning Compute"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: "main",
        });

        let pass_info = PassInfo::new(device, queue, initial_width, initial_height);

        let (camera_buffer, camera_bind_group) =
            create_camera_bind_group(device, glam::Vec4::ZERO, glam::Mat4::IDENTITY);

        Self {
            bloom_threshold_pipeline,
            bloom_blur_pipeline,
            bloom_combine_pipeline,
            bloom_upscale_pipeline,
            post_process_pipeline,
            skinning_pipeline,
            camera_buffer,
            camera_bind_group,
            pass_info,
        }
    }

    /// A faster alternative to recreating the entire object.
    ///
    /// Prefer this method over calling [SsbhRenderer::new] with the updated dimensions.
    /// To update the camera to a potentially new aspect ratio,
    /// pass the appropriate matrix to [SsbhRenderer::update_camera].
    pub fn resize(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, width: u32, height: u32) {
        self.pass_info = PassInfo::new(device, queue, width, height);
    }

    /// Updates the camera transforms.
    /// This method is lightweight, so it can be called each frame if necessary in the main renderloop.
    pub fn update_camera(&mut self, queue: &wgpu::Queue, transforms: CameraTransforms) {
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[transforms]));
    }

    // TODO: Animation?
    /// Renders the `render_meshes` uses the standard rendering passes for Smash Ultimate.
    pub fn render_ssbh_passes(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        render_models: &[RenderModel],
    ) {
        // Render meshes are sorted globally rather than per folder.
        // This allows all transparent draw calls to happen after opaque draw calls.
        let mut meshes: Vec<_> = render_models.iter().map(|m| &m.meshes).flatten().collect();
        meshes.sort_by(|a, b| a.render_order().cmp(&b.render_order()));

        let mut skinning_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Skinning Pass"),
        });

        skinning_pass.set_pipeline(&self.skinning_pipeline);

        crate::rendermesh::skin_render_meshes(&meshes, &mut skinning_pass);

        drop(skinning_pass);

        // TODO: Force having a color attachment for each fragment shader output in wgsl_to_wgpu?
        let mut model_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Model Pass"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: &self.pass_info.color.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            }],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.pass_info.depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        crate::rendermesh::draw_render_meshes(&meshes, &mut model_pass, &self.camera_bind_group);
        drop(model_pass);

        let mut bloom_threshold_pass = create_color_pass(
            encoder,
            &self.pass_info.bloom_threshold.view,
            Some("Bloom Threshold Pass"),
        );

        bloom_threshold_pass.set_pipeline(&self.bloom_threshold_pipeline);
        crate::shader::bloom_threshold::bind_groups::set_bind_groups(
            &mut bloom_threshold_pass,
            crate::shader::bloom_threshold::bind_groups::BindGroups {
                bind_group0: &self.pass_info.bloom_threshold_bind_group,
            },
        );
        bloom_threshold_pass.draw(0..3, 0..1);
        drop(bloom_threshold_pass);

        for i in 0..4 {
            let mut bloom_blur_pass =
                create_color_pass(encoder, &self.pass_info.bloom_blur_colors[i].view, None);

            bloom_blur_pass.set_pipeline(&self.bloom_blur_pipeline);
            crate::shader::bloom_blur::bind_groups::set_bind_groups(
                &mut bloom_blur_pass,
                crate::shader::bloom_blur::bind_groups::BindGroups {
                    bind_group0: &self.pass_info.bloom_blur_bind_groups[i],
                },
            );
            bloom_blur_pass.draw(0..3, 0..1);
        }

        let mut bloom_combine_pass = create_color_pass(
            encoder,
            &self.pass_info.bloom_combined.view,
            Some("Bloom Combined Pass"),
        );

        bloom_combine_pass.set_pipeline(&self.bloom_combine_pipeline);
        crate::shader::bloom_combine::bind_groups::set_bind_groups(
            &mut bloom_combine_pass,
            crate::shader::bloom_combine::bind_groups::BindGroups {
                bind_group0: &self.pass_info.bloom_combine_bind_group,
            },
        );
        bloom_combine_pass.draw(0..3, 0..1);
        drop(bloom_combine_pass);

        let mut bloom_upscale_pass = create_color_pass(
            encoder,
            &self.pass_info.bloom_upscaled.view,
            Some("Bloom Upscale Pass"),
        );

        bloom_upscale_pass.set_pipeline(&self.bloom_upscale_pipeline);
        crate::shader::bloom_upscale::bind_groups::set_bind_groups(
            &mut bloom_upscale_pass,
            crate::shader::bloom_upscale::bind_groups::BindGroups {
                bind_group0: &self.pass_info.bloom_upscale_bind_group,
            },
        );
        bloom_upscale_pass.draw(0..3, 0..1);
        drop(bloom_upscale_pass);

        // TODO: Models with _near should be drawn after bloom but before post processing?

        let mut post_processing_pass =
            create_color_pass(encoder, output_view, Some("Post Processing Pass"));

        post_processing_pass.set_pipeline(&self.post_process_pipeline);
        crate::shader::post_process::bind_groups::set_bind_groups(
            &mut post_processing_pass,
            crate::shader::post_process::bind_groups::BindGroups {
                bind_group0: &self.pass_info.post_process_bind_group,
            },
        );
        post_processing_pass.draw(0..3, 0..1);
        drop(post_processing_pass);
    }
}

fn create_color_pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    view: &'a wgpu::TextureView,
    label: Option<&'a str>,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label,
        color_attachments: &[wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: true,
            },
        }],
        depth_stencil_attachment: None,
    })
}

struct PassInfo {
    depth: TextureSamplerView,
    color: TextureSamplerView,
    bloom_threshold: TextureSamplerView,

    bloom_threshold_bind_group: crate::shader::bloom_threshold::bind_groups::BindGroup0,

    bloom_blur_colors: [TextureSamplerView; 4],
    bloom_blur_bind_groups: [crate::shader::bloom_blur::bind_groups::BindGroup0; 4],

    bloom_combined: TextureSamplerView,
    bloom_combine_bind_group: crate::shader::bloom_combine::bind_groups::BindGroup0,

    bloom_upscaled: TextureSamplerView,
    bloom_upscale_bind_group: crate::shader::bloom_upscale::bind_groups::BindGroup0,

    post_process_bind_group: crate::shader::post_process::bind_groups::BindGroup0,
}

impl PassInfo {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, width: u32, height: u32) -> Self {
        let depth = create_depth(device, width, height);

        let color = create_color_texture(device, width, height);
        let (bloom_threshold, bloom_threshold_bind_group) =
            create_bloom_threshold_bind_group(device, width / 4, height / 4, &color);
        let (bright_blur_colors, bloom_blur_bind_groups) =
            create_bloom_blur_bind_groups(device, width / 4, height / 4, &bloom_threshold);
        let (bloom_combined, bloom_combine_bind_group) =
            create_bloom_combine_bind_group(device, width / 4, height / 4, &bright_blur_colors);
        let (bloom_upscaled, bloom_upscale_bind_group) =
            create_bloom_upscale_bind_group(device, width / 2, height / 2, &bloom_combined);

        let post_process_bind_group =
            create_post_process_bind_group(device, queue, &color, &bloom_combined);
        Self {
            depth,
            color,
            bloom_threshold,
            bloom_threshold_bind_group,
            bloom_blur_colors: bright_blur_colors,
            bloom_blur_bind_groups,
            bloom_combined,
            bloom_combine_bind_group,
            bloom_upscaled,
            bloom_upscale_bind_group,
            post_process_bind_group,
        }
    }
}

// TODO: Move this to another module?
struct TextureSamplerView {
    sampler: wgpu::Sampler,
    view: wgpu::TextureView,
}

fn create_depth(device: &wgpu::Device, width: u32, height: u32) -> TextureSamplerView {
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let desc = wgpu::TextureDescriptor {
        label: Some("depth texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: crate::DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    };
    let texture = device.create_texture(&desc);

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        compare: Some(wgpu::CompareFunction::LessEqual),
        ..Default::default()
    });

    TextureSamplerView { view, sampler }
}

// TODO: Organize this?
fn create_color_texture(device: &wgpu::Device, width: u32, height: u32) -> TextureSamplerView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("color texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: crate::RGBA_COLOR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    });

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    TextureSamplerView { view, sampler }
}

fn create_bloom_threshold_bind_group(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    input: &TextureSamplerView,
) -> (
    TextureSamplerView,
    crate::shader::bloom_threshold::bind_groups::BindGroup0,
) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("color bright texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: crate::BLOOM_COLOR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    // TODO: There are additional blur passes.
    // TODO: Refactor this to be cleaner?
    // model color -> threshold -> blur -> apply bloom
    let bind_group = crate::shader::bloom_threshold::bind_groups::BindGroup0::from_bindings(
        device,
        crate::shader::bloom_threshold::bind_groups::BindGroupLayout0 {
            color_texture: &input.view,
            color_sampler: &input.sampler,
        },
    );

    (TextureSamplerView { view, sampler }, bind_group)
}

fn create_bloom_blur_bind_groups(
    device: &wgpu::Device,
    threshold_width: u32,
    threshold_height: u32,
    input: &TextureSamplerView,
) -> (
    [TextureSamplerView; 4],
    [crate::shader::bloom_blur::bind_groups::BindGroup0; 4],
) {
    // Create successively smaller images to increase the blur strength.
    // For a standard 1920x1080 window, the input is 480x270.
    // This gives sizes of 240x135 -> 120x67 -> 60x33 -> 30x16
    let (texture0, bind_group0) =
        create_blur_data(device, threshold_width / 2, threshold_height / 2, input);
    let (texture1, bind_group1) =
        create_blur_data(device, threshold_width / 4, threshold_height / 4, &texture0);
    let (texture2, bind_group2) =
        create_blur_data(device, threshold_width / 8, threshold_height / 8, &texture1);
    let (texture3, bind_group3) = create_blur_data(
        device,
        threshold_width / 16,
        threshold_height / 16,
        &texture2,
    );

    (
        [texture0, texture1, texture2, texture3],
        [bind_group0, bind_group1, bind_group2, bind_group3],
    )
}

fn create_blur_data(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    input: &TextureSamplerView,
) -> (
    TextureSamplerView,
    crate::shader::bloom_blur::bind_groups::BindGroup0,
) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("color blur texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: crate::BLOOM_COLOR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    let bind_group = crate::shader::bloom_blur::bind_groups::BindGroup0::from_bindings(
        device,
        crate::shader::bloom_blur::bind_groups::BindGroupLayout0 {
            color_texture: &input.view,
            color_sampler: &input.sampler,
        },
    );
    (TextureSamplerView { view, sampler }, bind_group)
}

fn create_bloom_combine_bind_group(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    bloom_inputs: &[TextureSamplerView; 4],
) -> (
    TextureSamplerView,
    crate::shader::bloom_combine::bind_groups::BindGroup0,
) {
    // TODO: This creation can can be reused?
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("color blur texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: crate::RGBA_COLOR_FORMAT, // TODO: Why doesn't the game use float here?
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    let bind_group = crate::shader::bloom_combine::bind_groups::BindGroup0::from_bindings(
        device,
        crate::shader::bloom_combine::bind_groups::BindGroupLayout0 {
            bloom0_texture: &bloom_inputs[0].view,
            bloom1_texture: &bloom_inputs[1].view,
            bloom2_texture: &bloom_inputs[2].view,
            bloom3_texture: &bloom_inputs[3].view,
            // TODO: Avoid creating multiple samplers?
            bloom_sampler: &bloom_inputs[0].sampler,
        },
    );

    (TextureSamplerView { view, sampler }, bind_group)
}

fn create_bloom_upscale_bind_group(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    input: &TextureSamplerView,
) -> (
    TextureSamplerView,
    crate::shader::bloom_upscale::bind_groups::BindGroup0,
) {
    // TODO: This creation can can be reused?
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("bloom upscale texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: crate::RGBA_COLOR_FORMAT, // TODO: Why doesn't the game use float here?
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    let bind_group = crate::shader::bloom_upscale::bind_groups::BindGroup0::from_bindings(
        device,
        crate::shader::bloom_upscale::bind_groups::BindGroupLayout0 {
            color_texture: &input.view,
            color_sampler: &input.sampler,
        },
    );

    (TextureSamplerView { view, sampler }, bind_group)
}

fn create_post_process_bind_group(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    color_input: &TextureSamplerView,
    bloom_input: &TextureSamplerView,
) -> crate::shader::post_process::bind_groups::BindGroup0 {
    // TODO: Where should stage specific assets be loaded?
    let (color_lut, color_lut_sampler) = load_texture_sampler_3d(device, queue);

    crate::shader::post_process::bind_groups::BindGroup0::from_bindings(
        device,
        crate::shader::post_process::bind_groups::BindGroupLayout0 {
            color_texture: &color_input.view,
            color_sampler: &color_input.sampler,
            color_lut: &color_lut,
            color_lut_sampler: &color_lut_sampler,
            bloom_texture: &bloom_input.view,
            bloom_sampler: &bloom_input.sampler,
        },
    )
}
