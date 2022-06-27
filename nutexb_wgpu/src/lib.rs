use log::warn;
pub use nutexb::NutexbFile;
use wgpu::util::DeviceExt;

pub fn create_texture(
    nutexb: &NutexbFile,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> wgpu::Texture {
    let size = wgpu::Extent3d {
        width: nutexb.footer.width,
        height: nutexb.footer.height,
        depth_or_array_layers: std::cmp::max(nutexb.footer.layer_count, nutexb.footer.depth),
    };

    let max_mips = size.max_mips();
    if nutexb.footer.mipmap_count > max_mips {
        warn!(
            "Mipmap count {} exceeds the maximum of {} for Nutexb {:?}.",
            nutexb.footer.mipmap_count, max_mips, nutexb.footer.string,
        );
    }

    device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some(&nutexb.footer.string.to_string()),
            size,
            // TODO: SHould this be an error?
            // TODO: How does in game handle this case?
            mip_level_count: std::cmp::min(nutexb.footer.mipmap_count, max_mips),
            sample_count: 1,
            dimension: if nutexb.footer.depth > 1 {
                wgpu::TextureDimension::D3
            } else {
                wgpu::TextureDimension::D2
            },
            format: wgpu_format(nutexb.footer.image_format),
            usage: wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::TEXTURE_BINDING,
        },
        &nutexb.deswizzled_data().unwrap(),
    )
}

fn wgpu_format(format: nutexb::NutexbFormat) -> wgpu::TextureFormat {
    match format {
        nutexb::NutexbFormat::R8Unorm => wgpu::TextureFormat::R8Unorm,
        nutexb::NutexbFormat::R8G8B8A8Unorm => wgpu::TextureFormat::Rgba8Unorm,
        nutexb::NutexbFormat::R8G8B8A8Srgb => wgpu::TextureFormat::Rgba8UnormSrgb,
        nutexb::NutexbFormat::B8G8R8A8Unorm => wgpu::TextureFormat::Bgra8Unorm,
        nutexb::NutexbFormat::B8G8R8A8Srgb => wgpu::TextureFormat::Bgra8UnormSrgb,
        nutexb::NutexbFormat::R32G32B32A32Float => wgpu::TextureFormat::Rgba32Float,
        nutexb::NutexbFormat::BC1Unorm => wgpu::TextureFormat::Bc1RgbaUnorm,
        nutexb::NutexbFormat::BC1Srgb => wgpu::TextureFormat::Bc1RgbaUnormSrgb,
        nutexb::NutexbFormat::BC2Unorm => wgpu::TextureFormat::Bc2RgbaUnorm,
        nutexb::NutexbFormat::BC2Srgb => wgpu::TextureFormat::Bc2RgbaUnormSrgb,
        nutexb::NutexbFormat::BC3Unorm => wgpu::TextureFormat::Bc3RgbaUnorm,
        nutexb::NutexbFormat::BC3Srgb => wgpu::TextureFormat::Bc3RgbaUnormSrgb,
        nutexb::NutexbFormat::BC4Unorm => wgpu::TextureFormat::Bc4RUnorm,
        nutexb::NutexbFormat::BC4Snorm => wgpu::TextureFormat::Bc4RSnorm,
        nutexb::NutexbFormat::BC5Unorm => wgpu::TextureFormat::Bc5RgUnorm,
        nutexb::NutexbFormat::BC5Snorm => wgpu::TextureFormat::Bc5RgSnorm,
        nutexb::NutexbFormat::BC6Sfloat => wgpu::TextureFormat::Bc6hRgbSfloat,
        nutexb::NutexbFormat::BC6Ufloat => wgpu::TextureFormat::Bc6hRgbUfloat,
        nutexb::NutexbFormat::BC7Unorm => wgpu::TextureFormat::Bc7RgbaUnorm,
        nutexb::NutexbFormat::BC7Srgb => wgpu::TextureFormat::Bc7RgbaUnormSrgb,
    }
}

/// The output format of [TextureRenderer::render_to_texture_rgba].
// TODO: Does it matter if this is srgb or unorm?
pub const RGBA_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

pub struct TextureRenderer {
    pipeline: wgpu::RenderPipeline,
    rgba_pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}

impl TextureRenderer {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("texture_bind_group_layout"),
        });

        Self {
            pipeline: create_render_pipeline(device, &layout, surface_format),
            rgba_pipeline: create_render_pipeline(device, &layout, RGBA_FORMAT),
            layout,
        }
    }

    // TODO: Add an option to not render the alpha.
    // TODO: Make the BindGroup type strongly typed using wgsl_to_wgpu?
    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        texture: &'a wgpu::BindGroup,
    ) {
        // Draw the RGBA texture to the screen.
        draw_textured_triangle(render_pass, &self.pipeline, texture);
    }

    // Convert a texture to the RGBA format using a shader.
    // This allows compressed textures like BC7 to be used as thumbnails in some applications.
    pub fn render_to_texture_rgba(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        width: u32,
        height: u32,
    ) -> wgpu::Texture {
        // TODO: Is this more efficient using compute shaders?
        let texture_bind_group = self.create_texture_bind_group(device, texture);

        let rgba_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2, // TODO: Convert 3d to 2d?
            format: RGBA_FORMAT,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
        });

        let rgba_texture_view = rgba_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Draw the texture to a second RGBA texture.
        // This is inefficient but tests the RGBA conversion.
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: &rgba_texture_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        });

        draw_textured_triangle(&mut render_pass, &self.rgba_pipeline, &texture_bind_group);

        drop(render_pass);

        // Ensure the texture write happens before returning the texture.
        // TODO: Reuse an existing queue to optimize converting many textures?
        // Reusing a queue requires appropriate synchronization to ensure writes complete.
        queue.submit(std::iter::once(encoder.finish()));

        rgba_texture
    }

    pub fn create_texture_bind_group(
        &self,
        device: &wgpu::Device,
        texture: &wgpu::Texture,
    ) -> wgpu::BindGroup {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Texture Bind Group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        })
    }
}

fn create_render_pipeline(
    device: &wgpu::Device,
    texture_bind_group_layout: &wgpu::BindGroupLayout,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
        label: Some("Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
    });
    let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts: &[texture_bind_group_layout],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&render_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[surface_format.into()],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    })
}

fn draw_textured_triangle<'a>(
    render_pass: &mut wgpu::RenderPass<'a>,
    pipeline: &'a wgpu::RenderPipeline,
    texture_bind_group: &'a wgpu::BindGroup,
) {
    render_pass.set_pipeline(pipeline);
    render_pass.set_bind_group(0, texture_bind_group, &[]);
    render_pass.draw(0..3, 0..1);
}
