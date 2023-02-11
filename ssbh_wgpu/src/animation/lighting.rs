use std::str::FromStr;

use glam::{Mat4, Quat, Vec3, Vec4};
use ssbh_data::{
    anim_data::{AnimData, GroupType, NodeData, TrackValues},
    matl_data::ParamId,
};

use crate::{
    shader::model::{Light, SceneAttributesForShaderFx, StageUniforms},
    uniforms::{boolean_index, float_index, vector_index},
};

use super::frame_value;

pub fn calculate_light_transform(rotation: Quat, scale: Vec3) -> Mat4 {
    // TODO: Why do we negate w?
    // TODO: Do translation and scale matter?
    // TODO: What controls the "scale" of the lighting region?
    let perspective_matrix =
        Mat4::orthographic_rh(-scale.x, scale.x, -scale.y, scale.y, -scale.z, scale.z);
    let model_view = Mat4::from_quat(Quat::from_xyzw(
        rotation.x,
        rotation.y,
        rotation.z,
        -rotation.w,
    ));

    perspective_matrix * model_view
}

pub fn light_direction(rotation: Quat) -> Vec4 {
    Mat4::from_quat(rotation) * Vec4::Z
}

impl StageUniforms {
    pub fn training() -> Self {
        let custom_boolean = [glam::UVec4::ZERO; 20];
        let mut custom_vector = [glam::Vec4::ZERO; 64];
        custom_vector[8] = glam::Vec4::ONE;
        let custom_float = [glam::Vec4::ZERO; 20];

        // TODO: Set the scene attributes from the training nuanmb.
        Self {
            light_chr: Light {
                color: glam::Vec4::splat(4.0),
                direction: light_direction(glam::Quat::from_xyzw(
                    -0.453154, -0.365998, -0.211309, 0.784886,
                )),
            },
            scene_attributes: SceneAttributesForShaderFx {
                custom_boolean,
                custom_vector,
                custom_float,
            },
        }
    }
}

impl Default for Light {
    fn default() -> Self {
        Self {
            color: glam::Vec4::ZERO,
            direction: glam::Vec4::ZERO,
        }
    }
}

impl Default for SceneAttributesForShaderFx {
    fn default() -> Self {
        Self {
            custom_boolean: [glam::UVec4::ZERO; 20],
            custom_vector: [glam::Vec4::ZERO; 64],
            custom_float: [glam::Vec4::ZERO; 20],
        }
    }
}

// TODO: Test cases.
pub fn animate_lighting(data: &AnimData, frame: f32) -> (StageUniforms, glam::Mat4) {
    let transform_group = data
        .groups
        .iter()
        .find(|g| g.group_type == GroupType::Transform);

    // TODO: use LightStg0 for the shadow direction?
    let light_chr = transform_group.and_then(|g| g.nodes.iter().find(|n| n.name == "LightChr"));
    let (light_chr, light_chr_rotation) =
        light_chr.map(|n| light_node(n, frame)).unwrap_or_default();

    // TODO: Take the current frame for animation?
    let scene_attributes = transform_group.and_then(|g| {
        g.nodes
            .iter()
            .find(|n| n.name == "sceneAttributesForShaderFX")
    });
    let scene_attributes = scene_attributes
        .map(|n| scene_attributes_node(n, frame))
        .unwrap_or_default();

    // TODO: What to use for the scale?
    let light_transform =
        calculate_light_transform(light_chr_rotation, glam::vec3(25.0, 25.0, 25.0));

    (
        StageUniforms {
            light_chr,
            scene_attributes,
        },
        light_transform,
    )
}

fn scene_attributes_node(node: &NodeData, frame: f32) -> SceneAttributesForShaderFx {
    // TODO: Interpolate vectors?
    let mut attributes = SceneAttributesForShaderFx::default();

    for track in &node.tracks {
        // Assign material parameters based on the parameter ID.
        // Stage parameters use the matl names despite have different functions.
        if let Ok(param) = ParamId::from_str(&track.name) {
            match &track.values {
                TrackValues::Float(values) => {
                    if let Some(index) = float_index(param) {
                        attributes.custom_float[index][0] = frame_value(values, frame);
                    }
                }
                TrackValues::Boolean(values) => {
                    if let Some(index) = boolean_index(param) {
                        attributes.custom_boolean[index][0] = frame_value(values, frame) as u32;
                    }
                }
                TrackValues::Vector4(values) => {
                    if let Some(index) = vector_index(param) {
                        attributes.custom_vector[index] =
                            frame_value(values, frame).to_array().into();
                    }
                }
                _ => (),
            }
        }
    }

    attributes
}

fn light_node(node: &NodeData, frame: f32) -> (Light, glam::Quat) {
    // TODO: Avoid unwrap.
    // TODO: Default to intensity of 1.0 instead?
    let float0 = node
        .tracks
        .iter()
        .find(|t| t.name == "CustomFloat0")
        .and_then(|t| match &t.values {
            TrackValues::Float(values) => Some(frame_value(values, frame)),
            _ => None,
        })
        .unwrap_or_default();

    let vector0 = node
        .tracks
        .iter()
        .find(|t| t.name == "CustomVector0")
        .and_then(|t| match &t.values {
            TrackValues::Vector4(values) => Some(frame_value(values, frame)),
            _ => None,
        })
        .unwrap_or_default();

    // TODO: Does translation and scale matter?
    // TODO: Make an interpolate function for TrackValues?
    let rotation = node
        .tracks
        .iter()
        .find(|t| t.name == "Transform")
        .and_then(|t| match &t.values {
            TrackValues::Transform(values) => Some(frame_value(values, frame)),
            _ => None,
        })
        .map(|t| Quat::from_array(t.rotation.to_array()))
        .unwrap_or(Quat::IDENTITY);

    // TODO: Return an intermediate type instead?
    (
        Light {
            color: glam::Vec4::from_array(vector0.to_array()) * float0,
            direction: light_direction(rotation),
        },
        rotation,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::assert_matrix_relative_eq;
    use approx::assert_relative_eq;

    // Test cases based on matching the variance shadow map from in game.
    // The LightStg0 rotation changes the fighter shadow direction.
    #[test]
    fn rotation_zero() {
        assert_matrix_relative_eq!(
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, -0.5, 0.5],
                [0.0, 0.0, 0.0, 1.0],
            ],
            calculate_light_transform(
                Quat::from_xyzw(0.0, 0.0, 0.0, 1.0),
                Vec3::new(1.0, 1.0, 1.0)
            )
            .transpose()
            .to_cols_array_2d()
        )
    }

    #[test]
    fn rotation_x_90_degrees() {
        assert_matrix_relative_eq!(
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.5, 0.0, 0.5],
                [0.0, 0.0, 0.0, 1.0],
            ],
            calculate_light_transform(
                Quat::from_xyzw(1.0, 0.0, 0.0, 1.0).normalize(),
                Vec3::new(1.0, 1.0, 1.0)
            )
            .transpose()
            .to_cols_array_2d()
        )
    }

    #[test]
    fn rotation_y_90_degrees() {
        assert_matrix_relative_eq!(
            [
                [0.0, 0.0, -1.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [-0.5, 0.0, 0.0, 0.5],
                [0.0, 0.0, 0.0, 1.0],
            ],
            calculate_light_transform(
                Quat::from_xyzw(0.0, 1.0, 0.0, 1.0).normalize(),
                Vec3::new(1.0, 1.0, 1.0)
            )
            .transpose()
            .to_cols_array_2d()
        )
    }

    #[test]
    fn rotation_z_90_degrees() {
        assert_matrix_relative_eq!(
            [
                [0.0, 1.0, 0.0, 0.0],
                [-1.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, -0.5, 0.5],
                [0.0, 0.0, 0.0, 1.0],
            ],
            calculate_light_transform(
                Quat::from_xyzw(0.0, 0.0, 1.0, 1.0).normalize(),
                Vec3::new(1.0, 1.0, 1.0)
            )
            .transpose()
            .to_cols_array_2d()
        )
    }

    // Test cases based on the direction vector from in game uniform buffers.
    // TODO: Add additional test cases from more stages.
    #[test]
    fn light_direction_light_chr_training() {
        let dir = light_direction(Quat::from_xyzw(-0.453154, -0.365998, -0.211309, 0.784886));
        assert_relative_eq!(-0.38302213, dir.x, epsilon = 0.0001f32);
        assert_relative_eq!(0.86602527, dir.y, epsilon = 0.0001f32);
        assert_relative_eq!(0.32139426, dir.z, epsilon = 0.0001f32);
        assert_eq!(0.0, dir.w);
    }
}
