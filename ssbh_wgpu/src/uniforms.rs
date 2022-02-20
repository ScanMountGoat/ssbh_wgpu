use ssbh_data::matl_data::{MatlEntryData, ParamId};
use wgpu::util::DeviceExt;

pub fn create_uniforms_buffer(
    material: Option<&MatlEntryData>,
    device: &wgpu::Device,
) -> wgpu::Buffer {
    let uniforms = material
        .map(|material| {
            let mut custom_vector = [glam::Vec4::ZERO; 64];
            for vector in &material.vectors {
                // TODO: Add this to ssbh_lib?
                let index = match vector.param_id {
                    ParamId::CustomVector0 => 0,
                    ParamId::CustomVector1 => 1,
                    ParamId::CustomVector2 => 2,
                    ParamId::CustomVector3 => 3,
                    ParamId::CustomVector4 => 4,
                    ParamId::CustomVector5 => 5,
                    ParamId::CustomVector6 => 6,
                    ParamId::CustomVector7 => 7,
                    ParamId::CustomVector8 => 8,
                    ParamId::CustomVector9 => 9,
                    ParamId::CustomVector10 => 10,
                    ParamId::CustomVector11 => 11,
                    ParamId::CustomVector12 => 12,
                    ParamId::CustomVector13 => 13,
                    ParamId::CustomVector14 => 14,
                    ParamId::CustomVector15 => 15,
                    ParamId::CustomVector16 => 16,
                    ParamId::CustomVector17 => 17,
                    ParamId::CustomVector18 => 18,
                    ParamId::CustomVector19 => 19,
                    ParamId::CustomVector20 => 20,
                    ParamId::CustomVector21 => 21,
                    ParamId::CustomVector22 => 22,
                    ParamId::CustomVector23 => 23,
                    ParamId::CustomVector24 => 24,
                    ParamId::CustomVector25 => 25,
                    ParamId::CustomVector26 => 26,
                    ParamId::CustomVector27 => 27,
                    ParamId::CustomVector28 => 28,
                    ParamId::CustomVector29 => 29,
                    ParamId::CustomVector30 => 30,
                    ParamId::CustomVector31 => 31,
                    ParamId::CustomVector32 => 32,
                    ParamId::CustomVector33 => 33,
                    ParamId::CustomVector34 => 34,
                    ParamId::CustomVector35 => 35,
                    ParamId::CustomVector36 => 36,
                    ParamId::CustomVector37 => 37,
                    ParamId::CustomVector38 => 38,
                    ParamId::CustomVector39 => 39,
                    ParamId::CustomVector40 => 40,
                    ParamId::CustomVector41 => 41,
                    ParamId::CustomVector42 => 42,
                    ParamId::CustomVector43 => 43,
                    ParamId::CustomVector44 => 44,
                    ParamId::CustomVector45 => 45,
                    ParamId::CustomVector46 => 46,
                    ParamId::CustomVector47 => 47,
                    ParamId::CustomVector48 => 48,
                    ParamId::CustomVector49 => 49,
                    ParamId::CustomVector50 => 50,
                    ParamId::CustomVector51 => 51,
                    ParamId::CustomVector52 => 52,
                    ParamId::CustomVector53 => 53,
                    ParamId::CustomVector54 => 54,
                    ParamId::CustomVector55 => 55,
                    ParamId::CustomVector56 => 56,
                    ParamId::CustomVector57 => 57,
                    ParamId::CustomVector58 => 58,
                    ParamId::CustomVector59 => 59,
                    ParamId::CustomVector60 => 60,
                    ParamId::CustomVector61 => 61,
                    ParamId::CustomVector62 => 62,
                    ParamId::CustomVector63 => 63,
                    _ => panic!("Unsupported vector param ID"),
                };

                custom_vector[index] = vector.data.to_array().into();
            }

            let mut custom_float = [glam::Vec4::ZERO; 20];
            for float in &material.floats {
                // TODO: Add this to ssbh_lib?
                let index = match float.param_id {
                    ParamId::CustomFloat0 => 0,
                    ParamId::CustomFloat1 => 1,
                    ParamId::CustomFloat2 => 2,
                    ParamId::CustomFloat3 => 3,
                    ParamId::CustomFloat4 => 4,
                    ParamId::CustomFloat5 => 5,
                    ParamId::CustomFloat6 => 6,
                    ParamId::CustomFloat7 => 7,
                    ParamId::CustomFloat8 => 8,
                    ParamId::CustomFloat9 => 9,
                    ParamId::CustomFloat10 => 10,
                    ParamId::CustomFloat11 => 11,
                    ParamId::CustomFloat12 => 12,
                    ParamId::CustomFloat13 => 13,
                    ParamId::CustomFloat14 => 14,
                    ParamId::CustomFloat15 => 15,
                    ParamId::CustomFloat16 => 16,
                    ParamId::CustomFloat17 => 17,
                    ParamId::CustomFloat18 => 18,
                    ParamId::CustomFloat19 => 19,
                    _ => panic!("Unsupported float param ID"),
                };

                custom_float[index].x = float.data;
            }

            let mut custom_boolean = [glam::Vec4::ZERO; 20];
            for boolean in &material.booleans {
                // TODO: Add this to ssbh_lib?
                let index = match boolean.param_id {
                    ParamId::CustomBoolean0 => 0,
                    ParamId::CustomBoolean1 => 1,
                    ParamId::CustomBoolean2 => 2,
                    ParamId::CustomBoolean3 => 3,
                    ParamId::CustomBoolean4 => 4,
                    ParamId::CustomBoolean5 => 5,
                    ParamId::CustomBoolean6 => 6,
                    ParamId::CustomBoolean7 => 7,
                    ParamId::CustomBoolean8 => 8,
                    ParamId::CustomBoolean9 => 9,
                    ParamId::CustomBoolean10 => 10,
                    ParamId::CustomBoolean11 => 11,
                    ParamId::CustomBoolean12 => 12,
                    ParamId::CustomBoolean13 => 13,
                    ParamId::CustomBoolean14 => 14,
                    ParamId::CustomBoolean15 => 15,
                    ParamId::CustomBoolean16 => 16,
                    ParamId::CustomBoolean17 => 17,
                    ParamId::CustomBoolean18 => 18,
                    ParamId::CustomBoolean19 => 19,
                    _ => panic!("Unsupported boolean param ID"),
                };

                custom_boolean[index].x = if boolean.data { 1.0 } else { 0.0 };
            }

            crate::shader::model::bind_groups::MaterialUniforms {
                custom_vector,
                custom_boolean,
                custom_float,
            }
        })
        .unwrap_or(
            // Missing values are always set to zero.
            crate::shader::model::bind_groups::MaterialUniforms {
                custom_vector: [glam::Vec4::ZERO; 64],
                custom_boolean: [glam::Vec4::ZERO; 20],
                custom_float: [glam::Vec4::ZERO; 20],
            },
        );

    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Transforms Buffer"),
        contents: bytemuck::cast_slice(&[uniforms]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    })
}
