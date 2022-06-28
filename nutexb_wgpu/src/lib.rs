use log::warn;
pub use nutexb::NutexbFile;
pub use shader::bind_groups::BindGroup0;
use shader::{
    bind_groups::{set_bind_groups, BindGroups},
    create_pipeline_layout, create_shader_module,
};
use wgpu::util::DeviceExt;

mod shader;

pub struct RenderSettings {
    pub render_rgba: [bool; 4],
    pub mipmap: f32,
    pub layer: f32,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            render_rgba: [true; 4],
            mipmap: 0.0,
            layer: 0.0,
        }
    }
}

impl From<&RenderSettings> for crate::shader::RenderSettings {
    fn from(settings: &RenderSettings) -> Self {
        Self {
            render_rgba: settings.render_rgba.map(|b| if b { 1.0 } else { 0.0 }),
            mipmap: [settings.mipmap; 4],
            layer: [settings.layer; 4],
        }
    }
}

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
            // TODO: Should this be an error?
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
}

impl TextureRenderer {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        Self {
            pipeline: create_render_pipeline(device, surface_format),
            rgba_pipeline: create_render_pipeline(device, RGBA_FORMAT),
        }
    }

    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        bind_group: &'a BindGroup0,
    ) {
        // Draw the RGBA texture to the screen.
        draw_textured_triangle(render_pass, &self.pipeline, bind_group);
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
        settings: &RenderSettings,
    ) -> wgpu::Texture {
        // TODO: Is this more efficient using compute shaders?
        let texture_bind_group = self.create_bind_group(device, texture, settings);

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

    // TODO: Set the texture and settings separately?
    pub fn create_bind_group(
        &self,
        device: &wgpu::Device,
        texture: &wgpu::Texture,
        settings: &RenderSettings,
    ) -> BindGroup0 {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let shader_settings = crate::shader::RenderSettings::from(settings);
        let settings_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Bone Transforms Buffer"),
            contents: bytemuck::cast_slice(&[shader_settings]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        shader::bind_groups::BindGroup0::from_bindings(
            device,
            shader::bind_groups::BindGroupLayout0 {
                t_diffuse: &view,
                s_diffuse: &sampler,
                render_settings: settings_buffer.as_entire_buffer_binding(),
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
    texture_bind_group: &'a BindGroup0,
) {
    render_pass.set_pipeline(pipeline);
    set_bind_groups(
        render_pass,
        BindGroups {
            bind_group0: texture_bind_group,
        },
    );
    render_pass.draw(0..3, 0..1);
}
