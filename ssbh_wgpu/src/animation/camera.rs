use super::{frame_value, AnimTransform};
use crate::CameraTransforms;
use glam::{vec4, Mat4, Quat, Vec3};
use ssbh_data::anim_data::{AnimData, GroupType, TrackValues};

pub struct CameraAnimValues {
    pub scale: Vec3,
    pub rotation: Quat,
    pub translation: Vec3,
    pub fov_y_radians: f32,
    pub near_clip: f32,
    pub far_clip: f32,
}

impl CameraAnimValues {
    /// Convert the animation values into the format expected by [SsbhRenderer](crate::SsbhRenderer).
    pub fn to_transforms(&self, width: u32, height: u32, scale_factor: f64) -> CameraTransforms {
        let translation = Mat4::from_translation(self.translation);
        let rotation = Mat4::from_quat(self.rotation);
        let scale = Mat4::from_scale(self.scale);

        // A slightly unusual transform order of RTS instead of TRS.
        let model_view_matrix = rotation * translation * scale;

        let aspect_ratio = width as f32 / height as f32;

        let perspective_matrix = Mat4::perspective_rh(
            self.fov_y_radians,
            aspect_ratio,
            self.near_clip,
            self.far_clip,
        );
        let mvp_matrix = perspective_matrix * model_view_matrix;

        let camera_pos = model_view_matrix.inverse().col(3);

        let screen_dimensions = vec4(width as f32, height as f32, scale_factor as f32, 0.0);

        CameraTransforms {
            model_view_matrix,
            mvp_matrix,
            mvp_inv_matrix: mvp_matrix.inverse(),
            camera_pos,
            screen_dimensions,
        }
    }
}

/// Calculate the camera transform from the tracks in `anim` at the given `frame`.
///
/// If a value is not present in the anim, the provided default values are used.
pub fn animate_camera(
    anim: &AnimData,
    frame: f32,
    default_fov: f32,
    default_near_clip: f32,
    default_far_clip: f32,
) -> Option<CameraAnimValues> {
    // TODO: Do all camera animations have this structure?
    // TODO: Are all these values required?
    let transform_node = anim
        .groups
        .iter()
        .find(|g| g.group_type == GroupType::Transform)?
        .nodes
        .iter()
        .find(|n| n.name == "gya_camera" || n.name == "camera_stage")?;

    let transform_track = transform_node.tracks.first()?;

    let transform = match &transform_track.values {
        TrackValues::Transform(values) => Some(AnimTransform::from(frame_value(values, frame))),
        _ => None,
    }?;

    // TODO: What happens with animations that don't include this node?
    let camera_node = anim
        .groups
        .iter()
        .find(|g| g.group_type == GroupType::Camera)
        .and_then(|group| {
            group
                .nodes
                .iter()
                .find(|n| n.name == "gya_cameraShape" || n.name == "camera_stageShape")
        });

    let near_clip = camera_node
        .and_then(|node| node.tracks.iter().find(|t| t.name == "NearClip"))
        .and_then(|track| match &track.values {
            TrackValues::Float(values) => Some(frame_value(values, frame)),
            _ => None,
        })
        .unwrap_or(default_near_clip);

    let far_clip = camera_node
        .and_then(|node| node.tracks.iter().find(|t| t.name == "FarClip"))
        .and_then(|track| match &track.values {
            TrackValues::Float(values) => Some(frame_value(values, frame)),
            _ => None,
        })
        .unwrap_or(default_far_clip);

    let fov_y_radians = camera_node
        .and_then(|node| node.tracks.iter().find(|t| t.name == "FieldOfView"))
        .and_then(|track| match &track.values {
            TrackValues::Float(values) => Some(frame_value(values, frame)),
            _ => None,
        })
        .unwrap_or(default_fov);

    let scale = transform.scale;

    // TODO: Why do we negate w like for lighting rotations?
    let rotation = transform.rotation.conjugate();

    // TODO: Why do we negate the translation?
    let translation = -transform.translation;

    Some(CameraAnimValues {
        scale,
        rotation,
        translation,
        fov_y_radians,
        near_clip,
        far_clip,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::assert_matrix_relative_eq;
    use ssbh_data::{
        anim_data::{GroupData, NodeData, TrackData, Transform, TransformFlags},
        Vector3, Vector4,
    };

    // TODO: Test missing how missing data and capitalization is handled in game.
    // TODO: Test if transform scale has any impact in game.
    // TODO: Test handling of stage and camera folder structures and missing nodes.

    #[test]
    fn test() {
        // Tested on /camera/fighter/mario/c00/j00win1.nuanmb using a single frame.
        let anim = AnimData {
            major_version: 2,
            minor_version: 0,
            final_frame_index: 0.0,
            groups: vec![
                GroupData {
                    group_type: GroupType::Transform,
                    nodes: vec![NodeData {
                        name: "gya_camera".to_owned(),
                        tracks: vec![TrackData {
                            name: "Transform".to_owned(),
                            compensate_scale: false,
                            transform_flags: TransformFlags::default(),
                            values: TrackValues::Transform(vec![Transform {
                                scale: Vector3::new(1.0, 1.0, 1.0),
                                rotation: Vector4::new(0.0, 0.7071, 0.0, 0.7071),
                                translation: Vector3::new(5.0, 10.0, -70.0),
                            }]),
                        }],
                    }],
                },
                GroupData {
                    group_type: GroupType::Camera,
                    nodes: vec![NodeData {
                        name: "gya_cameraShape".to_owned(),
                        tracks: vec![
                            TrackData {
                                name: "FarClip".to_owned(),
                                compensate_scale: false,
                                transform_flags: TransformFlags::default(),
                                values: TrackValues::Float(vec![100000.0]),
                            },
                            TrackData {
                                name: "FieldOfView".to_owned(),
                                compensate_scale: false,
                                transform_flags: TransformFlags::default(),
                                values: TrackValues::Float(vec![0.5]),
                            },
                            TrackData {
                                name: "NearClip".to_owned(),
                                compensate_scale: false,
                                transform_flags: TransformFlags::default(),
                                values: TrackValues::Float(vec![1.0]),
                            },
                        ],
                    }],
                },
            ],
        };

        let transform = animate_camera(&anim, 0.0, 0.5, 0.1, 1000.0)
            .unwrap()
            .to_transforms(128, 128, 1.0);

        // The matrix output after comparing the viewport with in game screenshots.
        // TODO: Investigate why this works differently than skel transforms.
        assert_matrix_relative_eq!(
            [
                [0.0, 0.0, -1.0, -1.0],
                [0.0, 3.9163172, 0.0, 0.0],
                [-3.9163177, 0.0, 0.0, 0.0],
                [-274.14224, -39.163174, 4.0000486, 5.0000086],
            ],
            transform.mvp_matrix.to_cols_array_2d()
        );
    }
}
