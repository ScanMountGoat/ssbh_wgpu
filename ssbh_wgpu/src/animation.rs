use self::constraints::{apply_aim_constraint, apply_orient_constraint};
use crate::{shader::skinning::AnimatedWorldTransforms, RenderMesh};
use ssbh_data::{
    anim_data::{GroupType, TrackValues, TransformFlags},
    matl_data::MatlEntryData,
    prelude::*,
    skel_data::BoneData,
    Vector3, Vector4,
};

mod constraints;

/// The maximum number of bones supported by the shader's uniform buffer.
pub const MAX_BONE_COUNT: usize = 512;

// Animation process is Skel, Anim -> Vec<AnimatedBone> -> [Mat4; 512], [Mat4; 512] -> Buffers.
// Evaluate the "tree" of Vec<AnimatedBone> to compute the final world transforms.
#[derive(Debug, Clone)]
pub struct AnimatedBone<'a> {
    bone: &'a BoneData,
    anim_transform: Option<AnimTransform>,
    compensate_scale: bool,
    flags: TransformFlags,
}

impl<'a> AnimatedBone<'a> {
    fn animated_transform(&self, scale_compensation: glam::Vec3) -> glam::Mat4 {
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

                adjusted_transform.to_mat4(scale_compensation)
            })
            .unwrap_or_else(|| glam::Mat4::from_cols_array_2d(&self.bone.transform))
    }

    fn transform(&self) -> glam::Mat4 {
        glam::Mat4::from_cols_array_2d(&self.bone.transform)
    }
}

#[derive(Debug, Clone, Copy)]
struct AnimTransform {
    translation: glam::Vec3,
    rotation: glam::Quat,
    scale: glam::Vec3,
}

impl AnimTransform {
    fn to_mat4(self, scale_compensation: glam::Vec3) -> glam::Mat4 {
        let translation = glam::Mat4::from_translation(self.translation);
        let rotation = glam::Mat4::from_quat(self.rotation);
        let scale = glam::Mat4::from_scale(self.scale);
        // The application order is scale -> rotation -> compensation -> translation.
        // The order is reversed here since glam is column-major.
        translation * glam::Mat4::from_scale(scale_compensation) * rotation * scale
    }
}

pub struct AnimationTransforms {
    // TODO: Use a better name to indicate that this is relative to the resting pose.
    /// The animated world transform of each bone relative to its resting pose.
    /// This is equal to `bone_world.inv() * animated_bone_world`.
    pub animated_world_transforms: AnimatedWorldTransforms,
    /// The world transform of each bone in the skeleton.
    // TODO: This name is confusing since it's still animated rather than using the rest pose.
    pub world_transforms: [glam::Mat4; MAX_BONE_COUNT],
}

impl AnimationTransforms {
    pub fn identity() -> Self {
        // We can just use the identity transform to represent no animation.
        // Mesh objects parented to a parent bone will likely be positioned at the origin.
        Self {
            animated_world_transforms: AnimatedWorldTransforms {
                transforms: [glam::Mat4::IDENTITY; MAX_BONE_COUNT],
                transforms_inv_transpose: [glam::Mat4::IDENTITY; MAX_BONE_COUNT],
            },
            world_transforms: [glam::Mat4::IDENTITY; MAX_BONE_COUNT],
        }
    }

    pub fn from_skel(skel: &SkelData) -> Self {
        // Calculate the transforms to use before animations are applied.
        // Calculate the world transforms for parenting mesh objects to bones.
        // The skel pose should already match the "pose" in the mesh geometry.
        let mut world_transforms = [glam::Mat4::IDENTITY; MAX_BONE_COUNT];

        // TODO: Add tests to make sure this is transposed correctly?
        for (i, bone) in skel.bones.iter().enumerate().take(MAX_BONE_COUNT) {
            // TODO: Return an error instead?
            let bone_world = skel
                .calculate_world_transform(bone)
                .map(|t| glam::Mat4::from_cols_array_2d(&t))
                .unwrap_or(glam::Mat4::IDENTITY);

            world_transforms[i] = bone_world;
        }

        Self {
            animated_world_transforms: AnimatedWorldTransforms {
                transforms: [glam::Mat4::IDENTITY; MAX_BONE_COUNT],
                transforms_inv_transpose: [glam::Mat4::IDENTITY; MAX_BONE_COUNT],
            },
            world_transforms,
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

// Take a reference to the transforms to avoid repeating large allocations.
// TODO: Separate module for skeletal animation?
// TODO: Benchmarks for criterion.rs that test performance scaling with bone and constraint count.
pub fn animate_skel<'a>(
    result: &mut AnimationTransforms,
    skel: &SkelData,
    anims: impl Iterator<Item = &'a AnimData>,
    hlpb: Option<&HlpbData>,
    frame: f32,
    should_loop: bool,
) {
    // TODO: Avoid allocating here?
    // TODO: Just take the bones or groups directly?
    let mut bones: Vec<_> = skel
        .bones
        .iter()
        .enumerate()
        .take(MAX_BONE_COUNT)
        .map(|(i, b)| {
            (
                i,
                AnimatedBone {
                    bone: b,
                    compensate_scale: false,
                    anim_transform: None,
                    flags: TransformFlags::default(),
                },
            )
        })
        .collect();

    // TODO: Is it faster to use a separate array for animation info?
    for anim in anims {
        // Assume final_frame_index is set to the length of the longest track.
        // Check for 0 length animations to avoid a potential NaN.
        let current_frame = if should_loop && anim.final_frame_index > 0.0 {
            frame.rem_euclid(anim.final_frame_index)
        } else {
            frame
        };
        apply_transforms(&mut bones, anim, current_frame);
    }

    animate_skel_inner(result, &mut bones, &skel.bones, hlpb);
}

pub fn animate_skel_inner(
    result: &mut AnimationTransforms,
    bones: &mut Vec<(usize, AnimatedBone)>,
    skel_bones: &[BoneData],
    hlpb: Option<&HlpbData>,
) {
    // TODO: Add tests for evaluation order.
    // The parent-child relationship determines the evaluation order.
    // The order is partial since only a parent and child bone are comparable.
    // We need a topological sort instead of a regular sort to enforce these dependencies.
    // TODO: Is there a cleaner way of doing this?
    // TODO: What is the performance impact of this?
    let mut topo_sort = topological_sort::TopologicalSort::<usize>::new();
    let mut evaluation_order = Vec::new();
    for (i, bone) in bones.iter() {
        if let Some(p) = bone.bone.parent_index {
            topo_sort.add_dependency(p, *i);
        } else {
            // Root bones won't be part of the dependency graph.
            // Add them manually to ensure all bones get evaluated.
            evaluation_order.push(*i);
        }
    }
    // TODO: Cycle checking?
    loop {
        let parts = topo_sort.pop_all();
        if parts.is_empty() {
            break;
        }

        evaluation_order.extend(parts);
    }

    // Assume parents always appear before their children.
    // This partial order respects dependencies, so bones can be iterated exactly once.
    // TODO: Can this be safely combined with the loop below?
    // TODO: Limit the amount of stack space this function needs.
    let mut bone_inv_world = [glam::Mat4::IDENTITY; MAX_BONE_COUNT];
    for i in &evaluation_order {
        let bone = &bones[*i];
        if let Some(parent_index) = bone.1.bone.parent_index {
            bone_inv_world[bone.0] = bone_inv_world[parent_index] * bone.1.transform();
        } else {
            bone_inv_world[bone.0] = bone.1.transform();
        }
    }
    for transform in &mut bone_inv_world {
        *transform = transform.inverse();
    }

    // Evaluate the world transforms first without constraints.
    // This solves some issues where the constraint source bone hasn't been evaluated yet.
    // TODO: Do constraints impact the evaluation order in game?
    // TODO: How to handle cyclic dependencies due to constraining bones to each other?
    for i in &evaluation_order {
        let bone = &bones[*i];
        let (parent_world, current) = calculate_world_transform(bones, &bone.1, result);
        result.world_transforms[bone.0] = parent_world * current;
    }

    for i in &evaluation_order {
        let bone = &bones[*i];
        let (parent_world, mut current) = calculate_world_transform(bones, &bone.1, result);

        if let Some(hlpb) = hlpb {
            apply_constraints(&mut current, hlpb, bone, result, skel_bones);
        }

        result.world_transforms[bone.0] = parent_world * current;
    }

    // TODO: Does constraining a bone affects the world transforms of its children?
    // TODO: Can we apply constraints after world transforms and avoid updating affected children?
    // TODO: How does the game handle circular dependencies from hlpb constraints?
    for i in (0..bones.len()).take(MAX_BONE_COUNT) {
        let anim_transform = result.world_transforms[i] * bone_inv_world[i];

        result.animated_world_transforms.transforms[i] = anim_transform;
        result.animated_world_transforms.transforms_inv_transpose[i] =
            anim_transform.inverse().transpose();
    }
}

fn apply_constraints(
    current: &mut glam::Mat4,
    hlpb: &HlpbData,
    bone: &(usize, AnimatedBone),
    result: &AnimationTransforms,
    bones: &[BoneData],
) {
    if let Some(constraint) = hlpb
        .orient_constraints
        .iter()
        .find(|o| o.target_bone_name == bone.1.bone.name)
    {
        if let Some(new_current) =
            apply_orient_constraint(&result.world_transforms, bones, constraint, *current)
        {
            *current = new_current;
        }
    }
    if let Some(constraint) = hlpb
        .aim_constraints
        .iter()
        .find(|a| a.target_bone_name1 == bone.1.bone.name)
    {
        if let Some(new_current) =
            apply_aim_constraint(&result.world_transforms, bones, constraint, *current)
        {
            *current = new_current;
        }
    }
}

fn calculate_world_transform(
    bones: &[(usize, AnimatedBone)],
    bone: &AnimatedBone,
    result: &AnimationTransforms,
) -> (glam::Mat4, glam::Mat4) {
    if let Some(parent_index) = bone.bone.parent_index {
        // TODO: Avoid potential indexing panics.
        let parent_transform = result.world_transforms[parent_index];

        // TODO: How to handle !inherit_scale && !compensate_scale?
        // TODO: Double check ScaleType for CompressionFlags.
        // The current implementation doesn't need to check inheritance, which seems odd.
        let scale_compensation = if bone.compensate_scale {
            // Compensate scale uses the parent's non accumulated scale.
            // TODO: How to handle the case where the parent isn't animated?
            let parent_scale = bones[parent_index]
                .1
                .anim_transform
                .map(|t| t.scale)
                .unwrap_or(glam::Vec3::ONE);
            1.0 / parent_scale
        } else {
            glam::Vec3::ONE
        };

        let current_transform = bone.animated_transform(scale_compensation);
        (parent_transform, current_transform)
    } else {
        (
            glam::Mat4::IDENTITY,
            bone.animated_transform(glam::Vec3::ONE),
        )
    }
}

fn apply_transforms<'a>(
    bones: &mut [(usize, AnimatedBone)],
    anim: &AnimData,
    frame: f32,
) -> Option<AnimatedBone<'a>> {
    for group in &anim.groups {
        if group.group_type == GroupType::Transform {
            for node in &group.nodes {
                // TODO: Multiple nodes with the bone's name?
                if let Some((_, bone)) = bones.iter_mut().find(|(_, b)| b.bone.name == node.name) {
                    // TODO: Multiple transform tracks per bone?
                    if let Some(track) = node.tracks.first() {
                        if let TrackValues::Transform(values) = &track.values {
                            *bone = create_animated_bone(frame, bone.bone, track, values);
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
    // Avoid modifying the original materials.
    // TODO: Iterate instead to avoid allocating?
    // TODO: Is this approach significantly slower than modifying in place?
    let mut changed_materials = materials.to_vec();

    for group in &anim.groups {
        if group.group_type == GroupType::Material {
            for node in &group.nodes {
                if let Some(material) = changed_materials
                    .iter_mut()
                    .find(|m| m.material_label == node.name)
                {
                    apply_material_track(node, frame, material);
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
    // TODO: Faster to use Vec3A?
    glam::Vec3::from(a.to_array()).lerp(glam::Vec3::from(b.to_array()), factor)
}

fn create_animated_bone<'a>(
    frame: f32,
    bone: &'a BoneData,
    track: &ssbh_data::anim_data::TrackData,
    values: &[ssbh_data::anim_data::Transform],
) -> AnimatedBone<'a> {
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
        compensate_scale: track.compensate_scale, // TODO: override compensate scale?
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
        anim_data::{GroupData, NodeData, TrackData, Transform, TransformFlags},
        hlpb_data::OrientConstraintData,
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
    fn anim_transform_to_mat4_compensation() {
        assert_matrix_relative_eq!(
            [
                [0.55, 0.0, 0.0, 0.0],
                [0.0, 1.7881394e-8, 0.59999996, 0.0],
                [0.0, -0.32499996, 3.8743018e-8, 0.0],
                [1.0, 2.0, 3.0, 1.0]
            ],
            AnimTransform {
                translation: glam::vec3(1.0, 2.0, 3.0),
                rotation: glam::Quat::from_rotation_x(90.0f32.to_radians()),
                scale: glam::vec3(1.1, 1.2, 1.3),
            }
            .to_mat4(glam::vec3(0.5, 0.25, 0.5))
            .to_cols_array_2d()
        );
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
            &mut AnimationTransforms::identity(),
            &SkelData {
                major_version: 1,
                minor_version: 0,
                bones: vec![identity_bone("A", None); 512],
            },
            [AnimData {
                major_version: 2,
                minor_version: 0,
                final_frame_index: 0.0,
                groups: Vec::new(),
            }]
            .iter(),
            None,
            0.0,
            true,
        );
    }

    #[test]
    fn apply_empty_animation_too_many_bones() {
        // TODO: Should this be an error?
        animate_skel(
            &mut AnimationTransforms::identity(),
            &SkelData {
                major_version: 1,
                minor_version: 0,
                bones: vec![identity_bone("A", None); 600],
            },
            [AnimData {
                major_version: 2,
                minor_version: 0,
                final_frame_index: 0.0,
                groups: Vec::new(),
            }]
            .iter(),
            None,
            0.0,
            true,
        );
    }

    #[test]
    fn apply_empty_animation_no_bones() {
        animate_skel(
            &mut AnimationTransforms::identity(),
            &SkelData {
                major_version: 1,
                minor_version: 0,
                bones: Vec::new(),
            },
            [AnimData {
                major_version: 2,
                minor_version: 0,
                final_frame_index: 0.0,
                groups: Vec::new(),
            }]
            .iter(),
            None,
            0.0,
            true,
        );
    }

    #[test]
    fn apply_animation_single_animated_bone() {
        // Check that the appropriate bones are set.
        // Check the construction of transformation matrices.
        let mut transforms = AnimationTransforms::identity();
        animate_skel(
            &mut transforms,
            &SkelData {
                major_version: 1,
                minor_version: 0,
                bones: vec![identity_bone("A", None)],
            },
            [AnimData {
                major_version: 2,
                minor_version: 0,
                final_frame_index: 0.0,
                groups: vec![GroupData {
                    group_type: GroupType::Transform,
                    nodes: vec![NodeData {
                        name: "A".to_string(),
                        tracks: vec![TrackData {
                            name: "Transform".to_string(),
                            compensate_scale: false,
                            values: TrackValues::Transform(vec![Transform {
                                scale: Vector3::new(1.0, 2.0, 3.0),
                                rotation: Vector4::new(1.0, 0.0, 0.0, 0.0),
                                translation: Vector3::new(4.0, 5.0, 6.0),
                            }]),
                            transform_flags: TransformFlags::default(),
                        }],
                    }],
                }],
            }]
            .iter(),
            None,
            0.0,
            true,
        );

        // TODO: Test the unused indices?
        assert_matrix_relative_eq!(
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, -2.0, 0.0, 0.0],
                [0.0, 0.0, -3.0, 0.0],
                [4.0, 5.0, 6.0, 1.0],
            ],
            transforms.animated_world_transforms.transforms[0].to_cols_array_2d()
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
                .to_cols_array_2d()
        );
        assert_matrix_relative_eq!(
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, -2.0, 0.0, 0.0],
                [0.0, 0.0, -3.0, 0.0],
                [4.0, 5.0, 6.0, 1.0],
            ],
            transforms.world_transforms[0].to_cols_array_2d()
        );
    }

    #[test]
    fn apply_animation_two_animations() {
        // Check that animations overlap properly.
        let mut transforms = AnimationTransforms::identity();
        animate_skel(
            &mut transforms,
            &SkelData {
                major_version: 1,
                minor_version: 0,
                bones: vec![
                    identity_bone("A", None),
                    identity_bone("B", None),
                    identity_bone("C", None),
                ],
            },
            [
                AnimData {
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
                                    compensate_scale: false,
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
                                    compensate_scale: false,
                                    values: TrackValues::Transform(vec![Transform {
                                        scale: Vector3::new(4.0, 5.0, 6.0),
                                        rotation: Vector4::new(0.0, 0.0, 0.0, 1.0),
                                        translation: Vector3::new(0.0, 0.0, 0.0),
                                    }]),
                                    transform_flags: TransformFlags::default(),
                                }],
                            },
                        ],
                    }],
                },
                AnimData {
                    major_version: 2,
                    minor_version: 0,
                    final_frame_index: 0.0,
                    groups: vec![GroupData {
                        group_type: GroupType::Transform,
                        nodes: vec![
                            NodeData {
                                name: "B".to_string(),
                                tracks: vec![TrackData {
                                    name: "Transform".to_string(),
                                    compensate_scale: false,
                                    values: TrackValues::Transform(vec![Transform {
                                        scale: Vector3::new(4.0, 5.0, 6.0),
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
                                    compensate_scale: false,
                                    values: TrackValues::Transform(vec![Transform {
                                        scale: Vector3::new(7.0, 8.0, 9.0),
                                        rotation: Vector4::new(0.0, 0.0, 0.0, 1.0),
                                        translation: Vector3::new(0.0, 0.0, 0.0),
                                    }]),
                                    transform_flags: TransformFlags::default(),
                                }],
                            },
                        ],
                    }],
                },
            ]
            .iter(),
            None,
            0.0,
            true,
        );

        // TODO: Test the unused indices?
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
                [4.0, 0.0, 0.0, 0.0],
                [0.0, 5.0, 0.0, 0.0],
                [0.0, 0.0, 6.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            transforms.animated_world_transforms.transforms[1].to_cols_array_2d()
        );
        assert_matrix_relative_eq!(
            [
                [7.0, 0.0, 0.0, 0.0],
                [0.0, 8.0, 0.0, 0.0],
                [0.0, 0.0, 9.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            transforms.animated_world_transforms.transforms[2].to_cols_array_2d()
        );
    }

    #[test]
    fn apply_animation_middle_bone_no_inherit_scale_no_compensate_scale() {
        let mut transforms = AnimationTransforms::identity();
        animate_skel(
            &mut transforms,
            &SkelData {
                major_version: 1,
                minor_version: 0,
                bones: vec![
                    identity_bone("A", None),
                    identity_bone("B", Some(0)),
                    identity_bone("C", Some(1)),
                ],
            },
            [AnimData {
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
                                compensate_scale: false,
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
                                // TODO: This acts just like scale inheritance?
                                compensate_scale: false,
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
                                compensate_scale: false,
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
            }]
            .iter(),
            None,
            0.0,
            true,
        );

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
                [0.0, 8.0, 0.0, 0.0],
                [0.0, 0.0, 27.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            transforms.animated_world_transforms.transforms[2].to_cols_array_2d()
        );
    }

    fn animate_three_bone_chain(
        scale: [f32; 3],
        compensate_scales: [bool; 3],
    ) -> AnimationTransforms {
        let mut transforms = AnimationTransforms::identity();
        animate_skel(
            &mut transforms,
            &SkelData {
                major_version: 1,
                minor_version: 0,
                bones: vec![
                    identity_bone("A", None),
                    identity_bone("B", Some(0)),
                    identity_bone("C", Some(1)),
                ],
            },
            [AnimData {
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
                                compensate_scale: compensate_scales[0],
                                values: TrackValues::Transform(vec![Transform {
                                    scale: scale.into(),
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
                                compensate_scale: compensate_scales[1],
                                values: TrackValues::Transform(vec![Transform {
                                    scale: scale.into(),
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
                                compensate_scale: compensate_scales[2],
                                values: TrackValues::Transform(vec![Transform {
                                    scale: scale.into(),
                                    rotation: Vector4::new(0.0, 0.0, 0.0, 1.0),
                                    translation: Vector3::new(0.0, 0.0, 0.0),
                                }]),
                                transform_flags: TransformFlags::default(),
                            }],
                        },
                    ],
                }],
            }]
            .iter(),
            None,
            0.0,
            true,
        );
        transforms
    }

    // TODO: test if the compensated scale is the scale that is inherited?
    // This can be done by setting the final bone's scale to 1.0.
    #[test]
    fn apply_animation_bone_chain_inherit_scale_no_compensate_scale() {
        let transforms = animate_three_bone_chain([1.0, 2.0, 3.0], [false, false, false]);

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
                [0.0, 8.0, 0.0, 0.0],
                [0.0, 0.0, 27.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            transforms.animated_world_transforms.transforms[2].to_cols_array_2d()
        );
    }

    #[test]
    fn apply_animation_bone_chain_inherit_scale_compensate_scale() {
        let transforms = animate_three_bone_chain([1.0, 2.0, 3.0], [false, false, true]);

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
                [0.0, 4.0, 0.0, 0.0],
                [0.0, 0.0, 9.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            transforms.animated_world_transforms.transforms[2].to_cols_array_2d()
        );
    }

    #[test]
    fn apply_animation_bone_chain_no_inherit_scale_no_compensate_scale() {
        let transforms = animate_three_bone_chain([1.0, 2.0, 3.0], [false, false, false]);

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
                [0.0, 8.0, 0.0, 0.0],
                [0.0, 0.0, 27.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            transforms.animated_world_transforms.transforms[2].to_cols_array_2d()
        );
    }

    #[test]
    fn apply_animation_bone_chain_no_inherit_scale_compensate_scale() {
        let transforms = animate_three_bone_chain([1.0, 2.0, 3.0], [false, false, true]);

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
                [0.0, 4.0, 0.0, 0.0],
                [0.0, 0.0, 9.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            transforms.animated_world_transforms.transforms[2].to_cols_array_2d()
        );
    }

    // TODO: Test additional TransformFlags combinations.
    #[test]
    fn apply_animation_bone_chain_override_transforms() {
        // Test resetting all transforms to their "resting" pose from the skel.
        let mut transforms = AnimationTransforms::identity();
        animate_skel(
            &mut transforms,
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
            [AnimData {
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
                                compensate_scale: false,
                                values: TrackValues::Transform(vec![Transform {
                                    scale: Vector3::new(2.0, 2.0, 2.0),
                                    rotation: Vector4::new(1.0, 0.0, 0.0, 0.0),
                                    translation: Vector3::new(0.0, 1.0, 2.0),
                                }]),
                                transform_flags: TransformFlags {
                                    override_translation: true,
                                    override_rotation: true,
                                    override_scale: true,
                                    override_compensate_scale: true,
                                },
                            }],
                        },
                        NodeData {
                            name: "B".to_string(),
                            tracks: vec![TrackData {
                                name: "Transform".to_string(),
                                compensate_scale: false,
                                values: TrackValues::Transform(vec![Transform {
                                    scale: Vector3::new(2.0, 2.0, 2.0),
                                    rotation: Vector4::new(1.0, 0.0, 0.0, 0.0),
                                    translation: Vector3::new(0.0, 1.0, 2.0),
                                }]),
                                transform_flags: TransformFlags {
                                    override_translation: true,
                                    override_rotation: true,
                                    override_scale: true,
                                    override_compensate_scale: true,
                                },
                            }],
                        },
                        NodeData {
                            name: "C".to_string(),
                            tracks: vec![TrackData {
                                name: "Transform".to_string(),
                                compensate_scale: false,
                                values: TrackValues::Transform(vec![Transform {
                                    scale: Vector3::new(2.0, 2.0, 2.0),
                                    rotation: Vector4::new(1.0, 0.0, 0.0, 0.0),
                                    translation: Vector3::new(0.0, 1.0, 2.0),
                                }]),
                                transform_flags: TransformFlags {
                                    override_translation: true,
                                    override_rotation: true,
                                    override_scale: true,
                                    override_compensate_scale: true,
                                },
                            }],
                        },
                    ],
                }],
            }]
            .iter(),
            None,
            0.0,
            true,
        );

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

    // TODO: How to reproduce the bug caused by precomputed world transforms?
    #[test]
    fn orient_constraints_chain() {
        // Bones are all at the origin but separated in the diagram for clarity.
        // Skel + Anim:
        // ^  ^
        // |  |
        // L0 L1    R0 -> <- R1

        // Skel + Anim + Hlpb (constrain L0 to R0 and L1 to R1):
        // L0 -> <- L1    R0 -> <- R1
        let l0 = identity_bone("L0", None);
        let l1 = identity_bone("L1", Some(0));
        let r0 = identity_bone("R0", None);
        let r1 = identity_bone("R1", Some(2));

        // Check for correctly precomputing world transforms in the hlpb step.
        // This impacts constraints applied to multiple bones in a chain.
        let mut transforms = AnimationTransforms::identity();

        // TODO: Adjust this test to detect incorrectly precomputing anim world transforms.
        animate_skel(
            &mut transforms,
            &ssbh_data::skel_data::SkelData {
                major_version: 1,
                minor_version: 0,
                bones: vec![l0, l1, r0, r1],
            },
            [AnimData {
                major_version: 2,
                minor_version: 0,
                final_frame_index: 0.0,
                groups: vec![GroupData {
                    group_type: GroupType::Transform,
                    nodes: vec![
                        NodeData {
                            name: "L0".to_string(),
                            tracks: vec![TrackData {
                                name: "Transform".to_string(),
                                compensate_scale: true,
                                values: TrackValues::Transform(vec![Transform {
                                    scale: Vector3::new(1.0, 1.0, 1.0),
                                    rotation: glam::Quat::from_rotation_z(0.0f32.to_radians())
                                        .to_array()
                                        .into(),
                                    translation: Vector3::new(1.0, 2.0, 3.0),
                                }]),
                                transform_flags: TransformFlags::default(),
                            }],
                        },
                        NodeData {
                            name: "L1".to_string(),
                            tracks: vec![TrackData {
                                name: "Transform".to_string(),
                                compensate_scale: true,
                                values: TrackValues::Transform(vec![Transform {
                                    scale: Vector3::new(1.0, 1.0, 1.0),
                                    rotation: glam::Quat::from_rotation_z(0.0f32.to_radians())
                                        .to_array()
                                        .into(),
                                    translation: Vector3::new(4.0, 5.0, 6.0),
                                }]),
                                transform_flags: TransformFlags::default(),
                            }],
                        },
                        NodeData {
                            name: "R0".to_string(),
                            tracks: vec![TrackData {
                                name: "Transform".to_string(),
                                compensate_scale: true,
                                values: TrackValues::Transform(vec![Transform {
                                    scale: Vector3::new(1.0, 1.0, 1.0),
                                    rotation: glam::Quat::from_rotation_z(90.0f32.to_radians())
                                        .to_array()
                                        .into(),
                                    translation: Vector3::new(1.0, 2.0, 3.0),
                                }]),
                                transform_flags: TransformFlags::default(),
                            }],
                        },
                        NodeData {
                            name: "R1".to_string(),
                            tracks: vec![TrackData {
                                name: "Transform".to_string(),
                                compensate_scale: true,
                                values: TrackValues::Transform(vec![Transform {
                                    scale: Vector3::new(1.0, 1.0, 1.0),
                                    rotation: glam::Quat::from_rotation_z(0.0f32.to_radians())
                                        .to_array()
                                        .into(),
                                    translation: Vector3::new(4.0, 5.0, 6.0),
                                }]),
                                transform_flags: TransformFlags::default(),
                            }],
                        },
                    ],
                }],
            }]
            .iter(),
            Some(&HlpbData {
                major_version: 1,
                minor_version: 0,
                aim_constraints: Vec::new(),
                orient_constraints: vec![
                    OrientConstraintData {
                        name: "constraint1".into(),
                        parent_bone_name1: "Root".into(), // TODO: What to put here?
                        parent_bone_name2: "Root".into(),
                        source_bone_name: "R0".into(),
                        target_bone_name: "L0".into(),
                        unk_type: 2,
                        constraint_axes: Vector3::new(1.0, 1.0, 1.0),
                        quat1: Vector4::new(0.0, 0.0, 0.0, 1.0),
                        quat2: Vector4::new(0.0, 0.0, 0.0, 1.0),
                        range_min: Vector3::new(-180.0, -180.0, -180.0),
                        range_max: Vector3::new(180.0, 180.0, 180.0),
                    },
                    OrientConstraintData {
                        name: "constraint2".into(),
                        parent_bone_name1: "Root".into(), // TODO: What to put here?
                        parent_bone_name2: "Root".into(),
                        source_bone_name: "R1".into(),
                        target_bone_name: "L1".into(),
                        unk_type: 2,
                        constraint_axes: Vector3::new(1.0, 1.0, 1.0),
                        quat1: Vector4::new(0.0, 0.0, 0.0, 1.0),
                        quat2: Vector4::new(0.0, 0.0, 0.0, 1.0),
                        range_min: Vector3::new(-180.0, -180.0, -180.0),
                        range_max: Vector3::new(180.0, 180.0, 180.0),
                    },
                ],
            }),
            0.0,
            true,
        );

        assert_matrix_relative_eq!(
            transforms.animated_world_transforms.transforms[0].to_cols_array_2d(),
            transforms.animated_world_transforms.transforms[2].to_cols_array_2d()
        );

        assert_matrix_relative_eq!(
            transforms.animated_world_transforms.transforms[1].to_cols_array_2d(),
            transforms.animated_world_transforms.transforms[3].to_cols_array_2d()
        );

        assert_matrix_relative_eq!(
            transforms.world_transforms[0].to_cols_array_2d(),
            transforms.world_transforms[2].to_cols_array_2d()
        );

        assert_matrix_relative_eq!(
            transforms.world_transforms[1].to_cols_array_2d(),
            transforms.world_transforms[3].to_cols_array_2d()
        );
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
                                compensate_scale: false,
                                values: TrackValues::Boolean(vec![true, false, true]),
                                transform_flags: TransformFlags::default(),
                            }],
                        },
                        NodeData {
                            name: "B".to_string(),
                            tracks: vec![TrackData {
                                name: "Visibility".to_string(),
                                compensate_scale: false,
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
