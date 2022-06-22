use log::info;
use nutexb_wgpu::NutexbFile;
use rayon::prelude::*;
use std::path::{Path, PathBuf};

mod pipeline;
mod shader;
mod texture;
mod uniforms;
mod vertex;

pub mod animation;
mod bone_rendering;
mod lighting;
mod renderer;
mod rendermesh;
mod shader_database;

pub use renderer::SsbhRenderer;
pub use rendermesh::{RenderMesh, RenderModel};
pub use shader::model::CameraTransforms;
pub use shader_database::{create_database, ShaderDatabase, ShaderProgram};
pub use texture::{create_default_textures, load_default_cube};

use ssbh_data::prelude::*;

pub use crate::pipeline::PipelineData;
use crate::rendermesh::RenderMeshSharedData;

pub use renderer::{DebugMode, RenderSettings, TransitionMaterial, RGBA_COLOR_FORMAT};

// TODO: Find a way to avoid using the format features for filterable f32 textures.
/// Required WGPU features for using this library.
/// This library currently only supports WGPU on native desktop platforms.
pub const REQUIRED_FEATURES: wgpu::Features = wgpu::Features::from_bits_truncate(
    wgpu::Features::TEXTURE_COMPRESSION_BC.bits()
        | wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER.bits()
        | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES.bits(),
);

// Store the file data for a single folder.
// This helps ensure files are loaded exactly once.
// Applications can instantiate this struct directly instead of using the filesystem.
pub struct ModelFolder {
    pub folder_name: String,
    // TODO: Will a hashmap be faster for this many items?
    // TODO: Should these be Result<T, E> to display error info in applications?
    pub meshes: Vec<(String, MeshData)>,
    pub skels: Vec<(String, SkelData)>,
    pub matls: Vec<(String, MatlData)>,
    pub modls: Vec<(String, ModlData)>,
    pub adjs: Vec<(String, AdjData)>,
    pub anims: Vec<(String, AnimData)>,
    pub hlpbs: Vec<(String, HlpbData)>,
    pub nutexbs: Vec<(String, NutexbFile)>,
}

impl ModelFolder {
    pub fn load_folder<P: AsRef<Path>>(folder: P) -> Self {
        Self {
            folder_name: folder.as_ref().to_string_lossy().to_string(),
            meshes: read_files(folder.as_ref(), "numshb", MeshData::from_file),
            skels: read_files(folder.as_ref(), "nusktb", SkelData::from_file),
            matls: read_files(folder.as_ref(), "numatb", MatlData::from_file),
            modls: read_files(folder.as_ref(), "numdlb", ModlData::from_file),
            anims: read_files(folder.as_ref(), "nuanmb", AnimData::from_file),
            adjs: read_files(folder.as_ref(), "adjb", AdjData::from_file),
            hlpbs: read_files(folder.as_ref(), "nuhlpb", HlpbData::from_file),
            nutexbs: read_files(folder.as_ref(), "nutexb", NutexbFile::read_from_file),
        }
    }

    /// Searches for the `"model.numdlb"` file in [modls](#structfield.modls).
    pub fn find_modl(&self) -> Option<&ModlData> {
        self.modls
            .iter()
            .find(|(f, _)| f == "model.numdlb")
            .map(|(_, m)| m)
    }

    /// Searches for the `"model.numatb"` file  in [matls](#structfield.matls).
    pub fn find_matl(&self) -> Option<&MatlData> {
        self.matls
            .iter()
            .find(|(f, _)| f == "model.numatb")
            .map(|(_, m)| m)
    }

    /// Searches for the `"model.nusktb"` file  in [skels](#structfield.skels).
    pub fn find_skel(&self) -> Option<&SkelData> {
        self.skels
            .iter()
            .find(|(f, _)| f == "model.nusktb")
            .map(|(_, m)| m)
    }

    /// Searches for the `"model.nuanmb"` file  in [anims](#structfield.anims).
    pub fn find_anim(&self) -> Option<&AnimData> {
        self.anims
            .iter()
            .find(|(f, _)| f == "model.nuanmb")
            .map(|(_, m)| m)
    }

    /// Searches for the `"model.nuhlpb"` file  in [hlpbs](#structfield.hlpbs).
    pub fn find_hlpb(&self) -> Option<&HlpbData> {
        self.hlpbs
            .iter()
            .find(|(f, _)| f == "model.nuhlpb")
            .map(|(_, m)| m)
    }

    /// Searches for the `"model.numshb"` file  in [meshes](#structfield.meshes).
    pub fn find_mesh(&self) -> Option<&MeshData> {
        self.meshes
            .iter()
            .find(|(f, _)| f == "model.numshb")
            .map(|(_, m)| m)
    }

    /// Searches for the `"model.adjb"` file  in [adjs](#structfield.adjs).
    pub fn find_adj(&self) -> Option<&AdjData> {
        self.adjs
            .iter()
            .find(|(f, _)| f == "model.adjb")
            .map(|(_, m)| m)
    }
}

pub fn load_render_models(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pipeline_data: &PipelineData,
    models: &[ModelFolder],
    // TODO: Group textures together?
    default_textures: &[(String, wgpu::Texture)],
    stage_cube: &(wgpu::TextureView, wgpu::Sampler),
    database: &ShaderDatabase,
) -> Vec<RenderModel> {
    let start = std::time::Instant::now();

    // TODO: Find a way to efficiently parallelize render mesh creation?
    let render_models: Vec<_> = models
        .iter()
        .map(|model| {
            // TODO: Should this use the file names in the modl itself?
            let shared_data = RenderMeshSharedData {
                pipeline_data,
                default_textures,
                stage_cube,
                mesh: model.find_mesh(),
                modl: model.find_modl(),
                skel: model.find_skel(),
                matl: model.find_matl(),
                adj: model.find_adj(),
                nutexbs: &model.nutexbs,
                hlpb: model
                    .hlpbs
                    .iter()
                    .find(|(f, _)| f == "model.nuhlpb")
                    .map(|(_, m)| m),
                database,
            };

            // TODO: Avoid creating the render model if there is no mesh?
            rendermesh::create_render_model(device, queue, &shared_data)
        })
        .collect();

    info!(
        "Load {:?} render model(s): {:?}",
        models.len(),
        start.elapsed()
    );

    render_models
}

/// Recursively load folders containing model files starting from `root`.
pub fn load_model_folders<P: AsRef<Path>>(root: P) -> Vec<ModelFolder> {
    // TODO: This could be made more robust.
    // TODO: Determine the minimum files required for a renderable model?
    // TODO: Also check for numdlb?
    // TODO: Specify a max depth?
    // TODO: Find all folders containing any of the supported files?
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
            // TODO: Find a way to test what happens if files are missing.
            let parent = p.path().parent()?;
            Some(ModelFolder::load_folder(parent))
        })
        .collect();
    info!(
        "Load {:?} ModelFolder(s): {:?}",
        models.len(),
        start.elapsed()
    );
    models
}

fn read_files<T, E, F>(parent: &Path, extension: &str, read_t: F) -> Vec<(String, T)>
where
    F: Fn(PathBuf) -> Result<T, E>,
{
    // TODO: Avoid repetitive system calls here?
    // We should be able to just iterate the directory once.
    std::fs::read_dir(parent)
        .map(|dir| {
            dir.filter_map(|p| p.ok().map(|p| p.path()))
                .filter(|p| p.extension().and_then(|p| p.to_str()) == Some(extension))
                .filter_map(|p| {
                    Some((
                        p.file_name()?.to_string_lossy().to_string(),
                        read_t(p).ok()?,
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
macro_rules! assert_vector_relative_eq {
    ($a:expr, $b:expr) => {
        assert!(
            $a.iter()
                .zip($b.iter())
                .all(|(a, b)| approx::relative_eq!(a, b, epsilon = 0.0001f32)),
            "Vectors not equal to within 0.0001.\nleft = {:?}\nright = {:?}",
            $a,
            $b
        )
    };
}

#[cfg(test)]
macro_rules! assert_matrix_relative_eq {
    ($a:expr, $b:expr) => {
        assert!(
            $a.iter()
                .flatten()
                .zip($b.iter().flatten())
                .all(|(a, b)| approx::relative_eq!(a, b, epsilon = 0.0001f32)),
            "Matrices not equal to within 0.0001.\nleft = {:?}\nright = {:?}",
            $a,
            $b
        )
    };
}

#[cfg(test)]
pub(crate) use assert_matrix_relative_eq;

#[cfg(test)]
pub(crate) use assert_vector_relative_eq;
