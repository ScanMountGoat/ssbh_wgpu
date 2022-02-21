use std::path::Path;

mod pipeline;
pub mod shader;
mod texture;
mod uniforms;
mod vertex;

pub mod camera;
mod renderer;
mod rendermesh;

pub use renderer::SsbhRenderer;
pub use rendermesh::RenderMesh;

use ssbh_data::{
    matl_data::MatlData, mesh_data::MeshData, modl_data::ModlData, skel_data::SkelData, SsbhData,
};

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
