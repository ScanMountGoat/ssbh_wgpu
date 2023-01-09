use strum::{Display, EnumString, EnumVariantNames};

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
    // TODO: Change this to Unlit?
    /// The final albedo or base color after applying textures and materials.
    Albedo,
    /// Relative shader complexity based on instruction count.
    ShaderComplexity,
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

/// Settings for configuring the rendered output of an [crate::SsbhRenderer].
/// These settings modify internal WGPU state and should only be updated as needed.
#[derive(PartialEq, Clone, Copy)]
pub struct RenderSettings {
    /// The attribute to render as the output color when [Some].
    pub debug_mode: DebugMode,
    /// The secondary material when rendering with [DebugMode::Shaded].
    /// The [transition_factor](#structfield.transition_factor) controls the mix intensity.
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
    /// Apply the in game scale factors such as `2.0` for colorSet1 when `true`.
    /// This applies to all modes including [DebugMode::Shaded].
    pub scale_vertex_color: bool,
    pub render_rgba: [bool; 4],
    /// Replaces the RGBA channels of the nor map (Texture4) with a default when false.
    pub render_nor: [bool; 4],
    /// Replaces the RGBA channels of the prm map (Texture6) with a default when false.
    pub render_prm: [bool; 4],
    /// Use a UV test pattern for UV debug modes when `true`. Otherwise, display UVs as RGB colors.
    pub use_uv_pattern: bool,
}

impl From<&RenderSettings> for crate::shader::model::RenderSettings {
    fn from(r: &RenderSettings) -> Self {
        Self {
            debug_mode: glam::UVec4::splat(r.debug_mode as u32),
            transition_material: glam::UVec4::splat(r.transition_material as u32),
            transition_factor: glam::vec4(r.transition_factor, 0.0, 0.0, 0.0),
            render_diffuse: glam::UVec4::splat(r.render_diffuse as u32),
            render_specular: glam::UVec4::splat(r.render_specular as u32),
            render_emission: glam::UVec4::splat(r.render_emission as u32),
            render_rim_lighting: glam::UVec4::splat(r.render_rim_lighting as u32),
            render_shadows: glam::UVec4::splat(r.render_shadows as u32),
            render_bloom: glam::UVec4::splat(r.render_bloom as u32),
            render_vertex_color: glam::UVec4::splat(r.render_vertex_color as u32),
            scale_vertex_color: glam::UVec4::splat(r.scale_vertex_color as u32),
            render_rgba: r.render_rgba.map(|b| if b { 1.0 } else { 0.0 }).into(),
            render_nor: r.render_nor.map(|b| b as u32).into(),
            render_prm: r.render_prm.map(|b| b as u32).into(),
            render_uv_pattern: glam::UVec4::splat(r.use_uv_pattern as u32),
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
            scale_vertex_color: true,
            render_rgba: [true; 4],
            render_nor: [true; 4],
            render_prm: [true; 4],
            use_uv_pattern: true,
        }
    }
}

/// Settings for configuring vertex skinning and skeletal animation rendering.
/// These settings modify internal WGPU state and should only be updated as needed.
#[derive(PartialEq, Clone, Copy)]
pub struct SkinningSettings {
    pub enable_parenting: bool,
    pub enable_skinning: bool,
}

impl From<&SkinningSettings> for crate::shader::skinning::SkinningSettings {
    fn from(s: &SkinningSettings) -> Self {
        Self {
            enable_parenting: glam::UVec4::splat(s.enable_parenting as u32),
            enable_skinning: glam::UVec4::splat(s.enable_skinning as u32),
        }
    }
}

impl Default for SkinningSettings {
    fn default() -> Self {
        Self {
            enable_parenting: true,
            enable_skinning: true,
        }
    }
}

/// Lightweight settings for configuring model rendering each frame.
///
/// Renders materials in a solid color for the given `mask_model_index` and
/// `mask_material_label`. Use `""` for disabling the mask.
#[derive(Debug, Default)]
pub struct ModelRenderOptions {
    pub draw_bones: bool,
    pub draw_bone_axes: bool,
    // TODO: Make these Option instead?
    pub mask_model_index: usize,
    pub mask_material_label: String,
    /// Draw a wireframe on shaded when `true` for all modes except [DebugMode::Shaded].
    pub draw_wireframe: bool,
    /// Draw an infinite grid on the XZ-axis when `true`.
    pub draw_floor_grid: bool,
    /// Draw collision shapes for the swing.prc when `true`.
    pub draw_swing: bool,
}
