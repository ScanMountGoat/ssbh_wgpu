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

pub fn calculate_light_transform(rotation: Quat, scale: Vec3) -> Mat4 {
    // TODO: This should be editable when changing stages.
    // TODO: Why do we negate w?
    // TODO: Read this value from the transform for LightStg0 from light00_set.nuanmb.
    // TODO: Do translation and scale matter?

    // TODO: What controls the "scale" of the lighting region?
    let perspective_matrix =
        Mat4::orthographic_rh(-scale.x, scale.x, -scale.y, scale.y, -scale.z, scale.z);
    let model_view = Mat4::from_quat(rotation);

    perspective_matrix * model_view
}

pub fn light_direction(rotation: Quat) -> Vec4 {
    Mat4::from_quat(rotation) * Vec4::Z
}

impl StageUniforms {
    pub fn training() -> Self {
        let custom_boolean = [[0; 4]; 20];
        let custom_vector = [[0.0; 4]; 64];
        let custom_float = [[0.0; 4]; 20];

        // TODO: Set the scene attributes from the training nuanmb.
        Self {
            light_chr: Light {
                color: [4.0; 4],
                direction: light_direction(glam::Quat::from_xyzw(
                    -0.453154, -0.365998, -0.211309, 0.784886,
                ))
                .to_array(),
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
            color: [0.0; 4],
            direction: [0.0; 4],
        }
    }
}

impl Default for SceneAttributesForShaderFx {
    fn default() -> Self {
        Self {
            custom_boolean: [[0; 4]; 20],
            custom_vector: [[0.0; 4]; 64],
            custom_float: [[0.0; 4]; 20],
        }
    }
}

impl From<&AnimData> for StageUniforms {
    fn from(data: &AnimData) -> Self {
        let transform_group = data
            .groups
            .iter()
            .find(|g| g.group_type == GroupType::Transform);

        let light_chr = transform_group.and_then(|g| g.nodes.iter().find(|n| n.name == "LightChr"));
        let light_chr = light_chr.map(light_node).unwrap_or_default();

        // TODO: Take the current frame for animation?
        let scene_attributes = transform_group.and_then(|g| {
            g.nodes
                .iter()
                .find(|n| n.name == "sceneAttributesForShaderFX")
        });
        let scene_attributes = scene_attributes
            .map(|node| scene_attributes_node(node))
            .unwrap_or_default();

        Self {
            light_chr,
            scene_attributes,
        }
    }
}

fn scene_attributes_node(node: &NodeData) -> SceneAttributesForShaderFx {
    // TODO: Interpolate vectors?
    let mut attributes = SceneAttributesForShaderFx::default();

    for track in &node.tracks {
        // Assign material parameters based on the parameter ID.
        // Stage parameters use the matl names despite have different functions.
        if let Ok(param) = ParamId::from_str(&track.name) {
            match &track.values {
                TrackValues::Float(v) => {
                    if let Some(index) = float_index(param) {
                        attributes.custom_float[index][0] = v[0];
                    }
                }
                TrackValues::Boolean(v) => {
                    if let Some(index) = boolean_index(param) {
                        attributes.custom_boolean[index][0] = if v[0] { 1 } else { 0 };
                    }
                }
                TrackValues::Vector4(v) => {
                    if let Some(index) = vector_index(param) {
                        attributes.custom_vector[index] = v[0].to_array();
                    }
                }
                _ => (),
            }
        }
    }

    attributes
}

fn light_node(node: &NodeData) -> Light {
    // TODO: Avoid unwrap.
    // TODO: Default to intensity of 1.0 instead?
    let float0 = node
        .tracks
        .iter()
        .find(|t| t.name == "CustomFloat0")
        .and_then(|t| match &t.values {
            TrackValues::Float(v) => Some(v[0]),
            _ => None,
        })
        .unwrap_or_default();

    let vector0 = node
        .tracks
        .iter()
        .find(|t| t.name == "CustomVector0")
        .and_then(|t| match &t.values {
            TrackValues::Vector4(v) => Some(v[0]),
            _ => None,
        })
        .unwrap_or_default();

    // TODO: Does translation and scale matter?
    let rotation = node
        .tracks
        .iter()
        .find(|t| t.name == "Transform")
        .and_then(|t| match &t.values {
            TrackValues::Transform(v) => Some(v[0]),
            _ => None,
        })
        .map(|t| Quat::from_array(t.rotation.to_array()))
        .unwrap_or(Quat::IDENTITY);

    Light {
        color: [
            vector0.x * float0,
            vector0.y * float0,
            vector0.z * float0,
            vector0.w * float0,
        ],
        direction: light_direction(rotation).to_array(),
    }
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;

    use super::*;

    use crate::assert_matrix_relative_eq;

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
                [0.0, 0.0, -1.0, 0.0],
                [0.0, -0.5, 0.0, 0.5],
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
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.5, 0.0, 0.0, 0.5],
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
                [0.0, -1.0, 0.0, 0.0],
                [1.0, 0.0, 0.0, 0.0],
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

    #[test]
    fn light_direction_light_chr_training() {
        let dir = light_direction(Quat::from_xyzw(-0.453154, -0.365998, -0.211309, 0.784886));
        assert_relative_eq!(-0.38302213, dir.x, epsilon = 0.0001f32);
        assert_relative_eq!(0.86602527, dir.y, epsilon = 0.0001f32);
        assert_relative_eq!(0.32139426, dir.z, epsilon = 0.0001f32);
        assert_eq!(0.0, dir.w);
    }
}
