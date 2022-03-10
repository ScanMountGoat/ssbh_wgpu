use ssbh_data::matl_data::*;
use wgpu::util::DeviceExt;

use crate::shader::model::MaterialUniforms;

pub fn create_uniforms_buffer(
    material: Option<&MatlEntryData>,
    device: &wgpu::Device,
) -> wgpu::Buffer {
    let uniforms = create_uniforms(material);

    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Material Uniforms Buffer"),
        contents: bytemuck::cast_slice(&[uniforms]),
        // COPY_DST allows applying animations without allocating new buffers
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    })
}

fn create_uniforms(material: Option<&MatlEntryData>) -> MaterialUniforms {
    material
        .map(|material| {
            // Ignore invalid parameters for now to avoid an error or panic.
            let mut custom_vector = [[0.0; 4]; 64];
            let mut has_vector = [[0.0; 4]; 64];
            for vector in &material.vectors {
                if let Some(index) = vector_index(vector) {
                    custom_vector[index] = vector.data.to_array();
                    has_vector[index][0] = 1.0;
                }
            }

            let mut custom_float = [[0.0; 4]; 20];
            let mut has_float = [[0.0; 4]; 20];
            for float in &material.floats {
                if let Some(index) = float_index(float) {
                    custom_float[index][0] = float.data;
                    has_float[index][0] = 1.0;
                }
            }

            let mut custom_boolean = [[0.0; 4]; 20];
            for boolean in &material.booleans {
                if let Some(index) = boolean_index(boolean) {
                    custom_boolean[index][0] = if boolean.data { 1.0 } else { 0.0 };
                }
            }

            let mut has_texture = [[0.0; 4]; 19];
            for texture in &material.textures {
                if let Some(index) = texture_index(texture) {
                    has_texture[index][0] = 1.0;
                }
            }

            MaterialUniforms {
                custom_vector,
                custom_boolean,
                custom_float,
                has_texture,
                has_float,
                has_vector,
            }
        })
        .unwrap_or(
            // Missing values are always set to zero.
            MaterialUniforms {
                custom_vector: [[0.0; 4]; 64],
                custom_boolean: [[0.0; 4]; 20],
                custom_float: [[0.0; 4]; 20],
                has_texture: [[0.0; 4]; 19],
                has_float: [[0.0; 4]; 20],
                has_vector: [[0.0; 4]; 64],
            },
        )
}

// TODO: Return an Option instead?
fn vector_index(vector: &Vector4Param) -> Option<usize> {
    match vector.param_id {
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

fn float_index(float: &FloatParam) -> Option<usize> {
    match float.param_id {
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

fn texture_index(texture: &TextureParam) -> Option<usize> {
    match texture.param_id {
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

fn boolean_index(boolean: &BooleanParam) -> Option<usize> {
    match boolean.param_id {
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
    use super::*;


    // TODO: Test has_value, values, etc

    #[test]
    fn create_uniforms_no_material() {
        assert_eq!(
            MaterialUniforms {
                custom_vector: [[0.0; 4]; 64],
                custom_boolean: [[0.0; 4]; 20],
                custom_float: [[0.0; 4]; 20],
                has_float: [[0.0; 4]; 20],
                has_texture: [[0.0; 4]; 19],
                has_vector: [[0.0; 4]; 64]
            },
            create_uniforms(None)
        );
    }

    #[test]
    fn create_uniforms_empty_material() {
        assert_eq!(
            MaterialUniforms {
                custom_vector: [[0.0; 4]; 64],
                custom_boolean: [[0.0; 4]; 20],
                custom_float: [[0.0; 4]; 20],
                has_float: [[0.0; 4]; 20],
                has_texture: [[0.0; 4]; 19],
                has_vector: [[0.0; 4]; 64]
            },
            create_uniforms(Some(&MatlEntryData {
                material_label: String::new(),
                shader_label: String::new(),
                blend_states: Vec::new(),
                floats: Vec::new(),
                booleans: Vec::new(),
                vectors: Vec::new(),
                rasterizer_states: Vec::new(),
                samplers: Vec::new(),
                textures: Vec::new()
            }))
        );
    }

    #[test]
    fn create_uniforms_invalid_parameter_indices() {
        // TODO: How is this handled in game?
        // Just ignore an invalid ParamId.
        assert_eq!(
            MaterialUniforms {
                custom_vector: [[0.0; 4]; 64],
                custom_boolean: [[0.0; 4]; 20],
                custom_float: [[0.0; 4]; 20],
                has_float: [[0.0; 4]; 20],
                has_texture: [[0.0; 4]; 19],
                has_vector: [[0.0; 4]; 64]
            },
            create_uniforms(Some(&MatlEntryData {
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
            }))
        );
    }
}
