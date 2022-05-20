use super::{animated_world_transform, AnimTransform, AnimatedBone};
use glam::Vec4Swizzles;
use ssbh_lib::formats::hlpb::{AimConstraint, Hlpb, OrientConstraint};

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
            // TODO: Allow multiple constraints per bone.
            // TODO: Can the effects of constraints stack?
            // TODO: Better handling of application order.
            // TODO: Also sort these constraints?
            for aim in &aim_constraints.elements {
                apply_aim_constraint(animated_bones, aim);
            }

            for orient in &orient_constraints.elements {
                apply_orient_constraint(animated_bones, orient);
            }
        }
    }
}

fn apply_aim_constraint(animated_bones: &mut [AnimatedBone], constraint: &AimConstraint) {
    // TODO: Investigate the remaining bone name fields.
    let source = animated_bones
        .iter()
        .find(|b| b.bone.name == constraint.aim_bone_name1.to_string_lossy())
        .cloned()
        .unwrap();

    // We want the target bone to point at the source bone.
    // TODO: Is there a way to do this without using the world transforms?
    let source_world = animated_world_transform(animated_bones, &source)
        .unwrap()
        .transpose();

    // TODO: Avoid finding the bone twice?
    let target_world = animated_world_transform(
        animated_bones,
        animated_bones
            .iter()
            .find(|b| b.bone.name == constraint.target_bone_name1.to_string_lossy())
            .unwrap(),
    )
    .unwrap()
    .transpose();

    let target = animated_bones
        .iter_mut()
        .find(|b| b.bone.name == constraint.target_bone_name1.to_string_lossy())
        .unwrap();

    // TODO: Can a bone not affected by the anim be the source?
    // TODO: Will the target of a constraint ever be animated?
    let mut target_transform = target
        .anim_transform
        .unwrap_or(AnimTransform::from_bone(&target.bone));

    let src_pos = source_world.col(3);
    let target_pos = target_world.col(3);

    // Get the local axes of the bone to constrain.
    let aim = (target_world * glam::Vec4::new(1.0, 0.0, 0.0, 0.0)).xyz();

    // Get the vector pointing to the desired bone.
    let v = src_pos.xyz() - target_pos.xyz();

    // Apply an additional rotation to orient the local axes towards the desired bone.
    // TODO: How to also incorporate the up vector?
    target_transform.rotation =
        target_transform.rotation * glam::Quat::from_rotation_arc(aim.normalize(), v.normalize());
    target.anim_transform = Some(target_transform);
}

fn apply_orient_constraint(animated_bones: &mut [AnimatedBone], constraint: &OrientConstraint) {
    // TODO: Investigate the remaining bone name fields.
    let source = animated_bones
        .iter()
        .find(|b| b.bone.name == constraint.parent_bone_name.to_string_lossy())
        .cloned()
        .unwrap();

    let target = animated_bones
        .iter()
        .find(|b| b.bone.name == constraint.driver_bone_name.to_string_lossy())
        .cloned()
        .unwrap();

    // TODO: What's the difference between root and bone name?
    // let source_parent = animated_bones
    //     .iter()
    //     .find(|b| b.bone.name == constraint.root_bone_name.to_string_lossy())
    //     .cloned()
    //     .unwrap();

    let source_parent = source.bone.parent_index.and_then(|i| animated_bones.get(i));

    // let target_parent = animated_bones
    //     .iter()
    //     .find(|b| b.bone.name == constraint.bone_name.to_string_lossy())
    //     .cloned()
    //     .unwrap();

    let target_parent = target.bone.parent_index.and_then(|i| animated_bones.get(i));

    let source_world = animated_world_transform(animated_bones, &source).unwrap();
    let target_world = animated_world_transform(animated_bones, &target).unwrap();
    let source_parent_world = source_parent
        .map(|p| animated_world_transform(animated_bones, p).unwrap())
        .unwrap_or(glam::Mat4::IDENTITY);
    let target_parent_world = target_parent
        .map(|p| animated_world_transform(animated_bones, p).unwrap())
        .unwrap_or(glam::Mat4::IDENTITY);

    let quat1 = glam::Quat::from_array(constraint.quat1.to_array());
    let quat2 = glam::Quat::from_array(constraint.quat2.to_array());

    let target = animated_bones
        .iter_mut()
        .find(|b| b.bone.name == constraint.driver_bone_name.to_string_lossy())
        .unwrap();

    // TODO: Can a bone not affected by the anim be the source?
    // TODO: Will the target of a constraint ever be animated?

    // TODO: Perform the copy rotation in world space and convert back to the appropriate space.
    // let source_transform = source.animated_transform(true).transpose();

    // Calculate the source bone's world orientation.
    // Convert to be relative to the target's parent using the inverse.
    // TODO: Create a test case that checks for the matrix multiplication order here.
    let source_transform = target_parent_world.transpose().inverse() * source_world.transpose();
    let (source_s, source_r, source_t) = (source_transform).to_scale_rotation_translation();
    // dbg!(source_s, source_r, source_t);

    let (source_rot_x, source_rot_y, source_rot_z) = (source_r).to_euler(glam::EulerRot::XYZ);

    // dbg!(source_parent_world, target_parent_world);

    // if let Some(source_transform) = source.anim_transform {
    // TODO: Do the unk types matter?

    // TODO: quat1 and quat2 correct for twists?

    // Leave the target transform as is since it's already relative to the target parent.
    let target_transform = target.animated_transform(true);
    let (target_s, target_r, target_t) = (target_transform).to_scale_rotation_translation();

    let (target_rot_x, target_rot_y, target_rot_z) = target_r.to_euler(glam::EulerRot::XYZ);

    let interp_rotation = glam::Quat::from_euler(
        glam::EulerRot::XYZ,
        interp(target_rot_x, source_rot_x, constraint.constraint_axes.x),
        interp(target_rot_y, source_rot_y, constraint.constraint_axes.y),
        interp(target_rot_z, source_rot_z, constraint.constraint_axes.z),
    );

    // let interp_rotation =  interp_rotation * glam::Quat::from_mat4(&target_parent_world);

    let mut new_transform = target
        .anim_transform
        .unwrap_or(AnimTransform::from_bone(&target.bone));

    new_transform.rotation = interp_rotation;
    target.anim_transform = Some(new_transform);
    // }

    // dbg!(target_transform.scale, target_transform.rotation, target_transform.translation);
    // let test_transform = target_parent_world.transpose().inverse() * target_world.transpose();
    // let test_transform = target_parent_world.transpose().inverse() * source_world.transpose();

    // let (s,r,t) = test_transform.to_scale_rotation_translation();
    // dbg!(s, r, t);
    // target_transform.scale = s;
    // target_transform.rotation = r;
    // target_transform.translation = t;

    // target.anim_transform = Some(target_transform);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_matrix_relative_eq, assert_vector_relative_eq};
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
    #[ignore]
    fn single_orient_constraint_missing_bones() {
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
    fn single_orient_constraint_copy_xyz() {
        let mut bones = vec![
            AnimatedBone {
                bone: identity_bone("A", None),
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::ZERO,
                    rotation: glam::Quat::from_axis_angle(
                        glam::Vec3::new(1.0, 2.0, 3.0).normalize(),
                        std::f32::consts::PI / 4.0,
                    ),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
            },
            AnimatedBone {
                bone: identity_bone("B", None),
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
            bones[0].anim_transform.unwrap().rotation.to_array(),
            bones[1].anim_transform.unwrap().rotation.to_array()
        );
    }

    #[test]
    fn orient_constraints_same_parent() {
        let mut bones = vec![
            AnimatedBone {
                bone: identity_bone("Root", None),
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::new(0.0, 0.0, 0.0),
                    rotation: glam::Quat::from_rotation_z(0.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
            },
            AnimatedBone {
                bone: identity_bone("A", Some(0)),
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::new(-1.0, 0.0, 0.0),
                    rotation: glam::Quat::from_rotation_z(90.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
            },
            AnimatedBone {
                bone: identity_bone("B", Some(0)),
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::new(1.0, 0.0, 0.0),
                    rotation: glam::Quat::from_rotation_z(0.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
            },
        ];

        // Copy the rotation of A to B.
        apply_hlpb_constraints(
            &mut bones,
            &Hlpb::V11 {
                aim_constraints: Vec::new().into(),
                orient_constraints: vec![OrientConstraint {
                    name: "constraint1".into(),
                    bone_name: "Root".into(),
                    root_bone_name: "Root".into(),
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
            bones[2].anim_transform.unwrap().rotation.to_array()
        );
    }

    #[test]
    fn orient_constraints_different_parents() {
        // Skel + Anim:
        // L2
        // ^
        // |
        // L1 <-- L0    R0 --> R1 --> R2

        // TODO: Skel + Anim + Hlpb (constrain R1 to L1):
        // L2                  R2
        // ^                   ^
        // |                   |
        // L1 <-- L0    R0 --> R1

        let mut bones = vec![
            AnimatedBone {
                bone: identity_bone("L0", None),
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::new(-1.0, 0.0, 0.0),
                    rotation: glam::Quat::from_rotation_z(90.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
            },
            AnimatedBone {
                bone: identity_bone("L1", Some(0)),
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::new(0.0, 1.0, 0.0),
                    rotation: glam::Quat::from_rotation_z(-90.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
            },
            AnimatedBone {
                bone: identity_bone("L2", Some(1)),
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::new(0.0, 1.0, 0.0),
                    rotation: glam::Quat::from_rotation_z(0.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
            },
            AnimatedBone {
                bone: identity_bone("R0", None),
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::new(1.0, 0.0, 0.0),
                    rotation: glam::Quat::from_rotation_z(-90.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
            },
            AnimatedBone {
                bone: identity_bone("R1", Some(3)),
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::new(0.0, 1.0, 0.0),
                    // TODO: What should this be without constraints?
                    rotation: glam::Quat::from_rotation_z(0.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
            },
            AnimatedBone {
                bone: identity_bone("R2", Some(4)),
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::new(0.0, 1.0, 0.0),
                    rotation: glam::Quat::from_rotation_z(0.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
            },
        ];

        // Copy the rotation of L1 to R1.
        apply_hlpb_constraints(
            &mut bones,
            &Hlpb::V11 {
                aim_constraints: Vec::new().into(),
                orient_constraints: vec![OrientConstraint {
                    name: "constraint1".into(),
                    bone_name: "Root".into(), // TODO: What to put here?
                    root_bone_name: "Root".into(),
                    parent_bone_name: "L1".into(),
                    driver_bone_name: "R1".into(),
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

        // TODO: Test the rotations?
        assert_vector_relative_eq!(
            [0.0, 0.0, 0.7071, 0.7071],
            bones[0].anim_transform.unwrap().rotation.to_array()
        );
        assert_vector_relative_eq!(
            [0.0, 0.0, -0.7071, 0.7071],
            bones[1].anim_transform.unwrap().rotation.to_array()
        );
        assert_vector_relative_eq!(
            [0.0, 0.0, 0.0, 1.0],
            bones[2].anim_transform.unwrap().rotation.to_array()
        );
        assert_vector_relative_eq!(
            [0.0, 0.0, -0.7071, 0.7071],
            bones[3].anim_transform.unwrap().rotation.to_array()
        );
        // TODO: This should be +90 degrees?
        // assert_vector_relative_eq!(
        //     [0.0, 0.0, 0.0, 1.0],
        //     bones[4].anim_transform.unwrap().rotation.to_array()
        // );
        // assert_vector_relative_eq!(
        //     [0.0, 0.0, 0.0, 1.0],
        //     bones[5].anim_transform.unwrap().rotation.to_array()
        // );

        let position_world = |bone| {
            animated_world_transform(&bones, bone)
                .unwrap()
                .transpose()
                .to_scale_rotation_translation()
                .2
                .to_array()
        };

        // L0, L1, L2
        assert_vector_relative_eq!([-1.0, 0.0, 0.0], position_world(&bones[0]));
        assert_vector_relative_eq!([-2.0, 0.0, 0.0], position_world(&bones[1]));
        assert_vector_relative_eq!([-2.0, 1.0, 0.0], position_world(&bones[2]));

        // R0, R1, R2
        assert_vector_relative_eq!([1.0, 0.0, 0.0], position_world(&bones[3]));
        assert_vector_relative_eq!([2.0, 0.0, 0.0], position_world(&bones[4]));
        assert_vector_relative_eq!([2.0, 1.0, 0.0], position_world(&bones[5]));
    }
}
