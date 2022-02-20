use std::path::Path;

mod pipeline;
pub mod shader;
mod texture;
mod uniforms;
mod vertex;

pub mod camera;
pub mod rendermesh;

use ssbh_data::{
    matl_data::MatlData, mesh_data::MeshData, modl_data::ModlData, skel_data::SkelData, SsbhData,
};
use texture::load_texture_sampler_3d;

// Rgba16Float is widely supported.
// The in game format uses less precision.
pub const BLOOM_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

// Bgra8Unorm and Bgra8UnormSrgb should always be supported.
pub const RGBA_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub struct ModelFolder {
    pub name: String,
    pub mesh: MeshData,
    pub skel: Option<SkelData>,
    pub matl: Option<MatlData>,
    pub modl: Option<ModlData>,
}

pub fn load_model_folders(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    surface_format: wgpu::TextureFormat,
    models: &[ModelFolder],
) -> Vec<rendermesh::RenderMesh> {
    // TODO: Not all models can reuse the same pipeline?
    // TODO: Use wgsl_to_wgpu to automate this?

    // TODO: Move this into library code and use with egui?
    // TODO: Not all of this needs to be recreated each frame?
    let shader = crate::shader::model::create_shader_module(device);

    // TODO: Reuse this for all pipelines?
    // TODO: Should the camera go in the push constants?
    let render_pipeline_layout = crate::shader::model::create_pipeline_layout(device);

    let start = std::time::Instant::now();
    let render_meshes: Vec<_> = models
        .iter()
        .map(|model| {
            rendermesh::create_render_meshes(
                device,
                queue,
                &render_pipeline_layout,
                &shader,
                surface_format,
                &model.name,
                &model.mesh,
                &model.skel,
                &model.matl,
                &model.modl,
            )
        })
        .flatten()
        .collect();
    println!(
        "Create {:?} render meshes: {:?}",
        render_meshes.len(),
        start.elapsed()
    );
    render_meshes
}

pub fn load_models<P: AsRef<Path>>(folder: P) -> Vec<ModelFolder> {
    // TODO: Load files in parallel.
    // TODO: This should just walk directories?

    // TODO: This could be made more robust.
    // TODO: Determine the minimum files required for a renderable model?

    let model_paths = globwalk::GlobWalkerBuilder::from_patterns(folder, &["*.{numshb}"])
        .build()
        .unwrap()
        .into_iter()
        .filter_map(Result::ok);
    let start = std::time::Instant::now();
    let models: Vec<_> = model_paths
        .filter_map(|p| {
            // TODO: Some folders don't have a numshb?
            // TODO: Can the mesh be optional?
            let mesh = MeshData::from_file(p.path().with_extension("numshb")).ok()?;
            let skel = SkelData::from_file(p.path().with_extension("nusktb")).ok();
            let matl = MatlData::from_file(p.path().with_extension("numatb")).ok();
            let modl = ModlData::from_file(p.path().with_extension("numdlb")).ok();

            let folder = p.path().parent().unwrap().to_str().unwrap().to_string();
            Some(ModelFolder {
                name: folder,
                mesh,
                skel,
                matl,
                modl,
            })
        })
        .collect();
    println!("Load {:?} model(s): {:?}", models.len(), start.elapsed());
    models
}

// TODO: Move this to another module?
pub struct TextureSamplerView {
    pub texture: wgpu::Texture,
    pub sampler: wgpu::Sampler,
    pub view: wgpu::TextureView,
}

pub fn create_depth(
    device: &wgpu::Device,
    size: winit::dpi::PhysicalSize<u32>,
) -> TextureSamplerView {
    let size = wgpu::Extent3d {
        width: size.width,
        height: size.height,
        depth_or_array_layers: 1,
    };
    let desc = wgpu::TextureDescriptor {
        label: Some("depth texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    };
    let texture = device.create_texture(&desc);

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        compare: Some(wgpu::CompareFunction::LessEqual),
        ..Default::default()
    });

    TextureSamplerView {
        texture,
        view,
        sampler,
    }
}

// TODO: Organize this?
pub fn create_color_texture(
    device: &wgpu::Device,
    size: winit::dpi::PhysicalSize<u32>,
) -> TextureSamplerView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("color texture"),
        size: wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: RGBA_COLOR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    });

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    TextureSamplerView {
        texture,
        view,
        sampler,
    }
}

pub fn create_bloom_threshold_bind_group(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    input: &TextureSamplerView,
) -> (
    TextureSamplerView,
    crate::shader::bloom_threshold::bind_groups::BindGroup0,
) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("color bright texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: BLOOM_COLOR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    // TODO: There are additional blur passes.
    // TODO: Refactor this to be cleaner?
    // model color -> threshold -> blur -> apply bloom
    let bind_group = crate::shader::bloom_threshold::bind_groups::BindGroup0::from_bindings(
        device,
        crate::shader::bloom_threshold::bind_groups::BindGroupLayout0 {
            color_texture: &input.view,
            color_sampler: &input.sampler,
        },
    );

    (
        TextureSamplerView {
            texture,
            view,
            sampler,
        },
        bind_group,
    )
}

pub fn create_bloom_blur_bind_groups(
    device: &wgpu::Device,
    threshold_width: u32,
    threshold_height: u32,
    input: &TextureSamplerView,
) -> (
    [TextureSamplerView; 4],
    [crate::shader::bloom_blur::bind_groups::BindGroup0; 4],
) {
    // Create successively smaller images to increase the blur strength.
    // For a standard 1920x1080 window, the input is 480x270.
    // This gives sizes of 240x135 -> 120x67 -> 60x33 -> 30x16
    let (texture0, bind_group0) =
        create_blur_data(device, threshold_width / 2, threshold_height / 2, input);
    let (texture1, bind_group1) =
        create_blur_data(device, threshold_width / 4, threshold_height / 4, &texture0);
    let (texture2, bind_group2) =
        create_blur_data(device, threshold_width / 8, threshold_height / 8, &texture1);
    let (texture3, bind_group3) = create_blur_data(
        device,
        threshold_width / 16,
        threshold_height / 16,
        &texture2,
    );

    (
        [texture0, texture1, texture2, texture3],
        [bind_group0, bind_group1, bind_group2, bind_group3],
    )
}

fn create_blur_data(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    input: &TextureSamplerView,
) -> (
    TextureSamplerView,
    shader::bloom_blur::bind_groups::BindGroup0,
) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("color blur texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: BLOOM_COLOR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    let bind_group = crate::shader::bloom_blur::bind_groups::BindGroup0::from_bindings(
        device,
        crate::shader::bloom_blur::bind_groups::BindGroupLayout0 {
            color_texture: &input.view,
            color_sampler: &input.sampler,
        },
    );
    (
        TextureSamplerView {
            texture,
            view,
            sampler,
        },
        bind_group,
    )
}

pub fn create_bloom_combine_bind_group(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    bloom_inputs: &[TextureSamplerView; 4],
) -> (
    TextureSamplerView,
    crate::shader::bloom_combine::bind_groups::BindGroup0,
) {
    // TODO: This creation can can be reused?
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("color blur texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: RGBA_COLOR_FORMAT, // TODO: Why doesn't the game use float here?
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    let bind_group = crate::shader::bloom_combine::bind_groups::BindGroup0::from_bindings(
        device,
        crate::shader::bloom_combine::bind_groups::BindGroupLayout0 {
            bloom0_texture: &bloom_inputs[0].view,
            bloom1_texture: &bloom_inputs[1].view,
            bloom2_texture: &bloom_inputs[2].view,
            bloom3_texture: &bloom_inputs[3].view,
            // TODO: Avoid creating multiple samplers?
            bloom_sampler: &bloom_inputs[0].sampler,
        },
    );

    (
        TextureSamplerView {
            texture,
            view,
            sampler,
        },
        bind_group,
    )
}

pub fn create_bloom_upscale_bind_group(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    input: &TextureSamplerView,
) -> (
    TextureSamplerView,
    crate::shader::bloom_upscale::bind_groups::BindGroup0,
) {
    // TODO: This creation can can be reused?
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("bloom upscale texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: RGBA_COLOR_FORMAT, // TODO: Why doesn't the game use float here?
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    let bind_group = crate::shader::bloom_upscale::bind_groups::BindGroup0::from_bindings(
        device,
        crate::shader::bloom_upscale::bind_groups::BindGroupLayout0 {
            color_texture: &input.view,
            color_sampler: &input.sampler,
        },
    );

    (
        TextureSamplerView {
            texture,
            view,
            sampler,
        },
        bind_group,
    )
}

pub fn create_post_process_bind_group(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    color_input: &TextureSamplerView,
    bloom_input: &TextureSamplerView,
) -> crate::shader::post_process::bind_groups::BindGroup0 {
    // TODO: Where should stage specific assets be loaded?
    let (color_lut, color_lut_sampler) = load_texture_sampler_3d(device, queue);

    crate::shader::post_process::bind_groups::BindGroup0::from_bindings(
        device,
        crate::shader::post_process::bind_groups::BindGroupLayout0 {
            color_texture: &color_input.view,
            color_sampler: &color_input.sampler,
            color_lut: &color_lut,
            color_lut_sampler: &color_lut_sampler,
            bloom_texture: &bloom_input.view,
            bloom_sampler: &bloom_input.sampler,
        },
    )
}
