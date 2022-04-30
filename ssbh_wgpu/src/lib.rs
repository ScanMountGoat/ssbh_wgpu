use rayon::prelude::*;
use std::path::Path;

mod pipeline;
mod shader;
mod texture;
mod uniforms;
mod vertex;

mod animation;
mod camera;
mod renderer;
mod rendermesh;

use nutexb_wgpu::NutexbFile;

pub use renderer::SsbhRenderer;
pub use rendermesh::{RenderMesh, RenderModel};
pub use shader::model::CameraTransforms;
pub use texture::{create_default_textures, load_default_cube};

use ssbh_data::prelude::*;

use crate::{pipeline::PipelineData, rendermesh::RenderMeshSharedData};

// Rgba16Float is widely supported.
// The in game format uses less precision.
pub const BLOOM_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

// Bgra8Unorm and Bgra8UnormSrgb should always be supported.
// We'll use SRGB since it's more compatible with less color format aware applications.
// This simplifies integrating with GUIs and image formats like PNG.
pub const RGBA_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

// Store the file data for a single folder.
// This helps ensure files are loaded exactly once.
// Applications can instantiate this struct directly instead of using the filesystem.
pub struct ModelFolder {
    pub folder_name: String,
    pub mesh: MeshData,
    // TODO: Should these be Result<Option<T>, E> to display error info in applications?
    // Option::None would just indicate a missing file.
    pub skel: Option<SkelData>,
    pub matl: Option<MatlData>,
    pub modl: Option<ModlData>,
    pub adj: Option<AdjData>,
    // TODO: Will a hashmap be faster for this many items?
    pub textures_by_file_name: Vec<(String, NutexbFile)>,
}

pub fn load_render_models(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    surface_format: wgpu::TextureFormat,
    models: &[ModelFolder],
    // TODO: Group textures together?
    default_textures: &[(String, wgpu::Texture)],
    stage_cube: &(wgpu::TextureView, wgpu::Sampler),
) -> Vec<RenderModel> {
    let start = std::time::Instant::now();

    let shader = crate::shader::model::create_shader_module(device);
    let layout = crate::shader::model::create_pipeline_layout(device);

    let pipeline_data = PipelineData {
        surface_format,
        layout,
        shader,
    };

    // TODO: Find a way to efficiently parallelize render mesh creation?
    let render_models: Vec<_> = models
        .iter()
        .map(|model| {
            let shared_data = RenderMeshSharedData {
                pipeline_data: &pipeline_data,
                model,
                default_textures,
                stage_cube,
            };

            rendermesh::create_render_model(device, queue, &shared_data)
        })
        .collect();

    println!(
        "Load {:?} render model(s): {:?}",
        models.len(),
        start.elapsed()
    );

    render_models
}

pub fn load_model_folders<P: AsRef<Path>>(root: P) -> Vec<ModelFolder> {
    // TODO: This could be made more robust.
    // TODO: Determine the minimum files required for a renderable model?
    // TODO: Also check for numdlb?
    let model_paths = globwalk::GlobWalkerBuilder::from_patterns(root, &["*.{numshb}"])
        .build()
        .unwrap()
        .into_iter()
        .filter_map(Result::ok);
    let start = std::time::Instant::now();
    let models: Vec<_> = model_paths
        .par_bridge()
        .filter_map(|p| {
            // TODO: Some folders don't have a numshb?
            // TODO: Can the mesh be optional?
            // TODO: Find a way to test what happens if these are None.
            let mesh = MeshData::from_file(p.path().with_extension("numshb")).ok()?;
            let skel = SkelData::from_file(p.path().with_extension("nusktb")).ok();
            let matl = MatlData::from_file(p.path().with_extension("numatb")).ok();
            let modl = ModlData::from_file(p.path().with_extension("numdlb")).ok();
            let adj = AdjData::from_file(p.path().with_extension("adjb")).ok();
            // TODO: Handle missing parent folder?
            let parent = p.path().parent().unwrap();
            let textures_by_file_name = textures_by_file_name(parent);

            let folder = parent.to_string_lossy().to_string();
            Some(ModelFolder {
                folder_name: folder,
                mesh,
                skel,
                matl,
                modl,
                adj,
                textures_by_file_name,
            })
        })
        .collect();
    println!("Load {:?} model(s): {:?}", models.len(), start.elapsed());
    models
}

fn textures_by_file_name(parent: &Path) -> Vec<(String, NutexbFile)> {
    std::fs::read_dir(parent)
        .unwrap() // TODO: Avoid unwrap?
        .par_bridge()
        .filter_map(|p| p.ok().map(|p| p.path()))
        .filter(|p| p.extension().and_then(|p| p.to_str()) == Some("nutexb"))
        .filter_map(|p| {
            Some((
                p.file_name()?.to_string_lossy().to_string(),
                NutexbFile::read_from_file(p).ok()?,
            ))
        })
        .collect()
}

// TODO: Where to put this?
// TODO: Develop tests cases based on simple quaternions like x 90, y90, etc.
pub fn calculate_light_transform() -> glam::Mat4 {
    // TODO: This should be editable when changing stages.
    // TODO: Why do we negate w?
    // TODO: Read this value from the transform for LightStg0 from light00_set.nuanmb.
    // TODO: Do translation and scale matter?
    let light_quaternion = glam::Quat::from_xyzw(-0.495286, -0.0751228, 0.0431234, -0.864401);

    // TODO: What controls the "scale" of the lighting region?
    let perspective_matrix = glam::Mat4::orthographic_rh(-50.0, 50.0, -50.0, 50.0, -50.0, 50.0);
    let model_view = glam::Mat4::from_quat(light_quaternion);

    perspective_matrix * model_view
}
