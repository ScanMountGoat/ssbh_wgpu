use bytemuck::Pod;
use encase::{internal::WriteInto, ShaderSize, ShaderType, StorageBuffer};
use log::{error, info};
use model::pipeline::PipelineData;
use rayon::prelude::*;
use ssbh_data::prelude::*;
use std::{
    error::Error,
    path::{Path, PathBuf},
    time::Duration,
};
use walkdir::WalkDir;
use wgpu::util::DeviceExt;
use xmb_lib::XmbFile;

// TODO: Rework this public API and improve docs.
pub use nutexb_wgpu::NutexbFile;

pub mod animation;
mod bone_name;
mod bone_rendering;
mod floor_grid;
mod model;
mod render_settings;
mod renderer;
mod sampler;
mod shader;
mod shader_database;
mod shape;
pub mod swing;
mod swing_rendering;
mod texture;
mod uniforms;
mod vertex;
pub mod viewport;

pub use bone_name::BoneNameRenderer;
pub use model::{RenderMesh, RenderModel};
pub use render_settings::{
    DebugMode, ModelRenderOptions, RenderSettings, SkinningSettings, TransitionMaterial,
};
pub use renderer::SsbhRenderer;
pub use shader::model::CameraTransforms;
pub use shader_database::{split_param, ShaderDatabase, ShaderProgram};
pub use texture::{create_default_textures, load_default_spec_cube};

// TODO: Find a way to avoid using the format features for filterable f32 textures.
/// Required WGPU features for using this library.
/// This library currently only supports WGPU on native desktop platforms.
pub const REQUIRED_FEATURES: wgpu::Features = wgpu::Features::TEXTURE_COMPRESSION_BC
    .union(wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER)
    .union(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES)
    .union(wgpu::Features::POLYGON_MODE_LINE)
    .union(wgpu::Features::DEPTH32FLOAT_STENCIL8)
    .union(wgpu::Features::TEXTURE_FORMAT_16BIT_NORM);

// TODO: Better name?
pub struct SharedRenderData {
    pipeline_data: PipelineData,
    default_textures: Vec<(String, wgpu::Texture, wgpu::TextureViewDimension)>,
    database: ShaderDatabase,
}

impl SharedRenderData {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self {
            pipeline_data: PipelineData::new(device),
            default_textures: create_default_textures(device, queue),
            database: ShaderDatabase::new(),
        }
    }

    pub fn default_textures(&self) -> &[(String, wgpu::Texture, wgpu::TextureViewDimension)] {
        &self.default_textures
    }

    pub fn database(&self) -> &ShaderDatabase {
        &self.database
    }

    /// Updates the default texture for `#replace_cubemap` from `nutexb`.
    /// Invalid nutexb files are ignored.
    ///
    /// Textures will need to be updated for each [RenderModel] with
    /// [RenderModel::recreate_materials] for this change to take effect.
    pub fn update_stage_cube_map(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        nutexb: &NutexbFile,
    ) {
        // TODO: Return errors.
        if let Some((_, texture, _)) = self
            .default_textures
            .iter_mut()
            .find(|(name, _, _)| name == "#replace_cubemap")
        {
            if let Ok((new_texture, wgpu::TextureViewDimension::Cube)) =
                nutexb_wgpu::create_texture(nutexb, device, queue)
            {
                *texture = new_texture;
            }
        }
    }

    /// Resets the default texture for `#replace_cubemap` to its default value.
    pub fn reset_stage_cube_map(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if let Some((_, texture, _)) = self
            .default_textures
            .iter_mut()
            .find(|(name, _, _)| name == "#replace_cubemap")
        {
            let (new_texture, _) = load_default_spec_cube(device, queue);
            *texture = new_texture;
        }
    }
}

pub type ModelFiles<T> = Vec<(String, Option<T>)>;

/// A collection of supported rendering related files in a model or animation folder.
///
/// A [ModelFolder] can be instantiated directly or from the file system using [ModelFolder::load_folder].
/// Convert to a renderable model using [load_render_models] or [RenderModel::from_folder].
#[derive(Debug)]
pub struct ModelFolder {
    // TODO: Will a hashmap be faster for this many items?
    pub meshes: ModelFiles<MeshData>,
    pub meshexes: ModelFiles<MeshExData>,
    pub skels: ModelFiles<SkelData>,
    pub matls: ModelFiles<MatlData>,
    pub modls: ModelFiles<ModlData>,
    pub adjs: ModelFiles<AdjData>,
    pub anims: ModelFiles<AnimData>,
    pub hlpbs: ModelFiles<HlpbData>,
    pub nutexbs: ModelFiles<NutexbFile>,
    pub xmbs: ModelFiles<XmbFile>,
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for ModelFolder {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        // TODO: Use option to make some files missing?
        Ok(Self {
            meshes: vec![("model.numshb".to_owned(), Ok(u.arbitrary()?))],
            skels: vec![("model.nusktb".to_owned(), Ok(u.arbitrary()?))],
            matls: vec![("model.numatb".to_owned(), Ok(u.arbitrary()?))],
            modls: vec![("model.numdlb".to_owned(), Ok(u.arbitrary()?))],
            adjs: vec![(u.arbitrary()?, Ok(u.arbitrary()?))],
            anims: vec![(u.arbitrary()?, Ok(u.arbitrary()?))],
            hlpbs: vec![(u.arbitrary()?, Ok(u.arbitrary()?))],
            nutexbs: vec![], // TODO: Createy arbitrary nutexbs
            meshexes: vec![("model.numshexb".to_owned(), Ok(u.arbitrary()?))],
            xmbs: vec![("model.xmb".to_owned(), Ok(u.arbitrary()?))],
        })
    }
}

impl ModelFolder {
    pub fn load_folder<P: AsRef<Path>>(folder: P) -> Self {
        let files: Vec<_> = std::fs::read_dir(folder)
            .map(|dir| dir.filter_map(|p| p.ok().map(|p| p.path())).collect())
            .unwrap_or_default();

        Self {
            meshes: read_files(&files, "numshb", MeshData::from_file),
            meshexes: read_files(&files, "numshexb", MeshExData::from_file),
            skels: read_files(&files, "nusktb", SkelData::from_file),
            matls: read_files(&files, "numatb", MatlData::from_file),
            modls: read_files(&files, "numdlb", ModlData::from_file),
            anims: read_files(&files, "nuanmb", AnimData::from_file),
            adjs: read_files(&files, "adjb", AdjData::from_file),
            hlpbs: read_files(&files, "nuhlpb", HlpbData::from_file),
            nutexbs: read_files(&files, "nutexb", |p| {
                NutexbFile::read_from_file(p).map_err(Into::into)
            }),
            xmbs: read_files(&files, "xmb", XmbFile::from_file),
        }
    }

    /// Finds the `"model.numdlb"` file in [modls](#structfield.modls).
    pub fn find_modl(&self) -> Option<&ModlData> {
        self.modls
            .iter()
            .find(|(f, _)| f == "model.numdlb")
            .and_then(|(_, m)| m.as_ref())
    }

    /// Finds the `"model.numatb"` file in [matls](#structfield.matls).
    pub fn find_matl(&self) -> Option<&MatlData> {
        self.matls
            .iter()
            .find(|(f, _)| f == "model.numatb")
            .and_then(|(_, m)| m.as_ref())
    }

    /// Finds the `"model.nusktb"` file in [skels](#structfield.skels).
    pub fn find_skel(&self) -> Option<&SkelData> {
        self.skels
            .iter()
            .find(|(f, _)| f == "model.nusktb")
            .and_then(|(_, m)| m.as_ref())
    }

    /// Finds the `"model.nuanmb"` file in [anims](#structfield.anims).
    pub fn find_anim(&self) -> Option<&AnimData> {
        self.anims
            .iter()
            .find(|(f, _)| f == "model.nuanmb")
            .and_then(|(_, m)| m.as_ref())
    }

    /// Finds the `"model.nuhlpb"` file in [hlpbs](#structfield.hlpbs).
    pub fn find_hlpb(&self) -> Option<&HlpbData> {
        self.hlpbs
            .iter()
            .find(|(f, _)| f == "model.nuhlpb")
            .and_then(|(_, m)| m.as_ref())
    }

    /// Finds the `"model.numshb"` file in [meshes](#structfield.meshes).
    pub fn find_mesh(&self) -> Option<&MeshData> {
        self.meshes
            .iter()
            .find(|(f, _)| f == "model.numshb")
            .and_then(|(_, m)| m.as_ref())
    }

    /// Finds the `"model.numshexb"` file in [meshexes](#structfield.meshexes).
    pub fn find_meshex(&self) -> Option<&MeshExData> {
        self.meshexes
            .iter()
            .find(|(f, _)| f == "model.numshexb")
            .and_then(|(_, m)| m.as_ref())
    }

    /// Finds the `"model.adjb"` file in [adjs](#structfield.adjs).
    pub fn find_adj(&self) -> Option<&AdjData> {
        self.adjs
            .iter()
            .find(|(f, _)| f == "model.adjb")
            .and_then(|(_, m)| m.as_ref())
    }

    /// Finds the `"model.nuhlpb"` file in [hlpbs](#structfield.hlpbs).
    pub fn find_nuhlpb(&self) -> Option<&HlpbData> {
        self.hlpbs
            .iter()
            .find(|(f, _)| f == "model.nuhlpb")
            .and_then(|(_, m)| m.as_ref())
    }

    /// Finds the `"model.xmb"` file in [xmbs](#structfield.xmbs).
    pub fn find_model_xmb(&self) -> Option<&XmbFile> {
        self.xmbs
            .iter()
            .find(|(f, _)| f == "model.xmb")
            .and_then(|(_, m)| m.as_ref())
    }

    // Returns `true` if the folder has no supported files.
    pub fn is_empty(&self) -> bool {
        self.meshes.is_empty()
            && self.skels.is_empty()
            && self.matls.is_empty()
            && self.modls.is_empty()
            && self.anims.is_empty()
            && self.adjs.is_empty()
            && self.hlpbs.is_empty()
            && self.nutexbs.is_empty()
    }
}

pub fn load_render_models<'a>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    models: impl IntoIterator<Item = &'a ModelFolder>,
    shared_data: &SharedRenderData,
) -> Vec<RenderModel> {
    let start = std::time::Instant::now();

    // TODO: Find a way to efficiently parallelize render mesh creation?
    let render_models: Vec<_> = models
        .into_iter()
        .map(|model| RenderModel::from_folder(device, queue, model, shared_data))
        .collect();

    info!(
        "Loaded {:?} render model(s): {:?}",
        render_models.len(),
        start.elapsed()
    );

    render_models
}

/// Recursively load folders and their paths from `root` with a max recursion depth of 4.
///
/// The recursion depth starts at 0 from `root`,
/// `"/fighter/mario"` will load model folders like "/fighter/mario/model/body/c00".
/// "/fighter" will exceed the maximum recursion depth and not load any model folders.
/// For applications using very deeply nested folders, call [ModelFolder::load_folder] directly.
pub fn load_model_folders<P: AsRef<Path>>(root: P) -> Vec<(PathBuf, ModelFolder)> {
    let start = std::time::Instant::now();

    // The ARC paths only need a max depth of 4 for model files.
    // Examples include mario/model/body/c00 or mario_galaxy/normal/model/stc_ring_set.
    // Opening the entire fighter folder has a depth of 5 and will likely crash.
    let models: Vec<_> = WalkDir::new(root)
        .max_depth(4)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_dir())
        .map(|e| (e.path().to_owned(), ModelFolder::load_folder(e.path())))
        .collect();

    info!(
        "Loaded {:?} ModelFolder(s): {:?}",
        models.len(),
        start.elapsed()
    );

    models
}

fn read_files<T, F>(files: &[PathBuf], extension: &str, read_t: F) -> ModelFiles<T>
where
    T: Send,
    F: Fn(PathBuf) -> Result<T, Box<dyn Error>> + Sync,
{
    // Threading optimizes loading many animations and doesn't negatively impact other files.
    files
        .iter()
        .filter(|p| p.extension().and_then(|p| p.to_str()) == Some(extension))
        .par_bridge()
        .filter_map(|p| {
            Some((
                p.file_name()?.to_string_lossy().to_string(),
                read_t(p.clone())
                    .inspect_err(|e| error!("Error reading {:?}: {}", p, e))
                    .ok(),
            ))
        })
        .collect()
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
macro_rules! assert_quat_relative_eq {
    ($a:expr, $b:expr) => {
        // Negating the axis and angle gives the same quaternion.
        // Always choose the angle to be positive for comparisons.
        // This helps avoid differences with SIMD vs scalar results.
        assert!(
            $a.iter()
                .map(|v| v * f32::signum($a[3]))
                .zip($b.iter().map(|v| v * f32::signum($b[3])))
                .all(|(a, b)| approx::relative_eq!(a, b, epsilon = 0.0001f32)),
            "Quaternions not equal to within 0.0001.\nleft = {:?}\nright = {:?}",
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

#[cfg(test)]
pub(crate) use assert_quat_relative_eq;

trait DeviceBufferExt {
    fn create_buffer_from_data<T: ShaderType + WriteInto + ShaderSize>(
        &self,
        label: &str,
        contents: &[T],
        usage: wgpu::BufferUsages,
    ) -> wgpu::Buffer;

    fn create_index_buffer(&self, label: &str, contents: &[u32]) -> wgpu::Buffer;

    fn create_buffer_from_bytes(
        &self,
        label: &str,
        contents: &[u8],
        usage: wgpu::BufferUsages,
    ) -> wgpu::Buffer;
}

impl DeviceBufferExt for wgpu::Device {
    fn create_buffer_from_data<T: ShaderType + WriteInto + ShaderSize>(
        &self,
        label: &str,
        data: &[T],
        usage: wgpu::BufferUsages,
    ) -> wgpu::Buffer {
        // Storage buffers also satisfy uniform buffer alignment requirements.
        let mut buffer = StorageBuffer::new(Vec::new());
        buffer.write(&data).unwrap();

        self.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: &buffer.into_inner(),
            usage,
        })
    }

    fn create_buffer_from_bytes(
        &self,
        label: &str,
        contents: &[u8],
        usage: wgpu::BufferUsages,
    ) -> wgpu::Buffer {
        self.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents,
            usage,
        })
    }

    fn create_index_buffer(&self, label: &str, data: &[u32]) -> wgpu::Buffer {
        // Vertex indices should be tightly packed and don't need encase.
        self.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(data),
            usage: wgpu::BufferUsages::INDEX,
        })
    }
}

trait QueueExt {
    fn write_data<D: Pod>(&self, buffer: &wgpu::Buffer, data: &[D]);
}

impl QueueExt for wgpu::Queue {
    fn write_data<D: Pod>(&self, buffer: &wgpu::Buffer, data: &[D]) {
        self.write_buffer(buffer, 0, bytemuck::cast_slice(data));
    }
}

/// Calculate the next frame given `time_since_last_frame` to ensure 60 fps animations play at full speed.
///
/// A `playback_speed` of `1.0` assumes an animation frame rate of 60 fps.
///
/// Non integral return values are possible if the elapsed time is not a multiple of `1 / (60 * playback_speed)` seconds.
/// ssbh_wgpu handles non integral frame values using interpolation where appropriate.
///
/// The return value may be more than `1.0` frames if `time_since_last_frame` is sufficiently large.
/// This is often called "frame skipping" and ssbh_wgpu will not render all animation frames.
pub fn next_frame(
    current_frame: f32,
    time_since_last_frame: Duration,
    final_frame_index: f32,
    playback_speed: f32,
    should_loop: bool,
) -> f32 {
    // Convert elapsed time to a delta in frames.
    // This relies on interpolation or frame skipping.
    // TODO: Ensure 60hz monitors always advanced by exactly one frame per refresh?
    let delta_t_frames = time_since_last_frame.as_secs_f64() * 60.0;

    let mut next_frame = current_frame + (delta_t_frames as f32 * playback_speed);

    if next_frame > final_frame_index && should_loop {
        // Wrap around to loop the animation.
        // This may not be seamless if the animations have different lengths.
        next_frame = if final_frame_index > 0.0 {
            next_frame.rem_euclid(final_frame_index)
        } else {
            // Use 0.0 instead of NaN for empty animations.
            0.0
        };
    }

    next_frame
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn next_frame_loop_zero_final_frame() {
        assert_eq!(0.0, next_frame(1.0, Duration::from_secs(0), 0.0, 1.0, true));
    }

    #[test]
    fn next_frame_loop_no_wrapping() {
        assert_eq!(
            60.0,
            next_frame(0.0, Duration::from_secs(1), 61.0, 1.0, true)
        );
    }

    #[test]
    fn next_frame_loop_wrap_around() {
        assert_eq!(
            1.0,
            next_frame(0.0, Duration::from_secs(1), 59.0, 1.0, true)
        );
    }

    #[test]
    fn next_frame_no_loop() {
        assert_eq!(
            59.0,
            next_frame(0.0, Duration::from_secs(1), 59.0, 1.0, false)
        );
    }
}
