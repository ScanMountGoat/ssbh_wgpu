use std::collections::HashSet;

use ssbh_data::{
    anim_data::{GroupType, TrackValues, TransformFlags, Vector3, Vector4},
    matl_data::MatlEntryData,
    prelude::*,
    skel_data::{BoneData, BoneTransformError},
};

use crate::{shader::skinning::AnimatedWorldTransforms, RenderMesh};

// Animation process is Skel, Anim -> Vec<AnimatedBone> -> [Mat4; 512], [Mat4; 512] -> Buffers.
// Evaluate the "tree" of Vec<AnimatedBone> to compute the final world transforms.
struct AnimatedBone {
    bone: BoneData,
    anim_transform: Option<AnimTransform>,
    compensate_scale: bool,
    inherit_scale: bool,
    flags: TransformFlags,
}

impl AnimatedBone {
    fn animated_transform(&self, include_anim_scale: bool) -> glam::Mat4 {
        self.anim_transform
            .as_ref()
            .map(|t| {
                // Decompose the default "rest" pose from the skeleton.
                // Transform flags allow some parts of the transform to be set externally.
                // For example, suppose Mario throws a different fighter like Bowser.
                // Mario's "thrown" anim needs to use some transforms from Bowser's skel.
                let (skel_scale, skel_rot, scale_trans) =
                    glam::Mat4::from_cols_array_2d(&self.bone.transform)
                        .to_scale_rotation_translation();

                let adjusted_transform = AnimTransform {
                    translation: if self.flags.override_translation {
                        scale_trans
                    } else {
                        t.translation
                    },
                    rotation: if self.flags.override_rotation {
                        skel_rot
                    } else {
                        t.rotation
                    },
                    scale: if self.flags.override_scale {
                        skel_scale
                    } else {
                        t.scale
                    },
                };

                transform_to_mat4(&adjusted_transform, include_anim_scale)
            })
            .unwrap_or_else(|| mat4_from_row2d(&self.bone.transform))
    }
}

struct AnimTransform {
    translation: glam::Vec3,
    rotation: glam::Quat,
    scale: glam::Vec3,
}

pub struct AnimationTransforms {
    // Box large arrays to avoid stack overflows in debug mode.
    /// The animated world transform of each bone relative to its resting pose.
    /// This is equal to `bone_world.inv() * animated_bone_world`.
    pub animated_world_transforms: Box<AnimatedWorldTransforms>,
    /// The world transform of each bone in the skeleton.
    pub world_transforms: Box<[glam::Mat4; 512]>,
}

impl AnimationTransforms {
    pub fn identity() -> Self {
        // We can just use the identity transform to represent no animation.
        // Mesh objects parented to a parent bone will likely be positioned at the origin.
        Self {
            animated_world_transforms: Box::new(AnimatedWorldTransforms {
                transforms: [glam::Mat4::IDENTITY; 512],
                transforms_inv_transpose: [glam::Mat4::IDENTITY; 512],
            }),
            world_transforms: Box::new([glam::Mat4::IDENTITY; 512]),
        }
    }

    pub fn from_skel(skel: &SkelData) -> Self {
        // Calculate the world transforms for parenting mesh objects to bones.
        // The skel pose should already match the "pose" in the mesh geometry.
        let mut world_transforms = [glam::Mat4::IDENTITY; 512];

        // TODO: Add tests to make sure this is transposed correctly?
        for (i, bone) in skel.bones.iter().enumerate().take(512) {
            let bone_world = skel.calculate_world_transform(bone).unwrap();
            let bone_world = glam::Mat4::from_cols_array_2d(&bone_world);
            world_transforms[i] = bone_world;
        }

        Self {
            animated_world_transforms: Box::new(AnimatedWorldTransforms {
                transforms: [glam::Mat4::IDENTITY; 512],
                transforms_inv_transpose: [glam::Mat4::IDENTITY; 512],
            }),
            world_transforms: Box::new(world_transforms),
        }
    }
}

pub trait Visibility {
    fn name(&self) -> &str;
    fn set_visibility(&mut self, visibility: bool);
}

impl Visibility for RenderMesh {
    fn name(&self) -> &str {
        &self.name
    }

    fn set_visibility(&mut self, visibility: bool) {
        self.is_visible = visibility;
    }
}

// Use tuples for testing since a RenderMesh is hard to construct.
// This also avoids needing to initialize WGPU during tests.
impl Visibility for (String, bool) {
    fn name(&self) -> &str {
        &self.0
    }

    fn set_visibility(&mut self, visibility: bool) {
        self.1 = visibility;
    }
}

// TODO: Separate module for skeletal animation?
pub fn animate_skel(skel: &SkelData, anim: &AnimData, frame: f32) -> AnimationTransforms {
    // TODO: Is this redundant with the initialization below?
    let mut world_transforms = [glam::Mat4::IDENTITY; 512];
    let mut transforms = [glam::Mat4::IDENTITY; 512];

    // TODO: Investigate optimizations for animations.
    let animated_bones: Vec<_> = skel
        .bones
        .iter()
        .map(|b| {
            find_transform(anim, b.clone(), frame).unwrap_or(AnimatedBone {
                bone: b.clone(),
                compensate_scale: false,
                inherit_scale: true,
                anim_transform: None,
                flags: TransformFlags::default(),
            })
        })
        .collect();

    // TODO: Avoid enumerate here?
    // TODO: Should scale inheritance be part of ssbh_data itself?
    // TODO: Is there a more efficient way of calculating this?
    for (i, (bone, animated_bone)) in skel
        .bones
        .iter()
        .zip(animated_bones.iter())
        .enumerate()
        .take(512)
    {
        // Smash is row-major but glam is column-major.
        // TODO: Is there an efficient way to calculate world transforms of all bones?
        // TODO: This wouldn't require a skel if there was a separate calculate_world(&[BoneData]) function.
        let bone_world = skel.calculate_world_transform(bone).unwrap();
        let bone_world = glam::Mat4::from_cols_array_2d(&bone_world).transpose();

        let bone_anim_world = animated_world_transform(&animated_bones, animated_bone).unwrap();

        // TODO: Is wgpu expecting row-major?
        transforms[i] = (bone_world.inverse() * bone_anim_world).transpose();
        world_transforms[i] = bone_anim_world.transpose();
    }

    let transforms_inv_transpose = transforms.map(|t| t.inverse().transpose());

    // TODO: Does it make more sense to use vec here?
    // This function is an implementation detail, so we can test/enforce the length.
    AnimationTransforms {
        world_transforms: Box::new(world_transforms),
        animated_world_transforms: Box::new(AnimatedWorldTransforms {
            transforms,
            transforms_inv_transpose,
        }),
    }
}

fn mat4_from_row2d(elements: &[[f32; 4]; 4]) -> glam::Mat4 {
    glam::Mat4::from_cols_array_2d(elements).transpose()
}

fn animated_world_transform(
    bones: &[AnimatedBone],
    root: &AnimatedBone,
) -> Result<glam::Mat4, BoneTransformError> {
    // TODO: Should we always include the root bone's scale?
    let mut current = root;
    let mut transform = current.animated_transform(true);

    let mut inherit_scale = current.inherit_scale;

    // Check for cycles by keeping track of previously visited locations.
    let mut visited = HashSet::new();

    // Accumulate transforms by travelling up the bone heirarchy.
    while let Some(parent_index) = current.bone.parent_index {
        // TODO: Validate the skel once for cycles to make this step faster?
        if !visited.insert(parent_index) {
            return Err(BoneTransformError::CycleDetected {
                index: parent_index,
            });
        }

        if let Some(parent_bone) = bones.get(parent_index) {
            // TODO: Does scale compensation take into account scaling in the skeleton?
            let parent_transform = parent_bone.animated_transform(inherit_scale);

            // Compensate scale only takes into account the immediate parent.
            // TODO: Test for inheritance being set.
            // TODO: Should this be current.inherit_scale instead?
            // TODO: What happens if both compensate_scale and inherit_scale are false?
            if current.compensate_scale && inherit_scale {
                if let Some(parent_transform) = &parent_bone.anim_transform {
                    let scale_compensation = glam::Mat4::from_scale(1.0 / parent_transform.scale);
                    // TODO: Make the tests more specific to account for this application order?
                    transform *= scale_compensation;
                }
            }

            transform = transform.mul_mat4(&parent_transform);
            current = parent_bone;
            // Disabling scale inheritance propogates up the bone chain.
            inherit_scale &= parent_bone.inherit_scale;
        } else {
            break;
        }
    }

    Ok(transform)
}

fn find_transform(anim: &AnimData, bone: BoneData, frame: f32) -> Option<AnimatedBone> {
    for group in &anim.groups {
        if group.group_type == GroupType::Transform {
            for node in &group.nodes {
                // TODO: Multiple nodes with the bone's name?
                if node.name == bone.name {
                    // TODO: Multiple transform tracks per bone?
                    if let Some(track) = node.tracks.first() {
                        if let TrackValues::Transform(values) = &track.values {
                            return Some(create_animated_bone(frame, bone, track, values));
                        }
                    }
                }
            }
        }
    }

    None
}

pub fn animate_visibility<V: Visibility>(anim: &AnimData, frame: f32, meshes: &mut [V]) {
    for group in &anim.groups {
        if group.group_type == GroupType::Visibility {
            for node in &group.nodes {
                if let Some(track) = node.tracks.first() {
                    // TODO: Multiple boolean tracks per node?
                    if let TrackValues::Boolean(values) = &track.values {
                        // TODO: Is this the correct way to process mesh names?
                        // TODO: Test visibility anims?
                        // TODO: Is this case sensitive?
                        // Ignore the _VIS_....
                        for mesh in meshes
                            .iter_mut()
                            .filter(|m| m.name().starts_with(&node.name))
                        {
                            // TODO: Share this between tracks?
                            let (current_frame, _, _) = frame_values(frame, track);
                            mesh.set_visibility(values[current_frame]);
                        }
                    }
                }
            }
        }
    }
}

// TODO: Add tests for this.
pub fn animate_materials(
    anim: &AnimData,
    frame: f32,
    materials: &[MatlEntryData],
) -> Vec<MatlEntryData> {
    // Return materials that were changed.
    // This avoids modifying the original materials.
    // TODO: Is this approach significantly slower than modifying in place?
    let mut changed_materials = Vec::new();

    for group in &anim.groups {
        if group.group_type == GroupType::Material {
            for node in &group.nodes {
                if let Some(material) = materials.iter().find(|m| m.material_label == node.name) {
                    // TODO: Does the speed of cloning here matter?
                    let mut changed_material = material.clone();

                    apply_material_track(node, frame, &mut changed_material);

                    changed_materials.push(changed_material);
                }
            }
        }
    }

    changed_materials
}

fn apply_material_track(
    node: &ssbh_data::anim_data::NodeData,
    frame: f32,
    changed_material: &mut MatlEntryData,
) {
    for track in &node.tracks {
        let (current_frame, _next_frame, _factor) = frame_values(frame, track);

        // TODO: Update material parameters based on the type.
        match &track.values {
            TrackValues::Transform(_) => todo!(),
            TrackValues::UvTransform(_) => {
                // TODO: UV transforms?
            }
            TrackValues::Float(v) => {
                if let Some(param) = changed_material
                    .floats
                    .iter_mut()
                    .find(|p| track.name == p.param_id.to_string())
                {
                    // TODO: Interpolate vectors?
                    param.data = v[current_frame];
                }
            }
            TrackValues::PatternIndex(_) => (),
            TrackValues::Boolean(v) => {
                if let Some(param) = changed_material
                    .booleans
                    .iter_mut()
                    .find(|p| track.name == p.param_id.to_string())
                {
                    param.data = v[current_frame];
                }
            }
            TrackValues::Vector4(v) => {
                if let Some(param) = changed_material
                    .vectors
                    .iter_mut()
                    .find(|p| track.name == p.param_id.to_string())
                {
                    // TODO: Interpolate vectors?
                    param.data = v[current_frame];
                }
            }
        }
    }
}

// TODO: Other animation group types?

fn interp_quat(a: &Vector4, b: &Vector4, factor: f32) -> glam::Quat {
    glam::Quat::from_xyzw(a.x, a.y, a.z, a.w)
        .lerp(glam::Quat::from_xyzw(b.x, b.y, b.z, b.w), factor)
}

fn interp_vec3(a: &Vector3, b: &Vector3, factor: f32) -> glam::Vec3 {
    glam::Vec3::from(a.to_array()).lerp(glam::Vec3::from(b.to_array()), factor)
}

fn transform_to_mat4(transform: &AnimTransform, include_scale: bool) -> glam::Mat4 {
    let translation = glam::Mat4::from_translation(transform.translation);

    let rotation = glam::Mat4::from_quat(transform.rotation);

    let scale = if include_scale {
        glam::Mat4::from_scale(transform.scale)
    } else {
        glam::Mat4::IDENTITY
    };

    // The application order is scale -> rotation -> translation.
    // The order is reversed here since glam is column-major.
    // TODO: Why do we transpose here?
    (translation * rotation * scale).transpose()
}

fn create_animated_bone(
    frame: f32,
    bone: BoneData,
    track: &ssbh_data::anim_data::TrackData,
    values: &[ssbh_data::anim_data::Transform],
) -> AnimatedBone {
    let (current_frame, next_frame, factor) = frame_values(frame, track);

    let current = values[current_frame];
    let next = values[next_frame];

    AnimatedBone {
        bone,
        anim_transform: Some(AnimTransform {
            translation: interp_vec3(&current.translation, &next.translation, factor),
            rotation: interp_quat(&current.rotation, &next.rotation, factor),
            scale: interp_vec3(&current.scale, &next.scale, factor),
        }),
        compensate_scale: track.scale_options.compensate_scale,
        inherit_scale: track.scale_options.inherit_scale,
        flags: track.transform_flags,
    }
}

fn frame_values(frame: f32, track: &ssbh_data::anim_data::TrackData) -> (usize, usize, f32) {
    // Force the frame to be in bounds.
    // TODO: Is this the correct way to handle single frame const animations?
    // TODO: Tests for interpolation?
    let current_frame = (frame.floor() as usize).clamp(0, track.values.len() - 1);
    let next_frame = (frame.ceil() as usize).clamp(0, track.values.len() - 1);
    // TODO: Not all animations interpolate?
    let factor = frame.fract();

    (current_frame, next_frame, factor)
}

#[cfg(test)]
mod tests {
    use ssbh_data::{
        anim_data::{GroupData, NodeData, ScaleOptions, TrackData, Transform, TransformFlags},
        skel_data::{BillboardType, BoneData},
    };

    use super::*;

    use crate::assert_matrix_relative_eq;

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
    fn animation_transforms_from_skel_512_bones() {
        AnimationTransforms::from_skel(&SkelData {
            major_version: 1,
            minor_version: 0,
            bones: vec![identity_bone("A", None); 512],
        });
    }

    #[test]
    fn animation_transforms_from_skel_600_bones() {
        // Make sure that this doesn't panic.
        AnimationTransforms::from_skel(&SkelData {
            major_version: 1,
            minor_version: 0,
            bones: vec![identity_bone("A", None); 600],
        });
    }

    // TODO: Cycle detection in the skeleton?
    // TODO: Validate the skeleton and convert to a new type?
    // TODO: Out of range frame indices (negative, too large, etc)
    // TODO: Interpolation behavior

    #[test]
    fn apply_empty_animation_512_bones() {
        // TODO: Should this enforce the limit in Smash Ultimate of 511 instead?
        animate_skel(
            &SkelData {
                major_version: 1,
                minor_version: 0,
                bones: vec![identity_bone("A", None); 512],
            },
            &AnimData {
                major_version: 2,
                minor_version: 0,
                final_frame_index: 0.0,
                groups: Vec::new(),
            },
            0.0,
        );
    }

    #[test]
    fn apply_empty_animation_too_many_bones() {
        // TODO: Should this be an error?
        animate_skel(
            &SkelData {
                major_version: 1,
                minor_version: 0,
                bones: vec![identity_bone("A", None); 600],
            },
            &AnimData {
                major_version: 2,
                minor_version: 0,
                final_frame_index: 0.0,
                groups: Vec::new(),
            },
            0.0,
        );
    }

    #[test]
    fn apply_empty_animation_no_bones() {
        animate_skel(
            &SkelData {
                major_version: 1,
                minor_version: 0,
                bones: Vec::new(),
            },
            &AnimData {
                major_version: 2,
                minor_version: 0,
                final_frame_index: 0.0,
                groups: Vec::new(),
            },
            0.0,
        );
    }

    #[test]
    fn apply_animation_single_animated_bone() {
        // Check that the appropriate bones are set.
        // Check the construction of transformation matrices.
        let transforms = animate_skel(
            &SkelData {
                major_version: 1,
                minor_version: 0,
                bones: vec![identity_bone("A", None)],
            },
            &AnimData {
                major_version: 2,
                minor_version: 0,
                final_frame_index: 0.0,
                groups: vec![GroupData {
                    group_type: GroupType::Transform,
                    nodes: vec![NodeData {
                        name: "A".to_string(),
                        tracks: vec![TrackData {
                            name: "Transform".to_string(),
                            scale_options: ScaleOptions::default(),
                            values: TrackValues::Transform(vec![Transform {
                                scale: Vector3::new(1.0, 2.0, 3.0),
                                rotation: Vector4::new(1.0, 0.0, 0.0, 0.0),
                                translation: Vector3::new(4.0, 5.0, 6.0),
                            }]),
                            transform_flags: TransformFlags::default(),
                        }],
                    }],
                }],
            },
            0.0,
        );

        // TODO: Test the unused indices?
        // TODO: No transpose?
        assert_matrix_relative_eq!(
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, -2.0, 0.0, 0.0],
                [0.0, 0.0, -3.0, 0.0],
                [4.0, 5.0, 6.0, 1.0],
            ],
            transforms.animated_world_transforms.transforms[0]
                // .transpose()
                .to_cols_array_2d()
        );
        assert_matrix_relative_eq!(
            [
                [1.0 / 1.0, 0.0, 0.0, 4.0 / -1.0],
                [0.0, -1.0 / 2.0, 0.0, 5.0 / 2.0],
                [0.0, 0.0, -1.0 / 3.0, 6.0 / 3.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            transforms
                .animated_world_transforms
                .transforms_inv_transpose[0]
                // .transpose()
                .to_cols_array_2d()
        );
        assert_matrix_relative_eq!(
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, -2.0, 0.0, 0.0],
                [0.0, 0.0, -3.0, 0.0],
                [4.0, 5.0, 6.0, 1.0],
            ],
            transforms.world_transforms[0]
                // .transpose()
                .to_cols_array_2d()
        );
    }

    #[test]
    fn apply_animation_bone_chain_inherit_scale() {
        // Include parent scale up the chain until a bone disables inheritance.
        let transforms = animate_skel(
            &SkelData {
                major_version: 1,
                minor_version: 0,
                bones: vec![
                    identity_bone("A", None),
                    identity_bone("B", Some(0)),
                    identity_bone("C", Some(1)),
                ],
            },
            &AnimData {
                major_version: 2,
                minor_version: 0,
                final_frame_index: 0.0,
                groups: vec![GroupData {
                    group_type: GroupType::Transform,
                    nodes: vec![
                        NodeData {
                            name: "A".to_string(),
                            tracks: vec![TrackData {
                                name: "Transform".to_string(),
                                scale_options: ScaleOptions {
                                    inherit_scale: true,
                                    compensate_scale: false,
                                },
                                values: TrackValues::Transform(vec![Transform {
                                    scale: Vector3::new(1.0, 2.0, 3.0),
                                    rotation: Vector4::new(0.0, 0.0, 0.0, 1.0),
                                    translation: Vector3::new(0.0, 0.0, 0.0),
                                }]),
                                transform_flags: TransformFlags::default(),
                            }],
                        },
                        NodeData {
                            name: "B".to_string(),
                            tracks: vec![TrackData {
                                name: "Transform".to_string(),
                                scale_options: ScaleOptions {
                                    inherit_scale: false,
                                    compensate_scale: false,
                                },
                                values: TrackValues::Transform(vec![Transform {
                                    scale: Vector3::new(1.0, 2.0, 3.0),
                                    rotation: Vector4::new(0.0, 0.0, 0.0, 1.0),
                                    translation: Vector3::new(0.0, 0.0, 0.0),
                                }]),
                                transform_flags: TransformFlags::default(),
                            }],
                        },
                        NodeData {
                            name: "C".to_string(),
                            tracks: vec![TrackData {
                                name: "Transform".to_string(),
                                scale_options: ScaleOptions {
                                    inherit_scale: true,
                                    compensate_scale: false,
                                },
                                values: TrackValues::Transform(vec![Transform {
                                    scale: Vector3::new(1.0, 2.0, 3.0),
                                    rotation: Vector4::new(0.0, 0.0, 0.0, 1.0),
                                    translation: Vector3::new(0.0, 0.0, 0.0),
                                }]),
                                transform_flags: TransformFlags::default(),
                            }],
                        },
                    ],
                }],
            },
            0.0,
        );

        // TODO: No transpose?
        assert_matrix_relative_eq!(
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 2.0, 0.0, 0.0],
                [0.0, 0.0, 3.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            transforms.animated_world_transforms.transforms[0].to_cols_array_2d()
        );
        assert_matrix_relative_eq!(
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 2.0, 0.0, 0.0],
                [0.0, 0.0, 3.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            transforms.animated_world_transforms.transforms[1].to_cols_array_2d()
        );
        assert_matrix_relative_eq!(
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 4.0, 0.0, 0.0],
                [0.0, 0.0, 9.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            transforms.animated_world_transforms.transforms[2].to_cols_array_2d()
        );
        // TODO: Test other matrices?
    }

    #[test]
    fn apply_animation_bone_chain_no_inherit_scale() {
        // Test if the root bone doesn't inherit scale.
        let transforms = animate_skel(
            &SkelData {
                major_version: 1,
                minor_version: 0,
                bones: vec![
                    identity_bone("A", None),
                    identity_bone("B", Some(0)),
                    identity_bone("C", Some(1)),
                ],
            },
            &AnimData {
                major_version: 2,
                minor_version: 0,
                final_frame_index: 0.0,
                groups: vec![GroupData {
                    group_type: GroupType::Transform,
                    nodes: vec![
                        NodeData {
                            name: "A".to_string(),
                            tracks: vec![TrackData {
                                name: "Transform".to_string(),
                                scale_options: ScaleOptions {
                                    inherit_scale: true,
                                    compensate_scale: false,
                                },
                                values: TrackValues::Transform(vec![Transform {
                                    scale: Vector3::new(1.0, 2.0, 3.0),
                                    rotation: Vector4::new(0.0, 0.0, 0.0, 1.0),
                                    translation: Vector3::new(0.0, 0.0, 0.0),
                                }]),
                                transform_flags: TransformFlags::default(),
                            }],
                        },
                        NodeData {
                            name: "B".to_string(),
                            tracks: vec![TrackData {
                                name: "Transform".to_string(),
                                scale_options: ScaleOptions {
                                    inherit_scale: true,
                                    compensate_scale: false,
                                },
                                values: TrackValues::Transform(vec![Transform {
                                    scale: Vector3::new(1.0, 2.0, 3.0),
                                    rotation: Vector4::new(0.0, 0.0, 0.0, 1.0),
                                    translation: Vector3::new(0.0, 0.0, 0.0),
                                }]),
                                transform_flags: TransformFlags::default(),
                            }],
                        },
                        NodeData {
                            name: "C".to_string(),
                            tracks: vec![TrackData {
                                name: "Transform".to_string(),
                                scale_options: ScaleOptions {
                                    inherit_scale: false,
                                    compensate_scale: false,
                                },
                                values: TrackValues::Transform(vec![Transform {
                                    scale: Vector3::new(1.0, 2.0, 3.0),
                                    rotation: Vector4::new(0.0, 0.0, 0.0, 1.0),
                                    translation: Vector3::new(0.0, 0.0, 0.0),
                                }]),
                                transform_flags: TransformFlags::default(),
                            }],
                        },
                    ],
                }],
            },
            0.0,
        );

        // TODO: No transpose?
        assert_matrix_relative_eq!(
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 2.0, 0.0, 0.0],
                [0.0, 0.0, 3.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            transforms.animated_world_transforms.transforms[0].to_cols_array_2d()
        );
        assert_matrix_relative_eq!(
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 4.0, 0.0, 0.0],
                [0.0, 0.0, 9.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            transforms.animated_world_transforms.transforms[1].to_cols_array_2d()
        );
        assert_matrix_relative_eq!(
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 2.0, 0.0, 0.0],
                [0.0, 0.0, 3.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            transforms.animated_world_transforms.transforms[2].to_cols_array_2d()
        );
        // TODO: Test other matrices?
    }

    #[test]
    fn apply_animation_bone_chain_compensate_scale() {
        // Test an entire chain with scale compensation.
        let transforms = animate_skel(
            &SkelData {
                major_version: 1,
                minor_version: 0,
                bones: vec![
                    identity_bone("A", None),
                    identity_bone("B", Some(0)),
                    identity_bone("C", Some(1)),
                ],
            },
            &AnimData {
                major_version: 2,
                minor_version: 0,
                final_frame_index: 0.0,
                groups: vec![GroupData {
                    group_type: GroupType::Transform,
                    nodes: vec![
                        NodeData {
                            name: "A".to_string(),
                            tracks: vec![TrackData {
                                name: "Transform".to_string(),
                                scale_options: ScaleOptions {
                                    inherit_scale: true,
                                    compensate_scale: true,
                                },
                                values: TrackValues::Transform(vec![Transform {
                                    scale: Vector3::new(1.0, 2.0, 3.0),
                                    rotation: Vector4::new(0.0, 0.0, 0.0, 1.0),
                                    translation: Vector3::new(0.0, 0.0, 0.0),
                                }]),
                                transform_flags: TransformFlags::default(),
                            }],
                        },
                        NodeData {
                            name: "B".to_string(),
                            tracks: vec![TrackData {
                                name: "Transform".to_string(),
                                scale_options: ScaleOptions {
                                    inherit_scale: true,
                                    compensate_scale: true,
                                },
                                values: TrackValues::Transform(vec![Transform {
                                    scale: Vector3::new(1.0, 2.0, 3.0),
                                    rotation: Vector4::new(0.0, 0.0, 0.0, 1.0),
                                    translation: Vector3::new(0.0, 0.0, 0.0),
                                }]),
                                transform_flags: TransformFlags::default(),
                            }],
                        },
                        NodeData {
                            name: "C".to_string(),
                            tracks: vec![TrackData {
                                name: "Transform".to_string(),
                                scale_options: ScaleOptions {
                                    inherit_scale: true,
                                    compensate_scale: true,
                                },
                                values: TrackValues::Transform(vec![Transform {
                                    scale: Vector3::new(1.0, 2.0, 3.0),
                                    rotation: Vector4::new(0.0, 0.0, 0.0, 1.0),
                                    translation: Vector3::new(0.0, 0.0, 0.0),
                                }]),
                                transform_flags: TransformFlags::default(),
                            }],
                        },
                    ],
                }],
            },
            0.0,
        );

        // TODO: No transpose?
        assert_matrix_relative_eq!(
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 2.0, 0.0, 0.0],
                [0.0, 0.0, 3.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            transforms.animated_world_transforms.transforms[0].to_cols_array_2d()
        );
        assert_matrix_relative_eq!(
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 2.0, 0.0, 0.0],
                [0.0, 0.0, 3.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            transforms.animated_world_transforms.transforms[1].to_cols_array_2d()
        );
        assert_matrix_relative_eq!(
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 2.0, 0.0, 0.0],
                [0.0, 0.0, 3.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            transforms.animated_world_transforms.transforms[2].to_cols_array_2d()
        );
        // TODO: Test other matrices?
    }

    // TODO: Test additional TransformFlags combinations.
    #[test]
    fn apply_animation_bone_chain_override_transforms() {
        // Test resetting all transforms to their "resting" pose from the skel.
        let transforms = animate_skel(
            &SkelData {
                major_version: 1,
                minor_version: 0,
                bones: vec![
                    // TODO: Don't use the identity here to make the test stricter?
                    identity_bone("A", None),
                    identity_bone("B", Some(0)),
                    identity_bone("C", Some(1)),
                ],
            },
            &AnimData {
                major_version: 2,
                minor_version: 0,
                final_frame_index: 0.0,
                groups: vec![GroupData {
                    group_type: GroupType::Transform,
                    nodes: vec![
                        NodeData {
                            name: "A".to_string(),
                            tracks: vec![TrackData {
                                name: "Transform".to_string(),
                                scale_options: ScaleOptions {
                                    inherit_scale: true,
                                    compensate_scale: false,
                                },
                                values: TrackValues::Transform(vec![Transform {
                                    scale: Vector3::new(2.0, 2.0, 2.0),
                                    rotation: Vector4::new(1.0, 0.0, 0.0, 0.0),
                                    translation: Vector3::new(0.0, 1.0, 2.0),
                                }]),
                                transform_flags: TransformFlags {
                                    override_translation: true,
                                    override_rotation: true,
                                    override_scale: true,
                                },
                            }],
                        },
                        NodeData {
                            name: "B".to_string(),
                            tracks: vec![TrackData {
                                name: "Transform".to_string(),
                                scale_options: ScaleOptions {
                                    inherit_scale: true,
                                    compensate_scale: false,
                                },
                                values: TrackValues::Transform(vec![Transform {
                                    scale: Vector3::new(2.0, 2.0, 2.0),
                                    rotation: Vector4::new(1.0, 0.0, 0.0, 0.0),
                                    translation: Vector3::new(0.0, 1.0, 2.0),
                                }]),
                                transform_flags: TransformFlags {
                                    override_translation: true,
                                    override_rotation: true,
                                    override_scale: true,
                                },
                            }],
                        },
                        NodeData {
                            name: "C".to_string(),
                            tracks: vec![TrackData {
                                name: "Transform".to_string(),
                                scale_options: ScaleOptions {
                                    inherit_scale: true,
                                    compensate_scale: false,
                                },
                                values: TrackValues::Transform(vec![Transform {
                                    scale: Vector3::new(2.0, 2.0, 2.0),
                                    rotation: Vector4::new(1.0, 0.0, 0.0, 0.0),
                                    translation: Vector3::new(0.0, 1.0, 2.0),
                                }]),
                                transform_flags: TransformFlags {
                                    override_translation: true,
                                    override_rotation: true,
                                    override_scale: true,
                                },
                            }],
                        },
                    ],
                }],
            },
            0.0,
        );

        // TODO: No transpose?
        assert_matrix_relative_eq!(
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            transforms.animated_world_transforms.transforms[0].to_cols_array_2d()
        );
        assert_matrix_relative_eq!(
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            transforms.animated_world_transforms.transforms[1].to_cols_array_2d()
        );
        assert_matrix_relative_eq!(
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            transforms.animated_world_transforms.transforms[2].to_cols_array_2d()
        );
        // TODO: Test other matrices?
    }

    #[test]
    fn apply_animation_visibility() {
        // Test that the _VIS tags are ignored in name handling.
        let mut meshes = vec![
            ("A_VIS_O_OBJSHAPE".to_string(), true),
            ("B_VIS_O_OBJSHAPE".to_string(), false),
            ("C_VIS_O_OBJSHAPE".to_string(), true),
        ];

        animate_visibility(
            &AnimData {
                major_version: 2,
                minor_version: 0,
                final_frame_index: 0.0,
                groups: vec![GroupData {
                    group_type: GroupType::Visibility,
                    nodes: vec![
                        NodeData {
                            name: "A".to_string(),
                            tracks: vec![TrackData {
                                name: "Visibility".to_string(),
                                scale_options: ScaleOptions::default(),
                                values: TrackValues::Boolean(vec![true, false, true]),
                                transform_flags: TransformFlags::default(),
                            }],
                        },
                        NodeData {
                            name: "B".to_string(),
                            tracks: vec![TrackData {
                                name: "Visibility".to_string(),
                                scale_options: ScaleOptions::default(),
                                values: TrackValues::Boolean(vec![false, true, false]),
                                transform_flags: TransformFlags::default(),
                            }],
                        },
                    ],
                }],
            },
            1.0,
            &mut meshes,
        );

        assert_eq!(false, meshes[0].1);
        assert_eq!(true, meshes[1].1);
        // The third mesh should be unchanged.
        assert_eq!(true, meshes[2].1);
    }
}
