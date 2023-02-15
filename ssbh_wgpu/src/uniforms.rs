use std::str::FromStr;

use crate::{
    model::SamplerCache,
    shader::model::PerMaterial,
    split_param,
    texture::{create_sampler, load_default, load_texture, LoadTextureError},
    DeviceBufferExt, ShaderDatabase,
};
use log::warn;
use ssbh_data::matl_data::*;
use wgpu::SamplerDescriptor;

// TODO: Move this to textures?
pub fn material_uniforms_bind_group(
    material: &ssbh_data::matl_data::MatlEntryData,
    device: &wgpu::Device,
    textures: &[(String, wgpu::Texture, wgpu::TextureViewDimension)],
    default_textures: &[(String, wgpu::Texture, wgpu::TextureViewDimension)],
    uniforms_buffer: &wgpu::Buffer, // TODO: Just return this?
    sampler_by_data: &mut SamplerCache,
) -> crate::shader::model::bind_groups::BindGroup2 {
    // TODO: Do all 2D textures default to white if the path isn't correct?
    let default_white = &default_textures
        .iter()
        .find(|d| d.0 == "/common/shader/sfxpbs/default_white")
        .unwrap()
        .1;

    let default_cube = &default_textures
        .iter()
        .find(|d| d.0 == "#replace_cubemap")
        .unwrap()
        .1;

    let load_texture = |texture_id, dim| {
        // TODO: Add proper path and parameter handling.
        // TODO: Find a way to test texture path loading.
        // This should also handle paths like "../texture.nutexb" and "/render/shader/bin/texture.nutexb".
        material.textures
                    .iter()
                    .find(|t| t.param_id == texture_id)
                    .map(|t| t.data.as_str())
            .and_then(|material_path| {
                load_texture(material_path, textures, default_textures, dim).map_err(|e| {
                    match e {
                        LoadTextureError::PathNotFound => {
                            warn!("Missing texture {:?} assigned to {}. Applying default texture.", material_path, texture_id)
                        },
                        LoadTextureError::DimensionMismatch { expected, actual } => {
                            warn!("Texture {:?} assigned to {} has invalid dimensions. Expected {:?} but found {:?}.", 
                                material_path, texture_id, expected, actual)
                        },
                    }
                }
                ).ok()
            }).unwrap_or_else(|| load_default(texture_id, default_cube, default_white))
    };

    update_sampler_cache(device, material, sampler_by_data);

    // TODO: This could be combined with above if we only cache for each material?
    let default_sampler = device.create_sampler(&SamplerDescriptor::default());
    let load_sampler = |sampler_id| {
        material
            .samplers
            .iter()
            .find(|s| s.param_id == sampler_id)
            .and_then(|sampler| sampler_by_data.iter().find(|(d, _)| d == &sampler.data))
            .map(|(_, s)| s)
            .unwrap_or(&default_sampler)
    };

    // TODO: Default texture for other cube maps?
    crate::shader::model::bind_groups::BindGroup2::from_bindings(
        device,
        crate::shader::model::bind_groups::BindGroupLayout2 {
            texture0: &load_texture(ParamId::Texture0, wgpu::TextureViewDimension::D2),
            sampler0: load_sampler(ParamId::Sampler0),
            texture1: &load_texture(ParamId::Texture1, wgpu::TextureViewDimension::D2),
            sampler1: load_sampler(ParamId::Sampler1),
            texture2: &load_texture(ParamId::Texture2, wgpu::TextureViewDimension::Cube),
            sampler2: load_sampler(ParamId::Sampler2),
            texture3: &load_texture(ParamId::Texture3, wgpu::TextureViewDimension::D2),
            sampler3: load_sampler(ParamId::Sampler3),
            texture4: &load_texture(ParamId::Texture4, wgpu::TextureViewDimension::D2),
            sampler4: load_sampler(ParamId::Sampler4),
            texture5: &load_texture(ParamId::Texture5, wgpu::TextureViewDimension::D2),
            sampler5: load_sampler(ParamId::Sampler5),
            texture6: &load_texture(ParamId::Texture6, wgpu::TextureViewDimension::D2),
            sampler6: load_sampler(ParamId::Sampler6),
            texture7: &load_texture(ParamId::Texture7, wgpu::TextureViewDimension::Cube),
            sampler7: load_sampler(ParamId::Sampler7),
            texture8: &load_texture(ParamId::Texture8, wgpu::TextureViewDimension::Cube),
            sampler8: load_sampler(ParamId::Sampler8),
            texture9: &load_texture(ParamId::Texture9, wgpu::TextureViewDimension::D2),
            sampler9: load_sampler(ParamId::Sampler9),
            texture10: &load_texture(ParamId::Texture10, wgpu::TextureViewDimension::D2),
            sampler10: load_sampler(ParamId::Sampler10),
            texture11: &load_texture(ParamId::Texture11, wgpu::TextureViewDimension::D2),
            sampler11: load_sampler(ParamId::Sampler11),
            texture12: &load_texture(ParamId::Texture12, wgpu::TextureViewDimension::D2),
            sampler12: load_sampler(ParamId::Sampler12),
            texture13: &load_texture(ParamId::Texture13, wgpu::TextureViewDimension::D2),
            sampler13: load_sampler(ParamId::Sampler13),
            texture14: &load_texture(ParamId::Texture14, wgpu::TextureViewDimension::D2),
            sampler14: load_sampler(ParamId::Sampler14),
            per_material: uniforms_buffer.as_entire_buffer_binding(),
        },
    )
}

pub fn default_material_uniforms_bind_group(
    device: &wgpu::Device,
    default_textures: &[(String, wgpu::Texture, wgpu::TextureViewDimension)],
    uniforms_buffer: &wgpu::Buffer,
) -> crate::shader::model::bind_groups::BindGroup2 {
    let default_black = &default_textures
        .iter()
        .find(|d| d.0 == "/common/shader/sfxpbs/default_black")
        .unwrap();
    let default_white = default_black.1.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(default_black.2),
        ..Default::default()
    });

    let default_cube = &default_textures
        .iter()
        .find(|d| d.0 == "#replace_cubemap")
        .unwrap();
    let default_cube = default_cube.1.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(default_cube.2),
        ..Default::default()
    });

    let default_sampler = device.create_sampler(&SamplerDescriptor::default());

    // TODO: Default texture for other cube maps?
    crate::shader::model::bind_groups::BindGroup2::from_bindings(
        device,
        crate::shader::model::bind_groups::BindGroupLayout2 {
            texture0: &default_white,
            sampler0: &default_sampler,
            texture1: &default_white,
            sampler1: &default_sampler,
            texture2: &default_cube,
            sampler2: &default_sampler,
            texture3: &default_white,
            sampler3: &default_sampler,
            texture4: &default_white,
            sampler4: &default_sampler,
            texture5: &default_white,
            sampler5: &default_sampler,
            texture6: &default_white,
            sampler6: &default_sampler,
            texture7: &default_cube,
            sampler7: &default_sampler,
            texture8: &default_cube,
            sampler8: &default_sampler,
            texture9: &default_white,
            sampler9: &default_sampler,
            texture10: &default_white,
            sampler10: &default_sampler,
            texture11: &default_white,
            sampler11: &default_sampler,
            texture12: &default_white,
            sampler12: &default_sampler,
            texture13: &default_white,
            sampler13: &default_sampler,
            texture14: &default_white,
            sampler14: &default_sampler,
            per_material: uniforms_buffer.as_entire_buffer_binding(),
        },
    )
}

fn update_sampler_cache(
    device: &wgpu::Device,
    material: &MatlEntryData,
    sampler_by_data: &mut SamplerCache,
) {
    for param in &material.samplers {
        if !sampler_by_data.iter().any(|(d, _)| d == &param.data) {
            let sampler = create_sampler(device, param.param_id, &param.data);
            sampler_by_data.push((param.data.clone(), sampler));
        }
    }
}

pub fn uniforms_buffer(
    material: &MatlEntryData,
    device: &wgpu::Device,
    database: &ShaderDatabase,
) -> wgpu::Buffer {
    let uniforms = per_material(material, database);
    device.create_buffer_from_data(
        "Material Uniforms Buffer",
        &[uniforms],
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    )
}

pub fn default_uniforms_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer_from_data(
        "Material Uniforms Buffer",
        &[DEFAULT_PER_MATERIAL],
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    )
}

// TODO: Test attributes, non required attributes, missing required attributes, etc.
pub fn per_material(material: &MatlEntryData, database: &ShaderDatabase) -> PerMaterial {
    // Ignore invalid parameters for now to avoid an error or panic.
    let mut custom_vector = [glam::Vec4::ZERO; 64];
    for vector in &material.vectors {
        if let Some(index) = vector_index(vector.param_id) {
            custom_vector[index] = vector.data.to_array().into();
        }
    }

    let mut custom_float = [glam::Vec4::ZERO; 20];
    for float in &material.floats {
        if let Some(index) = float_index(float.param_id) {
            custom_float[index][0] = float.data;
        }
    }

    let mut custom_boolean = [glam::UVec4::ZERO; 20];
    for boolean in &material.booleans {
        if let Some(index) = boolean_index(boolean.param_id) {
            custom_boolean[index][0] = boolean.data as u32;
        }
    }

    // The nufxlb defines what parameters are expected.
    // Not all shaders require all parameters.
    let mut has_texture = [glam::UVec4::ZERO; 19];
    let mut has_boolean = [glam::UVec4::ZERO; 20];
    let mut has_float = [glam::UVec4::ZERO; 20];
    let mut has_vector = [glam::UVec4::ZERO; 64];
    if let Some(program) = database.get(&material.shader_label) {
        for param_name in &program.material_parameters {
            // TODO: This is redundant to split twice.
            let (param, _) = split_param(param_name);
            // It's safe to assume the database has valid parameters.
            let id = ParamId::from_str(param).unwrap();
            if let Some(i) = texture_index(id) {
                has_texture[i][0] = 1;
            } else if let Some(i) = boolean_index(id) {
                has_boolean[i][0] = 1;
            } else if let Some(i) = float_index(id) {
                has_float[i][0] = 1;
            } else if let Some(i) = vector_index(id) {
                // Check which components are accessed by the shader binary.
                has_vector[i] = program.accessed_channels(param_name).map(u32::from).into();
            }
        }
    }

    let program = database.get(&material.shader_label);

    let (has_color_set1234, has_color_set567) = if let Some(program) = program {
        (
            glam::UVec4::new(
                program.has_attribute("colorSet1") as u32,
                program.has_attribute("colorSet2") as u32,
                program.has_attribute("colorSet3") as u32,
                program.has_attribute("colorSet4") as u32,
            ),
            glam::UVec4::new(
                program.has_attribute("colorSet5") as u32,
                program.has_attribute("colorSet6") as u32,
                program.has_attribute("colorSet7") as u32,
                0,
            ),
        )
    } else {
        (glam::UVec4::ZERO, glam::UVec4::ZERO)
    };

    let shader_settings = program
        .map(|program| glam::UVec4::new(program.discard as u32, program.premultiplied as u32, 0, 0))
        .unwrap_or_default();

    let lighting_settings = program
        .map(|program| {
            glam::UVec4::new(
                program.lighting as u32,
                program.sh as u32,
                program.receives_shadow as u32,
                0,
            )
        })
        .unwrap_or_default();

    let shader_complexity = program
        .map(|program| glam::Vec4::splat(program.complexity as f32))
        .unwrap_or_default();

    PerMaterial {
        custom_vector,
        custom_boolean,
        custom_float,
        has_texture,
        has_boolean,
        has_float,
        has_vector,
        has_color_set1234,
        has_color_set567,
        shader_settings,
        lighting_settings,
        shader_complexity,
    }
}

pub const DEFAULT_PER_MATERIAL: PerMaterial =
    // Missing values are always set to zero.
    PerMaterial {
        custom_vector: [glam::Vec4::ZERO; 64],
        custom_boolean: [glam::UVec4::ZERO; 20],
        custom_float: [glam::Vec4::ZERO; 20],
        has_boolean: [glam::UVec4::ZERO; 20],
        has_texture: [glam::UVec4::ZERO; 19],
        has_float: [glam::UVec4::ZERO; 20],
        has_vector: [glam::UVec4::ZERO; 64],
        has_color_set1234: glam::UVec4::ZERO,
        has_color_set567: glam::UVec4::ZERO,
        shader_settings: glam::UVec4::ZERO,
        lighting_settings: glam::UVec4::ZERO,
        shader_complexity: glam::Vec4::ZERO,
    };

// TODO: Make this an extension trait?
pub fn vector_index(param: ParamId) -> Option<usize> {
    match param {
        ParamId::CustomVector0 => Some(0),
        ParamId::CustomVector1 => Some(1),
        ParamId::CustomVector2 => Some(2),
        ParamId::CustomVector3 => Some(3),
        ParamId::CustomVector4 => Some(4),
        ParamId::CustomVector5 => Some(5),
        ParamId::CustomVector6 => Some(6),
        ParamId::CustomVector7 => Some(7),
        ParamId::CustomVector8 => Some(8),
        ParamId::CustomVector9 => Some(9),
        ParamId::CustomVector10 => Some(10),
        ParamId::CustomVector11 => Some(11),
        ParamId::CustomVector12 => Some(12),
        ParamId::CustomVector13 => Some(13),
        ParamId::CustomVector14 => Some(14),
        ParamId::CustomVector15 => Some(15),
        ParamId::CustomVector16 => Some(16),
        ParamId::CustomVector17 => Some(17),
        ParamId::CustomVector18 => Some(18),
        ParamId::CustomVector19 => Some(19),
        ParamId::CustomVector20 => Some(20),
        ParamId::CustomVector21 => Some(21),
        ParamId::CustomVector22 => Some(22),
        ParamId::CustomVector23 => Some(23),
        ParamId::CustomVector24 => Some(24),
        ParamId::CustomVector25 => Some(25),
        ParamId::CustomVector26 => Some(26),
        ParamId::CustomVector27 => Some(27),
        ParamId::CustomVector28 => Some(28),
        ParamId::CustomVector29 => Some(29),
        ParamId::CustomVector30 => Some(30),
        ParamId::CustomVector31 => Some(31),
        ParamId::CustomVector32 => Some(32),
        ParamId::CustomVector33 => Some(33),
        ParamId::CustomVector34 => Some(34),
        ParamId::CustomVector35 => Some(35),
        ParamId::CustomVector36 => Some(36),
        ParamId::CustomVector37 => Some(37),
        ParamId::CustomVector38 => Some(38),
        ParamId::CustomVector39 => Some(39),
        ParamId::CustomVector40 => Some(40),
        ParamId::CustomVector41 => Some(41),
        ParamId::CustomVector42 => Some(42),
        ParamId::CustomVector43 => Some(43),
        ParamId::CustomVector44 => Some(44),
        ParamId::CustomVector45 => Some(45),
        ParamId::CustomVector46 => Some(46),
        ParamId::CustomVector47 => Some(47),
        ParamId::CustomVector48 => Some(48),
        ParamId::CustomVector49 => Some(49),
        ParamId::CustomVector50 => Some(50),
        ParamId::CustomVector51 => Some(51),
        ParamId::CustomVector52 => Some(52),
        ParamId::CustomVector53 => Some(53),
        ParamId::CustomVector54 => Some(54),
        ParamId::CustomVector55 => Some(55),
        ParamId::CustomVector56 => Some(56),
        ParamId::CustomVector57 => Some(57),
        ParamId::CustomVector58 => Some(58),
        ParamId::CustomVector59 => Some(59),
        ParamId::CustomVector60 => Some(60),
        ParamId::CustomVector61 => Some(61),
        ParamId::CustomVector62 => Some(62),
        ParamId::CustomVector63 => Some(63),
        _ => None,
    }
}

pub fn float_index(param: ParamId) -> Option<usize> {
    match param {
        ParamId::CustomFloat0 => Some(0),
        ParamId::CustomFloat1 => Some(1),
        ParamId::CustomFloat2 => Some(2),
        ParamId::CustomFloat3 => Some(3),
        ParamId::CustomFloat4 => Some(4),
        ParamId::CustomFloat5 => Some(5),
        ParamId::CustomFloat6 => Some(6),
        ParamId::CustomFloat7 => Some(7),
        ParamId::CustomFloat8 => Some(8),
        ParamId::CustomFloat9 => Some(9),
        ParamId::CustomFloat10 => Some(10),
        ParamId::CustomFloat11 => Some(11),
        ParamId::CustomFloat12 => Some(12),
        ParamId::CustomFloat13 => Some(13),
        ParamId::CustomFloat14 => Some(14),
        ParamId::CustomFloat15 => Some(15),
        ParamId::CustomFloat16 => Some(16),
        ParamId::CustomFloat17 => Some(17),
        ParamId::CustomFloat18 => Some(18),
        ParamId::CustomFloat19 => Some(18),
        _ => None,
    }
}

pub fn texture_index(param: ParamId) -> Option<usize> {
    match param {
        ParamId::Texture0 => Some(0),
        ParamId::Texture1 => Some(1),
        ParamId::Texture2 => Some(2),
        ParamId::Texture3 => Some(3),
        ParamId::Texture4 => Some(4),
        ParamId::Texture5 => Some(5),
        ParamId::Texture6 => Some(6),
        ParamId::Texture7 => Some(7),
        ParamId::Texture8 => Some(8),
        ParamId::Texture9 => Some(9),
        ParamId::Texture10 => Some(10),
        ParamId::Texture11 => Some(11),
        ParamId::Texture12 => Some(12),
        ParamId::Texture13 => Some(13),
        ParamId::Texture14 => Some(14),
        ParamId::Texture15 => Some(15),
        ParamId::Texture16 => Some(16),
        ParamId::Texture17 => Some(17),
        ParamId::Texture18 => Some(18),
        ParamId::Texture19 => Some(19),
        _ => None,
    }
}

pub fn boolean_index(param: ParamId) -> Option<usize> {
    match param {
        ParamId::CustomBoolean0 => Some(0),
        ParamId::CustomBoolean1 => Some(1),
        ParamId::CustomBoolean2 => Some(2),
        ParamId::CustomBoolean3 => Some(3),
        ParamId::CustomBoolean4 => Some(4),
        ParamId::CustomBoolean5 => Some(5),
        ParamId::CustomBoolean6 => Some(6),
        ParamId::CustomBoolean7 => Some(7),
        ParamId::CustomBoolean8 => Some(8),
        ParamId::CustomBoolean9 => Some(9),
        ParamId::CustomBoolean10 => Some(10),
        ParamId::CustomBoolean11 => Some(11),
        ParamId::CustomBoolean12 => Some(12),
        ParamId::CustomBoolean13 => Some(13),
        ParamId::CustomBoolean14 => Some(14),
        ParamId::CustomBoolean15 => Some(15),
        ParamId::CustomBoolean16 => Some(16),
        ParamId::CustomBoolean17 => Some(17),
        ParamId::CustomBoolean18 => Some(18),
        ParamId::CustomBoolean19 => Some(19),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::ShaderProgram;

    use super::*;
    use ssbh_data::Vector4;

    #[test]
    fn create_default_uniforms() {
        assert_eq!(
            PerMaterial {
                custom_vector: [glam::Vec4::ZERO; 64],
                custom_boolean: [glam::UVec4::ZERO; 20],
                custom_float: [glam::Vec4::ZERO; 20],
                has_boolean: [glam::UVec4::ZERO; 20],
                has_float: [glam::UVec4::ZERO; 20],
                has_texture: [glam::UVec4::ZERO; 19],
                has_vector: [glam::UVec4::ZERO; 64],
                has_color_set1234: glam::UVec4::ZERO,
                has_color_set567: glam::UVec4::ZERO,
                shader_settings: glam::UVec4::ZERO,
                lighting_settings: glam::UVec4::ZERO,
                shader_complexity: glam::Vec4::ZERO
            },
            DEFAULT_PER_MATERIAL
        );
    }

    #[test]
    fn create_uniforms_empty_material() {
        assert_eq!(
            PerMaterial {
                custom_vector: [glam::Vec4::ZERO; 64],
                custom_boolean: [glam::UVec4::ZERO; 20],
                custom_float: [glam::Vec4::ZERO; 20],
                has_boolean: [glam::UVec4::ZERO; 20],
                has_float: [glam::UVec4::ZERO; 20],
                has_texture: [glam::UVec4::ZERO; 19],
                has_vector: [glam::UVec4::ZERO; 64],
                has_color_set1234: glam::UVec4::ZERO,
                has_color_set567: glam::UVec4::ZERO,
                shader_settings: glam::UVec4::ZERO,
                lighting_settings: glam::UVec4::ZERO,
                shader_complexity: glam::Vec4::ZERO
            },
            per_material(
                &MatlEntryData {
                    material_label: String::new(),
                    shader_label: String::new(),
                    blend_states: Vec::new(),
                    floats: Vec::new(),
                    booleans: Vec::new(),
                    vectors: Vec::new(),
                    rasterizer_states: Vec::new(),
                    samplers: Vec::new(),
                    textures: Vec::new(),
                },
                &ShaderDatabase::from_iter(std::iter::empty())
            )
        );
    }

    #[test]
    fn create_uniforms_invalid_parameter_indices() {
        // Just ignore an invalid ParamId.
        assert_eq!(
            PerMaterial {
                custom_vector: [glam::Vec4::ZERO; 64],
                custom_boolean: [glam::UVec4::ZERO; 20],
                custom_float: [glam::Vec4::ZERO; 20],
                has_boolean: [glam::UVec4::ZERO; 20],
                has_float: [glam::UVec4::ZERO; 20],
                has_texture: [glam::UVec4::ZERO; 19],
                has_vector: [glam::UVec4::ZERO; 64],
                has_color_set1234: glam::UVec4::ZERO,
                has_color_set567: glam::UVec4::ZERO,
                shader_settings: glam::UVec4::ZERO,
                lighting_settings: glam::UVec4::ZERO,
                shader_complexity: glam::Vec4::ZERO
            },
            per_material(
                &MatlEntryData {
                    material_label: String::new(),
                    shader_label: String::new(),
                    blend_states: vec![BlendStateParam {
                        param_id: ParamId::RasterizerState0,
                        data: BlendStateData::default()
                    }],
                    floats: vec![FloatParam {
                        param_id: ParamId::BlendState0,
                        data: 0.0
                    }],
                    booleans: vec![BooleanParam {
                        param_id: ParamId::CustomVector0,
                        data: false
                    }],
                    vectors: vec![Vector4Param {
                        param_id: ParamId::CustomBoolean0,
                        data: Vector4::default()
                    }],
                    rasterizer_states: vec![RasterizerStateParam {
                        param_id: ParamId::BlendState0,
                        data: RasterizerStateData::default()
                    }],
                    samplers: vec![SamplerParam {
                        param_id: ParamId::Texture0,
                        data: SamplerData::default()
                    }],
                    textures: vec![TextureParam {
                        param_id: ParamId::Sampler0,
                        data: String::new()
                    }],
                },
                &ShaderDatabase::from_iter(std::iter::empty())
            )
        );
    }

    #[test]
    fn create_uniforms_valid_parameters() {
        let mut expected = PerMaterial {
            custom_vector: [glam::Vec4::ZERO; 64],
            custom_boolean: [glam::UVec4::ZERO; 20],
            custom_float: [glam::Vec4::ZERO; 20],
            has_boolean: [glam::UVec4::ZERO; 20],
            has_float: [glam::UVec4::ZERO; 20],
            has_texture: [glam::UVec4::ZERO; 19],
            has_vector: [glam::UVec4::ZERO; 64],
            has_color_set1234: glam::UVec4::ZERO,
            has_color_set567: glam::UVec4::ZERO,
            shader_settings: glam::UVec4::new(1, 0, 0, 0),
            lighting_settings: glam::UVec4::ZERO,
            shader_complexity: glam::Vec4::ZERO,
        };
        expected.custom_vector[0] = glam::vec4(1.0, 2.0, 3.0, 4.0);
        expected.custom_vector[8] = glam::Vec4::splat(1.0);
        expected.custom_boolean[5] = glam::UVec4::new(1, 0, 0, 0);
        expected.custom_float[3] = glam::vec4(0.7, 0.0, 0.0, 0.0);
        // This is based on the database rather than the material.
        expected.has_texture[0] = glam::UVec4::new(1, 0, 0, 0);
        expected.has_boolean[1] = glam::UVec4::new(1, 0, 0, 0);
        expected.has_float[2] = glam::UVec4::new(1, 0, 0, 0);
        expected.has_vector[8] = glam::UVec4::new(1, 0, 0, 1);

        assert_eq!(
            expected,
            per_material(
                &MatlEntryData {
                    material_label: String::new(),
                    shader_label: "SFX_PBS_010002000800824f_opaque".to_owned(),
                    blend_states: vec![BlendStateParam {
                        param_id: ParamId::BlendState0,
                        data: BlendStateData::default()
                    }],
                    floats: vec![FloatParam {
                        param_id: ParamId::CustomFloat3,
                        data: 0.7
                    }],
                    booleans: vec![BooleanParam {
                        param_id: ParamId::CustomBoolean5,
                        data: true
                    }],
                    vectors: vec![
                        Vector4Param {
                            param_id: ParamId::CustomVector0,
                            data: Vector4 {
                                x: 1.0,
                                y: 2.0,
                                z: 3.0,
                                w: 4.0
                            }
                        },
                        Vector4Param {
                            param_id: ParamId::CustomVector8,
                            data: Vector4 {
                                x: 1.0,
                                y: 1.0,
                                z: 1.0,
                                w: 1.0
                            }
                        }
                    ],
                    rasterizer_states: vec![RasterizerStateParam {
                        param_id: ParamId::RasterizerState0,
                        data: RasterizerStateData::default()
                    }],
                    samplers: vec![SamplerParam {
                        param_id: ParamId::Sampler1,
                        data: SamplerData::default()
                    }],
                    textures: vec![TextureParam {
                        param_id: ParamId::Texture1,
                        data: String::new()
                    }],
                },
                &ShaderDatabase::from_iter(
                    [(
                        "SFX_PBS_010002000800824f".to_owned(),
                        // Check that channels are parsed properly.
                        ShaderProgram {
                            discard: true,
                            premultiplied: false,
                            receives_shadow: false,
                            lighting: false,
                            sh: false,
                            vertex_attributes: Vec::new(),
                            material_parameters: vec![
                                "Texture0".to_owned(),
                                "CustomBoolean1".to_owned(),
                                "CustomFloat2".to_owned(),
                                "CustomVector8.xw".to_owned()
                            ],
                            complexity: 0.0
                        }
                    )]
                    .into_iter()
                )
            )
        );
    }
}
