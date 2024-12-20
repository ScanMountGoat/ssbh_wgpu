use log::warn;
pub use nutexb::NutexbFile;
use shader::bind_groups::BindGroup0;
use shader::set_bind_groups;
use shader::{create_pipeline_layout, create_shader_module};
use thiserror::Error;
use wgpu::{util::DeviceExt, Limits};
use wgpu::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    TextureViewDimension,
};

#[allow(dead_code)]
mod shader {
    include!(concat!(env!("OUT_DIR"), "/shader.rs"));
}

/// Settings to control rendering of the texture.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderSettings {
    /// Channel toggles for `[red, green, blue, alpha]`.
    pub render_rgba: [bool; 4],
    /// The mip level to render starting from `0.0`.
    pub mipmap: f32,
    /// The depth or array layer to render.
    /// Cube maps have six layers.
    /// Depth textures should take values up to the texture's depth in pixels.
    pub layer: u32,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            render_rgba: [true; 4],
            mipmap: 0.0,
            layer: 0,
        }
    }
}

fn shader_settings(
    settings: &RenderSettings,
    dim: TextureViewDimension,
    size: (u32, u32, u32),
) -> crate::shader::RenderSettings {
    crate::shader::RenderSettings {
        render_rgba: settings.render_rgba.map(|b| if b { 1.0 } else { 0.0 }),
        mipmap: [settings.mipmap; 4],
        layer: [settings.layer; 4],
        texture_slot: [match dim {
            TextureViewDimension::D2 => 0,
            TextureViewDimension::Cube => 1,
            TextureViewDimension::D3 => 2,
            _ => 0,
        }; 4],
        texture_size: [size.0 as f32, size.1 as f32, size.2 as f32, 0.0],
    }
}

/// Errors that can occur while converting a [nutexb::NutexbFile] to a [wgpu::Texture].
#[derive(Debug, Error)]
pub enum CreateTextureError {
    #[error("an error occurred while deswizzling nutexb data")]
    SwizzleError,

    #[error("one of the texture dimensions is zero")]
    ZeroSizedDimension,

    #[error("one of the texture dimensions exceeds device limits")]
    DimensionExceedsLimit,

    #[error("the texture does not specify at least one mipmap")]
    ZeroMipmapCount,

    #[error("the texture does not specify at least one layer")]
    ZeroLayers,

    #[error("the texture layer count exceeds device limits")]
    LayerCountExceedsLimit,

    #[error(
        "the texture width {} is not a multiple of the block width {}",
        width,
        block_width
    )]
    UnalignedWidth { width: u32, block_width: u32 },

    #[error(
        "the texture height {} is not a multiple of the block height {}",
        height,
        block_height
    )]
    UnalignedHeight { height: u32, block_height: u32 },
}

/// Converts `nutexb` into a texture with the same format.
///
/// sRGB and non sRGB variants of the format are available as view formats.
/// Using the texture's original format in the view is always available.
pub fn create_texture(
    nutexb: &NutexbFile,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(wgpu::Texture, wgpu::TextureViewDimension), CreateTextureError> {
    let size = wgpu::Extent3d {
        width: nutexb.footer.width,
        height: nutexb.footer.height,
        depth_or_array_layers: std::cmp::max(nutexb.footer.layer_count, nutexb.footer.depth),
    };

    // TODO: Show what dimension is zero?
    if size.width == 0 || size.height == 0 || size.depth_or_array_layers == 0 {
        return Err(CreateTextureError::ZeroSizedDimension);
    }

    if nutexb.footer.mipmap_count == 0 {
        return Err(CreateTextureError::ZeroMipmapCount);
    }

    if nutexb.footer.layer_count == 0 {
        return Err(CreateTextureError::ZeroLayers);
    }

    let max_dimension = if nutexb.footer.depth > 1 {
        Limits::default().max_texture_dimension_3d
    } else {
        Limits::default().max_texture_dimension_2d
    };

    // TODO: Show dimensions?
    if nutexb.footer.width > max_dimension
        || nutexb.footer.height > max_dimension
        || nutexb.footer.depth > max_dimension
    {
        return Err(CreateTextureError::DimensionExceedsLimit);
    }

    if nutexb.footer.layer_count > Limits::default().max_texture_array_layers {
        return Err(CreateTextureError::LayerCountExceedsLimit);
    }

    let format = wgpu_format(nutexb.footer.image_format);
    let (block_width, block_height) = format.block_dimensions();
    if size.width % block_width != 0 {
        return Err(CreateTextureError::UnalignedWidth {
            width: size.width,
            block_width,
        });
    }
    if size.height % block_height != 0 {
        return Err(CreateTextureError::UnalignedHeight {
            height: size.height,
            block_height,
        });
    }

    let dimension = if nutexb.footer.depth > 1 {
        wgpu::TextureDimension::D3
    } else {
        wgpu::TextureDimension::D2
    };

    let label = nutexb.footer.string.to_string();

    let max_mips = size.max_mips(dimension);
    if nutexb.footer.mipmap_count > max_mips {
        warn!(
            "Mipmap count {} exceeds the maximum of {} for Nutexb {:?}.",
            nutexb.footer.mipmap_count, max_mips, label,
        );
    }

    // TODO: Preserve error information?
    let data = nutexb
        .deswizzled_data()
        .map_err(|_| CreateTextureError::SwizzleError)?;

    let texture = device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some(&label),
            size,
            // TODO: Should this be an error?
            // TODO: How does in game handle this case?
            mip_level_count: std::cmp::min(nutexb.footer.mipmap_count, max_mips),
            sample_count: 1,
            dimension,
            format,
            usage: wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[format.add_srgb_suffix(), format.remove_srgb_suffix()],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        &data,
    );

    // Return the dimensions since this isn't accessible from the texture itself.
    // TODO: Are there other dimensions for nutexb?
    let dim = if nutexb.footer.depth > 1 {
        wgpu::TextureViewDimension::D3
    } else if nutexb.footer.layer_count == 6 {
        wgpu::TextureViewDimension::Cube
    } else {
        wgpu::TextureViewDimension::D2
    };

    Ok((texture, dim))
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
        nutexb::NutexbFormat::BC6Sfloat => wgpu::TextureFormat::Bc6hRgbFloat,
        nutexb::NutexbFormat::BC6Ufloat => wgpu::TextureFormat::Bc6hRgbUfloat,
        nutexb::NutexbFormat::BC7Unorm => wgpu::TextureFormat::Bc7RgbaUnorm,
        nutexb::NutexbFormat::BC7Srgb => wgpu::TextureFormat::Bc7RgbaUnormSrgb,
    }
}

/// The output format of [TextureRenderer::render_to_texture_2d_rgba].
// TODO: Does it matter if this is srgb or unorm?
pub const RGBA_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

pub struct TextureRenderer {
    pipeline: wgpu::RenderPipeline,
    rgba_pipeline: wgpu::RenderPipeline,
    settings_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
    bindgroup: Option<BindGroup0>,
    // Workaround for sharing the same pipeline.
    // Unused textures still need a resource bound.
    default_2d: wgpu::TextureView,
    default_3d: wgpu::TextureView,
    default_cube: wgpu::TextureView,
}

impl TextureRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let shader_settings = shader_settings(
            &RenderSettings::default(),
            TextureViewDimension::D2,
            (1, 1, 1),
        );
        let settings_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("nutexb_wgpu Render Settings"),
            contents: bytemuck::cast_slice(&[shader_settings]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let default_2d = default_texture_2d(device, queue);
        let default_3d = default_texture_3d(device, queue);
        let default_cube = default_texture_cube(device, queue);

        Self {
            pipeline: create_render_pipeline(device, surface_format),
            rgba_pipeline: create_render_pipeline(device, RGBA_FORMAT),
            settings_buffer,
            sampler,
            bindgroup: None,
            default_2d,
            default_3d,
            default_cube,
        }
    }

    pub fn render(&self, render_pass: &mut wgpu::RenderPass<'_>) {
        // Draw the texture to the screen.
        // TODO: How to handle cube maps and 3d textures?
        if let Some(bind_group) = &self.bindgroup {
            draw_textured_triangle(render_pass, &self.pipeline, bind_group);
        }
    }

    /// Sets the next texture to render from `texture` and `dimension`.
    /// Sets the render settings from `settings`.
    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        texture_dimension: wgpu::TextureViewDimension,
        texture_size: (u32, u32, u32),
        settings: &RenderSettings,
    ) {
        // The renderer takes an existing render pass for easier integration.
        // Store and update state in self to work around the lifetime requirements.
        let bind_group = self.create_bind_group(
            device,
            queue,
            texture,
            texture_dimension,
            texture_size,
            settings,
        );
        self.bindgroup = Some(bind_group);
    }

    /// Render a texture to a 2D RGBA texture.
    ///
    /// This allows compressed textures like BC7 to be used as thumbnails in some applications.
    /// Cube maps and 3D textures will only render a single 2D face or slice based on the render settings.
    ///
    /// The sRGB suffix is ignored to avoid overly dark textures.
    pub fn render_to_texture_2d_rgba(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        texture_dimension: wgpu::TextureViewDimension,
        texture_size: (u32, u32, u32),
        render_width: u32,
        render_height: u32,
        settings: &RenderSettings,
    ) -> wgpu::Texture {
        // TODO: Is this more efficient using compute shaders?
        let texture_bind_group = self.create_bind_group(
            device,
            queue,
            texture,
            texture_dimension,
            texture_size,
            settings,
        );

        // TODO: Support 3D.
        let rgba_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: render_width,
                height: render_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2, // TODO: Convert 3d to 2d?
            format: RGBA_FORMAT,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let rgba_texture_view = rgba_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Draw the texture to a second RGBA texture.
        // This is inefficient but tests the RGBA conversion.
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &rgba_texture_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        draw_textured_triangle(&mut render_pass, &self.rgba_pipeline, &texture_bind_group);

        drop(render_pass);

        // Ensure the texture write happens before returning the texture.
        // TODO: Reuse an existing queue to optimize converting many textures?
        // Reusing a queue requires appropriate synchronization to ensure writes complete.
        queue.submit(std::iter::once(encoder.finish()));

        rgba_texture
    }

    // TODO: Set the texture and settings separately?
    // TODO: Take texture, dim, settings instead
    // Select a pipeline based on the dim?
    fn create_bind_group(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        dimension: wgpu::TextureViewDimension,
        size: (u32, u32, u32),
        settings: &RenderSettings,
    ) -> BindGroup0 {
        // TODO: How to switch bind groups based on the dimensions?
        // Remove the sRGB suffix to match the Rgba8Unorm output format.
        // This avoid sRGB textures rendering darker than intended in thumbnails.
        // This works since we add the view formats at creation.
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(dimension),
            format: Some(texture.format().remove_srgb_suffix()),
            ..Default::default()
        });

        let shader_settings = shader_settings(settings, dimension, size);
        queue.write_buffer(
            &self.settings_buffer,
            0,
            bytemuck::cast_slice(&[shader_settings]),
        );

        // Workaround for using the same pipeline.
        // Bind all resources and just choose one at render time.
        // TODO: Add dim to render settings.
        let (t_color_2d, t_color_cube, t_color_3d) = match dimension {
            TextureViewDimension::D2 => (&view, &self.default_cube, &self.default_3d),
            TextureViewDimension::Cube => (&self.default_2d, &view, &self.default_3d),
            TextureViewDimension::D3 => (&self.default_2d, &self.default_cube, &view),
            _ => (&self.default_2d, &self.default_cube, &self.default_3d),
        };

        shader::bind_groups::BindGroup0::from_bindings(
            device,
            shader::bind_groups::BindGroupLayout0 {
                t_color_2d,
                t_color_cube,
                t_color_3d,
                s_color: &self.sampler,
                render_settings: self.settings_buffer.as_entire_buffer_binding(),
            },
        )
    }
}

fn create_render_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = create_shader_module(device);
    let render_pipeline_layout = create_pipeline_layout(device);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("nutexb_wgpu Render Pipeline"),
        layout: Some(&render_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(surface_format.into())],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

fn draw_textured_triangle(
    render_pass: &mut wgpu::RenderPass<'_>,
    pipeline: &wgpu::RenderPipeline,
    texture_bind_group: &BindGroup0,
) {
    render_pass.set_pipeline(pipeline);
    set_bind_groups(render_pass, texture_bind_group);
    render_pass.draw(0..3, 0..1);
}

fn default_texture_2d(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    let size = Extent3d {
        width: 1,
        height: 1,
        depth_or_array_layers: 1,
    };
    device
        .create_texture_with_data(
            queue,
            &TextureDescriptor {
                label: Some("nutexb_wgpu Default 2D"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8Unorm,
                usage: TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &[0; 4],
        )
        .create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2),
            ..Default::default()
        })
}

fn default_texture_3d(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    let size = Extent3d {
        width: 1,
        height: 1,
        depth_or_array_layers: 1,
    };
    device
        .create_texture_with_data(
            queue,
            &TextureDescriptor {
                label: Some("nutexb_wgpu Default 3D"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D3,
                format: TextureFormat::Rgba8Unorm,
                usage: TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &[0; 4],
        )
        .create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D3),
            ..Default::default()
        })
}

fn default_texture_cube(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    let size = Extent3d {
        width: 1,
        height: 1,
        depth_or_array_layers: 6,
    };
    device
        .create_texture_with_data(
            queue,
            &TextureDescriptor {
                label: Some("nutexb_wgpu Default Cube"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8Unorm,
                usage: TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &[0; 4 * 6],
        )
        .create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        })
}
