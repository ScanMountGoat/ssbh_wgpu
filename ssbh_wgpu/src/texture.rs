use std::{
    num::{NonZeroU32, NonZeroU8},
    path::Path,
};

use log::warn;
use ssbh_data::matl_data::{MagFilter, MatlEntryData, MinFilter, ParamId, WrapMode};
use wgpu::{util::DeviceExt, Device, Queue, Sampler, Texture, TextureView, TextureViewDescriptor};

pub fn load_texture(
    material: &MatlEntryData,
    texture_id: ParamId,
    textures: &[(String, Texture)],
    default_textures: &[(String, Texture)],
) -> Option<TextureView> {
    // TODO: Add proper path and parameter handling.
    // TODO: Handle missing paths.
    // TODO: Find a way to test texture path loading.
    // For example, "#replace_cubemap" needs special handling.
    // This should also handle paths like "../texture.nutexb" and "/render/shader/bin/texture.nutexb".
    let material_path = material
        .textures
        .iter()
        .find(|t| t.param_id == texture_id)
        .map(|t| t.data.as_str())?;

    // TODO: Find a cleaner way to handle default textures.
    // TODO: This check shouldn't be case sensitive?
    let default = default_textures
        .iter()
        .find(|d| d.0.eq_ignore_ascii_case(material_path));

    let view = match default {
        Some((_, texture)) => texture.create_view(&TextureViewDescriptor::default()),
        None => {
            // TODO: Handle relative paths like "../texture_001"?
            // This shouldn't require an actual file system for better portability.
            // TODO: This function should return an error.
            // TODO: Case sensitive?
            textures
                .iter()
                .find(|(p, _)| {
                    Path::new(&p)
                        .with_extension("")
                        .as_os_str()
                        .eq_ignore_ascii_case(material_path)
                })
                .map(|(_, t)| t)
                .unwrap_or_else(|| {
                    // TODO: Is the default in game for missing textures always white (check cube maps)?
                    // TODO: Does changing the default white texture change the "missing" texture?
                    warn!("Invalid path {:?} assigned to {} for material {:?}. Applying default texture.", material_path, texture_id, material.material_label);
                    &default_textures
                        .iter()
                        .find(|d| d.0 == "/common/shader/sfxpbs/default_white")
                        .unwrap()
                        .1
                })
                .create_view(&TextureViewDescriptor::default())
        }
    };

    Some(view)
}

pub fn load_sampler(
    material: &MatlEntryData,
    device: &Device,
    sampler_id: ParamId,
) -> Option<Sampler> {
    let sampler = material
        .samplers
        .iter()
        .find(|t| t.param_id == sampler_id)?;

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some(&sampler.param_id.to_string()),
        address_mode_u: address_mode(sampler.data.wraps),
        address_mode_v: address_mode(sampler.data.wrapt),
        address_mode_w: address_mode(sampler.data.wrapr),
        mag_filter: mag_filter_mode(sampler.data.mag_filter),
        min_filter: min_filter_mode(sampler.data.min_filter),
        mipmap_filter: mip_filter_mode(sampler.data.min_filter),
        anisotropy_clamp: sampler
            .data
            .max_anisotropy
            .and_then(|m| NonZeroU8::new(m as u8)),
        // TODO: Set other options?
        ..Default::default()
    });

    Some(sampler)
}

pub fn load_default_cube(device: &Device, queue: &Queue) -> (TextureView, Sampler) {
    let size = wgpu::Extent3d {
        width: 64,
        height: 64,
        depth_or_array_layers: 6,
    };

    let texture = device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some("Default Stage Specular Cube"),
            size,
            mip_level_count: 7,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bc6hRgbUfloat,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        },
        include_bytes!("stage_cube_surface.bin"),
    );

    let view = texture.create_view(&TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    (view, sampler)
}

pub fn load_default_lut(
    device: &Device,
    queue: &wgpu::Queue,
) -> (wgpu::TextureView, wgpu::Sampler) {
    let default_lut = create_default_lut();

    let size = wgpu::Extent3d {
        width: 16,
        height: 16,
        depth_or_array_layers: 16,
    };

    let texture = device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some("Default Color Grading Lut"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        },
        &default_lut,
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D3),
        ..Default::default()
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    (view, sampler)
}

fn mip_filter_mode(filter: MinFilter) -> wgpu::FilterMode {
    // wgpu separates the min filter and mipmap filter.
    match filter {
        MinFilter::Nearest => wgpu::FilterMode::Nearest,
        MinFilter::LinearMipmapLinear => wgpu::FilterMode::Linear,
        MinFilter::LinearMipmapLinear2 => wgpu::FilterMode::Linear,
    }
}

fn min_filter_mode(filter: MinFilter) -> wgpu::FilterMode {
    match filter {
        MinFilter::Nearest => wgpu::FilterMode::Nearest,
        MinFilter::LinearMipmapLinear => wgpu::FilterMode::Linear,
        MinFilter::LinearMipmapLinear2 => wgpu::FilterMode::Linear,
    }
}

fn mag_filter_mode(filter: MagFilter) -> wgpu::FilterMode {
    match filter {
        MagFilter::Nearest => wgpu::FilterMode::Nearest,
        MagFilter::Linear => wgpu::FilterMode::Linear,
        MagFilter::Linear2 => wgpu::FilterMode::Linear,
    }
}

fn address_mode(wrap_mode: WrapMode) -> wgpu::AddressMode {
    match wrap_mode {
        WrapMode::Repeat => wgpu::AddressMode::Repeat,
        WrapMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
        WrapMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
        WrapMode::ClampToBorder => wgpu::AddressMode::ClampToBorder,
    }
}

pub fn create_default_textures(
    device: &Device,
    queue: &wgpu::Queue,
) -> Vec<(String, wgpu::Texture)> {
    // TODO: Return a dictionary?
    vec![
        solid_color_texture_2d(
            device,
            queue,
            [0, 0, 0, 255],
            "/common/shader/sfxpbs/default_black",
        ),
        solid_color_texture_2d(
            device,
            queue,
            [255, 255, 255, 255],
            "/common/shader/sfxpbs/default_color",
        ),
        solid_color_texture_2d(
            device,
            queue,
            [255, 255, 255, 255],
            "/common/shader/sfxpbs/default_color2",
        ),
        solid_color_texture_2d(
            device,
            queue,
            [255, 255, 255, 255],
            "/common/shader/sfxpbs/default_color3",
        ),
        solid_color_texture_2d(
            device,
            queue,
            [255, 255, 255, 255],
            "/common/shader/sfxpbs/default_color4",
        ),
        // TODO: This is an 8x8 yellow checkerboard
        solid_color_texture_2d(
            device,
            queue,
            [255, 255, 0, 255],
            "/common/shader/sfxpbs/default_diffuse2",
        ),
        solid_color_texture_2d(
            device,
            queue,
            [123, 121, 123, 255],
            "/common/shader/sfxpbs/default_gray",
        ),
        solid_color_texture_2d(
            device,
            queue,
            [0, 255, 255, 58],
            "/common/shader/sfxpbs/default_metallicbg",
        ),
        solid_color_texture_2d(
            device,
            queue,
            [132, 120, 255, 255],
            "/common/shader/sfxpbs/default_normal",
        ),
        solid_color_texture_2d(
            device,
            queue,
            [0, 255, 255, 58],
            "/common/shader/sfxpbs/default_params",
        ),
        solid_color_texture_2d(
            device,
            queue,
            [0, 65, 255, 255],
            "/common/shader/sfxpbs/default_params_r000_g025_b100",
        ),
        solid_color_texture_2d(
            device,
            queue,
            [255, 65, 255, 255],
            "/common/shader/sfxpbs/default_params_r100_g025_b100",
        ),
        solid_color_texture_2d(
            device,
            queue,
            [255, 255, 255, 255],
            "/common/shader/sfxpbs/default_params2",
        ),
        solid_color_texture_2d(
            device,
            queue,
            [0, 117, 255, 58],
            "/common/shader/sfxpbs/default_params3",
        ),
        solid_color_texture_2d(
            device,
            queue,
            [58, 61, 58, 255],
            "/common/shader/sfxpbs/default_params3",
        ),
        solid_color_texture_2d(
            device,
            queue,
            [255, 255, 255, 255],
            "/common/shader/sfxpbs/default_white",
        ),
        solid_color_texture_2d(
            device,
            queue,
            [128, 128, 255, 255],
            "/common/shader/sfxpbs/fighter/default_normal",
        ),
        solid_color_texture_2d(
            device,
            queue,
            [0, 255, 255, 58],
            "/common/shader/sfxpbs/fighter/default_params",
        ),
    ]
}

pub fn solid_color_texture_2d(
    device: &Device,
    queue: &wgpu::Queue,
    color: [u8; 4],
    label: &str,
) -> (String, wgpu::Texture) {
    // TODO: It may be faster to cache these.
    let texture_size = wgpu::Extent3d {
        width: 4,
        height: 4,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: texture_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::TEXTURE_BINDING,
    });
    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        bytemuck::cast_slice(&[color; 4 * 4]),
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: NonZeroU32::new(16),
            rows_per_image: None,
        },
        texture_size,
    );

    (label.to_string(), texture)
}

fn create_default_lut() -> Vec<u8> {
    // Create a 16x16x16 RGB LUT used as the default stage LUT.
    // This applies a subtle contrast/saturation adjustment.
    let gradient_values = [
        0u8, 15u8, 30u8, 46u8, 64u8, 82u8, 101u8, 121u8, 140u8, 158u8, 176u8, 193u8, 209u8, 224u8,
        240u8, 255u8,
    ];

    let bpp = 4;
    let mut result = vec![0u8; 16 * 16 * 16 * bpp];
    for z in 0..16 {
        for y in 0..16 {
            for x in 0..16 {
                let offset = (z * 16 * 16 + y * 16 + x) * bpp;
                result[offset] = gradient_values[x];
                result[offset + 1] = gradient_values[y];
                result[offset + 2] = gradient_values[z];
                result[offset + 3] = 255u8;
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    // TODO: Add tests cases for handling of paths and special paths like "#replace_cubemap".
    #[test]
    fn replace_cubemap() {}
}
