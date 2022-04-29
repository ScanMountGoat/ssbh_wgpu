use wgpu::{util::DeviceExt, ComputePassDescriptor, ComputePipelineDescriptor};

use crate::{
    calculate_light_transform, camera::create_camera_bind_group, pipeline::create_depth_pipeline,
    texture::load_texture_sampler_3d, CameraTransforms, RenderModel,
};

const SHADOW_MAP_WIDTH: u32 = 512;
const SHADOW_MAP_HEIGHT: u32 = 512;

/// A renderer for drawing a collection of [RenderModel].
pub struct SsbhRenderer {
    bloom_threshold_pipeline: wgpu::RenderPipeline,
    bloom_blur_pipeline: wgpu::RenderPipeline,
    bloom_combine_pipeline: wgpu::RenderPipeline,
    bloom_upscale_pipeline: wgpu::RenderPipeline,
    post_process_pipeline: wgpu::RenderPipeline,
    skinning_pipeline: wgpu::ComputePipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    // Store camera state for efficiently updating it later.
    // This avoids exposing shader implementations like bind groups.
    camera_buffer: wgpu::Buffer,
    camera_bind_group: crate::shader::model::bind_groups::BindGroup0,

    model_shadow_bind_group: crate::shader::model::bind_groups::BindGroup3,
    shadow_transform_bind_group: crate::shader::model_depth::bind_groups::BindGroup0,
    shadow_depth: TextureSamplerView,

    pass_info: PassInfo,
    // TODO: Rework this to allow for updating the lut externally.
    // TODO: What's the easiest format to allow these updates?
    color_lut: TextureSamplerView,
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
                    targets: &[crate::RGBA_COLOR_FORMAT.into()],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        // Shared shaders for bloom passes.
        // TODO: Should this be all screen texture shaders?
        let shader = crate::shader::bloom::create_shader_module(device);
        let pipeline_layout = crate::shader::bloom::create_pipeline_layout(device);
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
                    entry_point: "fs_threshold",
                    targets: &[crate::BLOOM_COLOR_FORMAT.into()],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

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
                entry_point: "fs_blur",
                targets: &[crate::BLOOM_COLOR_FORMAT.into()],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

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
                    entry_point: "fs_upscale",
                    targets: &[crate::RGBA_COLOR_FORMAT.into()],
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

        let module = crate::shader::skinning::create_shader_module(device);
        let pipeline_layout = crate::shader::skinning::create_pipeline_layout(device);
        // TODO: Better support compute shaders in wgsl_to_wgpu.
        let skinning_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Vertex Skinning Compute"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: "main",
        });

        let shadow_pipeline = create_depth_pipeline(device);

        // TODO: Where should stage specific assets be loaded?
        let (color_lut_view, color_lut_sampler) =
            load_texture_sampler_3d(device, queue, "color_grading_lut.nutexb");
        let color_lut = TextureSamplerView {
            view: color_lut_view,
            sampler: color_lut_sampler,
        };

        // TODO: Create a struct to store the stage rendering data?
        let pass_info = PassInfo::new(device, initial_width, initial_height, &color_lut);

        let (camera_buffer, camera_bind_group) =
            create_camera_bind_group(device, glam::Vec4::ZERO, glam::Mat4::IDENTITY);

        let light_transform = calculate_light_transform();

        let shadow_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[crate::shader::model_depth::CameraTransforms {
                mvp_matrix: light_transform,
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let shadow_transform_bind_group =
            crate::shader::model_depth::bind_groups::BindGroup0::from_bindings(
                device,
                crate::shader::model_depth::bind_groups::BindGroupLayout0 {
                    camera: &shadow_buffer,
                },
            );

        // Depth from the perspective of the light.
        // TODO: Multiple lights require multiple depth maps?
        let shadow_depth = create_depth(device, SHADOW_MAP_WIDTH, SHADOW_MAP_HEIGHT);

        let model_shadow_bind_group = crate::shader::model::bind_groups::BindGroup3::from_bindings(
            device,
            crate::shader::model::bind_groups::BindGroupLayout3 {
                texture_shadow: &shadow_depth.view,
                sampler_shadow: &shadow_depth.sampler,
                light: &shadow_buffer,
            },
        );

        Self {
            bloom_threshold_pipeline,
            bloom_blur_pipeline,
            bloom_combine_pipeline,
            bloom_upscale_pipeline,
            post_process_pipeline,
            skinning_pipeline,
            shadow_pipeline,
            camera_buffer,
            camera_bind_group,
            model_shadow_bind_group,
            pass_info,
            color_lut,
            shadow_transform_bind_group,
            shadow_depth,
        }
    }

    /// A faster alternative to creating a new [SsbhRenderer] with the desired size.
    ///
    /// Prefer this method over calling [SsbhRenderer::new] with the updated dimensions.
    /// To update the camera to a potentially new aspect ratio,
    /// pass the appropriate matrix to [SsbhRenderer::update_camera].
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.pass_info = PassInfo::new(device, width, height, &self.color_lut);
    }

    /// Updates the camera transforms.
    /// This method is lightweight, so it can be called each frame if necessary in the main renderloop.
    pub fn update_camera(&mut self, queue: &wgpu::Queue, transforms: CameraTransforms) {
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[transforms]));
    }

    /// Renders the `render_meshes` to `output_view` using the standard rendering passes for Smash Ultimate.
    /// The `output_view` should have the format [crate::RGBA_COLOR_FORMAT].
    pub fn render_ssbh_passes(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        render_models: &[RenderModel],
    ) {
        // Render meshes are sorted globally rather than per folder.
        // This allows all transparent draw calls to happen after opaque draw calls.
        // TODO: How to have RenderModel own all resources but still sort RenderMesh?
        // let mut meshes: Vec<_> = render_models.iter().flat_map(|m| &m.meshes).collect();
        // meshes.sort_by_key(|m| m.render_order());

        // Skin the render meshes using a compute pass instead of in the vertex shader.
        // Compute shaders give more flexibility compared to vertex shaders.
        // Modifying the vertex buffers once avoids redundant work in later passes.
        let mut skinning_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Skinning Pass"),
        });

        skinning_pass.set_pipeline(&self.skinning_pipeline);

        for model in render_models {
            crate::rendermesh::skin_render_meshes(&model.meshes, &mut skinning_pass);
        }

        drop(skinning_pass);

        // Depth only pass for shadow maps.
        let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Shadow Pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.shadow_depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        // TODO: Use the light transforms for the "camera".
        shadow_pass.set_pipeline(&self.shadow_pipeline);
        for model in render_models {
            model.draw_render_meshes_depth(&mut shadow_pass, &self.shadow_transform_bind_group);
        }
        drop(shadow_pass);

        // TODO: Force having a color attachment for each fragment shader output in wgsl_to_wgpu?
        // Draw the models to the initial color buffer.
        // TODO: Should this pass draw to a floating point target?
        // The in game format isn't 8-bit yet.
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

        for model in render_models {
            model.draw_render_meshes(
                &mut model_pass,
                &self.camera_bind_group,
                &self.model_shadow_bind_group,
            );
        }

        drop(model_pass);

        // Extract the portions of the image that contribute to bloom.
        bloom_pass(
            encoder,
            "Bloom Threshold Pass",
            &self.bloom_threshold_pipeline,
            &self.pass_info.bloom_threshold.view,
            &self.pass_info.bloom_threshold_bind_group,
        );

        // Repeatedly downsample and blur the thresholded bloom colors.
        for (texture, bind_group0) in &self.pass_info.bloom_blur_colors {
            bloom_pass(
                encoder,
                "Bloom Blur Pass",
                &self.bloom_blur_pipeline,
                &texture.view,
                bind_group0,
            );
        }

        // Combine the bloom textures into a single texture.
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

        // Upscale with bilinear filtering to smooth the result.
        bloom_pass(
            encoder,
            "Bloom Upscale Pass",
            &self.bloom_upscale_pipeline,
            &self.pass_info.bloom_upscaled.view,
            &self.pass_info.bloom_upscale_bind_group,
        );

        // TODO: Models with _near should be drawn after bloom but before post processing?
        // TODO: How does this impact the depth buffer?

        // Combine the model and bloom contributions and apply color grading.
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

fn bloom_pass(
    encoder: &mut wgpu::CommandEncoder,
    name: &str,
    pipeline: &wgpu::RenderPipeline,
    view: &wgpu::TextureView,
    bind_group: &crate::shader::bloom::bind_groups::BindGroup0,
) {
    let mut pass = create_color_pass(encoder, view, Some(name));
    pass.set_pipeline(pipeline);
    crate::shader::bloom::bind_groups::set_bind_groups(
        &mut pass,
        crate::shader::bloom::bind_groups::BindGroups {
            bind_group0: bind_group,
        },
    );
    pass.draw(0..3, 0..1);
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

    bloom_threshold_bind_group: crate::shader::bloom::bind_groups::BindGroup0,

    bloom_blur_colors: [(
        TextureSamplerView,
        crate::shader::bloom::bind_groups::BindGroup0,
    ); 4],

    bloom_combined: TextureSamplerView,
    bloom_combine_bind_group: crate::shader::bloom_combine::bind_groups::BindGroup0,

    bloom_upscaled: TextureSamplerView,
    bloom_upscale_bind_group: crate::shader::bloom::bind_groups::BindGroup0,

    post_process_bind_group: crate::shader::post_process::bind_groups::BindGroup0,
}

impl PassInfo {
    fn new(device: &wgpu::Device, width: u32, height: u32, color_lut: &TextureSamplerView) -> Self {
        let depth = create_depth(device, width, height);

        let color = create_texture_sampler(device, width, height, crate::RGBA_COLOR_FORMAT);

        // Bloom uses successively smaller render targets to increase the blur.
        let (bloom_threshold, bloom_threshold_bind_group) = create_bloom_bind_group(
            device,
            width / 4,
            height / 4,
            &color,
            crate::BLOOM_COLOR_FORMAT,
        );
        let bloom_blur_colors =
            create_bloom_blur_bind_groups(device, width / 4, height / 4, &bloom_threshold);
        let (bloom_combined, bloom_combine_bind_group) =
            create_bloom_combine_bind_group(device, width / 4, height / 4, &bloom_blur_colors);
        // A 2x bilinear upscale smooths the overall result.
        let (bloom_upscaled, bloom_upscale_bind_group) = create_bloom_bind_group(
            device,
            width / 2,
            height / 2,
            &bloom_combined,
            crate::RGBA_COLOR_FORMAT,
        );

        let post_process_bind_group =
            create_post_process_bind_group(device, &color, &bloom_combined, color_lut);
        Self {
            depth,
            color,
            bloom_threshold,
            bloom_threshold_bind_group,
            bloom_blur_colors,
            bloom_combined,
            bloom_combine_bind_group,
            bloom_upscaled,
            bloom_upscale_bind_group,
            post_process_bind_group,
        }
    }
}

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
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        compare: Some(wgpu::CompareFunction::LessEqual),
        ..Default::default()
    });

    TextureSamplerView { view, sampler }
}

fn create_texture_sampler(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> TextureSamplerView {
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
        format,
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

// TODO: Find a way to generate this from render pass descriptions.
fn create_bloom_blur_bind_groups(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    input: &TextureSamplerView,
) -> [(
    TextureSamplerView,
    crate::shader::bloom::bind_groups::BindGroup0,
); 4] {
    // Create successively smaller images to increase the blur strength.
    // For a standard 1920x1080 window, the thresholded input is 480x270.
    // This gives sizes of 240x135 -> 120x67 -> 60x33 -> 30x16
    let create_bind_group = |width, height, input| {
        create_bloom_bind_group(device, width, height, input, crate::BLOOM_COLOR_FORMAT)
    };

    let (texture0, bind_group0) = create_bind_group(width / 2, height / 2, input);
    let (texture1, bind_group1) = create_bind_group(width / 4, height / 4, &texture0);
    let (texture2, bind_group2) = create_bind_group(width / 8, height / 8, &texture1);
    let (texture3, bind_group3) = create_bind_group(width / 16, height / 16, &texture2);

    [
        (texture0, bind_group0),
        (texture1, bind_group1),
        (texture2, bind_group2),
        (texture3, bind_group3),
    ]
}

fn create_bloom_combine_bind_group(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    bloom_inputs: &[(
        TextureSamplerView,
        crate::shader::bloom::bind_groups::BindGroup0,
    ); 4],
) -> (
    TextureSamplerView,
    crate::shader::bloom_combine::bind_groups::BindGroup0,
) {
    let texture = create_texture_sampler(device, width, height, crate::RGBA_COLOR_FORMAT);

    let bind_group = crate::shader::bloom_combine::bind_groups::BindGroup0::from_bindings(
        device,
        crate::shader::bloom_combine::bind_groups::BindGroupLayout0 {
            bloom0_texture: &bloom_inputs[0].0.view,
            bloom1_texture: &bloom_inputs[1].0.view,
            bloom2_texture: &bloom_inputs[2].0.view,
            bloom3_texture: &bloom_inputs[3].0.view,
            bloom_sampler: &bloom_inputs[0].0.sampler,
        },
    );

    (texture, bind_group)
}

fn create_bloom_bind_group(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    input: &TextureSamplerView,
    format: wgpu::TextureFormat,
) -> (
    TextureSamplerView,
    crate::shader::bloom::bind_groups::BindGroup0,
) {
    let texture = create_texture_sampler(device, width, height, format);

    let bind_group = crate::shader::bloom::bind_groups::BindGroup0::from_bindings(
        device,
        crate::shader::bloom::bind_groups::BindGroupLayout0 {
            color_texture: &input.view,
            color_sampler: &input.sampler,
        },
    );

    (texture, bind_group)
}

fn create_post_process_bind_group(
    device: &wgpu::Device,
    color_input: &TextureSamplerView,
    bloom_input: &TextureSamplerView,
    color_lut: &TextureSamplerView,
) -> crate::shader::post_process::bind_groups::BindGroup0 {
    crate::shader::post_process::bind_groups::BindGroup0::from_bindings(
        device,
        crate::shader::post_process::bind_groups::BindGroupLayout0 {
            color_texture: &color_input.view,
            color_sampler: &color_input.sampler,
            color_lut: &color_lut.view,
            color_lut_sampler: &color_lut.sampler,
            bloom_texture: &bloom_input.view,
            bloom_sampler: &bloom_input.sampler,
        },
    )
}
