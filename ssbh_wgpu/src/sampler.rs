use ssbh_data::matl_data::{MagFilter, MinFilter, SamplerData, WrapMode};
use wgpu::SamplerDescriptor;

pub fn sampler_descriptor(data: &SamplerData) -> SamplerDescriptor {
    SamplerDescriptor {
        address_mode_u: address_mode(data.wraps),
        address_mode_v: address_mode(data.wrapt),
        address_mode_w: address_mode(data.wrapr),
        mag_filter: mag_filter_mode(data.mag_filter),
        min_filter: min_filter_mode(data.min_filter),
        mipmap_filter: mip_filter_mode(data.min_filter),
        anisotropy_clamp: if data.mag_filter == MagFilter::Nearest
            || data.min_filter == MinFilter::Nearest
        {
            // This must be 1 if all filter modes are not linear.
            1
        } else {
            data.max_anisotropy.map(|m| m as u16).unwrap_or(1)
        },
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

#[cfg(test)]
mod tests {
    use ssbh_data::{matl_data::MaxAnisotropy, Color4f};

    use super::*;

    #[test]
    fn sampler_anisotropy() {
        assert_eq!(
            SamplerDescriptor {
                label: None,
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToBorder,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                anisotropy_clamp: 2,
                ..Default::default()
            },
            sampler_descriptor(&SamplerData {
                wraps: WrapMode::Repeat,
                wrapt: WrapMode::ClampToEdge,
                wrapr: WrapMode::ClampToBorder,
                min_filter: MinFilter::LinearMipmapLinear,
                mag_filter: MagFilter::Linear,
                border_color: Color4f {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.0
                },
                lod_bias: 0.0,
                max_anisotropy: Some(MaxAnisotropy::Two)
            })
        )
    }

    #[test]
    fn sampler_invalid_anisotropy() {
        // Nearest filters should disable anisotropy.
        assert_eq!(
            SamplerDescriptor {
                label: None,
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::MirrorRepeat,
                address_mode_w: wgpu::AddressMode::ClampToBorder,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::FilterMode::Nearest,
                anisotropy_clamp: 1,
                ..Default::default()
            },
            sampler_descriptor(&SamplerData {
                wraps: WrapMode::Repeat,
                wrapt: WrapMode::MirroredRepeat,
                wrapr: WrapMode::ClampToBorder,
                min_filter: MinFilter::Nearest,
                mag_filter: MagFilter::Linear2,
                border_color: Color4f {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.0
                },
                lod_bias: 0.0,
                max_anisotropy: Some(MaxAnisotropy::Two)
            })
        )
    }

    #[test]
    fn sampler_no_anisotropy() {
        assert_eq!(
            SamplerDescriptor {
                label: None,
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                anisotropy_clamp: 1,
                ..Default::default()
            },
            sampler_descriptor(&SamplerData {
                wraps: WrapMode::ClampToEdge,
                wrapt: WrapMode::ClampToEdge,
                wrapr: WrapMode::ClampToEdge,
                min_filter: MinFilter::LinearMipmapLinear,
                mag_filter: MagFilter::Linear,
                border_color: Color4f {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.0
                },
                lod_bias: 0.0,
                max_anisotropy: None
            })
        )
    }
}
