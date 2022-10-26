use super::{world_transform, AnimTransform, AnimatedBone};
use glam::Vec4Swizzles;
use ssbh_data::hlpb_data::*;

fn interp(a: f32, b: f32, f: f32) -> f32 {
    (1.0 - f) * a + f * b
}

pub fn apply_hlpb_constraints(bones: &mut [AnimatedBone], hlpb: &HlpbData) {
    // TODO: Rename the orient constraint to include rotation in the name?

    // TODO: Can the effects of constraints stack?
    // TODO: Calculate application order to respect dependencies?
    // TODO: Return or log errors?
    for aim in &hlpb.aim_constraints {
        apply_aim_constraint(bones, aim);
    }

    for orient in &hlpb.orient_constraints {
        apply_orient_constraint(bones, orient);
    }
}

fn apply_aim_constraint(bones: &mut [AnimatedBone], constraint: &AimConstraintData) -> Option<()> {
    // TODO: Investigate the remaining bone name fields.
    let source = bones
        .iter()
        .position(|b| b.bone.name == constraint.aim_bone_name1)?;

    // We want the target bone to point at the source bone.
    // TODO: Is there a way to do this without using the world transforms?
    let source_world = world_transform(bones, source).unwrap_or(glam::Mat4::IDENTITY);

    // TODO: Avoid finding the bone twice?
    let target_world = world_transform(
        bones,
        bones
            .iter()
            .position(|b| b.bone.name == constraint.target_bone_name1)?,
    )
    .unwrap_or(glam::Mat4::IDENTITY);

    let target = bones
        .iter_mut()
        .find(|b| b.bone.name == constraint.target_bone_name1)?;

    // TODO: Can a bone not affected by the anim be the source?
    // TODO: Will the target of a constraint ever be animated?
    let mut target_transform = target
        .anim_transform
        .unwrap_or_else(|| AnimTransform::from_bone(target.bone));

    let src_pos = source_world.col(3);
    let target_pos = target_world.col(3);

    // Get the local axes of the bone to constrain.
    let aim = (target_world
        * glam::Vec4::new(constraint.aim.x, constraint.aim.y, constraint.aim.z, 0.0))
    .xyz();

    // Get the vector pointing to the desired bone.
    let v = src_pos.xyz() - target_pos.xyz();

    // Apply an additional rotation to orient the local axes towards the desired bone.
    // TODO: How to also incorporate the up vector?
    target_transform.rotation *= glam::Quat::from_rotation_arc(aim.normalize(), v.normalize());
    target.anim_transform = Some(target_transform);
    // This bone was modified, so its animated world transform needs to be recalculated.
    target.anim_world_transform = None;
    Some(())
}

// TODO: Improve tests.
fn apply_orient_constraint(
    bones: &mut [AnimatedBone],
    constraint: &OrientConstraintData,
) -> Option<()> {
    // TODO: Investigate the remaining bone name fields.
    // TODO: What's the difference between root and bone name?
    // TODO: Do the unk types matter?
    // TODO: quat1 and quat2 correct for twists?
    let source = bones
        .iter()
        .position(|b| b.bone.name == constraint.source_bone_name)?;

    let target = bones
        .iter()
        .position(|b| b.bone.name == constraint.target_bone_name)?;

    let source_parent = bones[source].bone.parent_index;
    let target_parent = bones[target].bone.parent_index;

    let source_world = world_transform(bones, source).unwrap_or(glam::Mat4::IDENTITY);
    let _target_world = world_transform(bones, target).unwrap_or(glam::Mat4::IDENTITY);
    // TODO: Do we need the source parent world?
    let _source_parent_world = source_parent
        .map(|p| world_transform(bones, p).unwrap_or(glam::Mat4::IDENTITY))
        .unwrap_or(glam::Mat4::IDENTITY);
    let target_parent_world = target_parent
        .map(|p| world_transform(bones, p).unwrap_or(glam::Mat4::IDENTITY))
        .unwrap_or(glam::Mat4::IDENTITY);

    // TODO: These angles correct twists for some models?
    let _quat1 = glam::Quat::from_array(constraint.quat1.to_array());
    let _quat2 = glam::Quat::from_array(constraint.quat2.to_array());

    let target = bones
        .iter_mut()
        .find(|b| b.bone.name == constraint.target_bone_name)?;

    // Calculate the source bone's world orientation.
    // Convert to be relative to the target's parent using the inverse.
    // TODO: Create a test case that checks for the matrix multiplication order here.
    let source_transform = target_parent_world.inverse() * source_world;
    let (_, source_r, _) = (source_transform).to_scale_rotation_translation();

    // Apply rotations in the order X -> Y -> Z.
    let (source_rot_z, source_rot_y, source_rot_x) = (source_r).to_euler(glam::EulerRot::ZYX);

    // Leave the target transform as is since it's already relative to the target parent.
    let target_transform = target.animated_transform(true);
    let (_, target_r, _) = (target_transform).to_scale_rotation_translation();

    let (target_rot_z, target_rot_y, target_rot_x) = target_r.to_euler(glam::EulerRot::ZYX);

    // The first angle is Z, the second angle is Y, and the third angle is X.
    let interp_rotation = glam::Quat::from_euler(
        glam::EulerRot::ZYX,
        interp(target_rot_z, source_rot_z, constraint.constraint_axes.z),
        interp(target_rot_y, source_rot_y, constraint.constraint_axes.y),
        interp(target_rot_x, source_rot_x, constraint.constraint_axes.x),
    );

    let mut new_transform = target
        .anim_transform
        .unwrap_or_else(|| AnimTransform::from_bone(target.bone));

    new_transform.rotation = interp_rotation;
    target.anim_transform = Some(new_transform);
    // This bone was modified, so its animated world transform needs to be recalculated.
    target.anim_world_transform = None;
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_vector_relative_eq;
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

    #[test]
    fn single_orient_constraint_missing_bones() {
        let a = identity_bone("A", None);
        let b = identity_bone("B", Some(0));
        let mut bones = vec![
            AnimatedBone {
                bone: &a,
                anim_transform: None,
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
                anim_world_transform: None,
            },
            AnimatedBone {
                bone: &b,
                anim_transform: None,
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
                anim_world_transform: None,
            },
        ];

        apply_hlpb_constraints(
            &mut bones,
            &HlpbData {
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
            },
        );
    }

    #[test]
    fn single_orient_constraint_copy_xyz() {
        let a = identity_bone("A", None);
        let b = identity_bone("B", None);
        let mut bones = vec![
            AnimatedBone {
                bone: &a,
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
                anim_world_transform: None,
            },
            AnimatedBone {
                bone: &b,
                anim_transform: None,
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
                anim_world_transform: None,
            },
        ];

        // Copy the rotation of A onto B.
        apply_hlpb_constraints(
            &mut bones,
            &HlpbData {
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
            },
        );

        assert_vector_relative_eq!(
            bones[0].anim_transform.unwrap().rotation.to_array(),
            bones[1].anim_transform.unwrap().rotation.to_array()
        );
    }

    #[test]
    fn single_orient_constraint_half_xyz() {
        let a = identity_bone("A", None);
        let b = identity_bone("B", None);

        // Use a rotation order of X -> Y -> Z.
        let mut bones = vec![
            AnimatedBone {
                bone: &a,
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::ZERO,
                    rotation: glam::Quat::from_euler(glam::EulerRot::ZYX, 0.3, 0.2, 0.1),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
                anim_world_transform: None,
            },
            AnimatedBone {
                bone: &b,
                anim_transform: None,
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
                anim_world_transform: None,
            },
        ];

        // Copy half of the rotation of A onto B.
        apply_hlpb_constraints(
            &mut bones,
            &HlpbData {
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
            },
        );

        assert_vector_relative_eq!(
            glam::Quat::from_euler(glam::EulerRot::ZYX, 0.15, 0.1, 0.05).to_array(),
            bones[1].anim_transform.unwrap().rotation.to_array()
        );
    }

    #[test]
    fn orient_constraints_same_parent() {
        let bone_root = identity_bone("Root", None);
        let a = identity_bone("A", Some(0));
        let b = identity_bone("B", Some(0));
        let mut bones = vec![
            AnimatedBone {
                bone: &bone_root,
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::new(0.0, 0.0, 0.0),
                    rotation: glam::Quat::from_rotation_z(0.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
                anim_world_transform: None,
            },
            AnimatedBone {
                bone: &a,
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::new(-1.0, 0.0, 0.0),
                    rotation: glam::Quat::from_rotation_z(90.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
                anim_world_transform: None,
            },
            AnimatedBone {
                bone: &b,
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::new(1.0, 0.0, 0.0),
                    rotation: glam::Quat::from_rotation_z(0.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
                anim_world_transform: None,
            },
        ];

        // Copy the rotation of A to B.
        apply_hlpb_constraints(
            &mut bones,
            &HlpbData {
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

        // Skel + Anim + Hlpb (constrain R1 to L1):
        // L2                  R2
        // ^                   ^
        // |                   |
        // L1 <-- L0    R0 --> R1
        let l0 = identity_bone("L0", None);
        let l1 = identity_bone("L1", Some(0));
        let l2 = identity_bone("L2", Some(1));
        let r0 = identity_bone("R0", None);
        let r1 = identity_bone("R1", Some(3));
        let r2 = identity_bone("R2", Some(4));
        let mut bones = vec![
            AnimatedBone {
                bone: &l0,
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::new(-1.0, 0.0, 0.0),
                    rotation: glam::Quat::from_rotation_z(90.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
                anim_world_transform: None,
            },
            AnimatedBone {
                bone: &l1,
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::new(0.0, 1.0, 0.0),
                    rotation: glam::Quat::from_rotation_z(-90.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
                anim_world_transform: None,
            },
            AnimatedBone {
                bone: &l2,
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::new(0.0, 1.0, 0.0),
                    rotation: glam::Quat::from_rotation_z(0.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
                anim_world_transform: None,
            },
            AnimatedBone {
                bone: &r0,
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::new(1.0, 0.0, 0.0),
                    rotation: glam::Quat::from_rotation_z(-90.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
                anim_world_transform: None,
            },
            AnimatedBone {
                bone: &r1,
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::new(0.0, 1.0, 0.0),
                    // TODO: What should this be without constraints?
                    rotation: glam::Quat::from_rotation_z(0.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
                anim_world_transform: None,
            },
            AnimatedBone {
                bone: &r2,
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::new(0.0, 1.0, 0.0),
                    rotation: glam::Quat::from_rotation_z(0.0f32.to_radians()),
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
                anim_world_transform: None,
            },
        ];

        // Copy the rotation of L1 to R1.
        apply_hlpb_constraints(
            &mut bones,
            &HlpbData {
                major_version: 1,
                minor_version: 0,
                aim_constraints: Vec::new().into(),
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
            },
        );

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
        assert_vector_relative_eq!(
            [0.0, 0.0, 0.7071, 0.7071],
            bones[4].anim_transform.unwrap().rotation.to_array()
        );
        assert_vector_relative_eq!(
            [0.0, 0.0, 0.0, 1.0],
            bones[5].anim_transform.unwrap().rotation.to_array()
        );

        let mut position_world = |bone| {
            world_transform(&mut bones, bone)
                .unwrap()
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
        let a = identity_bone("A", None);
        let b = identity_bone("B", None);
        let mut bones = vec![
            AnimatedBone {
                bone: &a,
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::new(0.0, 0.0, 0.0),
                    rotation: glam::Quat::IDENTITY,
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
                anim_world_transform: None,
            },
            AnimatedBone {
                bone: &b,
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::new(1.0, 0.0, 1.0),
                    rotation: glam::Quat::IDENTITY,
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
                anim_world_transform: None,
            },
        ];

        // Point the X-axis of A to B.
        apply_hlpb_constraints(
            &mut bones,
            &HlpbData {
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
            },
        );

        // A rotates along the Y-axis to point to B.
        assert_vector_relative_eq!(
            [0.0, 0.0, 0.0],
            bones[0].anim_transform.unwrap().translation.to_array()
        );
        assert_vector_relative_eq!(
            glam::Quat::from_rotation_y(-45.0f32.to_radians()).to_array(),
            bones[0].anim_transform.unwrap().rotation.to_array()
        );

        // B remains the same.
        assert_vector_relative_eq!(
            [1.0, 0.0, 1.0],
            bones[1].anim_transform.unwrap().translation.to_array()
        );
        assert_vector_relative_eq!(
            [0.0, 0.0, 0.0, 1.0],
            bones[1].anim_transform.unwrap().rotation.to_array()
        );
    }

    #[test]
    fn single_aim_constraint_missing_bones() {
        let a = identity_bone("A", None);
        let b = identity_bone("B", None);
        let mut bones = vec![
            AnimatedBone {
                bone: &a,
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::new(0.0, 0.0, 0.0),
                    rotation: glam::Quat::IDENTITY,
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
                anim_world_transform: None,
            },
            AnimatedBone {
                bone: &b,
                anim_transform: Some(AnimTransform {
                    translation: glam::Vec3::new(1.0, 0.0, 1.0),
                    rotation: glam::Quat::IDENTITY,
                    scale: glam::Vec3::ONE,
                }),
                compensate_scale: false,
                inherit_scale: false,
                flags: TransformFlags::default(),
                anim_world_transform: None,
            },
        ];

        apply_hlpb_constraints(
            &mut bones,
            &HlpbData {
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
            },
        );
    }
}
