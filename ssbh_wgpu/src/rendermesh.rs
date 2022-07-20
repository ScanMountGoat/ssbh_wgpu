use crate::{
    animation::{animate_materials, animate_skel, animate_visibility, AnimationTransforms},
    bone_rendering::*,
    pipeline::{create_pipeline, PipelineKey},
    texture::{load_default, load_sampler, load_texture, LoadTextureError},
    uniform_buffer, uniform_buffer_readonly,
    uniforms::create_uniforms_buffer,
    vertex::{buffer0, buffer1, mesh_object_buffers, skin_weights, MeshObjectBufferData},
    ModelFiles, ModelFolder, ShaderDatabase, SharedRenderData,
};
use glam::Vec4Swizzles;
use log::{debug, error, info, warn};
use nutexb_wgpu::NutexbFile;
use ssbh_data::{
    adj_data::AdjEntryData,
    matl_data::{MatlEntryData, ParamId},
    mesh_data::MeshObjectData,
    prelude::*,
};
use std::{collections::HashMap, error::Error, num::NonZeroU64};
use wgpu::{util::DeviceExt, SamplerDescriptor};
use wgpu_text::{
    font::FontRef,
    section::{BuiltInLineBreaker, Layout, Section, Text, VerticalAlign},
    TextBrush,
};

// Group resources shared between mesh objects.
// Shared resources can be updated once per model instead of per mesh.
// Keep most fields private since the buffer layout is an implementation detail.
// Assume render data is only shared within a folder.
// TODO: Associate animation folders with model folders?
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

    joint_world_transforms: wgpu::Buffer,
    bone_data: crate::shader::skeleton::bind_groups::BindGroup1,
    bone_data_outer: crate::shader::skeleton::bind_groups::BindGroup1,
    joint_data: crate::shader::skeleton::bind_groups::BindGroup1,
    joint_data_outer: crate::shader::skeleton::bind_groups::BindGroup1,

    // TODO: Use instancing instead.
    bone_bind_groups: Vec<crate::shader::skeleton::bind_groups::BindGroup2>,
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
    material_label: String,
    shader_label: String,
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
        render_pass_index(self.shader_label.get(25..).unwrap_or("")) + self.sort_bias as isize
    }
}

impl RenderModel {
    pub fn from_folder(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        model: &ModelFolder,
        shared_render_data: &SharedRenderData,
    ) -> Self {
        // TODO: Should this use the file names in the modl itself?
        // TODO: Avoid creating the render model if there is no mesh?
        let shared_data = RenderMeshSharedData {
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
                .and_then(|(_, m)| m.as_ref().ok()),
            shared_data: shared_render_data,
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

struct MaterialData {
    material_uniforms_bind_group: crate::shader::model::bind_groups::BindGroup1,
    _uniforms_buffer: wgpu::Buffer,
}

struct MeshBuffers {
    skinning_transforms: wgpu::Buffer,
    world_transforms: wgpu::Buffer,
}

impl RenderModel {
    /// Reassign the mesh materials based on `modl`.
    /// This does not create materials that do not already exist.
    pub fn reassign_materials(&mut self, modl: &ModlData, matl: Option<&MatlData>) {
        for mesh in &mut self.meshes {
            if let Some(entry) = modl.entries.iter().find(|e| {
                e.mesh_object_name == mesh.name && e.mesh_object_sub_index == mesh.sub_index
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

    /// Update the render data associated with `materials`.
    pub fn update_materials(
        &mut self,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        materials: &[MatlEntryData],
        shared_data: &SharedRenderData,
    ) {
        // TODO: Modify the buffer in place if possible to avoid allocations.
        self.material_data_by_label.clear();
        for material in materials {
            self.material_data_by_label
                .entry(material.material_label.clone())
                .or_insert_with(|| {
                    // Create a new pipeline if needed.
                    // Update the pipeline key for associated RenderMeshes.
                    // TODO: Update the pipeline key if the mesh depth settings change.
                    for mesh in self
                        .meshes
                        .iter_mut()
                        .filter(|m| m.material_label == material.material_label)
                    {
                        let pipeline_key = mesh.pipeline_key.with_material(Some(material));
                        self.pipelines.entry(pipeline_key).or_insert_with(|| {
                            create_pipeline(device, &shared_data.pipeline_data, &pipeline_key)
                        });

                        mesh.pipeline_key = pipeline_key;
                    }
                    create_material_data(device, Some(material), &self.textures, shared_data)
                });
        }
        // TODO: Efficiently remove unused entries if the counts have changed?
        // Renaming materials will quickly fill this cache.
    }

    pub fn apply_anim<'a>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        anims: impl Iterator<Item = &'a AnimData> + Clone,
        skel: Option<&SkelData>,
        matl: Option<&MatlData>,
        hlpb: Option<&HlpbData>,
        frame: f32,
        shared_data: &SharedRenderData,
    ) {
        // Update the buffers associated with each skel.
        // This avoids updating per mesh object and allocating new buffers.
        let start = std::time::Instant::now();

        // TODO: Restructure this to iterate the animations only once?
        for anim in anims.clone() {
            animate_visibility(anim, frame, &mut self.meshes);

            if let Some(matl) = matl {
                // Get a list of changed materials.
                // TODO: Is it possible to avoid per frame allocations here?
                let animated_materials = animate_materials(anim, frame, &matl.entries);
                // TODO: Should this go in a separate module?
                // Get updated uniform buffers for animated materials
                self.update_materials(device, queue, &animated_materials, shared_data);
            }
        }

        if let Some(skel) = skel {
            animate_skel(&mut self.animation_transforms, skel, anims, hlpb, frame);

            queue.write_buffer(
                &self.mesh_buffers.skinning_transforms,
                0,
                bytemuck::cast_slice(&[self.animation_transforms.animated_world_transforms]),
            );

            queue.write_buffer(
                &self.mesh_buffers.world_transforms,
                0,
                bytemuck::cast_slice(&self.animation_transforms.world_transforms),
            );

            let joint_transforms = joint_transforms(skel, &self.animation_transforms);
            queue.write_buffer(
                &self.joint_world_transforms,
                0,
                bytemuck::cast_slice(&joint_transforms),
            );
        }

        debug!("Apply Anim: {:?}", start.elapsed());
    }

    pub fn draw_skeleton<'a>(
        &'a self,
        skel: Option<&SkelData>,
        joint_buffers: &'a JointBuffers,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a crate::shader::skeleton::bind_groups::BindGroup0,
        // TODO: Create a struct for these?
        bone_pipeline: &'a wgpu::RenderPipeline,
        bone_outer_pipeline: &'a wgpu::RenderPipeline,
        joint_pipeline: &'a wgpu::RenderPipeline,
        joint_outer_pipeline: &'a wgpu::RenderPipeline,
        axes_pipeline: &'a wgpu::RenderPipeline,
    ) {
        if let Some(skel) = skel {
            self.draw_joints(
                joint_buffers,
                render_pass,
                skel,
                camera_bind_group,
                joint_pipeline,
                joint_outer_pipeline,
            );

            // Draw the bones after to cover up the geometry at the ends of the joints.
            self.draw_bones(
                joint_buffers,
                render_pass,
                skel,
                camera_bind_group,
                bone_pipeline,
                bone_outer_pipeline,
            );

            // TODO: Toggle this in render settings.
            self.draw_bone_axes(
                joint_buffers,
                render_pass,
                skel,
                camera_bind_group,
                axes_pipeline,
            )
        }
    }

    fn draw_joints<'a>(
        &'a self,
        joint_buffers: &'a JointBuffers,
        render_pass: &mut wgpu::RenderPass<'a>,
        skel: &SkelData,
        camera_bind_group: &'a crate::shader::skeleton::bind_groups::BindGroup0,
        skeleton_pipeline: &'a wgpu::RenderPipeline,
        skeleton_outer_pipeline: &'a wgpu::RenderPipeline,
    ) {
        self.draw_skel_inner(
            render_pass,
            skel,
            skeleton_outer_pipeline,
            &joint_buffers.joint_vertex_buffer_outer,
            &joint_buffers.joint_index_buffer,
            camera_bind_group,
            &self.joint_data_outer,
            joint_index_count() as u32,
        );

        self.draw_skel_inner(
            render_pass,
            skel,
            skeleton_pipeline,
            &joint_buffers.joint_vertex_buffer,
            &joint_buffers.joint_index_buffer,
            camera_bind_group,
            &self.joint_data,
            joint_index_count() as u32,
        );
    }

    fn draw_bones<'a>(
        &'a self,
        joint_buffers: &'a JointBuffers,
        render_pass: &mut wgpu::RenderPass<'a>,
        skel: &SkelData,
        camera_bind_group: &'a crate::shader::skeleton::bind_groups::BindGroup0,
        skeleton_pipeline: &'a wgpu::RenderPipeline,
        skeleton_outer_pipeline: &'a wgpu::RenderPipeline,
    ) {
        // TODO: Instancing?
        self.draw_skel_inner(
            render_pass,
            skel,
            skeleton_outer_pipeline,
            &joint_buffers.bone_vertex_buffer_outer,
            &joint_buffers.bone_index_buffer,
            camera_bind_group,
            &self.bone_data_outer,
            bone_index_count() as u32,
        );

        self.draw_skel_inner(
            render_pass,
            skel,
            skeleton_pipeline,
            &joint_buffers.bone_vertex_buffer,
            &joint_buffers.bone_index_buffer,
            camera_bind_group,
            &self.bone_data,
            bone_index_count() as u32,
        );
    }

    fn draw_bone_axes<'a>(
        &'a self,
        joint_buffers: &'a JointBuffers,
        render_pass: &mut wgpu::RenderPass<'a>,
        skel: &SkelData,
        camera_bind_group: &'a crate::shader::skeleton::bind_groups::BindGroup0,
        axes_pipeline: &'a wgpu::RenderPipeline,
    ) {
        // TODO: Instancing?
        self.draw_skel_inner(
            render_pass,
            skel,
            axes_pipeline,
            &joint_buffers.axes_vertex_buffer,
            &joint_buffers.axes_index_buffer,
            camera_bind_group,
            &self.bone_data_outer,
            bone_axes_index_count() as u32,
        );
    }

    fn draw_skel_inner<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        skel: &SkelData,
        pipeline: &'a wgpu::RenderPipeline,
        vertex_buffer: &'a wgpu::Buffer,
        index_buffer: &'a wgpu::Buffer,
        camera_bind_group: &'a crate::shader::skeleton::bind_groups::BindGroup0,
        bone_data_bind_group: &'a crate::shader::skeleton::bind_groups::BindGroup1,
        count: u32,
    ) {
        render_pass.set_pipeline(pipeline);
        render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        for i in 0..skel.bones.len() {
            crate::shader::skeleton::bind_groups::set_bind_groups(
                render_pass,
                crate::shader::skeleton::bind_groups::BindGroups::<'a> {
                    bind_group0: camera_bind_group,
                    bind_group1: bone_data_bind_group,
                    bind_group2: &self.bone_bind_groups[i],
                },
            );
            render_pass.draw_indexed(0..count, 0, 0..1);
        }
    }

    pub fn draw_render_meshes<'a>(
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
        for mesh in self
            .meshes
            .iter()
            .filter(|m| m.is_visible && m.shader_label.ends_with(pass))
        {
            // Meshes with no modl entry or an entry with an invalid material label are skipped entirely in game.
            // If the material entry is deleted from the matl, the mesh is also skipped.
            if let Some(material_data) = self.material_data_by_label.get(&mesh.material_label) {
                // TODO: Does the invalid shader pipeline take priority?
                if let Some(info) = shader_database.get(mesh.shader_label.get(..24).unwrap_or("")) {
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

                crate::shader::model::bind_groups::set_bind_groups(
                    render_pass,
                    crate::shader::model::bind_groups::BindGroups::<'a> {
                        bind_group0: per_frame_bind_group,
                        bind_group1: &material_data.material_uniforms_bind_group,
                    },
                );

                self.set_mesh_buffers(render_pass, mesh);

                // Prevent potential validation error from zero count on Metal.
                if mesh.vertex_index_count > 0 {
                    render_pass.draw_indexed(0..mesh.vertex_index_count as u32, 0, 0..1);
                }
            }
        }
    }

    pub fn draw_render_meshes_debug<'a>(
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

            crate::shader::model::bind_groups::set_bind_groups(
                render_pass,
                crate::shader::model::bind_groups::BindGroups::<'a> {
                    bind_group0: per_frame_bind_group,
                    bind_group1: &material_data.material_uniforms_bind_group,
                },
            );

            self.set_mesh_buffers(render_pass, mesh);

            // Prevent potential validation error from zero count on Metal.
            if mesh.vertex_index_count > 0 {
                render_pass.draw_indexed(0..mesh.vertex_index_count as u32, 0, 0..1);
            }
        }
    }

    pub fn draw_render_meshes_silhouettes<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        per_frame_bind_group: &'a crate::shader::model::bind_groups::BindGroup0,
    ) {
        // Assume the pipeline is already set.
        // TODO: Show meshes that aren't visible?
        for mesh in self
            .meshes
            .iter()
            .filter(|m| m.is_selected || self.is_selected)
        {
            // Render outlines for models with missing materials.
            let material_data = &self.default_material_data;
            crate::shader::model::bind_groups::set_bind_groups(
                render_pass,
                crate::shader::model::bind_groups::BindGroups::<'a> {
                    bind_group0: per_frame_bind_group,
                    bind_group1: &material_data.material_uniforms_bind_group,
                },
            );

            self.set_mesh_buffers(render_pass, mesh);

            // Prevent potential validation error from zero count on Metal.
            if mesh.vertex_index_count > 0 {
                render_pass.draw_indexed(0..mesh.vertex_index_count as u32, 0, 0..1);
            }
        }
    }

    pub fn draw_render_meshes_uv<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        per_frame_bind_group: &'a crate::shader::model::bind_groups::BindGroup0,
    ) {
        // Assume the pipeline is already set.
        for mesh in self.meshes.iter().filter(|m| m.is_selected) {
            // TODO: This should just use a default.
            if let Some(material_data) = self.material_data_by_label.get(&mesh.material_label) {
                crate::shader::model::bind_groups::set_bind_groups(
                    render_pass,
                    crate::shader::model::bind_groups::BindGroups::<'a> {
                        bind_group0: per_frame_bind_group,
                        bind_group1: &material_data.material_uniforms_bind_group,
                    },
                );

                self.set_mesh_buffers(render_pass, mesh);

                // Prevent potential validation error from zero count on Metal.
                if mesh.vertex_index_count > 0 {
                    render_pass.draw_indexed(0..mesh.vertex_index_count as u32, 0, 0..1);
                }
            }
        }
    }

    // TODO: Move this to bone_rendering?
    pub fn queue_bone_names(
        &self,
        skel: Option<&SkelData>,
        brush: &mut TextBrush<FontRef>,
        width: u32,
        height: u32,
        mvp: glam::Mat4,
        font_size: f32,
    ) {
        if let Some(skel) = skel {
            for (i, bone) in skel.bones.iter().enumerate() {
                let bone_world = *self
                    .animation_transforms
                    .world_transforms
                    .get(i)
                    .unwrap_or(&glam::Mat4::IDENTITY);
                let (position_x_screen, position_y_screen) =
                    bone_screen_position(bone_world, mvp, width, height);

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
        camera_bind_group: &'a crate::shader::model::bind_groups::BindGroup0,
    ) {
        // Assume only one shared bind group for all meshes.
        camera_bind_group.set(render_pass);
        for mesh in self.meshes.iter().filter(|m| m.is_visible) {
            self.set_mesh_buffers(render_pass, mesh);

            render_pass.draw_indexed(0..mesh.vertex_index_count as u32, 0, 0..1);
        }
    }
}

fn bone_screen_position(
    bone_world: glam::Mat4,
    mvp: glam::Mat4,
    width: u32,
    height: u32,
) -> (f32, f32) {
    let position = (mvp * bone_world) * glam::Vec4::new(0.0, 0.0, 0.0, 1.0);
    // Account for perspective correction.
    let position_clip = position.xyz() / position.w;
    // Convert from clip space [-1,1] to screen space [0,width] or [0,height].
    // Flip y vertically to match wgpu conventions.
    let position_x_screen = width as f32 * (position_clip.x * 0.5 + 0.5);
    let position_y_screen = height as f32 * (1.0 - (position_clip.y * 0.5 + 0.5));
    (position_x_screen, position_y_screen)
}

// TODO: Come up with a better name.
struct RenderMeshSharedData<'a> {
    shared_data: &'a SharedRenderData,
    mesh: Option<&'a MeshData>,
    modl: Option<&'a ModlData>,
    skel: Option<&'a SkelData>,
    matl: Option<&'a MatlData>,
    adj: Option<&'a AdjData>,
    hlpb: Option<&'a HlpbData>,
    nutexbs: &'a ModelFiles<NutexbFile>,
}

impl<'a> RenderMeshSharedData<'a> {
    fn to_render_model(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> RenderModel {
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
        let skinning_transforms_buffer = uniform_buffer(
            device,
            "Bone Transforms Buffer",
            &[animation_transforms.animated_world_transforms],
        );

        let world_transforms = uniform_buffer(
            device,
            "World Transforms Buffer",
            &animation_transforms.world_transforms,
        );

        // TODO: Clean this up.
        let bone_colors = bone_colors_buffer(device, self.skel, self.hlpb);

        let joint_transforms = self
            .skel
            .map(|skel| joint_transforms(skel, &animation_transforms))
            .unwrap_or_else(|| vec![glam::Mat4::IDENTITY; 512]);

        let joint_world_transforms =
            uniform_buffer(device, "Joint World Transforms Buffer", &joint_transforms);

        let bone_colors_outer = uniform_buffer_readonly(
            device,
            "Bone Colors Buffer",
            &vec![[0.0f32; 4]; crate::animation::MAX_BONE_COUNT],
        );

        // TODO: How to avoid applying scale to the bone geometry?
        let bone_data = bone_bind_group1(device, &world_transforms, &bone_colors);
        let bone_data_outer = bone_bind_group1(device, &world_transforms, &bone_colors_outer);
        let joint_data = bone_bind_group1(device, &joint_world_transforms, &bone_colors);
        let joint_data_outer =
            bone_bind_group1(device, &joint_world_transforms, &bone_colors_outer);

        let mesh_buffers = MeshBuffers {
            skinning_transforms: skinning_transforms_buffer,
            world_transforms,
        };

        let default_material_data = create_material_data(device, None, &[], &self.shared_data);

        let RenderMeshData {
            meshes,
            material_data_by_label,
            textures,
            pipelines,
            buffer_data,
        } = create_render_mesh_data(device, queue, &mesh_buffers, self);

        let bone_bind_groups = self.bone_bind_groups(device);

        info!(
            "Create {:?} render meshe(s), {:?} material(s), {:?} pipeline(s): {:?}",
            meshes.len(),
            material_data_by_label.len(),
            pipelines.len(),
            start.elapsed()
        );

        RenderModel {
            is_visible: true,
            is_selected: false,
            meshes,
            mesh_buffers,
            material_data_by_label,
            default_material_data,
            textures,
            pipelines,
            joint_world_transforms,
            bone_data,
            bone_data_outer,
            joint_data,
            joint_data_outer,
            bone_bind_groups,
            buffer_data,
            animation_transforms: Box::new(animation_transforms),
        }
    }

    fn bone_bind_groups(
        &self,
        device: &wgpu::Device,
    ) -> Vec<crate::shader::skeleton::bind_groups::BindGroup2> {
        self.skel
            .map(|skel| {
                skel.bones
                    .iter()
                    .enumerate()
                    .map(|(i, bone)| {
                        // TODO: Use instancing instead.
                        let per_bone = uniform_buffer_readonly(
                            device,
                            "Mesh Object Info Buffer",
                            &[crate::shader::skeleton::PerBone {
                                indices: [i as i32, parent_index(bone.parent_index), -1, -1],
                            }],
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

fn create_material_data(
    device: &wgpu::Device,
    material: Option<&MatlEntryData>,
    textures: &[(String, wgpu::Texture, wgpu::TextureViewDimension)],
    shared_data: &SharedRenderData,
) -> MaterialData {
    let uniforms_buffer = create_uniforms_buffer(material, device, &shared_data.database);
    let material_uniforms_bind_group = create_material_uniforms_bind_group(
        material,
        device,
        textures,
        &shared_data.default_textures,
        &shared_data.stage_cube,
        &uniforms_buffer,
    );

    MaterialData {
        material_uniforms_bind_group,
        _uniforms_buffer: uniforms_buffer,
    }
}

struct RenderMeshData {
    meshes: Vec<RenderMesh>,
    material_data_by_label: HashMap<String, MaterialData>,
    textures: Vec<(String, wgpu::Texture, wgpu::TextureViewDimension)>,
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

fn create_render_mesh_data(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    mesh_buffers: &MeshBuffers,
    shared_data: &RenderMeshSharedData,
) -> RenderMeshData {
    // TODO: Find a way to organize this.

    // Initialize textures exactly once for performance.
    // Unused textures are rare, so we won't lazy load them.
    let textures = create_textures(shared_data, device, queue);

    // Materials can be shared between mesh objects.
    let material_data_by_label = create_materials(shared_data, device, &textures);

    // TODO: Find a way to have fewer function parameters?
    let mut model_buffer0_data = Vec::new();
    let mut model_buffer1_data = Vec::new();
    let mut model_skin_weights_data = Vec::new();
    let mut model_index_data = Vec::new();

    let mut accesses = Vec::new();

    if let Some(mesh) = shared_data.mesh.as_ref() {
        for mesh_object in &mesh.objects {
            if let Err(e) = append_mesh_object_buffer_data(
                &mut accesses,
                &mut model_buffer0_data,
                &mut model_buffer1_data,
                &mut model_skin_weights_data,
                &mut model_index_data,
                mesh_object,
                shared_data,
            ) {
                error!(
                    "Error accessing vertex data for mesh {}: {}",
                    mesh_object.name, e
                );
            }
        }
    }

    let buffer_data = mesh_object_buffers(
        device,
        &model_buffer0_data,
        &model_buffer1_data,
        &model_skin_weights_data,
        &model_index_data,
    );

    // Mesh objects control the depth state of the pipeline.
    // In practice, each (shader,mesh) pair may need a unique pipeline.
    // Cache materials separately since materials may share a pipeline.
    // TODO: How to test these optimizations?
    let mut pipelines = HashMap::new();

    let meshes = create_render_meshes(
        shared_data,
        accesses,
        device,
        &mut pipelines,
        mesh_buffers,
        &buffer_data,
    )
    .unwrap_or_default();

    RenderMeshData {
        meshes,
        material_data_by_label,
        textures,
        pipelines,
        buffer_data,
    }
}

fn create_render_meshes(
    shared_data: &RenderMeshSharedData,
    accesses: Vec<MeshBufferAccess>,
    device: &wgpu::Device,
    pipelines: &mut HashMap<PipelineKey, wgpu::RenderPipeline>,
    mesh_buffers: &MeshBuffers,
    buffer_data: &MeshObjectBufferData,
) -> Option<Vec<RenderMesh>> {
    Some(
        shared_data
            .mesh?
            .objects
            .iter() // TODO: par_iter?
            .zip(accesses.into_iter())
            .enumerate()
            .filter_map(|(i, (mesh_object, access))| {
                // Some mesh objects have associated triangle adjacency.
                let adj_entry = shared_data
                    .adj
                    .and_then(|adj| adj.entries.iter().find(|e| e.mesh_object_index == i));

                create_render_mesh(
                    device,
                    mesh_object,
                    adj_entry,
                    pipelines,
                    mesh_buffers,
                    access,
                    shared_data,
                    buffer_data,
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
    shared_data: &RenderMeshSharedData,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Vec<(String, wgpu::Texture, wgpu::TextureViewDimension)> {
    let textures: Vec<_> = shared_data
        .nutexbs
        .iter()
        .filter_map(|(name, nutexb)| {
            let nutexb = nutexb
                .as_ref()
                .map_err(|e| {
                    error!("Failed to read nutexb file {}: {}", name, e);
                    e
                })
                .ok()?;
            let (texture, dim) = nutexb_wgpu::create_texture(nutexb, device, queue)
                .map_err(|e| {
                    error!("Failed to create nutexb texture {}: {}", name, e);
                    e
                })
                .ok()?;
            Some((name.clone(), texture, dim))
        })
        .collect();
    textures
}

fn create_materials(
    shared_data: &RenderMeshSharedData,
    device: &wgpu::Device,
    textures: &[(String, wgpu::Texture, wgpu::TextureViewDimension)],
) -> HashMap<String, MaterialData> {
    // TODO: Split into PerMaterial, PerObject, etc in the shaders?
    shared_data
        .matl
        .map(|matl| {
            matl.entries
                .iter()
                .map(|entry| {
                    let data = create_material_data(
                        device,
                        Some(entry),
                        textures,
                        shared_data.shared_data,
                    );
                    (entry.material_label.clone(), data)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn append_mesh_object_buffer_data(
    accesses: &mut Vec<MeshBufferAccess>,
    model_buffer0_data: &mut Vec<u8>,
    model_buffer1_data: &mut Vec<u8>,
    model_skin_weights_data: &mut Vec<u8>,
    model_index_data: &mut Vec<u8>,
    mesh_object: &MeshObjectData,
    shared_data: &RenderMeshSharedData,
) -> Result<(), ssbh_data::mesh_data::error::Error> {
    let buffer0_offset = model_buffer0_data.len();
    let buffer1_offset = model_buffer1_data.len();
    let weights_offset = model_skin_weights_data.len();
    let index_offset = model_index_data.len();

    let buffer0_vertices = buffer0(mesh_object)?;
    let buffer1_vertices = buffer1(mesh_object)?;
    let skin_weights = skin_weights(mesh_object, shared_data.skel)?;

    let buffer0_len = add_vertex_buffer_data(model_buffer0_data, &buffer0_vertices);
    let buffer1_len = add_vertex_buffer_data(model_buffer1_data, &buffer1_vertices);
    let skin_weights_len = add_vertex_buffer_data(model_skin_weights_data, &skin_weights);

    let index_data = bytemuck::cast_slice::<_, u8>(&mesh_object.vertex_indices);
    model_index_data.extend_from_slice(index_data);

    accesses.push(MeshBufferAccess {
        buffer0_start: buffer0_offset as u64,
        buffer0_size: buffer0_len as u64,
        buffer1_start: buffer1_offset as u64,
        buffer1_size: buffer1_len as u64,
        weights_start: weights_offset as u64,
        weights_size: skin_weights_len as u64,
        indices_start: index_offset as u64,
        indices_size: index_data.len() as u64,
    });
    Ok(())
}

fn add_vertex_buffer_data<T: bytemuck::Pod>(model_data: &mut Vec<u8>, vertices: &[T]) -> usize {
    let data = bytemuck::cast_slice::<_, u8>(vertices);
    model_data.extend_from_slice(data);

    // Enforce storage buffer alignment requirements between meshes.
    let n = wgpu::Limits::default().min_storage_buffer_offset_alignment as usize;
    let align = |x| ((x + n - 1) / n) * n;
    model_data.resize(align(model_data.len()), 0u8);

    data.len()
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
) -> Result<RenderMesh, Box<dyn Error>> {
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

    pipelines.entry(pipeline_key).or_insert_with(|| {
        create_pipeline(
            device,
            &shared_data.shared_data.pipeline_data,
            &pipeline_key,
        )
    });

    let vertex_count = mesh_object.vertex_count()?;

    // TODO: Function for this?
    let adjacency = adj_entry
        .map(|e| e.vertex_adjacency.iter().map(|i| *i as i32).collect())
        .unwrap_or_else(|| vec![-1i32; vertex_count * 18]);
    let adj_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("vertex buffer 0"),
        contents: bytemuck::cast_slice(&adjacency),
        usage: wgpu::BufferUsages::STORAGE,
    });

    // This is applied after skinning, so the source and destination buffer are the same.
    // TODO: Can this be done in a single dispatch for the entire model?
    // TODO: Add a proper error for empty meshes.
    // TODO: Investigate why empty meshes crash on emulators.
    let message = "Mesh has no vertices. Failed to create vertex buffers.";
    let buffer0_binding = wgpu::BufferBinding {
        buffer: &buffer_data.vertex_buffer0,
        offset: access.buffer0_start,
        size: Some(NonZeroU64::new(access.buffer0_size).ok_or(message)?),
    };

    let buffer0_source_binding = wgpu::BufferBinding {
        buffer: &buffer_data.vertex_buffer0_source,
        offset: access.buffer0_start,
        size: Some(NonZeroU64::new(access.buffer0_size).ok_or(message)?),
    };

    let weights_binding = wgpu::BufferBinding {
        buffer: &buffer_data.skinning_buffer,
        offset: access.weights_start,
        size: Some(NonZeroU64::new(access.weights_size).ok_or(message)?),
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
    let mesh_object_info_buffer = uniform_buffer_readonly(
        device,
        "Mesh Object Info Buffer",
        &[crate::shader::skinning::MeshObjectInfo {
            parent_index: [parent_index, -1, -1, -1],
        }],
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

    Ok(RenderMesh {
        name: mesh_object.name.clone(),
        material_label: material_label.clone(),
        shader_label,
        is_visible: true,
        is_selected: false,
        sort_bias: mesh_object.sort_bias,
        skinning_bind_group,
        skinning_transforms_bind_group,
        mesh_object_info_bind_group,
        pipeline_key,
        normals_bind_group: renormal_bind_group,
        sub_index: mesh_object.sub_index,
        vertex_count,
        vertex_index_count: mesh_object.vertex_indices.len(),
        access,
        attribute_names,
    })
}

fn create_material_uniforms_bind_group(
    material: Option<&ssbh_data::matl_data::MatlEntryData>,
    device: &wgpu::Device,
    textures: &[(String, wgpu::Texture, wgpu::TextureViewDimension)],
    default_textures: &[(String, wgpu::Texture, wgpu::TextureViewDimension)],
    stage_cube: &(wgpu::Texture, wgpu::Sampler),
    uniforms_buffer: &wgpu::Buffer, // TODO: Just return this?
) -> crate::shader::model::bind_groups::BindGroup1 {
    // TODO: Do all 2D textures default to white if the path isn't correct?
    let default_white = &default_textures
        .iter()
        .find(|d| d.0 == "/common/shader/sfxpbs/default_white")
        .unwrap()
        .1;

    let load_texture = |texture_id, dim| {
        material
            .and_then(|material| {
                // TODO: Add proper path and parameter handling.
                // TODO: Find a way to test texture path loading.
                // This should also handle paths like "../texture.nutexb" and "/render/shader/bin/texture.nutexb".
                material
                    .textures
                    .iter()
                    .find(|t| t.param_id == texture_id)
                    .map(|t| t.data.as_str())
            })
            .and_then(|material_path| {
                // TODO: Pass in replace cube map here?
                load_texture(material_path, textures, default_textures, dim).map_err(|e| {
                    match e {
                        LoadTextureError::PathNotFound => {
                            // TODO: This doesn't work for cube maps?
                            if material_path != "#replace_cubemap" {
                                warn!("Missing texture {:?} assigned to {}. Applying default texture.", material_path, texture_id)
                            }
                        },
                        LoadTextureError::DimensionMismatch { expected, actual } => {
                            warn!("Texture {:?} assigned to {} has invalid dimensions. Expected {:?} but found {:?}.", material_path, texture_id, expected, actual)
                        },
                    }
                }
                ).ok()
            }).unwrap_or_else(|| load_default(texture_id, stage_cube, default_white))
    };

    let load_sampler = |sampler_id| {
        material
            .and_then(|material| load_sampler(material, device, sampler_id))
            .unwrap_or_else(|| device.create_sampler(&SamplerDescriptor::default()))
    };

    // TODO: Better cube map handling.
    // TODO: Default texture for other cube maps?

    // TODO: How to enforce certain textures being cube maps?
    crate::shader::model::bind_groups::BindGroup1::from_bindings(
        device,
        crate::shader::model::bind_groups::BindGroupLayout1 {
            texture0: &load_texture(ParamId::Texture0, wgpu::TextureViewDimension::D2),
            sampler0: &load_sampler(ParamId::Sampler0),
            texture1: &load_texture(ParamId::Texture1, wgpu::TextureViewDimension::D2),
            sampler1: &load_sampler(ParamId::Sampler1),
            texture2: &load_texture(ParamId::Texture2, wgpu::TextureViewDimension::Cube),
            sampler2: &load_sampler(ParamId::Sampler2),
            texture3: &load_texture(ParamId::Texture3, wgpu::TextureViewDimension::D2),
            sampler3: &load_sampler(ParamId::Sampler3),
            texture4: &load_texture(ParamId::Texture4, wgpu::TextureViewDimension::D2),
            sampler4: &load_sampler(ParamId::Sampler4),
            texture5: &load_texture(ParamId::Texture5, wgpu::TextureViewDimension::D2),
            sampler5: &load_sampler(ParamId::Sampler5),
            texture6: &load_texture(ParamId::Texture6, wgpu::TextureViewDimension::D2),
            sampler6: &load_sampler(ParamId::Sampler6),
            texture7: &load_texture(ParamId::Texture7, wgpu::TextureViewDimension::Cube),
            sampler7: &load_sampler(ParamId::Sampler7),
            texture8: &load_texture(ParamId::Texture8, wgpu::TextureViewDimension::Cube),
            sampler8: &load_sampler(ParamId::Sampler8),
            texture9: &load_texture(ParamId::Texture9, wgpu::TextureViewDimension::D2),
            sampler9: &load_sampler(ParamId::Sampler9),
            texture10: &load_texture(ParamId::Texture10, wgpu::TextureViewDimension::D2),
            sampler10: &load_sampler(ParamId::Sampler10),
            texture11: &load_texture(ParamId::Texture11, wgpu::TextureViewDimension::D2),
            sampler11: &load_sampler(ParamId::Sampler11),
            texture12: &load_texture(ParamId::Texture12, wgpu::TextureViewDimension::D2),
            sampler12: &load_sampler(ParamId::Sampler12),
            texture13: &load_texture(ParamId::Texture13, wgpu::TextureViewDimension::D2),
            sampler13: &load_sampler(ParamId::Sampler13),
            texture14: &load_texture(ParamId::Texture14, wgpu::TextureViewDimension::D2),
            sampler14: &load_sampler(ParamId::Sampler14),
            uniforms: uniforms_buffer.as_entire_buffer_binding(),
        },
    )
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
        compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
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
        compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
    }
}
