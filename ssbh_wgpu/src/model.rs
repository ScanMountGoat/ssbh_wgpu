use crate::{
    animation::{animate_materials, animate_skel, animate_visibility, AnimationTransforms},
    bone_rendering::*,
    shape::IndexedMeshBuffers,
    swing::SwingPrc,
    swing_rendering::{draw_swing_collisions, SwingRenderData},
    vertex::MeshObjectBufferData,
    viewport::world_to_screen,
    ModelFolder, QueueExt, ShaderDatabase, SharedRenderData,
};
use glam::Vec4Swizzles;
use log::debug;
use mesh_creation::{
    create_material_data, MaterialData, MeshBufferAccess, MeshBuffers, RenderMeshSharedData,
};
use pipeline::{pipeline, PipelineKey};
use ssbh_data::{matl_data::MatlEntryData, meshex_data::EntryFlags, prelude::*};
use std::collections::HashMap;
use wgpu_text::{
    font::FontRef,
    section::{BuiltInLineBreaker, Layout, Section, Text, VerticalAlign},
    TextBrush,
};

mod mesh_creation;
pub mod pipeline;

// Group resources shared between mesh objects.
// Shared resources can be updated once per model instead of per mesh.
// Keep most fields private since the buffer layout is an implementation detail.
// Assume render data is only shared within a folder.
// TODO: Is it worth allowing models to reference textures from other folders?
pub struct RenderModel {
    pub meshes: Vec<RenderMesh>,
    pub is_visible: bool,
    pub is_selected: bool,
    mesh_buffers: MeshBuffers,
    material_data_by_label: HashMap<String, MaterialData>,
    default_material_data: MaterialData,
    pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
    textures: Vec<(String, wgpu::Texture, wgpu::TextureViewDimension)>,

    bone_render_data: BoneRenderData,

    // TODO: The swing pipelines should be created only once in the renderer.
    swing_render_data: SwingRenderData,

    buffer_data: MeshObjectBufferData,

    // Used for text rendering.
    animation_transforms: Box<AnimationTransforms>,
}

// A RenderMesh is view over a portion of the RenderModel data.
// TODO: All the render data should be owned by the RenderModel.
// Each RenderMesh corresponds to the data for a single draw call.
pub struct RenderMesh {
    pub name: String,
    pub is_visible: bool,
    pub is_selected: bool,
    pub subindex: u64,
    meshex_flags: EntryFlags, // TODO: How to update these?
    material_label: String,
    shader_label: String,
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

struct BoneRenderData {
    joint_world_transforms: wgpu::Buffer,
    bone_data: crate::shader::skeleton::bind_groups::BindGroup1,
    joint_data: crate::shader::skeleton::bind_groups::BindGroup1,
    // TODO: Use instancing instead?
    bone_bind_groups: Vec<crate::shader::skeleton::bind_groups::BindGroup2>,
}

impl RenderMesh {
    pub fn render_order(&self) -> isize {
        render_pass_index(self.shader_label.get(25..).unwrap_or("")) + self.sort_bias as isize
    }
}

impl RenderModel {
    pub fn from_folder(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        model: &ModelFolder,
        shared_data: &SharedRenderData,
    ) -> Self {
        // TODO: Should this use the file names in the modl itself?
        // TODO: Avoid creating the render model if there is no mesh?
        let shared_data = RenderMeshSharedData {
            mesh: model.find_mesh(),
            meshex: model.find_meshex(),
            modl: model.find_modl(),
            skel: model.find_skel(),
            matl: model.find_matl(),
            adj: model.find_adj(),
            nutexbs: &model.nutexbs,
            hlpb: model
                .hlpbs
                .iter()
                .find(|(f, _)| f == "model.nuhlpb")
                .and_then(|(_, m)| m.as_ref().ok()),

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

                let data =
                    create_material_data(device, Some(material), &self.textures, shared_data);
                (material.material_label.clone(), data)
            })
            .collect();
    }

    /// Apply skeletal and material animations for this model.
    ///
    /// If `should_loop` is true, `frame` values less than `0.0`
    /// or greater than the max frame count for each animation will wrap around.
    pub fn apply_anim<'a>(
        &mut self,
        queue: &wgpu::Queue,
        anims: impl Iterator<Item = &'a AnimData> + Clone,
        skel: Option<&SkelData>,
        matl: Option<&MatlData>,
        hlpb: Option<&HlpbData>,
        shared_data: &SharedRenderData,
        frame: f32,
        should_loop: bool,
    ) {
        // Update the buffers associated with each skel.
        // This avoids updating per mesh object and allocating new buffers.
        let start = std::time::Instant::now();

        // TODO: Restructure this to iterate the animations only once?
        for anim in anims.clone() {
            // Assume final_frame_index is set to the length of the longest track.
            let current_frame = if should_loop {
                frame.rem_euclid(anim.final_frame_index)
            } else {
                frame
            };
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
                frame,
                should_loop,
            );

            queue.write_data(
                &self.mesh_buffers.skinning_transforms,
                &[self.animation_transforms.animated_world_transforms],
            );

            queue.write_data(
                &self.mesh_buffers.world_transforms,
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
    /// Swing collisions are animated automatically in [RenderModel::apply_anim].
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

    pub fn draw_skeleton<'a>(
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

    pub fn draw_skeleton_silhouette<'a>(
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
            crate::shader::skeleton::bind_groups::set_bind_groups(
                render_pass,
                crate::shader::skeleton::bind_groups::BindGroups::<'a> {
                    bind_group0: camera_bind_group,
                    bind_group1: bone_data_bind_group,
                    bind_group2,
                },
            );
            render_pass.draw_indexed(0..buffers.index_count, 0, 0..1);
        }
    }

    pub fn draw_swing<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        swing_camera_bind_group: &'a crate::shader::swing::bind_groups::BindGroup0,
    ) {
        // TODO: Is it noticeably more efficient to batch shapes together?
        draw_swing_collisions(
            &self.swing_render_data,
            render_pass,
            swing_camera_bind_group,
        );
    }

    fn draw_mesh<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        mesh: &RenderMesh,
        bind_group0: &'a crate::shader::model::bind_groups::BindGroup0,
        bind_group1: &'a crate::shader::model::bind_groups::BindGroup1,
    ) {
        // Prevent potential validation error from empty meshes.
        if mesh.vertex_index_count > 0 {
            crate::shader::model::bind_groups::set_bind_groups(
                render_pass,
                crate::shader::model::bind_groups::BindGroups::<'a> {
                    bind_group0,
                    bind_group1,
                },
            );

            self.set_mesh_buffers(render_pass, mesh);

            render_pass.draw_indexed(0..mesh.vertex_index_count as u32, 0, 0..1);
        }
    }

    pub fn draw_meshes<'a>(
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
                    &material_data.material_uniforms_bind_group,
                );
            }
        }
    }

    pub fn draw_meshes_material_mask<'a>(
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
                &self.default_material_data.material_uniforms_bind_group,
            );
        }
    }

    pub fn draw_meshes_debug<'a>(
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
                &material_data.material_uniforms_bind_group,
            );
        }
    }

    pub fn draw_meshes_silhouettes<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        per_frame_bind_group: &'a crate::shader::model::bind_groups::BindGroup0,
    ) -> bool {
        // Assume the pipeline is already set.
        let mut active = false;
        // TODO: Show meshes that aren't visible?
        for mesh in self
            .meshes
            .iter()
            .filter(|m| m.is_selected || self.is_selected)
        {
            // Use defaults to render outlines for models with missing materials.
            self.draw_mesh(
                render_pass,
                mesh,
                per_frame_bind_group,
                &self.default_material_data.material_uniforms_bind_group,
            );
            active = true;
        }
        active
    }

    pub fn draw_meshes_uv<'a>(
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
                &self.default_material_data.material_uniforms_bind_group,
            );
        }
    }

    pub fn queue_bone_names(
        &self,
        skel: &SkelData,
        brush: &mut TextBrush<FontRef>,
        width: u32,
        height: u32,
        mvp: glam::Mat4,
        font_size: f32,
    ) {
        for (i, bone) in skel.bones.iter().enumerate() {
            let bone_world = *self
                .animation_transforms
                .world_transforms
                .get(i)
                .unwrap_or(&glam::Mat4::IDENTITY);

            let position = bone_world * glam::vec4(0.0, 0.0, 0.0, 1.0);
            let (position_x_screen, position_y_screen) =
                world_to_screen(position.xyz(), mvp, width, height);

            // Add a small offset to the bone position to reduce overlaps.
            let section = Section::default()
                .add_text(
                    (Text::new(&bone.name))
                        // TODO: Use the window's scale factor?
                        .with_scale(font_size)
                        .with_color([1.0, 1.0, 1.0, 1.0]),
                )
                .with_bounds((width as f32, height as f32))
                .with_layout(
                    Layout::default()
                        .v_align(VerticalAlign::Center)
                        .line_breaker(BuiltInLineBreaker::AnyCharLineBreaker),
                )
                .with_screen_position((position_x_screen + 10.0, position_y_screen))
                .to_owned();

            brush.queue(&section);
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

    pub fn draw_meshes_depth<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a crate::shader::model::bind_groups::BindGroup0,
    ) {
        // Assume only one shared bind group for all meshes.
        camera_bind_group.set(render_pass);

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
        crate::shader::skinning::bind_groups::set_bind_groups(
            compute_pass,
            crate::shader::skinning::bind_groups::BindGroups::<'a> {
                bind_group0: &mesh.skinning_bind_group,
                bind_group1: &mesh.skinning_transforms_bind_group,
                bind_group2: &mesh.mesh_object_info_bind_group,
                bind_group3,
            },
        );

        // Round up with ceil to avoid skipping vertices.
        let [workgroup_x, _, _] = crate::shader::skinning::compute::MAIN_WORKGROUP_SIZE;
        let workgroup_count = (mesh.vertex_count as f64 / workgroup_x as f64).ceil() as u32;
        compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
    }
}
