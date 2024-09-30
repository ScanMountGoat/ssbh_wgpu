use crate::{
    animation::{animate_materials, animate_skel, animate_visibility, AnimationTransforms},
    bone_rendering::*,
    shape::IndexedMeshBuffers,
    swing::SwingPrc,
    swing_rendering::{draw_swing_collisions, SwingRenderData},
    vertex::CombinedMeshBuffers,
    ModelFolder, QueueExt, ShaderDatabase, SharedRenderData,
};
use log::{debug, info};
use mesh_creation::{
    material_data, Material, MeshBufferAccess, RenderMeshSharedData, TransformBuffers,
};
use pipeline::{pipeline, PipelineKey};
use ssbh_data::{
    matl_data::{MatlEntryData, SamplerData},
    meshex_data::EntryFlags,
    prelude::*,
};
use std::collections::{HashMap, HashSet};

mod mesh_creation;
pub mod pipeline;

pub type SamplerCache = Vec<(SamplerData, wgpu::Sampler)>;

/// A renderable version of a [ModelFolder].
///
/// This encapsulates data shared between [RenderMesh] like materials, bones, and textures.
/// Grouping shared state reduces redundant state changes for faster creation and updating.
/// Most methods affecting a mesh are only available from the parent [RenderModel] for this reason.
pub struct RenderModel {
    pub meshes: Vec<RenderMesh>,
    /// Render the visible meshes in this model when `true`.
    pub is_visible: bool,
    /// Outline all the meshes in this model when `true` regardless of which meshes are selected.
    pub is_selected: bool,

    transforms: TransformBuffers,
    material_data_by_label: HashMap<String, Material>,
    default_material_data: Material,
    pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
    textures: Vec<(String, wgpu::Texture, wgpu::TextureViewDimension)>,

    per_model_bind_group: crate::shader::model::bind_groups::BindGroup1,

    // Skeleton
    bone_render_data: BoneRenderData,
    animation_transforms: Box<AnimationTransforms>,
    bone_names: Vec<String>,

    swing_render_data: SwingRenderData,

    mesh_buffers: CombinedMeshBuffers,
}

/// A view over the data for a single mesh object in the parent [RenderModel].
///
/// Each RenderMesh corresponds to the data for a single draw call.
// TODO: All the render data should be owned by the RenderModel.
pub struct RenderMesh {
    /// The name of the mesh object.
    pub name: String,
    /// The subindex of the mesh object if names are repeated.
    pub subindex: u64,
    /// Render this mesh when `true`.
    pub is_visible: bool,
    /// Outline this mesh when `true`.
    pub is_selected: bool,
    meshex_flags: EntryFlags, // TODO: How to update these?
    material_label: String,
    shader_label: String,
    renormal_bind_group: crate::shader::renormal::bind_groups::BindGroup0,
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

struct BoneRenderData {
    joint_world_transforms: wgpu::Buffer,
    bone_data: crate::shader::skeleton::bind_groups::BindGroup1,
    joint_data: crate::shader::skeleton::bind_groups::BindGroup1,
    // TODO: Use instancing instead?
    bone_bind_groups: Vec<crate::shader::skeleton::bind_groups::BindGroup2>,
}

impl RenderModel {
    pub fn from_folder(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        model: &ModelFolder,
        shared_data: &SharedRenderData,
    ) -> Self {
        info!("Creating render model.");
        // TODO: Should this use the file names in the modl itself?
        // TODO: Avoid creating the render model if there is no mesh?
        let shared_data = RenderMeshSharedData {
            mesh: model.find_mesh(),
            meshex: model.find_meshex(),
            modl: model.find_modl(),
            skel: model.find_skel(),
            matl: model.find_matl(),
            adj: model.find_adj(),
            hlpb: model.find_hlpb(),
            model_xmb: model.find_model_xmb(),
            nutexbs: &model.nutexbs,
            shared_data,
        };

        shared_data.to_render_model(device, queue)
    }

    /// Finds the texture with the given `file_name`.
    pub fn get_texture(
        &self,
        file_name: &str,
    ) -> Option<(&wgpu::Texture, &wgpu::TextureViewDimension)> {
        self.textures
            .iter()
            .find(|(f, _, _)| f == file_name)
            .map(|(_, t, d)| (t, d))
    }
}

impl RenderModel {
    /// Reassign the mesh materials based on `modl`.
    /// This does not create materials that do not already exist.
    pub fn reassign_materials(&mut self, modl: &ModlData, matl: Option<&MatlData>) {
        for mesh in &mut self.meshes {
            if let Some(entry) = modl.entries.iter().find(|e| {
                e.mesh_object_name == mesh.name && e.mesh_object_subindex == mesh.subindex
            }) {
                mesh.material_label = entry.material_label.clone();
                mesh.shader_label = matl
                    .and_then(|matl| {
                        matl.entries
                            .iter()
                            .find(|e| e.material_label == entry.material_label)
                    })
                    .map(|e| e.shader_label.to_string())
                    .unwrap_or_default();
            } else {
                // TODO: Should this use Option to avoid the case where a material has an emptry string label?
                mesh.material_label = String::new();
                mesh.shader_label = String::new();
            }
        }
    }

    /// Recreates the material render data from `materials`.
    ///
    /// This updates all material data, including texture assignments and pipeline changes like blending modes.
    /// The `material_label` for each [RenderMesh] does not change and should be updated with [RenderModel::reassign_materials].
    /// Avoid calling this every frame since creating new GPU resources is slow.
    pub fn recreate_materials(
        &mut self,
        device: &wgpu::Device,
        materials: &[MatlEntryData],
        shared_data: &SharedRenderData,
    ) {
        let mut sampler_by_data = SamplerCache::new();

        self.material_data_by_label = materials
            .iter()
            .map(|material| {
                // Only create new pipelines as needed since creation is slow.
                // Multiple meshes often share the same pipeline configuration.
                // TODO: Update the pipeline key if the mesh depth settings change.
                for mesh in self
                    .meshes
                    .iter_mut()
                    .filter(|m| m.material_label == material.material_label)
                {
                    let pipeline_key = mesh.pipeline_key.with_material(Some(material));
                    self.pipelines.entry(pipeline_key).or_insert_with(|| {
                        pipeline(device, &shared_data.pipeline_data, &pipeline_key)
                    });

                    // Update the pipeline key for associated RenderMeshes.
                    mesh.pipeline_key = pipeline_key;
                }

                let data = material_data(
                    device,
                    material,
                    &self.textures,
                    shared_data,
                    &mut sampler_by_data,
                );
                (material.material_label.clone(), data)
            })
            .collect();
    }

    /// Apply skeletal and material animations for this model.
    ///
    /// If `should_loop` is true, `frame` values less than `0.0`
    /// or greater than the max frame count for each animation will wrap around.
    pub fn apply_anims<'a>(
        &mut self,
        queue: &wgpu::Queue,
        anims: impl Iterator<Item = &'a AnimData> + Clone,
        skel: Option<&SkelData>,
        matl: Option<&MatlData>,
        hlpb: Option<&HlpbData>,
        shared_data: &SharedRenderData,
        current_frame: f32,
    ) {
        // Update the buffers associated with each skel.
        // This avoids updating per mesh object and allocating new buffers.
        let start = std::time::Instant::now();

        // TODO: Restructure this to iterate the animations only once?
        for anim in anims.clone() {
            // Assume final_frame_index is set to the length of the longest track.
            animate_visibility(anim, current_frame, &mut self.meshes);

            if let Some(matl) = matl {
                self.update_material_uniforms(anim, current_frame, matl, shared_data, queue);
            }
        }

        if let Some(skel) = skel {
            animate_skel(
                &mut self.animation_transforms,
                skel,
                anims,
                hlpb,
                current_frame,
            );

            queue.write_data(
                &self.transforms.skinning_transforms,
                &[self.animation_transforms.animated_world_transforms],
            );

            queue.write_data(
                &self.transforms.world_transforms,
                &self.animation_transforms.world_transforms,
            );

            // TODO: Avoid allocating here?
            let joint_transforms = joint_transforms(skel, &self.animation_transforms);
            queue.write_data(
                &self.bone_render_data.joint_world_transforms,
                &joint_transforms,
            );
        }

        self.swing_render_data.animate_collisions(
            queue,
            skel,
            &self.animation_transforms.world_transforms,
        );

        debug!("Apply Anim: {:?}", start.elapsed());
    }

    /// Creates the data for rendering the collisions in `swing_prc`.
    /// This method should be called once to initialize the swing collisions
    /// and any time collisions in the PRC are added, edited, or removed.
    /// Swing collisions are animated automatically in [RenderModel::apply_anims].
    pub fn recreate_swing_collisions(
        &mut self,
        device: &wgpu::Device,
        swing_prc: &SwingPrc,
        skel: Option<&SkelData>,
    ) {
        self.swing_render_data.update_collisions(
            device,
            swing_prc,
            skel,
            &self.animation_transforms.world_transforms,
        );
    }

    fn update_material_uniforms(
        &mut self,
        anim: &AnimData,
        frame: f32,
        matl: &MatlData,
        shared_data: &SharedRenderData,
        queue: &wgpu::Queue,
    ) {
        // Get a list of changed materials.
        // TODO: Avoid per frame allocations here?
        let animated_materials = animate_materials(anim, frame, &matl.entries);
        for material in animated_materials {
            self.material_data_by_label
                .entry(material.material_label.clone())
                .and_modify(|material_data| {
                    material_data.update(queue, &material, &shared_data.database);
                });
        }
    }

    pub(crate) fn draw_skeleton<'a>(
        &'a self,
        bone_buffers: &'a BoneBuffers,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a crate::shader::skeleton::bind_groups::BindGroup0,
        bone_pipelines: &'a BonePipelines,
        draw_bone_axes: bool,
    ) {
        self.draw_joints(
            bone_buffers,
            render_pass,
            camera_bind_group,
            &bone_pipelines.joint_pipeline,
        );

        // Draw the bones after to cover up the geometry at the ends of the joints.
        self.draw_bones(
            bone_buffers,
            render_pass,
            camera_bind_group,
            &bone_pipelines.bone_pipeline,
        );

        if draw_bone_axes {
            self.draw_bone_axes(
                bone_buffers,
                render_pass,
                camera_bind_group,
                &bone_pipelines.bone_axes_pipeline,
            )
        }
    }

    pub(crate) fn draw_skeleton_silhouette<'a>(
        &'a self,
        bone_buffers: &'a BoneBuffers,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a crate::shader::skeleton::bind_groups::BindGroup0,
        bone_pipelines: &'a BonePipelines,
    ) {
        self.draw_joints(
            bone_buffers,
            render_pass,
            camera_bind_group,
            &bone_pipelines.joint_pipeline,
        );

        // Draw the bones after to cover up the geometry at the ends of the joints.
        self.draw_bones(
            bone_buffers,
            render_pass,
            camera_bind_group,
            &bone_pipelines.bone_pipeline,
        );
    }

    fn draw_joints<'a>(
        &'a self,
        bone_buffers: &'a BoneBuffers,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a crate::shader::skeleton::bind_groups::BindGroup0,
        skeleton_pipeline: &'a wgpu::RenderPipeline,
    ) {
        self.draw_skel_inner(
            render_pass,
            skeleton_pipeline,
            &bone_buffers.joint_buffers,
            camera_bind_group,
            &self.bone_render_data.joint_data,
        );
    }

    fn draw_bones<'a>(
        &'a self,
        bone_buffers: &'a BoneBuffers,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a crate::shader::skeleton::bind_groups::BindGroup0,
        skeleton_pipeline: &'a wgpu::RenderPipeline,
    ) {
        // TODO: Instancing?
        self.draw_skel_inner(
            render_pass,
            skeleton_pipeline,
            &bone_buffers.bone_buffers,
            camera_bind_group,
            &self.bone_render_data.bone_data,
        );
    }

    fn draw_bone_axes<'a>(
        &'a self,
        bone_buffers: &'a BoneBuffers,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a crate::shader::skeleton::bind_groups::BindGroup0,
        axes_pipeline: &'a wgpu::RenderPipeline,
    ) {
        // TODO: Instancing?
        self.draw_skel_inner(
            render_pass,
            axes_pipeline,
            &bone_buffers.axes_buffers,
            camera_bind_group,
            &self.bone_render_data.bone_data,
        );
    }

    fn draw_skel_inner<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        pipeline: &'a wgpu::RenderPipeline,
        buffers: &'a IndexedMeshBuffers,
        camera_bind_group: &'a crate::shader::skeleton::bind_groups::BindGroup0,
        bone_data_bind_group: &'a crate::shader::skeleton::bind_groups::BindGroup1,
    ) {
        render_pass.set_pipeline(pipeline);
        buffers.set(render_pass);

        for bind_group2 in &self.bone_render_data.bone_bind_groups {
            crate::shader::skeleton::set_bind_groups(
                render_pass,
                camera_bind_group,
                bone_data_bind_group,
                bind_group2,
            );
            render_pass.draw_indexed(0..buffers.index_count, 0, 0..1);
        }
    }

    pub(crate) fn draw_swing(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        swing_pipeline: &wgpu::RenderPipeline,
        swing_camera_bind_group: &crate::shader::swing::bind_groups::BindGroup0,
        hidden_collisions: &HashSet<u64>,
    ) {
        // TODO: Is it noticeably more efficient to batch shapes together?
        draw_swing_collisions(
            &self.swing_render_data,
            render_pass,
            swing_pipeline,
            swing_camera_bind_group,
            hidden_collisions,
        );
    }

    fn draw_mesh<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        mesh: &RenderMesh,
        bind_group0: &'a crate::shader::model::bind_groups::BindGroup0,
        bind_group1: &'a crate::shader::model::bind_groups::BindGroup1,
        bind_group2: &'a crate::shader::model::bind_groups::BindGroup2,
    ) {
        // Prevent potential validation error from empty meshes.
        if mesh.vertex_index_count > 0 {
            crate::shader::model::set_bind_groups(
                render_pass,
                bind_group0,
                bind_group1,
                bind_group2,
            );

            self.set_mesh_buffers(render_pass, mesh);

            render_pass.draw_indexed(0..mesh.vertex_index_count as u32, 0, 0..1);
        }
    }

    pub(crate) fn draw_meshes<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        per_frame_bind_group: &'a crate::shader::model::bind_groups::BindGroup0,
        shader_database: &ShaderDatabase,
        invalid_shader_pipeline: &'a wgpu::RenderPipeline,
        invalid_attributes_pipeline: &'a wgpu::RenderPipeline,
        pass: &str,
    ) {
        // TODO: How to store all data in RenderModel but still draw sorted meshes?
        // TODO: Does sort bias only effect meshes within a model or the entire pass?
        // TODO: Test in game and add test cases for sorting.

        // The numshexb can disable rendering of some meshes.
        // This allows invisible meshes to still cast shadows.
        for mesh in self
            .meshes
            .iter()
            .filter(|m| m.is_visible && m.shader_label.ends_with(pass) && m.meshex_flags.draw_model)
        {
            // Meshes with no modl entry or an entry with an invalid material label are skipped entirely in game.
            // If the material entry is deleted from the matl, the mesh is also skipped.
            if let Some(material_data) = self.material_data_by_label.get(&mesh.material_label) {
                // TODO: Does the invalid shader pipeline take priority?
                if let Some(info) = shader_database.get(&mesh.shader_label) {
                    if info.has_required_attributes(&mesh.attribute_names) {
                        // TODO: Don't assume the pipeline exists?
                        render_pass.set_pipeline(&self.pipelines[&mesh.pipeline_key]);
                    } else {
                        render_pass.set_pipeline(invalid_attributes_pipeline);
                    }
                } else {
                    // TODO: Does this include invalid tags?
                    render_pass.set_pipeline(invalid_shader_pipeline);
                }

                self.draw_mesh(
                    render_pass,
                    mesh,
                    per_frame_bind_group,
                    &self.per_model_bind_group,
                    &material_data.material_uniforms_bind_group,
                );
            }
        }
    }

    pub(crate) fn draw_meshes_material_mask<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        per_frame_bind_group: &'a crate::shader::model::bind_groups::BindGroup0,
        selected_pipeline: &'a wgpu::RenderPipeline,
        material_label: &str,
    ) {
        // TODO: Show hidden meshes?
        render_pass.set_pipeline(selected_pipeline);
        for mesh in self
            .meshes
            .iter()
            .filter(|m| m.is_visible && m.material_label == material_label)
        {
            self.draw_mesh(
                render_pass,
                mesh,
                per_frame_bind_group,
                &self.per_model_bind_group,
                &self.default_material_data.material_uniforms_bind_group,
            );
        }
    }

    pub(crate) fn draw_meshes_debug<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        per_frame_bind_group: &'a crate::shader::model::bind_groups::BindGroup0,
    ) {
        // Assume the pipeline is already set.
        for mesh in self.meshes.iter().filter(|m| m.is_visible) {
            // Models should always show up in debug mode.
            let material_data = self
                .material_data_by_label
                .get(&mesh.material_label)
                .unwrap_or(&self.default_material_data);

            self.draw_mesh(
                render_pass,
                mesh,
                per_frame_bind_group,
                &self.per_model_bind_group,
                &material_data.material_uniforms_bind_group,
            );
        }
    }

    pub(crate) fn draw_meshes_silhouettes<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        per_frame_bind_group: &'a crate::shader::model::bind_groups::BindGroup0,
    ) -> bool {
        // Assume the pipeline is already set.
        let mut active = false;
        for mesh in self
            .meshes
            .iter()
            .filter(|m| m.is_selected || self.is_selected)
        {
            // Use defaults to still render outlines for models with missing materials.
            self.draw_mesh(
                render_pass,
                mesh,
                per_frame_bind_group,
                &self.per_model_bind_group,
                &self.default_material_data.material_uniforms_bind_group,
            );
            active = true;
        }
        active
    }

    pub(crate) fn draw_meshes_uv<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        per_frame_bind_group: &'a crate::shader::model::bind_groups::BindGroup0,
    ) {
        // Assume the pipeline is already set.
        for mesh in self.meshes.iter().filter(|m| m.is_selected) {
            self.draw_mesh(
                render_pass,
                mesh,
                per_frame_bind_group,
                &self.per_model_bind_group,
                &self.default_material_data.material_uniforms_bind_group,
            );
        }
    }

    pub(crate) fn bone_names_animated_world_transforms(
        &self,
    ) -> impl Iterator<Item = (&String, glam::Mat4)> {
        self.bone_names.iter().enumerate().map(|(i, name)| {
            let transform = *self
                .animation_transforms
                .world_transforms
                .get(i)
                .unwrap_or(&glam::Mat4::IDENTITY);

            (name, transform)
        })
    }

    fn set_mesh_buffers<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, mesh: &RenderMesh) {
        render_pass.set_vertex_buffer(
            0,
            mesh.access.buffer0.slice(&self.mesh_buffers.vertex_buffer0),
        );
        render_pass.set_vertex_buffer(
            1,
            mesh.access.buffer1.slice(&self.mesh_buffers.vertex_buffer1),
        );
        render_pass.set_index_buffer(
            mesh.access.indices.slice(&self.mesh_buffers.index_buffer),
            wgpu::IndexFormat::Uint32,
        );
    }

    pub(crate) fn draw_meshes_depth<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        per_frame_bind_group: &'a crate::shader::model::bind_groups::BindGroup0,
    ) {
        // Assume only shared bind groups for all meshes.
        per_frame_bind_group.set(render_pass);
        self.per_model_bind_group.set(render_pass);

        // The numshexb can disable shadows for transparent models or special effects.
        for mesh in self
            .meshes
            .iter()
            .filter(|m| m.is_visible && m.meshex_flags.cast_shadow)
        {
            // Prevent potential validation error from empty meshes.
            if mesh.vertex_index_count > 0 {
                self.set_mesh_buffers(render_pass, mesh);

                render_pass.draw_indexed(0..mesh.vertex_index_count as u32, 0, 0..1);
            }
        }
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
        crate::shader::renormal::set_bind_groups(compute_pass, &mesh.renormal_bind_group);

        // Round up with ceil to avoid skipping vertices.
        let [workgroup_x, _, _] = crate::shader::renormal::compute::MAIN_WORKGROUP_SIZE;
        let workgroup_count = (mesh.vertex_count as f64 / workgroup_x as f64).ceil() as u32;
        compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
    }
}

pub fn dispatch_skinning<'a>(
    meshes: &'a [RenderMesh],
    compute_pass: &mut wgpu::ComputePass<'a>,
    bind_group3: &'a crate::shader::skinning::bind_groups::BindGroup3,
) {
    // Assume the pipeline is already set.
    for mesh in meshes {
        crate::shader::skinning::set_bind_groups(
            compute_pass,
            &mesh.skinning_bind_group,
            &mesh.skinning_transforms_bind_group,
            &mesh.mesh_object_info_bind_group,
            bind_group3,
        );

        // Round up with ceil to avoid skipping vertices.
        let [workgroup_x, _, _] = crate::shader::skinning::compute::MAIN_WORKGROUP_SIZE;
        let workgroup_count = (mesh.vertex_count as f64 / workgroup_x as f64).ceil() as u32;
        compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
    }
}
