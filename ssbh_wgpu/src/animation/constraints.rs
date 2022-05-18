use super::{AnimTransform, AnimatedBone};
use ssbh_lib::formats::hlpb::Hlpb;

fn interp(a: f32, b: f32, f: f32) -> f32 {
    (1.0 - f) * a + f * b
}

pub fn apply_hlpb_constraints(animated_bones: &mut [AnimatedBone], hlpb: &Hlpb) {
    // TODO: Rename the orient constraint to include rotation in the name?
    match hlpb {
        Hlpb::V11 {
            aim_constraints,
            orient_constraints,
            ..
        } => {
            // TODO: Aim constraints?

            // Sort the constraints so that a bone's parents are evaluated first.
            // TODO: Also make sure dependencies are evaluated first?
            // TODO: Find a cleaner way to do this.
            // TODO: Optimize animation by presorting bones?
            let mut orient_constraints_sorted = orient_constraints.elements.clone();
            orient_constraints_sorted.sort_by(|a, b| {
                let a_bone = animated_bones
                    .iter()
                    .find(|b| b.bone.name == a.driver_bone_name.to_string_lossy());
                if let Some(bone) = a_bone {
                    if let Some(index) = bone.bone.parent_index {
                        if *animated_bones[index].bone.name == b.driver_bone_name.to_string_lossy()
                        {
                            return std::cmp::Ordering::Greater;
                        }
                    }
                }
                std::cmp::Ordering::Less
            });

            for orient in orient_constraints_sorted {
                apply_orient_constraint(animated_bones, orient);
            }
        }
    }
}

fn apply_orient_constraint(
    animated_bones: &mut [AnimatedBone],
    orient: ssbh_lib::formats::hlpb::OrientConstraint,
) {
    // TODO: Investigate the remaining bone name fields.
    let source = animated_bones
        .iter()
        .find(|b| b.bone.name == orient.parent_bone_name.to_string_lossy())
        .cloned();

    if let Some(source) = source {
        let quat1 = glam::Quat::from_array(orient.quat1.to_array());
        let quat2 = glam::Quat::from_array(orient.quat2.to_array());

        for target_bone in animated_bones
            .iter_mut()
            .filter(|b| b.bone.name == orient.driver_bone_name.to_string_lossy())
        {
            // TODO: Can a bone not affected by the anim be the source?
            // TODO: Will the target of a constraint ever be animated?
            let mut target_transform = target_bone
                .anim_transform
                .unwrap_or(AnimTransform::from_bone(&target_bone.bone));

            if let Some(source_transform) = source.anim_transform {
                // TODO: Do the unk types matter?

                // TODO: quat1 and quat2 correct for twists?
                let (target_rot_x, target_rot_y, target_rot_z) =
                    target_transform.rotation.to_euler(glam::EulerRot::XYZ);

                let (source_rot_x, source_rot_y, source_rot_z) =
                    source_transform.rotation.to_euler(glam::EulerRot::XYZ);

                target_transform.rotation = glam::Quat::from_euler(
                    glam::EulerRot::XYZ,
                    interp(target_rot_x, source_rot_x, orient.constraint_axes.x),
                    interp(target_rot_y, source_rot_y, orient.constraint_axes.y),
                    interp(target_rot_z, source_rot_z, orient.constraint_axes.z),
                );
            }

            target_bone.anim_transform = Some(target_transform);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_vector_relative_eq;
    use ssbh_data::{
        anim_data::{TransformFlags, Vector3, Vector4},
        skel_data::{BillboardType, BoneData},
    };
    use ssbh_lib::formats::hlpb::OrientConstraint;

    fn identity_bone(name: &str, parent_index: Option<usize>) -> BoneData {
        BoneData {
            name: name.to_string(),
            // Start with the identity to make this simpler.
            transform: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            parent_index,
            billboard_type: BillboardType::Disabled,
        }
    }

    #[test]
    fn apply_single_orient_constraint_missing_bones() {
        let mut bones = vec![
            AnimatedBone {
                bone: identity_bone("A", None),
                anim_transform: None,
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
            },
            AnimatedBone {
                bone: identity_bone("B", Some(0)),
                anim_transform: None,
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
            },
        ];

        apply_hlpb_constraints(
            &mut bones,
            &Hlpb::V11 {
                aim_constraints: Vec::new().into(),
                orient_constraints: vec![OrientConstraint {
                    name: "".into(),
                    bone_name: "".into(),
                    root_bone_name: "".into(),
                    parent_bone_name: "".into(),
                    driver_bone_name: "".into(),
                    unk_type: 2,
                    constraint_axes: Vector3::new(1.0, 1.0, 1.0),
                    quat1: Vector4::new(0.0, 0.0, 0.0, 1.0),
                    quat2: Vector4::new(0.0, 0.0, 0.0, 1.0),
                    range_min: Vector3::new(-180.0, -180.0, -180.0),
                    range_max: Vector3::new(180.0, 180.0, 180.0),
                }]
                .into(),
                constraint_indices: Vec::new().into(),
                constraint_types: Vec::new().into(),
            },
        );
    }

    #[test]
    fn apply_single_orient_constraint_copy_xyz() {
        let mut bones = vec![
            AnimatedBone {
                bone: identity_bone("A", None),
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::ZERO,
                    rotation: glam::Quat::from_axis_angle(
                        glam::Vec3::ONE.normalize(),
                        std::f32::consts::PI / 2.0,
                    ),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
            },
            AnimatedBone {
                bone: identity_bone("B", Some(0)),
                anim_transform: None,
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
            },
        ];

        // Copy the rotation of A onto B.
        apply_hlpb_constraints(
            &mut bones,
            &Hlpb::V11 {
                aim_constraints: Vec::new().into(),
                orient_constraints: vec![OrientConstraint {
                    name: "constraint1".into(),
                    bone_name: "A".into(),
                    root_bone_name: "A".into(),
                    parent_bone_name: "A".into(),
                    driver_bone_name: "B".into(),
                    unk_type: 2,
                    constraint_axes: Vector3::new(1.0, 1.0, 1.0),
                    quat1: Vector4::new(0.0, 0.0, 0.0, 1.0),
                    quat2: Vector4::new(0.0, 0.0, 0.0, 1.0),
                    range_min: Vector3::new(-180.0, -180.0, -180.0),
                    range_max: Vector3::new(180.0, 180.0, 180.0),
                }]
                .into(),
                constraint_indices: Vec::new().into(),
                constraint_types: Vec::new().into(),
            },
        );

        assert_vector_relative_eq!(
            bones[1].anim_transform.unwrap().rotation.to_array(),
            bones[0].anim_transform.unwrap().rotation.to_array()
        );
    }
}
