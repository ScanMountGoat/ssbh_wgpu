use log::{error, info};
use nutexb_wgpu::NutexbFile;
use ssbh_data::prelude::*;
use std::{
    error::Error,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;
// TODO: Use rayon to speed up load times?

mod pipeline;
mod shader;
mod texture;
mod uniforms;
mod vertex;

// TODO: Should this just be it's own project to help with testing?
pub mod animation;

mod bone_rendering;
mod lighting;
mod renderer;
mod rendermesh;
mod shader_database;

pub use crate::pipeline::PipelineData;
pub use renderer::SsbhRenderer;
pub use renderer::{DebugMode, RenderSettings, TransitionMaterial, RGBA_COLOR_FORMAT};
pub use rendermesh::{RenderMesh, RenderModel};
pub use shader::model::CameraTransforms;
pub use shader_database::{create_database, ShaderDatabase, ShaderProgram};
pub use texture::{create_default_textures, load_default_spec_cube};

// TODO: Find a way to avoid using the format features for filterable f32 textures.
/// Required WGPU features for using this library.
/// This library currently only supports WGPU on native desktop platforms.
pub const REQUIRED_FEATURES: wgpu::Features = wgpu::Features::from_bits_truncate(
    wgpu::Features::TEXTURE_COMPRESSION_BC.bits()
        | wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER.bits()
        | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES.bits(),
);

// TODO: Make these fields get only like fn database(&self)?
// TODO: Better name?
pub struct SharedRenderData {
    pub pipeline_data: PipelineData,
    pub default_textures: Vec<(String, wgpu::Texture, wgpu::TextureViewDimension)>,
    pub stage_cube: (wgpu::Texture, wgpu::Sampler), // TODO: This should be editable?
    pub database: ShaderDatabase,
}

impl SharedRenderData {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            pipeline_data: PipelineData::new(device, surface_format),
            default_textures: create_default_textures(device, queue),
            stage_cube: load_default_spec_cube(device, queue),
            database: create_database(),
        }
    }
}

pub type ModelFiles<T> = Vec<(String, Result<T, Box<dyn Error>>)>;

// Store the file data for a single folder.
// This helps ensure files are loaded exactly once.
// Applications can instantiate this struct directly instead of using the filesystem.
#[derive(Debug)]
pub struct ModelFolder {
    pub folder_name: String,
    // TODO: Will a hashmap be faster for this many items?
    // TODO: Should these be Result<T, E> to display error info in applications?
    pub meshes: ModelFiles<MeshData>,
    pub skels: ModelFiles<SkelData>,
    pub matls: ModelFiles<MatlData>,
    pub modls: ModelFiles<ModlData>,
    pub adjs: ModelFiles<AdjData>,
    pub anims: ModelFiles<AnimData>,
    pub hlpbs: ModelFiles<HlpbData>,
    pub nutexbs: ModelFiles<NutexbFile>,
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for ModelFolder {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self {
            folder_name: u.arbitrary()?,
            meshes: vec![(u.arbitrary()?, Ok(u.arbitrary()?))],
            skels: vec![(u.arbitrary()?, Ok(u.arbitrary()?))],
            matls: vec![(u.arbitrary()?, Ok(u.arbitrary()?))],
            modls: vec![(u.arbitrary()?, Ok(u.arbitrary()?))],
            adjs: vec![(u.arbitrary()?, Ok(u.arbitrary()?))],
            anims: vec![(u.arbitrary()?, Ok(u.arbitrary()?))],
            hlpbs: vec![(u.arbitrary()?, Ok(u.arbitrary()?))],
            nutexbs: vec![],
        })
    }
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
            .and_then(|(_, m)| m.as_ref().ok())
    }

    /// Searches for the `"model.numatb"` file in [matls](#structfield.matls).
    pub fn find_matl(&self) -> Option<&MatlData> {
        self.matls
            .iter()
            .find(|(f, _)| f == "model.numatb")
            .and_then(|(_, m)| m.as_ref().ok())
    }

    /// Searches for the `"model.nusktb"` file in [skels](#structfield.skels).
    pub fn find_skel(&self) -> Option<&SkelData> {
        self.skels
            .iter()
            .find(|(f, _)| f == "model.nusktb")
            .and_then(|(_, m)| m.as_ref().ok())
    }

    /// Searches for the `"model.nuanmb"` file in [anims](#structfield.anims).
    pub fn find_anim(&self) -> Option<&AnimData> {
        self.anims
            .iter()
            .find(|(f, _)| f == "model.nuanmb")
            .and_then(|(_, m)| m.as_ref().ok())
    }

    /// Searches for the `"model.nuhlpb"` file in [hlpbs](#structfield.hlpbs).
    pub fn find_hlpb(&self) -> Option<&HlpbData> {
        self.hlpbs
            .iter()
            .find(|(f, _)| f == "model.nuhlpb")
            .and_then(|(_, m)| m.as_ref().ok())
    }

    /// Searches for the `"model.numshb"` file in [meshes](#structfield.meshes).
    pub fn find_mesh(&self) -> Option<&MeshData> {
        self.meshes
            .iter()
            .find(|(f, _)| f == "model.numshb")
            .and_then(|(_, m)| m.as_ref().ok())
    }

    /// Searches for the `"model.adjb"` file in [adjs](#structfield.adjs).
    pub fn find_adj(&self) -> Option<&AdjData> {
        self.adjs
            .iter()
            .find(|(f, _)| f == "model.adjb")
            .and_then(|(_, m)| m.as_ref().ok())
    }
}

pub fn load_render_models(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    models: &[ModelFolder],
    shared_data: &SharedRenderData,
) -> Vec<RenderModel> {
    let start = std::time::Instant::now();

    // TODO: Find a way to efficiently parallelize render mesh creation?
    let render_models: Vec<_> = models
        .iter()
        .map(|model| RenderModel::from_folder(device, queue, model, shared_data))
        .collect();

    info!(
        "Load {:?} render model(s): {:?}",
        models.len(),
        start.elapsed()
    );

    render_models
}

/// Recursively load folders from `root` with a max recursion depth of 4.
///
/// The recursion depth starts at 0 from `root`,
/// `"/fighter/mario"` will load model folders like "/fighter/mario/model/body/c00".
/// "/fighter" will exceed the maximum recursion depth and not load any model folders.
/// For applications using very deeply nested folders, call [ModelFolder::load_folder] directly.
pub fn load_model_folders<P: AsRef<Path>>(root: P) -> Vec<ModelFolder> {
    let start = std::time::Instant::now();

    // The ARC paths only need a max depth of 4 for model files.
    // Examples include mario/model/body/c00 or mario_galaxy/normal/model/stc_ring_set.
    // Opening the entire fighter folder has a depth of 5 and will likely crash.
    let models: Vec<_> = WalkDir::new(root)
        .max_depth(4)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_dir())
        .map(|e| ModelFolder::load_folder(e.path()))
        .collect();

    info!(
        "Load {:?} ModelFolder(s): {:?}",
        models.len(),
        start.elapsed()
    );

    models
}

fn read_files<T, F>(parent: &Path, extension: &str, read_t: F) -> ModelFiles<T>
where
    F: Fn(PathBuf) -> Result<T, Box<dyn Error>>,
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
                        read_t(p.clone()).map_err(|e| {
                            error!("Error reading {:?}: {}", p, e);
                            e
                        }),
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
