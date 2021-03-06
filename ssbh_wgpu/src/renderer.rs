use crate::{
    bone_rendering::{BoneBuffers, BonePipelines},
    lighting::{anim_to_lights, calculate_light_transform},
    pipeline::{
        create_debug_pipeline, create_depth_pipeline, create_invalid_attributes_pipeline,
        create_invalid_shader_pipeline, create_selected_material_pipeline,
        create_silhouette_pipeline, create_uv_pipeline, create_wireframe_pipeline,
    },
    texture::{load_default_lut, uv_pattern},
    uniform_buffer, CameraTransforms, RenderModel, ShaderDatabase,
};
use glyph_brush::DefaultSectionHasher;
use ssbh_data::{anim_data::AnimData, skel_data::SkelData};
use strum::{Display, EnumString, EnumVariantNames};
use wgpu::{ComputePassDescriptor, ComputePipelineDescriptor};
use wgpu_text::{font::FontRef, BrushBuilder, TextBrush};

// Rgba16Float is widely supported.
// The in game format uses less precision.
const BLOOM_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

// Bgra8Unorm and Bgra8UnormSrgb should always be supported.
// We'll use SRGB since it's more compatible with less color format aware applications.
// This simplifies integrating with GUIs and image formats like PNG.
pub const RGBA_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
pub const DEPTH_STENCIL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;

// TODO: The in game format is R16G16_UNORM
// TODO: Find a way to get this working without filtering samplers?
const VARIANCE_SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg32Float;

const SHADOW_MAP_WIDTH: u32 = 1024;
const SHADOW_MAP_HEIGHT: u32 = 1024;

// Halve the dimensions for additional smoothing.
const VARIANCE_SHADOW_WIDTH: u32 = 512;
const VARIANCE_SHADOW_HEIGHT: u32 = 512;

// TODO: Module level documention for how to use this?
// TODO: Separate modes for selecting parameters by index (ex: Booleans[3])?
#[derive(PartialEq, Eq, Copy, Clone, Display, EnumVariantNames, EnumString)]
pub enum DebugMode {
    /// The default shaded mode supporting lighting and post processing.
    Shaded,
    /// The Position0 vertex attribute.
    Position0,
    /// The Normal0 vertex attribute.
    Normal0,
    /// The Tangent0 vertex attribute.
    Tangent0,
    ColorSet1,
    ColorSet2,
    ColorSet3,
    ColorSet4,
    ColorSet5,
    ColorSet6,
    ColorSet7,
    Texture0,
    Texture1,
    Texture2,
    Texture3,
    Texture4,
    Texture5,
    Texture6,
    Texture7,
    Texture8,
    Texture9,
    Texture10,
    Texture11,
    Texture12,
    Texture13,
    Texture14,
    Texture16,
    /// The map1 vertex attribute.
    Map1,
    /// The bake1 vertex attribute.
    Bake1,
    /// The uvSet vertex attribute.
    UvSet,
    /// The uvSet1 vertex attribute.
    UvSet1,
    /// The uvSet2 vertex attribute.
    UvSet2,
    /// Lambertian diffuse shading with normal mapping.
    Basic,
    /// Vertex normals with normal mapping.
    Normals,
    /// Calculated bitangent vectors for Smash Ultimate.
    Bitangents,
    /// The final albedo or base color after applying textures and materials.
    Albedo,
}

/// The secondary material for material transitions when using [DebugMode::Shaded].
#[derive(PartialEq, Eq, Copy, Clone, Display, EnumVariantNames, EnumString)]
pub enum TransitionMaterial {
    /// The colored material of Inkling's ink.
    Ink,
    /// The metallic material of the metal box item.
    MetalBox,
    /// The gold material of the Xerneas Pokemon summon.
    Gold,
    /// The purple material of the Ditto Pokemon summon.
    Ditto,
}

/// Settings for configuring the rendered output of an [SsbhRenderer].
#[derive(PartialEq, Clone, Copy)]
pub struct RenderSettings {
    /// The attribute to render as the output color when [Some].
    pub debug_mode: DebugMode,
    pub transition_material: TransitionMaterial,
    /// The amount to blend between the regular material and the [transition_material](#structfield.transition_material).
    /// 0.0 = regular material, 1.0 = transition material.
    pub transition_factor: f32,
    pub render_diffuse: bool,
    pub render_specular: bool,
    pub render_emission: bool,
    pub render_rim_lighting: bool,
    pub render_shadows: bool,
    pub render_bloom: bool,
    pub render_vertex_color: bool,
    pub render_rgba: [bool; 4],
    /// Replaces the RGBA channels of the nor map (Texture4) with a default when false.
    pub render_nor: [bool; 4],
    /// Replaces the RGBA channels of the prm map (Texture6) with a default when false.
    pub render_prm: [bool; 4],
    /// Use a UV test pattern for UV debug modes when `true`. Otherwise, display UVs as RGB colors.
    pub use_uv_pattern: bool,
    /// Draw a wireframe on shaded when `true` for all modes except [DebugMode::Shaded].
    pub wireframe: bool,
}

impl From<&RenderSettings> for crate::shader::model::RenderSettings {
    fn from(r: &RenderSettings) -> Self {
        Self {
            debug_mode: [r.debug_mode as u32; 4],
            transition_material: [r.transition_material as u32; 4],
            transition_factor: [r.transition_factor, 0.0, 0.0, 0.0],
            render_diffuse: [if r.render_diffuse { 1 } else { 0 }; 4],
            render_specular: [if r.render_specular { 1 } else { 0 }; 4],
            render_emission: [if r.render_emission { 1 } else { 0 }; 4],
            render_rim_lighting: [if r.render_rim_lighting { 1 } else { 0 }; 4],
            render_shadows: [if r.render_shadows { 1 } else { 0 }; 4],
            render_bloom: [if r.render_bloom { 1 } else { 0 }; 4],
            render_vertex_color: [if r.render_vertex_color { 1 } else { 0 }; 4],
            render_rgba: r.render_rgba.map(|b| if b { 1.0 } else { 0.0 }),
            render_nor: r.render_nor.map(|b| if b { 1 } else { 0 }),
            render_prm: r.render_prm.map(|b| if b { 1 } else { 0 }),
            render_uv_pattern: [if r.use_uv_pattern { 1 } else { 0 }; 4],
        }
    }
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            debug_mode: DebugMode::Shaded,
            transition_material: TransitionMaterial::Ink,
            transition_factor: 0.0,
            render_diffuse: true,
            render_specular: true,
            render_emission: true,
            render_rim_lighting: true,
            render_shadows: true,
            render_bloom: true,
            render_vertex_color: true,
            render_rgba: [true; 4],
            render_nor: [true; 4],
            render_prm: [true; 4],
            use_uv_pattern: true,
            wireframe: false,
        }
    }
}

/// A renderer for drawing a collection of [RenderModel].
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

    bone_pipelines: BonePipelines,
    bone_buffers: BoneBuffers,

    // Store camera state for efficiently updating it later.
    // This avoids exposing shader implementations like bind groups.
    camera_buffer: wgpu::Buffer,
    stage_uniforms_buffer: wgpu::Buffer,
    light_transform_buffer: wgpu::Buffer,
    per_frame_bind_group: crate::shader::model::bind_groups::BindGroup0,
    skeleton_camera_bind_group: crate::shader::skeleton::bind_groups::BindGroup0,

    shadow_depth: TextureSamplerView,
    variance_shadow: TextureSamplerView,
    variance_bind_group: crate::shader::variance_shadow::bind_groups::BindGroup0,

    pass_info: PassInfo,
    // TODO: Rework this to allow for updating the lut externally.
    // TODO: What's the easiest format to allow these updates?
    color_lut: TextureSamplerView,

    // TODO: Should this be configurable at runtime?
    clear_color: [f64; 3],

    render_settings: RenderSettings,
    render_settings_buffer: wgpu::Buffer,

    // TODO: Find a way to simplify this?
    brush: Option<TextBrush<FontRef<'static>, DefaultSectionHasher>>,
}

impl SsbhRenderer {
    /// Initializes the renderer for the given dimensions and monitor scaling.
    ///
    /// The `scale_factor` should typically match the monitor scaling in the OS such as `1.5` for 150% scaling.
    /// If unsure, set `scale_factor` to `1.0`.
    ///
    /// The `clear_color` determines the RGB color of the viewport background.
    ///
    /// This is an expensive operation, so applications should create and reuse a single [SsbhRenderer].
    /// Use [SsbhRenderer::resize] and [SsbhRenderer::update_camera] for changing window sizes and user interaction.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        initial_width: u32,
        initial_height: u32,
        scale_factor: f64,
        clear_color: [f64; 3],
        font_bytes: &'static [u8],
    ) -> Self {
        let shader = crate::shader::post_process::create_shader_module(device);
        let layout = crate::shader::post_process::create_pipeline_layout(device);
        let post_process_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Post Processing Pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(crate::RGBA_COLOR_FORMAT.into())],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        let shader = crate::shader::overlay::create_shader_module(device);
        let layout = crate::shader::overlay::create_pipeline_layout(device);
        let overlay_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Overlay Pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(crate::RGBA_COLOR_FORMAT.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        // Shared shaders for bloom passes.
        // TODO: Should this be all screen texture shaders?
        let shader = crate::shader::bloom::create_shader_module(device);
        let layout = crate::shader::bloom::create_pipeline_layout(device);
        let bloom_threshold_pipeline =
            create_screen_pipeline(device, &shader, &layout, "fs_threshold", BLOOM_COLOR_FORMAT);

        let bloom_blur_pipeline =
            create_screen_pipeline(device, &shader, &layout, "fs_blur", BLOOM_COLOR_FORMAT);

        let bloom_upscale_pipeline = create_screen_pipeline(
            device,
            &shader,
            &layout,
            "fs_upscale",
            crate::RGBA_COLOR_FORMAT,
        );

        let shader = crate::shader::bloom_combine::create_shader_module(device);
        let layout = crate::shader::bloom_combine::create_pipeline_layout(device);
        let bloom_combine_pipeline = create_screen_pipeline(
            device,
            &shader,
            &layout,
            "fs_main",
            crate::RGBA_COLOR_FORMAT,
        );

        let module = crate::shader::skinning::create_shader_module(device);
        let layout = crate::shader::skinning::create_pipeline_layout(device);
        // TODO: Better support compute shaders in wgsl_to_wgpu.
        let skinning_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Vertex Skinning Compute"),
            layout: Some(&layout),
            module: &module,
            entry_point: "main",
        });

        let module = crate::shader::renormal::create_shader_module(device);
        let layout = crate::shader::renormal::create_pipeline_layout(device);
        // TODO: Better support compute shaders in wgsl_to_wgpu.
        let renormal_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Vertex Renormal Compute"),
            layout: Some(&layout),
            module: &module,
            entry_point: "main",
        });

        let shadow_pipeline = create_depth_pipeline(device);

        let shader = crate::shader::variance_shadow::create_shader_module(device);
        let layout = crate::shader::variance_shadow::create_pipeline_layout(device);
        let variance_shadow_pipeline =
            create_screen_pipeline(device, &shader, &layout, "fs_main", VARIANCE_SHADOW_FORMAT);

        // TODO: Where should stage specific assets be loaded?
        let (color_lut_view, color_lut_sampler) = load_default_lut(device, queue);
        let color_lut = TextureSamplerView {
            view: color_lut_view,
            sampler: color_lut_sampler,
        };

        // TODO: Create a struct to store the stage rendering data?
        let pass_info = PassInfo::new(
            device,
            initial_width,
            initial_height,
            scale_factor,
            &color_lut,
        );

        // Assume the user will update the camera, so these values don't matter.
        let camera_buffer = uniform_buffer(
            device,
            "Camera Buffer",
            &[crate::shader::model::CameraTransforms {
                model_view_matrix: glam::Mat4::IDENTITY,
                mvp_matrix: glam::Mat4::IDENTITY,
                camera_pos: [0.0, 0.0, -1.0, 1.0],
                screen_dimensions: [1.0; 4],
            }],
        );

        // TODO: Don't always assume that the camera bind groups are identical.
        let skeleton_camera_bind_group =
            crate::shader::skeleton::bind_groups::BindGroup0::from_bindings(
                device,
                crate::shader::skeleton::bind_groups::BindGroupLayout0 {
                    camera: camera_buffer.as_entire_buffer_binding(),
                },
            );

        let light_transform = calculate_light_transform(
            glam::Quat::from_xyzw(-0.495286, -0.0751228, 0.0431234, 0.864401),
            glam::Vec3::new(25.0, 25.0, 50.0),
        );

        let light_transform_buffer = uniform_buffer(
            device,
            "Light Transform Buffer",
            &[crate::shader::model::LightTransforms { light_transform }],
        );

        // Depth from the perspective of the light.
        // TODO: Multiple lights require multiple depth maps?
        let shadow_depth = create_depth(device, SHADOW_MAP_WIDTH, SHADOW_MAP_HEIGHT);

        let variance_shadow = create_texture_sampler(
            device,
            VARIANCE_SHADOW_WIDTH,
            VARIANCE_SHADOW_HEIGHT,
            VARIANCE_SHADOW_FORMAT,
        );

        let render_settings = RenderSettings::default();
        let render_settings_buffer = uniform_buffer(
            device,
            "Render Settings Buffer",
            &[crate::shader::model::RenderSettings::from(&render_settings)],
        );

        // The light nuanmb should be public with conversions for quaternions, vectors, etc being private.
        // stage light nuanmb -> uniform struct -> buffer
        let stage_uniforms_buffer = uniform_buffer(
            device,
            "Stage Uniforms Buffer",
            &[crate::shader::model::StageUniforms::training()],
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

        let per_frame_bind_group = crate::shader::model::bind_groups::BindGroup0::from_bindings(
            device,
            crate::shader::model::bind_groups::BindGroupLayout0 {
                camera: camera_buffer.as_entire_buffer_binding(),
                texture_shadow: &variance_shadow.view,
                default_sampler: &default_sampler,
                light: light_transform_buffer.as_entire_buffer_binding(),
                render_settings: render_settings_buffer.as_entire_buffer_binding(),
                stage_uniforms: stage_uniforms_buffer.as_entire_buffer_binding(),
                uv_pattern: &uv_pattern.create_view(&wgpu::TextureViewDescriptor::default()),
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

        let invalid_shader_pipeline = create_invalid_shader_pipeline(device, RGBA_COLOR_FORMAT);
        let invalid_attributes_pipeline =
            create_invalid_attributes_pipeline(device, RGBA_COLOR_FORMAT);
        let debug_pipeline = create_debug_pipeline(device, RGBA_COLOR_FORMAT);
        let silhouette_pipeline = create_silhouette_pipeline(device, RGBA_COLOR_FORMAT);
        let outline_pipeline = create_outline_pipeline(device, RGBA_COLOR_FORMAT);
        let uv_pipeline = create_uv_pipeline(device, RGBA_COLOR_FORMAT);
        let wireframe_pipeline = create_wireframe_pipeline(device, RGBA_COLOR_FORMAT);

        // TODO: Does this need to match the initial config?
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: RGBA_COLOR_FORMAT,
            width: initial_width,
            height: initial_height,
            present_mode: wgpu::PresentMode::Mailbox,
        };

        // TODO: Log errors?
        let brush = BrushBuilder::using_font_bytes(font_bytes)
            .ok()
            .map(|b| b.build(device, &config));

        let bone_pipelines = BonePipelines::new(device);
        let bone_buffers = BoneBuffers::new(device);

        let selected_material_pipeline =
            create_selected_material_pipeline(device, RGBA_COLOR_FORMAT);

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
            light_transform_buffer,
            bone_pipelines,
            invalid_shader_pipeline,
            invalid_attributes_pipeline,
            debug_pipeline,
            silhouette_pipeline,
            outline_pipeline,
            uv_pipeline,
            render_settings,
            render_settings_buffer,
            brush,
            bone_buffers,
            overlay_pipeline,
            wireframe_pipeline,
            selected_material_pipeline,
        }
    }

    /// A faster alternative to creating a new [SsbhRenderer] with the desired size.
    ///
    /// The `scale_factor` maps physical pixels to logical pixels.
    /// This adjusts screen based effects such as bloom to have a more appropriate scale on high DPI screens.
    /// This should usually match the current monitor's scaling factor
    /// in the OS such as `1.5` for 150% scaling.
    ///
    /// Prefer this method over calling [SsbhRenderer::new] with the updated dimensions.
    /// To update the camera to a potentially new aspect ratio,
    /// pass the appropriate matrix to [SsbhRenderer::update_camera].
    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        scale_factor: f64,
    ) {
        self.pass_info = PassInfo::new(device, width, height, scale_factor, &self.color_lut);
        if let Some(brush) = self.brush.as_mut() {
            brush.resize_view(width as f32, height as f32, queue);
        }
    }

    /// Updates the camera transforms.
    /// This method is lightweight, so it can be called each frame if necessary.
    pub fn update_camera(&mut self, queue: &wgpu::Queue, transforms: CameraTransforms) {
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[transforms]));
    }

    /// Updates the render settings.
    /// This method is lightweight, so it can be called each frame if necessary.
    pub fn update_render_settings(
        &mut self,
        queue: &wgpu::Queue,
        render_settings: &RenderSettings,
    ) {
        self.render_settings = *render_settings;
        queue.write_buffer(
            &self.render_settings_buffer,
            0,
            bytemuck::cast_slice(&[crate::shader::model::RenderSettings::from(render_settings)]),
        );
    }

    /// Updates the stage lighting data.
    /// This method is lightweight, so it can be called each frame if necessary.
    pub fn update_stage_uniforms(&mut self, queue: &wgpu::Queue, data: &AnimData) {
        // TODO: How to animate using the current frame?
        let (stage_uniforms, light_transform) = anim_to_lights(data);

        queue.write_buffer(
            &self.stage_uniforms_buffer,
            0,
            bytemuck::cast_slice(&[stage_uniforms]),
        );

        queue.write_buffer(
            &self.light_transform_buffer,
            0,
            bytemuck::cast_slice(&[crate::shader::model::LightTransforms { light_transform }]),
        );
    }

    /// Sets the viewport background color.
    pub fn set_clear_color(&mut self, color: [f64; 3]) {
        self.clear_color = color;
    }

    /// Renders the `render_meshes` to `output_view` using the standard rendering passes for Smash Ultimate.
    ///
    /// The `output_view` should have the format [crate::RGBA_COLOR_FORMAT].
    /// The output is cleared before drawing.
    pub fn render_ssbh_passes(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        render_models: &[RenderModel],
        shader_database: &ShaderDatabase,
    ) {
        // Render meshes are sorted globally rather than per folder.
        // This allows all transparent draw calls to happen after opaque draw calls.
        // TODO: How to have RenderModel own all resources but still sort RenderMesh?
        // let mut meshes: Vec<_> = render_models.iter().flat_map(|m| &m.meshes).collect();
        // meshes.sort_by_key(|m| m.render_order());

        // Transform the vertex positions and normals.
        // Always run compute passes to preserve vertex positions when switching to debug shading.
        self.skinning_pass(encoder, render_models);
        self.renormal_pass(encoder, render_models);

        // TODO: Don't make color_final a parameter since we already take self.
        if self.render_settings.debug_mode != DebugMode::Shaded {
            self.model_debug_pass(encoder, render_models);
        } else {
            // Depth only pass for shadow maps.
            self.shadow_pass(encoder, render_models);

            // Create the two channel shadow map for variance shadows.
            self.variance_shadow_pass(encoder);

            // Draw the models to the initial color texture.
            self.model_pass(encoder, render_models, shader_database);

            // Extract the portions of the image that contribute to bloom.
            self.bloom_threshold_pass(encoder, self.render_settings.render_bloom);

            // Repeatedly downsample and blur the thresholded bloom colors.
            self.bloom_blur_passes(encoder);

            // Combine the bloom textures into a single texture.
            self.bloom_combine_pass(encoder);

            // Upscale with bilinear filtering to smooth the result.
            self.bloom_upscale_pass(encoder);

            // TODO: Models with _near should be drawn after bloom but before post processing?
            // TODO: How does this impact the depth buffer?

            // Combine the model and bloom contributions and apply color grading.
            self.post_processing_pass(encoder, &self.pass_info.color_final.view);
        }

        // TODO: Add additional passes to create outlines?
        // Draw selected meshes to silhouette texture and stencil texture.
        let rendered_silhouette = self.model_silhouette_pass(encoder, render_models);

        // Expand silhouettes to create outlines using stencil texture
        self.outline_pass(encoder, rendered_silhouette);

        // Composite the outlines onto the result of the debug or shaded passes.
        self.overlay_pass(encoder, output_view);
    }

    /// Renders UVs for all of the meshes with `is_selected` set to `true`.
    pub fn render_model_uvs<'a>(
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

    /// Renders the corresponding skeleton in `skel` for each model in `render_models` to `output_view`.
    ///
    /// The `output_view` should have the format [crate::RGBA_COLOR_FORMAT].
    /// The output is not cleared before drawing.
    pub fn render_skeleton(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        render_models: &[RenderModel],
        skels: &[Option<&SkelData>],
        width: u32,
        height: u32,
        mvp: glam::Mat4,
        bone_name_font_size: Option<f32>,
        draw_bone_axes: bool,
    ) -> Option<wgpu::CommandBuffer> {
        self.skeleton_pass(encoder, render_models, output_view, skels, draw_bone_axes);

        if let Some(font_size) = bone_name_font_size {
            let brush = self.brush.as_mut()?;

            for (model, skel) in render_models.iter().zip(skels) {
                model.queue_bone_names(*skel, brush, width, height, mvp, font_size);
            }

            // TODO: Make text rendering optional without returning the command buffer?
            Some(brush.draw(device, output_view, queue))
        } else {
            None
        }
    }

    /// Renders meshes for the selected model and material with a solid color.
    ///
    /// The `output_view` should have the format [crate::RGBA_COLOR_FORMAT].
    pub fn render_material_mask(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        render_models: &[RenderModel],
        model_index: usize,
        material_label: &str,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Material Mask Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.pass_info.depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: false,
                }),
                stencil_ops: None,
            }),
        });

        // Material labels may be repeated in multiple models.
        // Only show the selected material for the specified model.
        if let Some(model) = render_models.get(model_index) {
            model.draw_meshes_material_mask(
                &mut pass,
                &self.per_frame_bind_group,
                &self.selected_material_pipeline,
                material_label,
            );
        }
    }

    fn bloom_upscale_pass(&self, encoder: &mut wgpu::CommandEncoder) {
        bloom_pass(
            encoder,
            "Bloom Upscale Pass",
            &self.bloom_upscale_pipeline,
            &self.pass_info.bloom_upscaled.view,
            &self.pass_info.bloom_upscale_bind_group,
        );
    }

    fn bloom_blur_passes(&self, encoder: &mut wgpu::CommandEncoder) {
        for (texture, bind_group0) in &self.pass_info.bloom_blur_colors {
            bloom_pass(
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
            bloom_pass(
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
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
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
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });
        variance_shadow_pass.set_pipeline(&self.variance_shadow_pipeline);
        crate::shader::variance_shadow::bind_groups::set_bind_groups(
            &mut variance_shadow_pass,
            crate::shader::variance_shadow::bind_groups::BindGroups {
                bind_group0: &self.variance_bind_group,
            },
        );
        variance_shadow_pass.draw(0..3, 0..1);
    }

    fn skinning_pass(&self, encoder: &mut wgpu::CommandEncoder, render_models: &[RenderModel]) {
        // Skin the render meshes using a compute pass instead of in the vertex shader.
        // Compute shaders give more flexibility compared to vertex shaders.
        // Modifying the vertex buffers once avoids redundant work in later passes.
        let mut skinning_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Skinning Pass"),
        });
        skinning_pass.set_pipeline(&self.skinning_pipeline);
        for model in render_models {
            crate::rendermesh::dispatch_skinning(&model.meshes, &mut skinning_pass);
        }
    }

    fn renormal_pass(&self, encoder: &mut wgpu::CommandEncoder, render_models: &[RenderModel]) {
        // TODO: This doesn't appear to be a compute shader in game?
        // TODO: What is the performance cost of this?
        let mut renormal_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Renormal Pass"),
        });
        renormal_pass.set_pipeline(&self.renormal_pipeline);
        for model in render_models {
            crate::rendermesh::dispatch_renormal(&model.meshes, &mut renormal_pass);
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
        let mut model_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Model Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.pass_info.color.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(self.clear_color()),
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.pass_info.depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        // TODO: Investigate sorting.
        self.draw_render_models(render_models, &mut model_pass, shader_database, "opaque");
        self.draw_render_models(render_models, &mut model_pass, shader_database, "far");
        self.draw_render_models(render_models, &mut model_pass, shader_database, "sort");
        self.draw_render_models(render_models, &mut model_pass, shader_database, "near");
    }

    fn draw_render_models<'a>(
        &'a self,
        render_models: &'a [RenderModel],
        model_pass: &mut wgpu::RenderPass<'a>,
        shader_database: &ShaderDatabase,
        pass: &str,
    ) {
        for model in render_models.iter().filter(|m| m.is_visible) {
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

    fn model_silhouette_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        render_models: &[RenderModel],
    ) -> bool {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Model Silhouette Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.pass_info.selected_silhouettes.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.pass_info.selected_stencil.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0xff),
                    store: true,
                }),
            }),
        });

        pass.set_pipeline(&self.silhouette_pipeline);

        let mut active = false;
        for model in render_models {
            active |= model.draw_meshes_silhouettes(&mut pass, &self.per_frame_bind_group);
        }
        active
    }

    fn model_debug_pass(&self, encoder: &mut wgpu::CommandEncoder, render_models: &[RenderModel]) {
        let mut debug_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Model Debug Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.pass_info.color_final.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(self.clear_color()),
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.pass_info.depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        debug_pass.set_pipeline(&self.debug_pipeline);
        for model in render_models {
            model.draw_meshes_debug(&mut debug_pass, &self.per_frame_bind_group);
        }

        // TODO: Add antialiasing?
        if self.render_settings.wireframe {
            debug_pass.set_pipeline(&self.wireframe_pipeline);
            for model in render_models {
                model.draw_meshes_debug(&mut debug_pass, &self.per_frame_bind_group);
            }
        }
    }

    fn clear_color(&self) -> wgpu::Color {
        // Always clear alpha to avoid post processing the background.
        wgpu::Color {
            r: self.clear_color[0],
            g: self.clear_color[1],
            b: self.clear_color[2],
            a: 0.0,
        }
    }

    fn skeleton_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        render_models: &[RenderModel],
        view: &wgpu::TextureView,
        skels: &[Option<&SkelData>],
        draw_bone_axes: bool,
    ) {
        // TODO: Force having a color attachment for each fragment shader output in wgsl_to_wgpu?
        let mut skeleton_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Skeleton Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.pass_info.depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        for (model, skel) in render_models.iter().zip(skels) {
            model.draw_skeleton(
                *skel,
                &self.bone_buffers,
                &mut skeleton_pass,
                &self.skeleton_camera_bind_group,
                &self.bone_pipelines,
                draw_bone_axes,
            );
        }
    }

    fn outline_pass(&self, encoder: &mut wgpu::CommandEncoder, enabled: bool) {
        // Always clear the outlines even if nothing is selected.
        let mut outline_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Outline Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.pass_info.selected_outlines.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.pass_info.selected_stencil.view,
                depth_ops: None,
                stencil_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: false,
                }),
            }),
        });

        if enabled {
            outline_pass.set_pipeline(&self.outline_pipeline);
            crate::shader::outline::bind_groups::set_bind_groups(
                &mut outline_pass,
                crate::shader::outline::bind_groups::BindGroups {
                    bind_group0: &self.pass_info.outline_bind_group,
                },
            );
            // Mask out the inner black regions to keep the outline.
            outline_pass.set_stencil_reference(0xff);
            outline_pass.draw(0..3, 0..1);
        }
    }

    fn overlay_pass(&self, encoder: &mut wgpu::CommandEncoder, output_view: &wgpu::TextureView) {
        let mut overlay_pass = create_color_pass(encoder, output_view, Some("Overlay Pass"));
        overlay_pass.set_pipeline(&self.overlay_pipeline);
        crate::shader::overlay::bind_groups::set_bind_groups(
            &mut overlay_pass,
            crate::shader::overlay::bind_groups::BindGroups {
                bind_group0: &self.pass_info.overlay_bind_group,
            },
        );
        overlay_pass.draw(0..3, 0..1);
    }

    fn post_processing_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
    ) {
        let mut post_processing_pass =
            create_color_pass(encoder, output_view, Some("Post Processing Pass"));
        post_processing_pass.set_pipeline(&self.post_process_pipeline);
        crate::shader::post_process::bind_groups::set_bind_groups(
            &mut post_processing_pass,
            crate::shader::post_process::bind_groups::BindGroups {
                bind_group0: &self.pass_info.post_process_bind_group,
            },
        );
        post_processing_pass.draw(0..3, 0..1);
    }

    fn bloom_combine_pass(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut bloom_combine_pass = create_color_pass(
            encoder,
            &self.pass_info.bloom_combined.view,
            Some("Bloom Combined Pass"),
        );
        bloom_combine_pass.set_pipeline(&self.bloom_combine_pipeline);
        crate::shader::bloom_combine::bind_groups::set_bind_groups(
            &mut bloom_combine_pass,
            crate::shader::bloom_combine::bind_groups::BindGroups {
                bind_group0: &self.pass_info.bloom_combine_bind_group,
            },
        );
        bloom_combine_pass.draw(0..3, 0..1);
    }

    fn shadow_pass(&self, encoder: &mut wgpu::CommandEncoder, render_models: &[RenderModel]) {
        let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Shadow Pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.shadow_depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });
        shadow_pass.set_pipeline(&self.shadow_pipeline);
        for model in render_models {
            model.draw_render_meshes_depth(&mut shadow_pass, &self.per_frame_bind_group);
        }
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
            entry_point: "vs_main",
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: fs_main,
            targets: &[Some(target.into())],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    })
}

fn bloom_pass(
    encoder: &mut wgpu::CommandEncoder,
    name: &str,
    pipeline: &wgpu::RenderPipeline,
    view: &wgpu::TextureView,
    bind_group: &crate::shader::bloom::bind_groups::BindGroup0,
) {
    let mut pass = create_color_pass(encoder, view, Some(name));
    pass.set_pipeline(pipeline);
    crate::shader::bloom::bind_groups::set_bind_groups(
        &mut pass,
        crate::shader::bloom::bind_groups::BindGroups {
            bind_group0: bind_group,
        },
    );
    pass.draw(0..3, 0..1);
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
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: true,
            },
        })],
        depth_stencil_attachment: None,
    })
}

struct PassInfo {
    depth: TextureSamplerView,
    color: TextureSamplerView,

    // Final color before applying overlays
    color_final: TextureSamplerView,

    bloom_threshold: TextureSamplerView,

    selected_stencil: TextureSamplerView,
    selected_silhouettes: TextureSamplerView,
    selected_outlines: TextureSamplerView,

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
    outline_bind_group: crate::shader::outline::bind_groups::BindGroup0,
}

impl PassInfo {
    fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        scale_factor: f64,
        color_lut: &TextureSamplerView,
    ) -> Self {
        let depth = create_depth(device, width, height);

        let color = create_texture_sampler(device, width, height, crate::RGBA_COLOR_FORMAT);
        let color_final = create_texture_sampler(device, width, height, crate::RGBA_COLOR_FORMAT);

        // Bloom uses successively smaller render targets to increase the blur.
        // Account for monitor scaling to avoid a smaller perceived radius on high DPI screens.
        // Some devices like laptops or phones have weak GPUs but high DPI screens.
        // Lowering bloom resolution can reduce performance bottlenecks on these devices.
        let scale_factor = scale_factor.max(1.0);
        let bloom_width = (width as f64 / scale_factor) as u32;
        let bloom_height = (height as f64 / scale_factor) as u32;

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
        );
        // A 2x bilinear upscale smooths the overall result.
        let (bloom_upscaled, bloom_upscale_bind_group) = create_bloom_bind_group(
            device,
            bloom_width / 2,
            bloom_height / 2,
            &bloom_combined,
            crate::RGBA_COLOR_FORMAT,
        );

        let post_process_bind_group =
            create_post_process_bind_group(device, &color, &bloom_upscaled, color_lut);

        let selected_stencil = create_depth_stencil(device, width, height);
        // TODO: Downsample these textures based on scaling for thicker outlines?
        let selected_silhouettes =
            create_texture_sampler(device, width, height, crate::RGBA_COLOR_FORMAT);
        let selected_outlines =
            create_texture_sampler(device, width, height, crate::RGBA_COLOR_FORMAT);

        let outline_bind_group = create_outline_bind_group(device, &selected_silhouettes);

        let overlay_bind_group =
            create_overlay_bind_group(device, &color_final, &selected_outlines);

        Self {
            depth,
            color,
            color_final,
            bloom_threshold,
            bloom_threshold_bind_group,
            bloom_blur_colors,
            bloom_combined,
            bloom_combine_bind_group,
            bloom_upscaled,
            bloom_upscale_bind_group,
            post_process_bind_group,
            selected_stencil,
            selected_silhouettes,
            selected_outlines,
            overlay_bind_group,
            outline_bind_group,
        }
    }
}

struct TextureSamplerView {
    sampler: wgpu::Sampler,
    view: wgpu::TextureView,
}

fn create_depth(device: &wgpu::Device, width: u32, height: u32) -> TextureSamplerView {
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let desc = wgpu::TextureDescriptor {
        label: Some("depth texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    };
    let texture = device.create_texture(&desc);

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        compare: None,
        ..Default::default()
    });

    TextureSamplerView { view, sampler }
}

fn create_depth_stencil(device: &wgpu::Device, width: u32, height: u32) -> TextureSamplerView {
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let desc = wgpu::TextureDescriptor {
        label: Some("Depth Stencil Texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_STENCIL_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    };
    let texture = device.create_texture(&desc);

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
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
) -> TextureSamplerView {
    // TODO: Labels
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("color texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    });

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
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
) -> (
    TextureSamplerView,
    crate::shader::bloom_combine::bind_groups::BindGroup0,
) {
    let texture = create_texture_sampler(device, width, height, crate::RGBA_COLOR_FORMAT);

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
    let texture = create_texture_sampler(device, width, height, format);

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
) -> crate::shader::overlay::bind_groups::BindGroup0 {
    crate::shader::overlay::bind_groups::BindGroup0::from_bindings(
        device,
        crate::shader::overlay::bind_groups::BindGroupLayout0 {
            color_texture: &color_final.view,
            color_sampler: &color_final.sampler,
            outline_texture: &outline_texture.view,
            outline_sampler: &outline_texture.sampler,
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
    let shader = crate::shader::outline::create_shader_module(device);
    let render_pipeline_layout = crate::shader::outline::create_pipeline_layout(device);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Outline"),
        layout: Some(&render_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(surface_format.into())],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_STENCIL_FORMAT,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::Always,
            // Use the mask from earlier only keep the blurred outline.
            stencil: wgpu::StencilState {
                front: wgpu::StencilFaceState {
                    compare: wgpu::CompareFunction::Equal,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::Keep,
                },
                back: wgpu::StencilFaceState {
                    compare: wgpu::CompareFunction::Equal,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::Keep,
                },
                read_mask: 0xff,
                write_mask: 0xff,
            },
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    })
}
