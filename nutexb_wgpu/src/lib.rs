pub use nutexb::NutexbFile;
use wgpu::util::DeviceExt;

// Create a type to make converting to wgpu textures easier.
// TODO: Does this need to be public and/or have public fields?
pub struct NutexbImage {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub mipmap_count: u32,
    pub array_count: u32,
    pub deswizzled_surface: Vec<u8>,
    pub texture_format: wgpu::TextureFormat,
}

impl NutexbImage {
    pub fn create_texture(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::Texture {
        device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: self.width,
                    height: self.height,
                    depth_or_array_layers: std::cmp::max(self.array_count, self.depth),
                },
                mip_level_count: self.mipmap_count,
                sample_count: 1,
                dimension: if self.depth > 1 {
                    wgpu::TextureDimension::D3
                } else {
                    wgpu::TextureDimension::D2
                },
                format: self.texture_format,
                usage: wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::TEXTURE_BINDING,
            },
            &self.deswizzled_surface,
        )
    }
}

impl From<&NutexbFile> for NutexbImage {
    fn from(nutexb: &NutexbFile) -> Self {
        let width = nutexb.footer.width;
        let height = nutexb.footer.height;
        let depth = nutexb.footer.depth;
        let mipmap_count = nutexb.footer.mipmap_count;
        let array_count = nutexb.footer.layer_count;

        let deswizzled_surface = nutexb.deswizzled_data().unwrap();

        let texture_format = match nutexb.footer.image_format {
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
        };

        Self {
            width,
            height,
            depth,
            deswizzled_surface,
            texture_format,
            mipmap_count,
            array_count,
        }
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
    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        texture: &wgpu::BindGroup,
    ) {
        // Draw the RGBA texture to the screen.
        draw_textured_triangle(encoder, output_view, &self.pipeline, texture);
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
        let texture_bind_group = self.create_texture_bind_group(&device, texture);

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

        draw_textured_triangle(
            &mut encoder,
            &rgba_texture_view,
            &self.rgba_pipeline,
            &texture_bind_group,
        );

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

fn draw_textured_triangle(
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    texture_bind_group: &wgpu::BindGroup,
) {
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Render Pass"),
        color_attachments: &[wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: true,
            },
        }],
        depth_stencil_attachment: None,
    });

    render_pass.set_pipeline(pipeline);
    render_pass.set_bind_group(0, texture_bind_group, &[]);
    render_pass.draw(0..3, 0..1);
}
