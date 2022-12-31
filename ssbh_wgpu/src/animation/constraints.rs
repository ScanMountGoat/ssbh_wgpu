use glam::Vec4Swizzles;
use ssbh_data::{hlpb_data::*, skel_data::BoneData};

fn interp(a: f32, b: f32, f: f32) -> f32 {
    (1.0 - f) * a + f * b
}

// TODO: Improve tests.
// TODO: Fix evaluation order when siblings are constrained to each other?
pub fn apply_aim_constraint(
    world_transforms: &[glam::Mat4],
    bones: &[BoneData],
    constraint: &AimConstraintData,
    target_transform: glam::Mat4,
) -> Option<glam::Mat4> {
    // TODO: Investigate the remaining bone name fields.
    let source = bones
        .iter()
        .position(|b| b.name == constraint.aim_bone_name1)?;

    // We want the target bone to point at the source bone.
    let source_world = *world_transforms
        .get(source)
        .unwrap_or(&glam::Mat4::IDENTITY);

    let target = bones
        .iter()
        .position(|b| b.name == constraint.target_bone_name1)?;

    // TODO: Avoid finding the bone twice?
    let target_world = *world_transforms
        .get(target)
        .unwrap_or(&glam::Mat4::IDENTITY);

    let _target = bones
        .iter()
        .find(|b| b.name == constraint.target_bone_name1)?;

    // TODO: Can a bone not affected by the anim be the source?
    // TODO: Will the target of a constraint ever be animated?
    let src_pos = source_world.col(3);
    let target_pos = target_world.col(3);

    // Get the local axes of the bone to constrain.
    let aim = (target_world
        * glam::vec4(constraint.aim.x, constraint.aim.y, constraint.aim.z, 0.0))
    .xyz();

    // Get the vector pointing to the desired bone.
    let v = src_pos.xyz() - target_pos.xyz();

    // TODO: Is it correct to assume the target transform is always relative to the target parent?
    // Apply an additional rotation to orient the local axes towards the desired bone.
    // TODO: How to also incorporate the up vector?
    let (target_s, mut target_r, target_t) = target_transform.to_scale_rotation_translation();
    target_r *= glam::Quat::from_rotation_arc(aim.normalize(), v.normalize());

    Some(glam::Mat4::from_scale_rotation_translation(
        target_s, target_r, target_t,
    ))
}

// TODO: Improve tests.
pub fn apply_orient_constraint(
    world_transforms: &[glam::Mat4],
    bones: &[BoneData],
    constraint: &OrientConstraintData,
    target_transform: glam::Mat4,
) -> Option<glam::Mat4> {
    // TODO: Investigate the remaining bone name fields.
    // TODO: What's the difference between root and bone name?
    // TODO: Do the unk types matter?
    // TODO: quat1 and quat2 correct for twists?
    let source = bones
        .iter()
        .position(|b| b.name == constraint.source_bone_name)?;

    let target = bones
        .iter()
        .position(|b| b.name == constraint.target_bone_name)?;

    let source_parent = bones[source].parent_index;
    let target_parent = bones[target].parent_index;

    let source_world = *world_transforms
        .get(source)
        .unwrap_or(&glam::Mat4::IDENTITY);
    let _target_world = *world_transforms
        .get(target)
        .unwrap_or(&glam::Mat4::IDENTITY);
    // TODO: Do we need the source parent world?
    let _source_parent_world = source_parent
        .map(|p| world_transforms.get(p).unwrap_or(&glam::Mat4::IDENTITY))
        .unwrap_or(&glam::Mat4::IDENTITY);
    let target_parent_world = target_parent
        .map(|p| world_transforms.get(p).unwrap_or(&glam::Mat4::IDENTITY))
        .unwrap_or(&glam::Mat4::IDENTITY);

    // TODO: These angles correct twists for some models?
    let _quat1 = glam::Quat::from_array(constraint.quat1.to_array());
    let _quat2 = glam::Quat::from_array(constraint.quat2.to_array());

    let _target_bone = bones
        .iter()
        .find(|b| b.name == constraint.target_bone_name)?;

    // Calculate the source bone's world orientation.
    // Convert to be relative to the target's parent using the inverse.
    // TODO: Create a test case that checks for the matrix multiplication order here.
    let source_transform = target_parent_world.inverse() * source_world;
    let (_, source_r, _) = (source_transform).to_scale_rotation_translation();

    // Apply rotations in the order X -> Y -> Z.
    let (source_rot_z, source_rot_y, source_rot_x) = source_r.to_euler(glam::EulerRot::ZYX);

    // TODO: Is it correct to assume the target transform is always relative to the target parent?
    let (target_s, target_r, target_t) = target_transform.to_scale_rotation_translation();

    let (target_rot_z, target_rot_y, target_rot_x) = target_r.to_euler(glam::EulerRot::ZYX);

    // The first angle is Z, the second angle is Y, and the third angle is X.
    let interp_rotation = glam::Quat::from_euler(
        glam::EulerRot::ZYX,
        interp(target_rot_z, source_rot_z, constraint.constraint_axes.z),
        interp(target_rot_y, source_rot_y, constraint.constraint_axes.y),
        interp(target_rot_x, source_rot_x, constraint.constraint_axes.x),
    );

    Some(glam::Mat4::from_scale_rotation_translation(
        target_s,
        interp_rotation,
        target_t,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        animation::{animate_skel_inner, AnimTransform, AnimatedBone, AnimationTransforms},
        assert_vector_relative_eq,
    };
    use ssbh_data::{
        anim_data::TransformFlags,
        skel_data::{BillboardType, BoneData},
        Vector3, Vector4,
    };

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

    fn relative_rotation(
        result: &AnimationTransforms,
        skel_bones: &[BoneData],
        i: usize,
    ) -> glam::Quat {
        // Get the transform relative to its parent and extract its rotation.
        // Hlpb constraints only affect rotation, so ignore translation and scale.
        (skel_bones[i]
            .parent_index
            .map(|p| result.world_transforms[p].inverse())
            .unwrap_or(glam::Mat4::IDENTITY)
            * result.world_transforms[i])
            .to_scale_rotation_translation()
            .1
    }

    fn relative_translation(
        result: &AnimationTransforms,
        skel_bones: &[BoneData],
        i: usize,
    ) -> glam::Vec3 {
        // Get the transform relative to its parent and extract its translation.
        (skel_bones[i]
            .parent_index
            .map(|p| result.world_transforms[p].inverse())
            .unwrap_or(glam::Mat4::IDENTITY)
            * result.world_transforms[i])
            .to_scale_rotation_translation()
            .2
    }

    #[test]
    fn single_orient_constraint_missing_bones() {
        let skel_bones = vec![identity_bone("A", None), identity_bone("B", Some(0))];

        let bones = vec![
            AnimatedBone {
                bone: &skel_bones[0],
                anim_transform: None,
                compensate_scale: false,
                flags: TransformFlags::default(),
            },
            AnimatedBone {
                bone: &skel_bones[1],
                anim_transform: None,
                compensate_scale: false,
                flags: TransformFlags::default(),
            },
        ];
        // TODO: Manage the indices in a cleaner way.
        let mut bones: Vec<_> = bones.into_iter().enumerate().collect();

        let mut result = AnimationTransforms::identity();
        animate_skel_inner(
            &mut result,
            &mut bones,
            &skel_bones,
            Some(&HlpbData {
                major_version: 1,
                minor_version: 0,
                aim_constraints: Vec::new(),
                orient_constraints: vec![OrientConstraintData {
                    name: String::new(),
                    parent_bone_name1: String::new(),
                    parent_bone_name2: String::new(),
                    source_bone_name: String::new(),
                    target_bone_name: String::new(),
                    unk_type: 2,
                    constraint_axes: Vector3::new(1.0, 1.0, 1.0),
                    quat1: Vector4::new(0.0, 0.0, 0.0, 1.0),
                    quat2: Vector4::new(0.0, 0.0, 0.0, 1.0),
                    range_min: Vector3::new(-180.0, -180.0, -180.0),
                    range_max: Vector3::new(180.0, 180.0, 180.0),
                }],
            }),
        );
    }

    #[test]
    fn single_orient_constraint_copy_xyz() {
        let skel_bones = vec![identity_bone("A", None), identity_bone("B", None)];

        let bones = vec![
            AnimatedBone {
                bone: &skel_bones[0],
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::ZERO,
                    rotation: glam::Quat::from_axis_angle(
                        glam::vec3(1.0, 2.0, 3.0).normalize(),
                        std::f32::consts::PI / 4.0,
                    ),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                flags: TransformFlags::default(),
            },
            AnimatedBone {
                bone: &skel_bones[1],
                anim_transform: None,
                compensate_scale: false,
                flags: TransformFlags::default(),
            },
        ];
        // TODO: Manage the indices in a cleaner way.
        let mut bones: Vec<_> = bones.into_iter().enumerate().collect();

        // Copy the rotation of A onto B.
        let mut result = AnimationTransforms::identity();
        animate_skel_inner(
            &mut result,
            &mut bones,
            &skel_bones,
            Some(&HlpbData {
                major_version: 1,
                minor_version: 0,
                aim_constraints: Vec::new(),
                orient_constraints: vec![OrientConstraintData {
                    name: "constraint1".into(),
                    parent_bone_name1: "A".into(),
                    parent_bone_name2: "A".into(),
                    source_bone_name: "A".into(),
                    target_bone_name: "B".into(),
                    unk_type: 2,
                    constraint_axes: Vector3::new(1.0, 1.0, 1.0),
                    quat1: Vector4::new(0.0, 0.0, 0.0, 1.0),
                    quat2: Vector4::new(0.0, 0.0, 0.0, 1.0),
                    range_min: Vector3::new(-180.0, -180.0, -180.0),
                    range_max: Vector3::new(180.0, 180.0, 180.0),
                }],
            }),
        );

        assert_vector_relative_eq!(
            relative_rotation(&result, &skel_bones, 0).to_array(),
            relative_rotation(&result, &skel_bones, 1).to_array()
        );
    }

    #[test]
    fn single_orient_constraint_half_xyz() {
        let skel_bones = vec![identity_bone("A", None), identity_bone("B", None)];

        // Use a rotation order of X -> Y -> Z.
        let bones = vec![
            AnimatedBone {
                bone: &skel_bones[0],
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::ZERO,
                    rotation: glam::Quat::from_euler(glam::EulerRot::ZYX, 0.3, 0.2, 0.1),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                flags: TransformFlags::default(),
            },
            AnimatedBone {
                bone: &skel_bones[1],
                anim_transform: None,
                compensate_scale: false,
                flags: TransformFlags::default(),
            },
        ];
        // TODO: Manage the indices in a cleaner way.
        let mut bones: Vec<_> = bones.into_iter().enumerate().collect();

        // Copy half of the rotation of A onto B.
        let mut result = AnimationTransforms::identity();
        animate_skel_inner(
            &mut result,
            &mut bones,
            &skel_bones,
            Some(&HlpbData {
                major_version: 1,
                minor_version: 0,
                aim_constraints: Vec::new(),
                orient_constraints: vec![OrientConstraintData {
                    name: "constraint1".into(),
                    parent_bone_name1: "A".into(),
                    parent_bone_name2: "A".into(),
                    source_bone_name: "A".into(),
                    target_bone_name: "B".into(),
                    unk_type: 2,
                    constraint_axes: Vector3::new(0.5, 0.5, 0.5),
                    quat1: Vector4::new(0.0, 0.0, 0.0, 1.0),
                    quat2: Vector4::new(0.0, 0.0, 0.0, 1.0),
                    range_min: Vector3::new(-180.0, -180.0, -180.0),
                    range_max: Vector3::new(180.0, 180.0, 180.0),
                }],
            }),
        );

        assert_vector_relative_eq!(
            glam::Quat::from_euler(glam::EulerRot::ZYX, 0.15, 0.1, 0.05).to_array(),
            relative_rotation(&result, &skel_bones, 1).to_array()
        );
    }

    #[test]
    fn orient_constraints_same_parent() {
        let skel_bones = vec![
            identity_bone("Root", None),
            identity_bone("A", Some(0)),
            identity_bone("B", Some(0)),
        ];

        let bones = vec![
            AnimatedBone {
                bone: &skel_bones[0],
                anim_transform: Some(AnimTransform {
                    translation: glam::vec3(0.0, 0.0, 0.0),
                    rotation: glam::Quat::from_rotation_z(0.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                flags: TransformFlags::default(),
            },
            AnimatedBone {
                bone: &skel_bones[1],
                anim_transform: Some(AnimTransform {
                    translation: glam::vec3(-1.0, 0.0, 0.0),
                    rotation: glam::Quat::from_rotation_z(90.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                flags: TransformFlags::default(),
            },
            AnimatedBone {
                bone: &skel_bones[2],
                anim_transform: Some(AnimTransform {
                    translation: glam::vec3(1.0, 0.0, 0.0),
                    rotation: glam::Quat::from_rotation_z(0.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                flags: TransformFlags::default(),
            },
        ];
        // TODO: Manage the indices in a cleaner way.
        let mut bones: Vec<_> = bones.into_iter().enumerate().collect();

        // Copy the rotation of A to B.
        let mut result = AnimationTransforms::identity();
        animate_skel_inner(
            &mut result,
            &mut bones,
            &skel_bones,
            Some(&HlpbData {
                major_version: 1,
                minor_version: 0,
                aim_constraints: Vec::new(),
                orient_constraints: vec![OrientConstraintData {
                    name: "constraint1".into(),
                    parent_bone_name1: "Root".into(),
                    parent_bone_name2: "Root".into(),
                    source_bone_name: "A".into(),
                    target_bone_name: "B".into(),
                    unk_type: 2,
                    constraint_axes: Vector3::new(1.0, 1.0, 1.0),
                    quat1: Vector4::new(0.0, 0.0, 0.0, 1.0),
                    quat2: Vector4::new(0.0, 0.0, 0.0, 1.0),
                    range_min: Vector3::new(-180.0, -180.0, -180.0),
                    range_max: Vector3::new(180.0, 180.0, 180.0),
                }],
            }),
        );

        assert_vector_relative_eq!(
            relative_rotation(&result, &skel_bones, 1).to_array(),
            relative_rotation(&result, &skel_bones, 2).to_array()
        );
    }

    #[test]
    fn orient_constraints_different_parents() {
        // Skel + Anim:
        // L2
        // ^
        // |
        // L1 <-- L0    R0 --> R1 --> R2

        // Skel + Anim + Hlpb (constrain R1 to L1):
        // L2                  R2
        // ^                   ^
        // |                   |
        // L1 <-- L0    R0 --> R1
        let skel_bones = vec![
            identity_bone("L0", None),
            identity_bone("L1", Some(0)),
            identity_bone("L2", Some(1)),
            identity_bone("R0", None),
            identity_bone("R1", Some(3)),
            identity_bone("R2", Some(4)),
        ];

        let bones = vec![
            AnimatedBone {
                bone: &skel_bones[0],
                anim_transform: Some(AnimTransform {
                    translation: glam::vec3(-1.0, 0.0, 0.0),
                    rotation: glam::Quat::from_rotation_z(90.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                flags: TransformFlags::default(),
            },
            AnimatedBone {
                bone: &skel_bones[1],
                anim_transform: Some(AnimTransform {
                    translation: glam::vec3(0.0, 1.0, 0.0),
                    rotation: glam::Quat::from_rotation_z(-90.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                flags: TransformFlags::default(),
            },
            AnimatedBone {
                bone: &skel_bones[2],
                anim_transform: Some(AnimTransform {
                    translation: glam::vec3(0.0, 1.0, 0.0),
                    rotation: glam::Quat::from_rotation_z(0.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                flags: TransformFlags::default(),
            },
            AnimatedBone {
                bone: &skel_bones[3],
                anim_transform: Some(AnimTransform {
                    translation: glam::vec3(1.0, 0.0, 0.0),
                    rotation: glam::Quat::from_rotation_z(-90.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                flags: TransformFlags::default(),
            },
            AnimatedBone {
                bone: &skel_bones[4],
                anim_transform: Some(AnimTransform {
                    translation: glam::vec3(0.0, 1.0, 0.0),
                    // TODO: What should this be without constraints?
                    rotation: glam::Quat::from_rotation_z(0.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                flags: TransformFlags::default(),
            },
            AnimatedBone {
                bone: &skel_bones[5],
                anim_transform: Some(AnimTransform {
                    translation: glam::vec3(0.0, 1.0, 0.0),
                    rotation: glam::Quat::from_rotation_z(0.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                flags: TransformFlags::default(),
            },
        ];
        // TODO: Manage the indices in a cleaner way.
        let mut bones: Vec<_> = bones.into_iter().enumerate().collect();

        // Copy the rotation of L1 to R1.
        let mut result = AnimationTransforms::identity();
        animate_skel_inner(
            &mut result,
            &mut bones,
            &skel_bones,
            Some(&HlpbData {
                major_version: 1,
                minor_version: 0,
                aim_constraints: Vec::new(),
                orient_constraints: vec![OrientConstraintData {
                    name: "constraint1".into(),
                    parent_bone_name1: "Root".into(), // TODO: What to put here?
                    parent_bone_name2: "Root".into(),
                    source_bone_name: "L1".into(),
                    target_bone_name: "R1".into(),
                    unk_type: 2,
                    constraint_axes: Vector3::new(1.0, 1.0, 1.0),
                    quat1: Vector4::new(0.0, 0.0, 0.0, 1.0),
                    quat2: Vector4::new(0.0, 0.0, 0.0, 1.0),
                    range_min: Vector3::new(-180.0, -180.0, -180.0),
                    range_max: Vector3::new(180.0, 180.0, 180.0),
                }],
            }),
        );

        assert_vector_relative_eq!(
            [0.0, 0.0, 0.7071, 0.7071],
            relative_rotation(&result, &skel_bones, 0).to_array()
        );
        assert_vector_relative_eq!(
            [0.0, 0.0, 0.7071, -0.7071],
            relative_rotation(&result, &skel_bones, 1).to_array()
        );
        assert_vector_relative_eq!(
            [0.0, 0.0, 0.0, 1.0],
            relative_rotation(&result, &skel_bones, 2).to_array()
        );
        assert_vector_relative_eq!(
            [0.0, 0.0, 0.7071, -0.7071],
            relative_rotation(&result, &skel_bones, 3).to_array()
        );
        assert_vector_relative_eq!(
            [0.0, 0.0, 0.7071, 0.7071],
            relative_rotation(&result, &skel_bones, 4).to_array()
        );
        assert_vector_relative_eq!(
            [0.0, 0.0, 0.0, 1.0],
            relative_rotation(&result, &skel_bones, 5).to_array()
        );

        let position_world = |bone: usize| {
            result.world_transforms[bone]
                .to_scale_rotation_translation()
                .2
                .to_array()
        };

        // L0, L1, L2
        assert_vector_relative_eq!([-1.0, 0.0, 0.0], position_world(0));
        assert_vector_relative_eq!([-2.0, 0.0, 0.0], position_world(1));
        assert_vector_relative_eq!([-2.0, 1.0, 0.0], position_world(2));

        // R0, R1, R2
        assert_vector_relative_eq!([1.0, 0.0, 0.0], position_world(3));
        assert_vector_relative_eq!([2.0, 0.0, 0.0], position_world(4));
        assert_vector_relative_eq!([2.0, 1.0, 0.0], position_world(5));
    }

    #[test]
    fn single_aim_constraint_xz_plane() {
        // TODO: How test up vector?
        // Skel + Anim:
        //      B ->
        // A ->

        // Skel + Anim + Hlpb (point A to B):
        //    B ->
        // A /
        let skel_bones = vec![identity_bone("A", None), identity_bone("B", None)];

        let bones = vec![
            AnimatedBone {
                bone: &skel_bones[0],
                anim_transform: Some(AnimTransform {
                    translation: glam::vec3(0.0, 0.0, 0.0),
                    rotation: glam::Quat::IDENTITY,
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                flags: TransformFlags::default(),
            },
            AnimatedBone {
                bone: &skel_bones[1],
                anim_transform: Some(AnimTransform {
                    translation: glam::vec3(1.0, 0.0, 1.0),
                    rotation: glam::Quat::IDENTITY,
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                flags: TransformFlags::default(),
            },
        ];
        // TODO: Manage the indices in a cleaner way.
        let mut bones: Vec<_> = bones.into_iter().enumerate().collect();

        // Point the X-axis of A to B.
        let mut result = AnimationTransforms::identity();
        animate_skel_inner(
            &mut result,
            &mut bones,
            &skel_bones,
            Some(&HlpbData {
                major_version: 1,
                minor_version: 0,
                aim_constraints: vec![AimConstraintData {
                    name: "constraint".to_string(),
                    aim_bone_name1: "B".to_string(), // TODO: What to put here?
                    aim_bone_name2: "B".to_string(),
                    aim_type1: "DEFAULT".to_string(),
                    aim_type2: "DEFAULT".to_string(),
                    target_bone_name1: "A".to_string(),
                    target_bone_name2: "A".to_string(),
                    unk1: 0,
                    unk2: 0,
                    aim: Vector3::new(1.0, 0.0, 0.0),
                    up: Vector3::new(0.0, 1.0, 0.0),
                    quat1: Vector4::new(0.0, 0.0, 0.0, 1.0),
                    quat2: Vector4::new(0.0, 0.0, 0.0, 1.0),
                }],
                orient_constraints: Vec::new(),
            }),
        );

        // A rotates along the Y-axis to point to B.
        assert_vector_relative_eq!(
            [0.0, 0.0, 0.0],
            relative_translation(&result, &skel_bones, 0).to_array()
        );
        assert_vector_relative_eq!(
            glam::Quat::from_rotation_y(-45.0f32.to_radians()).to_array(),
            relative_rotation(&result, &skel_bones, 0).to_array()
        );

        // B remains the same.
        assert_vector_relative_eq!(
            [1.0, 0.0, 1.0],
            relative_translation(&result, &skel_bones, 1).to_array()
        );
        assert_vector_relative_eq!(
            [0.0, 0.0, 0.0, 1.0],
            relative_rotation(&result, &skel_bones, 1).to_array()
        );
    }

    #[test]
    fn single_aim_constraint_missing_bones() {
        let skel_bones = vec![identity_bone("A", None), identity_bone("B", None)];

        let bones = vec![
            AnimatedBone {
                bone: &skel_bones[0],
                anim_transform: Some(AnimTransform {
                    translation: glam::vec3(0.0, 0.0, 0.0),
                    rotation: glam::Quat::IDENTITY,
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                flags: TransformFlags::default(),
            },
            AnimatedBone {
                bone: &skel_bones[1],
                anim_transform: Some(AnimTransform {
                    translation: glam::vec3(1.0, 0.0, 1.0),
                    rotation: glam::Quat::IDENTITY,
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                flags: TransformFlags::default(),
            },
        ];
        // TODO: Manage the indices in a cleaner way.
        let mut bones: Vec<_> = bones.into_iter().enumerate().collect();

        let mut result = AnimationTransforms::identity();
        animate_skel_inner(
            &mut result,
            &mut bones,
            &skel_bones,
            Some(&HlpbData {
                major_version: 1,
                minor_version: 0,
                aim_constraints: vec![AimConstraintData {
                    name: "constraint".to_string(),
                    aim_bone_name1: String::new(),
                    aim_bone_name2: String::new(),
                    aim_type1: "DEFAULT".to_string(),
                    aim_type2: "DEFAULT".to_string(),
                    target_bone_name1: String::new(),
                    target_bone_name2: String::new(),
                    unk1: 0,
                    unk2: 0,
                    aim: Vector3::new(1.0, 0.0, 0.0),
                    up: Vector3::new(0.0, 1.0, 0.0),
                    quat1: Vector4::new(0.0, 0.0, 0.0, 1.0),
                    quat2: Vector4::new(0.0, 0.0, 0.0, 1.0),
                }],
                orient_constraints: Vec::new(),
            }),
        );
    }
}
