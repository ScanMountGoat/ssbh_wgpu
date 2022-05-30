use crate::{
    animation::{animate_materials, animate_skel, animate_visibility, AnimationTransforms},
    pipeline::{create_pipeline, PipelineKey},
    renderer::DebugMode,
    texture::{load_sampler, load_texture},
    uniforms::create_uniforms_buffer,
    vertex::{buffer0, buffer1, mesh_object_buffers, skin_weights, MeshObjectBufferData},
    PipelineData, ShaderDatabase,
};
use glam::Vec4Swizzles;
use nutexb_wgpu::NutexbFile;
use ssbh_data::{
    adj_data::AdjEntryData,
    matl_data::{MatlEntryData, ParamId},
    mesh_data::MeshObjectData,
    prelude::*,
};
use std::collections::HashMap;
use wgpu::{util::DeviceExt, SamplerDescriptor, TextureViewDescriptor};

// Group resources shared between mesh objects.
// Shared resources can be updated once per model instead of per mesh.
// Keep most fields private since the buffer layout is an implementation detail.
// Assume render data is only shared within a folder.
// TODO: Associate animation folders with model folders?
// TODO: Is it worth allowing models to reference textures from other folders?
pub struct RenderModel {
    pub meshes: Vec<RenderMesh>,
    skel: Option<SkelData>,
    matl: Option<MatlData>,
    hlpb: Option<HlpbData>,
    mesh_buffers: MeshBuffers,
    material_data_by_label: HashMap<String, MaterialData>,
    pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
    textures: Vec<(String, wgpu::Texture)>, // (file name, texture)
    // TODO: Store this with the renderer since we only need one?
    // TODO: Avoid storing duplicate geometry for the scaled version?
    bone_vertex_buffer: wgpu::Buffer,
    bone_vertex_buffer_outer: wgpu::Buffer,
    bone_index_buffer: wgpu::Buffer,
    joint_vertex_buffer: wgpu::Buffer,
    joint_vertex_buffer_outer: wgpu::Buffer,
    joint_index_buffer: wgpu::Buffer,
    joint_world_transforms_buffer: wgpu::Buffer,
    bone_data_bind_group: crate::shader::skeleton::bind_groups::BindGroup1,
    bone_data_outer_bind_group: crate::shader::skeleton::bind_groups::BindGroup1,
    joint_data_bind_group: crate::shader::skeleton::bind_groups::BindGroup1,
    joint_data_outer_bind_group: crate::shader::skeleton::bind_groups::BindGroup1,

    // TODO: Use instancing instead.
    bone_bind_groups: Vec<crate::shader::skeleton::bind_groups::BindGroup2>,
    buffer_data: MeshObjectBufferData,
}

// A RenderMesh is view over a portion of the RenderModel data.
// TODO: All the render data should be owned by the RenderModel.
// Each RenderMesh corresponds to the data for a single draw call.
pub struct RenderMesh {
    pub name: String,
    pub is_visible: bool,
    material_label: String,
    shader_label: String,
    shader_tag: String, // TODO: Don't store the tag?
    sub_index: u64,
    sort_bias: i32,
    normals_bind_group: crate::shader::renormal::bind_groups::BindGroup0,
    skinning_bind_group: crate::shader::skinning::bind_groups::BindGroup0,
    skinning_transforms_bind_group: crate::shader::skinning::bind_groups::BindGroup1,
    mesh_object_info_bind_group: crate::shader::skinning::bind_groups::BindGroup2,
    // TODO: How to update this when materials/shaders change?
    pipeline_key: PipelineKey,
    vertex_count: usize,
    vertex_index_count: usize,
    access: MeshBufferAccess,
    attribute_names: Vec<String>,
}

impl RenderMesh {
    pub fn render_order(&self) -> isize {
        render_pass_index(&self.shader_tag) + self.sort_bias as isize
    }
}

struct MaterialData {
    material_uniforms_bind_group: crate::shader::model::bind_groups::BindGroup1,
    uniforms_buffer: wgpu::Buffer,
}

struct MeshBuffers {
    // TODO: Share vertex buffers?
    skinning_transforms: wgpu::Buffer,
    world_transforms: wgpu::Buffer,
}

impl RenderModel {
    /// Reassign the mesh materials based on `modl`.
    /// This does not create materials that do not already exist.
    pub fn reassign_materials(&mut self, modl: &ModlData) {
        // TODO: There should be a separate pipeline to use if the material assignment fails?
        // TODO: How does in game handle invalid material labels?
        for mesh in &mut self.meshes {
            if let Some(entry) = modl.entries.iter().find(|e| {
                e.mesh_object_name == mesh.name && e.mesh_object_sub_index == mesh.sub_index
            }) {
                mesh.material_label = entry.material_label.clone();
            }
        }
    }

    /// Update the render data associated with `material`.
    pub fn update_material(
        &mut self,
        device: &wgpu::Device,
        material: &MatlEntryData,
        pipeline_data: &PipelineData,
        default_textures: &[(String, wgpu::Texture)],
        stage_cube: &(wgpu::TextureView, wgpu::Sampler),
    ) {
        if let Some(data) = self
            .material_data_by_label
            .get_mut(&material.material_label)
        {
            // TODO: Update textures and materials separately?
            // TODO: Reuse the existing uniforms buffer and write to it instead.
            let uniforms_buffer = create_uniforms_buffer(Some(material), device);
            data.material_uniforms_bind_group = create_material_uniforms_bind_group(
                Some(material),
                device,
                &self.textures,
                default_textures,
                stage_cube,
                &uniforms_buffer,
            );

            // Create a new pipeline if needed.
            // TODO: How to get the mesh depth write and depth test information?
            let pipeline_key = PipelineKey::new(false, false, Some(material));
            self.pipelines
                .entry(pipeline_key)
                .or_insert_with(|| create_pipeline(device, pipeline_data, &pipeline_key));

            // Update the pipeline key for associated RenderMeshes.
            for mesh in self
                .meshes
                .iter_mut()
                .filter(|m| m.material_label == material.material_label)
            {
                mesh.pipeline_key = pipeline_key;
            }
        }
    }

    // TODO: Does it make sense to just pass None to "reset" the animation?
    pub fn apply_anim(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        anim: Option<&AnimData>,
        frame: f32,
        pipeline_data: &PipelineData,
        default_textures: &[(String, wgpu::Texture)],
        stage_cube: &(wgpu::TextureView, wgpu::Sampler),
    ) {
        // Update the buffers associated with each skel.
        // This avoids updating per mesh object and allocating new buffers.
        // TODO: How to "reset" an animation?

        if let Some(anim) = anim {
            animate_visibility(anim, frame, &mut self.meshes);

            if let Some(matl) = &self.matl {
                // Get a list of changed materials.
                let animated_materials = animate_materials(anim, frame, &matl.entries);
                for material in animated_materials {
                    // TODO: Should this go in a separate module?
                    // Get updated uniform buffers for animated materials
                    self.update_material(
                        device,
                        &material,
                        pipeline_data,
                        default_textures,
                        stage_cube,
                    );
                }
            }

            if let Some(skel) = &self.skel {
                let animation_transforms = animate_skel(skel, anim, self.hlpb.as_ref(), frame);
                queue.write_buffer(
                    &self.mesh_buffers.skinning_transforms,
                    0,
                    bytemuck::cast_slice(&[*animation_transforms.animated_world_transforms]),
                );

                queue.write_buffer(
                    &self.mesh_buffers.world_transforms,
                    0,
                    bytemuck::cast_slice(&(*animation_transforms.world_transforms)),
                );

                let joint_transforms = joint_transforms(skel, &animation_transforms);
                queue.write_buffer(
                    &self.joint_world_transforms_buffer,
                    0,
                    bytemuck::cast_slice(&joint_transforms),
                );
            }
        }
    }

    pub fn draw_skeleton<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a crate::shader::skeleton::bind_groups::BindGroup0,
        bone_pipeline: &'a wgpu::RenderPipeline,
        bone_outer_pipeline: &'a wgpu::RenderPipeline,
        joint_pipeline: &'a wgpu::RenderPipeline,
        joint_outer_pipeline: &'a wgpu::RenderPipeline,
    ) {
        if let Some(skel) = self.skel.as_ref() {
            self.draw_joints(
                render_pass,
                skel,
                camera_bind_group,
                joint_pipeline,
                joint_outer_pipeline,
            );
            // Draw the bones last to cover up the geometry at the ends of the joints.
            self.draw_bones(
                render_pass,
                skel,
                camera_bind_group,
                bone_pipeline,
                bone_outer_pipeline,
            );
        }
    }

    fn draw_joints<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        skel: &SkelData,
        camera_bind_group: &'a crate::shader::skeleton::bind_groups::BindGroup0,
        skeleton_pipeline: &'a wgpu::RenderPipeline,
        skeleton_outer_pipeline: &'a wgpu::RenderPipeline,
    ) {
        render_pass.set_index_buffer(self.joint_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        
        render_pass.set_pipeline(skeleton_outer_pipeline);
        render_pass.set_vertex_buffer(0, self.joint_vertex_buffer_outer.slice(..));
        for i in 0..skel.bones.len() {
            // Check for the parent to only draw joints between connected bones.
            if skel.bones[i].parent_index.is_some() {
                crate::shader::skeleton::bind_groups::set_bind_groups(
                    render_pass,
                    crate::shader::skeleton::bind_groups::BindGroups::<'a> {
                        bind_group0: camera_bind_group,
                        bind_group1: &self.joint_data_outer_bind_group,
                        bind_group2: &self.bone_bind_groups[i],
                    },
                );

                render_pass.draw_indexed(0..pyramid_indices().len() as u32, 0, 0..1);
            }
        }

        render_pass.set_pipeline(skeleton_pipeline);
        render_pass.set_vertex_buffer(0, self.joint_vertex_buffer.slice(..));
        for i in 0..skel.bones.len() {
            if skel.bones[i].parent_index.is_some() {
                crate::shader::skeleton::bind_groups::set_bind_groups(
                    render_pass,
                    crate::shader::skeleton::bind_groups::BindGroups::<'a> {
                        bind_group0: camera_bind_group,
                        bind_group1: &self.joint_data_bind_group,
                        bind_group2: &self.bone_bind_groups[i],
                    },
                );

                render_pass.draw_indexed(0..pyramid_indices().len() as u32, 0, 0..1);
            }
        }
    }

    fn draw_bones<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        skel: &SkelData,
        camera_bind_group: &'a crate::shader::skeleton::bind_groups::BindGroup0,
        skeleton_pipeline: &'a wgpu::RenderPipeline,
        skeleton_outer_pipeline: &'a wgpu::RenderPipeline,
    ) {
        render_pass.set_pipeline(skeleton_outer_pipeline);
        render_pass.set_index_buffer(self.bone_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        // TODO: Instancing?
        render_pass.set_vertex_buffer(0, self.bone_vertex_buffer_outer.slice(..));
        for i in 0..skel.bones.len() {
            crate::shader::skeleton::bind_groups::set_bind_groups(
                render_pass,
                crate::shader::skeleton::bind_groups::BindGroups::<'a> {
                    bind_group0: camera_bind_group,
                    bind_group1: &self.bone_data_outer_bind_group,
                    bind_group2: &self.bone_bind_groups[i],
                },
            );

            render_pass.draw_indexed(0..sphere_indices().len() as u32, 0, 0..1);
        }
        render_pass.set_pipeline(skeleton_pipeline);
        render_pass.set_vertex_buffer(0, self.bone_vertex_buffer.slice(..));
        for i in 0..skel.bones.len() {
            crate::shader::skeleton::bind_groups::set_bind_groups(
                render_pass,
                crate::shader::skeleton::bind_groups::BindGroups::<'a> {
                    bind_group0: camera_bind_group,
                    bind_group1: &self.bone_data_bind_group,
                    bind_group2: &self.bone_bind_groups[i],
                },
            );

            render_pass.draw_indexed(0..sphere_indices().len() as u32, 0, 0..1);
        }
    }

    pub fn draw_render_meshes<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        per_frame_bind_group: &'a crate::shader::model::bind_groups::BindGroup0,
        stage_uniforms_bind_group: &'a crate::shader::model::bind_groups::BindGroup2,
        shader_database: &ShaderDatabase,
        invalid_shader_pipeline: &'a wgpu::RenderPipeline,
        invalid_attributes_pipeline: &'a wgpu::RenderPipeline,
    ) {
        // TODO: How to store all data in RenderModel but still draw sorted meshes?
        for mesh in self.meshes.iter().filter(|m| m.is_visible) {
            // Mesh objects with no modl entry or an invalid material label are skipped entirely in game.
            if let Some(material_data) = self.material_data_by_label.get(&mesh.material_label) {
                // TODO: Does the invalid shader pipeline take priority?
                // TODO: How to handle missing position, tangent, normal?
                if let Some(info) = shader_database.get(&mesh.shader_label) {
                    if info
                        .vertex_attributes
                        .iter()
                        .all(|a| mesh.attribute_names.contains(a))
                    {
                        // TODO: Don't assume the pipeline exists?
                        render_pass.set_pipeline(&self.pipelines[&mesh.pipeline_key]);
                    } else {
                        render_pass.set_pipeline(invalid_attributes_pipeline);
                    }
                } else {
                    // TODO: Does this include invalid tags?
                    render_pass.set_pipeline(invalid_shader_pipeline);
                }

                crate::shader::model::bind_groups::set_bind_groups(
                    render_pass,
                    crate::shader::model::bind_groups::BindGroups::<'a> {
                        bind_group0: per_frame_bind_group,
                        bind_group1: &material_data.material_uniforms_bind_group,
                        bind_group2: stage_uniforms_bind_group,
                    },
                );

                self.set_mesh_buffers(render_pass, mesh);

                render_pass.draw_indexed(0..mesh.vertex_index_count as u32, 0, 0..1);
            }
        }
    }

    pub fn draw_render_meshes_debug<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        per_frame_bind_group: &'a crate::shader::model::bind_groups::BindGroup0,
        stage_uniforms_bind_group: &'a crate::shader::model::bind_groups::BindGroup2,
    ) {
        // Assume the pipeline is already set.
        for mesh in self.meshes.iter().filter(|m| m.is_visible) {
            if let Some(material_data) = self.material_data_by_label.get(&mesh.material_label) {
                crate::shader::model::bind_groups::set_bind_groups(
                    render_pass,
                    crate::shader::model::bind_groups::BindGroups::<'a> {
                        bind_group0: per_frame_bind_group,
                        bind_group1: &material_data.material_uniforms_bind_group,
                        bind_group2: stage_uniforms_bind_group,
                    },
                );

                self.set_mesh_buffers(render_pass, mesh);

                render_pass.draw_indexed(0..mesh.vertex_index_count as u32, 0, 0..1);
            }
        }
    }

    fn set_mesh_buffers<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, mesh: &RenderMesh) {
        render_pass.set_vertex_buffer(
            0,
            self.buffer_data.vertex_buffer0.slice(
                mesh.access.buffer0_start..mesh.access.buffer0_start + mesh.access.buffer0_size,
            ),
        );
        render_pass.set_vertex_buffer(
            1,
            self.buffer_data.vertex_buffer1.slice(
                mesh.access.buffer1_start..mesh.access.buffer1_start + mesh.access.buffer1_size,
            ),
        );
        render_pass.set_index_buffer(
            self.buffer_data.index_buffer.slice(
                mesh.access.indices_start..mesh.access.indices_start + mesh.access.indices_size,
            ),
            wgpu::IndexFormat::Uint32,
        );
    }

    pub fn draw_render_meshes_depth<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a crate::shader::model_depth::bind_groups::BindGroup0,
    ) {
        for mesh in self.meshes.iter().filter(|m| m.is_visible) {
            crate::shader::model_depth::bind_groups::set_bind_groups(
                render_pass,
                crate::shader::model_depth::bind_groups::BindGroups::<'a> {
                    bind_group0: camera_bind_group,
                },
            );

            self.set_mesh_buffers(render_pass, mesh);

            render_pass.draw_indexed(0..mesh.vertex_index_count as u32, 0, 0..1);
        }
    }
}

// TODO: Come up with a more descriptive name for this.
pub struct RenderMeshSharedData<'a> {
    pub pipeline_data: &'a PipelineData,
    pub default_textures: &'a [(String, wgpu::Texture)],
    pub stage_cube: &'a (wgpu::TextureView, wgpu::Sampler),
    pub mesh: Option<&'a MeshData>,
    pub modl: Option<&'a ModlData>,
    pub skel: Option<&'a SkelData>,
    pub matl: Option<&'a MatlData>,
    pub adj: Option<&'a AdjData>,
    pub hlpb: Option<&'a HlpbData>,
    pub nutexbs: &'a [(String, NutexbFile)],
}

fn pyramid() -> Vec<[f32; 3]> {
    // Pos0 Nrm0 Pos1 Nrm1 ...
    vec![
        [0.000000, 1.000000, 0.000000],
        [-0.1400, 0.9901, 0.0000],
        [-0.707107, 0.900000, -0.707107],
        [-0.7863, -0.6178, 0.0000],
        [0.707107, 0.900000, -0.707107],
        [0.7863, -0.6178, 0.0000],
        [0.707107, 0.900000, 0.707107],
        [0.0000, -0.6178, 0.7863],
        [-0.707107, 0.900000, 0.707107],
        [-0.7863, -0.6178, 0.0000],
        [0.000000, 0.000000, 0.000000],
        [-0.7863, -0.6178, 0.0000],
        [0.707107, 0.900000, -0.707107],
        [0.0000, 0.9901, -0.1400],
        [0.707107, 0.900000, -0.707107],
        [0.0000, -0.6178, -0.7863],
        [0.707107, 0.900000, -0.707107],
        [0.1400, 0.9901, 0.0000],
        [0.000000, 1.000000, 0.000000],
        [0.0000, 0.9901, -0.1400],
        [0.000000, 1.000000, 0.000000],
        [0.1400, 0.9901, 0.0000],
        [0.000000, 1.000000, 0.000000],
        [0.0000, 0.9901, 0.1400],
        [-0.707107, 0.900000, -0.707107],
        [0.0000, 0.9901, -0.1400],
        [-0.707107, 0.900000, -0.707107],
        [0.0000, -0.6178, -0.7863],
        [-0.707107, 0.900000, -0.707107],
        [-0.1400, 0.9901, 0.0000],
        [0.000000, 0.000000, 0.000000],
        [0.0000, -0.6178, -0.7863],
        [0.000000, 0.000000, 0.000000],
        [0.7863, -0.6178, 0.0000],
        [0.000000, 0.000000, 0.000000],
        [0.0000, -0.6178, 0.7863],
        [0.707107, 0.900000, 0.707107],
        [0.1400, 0.9901, 0.0000],
        [0.707107, 0.900000, 0.707107],
        [0.7863, -0.6178, 0.0000],
        [0.707107, 0.900000, 0.707107],
        [0.0000, 0.9901, 0.1400],
        [-0.707107, 0.900000, 0.707107],
        [0.0000, 0.9901, 0.1400],
        [-0.707107, 0.900000, 0.707107],
        [0.0000, -0.6178, 0.7863],
        [-0.707107, 0.900000, 0.707107],
        [-0.1400, 0.9901, 0.0000],
        
    ]
}

fn pyramid_outer() -> Vec<[f32; 3]> {
    // TODO: Scale positions along the normal direction?
    let scale = 1.25;
    pyramid()
        .iter()
        .map(|v| [v[0] * scale, v[1], v[2] * scale])
        .collect()
}

fn pyramid_indices() -> Vec<u32> {
    // An inverted pyramid with a pyramid base.
    vec![9, 6, 12,
    13, 7, 15,
    10, 18, 8,
    2, 19, 16,
    11, 21, 20,
    3, 22, 17,
    0, 14, 23,
    4, 1, 5,]
}

fn sphere() -> Vec<[f32; 3]> {
    // Pos0 Nrm0 Pos1 Nrm1 ...
    vec![
        [0.000000, 0.923880, -0.382683],
        [0.000000, 0.923880, -0.382683],
        [0.000000, 0.707107, -0.707107],
        [0.000000, 0.707107, -0.707107],
        [0.000000, 0.382683, -0.923880],
        [0.000000, 0.382683, -0.923880],
        [0.000000, -0.000000, -1.000000],
        [0.000000, -0.000000, -1.000000],
        [0.000000, -0.382683, -0.923880],
        [0.000000, -0.382683, -0.923880],
        [0.000000, -0.707107, -0.707107],
        [0.000000, -0.707107, -0.707107],
        [0.000000, -0.923880, -0.382683],
        [0.000000, -0.923880, -0.382683],
        [0.270598, 0.923880, -0.270598],
        [0.270598, 0.923880, -0.270598],
        [0.500000, 0.707107, -0.500000],
        [0.500000, 0.707107, -0.500000],
        [0.653282, 0.382683, -0.653281],
        [0.653282, 0.382683, -0.653281],
        [0.707107, -0.000000, -0.707107],
        [0.707107, -0.000000, -0.707107],
        [0.653282, -0.382683, -0.653282],
        [0.653282, -0.382683, -0.653282],
        [0.500000, -0.707107, -0.500000],
        [0.500000, -0.707107, -0.500000],
        [0.270598, -0.923880, -0.270598],
        [0.270598, -0.923880, -0.270598],
        [0.382684, 0.923880, 0.000000],
        [0.382684, 0.923880, 0.000000],
        [0.707107, 0.707107, 0.000000],
        [0.707107, 0.707107, 0.000000],
        [0.923880, 0.382683, 0.000000],
        [0.923880, 0.382683, 0.000000],
        [1.000000, -0.000000, 0.000000],
        [1.000000, -0.000000, 0.000000],
        [0.923880, -0.382683, 0.000000],
        [0.923880, -0.382683, 0.000000],
        [0.707107, -0.707107, 0.000000],
        [0.707107, -0.707107, 0.000000],
        [0.382684, -0.923880, 0.000000],
        [0.382684, -0.923880, 0.000000],
        [0.270598, 0.923880, 0.270598],
        [0.270598, 0.923880, 0.270598],
        [0.500000, 0.707107, 0.500000],
        [0.500000, 0.707107, 0.500000],
        [0.653282, 0.382683, 0.653282],
        [0.653282, 0.382683, 0.653282],
        [0.707107, -0.000000, 0.707107],
        [0.707107, -0.000000, 0.707107],
        [0.653282, -0.382683, 0.653282],
        [0.653282, -0.382683, 0.653282],
        [0.500000, -0.707107, 0.500000],
        [0.500000, -0.707107, 0.500000],
        [0.270598, -0.923880, 0.270598],
        [0.270598, -0.923880, 0.270598],
        [0.000000, 0.923880, 0.382684],
        [0.000000, 0.923880, 0.382684],
        [0.000000, 0.707107, 0.707107],
        [0.000000, 0.707107, 0.707107],
        [0.000000, 0.382683, 0.923880],
        [0.000000, 0.382683, 0.923880],
        [0.000000, -0.000000, 1.000000],
        [0.000000, -0.000000, 1.000000],
        [0.000000, -0.382683, 0.923880],
        [0.000000, -0.382683, 0.923880],
        [0.000000, -0.707107, 0.707107],
        [0.000000, -0.707107, 0.707107],
        [0.000000, -0.923880, 0.382684],
        [0.000000, -0.923880, 0.382684],
        [-0.000000, 1.000000, 0.000000],
        [-0.000000, 1.000000, 0.000000],
        [-0.270598, 0.923880, 0.270598],
        [-0.270598, 0.923880, 0.270598],
        [-0.500000, 0.707107, 0.500000],
        [-0.500000, 0.707107, 0.500000],
        [-0.653281, 0.382683, 0.653282],
        [-0.653281, 0.382683, 0.653282],
        [-0.707107, -0.000000, 0.707107],
        [-0.707107, -0.000000, 0.707107],
        [-0.653282, -0.382683, 0.653282],
        [-0.653282, -0.382683, 0.653282],
        [-0.500000, -0.707107, 0.500000],
        [-0.500000, -0.707107, 0.500000],
        [-0.270598, -0.923880, 0.270598],
        [-0.270598, -0.923880, 0.270598],
        [-0.382684, 0.923880, 0.000000],
        [-0.382684, 0.923880, 0.000000],
        [-0.707107, 0.707107, 0.000000],
        [-0.707107, 0.707107, 0.000000],
        [-0.923879, 0.382683, 0.000000],
        [-0.923879, 0.382683, 0.000000],
        [-1.000000, -0.000000, 0.000000],
        [-1.000000, -0.000000, 0.000000],
        [-0.923880, -0.382683, 0.000000],
        [-0.923880, -0.382683, 0.000000],
        [-0.707107, -0.707107, 0.000000],
        [-0.707107, -0.707107, 0.000000],
        [-0.382684, -0.923880, 0.000000],
        [-0.382684, -0.923880, 0.000000],
        [0.000000, -1.000000, 0.000000],
        [0.000000, -1.000000, 0.000000],
        [-0.270598, 0.923880, -0.270598],
        [-0.270598, 0.923880, -0.270598],
        [-0.500000, 0.707107, -0.500000],
        [-0.500000, 0.707107, -0.500000],
        [-0.653281, 0.382683, -0.653281],
        [-0.653281, 0.382683, -0.653281],
        [-0.707107, -0.000000, -0.707107],
        [-0.707107, -0.000000, -0.707107],
        [-0.653282, -0.382683, -0.653281],
        [-0.653282, -0.382683, -0.653281],
        [-0.500000, -0.707107, -0.500000],
        [-0.500000, -0.707107, -0.500000],
        [-0.270598, -0.923880, -0.270598],
        [-0.270598, -0.923880, -0.270598],
    ]
}

fn sphere_outer() -> Vec<[f32; 3]> {
    let scale = 1.25;
    sphere()
        .iter()
        .map(|v| [v[0] * scale, v[1] * scale, v[2] * scale])
        .collect()
}

fn sphere_indices() -> Vec<u32> {
    vec![
        5, 13, 6, 3, 11, 4, 1, 9, 2, 0, 35, 7, 50, 6, 13, 4, 12, 5, 2, 10, 3, 0, 8, 1, 7, 35, 14,
        50, 13, 20, 11, 19, 12, 9, 17, 10, 7, 15, 8, 12, 20, 13, 11, 17, 18, 8, 16, 9, 19, 25, 26,
        16, 24, 17, 14, 22, 15, 19, 27, 20, 18, 24, 25, 15, 23, 16, 14, 35, 21, 50, 20, 27, 26, 34,
        27, 24, 32, 25, 22, 30, 23, 21, 35, 28, 50, 27, 34, 25, 33, 26, 23, 31, 24, 21, 29, 22, 31,
        40, 32, 29, 38, 30, 28, 35, 36, 50, 34, 42, 32, 41, 33, 30, 39, 31, 28, 37, 29, 34, 41, 42,
        50, 42, 49, 40, 48, 41, 38, 46, 39, 36, 44, 37, 42, 48, 49, 40, 46, 47, 38, 44, 45, 36, 35,
        43, 45, 54, 46, 43, 52, 44, 48, 57, 49, 46, 55, 47, 44, 53, 45, 43, 35, 51, 50, 49, 57, 47,
        56, 48, 56, 6, 57, 54, 4, 55, 52, 2, 53, 51, 35, 0, 50, 57, 6, 55, 5, 56, 54, 2, 3, 52, 0,
        1, 5, 12, 13, 3, 10, 11, 1, 8, 9, 4, 11, 12, 2, 9, 10, 0, 7, 8, 11, 18, 19, 9, 16, 17, 7,
        14, 15, 12, 19, 20, 11, 10, 17, 8, 15, 16, 19, 18, 25, 16, 23, 24, 14, 21, 22, 19, 26, 27,
        18, 17, 24, 15, 22, 23, 26, 33, 34, 24, 31, 32, 22, 29, 30, 25, 32, 33, 23, 30, 31, 21, 28,
        29, 31, 39, 40, 29, 37, 38, 32, 40, 41, 30, 38, 39, 28, 36, 37, 34, 33, 41, 40, 47, 48, 38,
        45, 46, 36, 43, 44, 42, 41, 48, 40, 39, 46, 38, 37, 44, 45, 53, 54, 43, 51, 52, 48, 56, 57,
        46, 54, 55, 44, 52, 53, 47, 55, 56, 56, 5, 6, 54, 3, 4, 52, 1, 2, 55, 4, 5, 54, 53, 2, 52,
        51, 0,
    ]
}

fn bone_colors(skel: Option<&SkelData>, hlpb: Option<&HlpbData>) -> Vec<[f32; 4]> {
    // Match the color scheme used for the Blender addon.
    let helper_color = [0.3, 0.0, 0.6, 1.0];
    let default_color = [0.65, 0.65, 0.65, 1.0];

    let mut colors = vec![[0.0; 4]; crate::animation::MAX_BONE_COUNT];
    if let Some(skel) = skel {
        for (i, bone) in skel.bones.iter().enumerate() {
            colors[i] = default_color;

            // TODO: Check for swing bones.

            // Color helper bones using a different color.
            if let Some(hlpb) = hlpb {
                for constraint in &hlpb.aim_constraints {
                    if bone.name == constraint.target_bone_name2 {
                        colors[i] = helper_color;
                    }
                }

                for constraint in &hlpb.orient_constraints {
                    if bone.name == constraint.driver_bone_name {
                        colors[i] = helper_color;
                    }
                }
            }
        }
    }
    colors
}

pub fn create_render_model(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    shared_data: &RenderMeshSharedData,
) -> RenderModel {
    let start = std::time::Instant::now();

    // Attempt to initialize transforms using the skel.
    // This correctly positions mesh objects parented to a bone.
    // Otherwise, don't apply any transformations.
    // TODO: Is it worth matching the in game behavior for a missing skel?
    // "Invisible" models might be more confusing for users to understand.
    let anim_transforms = shared_data
        .skel
        .map(AnimationTransforms::from_skel)
        .unwrap_or_else(AnimationTransforms::identity);

    // Share the transforms buffer to avoid redundant updates.
    // TODO: Enforce bone count being at most 511?
    let skinning_transforms_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Bone Transforms Buffer"),
        contents: bytemuck::cast_slice(&[*anim_transforms.animated_world_transforms]),
        // COPY_DST allows applying animations without allocating new buffers
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let world_transforms_buffer =
        create_world_transforms_buffer(device, &anim_transforms.world_transforms);

    // TODO: Add bone drawing to its own module.
    // TODO: Clean this up.
    let bone_colors_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Bone Colors Buffer"),
        contents: bytemuck::cast_slice(&bone_colors(shared_data.skel, shared_data.hlpb)),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    // TODO: How to avoid applying scale to the bone geometry?
    let bone_data_bind_group = crate::shader::skeleton::bind_groups::BindGroup1::from_bindings(
        device,
        crate::shader::skeleton::bind_groups::BindGroupLayout1 {
            world_transforms: world_transforms_buffer.as_entire_buffer_binding(),
            bone_colors: bone_colors_buffer.as_entire_buffer_binding(),
        },
    );

    let joint_transforms = shared_data
        .skel
        .map(|skel| joint_transforms(skel, &anim_transforms))
        .unwrap_or_else(|| vec![glam::Mat4::IDENTITY; 512]);

    let joint_world_transforms_buffer =
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Joint World Transforms Buffer"),
            contents: bytemuck::cast_slice(&joint_transforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

    let bone_colors_buffer_outer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Bone Colors Buffer"),
        contents: bytemuck::cast_slice(&vec![[0.0f32; 4]; crate::animation::MAX_BONE_COUNT]),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let bone_data_outer_bind_group =
        crate::shader::skeleton::bind_groups::BindGroup1::from_bindings(
            device,
            crate::shader::skeleton::bind_groups::BindGroupLayout1 {
                world_transforms: world_transforms_buffer.as_entire_buffer_binding(),
                bone_colors: bone_colors_buffer_outer.as_entire_buffer_binding(),
            },
        );

    let joint_data_bind_group = crate::shader::skeleton::bind_groups::BindGroup1::from_bindings(
        device,
        crate::shader::skeleton::bind_groups::BindGroupLayout1 {
            world_transforms: joint_world_transforms_buffer.as_entire_buffer_binding(),
            bone_colors: bone_colors_buffer.as_entire_buffer_binding(),
        },
    );

    let joint_data_outer_bind_group =
        crate::shader::skeleton::bind_groups::BindGroup1::from_bindings(
            device,
            crate::shader::skeleton::bind_groups::BindGroupLayout1 {
                world_transforms: joint_world_transforms_buffer.as_entire_buffer_binding(),
                bone_colors: bone_colors_buffer_outer.as_entire_buffer_binding(),
            },
        );

    let mesh_buffers = MeshBuffers {
        skinning_transforms: skinning_transforms_buffer,
        world_transforms: world_transforms_buffer,
    };

    let RenderMeshData {
        meshes,
        material_data_by_label,
        textures,
        pipelines,
        buffer_data,
    } = create_render_meshes(device, queue, &mesh_buffers, shared_data);

    // TODO: Move this to the renderer since it's shared?
    let bone_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Bone Vertex Buffer"),
        contents: bytemuck::cast_slice(&sphere()),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let bone_vertex_buffer_outer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Bone Vertex Buffer Outer"),
        contents: bytemuck::cast_slice(&sphere_outer()),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let bone_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Bone Index Buffer"),
        contents: bytemuck::cast_slice(&sphere_indices()),
        usage: wgpu::BufferUsages::INDEX,
    });

    let joint_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Bone Joint Vertex Buffer"),
        contents: bytemuck::cast_slice(&pyramid()),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let joint_vertex_buffer_outer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Bone Joint Vertex Buffer Outer"),
        contents: bytemuck::cast_slice(&pyramid_outer()),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let joint_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Join Index Buffer"),
        contents: bytemuck::cast_slice(&pyramid_indices()),
        usage: wgpu::BufferUsages::INDEX,
    });

    let mut bone_bind_groups = Vec::new();
    if let Some(skel) = shared_data.skel {
        for (i, bone) in skel.bones.iter().enumerate() {
            // TODO: Use instancing instead.
            let per_bone = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Mesh Object Info Buffer"),
                contents: bytemuck::cast_slice(&[crate::shader::skeleton::PerBone {
                    indices: [
                        i as i32,
                        bone.parent_index.map(|p| p as i32).unwrap_or(-1),
                        -1,
                        -1,
                    ],
                }]),
                usage: wgpu::BufferUsages::UNIFORM,
            });

            let bind_group2 = crate::shader::skeleton::bind_groups::BindGroup2::from_bindings(
                device,
                crate::shader::skeleton::bind_groups::BindGroupLayout2 {
                    per_bone: per_bone.as_entire_buffer_binding(),
                },
            );
            bone_bind_groups.push(bind_group2);
        }
    }

    println!(
        "Create {:?} render meshes, {:?} materials, {:?} pipelines: {:?}",
        meshes.len(),
        material_data_by_label.len(),
        pipelines.len(),
        start.elapsed()
    );

    // TODO: Avoid clone.
    RenderModel {
        meshes,
        skel: shared_data.skel.cloned(),
        matl: shared_data.matl.cloned(),
        hlpb: shared_data.hlpb.cloned(),
        mesh_buffers,
        material_data_by_label,
        textures,
        pipelines,
        bone_vertex_buffer,
        bone_vertex_buffer_outer,
        joint_vertex_buffer,
        joint_vertex_buffer_outer,
        joint_index_buffer,
        joint_world_transforms_buffer,
        bone_index_buffer,
        bone_data_bind_group,
        bone_data_outer_bind_group,
        joint_data_bind_group,
        joint_data_outer_bind_group,
        bone_bind_groups,
        buffer_data,
    }
}

fn joint_transforms(skel: &SkelData, anim_transforms: &AnimationTransforms) -> Vec<glam::Mat4> {
    let mut joint_transforms: Vec<_> = skel
        .bones
        .iter()
        .enumerate()
        .map(|(i, bone)| {
            let pos = anim_transforms.world_transforms[i].col(3).xyz();
            let mut parent_pos = pos;
            if let Some(parent_index) = bone.parent_index {
                parent_pos = anim_transforms.world_transforms[parent_index].col(3).xyz();
            }
            let scale = pos.distance(parent_pos);

            // Assume an inverted pyramid with up as the Y-axis.
            // 1. Scale the pyramid along the Y-axis to have the appropriate length.
            // 2. Rotate the pyramid to point to its parent.
            // 3. Translate the bone to its world position.
            let rotation =
                glam::Quat::from_rotation_arc(glam::Vec3::Y, (parent_pos - pos).normalize());
            glam::Mat4::from_translation(pos)
                * glam::Mat4::from_quat(rotation)
                * glam::Mat4::from_scale(glam::Vec3::new(1.0, scale, 1.0))
        })
        .collect();
    joint_transforms.resize(crate::animation::MAX_BONE_COUNT, glam::Mat4::IDENTITY);
    joint_transforms
}

fn create_material_data(
    device: &wgpu::Device,
    material: Option<&MatlEntryData>,
    textures: &[(String, wgpu::Texture)], // TODO: document that this uses file name?
    default_textures: &[(String, wgpu::Texture)], // TODO: document that this is an absolute path?
    stage_cube: &(wgpu::TextureView, wgpu::Sampler),
) -> MaterialData {
    let uniforms_buffer = create_uniforms_buffer(material, device);
    let material_uniforms_bind_group = create_material_uniforms_bind_group(
        material,
        device,
        textures,
        default_textures,
        stage_cube,
        &uniforms_buffer,
    );

    MaterialData {
        material_uniforms_bind_group,
        uniforms_buffer,
    }
}

struct RenderMeshData {
    meshes: Vec<RenderMesh>,
    material_data_by_label: HashMap<String, MaterialData>,
    textures: Vec<(String, wgpu::Texture)>,
    pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
    buffer_data: MeshObjectBufferData,
}

struct MeshBufferAccess {
    buffer0_start: u64,
    buffer0_size: u64,
    buffer1_start: u64,
    buffer1_size: u64,
    weights_start: u64,
    weights_size: u64,
    indices_start: u64,
    indices_size: u64,
}

fn create_render_meshes(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    mesh_buffers: &MeshBuffers,
    shared_data: &RenderMeshSharedData,
) -> RenderMeshData {
    // TODO: Find a way to organize this.

    // Initialize textures exactly once for performance.
    // Unused textures are rare, so we won't lazy load them.
    let textures: Vec<_> = shared_data
        .nutexbs
        .iter()
        .map(|(name, nutexb)| {
            (
                name.clone(),
                nutexb_wgpu::create_texture(nutexb, device, queue),
            )
        })
        .collect();

    // Mesh objects control the depth state of the pipeline.
    // In practice, each (shader,mesh) pair may need a unique pipeline.
    // Cache materials separately since materials may share a pipeline.
    // TODO: How to test these optimizations?
    let mut pipelines = HashMap::new();

    // Similarly, materials can be shared between mesh objects.
    // TODO: Split into PerMaterial, PerObject, etc in the shaders?
    let material_data_by_label: HashMap<_, _> = shared_data
        .matl
        .map(|matl| {
            matl.entries
                .iter()
                .map(|entry| {
                    let data = create_material_data(
                        device,
                        Some(entry),
                        &textures,
                        shared_data.default_textures,
                        shared_data.stage_cube,
                    );
                    (entry.material_label.clone(), data)
                })
                .collect()
        })
        .unwrap_or_default();

    // TODO: Find a way to have fewer function parameters?
    let mut index_offset = 0;

    let mut model_buffer0_data = Vec::new();
    let mut model_buffer1_data = Vec::new();
    let mut model_skin_weights_data = Vec::new();
    let mut model_vertex_indices = Vec::new();

    let mut accesses = Vec::new();

    let align = |x, n| ((x + n - 1) / n) * n;

    for mesh_object in &shared_data.mesh.unwrap().objects {
        let buffer0_offset = model_buffer0_data.len();
        let buffer1_offset = model_buffer1_data.len();
        let weights_offset = model_skin_weights_data.len();

        // TODO: Offset alignment of 256?
        let buffer0_vertices = buffer0(mesh_object);
        let buffer0_data = bytemuck::cast_slice::<_, u8>(&buffer0_vertices);
        model_buffer0_data.extend_from_slice(buffer0_data);
        model_buffer0_data.resize(align(model_buffer0_data.len(), 256), 0u8);

        let buffer1_vertices = buffer1(mesh_object);
        let buffer1_data = bytemuck::cast_slice::<_, u8>(&buffer1_vertices);
        model_buffer1_data.extend_from_slice(buffer1_data);
        model_buffer1_data.resize(align(model_buffer1_data.len(), 256), 0u8);

        let skin_weights = skin_weights(mesh_object, shared_data.skel);
        let skin_weights_data = bytemuck::cast_slice::<_, u8>(&skin_weights);
        model_skin_weights_data.extend_from_slice(skin_weights_data);
        model_skin_weights_data.resize(align(model_skin_weights_data.len(), 256), 0u8);

        let indices_size = bytemuck::cast_slice::<_, u8>(&mesh_object.vertex_indices).len();
        model_vertex_indices.extend_from_slice(&mesh_object.vertex_indices);

        accesses.push(MeshBufferAccess {
            buffer0_start: buffer0_offset as u64,
            buffer0_size: buffer0_data.len() as u64,
            buffer1_start: buffer1_offset as u64,
            buffer1_size: buffer1_data.len() as u64,
            weights_start: weights_offset as u64,
            weights_size: skin_weights_data.len() as u64,
            indices_start: index_offset as u64,
            indices_size: indices_size as u64,
        });

        index_offset += indices_size;
    }

    let buffer_data = mesh_object_buffers(
        device,
        &model_buffer0_data,
        &model_buffer1_data,
        &model_skin_weights_data,
        &model_vertex_indices,
    );

    let meshes: Vec<_> = shared_data
        .mesh
        .unwrap()
        .objects
        .iter() // TODO: par_iter?
        .zip(accesses.into_iter())
        .enumerate()
        .map(|(i, (mesh_object, access))| {
            // Some mesh objects have associated triangle adjacency.
            let adj_entry = shared_data
                .adj
                .and_then(|adj| adj.entries.iter().find(|e| e.mesh_object_index == i));

            create_render_mesh(
                device,
                mesh_object,
                adj_entry,
                &mut pipelines,
                mesh_buffers,
                access,
                shared_data,
                &buffer_data,
            )
        })
        .collect();

    RenderMeshData {
        meshes,
        material_data_by_label,
        textures,
        pipelines,
        buffer_data,
    }
}

// TODO: Group these parameters?
fn create_render_mesh(
    device: &wgpu::Device,
    mesh_object: &MeshObjectData,
    adj_entry: Option<&AdjEntryData>,
    pipelines: &mut HashMap<PipelineKey, wgpu::RenderPipeline>,
    mesh_buffers: &MeshBuffers,
    access: MeshBufferAccess,
    shared_data: &RenderMeshSharedData,
    buffer_data: &MeshObjectBufferData,
) -> RenderMesh {
    // TODO: These could be cleaner as functions.
    // TODO: Is using a default for the material label ok?
    let material_label = shared_data
        .modl
        .and_then(|m| {
            m.entries
                .iter()
                .find(|e| {
                    e.mesh_object_name == mesh_object.name
                        && e.mesh_object_sub_index == mesh_object.sub_index
                })
                .map(|e| &e.material_label)
        })
        .unwrap_or(&String::new())
        .to_string();

    let material = shared_data.matl.and_then(|matl| {
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
    );

    pipelines
        .entry(pipeline_key)
        .or_insert_with(|| create_pipeline(device, shared_data.pipeline_data, &pipeline_key));

    // TODO: Function for this?
    let adjacency = adj_entry
        .map(|e| e.vertex_adjacency.iter().map(|i| *i as i32).collect())
        .unwrap_or_else(|| vec![-1i32; mesh_object.vertex_count().unwrap() * 18]);
    let adj_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("vertex buffer 0"),
        contents: bytemuck::cast_slice(&adjacency),
        usage: wgpu::BufferUsages::STORAGE,
    });

    // This is applied after skinning, so the source and destination buffer are the same.
    // TODO: Offsets need to be aligned to 256 bytes?
    // TODO: Add padding between mesh objects?
    // TODO: Can this be done in a single dispatch for the entire model?
    // That would avoid any issues with alignment.
    let buffer0_binding = wgpu::BufferBinding {
        buffer: &buffer_data.vertex_buffer0,
        offset: access.buffer0_start,
        size: Some(std::num::NonZeroU64::new(access.buffer0_size).unwrap()),
    };

    let buffer0_source_binding = wgpu::BufferBinding {
        buffer: &buffer_data.vertex_buffer0_source,
        offset: access.buffer0_start,
        size: Some(std::num::NonZeroU64::new(access.buffer0_size).unwrap()),
    };

    let weights_binding = wgpu::BufferBinding {
        buffer: &buffer_data.skinning_buffer,
        offset: access.weights_start,
        size: Some(std::num::NonZeroU64::new(access.weights_size).unwrap()),
    };

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
                transforms: mesh_buffers.skinning_transforms.as_entire_buffer_binding(),
                world_transforms: mesh_buffers.world_transforms.as_entire_buffer_binding(),
            },
        );

    let parent_index = find_parent_index(mesh_object, shared_data.skel);
    let mesh_object_info_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Mesh Object Info Buffer"),
        contents: bytemuck::cast_slice(&[crate::shader::skinning::MeshObjectInfo {
            parent_index: [parent_index, -1, -1, -1],
        }]),
        usage: wgpu::BufferUsages::UNIFORM,
    });

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
    let shader_tag = material
        .and_then(|m| m.shader_label.get(25..))
        .unwrap_or("")
        .to_string();

    let shader_label = material
        .and_then(|m| m.shader_label.get(..24))
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

    RenderMesh {
        name: mesh_object.name.clone(),
        material_label: material_label.clone(),
        shader_label,
        shader_tag,
        is_visible: true,
        sort_bias: mesh_object.sort_bias,
        skinning_bind_group,
        skinning_transforms_bind_group,
        mesh_object_info_bind_group,
        pipeline_key,
        normals_bind_group: renormal_bind_group,
        sub_index: mesh_object.sub_index,
        vertex_count: mesh_object.vertex_count().unwrap(),
        vertex_index_count: mesh_object.vertex_indices.len(),
        access,
        attribute_names,
    }
}

fn create_material_uniforms_bind_group(
    material: Option<&ssbh_data::matl_data::MatlEntryData>,
    device: &wgpu::Device,
    textures: &[(String, wgpu::Texture)],
    default_textures: &[(String, wgpu::Texture)],
    stage_cube: &(wgpu::TextureView, wgpu::Sampler),
    uniforms_buffer: &wgpu::Buffer, // TODO: Just return this?
) -> crate::shader::model::bind_groups::BindGroup1 {
    // TODO: Do all textures default to white if the path isn't correct?
    // TODO: Default cube map?
    let default_white = &default_textures
        .iter()
        .find(|d| d.0 == "/common/shader/sfxpbs/default_white")
        .unwrap()
        .1;

    let load_texture = |texture_id| {
        load_texture(material, texture_id, textures, default_textures)
            .unwrap_or_else(|| default_white.create_view(&TextureViewDescriptor::default()))
    };

    let load_sampler = |sampler_id| {
        load_sampler(material, device, sampler_id)
            .unwrap_or_else(|| device.create_sampler(&SamplerDescriptor::default()))
    };

    // TODO: Better cube map handling.
    // TODO: Default texture for other cube maps?

    // TODO: How to enforce certain textures being cube maps?
    crate::shader::model::bind_groups::BindGroup1::from_bindings(
        device,
        crate::shader::model::bind_groups::BindGroupLayout1 {
            texture0: &load_texture(ParamId::Texture0),
            sampler0: &load_sampler(ParamId::Sampler0),
            texture1: &load_texture(ParamId::Texture1),
            sampler1: &load_sampler(ParamId::Sampler1),
            texture2: &stage_cube.0,
            sampler2: &load_sampler(ParamId::Sampler2),
            texture3: &load_texture(ParamId::Texture3),
            sampler3: &load_sampler(ParamId::Sampler3),
            texture4: &load_texture(ParamId::Texture4),
            sampler4: &load_sampler(ParamId::Sampler4),
            texture5: &load_texture(ParamId::Texture5),
            sampler5: &load_sampler(ParamId::Sampler5),
            texture6: &load_texture(ParamId::Texture6),
            sampler6: &load_sampler(ParamId::Sampler6),
            texture7: &stage_cube.0,
            sampler7: &load_sampler(ParamId::Sampler7),
            texture8: &stage_cube.0,
            sampler8: &load_sampler(ParamId::Sampler8),
            texture9: &load_texture(ParamId::Texture9),
            sampler9: &load_sampler(ParamId::Sampler9),
            texture10: &load_texture(ParamId::Texture10),
            sampler10: &load_sampler(ParamId::Sampler10),
            texture11: &load_texture(ParamId::Texture11),
            sampler11: &load_sampler(ParamId::Sampler11),
            texture12: &load_texture(ParamId::Texture12),
            sampler12: &load_sampler(ParamId::Sampler12),
            texture13: &load_texture(ParamId::Texture13),
            sampler13: &load_sampler(ParamId::Sampler13),
            texture14: &load_texture(ParamId::Texture14),
            sampler14: &load_sampler(ParamId::Sampler14),
            uniforms: uniforms_buffer.as_entire_buffer_binding(),
        },
    )
}

// TODO: Where to put this?
// TODO: Module for skinning buffers?
fn create_world_transforms_buffer(
    device: &wgpu::Device,
    animated_world_transforms: &[glam::Mat4; 512],
) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("World Transforms Buffer"),
        contents: bytemuck::cast_slice(animated_world_transforms),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    })
}

fn find_parent_index(mesh_object: &MeshObjectData, skel: Option<&SkelData>) -> i32 {
    // Only include a parent if there are no bone influences.
    // TODO: What happens if there are influences and a parent bone?
    if mesh_object.bone_influences.is_empty() {
        skel.as_ref()
            .and_then(|skel| {
                skel.bones
                    .iter()
                    .position(|b| b.name == mesh_object.parent_bone_name)
            })
            .map(|i| i as i32)
            .unwrap_or(-1)
    } else {
        -1
    }
}

fn render_pass_index(tag: &str) -> isize {
    match tag {
        "opaque" => 0,
        "far" => 1,
        "sort" => 2,
        "near" => 3,
        _ => 0, // TODO: How to handle invalid tags?
    }
}

pub fn dispatch_renormal<'a>(meshes: &'a [RenderMesh], compute_pass: &mut wgpu::ComputePass<'a>) {
    // Assume the pipeline is already set.
    // Some meshes have a material label tag to enable the recalculating of normals.
    // This helps with animations with large deformations.
    // TODO: Is this check case sensitive?
    for mesh in meshes
        .iter()
        .filter(|m| m.material_label.contains("RENORMAL"))
    {
        crate::shader::renormal::bind_groups::set_bind_groups(
            compute_pass,
            crate::shader::renormal::bind_groups::BindGroups::<'a> {
                bind_group0: &mesh.normals_bind_group,
            },
        );

        // The shader's local workgroup size is (256, 1, 1).
        // Round up to avoid skipping vertices.
        let workgroup_count = (mesh.vertex_count as f64 / 256.0).ceil() as u32;
        compute_pass.dispatch(workgroup_count, 1, 1);
    }
}

pub fn dispatch_skinning<'a>(meshes: &'a [RenderMesh], compute_pass: &mut wgpu::ComputePass<'a>) {
    // Assume the pipeline is already set.
    for mesh in meshes {
        crate::shader::skinning::bind_groups::set_bind_groups(
            compute_pass,
            crate::shader::skinning::bind_groups::BindGroups::<'a> {
                bind_group0: &mesh.skinning_bind_group,
                bind_group1: &mesh.skinning_transforms_bind_group,
                bind_group2: &mesh.mesh_object_info_bind_group,
            },
        );

        // The shader's local workgroup size is (256, 1, 1).
        // Round up to avoid skipping vertices.
        let workgroup_count = (mesh.vertex_count as f64 / 256.0).ceil() as u32;
        compute_pass.dispatch(workgroup_count, 1, 1);
    }
}
