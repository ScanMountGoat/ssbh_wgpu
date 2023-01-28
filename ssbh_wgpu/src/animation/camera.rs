use super::{frame_values, interpolate_f32, interpolate_transform};
use crate::CameraTransforms;
use ssbh_data::anim_data::{AnimData, GroupType, TrackValues};

/// Calculate the camera transform from the tracks in `anim` at the given `frame`.
pub fn animate_camera(
    anim: &AnimData,
    frame: f32,
    aspect_ratio: f32,
    screen_dimensions: glam::Vec4,
    default_fov: f32,
    default_near_clip: f32,
    default_far_clip: f32,
) -> Option<CameraTransforms> {
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
        TrackValues::Transform(values) => {
            let (current, next, factor) = frame_values(frame, values);
            Some(interpolate_transform(current, next, factor))
        }
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
            TrackValues::Float(values) => {
                let (current, next, factor) = frame_values(frame, values);
                Some(interpolate_f32(*current, *next, factor))
            }
            _ => None,
        })
        .unwrap_or(default_near_clip);

    let far_clip = camera_node
        .and_then(|node| node.tracks.iter().find(|t| t.name == "FarClip"))
        .and_then(|track| match &track.values {
            TrackValues::Float(values) => {
                let (current, next, factor) = frame_values(frame, values);
                Some(interpolate_f32(*current, *next, factor))
            }
            _ => None,
        })
        .unwrap_or(default_far_clip);

    let fov = camera_node
        .and_then(|node| node.tracks.iter().find(|t| t.name == "FieldOfView"))
        .and_then(|track| match &track.values {
            TrackValues::Float(values) => {
                let (current, next, factor) = frame_values(frame, values);
                Some(interpolate_f32(*current, *next, factor))
            }
            _ => None,
        })
        .unwrap_or(default_fov);

    // TODO: Why do we negate the translation?
    let translation = glam::Mat4::from_translation(-transform.translation);
    // TODO: Why do we negate w like for lighting rotations?
    let rotation = glam::Mat4::from_quat(glam::quat(
        transform.rotation.x,
        transform.rotation.y,
        transform.rotation.z,
        -transform.rotation.w,
    ));
    let scale = glam::Mat4::from_scale(transform.scale);

    let model_view_matrix = rotation * translation * scale;

    let perspective_matrix = glam::Mat4::perspective_rh(fov, aspect_ratio, near_clip, far_clip);
    let mvp_matrix = perspective_matrix * model_view_matrix;

    let camera_pos = model_view_matrix.inverse().col(3);

    Some(CameraTransforms {
        model_view_matrix,
        mvp_matrix,
        mvp_inv_matrix: mvp_matrix.inverse(),
        camera_pos,
        screen_dimensions,
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

        let screen_dimensions = glam::vec4(1920.0, 1080.0, 1.0, 0.0);
        let transform =
            animate_camera(&anim, 0.0, 1.0, screen_dimensions, 0.5, 0.1, 1000.0).unwrap();

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
