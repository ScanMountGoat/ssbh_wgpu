use std::collections::HashSet;

use crate::{
    animation::lighting::animate_lighting,
    bone_rendering::{BoneBuffers, BonePipelines},
    floor_grid::FloorGridRenderData,
    model::pipeline::*,
    render_settings::*,
    swing_rendering::swing_pipeline,
    texture::{load_default_lut, uv_pattern, TextureSamplerView},
    CameraTransforms, DeviceBufferExt, QueueExt, RenderModel, ShaderDatabase,
};
use glam::{ivec4, vec2, vec3, vec4, Mat4, UVec4, Vec4};
use nutexb_wgpu::NutexbFile;
use ssbh_data::anim_data::AnimData;
use wgpu::ComputePassDescriptor;

// Used internally for model rendering passes.
// The final render pass uses a user configurable format.
// TODO: We need at least 16 bits to avoid banding from gamma correction.
// TODO: Switch to Rgba16Unorm once validation issues are resolved.
// TODO: Try and get R10G10B10A2 working without banding like in game.
pub const RGBA_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

// TODO: Adjust this to use less precision.
// Rgba16Float is widely supported.
// The in game format uses less precision.
const BLOOM_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

// Alpha to coverage on metal requires a sample count above 1.
// 4 is a widely supported value for MSAA samples.
pub const MSAA_SAMPLE_COUNT: u32 = 4;

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

// Modified from Rg16Unorm for better compatibility.
// TODO: Switch to Rg16Unorm once validation issues are resolved.
const VARIANCE_SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;

const SHADOW_MAP_WIDTH: u32 = 1024;
const SHADOW_MAP_HEIGHT: u32 = 1024;

// Halve the dimensions for additional smoothing.
const VARIANCE_SHADOW_WIDTH: u32 = 512;
const VARIANCE_SHADOW_HEIGHT: u32 = 512;

/// A renderer for drawing a collection of [RenderModel].
///
/// Create a renderer with [SsbhRenderer::new].
/// This is an expensive operation, so applications should create and reuse a single [SsbhRenderer].
///
/// Methods that require a [wgpu::Device] reference are potentially costly and shouldn't be called each frame.
/// Methods that only take a [wgpu::Queue] reference are lightweight and can be called each frame if needed.
pub struct SsbhRenderer {
    bloom_threshold_pipeline: wgpu::RenderPipeline,
    bloom_blur_pipeline: wgpu::RenderPipeline,
    bloom_combine_pipeline: wgpu::RenderPipeline,
    bloom_upscale_pipeline: wgpu::RenderPipeline,
    post_process_pipeline: wgpu::RenderPipeline,

    // TODO: Group model related pipelines?
    skinning_pipeline: wgpu::ComputePipeline,
    renormal_pipeline: wgpu::ComputePipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    variance_shadow_pipeline: wgpu::RenderPipeline,
    invalid_shader_pipeline: wgpu::RenderPipeline,
    invalid_attributes_pipeline: wgpu::RenderPipeline,
    debug_pipeline: wgpu::RenderPipeline,
    silhouette_pipeline: wgpu::RenderPipeline,
    outline_pipeline: wgpu::RenderPipeline,
    uv_pipeline: wgpu::RenderPipeline,
    overlay_pipeline: wgpu::RenderPipeline,
    wireframe_pipeline: wgpu::RenderPipeline,
    selected_material_pipeline: wgpu::RenderPipeline,

    swing_camera_bind_group: crate::shader::swing::bind_groups::BindGroup0,
    swing_pipeline: wgpu::RenderPipeline,

    bone_pipelines: BonePipelines,
    bone_buffers: BoneBuffers,

    floor_grid: FloorGridRenderData,

    // Store camera state for efficiently updating it later.
    // This avoids exposing shader implementations like bind groups.
    camera_buffer: wgpu::Buffer,
    stage_uniforms_buffer: wgpu::Buffer,
    per_frame_bind_group: crate::shader::model::bind_groups::BindGroup0,
    skeleton_camera_bind_group: crate::shader::skeleton::bind_groups::BindGroup0,

    shadow_depth: TextureSamplerView,
    variance_shadow: TextureSamplerView,
    variance_bind_group: crate::shader::variance_shadow::bind_groups::BindGroup0,

    pass_info: PassInfo,

    color_lut: TextureSamplerView,

    clear_color: [f64; 4],

    render_settings: RenderSettings,
    render_settings_buffer: wgpu::Buffer,

    skinning_settings_buffer: wgpu::Buffer,
    skinning_settings_bind_group: crate::shader::skinning::bind_groups::BindGroup3,

    surface_format: wgpu::TextureFormat,

    current_frame_buffer: wgpu::Buffer,

    // TODO: Store this in the model itself and update during animations?
    per_object_buffer: wgpu::Buffer,
}

impl SsbhRenderer {
    /// Initializes the renderer for the given dimensions and monitor scaling.
    ///
    /// The `scale_factor` should typically match the monitor scaling in the OS such as `1.5` for 150% scaling.
    /// If unsure, set `scale_factor` to `1.0`.
    ///
    /// The `clear_color` determines the RGBA color of the viewport background.
    ///
    /// The `surface_format` is used by the final render pass and should match the main window surface.
    /// [wgpu::TextureFormat::Bgra8Unorm] or [wgpu::TextureFormat::Bgra8UnormSrgb] have the best compatibility.
    /// The final render pass will transform output colors accordingly
    /// depending on whether `surface_format` is an sRGB format or not.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        scale_factor: f32,
        clear_color: [f64; 4],
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = crate::shader::post_process::create_shader_module(device);
        let layout = crate::shader::post_process::create_pipeline_layout(device);
        let post_process_pipeline =
            create_screen_pipeline(device, &shader, &layout, "fs_main", RGBA_COLOR_FORMAT);

        let shader = crate::shader::overlay::create_shader_module(device);
        let layout = crate::shader::overlay::create_pipeline_layout(device);
        let overlay_pipeline =
            create_screen_pipeline(device, &shader, &layout, "fs_main", surface_format);

        // Shared shaders for bloom passes.
        // TODO: Should this be all screen texture shaders?
        let shader = crate::shader::bloom::create_shader_module(device);
        let layout = crate::shader::bloom::create_pipeline_layout(device);
        let bloom_threshold_pipeline =
            create_screen_pipeline(device, &shader, &layout, "fs_threshold", BLOOM_COLOR_FORMAT);

        let bloom_blur_pipeline =
            create_screen_pipeline(device, &shader, &layout, "fs_blur", BLOOM_COLOR_FORMAT);

        let bloom_upscale_pipeline =
            create_screen_pipeline(device, &shader, &layout, "fs_upscale", RGBA_COLOR_FORMAT);

        let shader = crate::shader::bloom_combine::create_shader_module(device);
        let layout = crate::shader::bloom_combine::create_pipeline_layout(device);
        let bloom_combine_pipeline =
            create_screen_pipeline(device, &shader, &layout, "fs_main", RGBA_COLOR_FORMAT);

        let skinning_pipeline = crate::shader::skinning::compute::create_main_pipeline(device);
        let renormal_pipeline = crate::shader::renormal::compute::create_main_pipeline(device);

        let shadow_pipeline = depth_pipeline(device);

        let shader = crate::shader::variance_shadow::create_shader_module(device);
        let layout = crate::shader::variance_shadow::create_pipeline_layout(device);
        let variance_shadow_pipeline =
            create_screen_pipeline(device, &shader, &layout, "fs_main", VARIANCE_SHADOW_FORMAT);

        // TODO: Where should stage specific assets be loaded?
        let color_lut = load_default_lut(device, queue);

        // TODO: Create a struct to store the stage rendering data?
        let pass_info = PassInfo::new(
            device,
            width,
            height,
            scale_factor,
            &color_lut,
            surface_format,
        );

        // Assume the user will update the camera, so these values don't matter.
        let camera_buffer = device.create_buffer_from_data(
            "Camera Buffer",
            &[crate::shader::model::CameraTransforms {
                model_view_matrix: Mat4::IDENTITY,
                projection_matrix: Mat4::IDENTITY,
                mvp_matrix: Mat4::IDENTITY,
                mvp_inv_matrix: Mat4::IDENTITY,
                camera_pos: vec4(0.0, 0.0, -1.0, 1.0),
                screen_dimensions: vec4(1.0, 1.0, 1.0, 1.0),
            }],
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );

        // TODO: Don't always assume that the camera bind groups are identical.
        let skeleton_camera_bind_group =
            crate::shader::skeleton::bind_groups::BindGroup0::from_bindings(
                device,
                crate::shader::skeleton::bind_groups::BindGroupLayout0 {
                    camera: camera_buffer.as_entire_buffer_binding(),
                },
            );

        // Depth from the perspective of the light.
        // TODO: Multiple lights require multiple depth maps?
        let shadow_depth = create_depth(device, SHADOW_MAP_WIDTH, SHADOW_MAP_HEIGHT, 1);

        let variance_shadow = create_texture_sampler(
            device,
            VARIANCE_SHADOW_WIDTH,
            VARIANCE_SHADOW_HEIGHT,
            VARIANCE_SHADOW_FORMAT,
            1,
        );

        let render_settings = RenderSettings::default();
        let render_settings_buffer = device.create_buffer_from_data(
            "Render Settings Buffer",
            &[crate::shader::model::RenderSettings::from(&render_settings)],
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );

        // The light nuanmb should be public with conversions for quaternions, vectors, etc being private.
        // stage light nuanmb -> uniform struct -> buffer
        let stage_uniforms_buffer = device.create_buffer_from_data(
            "Stage Uniforms Buffer",
            &[crate::shader::model::StageUniforms::training()],
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );

        let uv_pattern = uv_pattern(device, queue);

        // Share this with UVs and shadow maps to reduce sampler usage.
        // Metal on MacOS expects at most 16 samplers.
        let default_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let current_frame_buffer = device.create_buffer_from_data(
            "Current Frame Buffer",
            &[Vec4::ZERO],
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );

        let per_object_buffer = device.create_buffer_from_data(
            "PerObject Buffer",
            &[per_object(0.0)],
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );

        let for_pass_buffer = device.create_buffer_from_data(
            "ForPass Buffer",
            &[crate::shader::model::ForPass {
                hdr_range: vec4(0.5, 2.0, 0.0, 1.0),
            }],
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );

        let per_frame_buffer = device.create_buffer_from_data(
            "PerFrame Buffer",
            &[per_frame(width, height)],
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );

        let per_view_buffer = device.create_buffer_from_data(
            "PerViewCBuffer Buffer",
            &[per_view(width, height)],
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );

        let per_world_buffer = device.create_buffer_from_data(
            "PerWorldCBuffer Buffer",
            &[per_world()],
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );

        let per_frame_bind_group = crate::shader::model::bind_groups::BindGroup0::from_bindings(
            device,
            crate::shader::model::bind_groups::BindGroupLayout0 {
                camera: camera_buffer.as_entire_buffer_binding(),
                texture_shadow: &variance_shadow.view,
                default_sampler: &default_sampler,
                render_settings: render_settings_buffer.as_entire_buffer_binding(),
                stage_uniforms: stage_uniforms_buffer.as_entire_buffer_binding(),
                uv_pattern: &uv_pattern.create_view(&wgpu::TextureViewDescriptor::default()),
                current_frame: current_frame_buffer.as_entire_buffer_binding(),
                per_object: per_object_buffer.as_entire_buffer_binding(),
                for_pass: for_pass_buffer.as_entire_buffer_binding(),
                per_frame: per_frame_buffer.as_entire_buffer_binding(),
                per_view: per_view_buffer.as_entire_buffer_binding(),
                per_world: per_world_buffer.as_entire_buffer_binding(),
            },
        );

        // TODO: Is it ok to just use the variance shadow map sampler?
        // We don't want a comparison sampler for this pipeline.
        let variance_bind_group =
            crate::shader::variance_shadow::bind_groups::BindGroup0::from_bindings(
                device,
                crate::shader::variance_shadow::bind_groups::BindGroupLayout0 {
                    texture_shadow: &shadow_depth.view,
                    sampler_shadow: &variance_shadow.sampler,
                },
            );

        let invalid_shader_pipeline = invalid_shader_pipeline(device);
        let invalid_attributes_pipeline = invalid_attributes_pipeline(device);
        let debug_pipeline = debug_pipeline(device);
        let silhouette_pipeline = silhouette_pipeline(device, surface_format);
        let outline_pipeline = create_outline_pipeline(device, surface_format);
        let uv_pipeline = uv_pipeline(device, surface_format);
        let wireframe_pipeline = wireframe_pipeline(device);

        let bone_pipelines = BonePipelines::new(device, RGBA_COLOR_FORMAT);
        let bone_buffers = BoneBuffers::new(device);

        let selected_material_pipeline = selected_material_pipeline(device);

        let skinning_settings_buffer = device.create_buffer_from_data(
            "Skinning Settings Buffer",
            &[crate::shader::skinning::SkinningSettings::from(
                &SkinningSettings::default(),
            )],
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let skinning_settings_bind_group =
            crate::shader::skinning::bind_groups::BindGroup3::from_bindings(
                device,
                crate::shader::skinning::bind_groups::BindGroupLayout3 {
                    settings: skinning_settings_buffer.as_entire_buffer_binding(),
                },
            );

        // TODO: Don't always assume that the camera bind groups are identical.
        let swing_camera_bind_group = crate::shader::swing::bind_groups::BindGroup0::from_bindings(
            device,
            crate::shader::swing::bind_groups::BindGroupLayout0 {
                camera: camera_buffer.as_entire_buffer_binding(),
            },
        );

        let floor_grid = FloorGridRenderData::new(device, &camera_buffer, RGBA_COLOR_FORMAT);

        let swing_pipeline = swing_pipeline(device, surface_format);

        Self {
            bloom_threshold_pipeline,
            bloom_blur_pipeline,
            bloom_combine_pipeline,
            bloom_upscale_pipeline,
            post_process_pipeline,
            skinning_pipeline,
            renormal_pipeline,
            shadow_pipeline,
            camera_buffer,
            per_frame_bind_group,
            skeleton_camera_bind_group,
            pass_info,
            color_lut,
            shadow_depth,
            variance_shadow_pipeline,
            variance_shadow,
            variance_bind_group,
            clear_color,
            stage_uniforms_buffer,
            bone_pipelines,
            invalid_shader_pipeline,
            invalid_attributes_pipeline,
            debug_pipeline,
            silhouette_pipeline,
            outline_pipeline,
            uv_pipeline,
            render_settings,
            render_settings_buffer,
            bone_buffers,
            overlay_pipeline,
            wireframe_pipeline,
            selected_material_pipeline,
            skinning_settings_buffer,
            skinning_settings_bind_group,
            swing_camera_bind_group,
            swing_pipeline,
            floor_grid,
            surface_format,
            current_frame_buffer,
            per_object_buffer,
        }
    }

    /// A faster alternative to creating a new [SsbhRenderer] with the desired size.
    ///
    /// Prefer this method over calling [SsbhRenderer::new] with the updated dimensions.
    /// To update the camera to a potentially new aspect ratio,
    /// pass the appropriate matrix to [SsbhRenderer::update_camera].
    ///
    /// The `scale_factor` maps physical pixels to logical pixels.
    /// This adjusts screen based effects such as bloom to have a more appropriate scale on high DPI screens.
    /// This should usually match the current monitor's scaling factor
    /// in the OS such as `1.5` for 150% scaling. If unsure, use a value of `1.0`.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32, scale_factor: f32) {
        self.pass_info = PassInfo::new(
            device,
            width,
            height,
            scale_factor,
            &self.color_lut,
            self.surface_format,
        );
    }

    // TODO: Document that anything that takes a device reference shouldn't be called each frame.
    /// Updates the camera transforms.
    pub fn update_camera(&mut self, queue: &wgpu::Queue, transforms: CameraTransforms) {
        queue.write_data(&self.camera_buffer, &[transforms]);
    }

    /// Updates the current frame for material animations.
    pub fn update_current_frame(&mut self, queue: &wgpu::Queue, current_frame: f32) {
        queue.write_data(
            &self.current_frame_buffer,
            &[vec4(current_frame, 0.0, 0.0, 0.0)],
        );
        queue.write_data(&self.per_object_buffer, &[per_object(current_frame)]);
    }

    /// Updates the render settings.
    pub fn update_render_settings(
        &mut self,
        queue: &wgpu::Queue,
        render_settings: &RenderSettings,
    ) {
        self.render_settings = *render_settings;
        queue.write_data(
            &self.render_settings_buffer,
            &[crate::shader::model::RenderSettings::from(render_settings)],
        );
    }

    /// Updates the skinning settings.
    pub fn update_skinning_settings(
        &mut self,
        queue: &wgpu::Queue,
        skinning_settings: &SkinningSettings,
    ) {
        queue.write_data(
            &self.skinning_settings_buffer,
            &[crate::shader::skinning::SkinningSettings::from(
                skinning_settings,
            )],
        );
    }

    /// Updates the stage lighting data to the given `frame`.
    pub fn update_stage_uniforms(&mut self, queue: &wgpu::Queue, data: &AnimData, frame: f32) {
        let stage_uniforms = animate_lighting(data, frame);
        queue.write_data(&self.stage_uniforms_buffer, &[stage_uniforms]);
    }

    /// Resets the stage uniforms and lighting to their default values.
    pub fn reset_stage_uniforms(&mut self, queue: &wgpu::Queue) {
        queue.write_data(
            &self.stage_uniforms_buffer,
            &[crate::shader::model::StageUniforms::training()],
        );
    }

    /// Updates the stage color grading LUT texture.
    /// Invalid nutexb files are ignored and the texture will not be updated.
    pub fn update_color_lut(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        nutexb: &NutexbFile,
    ) {
        // TODO: Return or log errors?
        if let Ok((texture, dim)) = nutexb_wgpu::create_texture(nutexb, device, queue) {
            if dim == wgpu::TextureViewDimension::D3 {
                let color_lut = TextureSamplerView {
                    view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                        min_filter: wgpu::FilterMode::Linear,
                        mag_filter: wgpu::FilterMode::Linear,
                        ..Default::default()
                    }),
                };
                self.pass_info.post_process_bind_group = create_post_process_bind_group(
                    device,
                    &self.pass_info.color,
                    &self.pass_info.bloom_upscaled,
                    &color_lut,
                );
            }
        }
    }

    /// Resets the color grading LUT texture to its default value.
    pub fn reset_color_lut(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let color_lut = load_default_lut(device, queue);
        self.pass_info.post_process_bind_group = create_post_process_bind_group(
            device,
            &self.pass_info.color,
            &self.pass_info.bloom_upscaled,
            &color_lut,
        );
    }

    /// Sets the viewport background color.
    pub fn set_clear_color(&mut self, color: [f64; 4]) {
        self.clear_color = color;
    }

    // TODO: Add a code example to show how to drop the pass.
    // TODO: Simplify parameters?
    /// Renders the `render_models` to `output_view` using the standard rendering passes for Smash Ultimate.
    ///
    /// The `output_view` should have the format used for [Self::new].
    /// The output is cleared before drawing.
    ///
    /// Returns the final color pass with no depth attachment.
    /// This enables adding efficient overlays.
    /// Remember to drop the pass when done using it!
    pub fn render_models<'a>(
        &'a self,
        encoder: &'a mut wgpu::CommandEncoder,
        output_view: &'a wgpu::TextureView,
        render_models: &'a [RenderModel],
        shader_database: &ShaderDatabase,
        options: &ModelRenderOptions,
    ) -> wgpu::RenderPass<'a> {
        self.begin_render_models(encoder, render_models, shader_database, options);

        let mut pass = create_color_pass(encoder, output_view, Some("Overlay Pass"));
        self.end_render_models(&mut pass);

        pass
    }

    /// Renders the `render_models` to internal textures.
    /// Complete rendering to the final output pass using [Self::end_render_models].
    pub fn begin_render_models<'a>(
        &'a self,
        encoder: &'a mut wgpu::CommandEncoder,
        render_models: &'a [RenderModel],
        shader_database: &ShaderDatabase,
        options: &ModelRenderOptions,
    ) {
        // TODO: How to have RenderModel own all resources but still sort RenderMesh?

        // Transform the vertex positions and normals.
        // Always run compute passes to preserve vertex positions when switching to debug shading.
        self.skinning_pass(encoder, render_models.iter());
        self.renormal_pass(encoder, render_models.iter());

        // TODO: Benchmark and investigate compute shaders for post processing.
        // TODO: Don't make color_final a parameter since we already take self.
        if self.render_settings.debug_mode != DebugMode::Shaded {
            // TODO: Use msaa and resolve to color_final
            self.model_debug_pass(
                encoder,
                render_models,
                options.mask_model_index,
                &options.mask_material_label,
                options.draw_wireframe,
                options.draw_floor_grid,
            );
        } else {
            // Depth only pass for shadow maps.
            self.shadow_pass(encoder, render_models.iter());

            // Create the two channel shadow map for variance shadows.
            self.variance_shadow_pass(encoder);

            // Draw the models to the initial color texture.
            self.model_pass(encoder, render_models, shader_database);

            // TODO: Will these be faster as compute passes?
            // Extract the portions of the image that contribute to bloom.
            self.bloom_threshold_pass(encoder, self.render_settings.render_bloom);

            // Repeatedly downsample and blur the thresholded bloom colors.
            self.bloom_blur_passes(encoder);

            // Combine the bloom textures into a single texture.
            self.bloom_combine_pass(encoder);

            // Upscale with bilinear filtering to smooth the result.
            self.bloom_upscale_pass(encoder);

            self.model_near_pass(
                encoder,
                render_models,
                shader_database,
                options.mask_model_index,
                &options.mask_material_label,
                options.draw_floor_grid,
            );

            // Combine the model and bloom contributions and apply color grading.
            self.post_processing_pass(encoder, &self.pass_info.color_final.view);
        }

        // TODO: Should this also use multisampling?
        // TODO: Disable this pass if not needed.
        self.skeleton_pass(
            encoder,
            render_models.iter(),
            &self.pass_info.color_final.view,
            options.draw_bones,
            options.draw_bone_axes,
        );

        // Check if silhouettes were rendered since the outline pass is slow.
        let mut rendered_silhouette = self.skeleton_silhouette_pass(
            encoder,
            render_models.iter(),
            &self.pass_info.skel_mask.view,
            options.draw_bones,
        );

        // Draw selected meshes to silhouette textures.
        // TODO: This can be combined with the model and model debug pass.
        rendered_silhouette |= self.model_silhouette_pass(encoder, render_models.iter());

        // Expand silhouettes to create outlines.
        // TODO: Will this be faster as a compute shader?
        // TODO: Benchmark this on integrated graphics.
        // TODO: Only run this pass if needed.
        self.outline_pass(
            encoder,
            rendered_silhouette,
            &self.pass_info.silhouette_outlines.view,
            &self.pass_info.outline_bind_group,
        );

        self.outline_pass(
            encoder,
            rendered_silhouette,
            &self.pass_info.skel_outlines.view,
            &self.pass_info.skel_outline_bind_group,
        );
    }

    /// Completes rendering by drawing the models and any overlays to `render_pass`.
    /// The `render_pass` should use the color format used for creation with no depth attachment.
    pub fn end_render_models(&self, render_pass: &mut wgpu::RenderPass<'_>) {
        // TODO: This can be combined with post processing?
        // Composite the outlines onto the result of the debug or shaded passes.
        self.overlay_pass(render_pass);
    }

    /// Renders UVs for all of the meshes with `is_selected` set to `true`.
    pub fn render_models_uv<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        render_models: &'a [RenderModel],
    ) {
        // Take a render pass instead of an encoder to make this easier to integrate.
        render_pass.set_pipeline(&self.uv_pipeline);

        // TODO: Just take an iterator over render meshes instead?
        for model in render_models {
            model.draw_meshes_uv(render_pass, &self.per_frame_bind_group);
        }
    }

    /// Render the collision shapes for `render_model` with hashes not in `hidden_collisions`.
    ///
    /// Collision data should be initialized first using [RenderModel::recreate_swing_collisions].
    /// Pass an empty set to show all collisions.
    pub fn render_swing(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        render_model: &RenderModel,
        hidden_collisions: &HashSet<u64>,
    ) {
        render_model.draw_swing(
            render_pass,
            &self.swing_pipeline,
            &self.swing_camera_bind_group,
            hidden_collisions,
        );
    }

    fn draw_material_mask<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        render_models: impl Iterator<Item = &'a RenderModel>,
        model_index: usize,
        material_label: &str,
    ) {
        // Material labels may be repeated in multiple models.
        // Only show the selected material for the specified model.
        if let Some(model) = render_models.into_iter().nth(model_index) {
            model.draw_meshes_material_mask(
                pass,
                &self.per_frame_bind_group,
                &self.selected_material_pipeline,
                material_label,
            );
        }
    }

    fn bloom_upscale_pass(&self, encoder: &mut wgpu::CommandEncoder) {
        self.bloom_pass(
            encoder,
            "Bloom Upscale Pass",
            &self.bloom_upscale_pipeline,
            &self.pass_info.bloom_upscaled.view,
            &self.pass_info.bloom_upscale_bind_group,
        );
    }

    fn bloom_blur_passes(&self, encoder: &mut wgpu::CommandEncoder) {
        for (texture, bind_group0) in &self.pass_info.bloom_blur_colors {
            self.bloom_pass(
                encoder,
                "Bloom Blur Pass",
                &self.bloom_blur_pipeline,
                &texture.view,
                bind_group0,
            );
        }
    }

    fn bloom_threshold_pass(&self, encoder: &mut wgpu::CommandEncoder, enable_bloom: bool) {
        if enable_bloom {
            self.bloom_pass(
                encoder,
                "Bloom Threshold Pass",
                &self.bloom_threshold_pipeline,
                &self.pass_info.bloom_threshold.view,
                &self.pass_info.bloom_threshold_bind_group,
            );
        } else {
            // TODO: Find a more efficient way to toggle bloom rendering.
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bloom Threshold Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.pass_info.bloom_threshold.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
    }

    fn variance_shadow_pass(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut variance_shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Variance Shadow Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.variance_shadow.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        variance_shadow_pass.set_pipeline(&self.variance_shadow_pipeline);
        crate::shader::variance_shadow::set_bind_groups(
            &mut variance_shadow_pass,
            &self.variance_bind_group,
        );
        variance_shadow_pass.draw(0..3, 0..1);
    }

    fn skinning_pass<'a>(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        render_models: impl Iterator<Item = &'a RenderModel>,
    ) {
        // Skin the render meshes using a compute pass instead of in the vertex shader.
        // Compute shaders give more flexibility compared to vertex shaders.
        // Modifying the vertex buffers once avoids redundant work in later passes.
        let mut skinning_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Skinning Pass"),
            timestamp_writes: None,
        });
        skinning_pass.set_pipeline(&self.skinning_pipeline);

        for model in render_models {
            crate::model::dispatch_skinning(
                &model.meshes,
                &mut skinning_pass,
                &self.skinning_settings_bind_group,
            );
        }
    }

    fn renormal_pass<'a>(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        render_models: impl Iterator<Item = &'a RenderModel>,
    ) {
        // TODO: This doesn't appear to be a compute shader in game?
        // TODO: What is the performance cost of this?
        let mut renormal_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Renormal Pass"),
            timestamp_writes: None,
        });
        renormal_pass.set_pipeline(&self.renormal_pipeline);
        for model in render_models {
            crate::model::dispatch_renormal(&model.meshes, &mut renormal_pass);
        }
    }

    fn model_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        render_models: &[RenderModel],
        shader_database: &ShaderDatabase,
    ) {
        // TODO: Force having a color attachment for each fragment shader output in wgsl_to_wgpu?
        // TODO: Should this pass draw to a floating point target?
        // The in game format isn't 8-bit yet.
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Model Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.pass_info.color_msaa.view,
                resolve_target: Some(&self.pass_info.color.view),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.pass_info.depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        // TODO: Investigate sorting.
        self.draw_render_models(render_models.iter(), &mut pass, shader_database, "opaque");
        self.draw_render_models(render_models.iter(), &mut pass, shader_database, "far");
        self.draw_render_models(render_models.iter(), &mut pass, shader_database, "sort");
    }

    fn model_near_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        render_models: &[RenderModel],
        shader_database: &ShaderDatabase,
        mask_model_index: usize,
        mask_material_label: &str,
        floor_grid: bool,
    ) {
        // TODO: Force having a color attachment for each fragment shader output in wgsl_to_wgpu?
        // TODO: Should this pass draw to a floating point target?
        // The in game format isn't 8-bit yet.
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Model Near Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.pass_info.color_msaa.view,
                resolve_target: Some(&self.pass_info.color.view),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.pass_info.depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        // Models with _near should be drawn after bloom but before post processing?
        // TODO: How does this impact the depth buffer?
        // TODO: Investigate sorting.
        self.draw_render_models(render_models.iter(), &mut pass, shader_database, "near");

        self.draw_material_mask(
            &mut pass,
            render_models.iter(),
            mask_model_index,
            mask_material_label,
        );

        // Draw this last to avoid obscuring models or masks.
        if floor_grid {
            self.floor_grid.draw(&mut pass);
        }
    }

    fn draw_render_models<'a>(
        &'a self,
        render_models: impl Iterator<Item = &'a RenderModel>,
        model_pass: &mut wgpu::RenderPass<'a>,
        shader_database: &ShaderDatabase,
        pass: &str,
    ) {
        for model in render_models.into_iter().filter(|m| m.is_visible) {
            model.draw_meshes(
                model_pass,
                &self.per_frame_bind_group,
                shader_database,
                &self.invalid_shader_pipeline,
                &self.invalid_attributes_pipeline,
                pass,
            );
        }
    }

    fn model_silhouette_pass<'a>(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        render_models: impl Iterator<Item = &'a RenderModel>,
    ) -> bool {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Model Silhouette Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.pass_info.silhouette_mask.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_pipeline(&self.silhouette_pipeline);

        let mut active = false;
        for model in render_models {
            active |= model.draw_meshes_silhouettes(&mut pass, &self.per_frame_bind_group);
        }
        active
    }

    fn model_debug_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        render_models: &[RenderModel],
        mask_model_index: usize,
        mask_material_label: &str,
        wireframe: bool,
        floor_grid: bool,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Model Debug Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.pass_info.color_msaa.view,
                resolve_target: Some(&self.pass_info.color_final.view),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(self.clear_color()),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.pass_info.depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_pipeline(&self.debug_pipeline);
        for model in render_models.iter().filter(|m| m.is_visible) {
            model.draw_meshes_debug(&mut pass, &self.per_frame_bind_group);
        }

        // TODO: Add antialiasing?
        if wireframe {
            pass.set_pipeline(&self.wireframe_pipeline);
            for model in render_models.iter().filter(|m| m.is_visible) {
                model.draw_meshes_debug(&mut pass, &self.per_frame_bind_group);
            }
        }

        self.draw_material_mask(
            &mut pass,
            render_models.iter(),
            mask_model_index,
            mask_material_label,
        );

        // Draw this last to avoid obscuring models or masks.
        if floor_grid {
            self.floor_grid.draw(&mut pass);
        }
    }

    fn clear_color(&self) -> wgpu::Color {
        wgpu::Color {
            r: self.clear_color[0],
            g: self.clear_color[1],
            b: self.clear_color[2],
            a: self.clear_color[3],
        }
    }

    fn skeleton_pass<'a>(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        render_models: impl Iterator<Item = &'a RenderModel>,
        view: &wgpu::TextureView,
        draw_bones: bool,
        draw_bone_axes: bool,
    ) {
        // TODO: Force having a color attachment for each fragment shader output in wgsl_to_wgpu?
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Skeleton Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    // TODO: Combine with another pass to avoid loading.
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.pass_info.skel_depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        if draw_bones {
            for model in render_models {
                model.draw_skeleton(
                    &self.bone_buffers,
                    &mut pass,
                    &self.skeleton_camera_bind_group,
                    &self.bone_pipelines,
                    draw_bone_axes,
                );
            }
        }
    }

    fn skeleton_silhouette_pass<'a>(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        render_models: impl Iterator<Item = &'a RenderModel>,
        view: &wgpu::TextureView,
        draw_bones: bool,
    ) -> bool {
        // TODO: Force having a color attachment for each fragment shader output in wgsl_to_wgpu?
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Skeleton Silhouette Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        if draw_bones {
            for model in render_models {
                model.draw_skeleton_silhouette(
                    &self.bone_buffers,
                    &mut pass,
                    &self.skeleton_camera_bind_group,
                    &self.bone_pipelines,
                );
            }
        }

        draw_bones
    }

    fn outline_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        enabled: bool,
        output: &wgpu::TextureView,
        mask_bind_group: &crate::shader::outline::bind_groups::BindGroup0,
    ) {
        // Always clear the outlines even if nothing is selected.
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Outline Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        if enabled {
            pass.set_pipeline(&self.outline_pipeline);
            crate::shader::outline::set_bind_groups(&mut pass, mask_bind_group);
            pass.draw(0..3, 0..1);
        }
    }

    fn overlay_pass(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_pipeline(&self.overlay_pipeline);
        crate::shader::overlay::set_bind_groups(pass, &self.pass_info.overlay_bind_group);
        pass.draw(0..3, 0..1);
    }

    fn post_processing_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
    ) {
        // Set the clear color here to avoid triggering bloom.
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Post Processing Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(self.clear_color()),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.post_process_pipeline);
        crate::shader::post_process::set_bind_groups(
            &mut pass,
            &self.pass_info.post_process_bind_group,
        );
        pass.draw(0..3, 0..1);
    }

    fn bloom_combine_pass(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut pass = create_color_pass(
            encoder,
            &self.pass_info.bloom_combined.view,
            Some("Bloom Combined Pass"),
        );

        pass.set_pipeline(&self.bloom_combine_pipeline);
        crate::shader::bloom_combine::set_bind_groups(
            &mut pass,
            &self.pass_info.bloom_combine_bind_group,
        );
        pass.draw(0..3, 0..1);
    }

    fn shadow_pass<'a>(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        render_models: impl Iterator<Item = &'a RenderModel>,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Shadow Pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.shadow_depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_pipeline(&self.shadow_pipeline);
        for model in render_models.into_iter().filter(|m| m.is_visible) {
            model.draw_meshes_depth(&mut pass, &self.per_frame_bind_group);
        }
    }

    fn bloom_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        name: &str,
        pipeline: &wgpu::RenderPipeline,
        view: &wgpu::TextureView,
        bind_group: &crate::shader::bloom::bind_groups::BindGroup0,
    ) {
        let mut pass = create_color_pass(encoder, view, Some(name));

        pass.set_pipeline(pipeline);
        crate::shader::bloom::set_bind_groups(&mut pass, bind_group);
        pass.draw(0..3, 0..1);
    }
}

// Data taken from mario c00 face on training stage.
// TODO: How much of this needs to be updated dynamically?
// TODO: updates from lightSet values?
fn per_object(current_frame: f32) -> crate::shader::model::PerObject {
    // Advancing by 1 frame in training mode increases the value by 1.0 / 60.0.
    // TODO: This is only not zero for models with UV scroll animations.
    // TODO: Should this take the playback speed into account?
    let current_time_seconds = current_frame / 60.0;

    crate::shader::model::PerObject {
        light_map_matrix: Mat4::IDENTITY,
        blink_color: vec4(1.0, 1.0, 1.0, 0.0),
        g_constant_volume: vec4(1.0, 1.0, 1.0, 1.0),
        g_constant_offset: vec4(0.0, 0.0, 0.0, 0.0),
        uv_scroll_counter: vec4(current_time_seconds, 0.0, 0.0, 0.0),
        spycloak_params: vec4(0.0, 0.0, 0.0, 0.0),
        compress_param: vec4(1.0, 0.0, 0.0, 1.0),
        g_fresnel_color: vec4(1.5, 1.5, 1.5, 1.0),
        costume_skin_color: vec4(1.0, 0.82745, 0.67843, 0.0),
        outline_color: vec4(0.0, 0.0, 0.0, 0.0),
        light_dir_color1: vec4(4.0, 4.0, 4.0, 1.0),
        light_dir1: vec4(0.38302, -0.86603, -0.32139, 0.0),
        shadow_map_param: vec4(0.001, 0.0, 0.0, 0.0),
        char_shadow_color: vec4(1.0, 1.0, 1.0, 0.0),
        bg_shadow_color: vec4(0.70, 0.70, 0.70, 0.0),
        silhouette_far_color: vec3(0.25, 0.25, 0.25),
        pad: 0.0,
        c_ar: vec4(0.14186, 0.04903, -0.082, 1.11054),
        c_ag: vec4(0.14717, 0.03699, -0.08283, 1.11036),
        c_ab: vec4(0.1419, 0.04334, -0.08283, 1.11018),
        change_metal: vec4(0.0, 0.0, 1.0, 0.0),
        burn_color: vec4(2.0, 0.20, 0.10, 0.0),
        ink_color: vec4(0.0, 0.0, 0.0, 0.0),
        flashing_param: vec4(1.0, 0.0, 0.0, 1.0),
        char_color_grading: vec4(1.0, 1.0, 50.0, 1.0),
    }
}

fn per_world() -> crate::shader::model::PerWorldCBuffer {
    // TODO: Where do these matrices come from?
    crate::shader::model::PerWorldCBuffer {
        world_matrix: Mat4::from_cols_array_2d(&[
            [-0.03869, 0.99923, -0.00633, 0.0],
            [0.78148, 0.0342, 0.623, 0.0],
            [0.62273, 0.01916, -0.7822, 0.0],
            [-29.053, 9.42791, 0.5574, 1.0],
        ]),
        world_inverse_matrix: Mat4::from_cols_array_2d(&[
            [-0.03869, 0.78148, 0.62273, 0.0],
            [0.99923, 0.0342, 0.01916, 0.0],
            [-0.00633, 0.623, -0.7822, 0.0],
            [-10.54127, 22.03453, 18.34759, 1.0],
        ]),
        m_is_shadow_caster: ivec4(0, 0, 0, 0),
    }
}

fn per_view(width: u32, height: u32) -> crate::shader::model::PerViewCBuffer {
    // TODO: are these matrices just the camera matrices?
    let view_matrix = Mat4::from_cols_array_2d(&[
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [30.0, -7.76, -221.88028, 1.0],
    ]);
    crate::shader::model::PerViewCBuffer {
        view_matrix: [view_matrix, Mat4::IDENTITY],
        view_inverse_matrix: view_matrix.inverse(),
        projection_matrix: Mat4::from_cols_array_2d(&[
            [0.05806, 0.0, 0.0, 0.0],
            [0.0, 0.10323, 0.0, 0.0],
            [0.0, 0.0, -0.00045, 0.0],
            [0.0, 0.0, -0.0009, 1.0],
        ]),
        projection_inverse_matrix: Mat4::from_cols_array_2d(&[
            [17.22223, 0.0, 0.0, 0.0],
            [0.0, 9.6875, 0.0, 0.0],
            [0.0, 0.0, -2219.88037, 0.0],
            [0.0, 0.0, -2.0, 1.0],
        ]),
        screen_size: vec2(width as f32, height as f32),
        inverse_screen_size_2d: vec2(1.0 / width as f32, 1.0 / height as f32),
        rt_scale_factor: vec2(1.0, 1.0),
        rt_scale_factor_3d: vec2(1.0, 1.0),
    }
}

fn per_frame(width: u32, height: u32) -> crate::shader::model::PerFrame {
    // TODO: Where does this shadow matrix come from?
    let shadow_map_matrix = Mat4::from_cols_array_2d(&[
        [0.01125, 0.00143, 0.00251, 0.0],
        [0.00229, -0.00703, -0.01237, 0.0],
        [0.0, 0.01249, -0.00725, 0.0],
        [0.64666, 0.55785, 0.60843, 1.0],
    ]);
    // TODO: What is the depth of field texture?
    crate::shader::model::PerFrame {
        depth_of_field0: vec4(0.0, 0.0, 0.0, 0.0),
        depth_of_field1: vec4(0.0, 0.0, 0.0, 0.0),
        depth_of_field_tex_size: vec4(0.00104, 0.00185, 0.0, 0.0), // TODO: 1 / (2*width), 1 / (2 * height)
        sun_shaft_light_param0: vec4(0.0, 0.0, 0.0, 0.0),
        sun_shaft_light_param1: vec4(0.0, 0.0, 0.0, 0.0),
        sun_shaft_blur_param: [vec4(0.0, 0.0, 1.0, 1.0), vec4(0.0, 0.0, 0.0, 0.0)],
        sun_shaft_composite_param: vec4(0.0, 0.0, 0.0, 0.0),
        glare_abstract_param: vec4(0.925, 3.0, 0.0, 0.0),
        glare_blend_ratio: vec4(0.32, 0.10, 0.20, 0.25),
        render_target_tex_size: vec4(
            1.0 / width as f32,
            1.0 / height as f32,
            1.0 / width as f32,
            1.0 / height as f32,
        ),
        light_any_param: vec4(0.0, 0.0, 0.0, 0.0),
        rim_light_dir: vec4(0.0, 0.0, 1.0, 0.99495),
        lens_flare_param: vec4(0.0, 0.0, 0.0, 0.0),
        outline_param: vec4(0.25, 0.40, 0.0, 0.0),
        multi_shadow_matrix: [
            Mat4::from_cols_array_2d(&[
                [0.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 0.0, 0.0],
                [100.0, 100.0, 0.0, 1.00],
            ]),
            Mat4::from_cols_array_2d(&[
                [0.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 0.0, 0.0],
                [100.0, 100.0, 0.0, 1.00],
            ]),
            Mat4::from_cols_array_2d(&[
                [0.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 0.0, 0.0],
                [100.0, 100.0, 0.0, 1.00],
            ]),
            Mat4::from_cols_array_2d(&[
                [0.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 0.0, 0.0],
                [100.0, 100.0, 0.0, 1.00],
            ]),
        ],
        shadow_map_matrix,
        effect_light_param0: vec4(0.10, 0.10, -15.0, 0.0),
        effect_light_param1: vec4(30.0, 12.0, 29.0, 11.0),
        effect_light_param2: vec4(499.50, 360.0, 0.0, 0.0),
        bg_rot_inv: Mat4::IDENTITY,
        g_fog_color: vec4(0.30, 0.45, 1.0, 1.0),
        g_fog_params: vec4(0.001, 0.0, 2.0, 1.0),
        g_fog_height_params: vec4(0.0, 1000.0, 1.0, 0.20),
        g_fog_color_sun_dir: vec4(0.80, 0.40, 0.30, 0.0),
        g_sun_fog_dir: vec4(0.09966, -0.20195, -0.97431, 1.0),
        g_fog_sky_params: vec4(8.0, 0.20, 50000.0, 0.0),
        g_light_map_gain: vec4(1.0, 1.0, 1.0, 1.0),
        g_ibl_color_gain: vec4(1.0, 1.0, 1.0, 1.0),
        g_fog_new_params: vec4(0.0, 100000.0, 1.0, -0.79355),
        g_ibl_scale: vec4(1.0, 1.0, 1.0, 1.0),
        dbg_material_id: vec4(0.0, 1.0, 0.0, 0.0),
        stage_color_grading: vec4(1.0, 1.0, 0.0, -1.0),
        g_light_map_mix_weight: vec4(1.0, 0.0, 0.0, 0.0),
        g_far_color_gain: vec4(1.0, 1.0, 1.0, 1.0),
        g_far_color_offset: vec4(0.0, 0.0, 0.0, 0.0),
        c_ar_reflection: vec4(0.00053, 0.23903, 0.00716, 0.55124),
        c_ag_reflection: vec4(0.00053, 0.2324, 0.00053, 0.55124),
        c_ab_reflection: vec4(0.00053, 0.2324, 0.00053, 0.54551),
    }
}

fn create_screen_pipeline(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    fs_main: &str,
    target: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        // TODO: Labels?
        label: None,
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(fs_main),
            targets: &[Some(wgpu::ColorTargetState {
                format: target,
                // Enable blending to allow transparent screenshots.
                // Use max so an opaque clear color forces opaque output.
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent::OVER,
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Max,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_color_pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    view: &'a wgpu::TextureView,
    label: Option<&'a str>,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

// TODO: Move this to it's own module?
struct PassInfo {
    // TODO: most of these just need a view?
    color: TextureSamplerView,
    color_msaa: TextureSamplerView,
    depth: TextureSamplerView,

    // TODO: Most of these textures can just be cleared and reused.
    skel_depth: TextureSamplerView,
    skel_mask: TextureSamplerView,
    skel_outlines: TextureSamplerView,
    skel_outline_bind_group: crate::shader::outline::bind_groups::BindGroup0,

    // Final color before applying overlays
    color_final: TextureSamplerView,

    bloom_threshold: TextureSamplerView,

    silhouette_mask: TextureSamplerView,
    silhouette_outlines: TextureSamplerView,
    outline_bind_group: crate::shader::outline::bind_groups::BindGroup0,

    bloom_threshold_bind_group: crate::shader::bloom::bind_groups::BindGroup0,

    bloom_blur_colors: [(
        TextureSamplerView,
        crate::shader::bloom::bind_groups::BindGroup0,
    ); 4],

    bloom_combined: TextureSamplerView,
    bloom_combine_bind_group: crate::shader::bloom_combine::bind_groups::BindGroup0,

    bloom_upscaled: TextureSamplerView,
    bloom_upscale_bind_group: crate::shader::bloom::bind_groups::BindGroup0,

    post_process_bind_group: crate::shader::post_process::bind_groups::BindGroup0,
    overlay_bind_group: crate::shader::overlay::bind_groups::BindGroup0,
}

impl PassInfo {
    fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        scale_factor: f32,
        color_lut: &TextureSamplerView,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let depth = create_depth(device, width, height, MSAA_SAMPLE_COUNT);

        // TODO: Use msaa for skeleton drawing?
        let skel_depth = create_depth(device, width, height, 1);
        let skel_mask = create_texture_sampler(device, width, height, RGBA_COLOR_FORMAT, 1);
        let skel_outlines = create_texture_sampler(device, width, height, surface_format, 1);
        let skel_outline_bind_group = create_outline_bind_group(device, &skel_mask);

        let color = create_texture_sampler(device, width, height, RGBA_COLOR_FORMAT, 1);
        let color_msaa =
            create_texture_sampler(device, width, height, RGBA_COLOR_FORMAT, MSAA_SAMPLE_COUNT);
        let color_final = create_texture_sampler(device, width, height, RGBA_COLOR_FORMAT, 1);

        // Bloom uses successively smaller render targets to increase the blur.
        // Account for monitor scaling to avoid a smaller perceived radius on high DPI screens.
        // Some devices like laptops or phones have weak GPUs but high DPI screens.
        // Lowering bloom resolution can reduce performance bottlenecks on these devices.
        let scale_factor = scale_factor.max(1.0);
        let bloom_width = (width as f32 / scale_factor) as u32;
        let bloom_height = (height as f32 / scale_factor) as u32;

        let (bloom_threshold, bloom_threshold_bind_group) = create_bloom_bind_group(
            device,
            bloom_width / 4,
            bloom_height / 4,
            &color,
            BLOOM_COLOR_FORMAT,
        );
        let bloom_blur_colors = create_bloom_blur_bind_groups(
            device,
            bloom_width / 4,
            bloom_height / 4,
            &bloom_threshold,
        );
        let (bloom_combined, bloom_combine_bind_group) = create_bloom_combine_bind_group(
            device,
            bloom_width / 4,
            bloom_height / 4,
            &bloom_blur_colors,
            RGBA_COLOR_FORMAT,
        );
        // A 2x bilinear upscale smooths the overall result.
        let (bloom_upscaled, bloom_upscale_bind_group) = create_bloom_bind_group(
            device,
            bloom_width / 2,
            bloom_height / 2,
            &bloom_combined,
            RGBA_COLOR_FORMAT,
        );

        let post_process_bind_group =
            create_post_process_bind_group(device, &color, &bloom_upscaled, color_lut);

        let silhouette_mask = create_texture_sampler(device, width, height, surface_format, 1);
        let silhouette_outlines = create_texture_sampler(device, width, height, surface_format, 1);
        let outline_bind_group = create_outline_bind_group(device, &silhouette_mask);

        let overlay_bind_group = create_overlay_bind_group(
            device,
            &color_final,
            &silhouette_outlines,
            &skel_outlines,
            surface_format.is_srgb(),
        );

        Self {
            depth,
            skel_depth,
            skel_mask,
            skel_outlines,
            color,
            color_msaa,
            color_final,
            bloom_threshold,
            bloom_threshold_bind_group,
            bloom_blur_colors,
            bloom_combined,
            bloom_combine_bind_group,
            bloom_upscaled,
            bloom_upscale_bind_group,
            post_process_bind_group,
            silhouette_mask,
            silhouette_outlines,
            overlay_bind_group,
            outline_bind_group,
            skel_outline_bind_group,
        }
    }
}

fn create_depth(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    sample_count: u32,
) -> TextureSamplerView {
    let size = wgpu::Extent3d {
        width: width.max(1),
        height: height.max(1),
        depth_or_array_layers: 1,
    };
    let desc = wgpu::TextureDescriptor {
        label: Some("depth texture"),
        size,
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    };
    let texture = device.create_texture(&desc);

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        compare: None,
        ..Default::default()
    });

    TextureSamplerView { view, sampler }
}

fn create_texture_sampler(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    sample_count: u32,
) -> TextureSamplerView {
    // TODO: Labels
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("color texture"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });

    TextureSamplerView { view, sampler }
}

// TODO: Find a way to generate this from render pass descriptions.
fn create_bloom_blur_bind_groups(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    input: &TextureSamplerView,
) -> [(
    TextureSamplerView,
    crate::shader::bloom::bind_groups::BindGroup0,
); 4] {
    // Create successively smaller images to increase the blur strength.
    // For a standard 1920x1080 window, the thresholded input is 480x270.
    // This gives sizes of 240x135 -> 120x67 -> 60x33 -> 30x16
    let create_bind_group = |width, height, input| {
        create_bloom_bind_group(device, width, height, input, BLOOM_COLOR_FORMAT)
    };

    let (texture0, bind_group0) = create_bind_group(width / 2, height / 2, input);
    let (texture1, bind_group1) = create_bind_group(width / 4, height / 4, &texture0);
    let (texture2, bind_group2) = create_bind_group(width / 8, height / 8, &texture1);
    let (texture3, bind_group3) = create_bind_group(width / 16, height / 16, &texture2);

    [
        (texture0, bind_group0),
        (texture1, bind_group1),
        (texture2, bind_group2),
        (texture3, bind_group3),
    ]
}

fn create_bloom_combine_bind_group(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    bloom_inputs: &[(
        TextureSamplerView,
        crate::shader::bloom::bind_groups::BindGroup0,
    ); 4],
    surface_format: wgpu::TextureFormat,
) -> (
    TextureSamplerView,
    crate::shader::bloom_combine::bind_groups::BindGroup0,
) {
    let texture = create_texture_sampler(device, width, height, surface_format, 1);

    let bind_group = crate::shader::bloom_combine::bind_groups::BindGroup0::from_bindings(
        device,
        crate::shader::bloom_combine::bind_groups::BindGroupLayout0 {
            bloom0_texture: &bloom_inputs[0].0.view,
            bloom1_texture: &bloom_inputs[1].0.view,
            bloom2_texture: &bloom_inputs[2].0.view,
            bloom3_texture: &bloom_inputs[3].0.view,
            bloom_sampler: &bloom_inputs[0].0.sampler,
        },
    );

    (texture, bind_group)
}

fn create_bloom_bind_group(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    input: &TextureSamplerView,
    format: wgpu::TextureFormat,
) -> (
    TextureSamplerView,
    crate::shader::bloom::bind_groups::BindGroup0,
) {
    let texture = create_texture_sampler(device, width, height, format, 1);

    let bind_group = crate::shader::bloom::bind_groups::BindGroup0::from_bindings(
        device,
        crate::shader::bloom::bind_groups::BindGroupLayout0 {
            color_texture: &input.view,
            color_sampler: &input.sampler,
        },
    );

    (texture, bind_group)
}

fn create_post_process_bind_group(
    device: &wgpu::Device,
    color_input: &TextureSamplerView,
    bloom_input: &TextureSamplerView,
    color_lut: &TextureSamplerView,
) -> crate::shader::post_process::bind_groups::BindGroup0 {
    crate::shader::post_process::bind_groups::BindGroup0::from_bindings(
        device,
        crate::shader::post_process::bind_groups::BindGroupLayout0 {
            color_texture: &color_input.view,
            color_sampler: &color_input.sampler,
            color_lut: &color_lut.view,
            color_lut_sampler: &color_lut.sampler,
            bloom_texture: &bloom_input.view,
            bloom_sampler: &bloom_input.sampler,
        },
    )
}

fn create_overlay_bind_group(
    device: &wgpu::Device,
    color_final: &TextureSamplerView,
    outline_texture: &TextureSamplerView,
    skel_outline_texture: &TextureSamplerView,
    is_srgb: bool,
) -> crate::shader::overlay::bind_groups::BindGroup0 {
    let buffer = device.create_buffer_from_data(
        "Overlay Settings Buffer",
        &[crate::shader::overlay::OverlaySettings {
            is_srgb: UVec4::splat(is_srgb as u32),
        }],
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    );

    crate::shader::overlay::bind_groups::BindGroup0::from_bindings(
        device,
        crate::shader::overlay::bind_groups::BindGroupLayout0 {
            color_texture: &color_final.view,
            color_sampler: &color_final.sampler,
            outline_texture1: &outline_texture.view,
            outline_texture2: &skel_outline_texture.view,
            outline_sampler: &outline_texture.sampler,
            settings: buffer.as_entire_buffer_binding(),
        },
    )
}

fn create_outline_bind_group(
    device: &wgpu::Device,
    color_final: &TextureSamplerView,
) -> crate::shader::outline::bind_groups::BindGroup0 {
    crate::shader::outline::bind_groups::BindGroup0::from_bindings(
        device,
        crate::shader::outline::bind_groups::BindGroupLayout0 {
            color_texture: &color_final.view,
            color_sampler: &color_final.sampler,
        },
    )
}

fn create_outline_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let module = crate::shader::outline::create_shader_module(device);
    let render_pipeline_layout = crate::shader::outline::create_pipeline_layout(device);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Outline"),
        layout: Some(&render_pipeline_layout),
        vertex: crate::shader::outline::vertex_state(
            &module,
            &crate::shader::outline::vs_main_entry(),
        ),
        fragment: Some(crate::shader::outline::fragment_state(
            &module,
            &crate::shader::outline::fs_main_entry([Some(surface_format.into())]),
        )),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}
