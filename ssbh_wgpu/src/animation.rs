use ssbh_data::{anim_data::TrackValues, prelude::*};

use crate::shader::skinning::Transforms;

pub struct AnimationTransforms {
    // Box large arrays to avoid stack overflows in debug mode.
    pub transforms: Box<Transforms>,
    pub world_transforms: Box<[glam::Mat4; 512]>,
}

// TODO: How to test this?
// TODO: Create test cases for test animations on short bone chains?
pub fn apply_animation(skel: &SkelData, anim: &AnimData, frame: f32) -> AnimationTransforms {
    // TODO: Make the skel optional?

    // TODO: Is this redundant?
    let mut anim_world_transforms = [glam::Mat4::IDENTITY; 512];
    let mut transforms = [glam::Mat4::IDENTITY; 512];

    // HACK: Duplicate the bone heirachy and override with the anim nodes.
    // A proper solution will take into account scaling, interpolation, etc.
    // This should be its own module or potentially a separate package.
    // TODO: There's probably an efficient execution order using the heirarchy.
    // There might not be enough bones to benefit from parallel execution.
    // We won't worry about redundant matrix multiplications for now.
    let mut animated_skel = skel.clone();
    for group in &anim.groups {
        for node in &group.nodes {
            if let Some(track) = node.tracks.first() {
                if let TrackValues::Transform(values) = &track.values {
                    // TODO: Set the frame/interpolation?
                    // TODO: Faster to use SIMD types?
                    if let Some(bone) = animated_skel.bones.iter_mut().find(|b| b.name == node.name)
                    {
                        // Force the frame to be in bounds.
                        // TODO: Is this the correct way to handle single frame const animations?
                        // TODO: Tests for interpolation?
                        let current_frame =
                            (frame.floor() as usize).clamp(0, track.values.len() - 1);
                        let next_frame = (frame.ceil() as usize).clamp(0, track.values.len() - 1);

                        // TODO: Not all animations interpolate?
                        let factor = frame.fract();

                        let current = values[current_frame];
                        let next = values[next_frame];

                        let translation = glam::Mat4::from_translation(
                            glam::Vec3::from(current.translation.to_array())
                                .lerp(glam::Vec3::from(next.translation.to_array()), factor),
                        );
                        let rotation = glam::Mat4::from_quat(
                            glam::Quat::from_xyzw(
                                current.rotation.x,
                                current.rotation.y,
                                current.rotation.z,
                                current.rotation.w,
                            )
                            .lerp(
                                glam::Quat::from_xyzw(
                                    next.rotation.x,
                                    next.rotation.y,
                                    next.rotation.z,
                                    next.rotation.w,
                                ),
                                factor,
                            ),
                        );
                        let anim_transform = translation * rotation;

                        // TODO: Why do we not transpose here?
                        bone.transform = anim_transform.to_cols_array_2d();
                    }
                }
            }
        }
    }

    for (i, bone) in skel.bones.iter().enumerate() {
        // Smash is row-major but glam is column-major.
        let bone_world = skel.calculate_world_transform(bone).unwrap();
        let bone_world = glam::Mat4::from_cols_array_2d(&bone_world).transpose();

        // TODO: Apply animations?
        // TODO: Separate module with tests to check edge cases.

        let bone_anim_world = animated_skel
            .calculate_world_transform(
                animated_skel
                    .bones
                    .iter()
                    .find(|b| b.name == bone.name)
                    .unwrap(),
            )
            .unwrap();
        let bone_anim_world = glam::Mat4::from_cols_array_2d(&bone_anim_world).transpose();

        // TODO: Is wgpu expecting row-major?
        transforms[i] = (bone_world.inverse() * bone_anim_world).transpose();
        anim_world_transforms[i] = bone_anim_world.transpose();
    }

    let transforms_inv_transpose = transforms.map(|t| t.inverse().transpose());

    AnimationTransforms {
        world_transforms: Box::new(anim_world_transforms),
        transforms: Box::new(Transforms {
            transforms,
            transforms_inv_transpose,
        }),
    }
}
