pub use nutexb::NutexbFile;
use wgpu::util::DeviceExt;

// TODO: Rework this to have separate crates for the demo.
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
                    depth_or_array_layers: std::cmp::max(self.array_count, self.depth), // TODO: 3d textures?
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

pub fn get_nutexb_data(nutexb: &NutexbFile) -> NutexbImage {
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

    NutexbImage {
        width,
        height,
        depth,
        deswizzled_surface,
        texture_format,
        mipmap_count,
        array_count,
    }
}
