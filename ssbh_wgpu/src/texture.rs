use ssbh_data::matl_data::{MagFilter, MatlEntryData, MinFilter, ParamId, WrapMode};
use std::{
    num::{NonZeroU32, NonZeroU8},
    path::Path,
};
use wgpu::{
    util::DeviceExt, Device, Queue, Sampler, SamplerDescriptor, Texture, TextureAspect,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView,
    TextureViewDescriptor, TextureViewDimension,
};

pub enum LoadTextureError {
    PathNotFound,
    DimensionMismatch {
        expected: TextureViewDimension,
        actual: TextureViewDimension,
    },
}

pub fn load_texture(
    material_path: &str,
    textures: &[(String, Texture, TextureViewDimension)],
    default_textures: &[(String, Texture, TextureViewDimension)],
    dimension: TextureViewDimension,
) -> Result<TextureView, LoadTextureError> {
    // TODO: Handle relative paths like "../texture_001"?
    // This shouldn't require an actual file system for better portability.
    let (_, t, d) = textures
        .iter()
        .chain(default_textures)
        .find(|(p, _, _)| {
            Path::new(&p)
                .with_extension("")
                .as_os_str()
                .eq_ignore_ascii_case(material_path)
        })
        .ok_or(LoadTextureError::PathNotFound)?;

    if *d == dimension {
        Ok(t.create_view(&TextureViewDescriptor {
            dimension: Some(*d),
            ..Default::default()
        }))
    } else {
        Err(LoadTextureError::DimensionMismatch {
            expected: dimension,
            actual: *d,
        })
    }
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

    let sampler = device.create_sampler(&SamplerDescriptor {
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

pub fn load_default(
    param_id: ParamId,
    stage_cube: &(Texture, Sampler),
    default_white: &Texture,
) -> TextureView {
    match param_id {
        ParamId::Texture2 | ParamId::Texture7 | ParamId::Texture8 => {
            // TODO: Diffuse cube maps seem to load a different stage cube map?
            // TODO: Investigate irradiance cube maps.
            stage_cube.0.create_view(&TextureViewDescriptor {
                dimension: Some(TextureViewDimension::Cube),
                ..Default::default()
            })
        }
        _ => default_white.create_view(&TextureViewDescriptor::default()),
    }
}

pub fn load_default_spec_cube(device: &Device, queue: &Queue) -> (Texture, Sampler) {
    let size = wgpu::Extent3d {
        width: 64,
        height: 64,
        depth_or_array_layers: 6,
    };

    let texture = device.create_texture_with_data(
        queue,
        &TextureDescriptor {
            label: Some("Default Stage Specular Cube"),
            size,
            mip_level_count: 7,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Bc6hRgbUfloat,
            usage: TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
        },
        include_bytes!("stage_cube_surface.bin"),
    );

    let sampler = device.create_sampler(&SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    (texture, sampler)
}

pub fn load_default_lut(device: &Device, queue: &wgpu::Queue) -> (TextureView, Sampler) {
    let default_lut = create_default_lut();

    let size = wgpu::Extent3d {
        width: 16,
        height: 16,
        depth_or_array_layers: 16,
    };

    let texture = device.create_texture_with_data(
        queue,
        &TextureDescriptor {
            label: Some("Default Color Grading Lut"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D3,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
        },
        &default_lut,
    );

    let view = texture.create_view(&TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D3),
        ..Default::default()
    });
    let sampler = device.create_sampler(&SamplerDescriptor {
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
) -> Vec<(String, Texture, TextureViewDimension)> {
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
        default_diffuse2(device, queue),
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
) -> (String, Texture, TextureViewDimension) {
    let texture_size = wgpu::Extent3d {
        width: 4,
        height: 4,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&TextureDescriptor {
        label: Some(label),
        size: texture_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::COPY_SRC | TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
    });
    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        bytemuck::cast_slice(&[color; 4 * 4]),
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: NonZeroU32::new(16),
            rows_per_image: None,
        },
        texture_size,
    );

    (label.to_string(), texture, TextureViewDimension::D2)
}

pub fn default_diffuse2(
    device: &Device,
    queue: &wgpu::Queue,
) -> (String, Texture, TextureViewDimension) {
    let texture_size = wgpu::Extent3d {
        width: 8,
        height: 8,
        depth_or_array_layers: 1,
    };

    // This default texture isn't a solid color, so load the actual surface from a file.
    let texture = device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some("/common/shader/sfxpbs/default_diffuse2"),
            size: texture_size,
            mip_level_count: 4,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Bc3RgbaUnorm,
            usage: TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
        },
        include_bytes!("default_diffuse2_surface.bin"),
    );

    (
        "/common/shader/sfxpbs/default_diffuse2".to_string(),
        texture,
        TextureViewDimension::D2,
    )
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
