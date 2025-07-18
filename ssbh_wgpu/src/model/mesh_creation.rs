use super::pipeline::{pipeline, PipelineKey};
use crate::{
    animation::AnimationTransforms,
    bone_rendering::*,
    model::{BoneRenderData, SamplerCache},
    renderer::RGBA_COLOR_FORMAT,
    swing_rendering::SwingRenderData,
    uniforms::{
        default_material_uniforms_bind_group, default_uniforms_buffer,
        material_uniforms_bind_group, per_material, uniforms_buffer,
    },
    vertex::{buffer0, buffer1, combined_mesh_buffers, skin_weights, CombinedMeshBuffers},
    DeviceBufferExt, ModelFiles, RenderMesh, RenderModel, ShaderDatabase, SharedRenderData,
};
use encase::{DynamicStorageBuffer, ShaderType};
use log::{error, info};
use nutexb_wgpu::NutexbFile;
use ssbh_data::{
    adj_data::AdjEntryData, matl_data::MatlEntryData, mesh_data::MeshObjectData,
    meshex_data::EntryFlags, prelude::*,
};
use std::{collections::HashMap, error::Error, num::NonZeroU64};
use xmb_lib::XmbFile;

pub struct Material {
    pub material_uniforms_bind_group: crate::shader::model::bind_groups::BindGroup2,
    pub uniforms_buffer: wgpu::Buffer,
}

impl Material {
    pub fn update(
        &mut self,
        queue: &wgpu::Queue,
        material: &MatlEntryData,
        database: &ShaderDatabase,
    ) {
        // Material animations don't assign textures.
        // We only need to update the material parameter buffer.
        // This avoids creating GPU resources each frame.
        let uniforms = per_material(material, database);
        queue.write_buffer(&self.uniforms_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }
}

pub struct TransformBuffers {
    pub skinning_transforms: wgpu::Buffer,
    pub world_transforms: wgpu::Buffer,
}

struct RenderMeshData {
    meshes: Vec<RenderMesh>,
    material_data_by_label: HashMap<String, Material>,
    textures: Vec<(String, wgpu::Texture, wgpu::TextureViewDimension)>,
    pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
    buffer_data: CombinedMeshBuffers,
}

// TODO: Come up with a better name.
pub struct RenderMeshSharedData<'a> {
    pub shared_data: &'a SharedRenderData,
    pub mesh: Option<&'a MeshData>,
    pub meshex: Option<&'a MeshExData>,
    pub modl: Option<&'a ModlData>,
    pub skel: Option<&'a SkelData>,
    pub matl: Option<&'a MatlData>,
    pub adj: Option<&'a AdjData>,
    pub hlpb: Option<&'a HlpbData>,
    pub model_xmb: Option<&'a XmbFile>,
    pub nutexbs: &'a ModelFiles<NutexbFile>,
}

impl<'a> RenderMeshSharedData<'a> {
    pub fn to_render_model(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> RenderModel {
        let start = std::time::Instant::now();

        // Attempt to initialize transforms using the skel.
        // This correctly positions mesh objects parented to a bone.
        // Otherwise, don't apply any transformations.
        // TODO: Is it worth matching the in game behavior for a missing skel?
        // "Invisible" models might be more confusing for users to understand.
        let animation_transforms = self
            .skel
            .map(AnimationTransforms::from_skel)
            .unwrap_or_else(AnimationTransforms::identity);

        // Share the transforms buffer to avoid redundant updates.
        let skinning_transforms_buffer = device.create_buffer_from_data(
            "Bone Transforms Buffer",
            &[animation_transforms.animated_world_transforms],
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );

        let world_transforms = device.create_buffer_from_data(
            "World Transforms Buffer",
            &animation_transforms.world_transforms,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );

        let swing_render_data = SwingRenderData::new(device, &world_transforms);

        let bone_render_data =
            self.create_bone_render_data(device, &animation_transforms, &world_transforms);

        let mesh_buffers = TransformBuffers {
            skinning_transforms: skinning_transforms_buffer,
            world_transforms,
        };

        let default_material_data = default_material_data(device, self.shared_data);

        let per_model_buffer = self.per_model_buffer(device);

        let per_model_bind_group = crate::shader::model::bind_groups::BindGroup1::from_bindings(
            device,
            crate::shader::model::bind_groups::BindGroupLayout1 {
                per_model: per_model_buffer.as_entire_buffer_binding(),
            },
        );

        let bone_names = self
            .skel
            .map(|skel| skel.bones.iter().map(|b| b.name.clone()).collect())
            .unwrap_or_default();

        let RenderMeshData {
            meshes,
            material_data_by_label,
            textures,
            pipelines,
            buffer_data,
        } = self.create_render_mesh_data(device, queue, &mesh_buffers);

        info!(
            "Created {:?} render meshe(s), {:?} material(s), {:?} pipeline(s): {:?}",
            meshes.len(),
            material_data_by_label.len(),
            pipelines.len(),
            start.elapsed()
        );

        RenderModel {
            is_visible: true,
            is_selected: false,
            meshes,
            transforms: mesh_buffers,
            material_data_by_label,
            default_material_data,
            textures,
            pipelines,
            bone_render_data,
            mesh_buffers: buffer_data,
            animation_transforms: Box::new(animation_transforms),
            swing_render_data,
            per_model_bind_group,
            bone_names,
        }
    }

    fn per_model_buffer(&self, device: &wgpu::Device) -> wgpu::Buffer {
        // Get light set information from the model.xmb.
        // We don't initialize the light data itself here.
        // This allows lighting to be updated globally for all models.
        let model_entry = self.model_xmb.and_then(|xmb| xmb.entries.first());
        let is_stage = model_entry
            .and_then(|e| e.attributes.get("type"))
            .map(|model_type| model_type == "stage")
            .unwrap_or_default();

        let lightset: u32 = model_entry
            .and_then(|e| e.children.iter().find(|c| c.name == "lightset"))
            .and_then(|e| e.attributes.get("number"))
            .and_then(|a| a.parse().ok())
            .unwrap_or_default();

        // TODO: Include other model.xmb rendering related settings.
        device.create_buffer_from_data(
            "PerModel",
            &[crate::shader::model::PerModel {
                light_set_index: glam::uvec4(is_stage as u32, lightset, 0, 0),
            }],
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        )
    }

    fn create_bone_render_data(
        &self,
        device: &wgpu::Device,
        animation_transforms: &AnimationTransforms,
        world_transforms: &wgpu::Buffer,
    ) -> BoneRenderData {
        let bone_colors = bone_colors_buffer(device, self.skel, self.hlpb);

        let joint_transforms = self
            .skel
            .map(|skel| joint_transforms(skel, animation_transforms))
            .unwrap_or_else(|| vec![glam::Mat4::IDENTITY; 512]);

        let joint_world_transforms = device.create_buffer_from_data(
            "Joint World Transforms Buffer",
            &joint_transforms,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );

        let bone_data = bone_bind_group1(device, world_transforms, &bone_colors);
        let joint_data = bone_bind_group1(device, &joint_world_transforms, &bone_colors);
        let bone_bind_groups = bone_bind_groups(device, self.skel);

        BoneRenderData {
            joint_world_transforms,
            bone_data,
            joint_data,
            bone_bind_groups,
        }
    }

    fn create_render_mesh_data(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mesh_buffers: &TransformBuffers,
    ) -> RenderMeshData {
        // TODO: Find a way to organize this.

        // Initialize textures exactly once for performance.
        // Unused textures are rare, so we won't lazy load them.
        let textures = self.create_textures(device, queue);

        // Materials can be shared between mesh objects.
        let material_data_by_label = self.create_materials(device, &textures);

        // DynamicStorageBuffer ensures mesh object offsets are properly aligned.
        let mut model_buffer0_data = DynamicStorageBuffer::new(Vec::new());
        let mut model_buffer1_data = Vec::new();
        let mut model_skin_weights_data = DynamicStorageBuffer::new(Vec::new());

        let mut model_indices = Vec::new();

        let mut accesses = Vec::new();

        // TODO: Refactor this to use iterators.
        if let Some(mesh) = self.mesh.as_ref() {
            for mesh_object in &mesh.objects {
                // TODO: Find a way to have fewer function parameters?
                match append_mesh_object_buffer_data(
                    &mut model_buffer0_data,
                    &mut model_buffer1_data,
                    &mut model_skin_weights_data,
                    &mut model_indices,
                    mesh_object,
                    self,
                ) {
                    Ok(access) => {
                        accesses.push(access);
                    }
                    Err(e) => {
                        error!(
                            "Error accessing vertex data for mesh {}: {}",
                            mesh_object.name, e
                        );
                    }
                }
            }
        }

        let combined_mesh_buffers = combined_mesh_buffers(
            device,
            &model_buffer0_data.into_inner(),
            &model_buffer1_data,
            &model_skin_weights_data.into_inner(),
            &model_indices,
        );

        // Mesh objects control the depth state of the pipeline.
        // Each (shader, mesh) pair may need a unique pipeline.
        // Cache materials separately since materials may share a pipeline.
        let mut pipelines = HashMap::new();

        let meshes = self
            .create_render_meshes(
                accesses,
                device,
                &mut pipelines,
                mesh_buffers,
                &combined_mesh_buffers,
            )
            .unwrap_or_default();

        RenderMeshData {
            meshes,
            material_data_by_label,
            textures,
            pipelines,
            buffer_data: combined_mesh_buffers,
        }
    }

    fn create_render_meshes(
        &self,
        accesses: Vec<MeshBufferAccess>,
        device: &wgpu::Device,
        pipelines: &mut HashMap<PipelineKey, wgpu::RenderPipeline>,
        transform_buffers: &TransformBuffers,
        mesh_buffers: &CombinedMeshBuffers,
    ) -> Option<Vec<RenderMesh>> {
        Some(
            self.mesh?
                .objects
                .iter() // TODO: par_iter?
                .zip(accesses)
                .enumerate()
                .filter_map(|(i, (mesh_object, access))| {
                    // Some mesh objects have associated triangle adjacency.
                    let adj_entry = self
                        .adj
                        .and_then(|adj| adj.entries.iter().find(|e| e.mesh_object_index == i));

                    // Find rendering flags from the numshexb.
                    let meshex_flags = self
                        .meshex
                        .and_then(|meshex| {
                            meshex
                                .mesh_object_groups
                                .iter()
                                .find(|g| g.mesh_object_full_name == mesh_object.name)
                        })
                        .and_then(|g| g.entry_flags.get(mesh_object.subindex as usize));

                    self.create_render_mesh(
                        device,
                        mesh_object,
                        adj_entry,
                        meshex_flags.copied(),
                        pipelines,
                        transform_buffers,
                        access,
                        mesh_buffers,
                    )
                    .map_err(|e| {
                        error!(
                            "Error creating render mesh for mesh {}: {}",
                            mesh_object.name, e
                        );
                        e
                    })
                    .ok()
                })
                .collect(),
        )
    }

    fn create_textures(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Vec<(String, wgpu::Texture, wgpu::TextureViewDimension)> {
        self.nutexbs
            .iter()
            .filter_map(|(name, nutexb)| {
                let nutexb = nutexb.as_ref()?;
                let (texture, dim) = nutexb_wgpu::create_texture(nutexb, device, queue)
                    .inspect_err(|e| error!("Failed to create nutexb texture {}: {}", name, e))
                    .ok()?;
                Some((name.clone(), texture, dim))
            })
            .collect()
    }

    fn create_materials(
        &self,
        device: &wgpu::Device,
        textures: &[(String, wgpu::Texture, wgpu::TextureViewDimension)],
    ) -> HashMap<String, Material> {
        // Some devices only support up to 4000 sampler allocations.
        // Models use very few unique sampler settings in practice.
        // Samplers are immutable and can be safely cached.
        let mut sampler_by_data = SamplerCache::new();

        // TODO: Split into PerMaterial, PerObject, etc in the shaders?
        let materials = self
            .matl
            .map(|matl| {
                matl.entries
                    .iter()
                    .map(|entry| {
                        let data = material_data(
                            device,
                            entry,
                            textures,
                            self.shared_data,
                            &mut sampler_by_data,
                        );
                        (entry.material_label.clone(), data)
                    })
                    .collect()
            })
            .unwrap_or_default();

        info!("Created {} samplers", sampler_by_data.len());

        materials
    }

    // TODO: Group these parameters?
    fn create_render_mesh(
        &self,
        device: &wgpu::Device,
        mesh_object: &MeshObjectData,
        adj_entry: Option<&AdjEntryData>,
        meshex_flags: Option<EntryFlags>,
        pipelines: &mut HashMap<PipelineKey, wgpu::RenderPipeline>,
        transforms: &TransformBuffers,
        access: MeshBufferAccess,
        buffers: &CombinedMeshBuffers,
    ) -> Result<RenderMesh, Box<dyn Error>> {
        // TODO: These could be cleaner as functions.
        // TODO: Is using a default for the material label ok?
        let material_label = self
            .modl
            .and_then(|m| {
                m.entries
                    .iter()
                    .find(|e| {
                        e.mesh_object_name == mesh_object.name
                            && e.mesh_object_subindex == mesh_object.subindex
                    })
                    .map(|e| &e.material_label)
            })
            .unwrap_or(&String::new())
            .to_string();

        let material = self.matl.and_then(|matl| {
            matl.entries
                .iter()
                .find(|e| e.material_label == material_label)
        });

        // Pipeline creation is expensive.
        // Lazily initialize pipelines and share pipelines when possible.
        // TODO: Should we delete unused pipelines when changes require a new pipeline?
        let pipeline_key = PipelineKey::new(
            mesh_object.disable_depth_write,
            mesh_object.disable_depth_test,
            material,
            RGBA_COLOR_FORMAT,
        );

        pipelines
            .entry(pipeline_key)
            .or_insert_with(|| pipeline(device, &self.shared_data.pipeline_data, &pipeline_key));

        let vertex_count = mesh_object.vertex_count()?;

        // TODO: Function for this?
        let adjacency = adj_entry
            .map(|e| e.vertex_adjacency.iter().map(|i| *i as i32).collect())
            .unwrap_or_else(|| vec![-1i32; vertex_count * 18]);
        let adj_buffer = device.create_buffer_from_data(
            "Adjacency Buffer",
            &adjacency,
            wgpu::BufferUsages::STORAGE,
        );

        // This is applied after skinning, so the source and destination buffer are the same.
        // TODO: Can this be done in a single dispatch for the entire model?
        // TODO: Add a proper error for empty meshes.
        // TODO: Investigate why empty meshes crash on emulators.
        let buffer0_binding = access.buffer0.binding(&buffers.vertex_buffer0)?;
        let buffer0_source_binding = access.buffer0.binding(&buffers.vertex_buffer0_source)?;
        let weights_binding = access.weights.binding(&buffers.skinning_buffer)?;

        let renormal_bind_group = crate::shader::renormal::bind_groups::BindGroup0::from_bindings(
            device,
            crate::shader::renormal::bind_groups::BindGroupLayout0 {
                vertices: buffer0_binding.clone(),
                adj_data: adj_buffer.as_entire_buffer_binding(),
            },
        );

        let skinning_bind_group = crate::shader::skinning::bind_groups::BindGroup0::from_bindings(
            device,
            crate::shader::skinning::bind_groups::BindGroupLayout0 {
                src: buffer0_source_binding,
                vertex_weights: weights_binding,
                dst: buffer0_binding.clone(),
            },
        );

        let skinning_transforms_bind_group =
            crate::shader::skinning::bind_groups::BindGroup1::from_bindings(
                device,
                crate::shader::skinning::bind_groups::BindGroupLayout1 {
                    transforms: transforms.skinning_transforms.as_entire_buffer_binding(),
                    world_transforms: transforms.world_transforms.as_entire_buffer_binding(),
                },
            );

        let parent_index = find_parent_index(mesh_object, self.skel);
        let mesh_object_info_buffer = device.create_buffer_from_data(
            "Mesh Object Info Buffer",
            &[crate::shader::skinning::MeshObjectInfo {
                parent_index: glam::IVec4::new(parent_index, -1, -1, -1),
            }],
            wgpu::BufferUsages::UNIFORM,
        );

        let mesh_object_info_bind_group =
            crate::shader::skinning::bind_groups::BindGroup2::from_bindings(
                device,
                crate::shader::skinning::bind_groups::BindGroupLayout2 {
                    mesh_object_info: mesh_object_info_buffer.as_entire_buffer_binding(),
                },
            );

        // The end of the shader label is used to determine draw order.
        // ex: "SFX_PBS_0101000008018278_sort" has a tag of "sort".
        // The render order is opaque -> far -> sort -> near.
        // TODO: How to handle missing tags?
        let shader_label = material
            .map(|m| m.shader_label.as_str())
            .unwrap_or("")
            .to_string();

        let attribute_names = mesh_object
            .positions
            .iter()
            .map(|a| a.name.clone())
            .chain(mesh_object.normals.iter().map(|a| a.name.clone()))
            .chain(mesh_object.tangents.iter().map(|a| a.name.clone()))
            .chain(
                mesh_object
                    .texture_coordinates
                    .iter()
                    .map(|a| a.name.clone()),
            )
            .chain(mesh_object.color_sets.iter().map(|a| a.name.clone()))
            .collect();

        // TODO: Set entry flags?
        Ok(RenderMesh {
            name: mesh_object.name.clone(),
            material_label: material_label.clone(),
            shader_label,
            is_visible: true,
            is_selected: false,
            meshex_flags: meshex_flags.unwrap_or(EntryFlags {
                draw_model: true,
                cast_shadow: true,
            }),
            skinning_bind_group,
            skinning_transforms_bind_group,
            mesh_object_info_bind_group,
            pipeline_key,
            renormal_bind_group,
            subindex: mesh_object.subindex,
            vertex_count,
            vertex_index_count: mesh_object.vertex_indices.len(),
            access,
            attribute_names,
        })
    }
}

fn bone_bind_group1(
    device: &wgpu::Device,
    world_transforms: &wgpu::Buffer,
    bone_colors: &wgpu::Buffer,
) -> crate::shader::skeleton::bind_groups::BindGroup1 {
    crate::shader::skeleton::bind_groups::BindGroup1::from_bindings(
        device,
        crate::shader::skeleton::bind_groups::BindGroupLayout1 {
            world_transforms: world_transforms.as_entire_buffer_binding(),
            bone_colors: bone_colors.as_entire_buffer_binding(),
        },
    )
}

pub fn material_data(
    device: &wgpu::Device,
    material: &MatlEntryData,
    textures: &[(String, wgpu::Texture, wgpu::TextureViewDimension)],
    shared_data: &SharedRenderData,
    sampler_by_data: &mut SamplerCache,
) -> Material {
    let uniforms_buffer = uniforms_buffer(material, device, &shared_data.database);
    let material_uniforms_bind_group = material_uniforms_bind_group(
        material,
        device,
        textures,
        &shared_data.default_textures,
        &uniforms_buffer,
        sampler_by_data,
    );

    Material {
        material_uniforms_bind_group,
        uniforms_buffer,
    }
}

pub fn default_material_data(device: &wgpu::Device, shared_data: &SharedRenderData) -> Material {
    let uniforms_buffer = default_uniforms_buffer(device);
    let material_uniforms_bind_group = default_material_uniforms_bind_group(
        device,
        &shared_data.default_textures,
        &uniforms_buffer,
    );

    Material {
        material_uniforms_bind_group,
        uniforms_buffer,
    }
}

pub struct MeshBufferAccess {
    pub buffer0: BufferAccess,
    pub buffer1: BufferAccess,
    pub weights: BufferAccess,
    pub indices: BufferAccess,
}

pub struct BufferAccess {
    start: u64,
    size: u64,
}

// TODO: Avoid creating empty accesses entirely.
// TODO: Double check that this handles empty meshes properly.
impl BufferAccess {
    pub fn binding<'a>(
        &self,
        buffer: &'a wgpu::Buffer,
    ) -> Result<wgpu::BufferBinding<'a>, Box<dyn Error>> {
        Ok(wgpu::BufferBinding::<'a> {
            buffer,
            offset: self.start,
            size: Some(
                NonZeroU64::new(self.size)
                    .ok_or("Mesh has no vertices. Failed to create vertex buffers.")?,
            ),
        })
    }

    pub fn slice<'a>(&self, buffer: &'a wgpu::Buffer) -> wgpu::BufferSlice<'a> {
        buffer.slice(self.start..self.start + self.size)
    }
}

fn append_mesh_object_buffer_data(
    model_buffer0_data: &mut DynamicStorageBuffer<Vec<u8>>,
    model_buffer1_data: &mut Vec<u8>,
    model_skin_weights_data: &mut DynamicStorageBuffer<Vec<u8>>,
    model_index_data: &mut Vec<u32>,
    mesh_object: &MeshObjectData,
    shared_data: &RenderMeshSharedData,
) -> Result<MeshBufferAccess, ssbh_data::mesh_data::error::Error> {
    // DynamicStorageBuffer enforces the offset alignment for each mesh.
    let buffer0_vertices = buffer0(mesh_object)?;
    let buffer0_vertices: &[crate::shader::skinning::VertexInput0] =
        bytemuck::cast_slice(&buffer0_vertices);
    let buffer0_offset = model_buffer0_data
        .write(bytemuck::cast_slice::<
            _,
            crate::shader::skinning::VertexInput0,
        >(buffer0_vertices))
        .unwrap();

    // Only buffer0 vertices are used as a storage buffer.
    let buffer1_vertices = buffer1(mesh_object)?;
    let buffer1_data: &[u8] = bytemuck::cast_slice(&buffer1_vertices);
    let buffer1_offset = model_buffer1_data.len() as u64;
    model_buffer1_data.extend_from_slice(bytemuck::cast_slice(buffer1_data));

    let skin_weights = skin_weights(mesh_object, shared_data.skel)?;
    let weights_offset = model_skin_weights_data.write(&skin_weights).unwrap();

    // Only the index buffer is tightly packed.
    let index_offset = (model_index_data.len() * std::mem::size_of::<u32>()) as u64;
    model_index_data.extend_from_slice(&mesh_object.vertex_indices);

    Ok(MeshBufferAccess {
        buffer0: BufferAccess {
            start: buffer0_offset,
            size: buffer0_vertices.size().get(),
        },
        buffer1: BufferAccess {
            start: buffer1_offset,
            size: buffer1_data.len() as u64,
        },
        weights: BufferAccess {
            start: weights_offset,
            size: skin_weights.size().get(),
        },
        indices: BufferAccess {
            start: index_offset,
            size: (mesh_object.vertex_indices.len() * std::mem::size_of::<u32>()) as u64,
        },
    })
}

fn bone_bind_groups(
    device: &wgpu::Device,
    skel: Option<&SkelData>,
) -> Vec<crate::shader::skeleton::bind_groups::BindGroup2> {
    skel.map(|skel| {
        skel.bones
            .iter()
            .enumerate()
            .map(|(i, bone)| {
                // TODO: Use instancing instead.
                let per_bone = device.create_buffer_from_data(
                    "Mesh Object Info Buffer",
                    &[crate::shader::skeleton::PerBone {
                        indices: glam::IVec4::new(
                            i as i32,
                            parent_index(bone.parent_index),
                            -1,
                            -1,
                        ),
                    }],
                    wgpu::BufferUsages::UNIFORM,
                );

                crate::shader::skeleton::bind_groups::BindGroup2::from_bindings(
                    device,
                    crate::shader::skeleton::bind_groups::BindGroupLayout2 {
                        per_bone: per_bone.as_entire_buffer_binding(),
                    },
                )
            })
            .collect()
    })
    .unwrap_or_default()
}

// TODO: Where to put this?
// TODO: Module for skinning buffers?
fn parent_index(index: Option<usize>) -> i32 {
    index.map(|i| i as i32).unwrap_or(-1)
}

fn find_parent_index(mesh: &MeshObjectData, skel: Option<&SkelData>) -> i32 {
    // Only include a parent if there are no bone influences.
    // TODO: What happens if there are influences and a parent bone?
    if mesh.bone_influences.is_empty() {
        parent_index(skel.as_ref().and_then(|skel| {
            skel.bones
                .iter()
                .position(|b| b.name == mesh.parent_bone_name)
        }))
    } else {
        -1
    }
}
