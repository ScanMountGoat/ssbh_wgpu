use std::num::{NonZeroU32, NonZeroU8};

use nutexb_wgpu::NutexbFile;
use ssbh_data::matl_data::{MagFilter, MatlEntryData, MinFilter, ParamId, SamplerData, WrapMode};
use wgpu::Device;

pub fn load_texture_sampler_or_default(
    device: &Device,
    queue: &wgpu::Queue,
    material: Option<&MatlEntryData>,
    folder: &str,
    texture_id: ParamId,
    sampler_id: ParamId,
    default: [u8; 4],
    default_textures: &[(&'static str, wgpu::Texture)],
) -> (wgpu::TextureView, wgpu::Sampler) {
    load_texture_sampler(
        material,
        device,
        queue,
        folder,
        texture_id,
        sampler_id,
        default_textures,
    )
    .unwrap_or_else(|| {
        let default = default_texture_sampler_2d(device, queue, default);
        // TODO: Avoid duplicates.
        (
            default.create_view(&wgpu::TextureViewDescriptor::default()),
            device.create_sampler(&wgpu::SamplerDescriptor::default()),
        )
    })
}

fn load_texture_sampler(
    material: Option<&MatlEntryData>,
    device: &Device,
    queue: &wgpu::Queue,
    folder: &str,
    texture_id: ParamId,
    sampler_id: ParamId,
    default_textures: &[(&'static str, wgpu::Texture)],
) -> Option<(wgpu::TextureView, wgpu::Sampler)> {
    // TODO: Add proper path and parameter handling.
    // TODO: Handle missing paths.
    // TODO: Cache the texture creation?
    let material = material?;

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
    let default = default_textures.iter().find(|d| d.0 == material_path.to_lowercase());

    let view = match default {
        Some((_, texture)) => texture.create_view(&wgpu::TextureViewDescriptor::default()),
        None => {
            let absolute_path = std::path::Path::new(folder)
                .join(material_path)
                .with_extension("nutexb");
                
            // TODO: This function should return an error.
            let nutexb = NutexbFile::read_from_file(absolute_path).unwrap();
            let texture = nutexb_wgpu::get_nutexb_data(&nutexb).create_texture(device, queue);
            texture.create_view(&wgpu::TextureViewDescriptor::default())
        }
    };

    let sampler_data = material
        .samplers
        .iter()
        .find(|t| t.param_id == sampler_id)
        .map(|t| sampler_descriptor(&t.data))?;

    let sampler = device.create_sampler(&sampler_data);

    Some((view, sampler))
}

// TODO: Share code with above?
pub fn load_texture_sampler_cube_or_default(
    device: &Device,
    queue: &wgpu::Queue,
    material: Option<&MatlEntryData>,
    folder: &str,
    texture_id: ParamId,
    sampler_id: ParamId,
    _default: [u8; 4],
) -> (wgpu::TextureView, wgpu::Sampler) {
    // TODO: Create a default texture?
    load_texture_sampler_cube(material, device, queue, folder, texture_id, sampler_id).unwrap()
}

fn load_texture_sampler_cube(
    _material: Option<&MatlEntryData>,
    device: &Device,
    queue: &wgpu::Queue,
    _folder: &str,
    _texture_id: ParamId,
    _sampler_id: ParamId,
) -> Option<(wgpu::TextureView, wgpu::Sampler)> {
    // TODO: Add proper path and parameter handling.
    // TODO: Handle missing paths.
    // TODO: Cache the texture creation?
    // let material = material?;

    // TODO: Find a way to test texture path loading.
    // For example, "#replace_cubemap" needs special handling.
    // This should also handle paths like "../texture.nutexb" and "/render/shader/bin/texture.nutexb".
    // let material_path = material
    //     .textures
    //     .iter()
    //     .find(|t| t.param_id == texture_id)
    //     .map(|t| t.data.as_str())?;
    // let absolute_path = std::path::Path::new(folder)
    //     .join(material_path)
    //     .with_extension("nutexb");
    // TODO: Handle #replace_cubemap?
    // TODO: Don't hardcode this path.
    let absolute_path = std::path::Path::new("reflection_cubemap.nutexb");

    // TODO: This function should return an error.
    let nutexb = NutexbFile::read_from_file(absolute_path).unwrap();
    let texture = nutexb_wgpu::get_nutexb_data(&nutexb).create_texture(device, queue);
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    });

    // let sampler_data = material
    //     .samplers
    //     .iter()
    //     .find(|t| t.param_id == sampler_id)
    //     .map(|t| sampler_descriptor(&t.data))?;
    // let sampler = device.create_sampler(&sampler_data);
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    Some((view, sampler))
}

pub fn load_texture_sampler_3d(
    device: &Device,
    queue: &wgpu::Queue,
) -> (wgpu::TextureView, wgpu::Sampler) {
    // TODO: Don't hardcode this path.
    let absolute_path = std::path::Path::new("color_grading_lut.nutexb");

    // TODO: This function should return an error.
    let nutexb = NutexbFile::read_from_file(absolute_path).unwrap();
    let texture = nutexb_wgpu::get_nutexb_data(&nutexb).create_texture(device, queue);
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

fn sampler_descriptor(data: &SamplerData) -> wgpu::SamplerDescriptor {
    wgpu::SamplerDescriptor {
        label: None, // TODO: Set label to the param ID?
        address_mode_u: address_mode(data.wraps),
        address_mode_v: address_mode(data.wrapt),
        address_mode_w: address_mode(data.wrapr),
        mag_filter: mag_filter_mode(data.mag_filter),
        min_filter: min_filter_mode(data.min_filter),
        mipmap_filter: mip_filter_mode(data.min_filter),
        anisotropy_clamp: data
            .max_anisotropy
            .map(|m| NonZeroU8::new(m as u8).unwrap()),
        // TODO: Set other options?
        ..Default::default()
    }
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

// TODO: Do we really need samplers here?
// The sampler won't matter for solid color defaults.
// The material may be missing the texture but not the sampler.
pub fn create_default_textures(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Vec<(&'static str, wgpu::Texture)> {
    // TODO: Avoid duplicates.
    // TODO: Return a dictionary?
    vec![
        (
            "/common/shader/sfxpbs/default_black",
            default_texture_sampler_2d(device, queue, [0, 0, 0, 255]),
        ),
        (
            "/common/shader/sfxpbs/default_color",
            default_texture_sampler_2d(device, queue, [255, 255, 255, 255]),
        ),
        (
            "/common/shader/sfxpbs/default_color2",
            default_texture_sampler_2d(device, queue, [255, 255, 255, 255]),
        ),
        (
            "/common/shader/sfxpbs/default_color3",
            default_texture_sampler_2d(device, queue, [255, 255, 255, 255]),
        ),
        (
            "/common/shader/sfxpbs/default_color4",
            default_texture_sampler_2d(device, queue, [255, 255, 255, 255]),
        ),
        (
            // TODO: This is an 8x8 yellow checkerboard
            "/common/shader/sfxpbs/default_diffuse2",
            default_texture_sampler_2d(device, queue, [255, 255, 0, 255]),
        ),
        (
            "/common/shader/sfxpbs/default_gray",
            default_texture_sampler_2d(device, queue, [123, 121, 123, 255]),
        ),
        (
            "/common/shader/sfxpbs/default_metallicbg",
            default_texture_sampler_2d(device, queue, [0, 255, 255, 58]),
        ),
        (
            "/common/shader/sfxpbs/default_normal",
            default_texture_sampler_2d(device, queue, [132, 120, 255, 255]),
        ),
        (
            "/common/shader/sfxpbs/default_params",
            default_texture_sampler_2d(device, queue, [0, 255, 255, 58]),
        ),
        (
            "/common/shader/sfxpbs/default_params_r000_g025_b100",
            default_texture_sampler_2d(device, queue, [0, 65, 255, 255]),
        ),
        (
            "/common/shader/sfxpbs/default_params_r100_g025_b100",
            default_texture_sampler_2d(device, queue, [255, 65, 255, 255]),
        ),
        (
            "/common/shader/sfxpbs/default_params2",
            default_texture_sampler_2d(device, queue, [255, 255, 255, 255]),
        ),
        (
            "/common/shader/sfxpbs/default_params3",
            default_texture_sampler_2d(device, queue, [0, 117, 255, 58]),
        ),
        (
            "/common/shader/sfxpbs/default_params3",
            default_texture_sampler_2d(device, queue, [58, 61, 58, 255]),
        ),
        (
            "/common/shader/sfxpbs/default_white",
            default_texture_sampler_2d(device, queue, [255, 255, 255, 255]),
        ),
        (
            "/common/shader/sfxpbs/fighter/default_normal",
            default_texture_sampler_2d(device, queue, [128, 128, 255, 255]),
        ),
        (
            "/common/shader/sfxpbs/fighter/default_normal",
            default_texture_sampler_2d(device, queue, [0, 255, 255, 58]),
        ),
    ]
}

fn default_texture_sampler_2d(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    color: [u8; 4],
) -> wgpu::Texture {
    // TODO: It may be faster to cache these.
    let texture_size = wgpu::Extent3d {
        width: 4,
        height: 4,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
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

    texture
}

#[cfg(test)]
mod tests {
    // TODO: Add tests cases for handling of paths and special paths like "#replace_cubemap".
    #[test]
    fn replace_cubemap() {}
}
