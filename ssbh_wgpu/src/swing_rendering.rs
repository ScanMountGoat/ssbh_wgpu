use std::collections::HashSet;

use glam::Vec4Swizzles;
use prc::hash40::Hash40;
use ssbh_data::skel_data::SkelData;

use crate::{
    shape::{
        capsule_mesh_buffers, capsule_vertices, plane_mesh_buffers, sphere_mesh_buffers,
        IndexedMeshBuffers,
    },
    swing::*,
    DeviceBufferExt, QueueExt,
};

const SPHERE_COLOR: glam::Vec4 = glam::vec4(1.0, 0.0, 0.0, 1.0);
const OVAL_COLOR: glam::Vec4 = glam::vec4(0.0, 1.0, 0.0, 1.0);
const ELLIPSOID_COLOR: glam::Vec4 = glam::vec4(0.0, 1.0, 1.0, 1.0);
const CAPSULE_COLOR: glam::Vec4 = glam::vec4(1.0, 0.0, 0.0, 1.0);
const PLANE_COLOR: glam::Vec4 = glam::vec4(1.0, 1.0, 0.0, 1.0);

pub struct SwingRenderData {
    // Spheres and planes all share the same vertex data.
    pub sphere_buffers: IndexedMeshBuffers,
    pub plane_buffers: IndexedMeshBuffers,
    pub bind_group1: crate::shader::swing::bind_groups::BindGroup1,
    pub collisions: CollisionData,
}

pub struct CollisionData {
    pub spheres: Vec<ShapeRenderData>,
    pub ovals: Vec<(IndexedMeshBuffers, ShapeRenderData)>,
    pub ellipsoids: Vec<ShapeRenderData>,
    pub capsules: Vec<(IndexedMeshBuffers, ShapeRenderData)>,
    pub planes: Vec<ShapeRenderData>,
    // TODO: Is there another way to store bone information?
    pub prc_capsules: Vec<Capsule>,
    pub prc_ovals: Vec<Oval>,
}

impl CollisionData {
    pub fn new() -> Self {
        Self {
            spheres: Vec::new(),
            ovals: Vec::new(),
            ellipsoids: Vec::new(),
            capsules: Vec::new(),
            planes: Vec::new(),
            // TODO: Find a better way to store the prc data for animating.
            prc_capsules: Vec::new(),
            prc_ovals: Vec::new(),
        }
    }

    pub fn from_swing(
        device: &wgpu::Device,
        swing_prc: &SwingPrc,
        skel: Option<&SkelData>,
        world_transforms: &[glam::Mat4],
    ) -> Self {
        Self {
            spheres: spheres(device, &swing_prc.spheres, skel),
            ovals: ovals(device, &swing_prc.ovals, skel, world_transforms),
            ellipsoids: ellipsoids(device, &swing_prc.ellipsoids, skel),
            capsules: capsules(device, &swing_prc.capsules, skel, world_transforms),
            planes: planes(device, &swing_prc.planes, skel),
            // TODO: Find a better way to store the prc data for animating.
            prc_capsules: swing_prc.capsules.clone(),
            prc_ovals: swing_prc.ovals.clone(),
        }
    }
}

// TODO: Figure out which objects don't need to be recreated every frame.
impl SwingRenderData {
    pub fn new(device: &wgpu::Device, bone_world_transforms_buffer: &wgpu::Buffer) -> Self {
        let sphere_buffers = sphere_mesh_buffers(device);
        let plane_buffers = plane_mesh_buffers(device);

        let bind_group1 = crate::shader::swing::bind_groups::BindGroup1::from_bindings(
            device,
            crate::shader::swing::bind_groups::BindGroupLayout1 {
                world_transforms: bone_world_transforms_buffer.as_entire_buffer_binding(),
            },
        );

        Self {
            sphere_buffers,
            plane_buffers,
            bind_group1,
            // TODO: Group these together into a struct?
            collisions: CollisionData::new(),
        }
    }

    pub fn update_collisions(
        &mut self,
        device: &wgpu::Device,
        swing_prc: &SwingPrc,
        skel: Option<&SkelData>,
        world_transforms: &[glam::Mat4],
    ) {
        // Only update the data related to collisions.
        // Recreating pipelines is slow.
        self.collisions = CollisionData::from_swing(device, swing_prc, skel, world_transforms)
    }

    pub fn animate_collisions(
        &self,
        queue: &wgpu::Queue,
        skel: Option<&SkelData>,
        world_transforms: &[glam::Mat4],
    ) {
        // TODO: Write to the buffer for capsules and ovals.
        // TODO: Find a better way to store the PRC collision data.
        // We need information from the PRC to regenerate vertex data while animating for some shapes.
        for ((buffers, shape), c) in self
            .collisions
            .capsules
            .iter()
            .zip(self.collisions.prc_capsules.iter())
        {
            // TODO: Find a way to avoid specifying this logic in multiple places.
            // TODO: How to reduce repeated logic for buffer creation and writing?
            let (height, per_shape) = capsules_per_shape(skel, c, world_transforms, CAPSULE_COLOR);
            let data = capsule_vertices(8, 8, height, c.start_radius, c.end_radius);
            queue.write_data(&buffers.vertex_buffer, &data);

            // TODO: Find a way to avoid needing the swing_prc again.
            shape.update(queue, per_shape);
        }

        for ((buffers, shape), o) in self
            .collisions
            .ovals
            .iter()
            .zip(self.collisions.prc_ovals.iter())
        {
            // TODO: Find a way to avoid specifying this logic in multiple places.
            // TODO: How to reduce repeated logic for buffer creation and writing?
            // TODO: Implement proper oval rendering.
            // Use capsules for now since they both use a start/end bone.
            let capsule = Capsule {
                name: o.name,
                start_bonename: o.start_bonename,
                end_bonename: o.end_bonename,
                start_offset_x: o.start_offset_x,
                start_offset_y: o.start_offset_y,
                start_offset_z: o.start_offset_z,
                end_offset_x: o.end_offset_x,
                end_offset_y: o.end_offset_y,
                end_offset_z: o.end_offset_z,
                start_radius: o.radius,
                end_radius: o.radius,
            };
            let (height, per_shape) =
                capsules_per_shape(skel, &capsule, world_transforms, OVAL_COLOR);
            let data = capsule_vertices(8, 8, height, o.radius, o.radius);
            queue.write_data(&buffers.vertex_buffer, &data);

            // TODO: Find a way to avoid needing the swing_prc again.
            shape.update(queue, per_shape);
        }
    }
}

pub struct ShapeRenderData {
    hash: u64,
    // Store the buffer for updating shapes without allocating new bind groups.
    buffer: wgpu::Buffer,
    bind_group: crate::shader::swing::bind_groups::BindGroup2,
}

impl ShapeRenderData {
    pub fn new(
        device: &wgpu::Device,
        hash: u64,
        per_shape: crate::shader::swing::PerShape,
    ) -> Self {
        let buffer = device.create_buffer_from_data(
            "Swing Buffer2",
            &[per_shape],
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );

        let bind_group = crate::shader::swing::bind_groups::BindGroup2::from_bindings(
            device,
            crate::shader::swing::bind_groups::BindGroupLayout2 {
                per_shape: buffer.as_entire_buffer_binding(),
            },
        );

        Self {
            hash,
            buffer,
            bind_group,
        }
    }

    pub fn update(&self, queue: &wgpu::Queue, per_shape: crate::shader::swing::PerShape) {
        queue.write_data(&self.buffer, &[per_shape]);
    }
}

fn spheres(
    device: &wgpu::Device,
    spheres: &[Sphere],
    skel: Option<&SkelData>,
) -> Vec<ShapeRenderData> {
    spheres
        .iter()
        .map(|s| {
            ShapeRenderData::new(
                device,
                s.name.0,
                crate::shader::swing::PerShape {
                    bone_indices: glam::IVec4::new(bone_index(skel, s.bonename), -1, -1, -1),
                    start_transform: (glam::Mat4::from_translation(glam::vec3(s.cx, s.cy, s.cz))
                        * glam::Mat4::from_scale(glam::Vec3::splat(s.radius))),
                    color: SPHERE_COLOR,
                },
            )
        })
        .collect()
}

fn ellipsoids(
    device: &wgpu::Device,
    ellipsoids: &[Ellipsoid],
    skel: Option<&SkelData>,
) -> Vec<ShapeRenderData> {
    ellipsoids
        .iter()
        .map(|e| {
            // TODO: Is r rotation since it's usually 0?
            ShapeRenderData::new(
                device,
                e.name.0,
                crate::shader::swing::PerShape {
                    bone_indices: glam::IVec4::new(bone_index(skel, e.bonename), -1, -1, -1),
                    start_transform: (glam::Mat4::from_translation(glam::vec3(e.cx, e.cy, e.cz))
                        * glam::Mat4::from_scale(glam::vec3(e.sx, e.sy, e.sz))),
                    color: ELLIPSOID_COLOR,
                },
            )
        })
        .collect()
}

fn ovals(
    device: &wgpu::Device,
    ovals: &[Oval],
    skel: Option<&SkelData>,
    world_transforms: &[glam::Mat4],
) -> Vec<(IndexedMeshBuffers, ShapeRenderData)> {
    ovals
        .iter()
        .map(|o| {
            // TODO: Implement proper oval rendering.
            // Use capsules for now since they both use a start/end bone.
            let capsule = Capsule {
                name: o.name,
                start_bonename: o.start_bonename,
                end_bonename: o.end_bonename,
                start_offset_x: o.start_offset_x,
                start_offset_y: o.start_offset_y,
                start_offset_z: o.start_offset_z,
                end_offset_x: o.end_offset_x,
                end_offset_y: o.end_offset_y,
                end_offset_z: o.end_offset_z,
                start_radius: o.radius,
                end_radius: o.radius,
            };
            let (height, per_shape) =
                capsules_per_shape(skel, &capsule, world_transforms, OVAL_COLOR);
            let mesh_buffers = capsule_mesh_buffers(device, height, o.radius, o.radius);

            (
                mesh_buffers,
                ShapeRenderData::new(device, o.name.0, per_shape),
            )
        })
        .collect()
}

fn capsules(
    device: &wgpu::Device,
    capsules: &[Capsule],
    skel: Option<&SkelData>,
    world_transforms: &[glam::Mat4],
) -> Vec<(IndexedMeshBuffers, ShapeRenderData)> {
    capsules
        .iter()
        .map(|c| {
            let (height, per_shape) = capsules_per_shape(skel, c, world_transforms, CAPSULE_COLOR);
            // TODO: Rework this to get the vertex data to write to an existing buffer.
            let mesh_buffers = capsule_mesh_buffers(device, height, c.start_radius, c.end_radius);

            (
                mesh_buffers,
                ShapeRenderData::new(device, c.name.0, per_shape),
            )
        })
        .collect()
}

fn capsules_per_shape(
    skel: Option<&SkelData>,
    c: &Capsule,
    world_transforms: &[glam::Mat4],
    color: glam::Vec4,
) -> (f32, crate::shader::swing::PerShape) {
    let (height, transform) =
        capsule_transform(c, skel, world_transforms).unwrap_or((1.0, glam::Mat4::IDENTITY));

    let per_shape = crate::shader::swing::PerShape {
        bone_indices: glam::IVec4::splat(-1),
        start_transform: transform,
        color,
    };

    (height, per_shape)
}

fn capsule_transform(
    c: &Capsule,
    skel: Option<&SkelData>,
    world_transforms: &[glam::Mat4],
) -> Option<(f32, glam::Mat4)> {
    // The capsule needs to be transformed to span the two bones.
    let start_i = bone_position(skel, c.start_bonename)?;
    let end_i = bone_position(skel, c.end_bonename)?;

    let start_bone_pos = world_transforms.get(start_i)?.col(3).xyz();
    let _start_offset = glam::vec3(c.start_offset_x, c.start_offset_y, c.start_offset_z);

    let end_bone_pos = world_transforms.get(end_i)?.col(3).xyz();
    let _end_offset = glam::vec3(c.end_offset_x, c.end_offset_y, c.end_offset_z);

    // Assume the shape is along the Z-axis and has unit dimensions.
    let direction = end_bone_pos - start_bone_pos;

    // TODO: How to include the offsets?
    // Rotate the shape to point to both bones.
    // Handle the case where start equals end and the direction vector is zero.
    let rotation = glam::Quat::from_rotation_arc(glam::Vec3::Z, direction.normalize_or_zero());

    // Translate the bone in between the two bones.
    let center = (end_bone_pos + start_bone_pos) / 2.0;

    Some((
        direction.length(),
        glam::Mat4::from_translation(center) * glam::Mat4::from_quat(rotation),
    ))
}

fn planes(
    device: &wgpu::Device,
    planes: &[Plane],
    skel: Option<&SkelData>,
) -> Vec<ShapeRenderData> {
    planes
        .iter()
        .map(|p| {
            // Assume the plane points in the direction of the positive Z-axis.
            // Rotate the plane to point in the direction (nx, ny, nz).
            // TODO: Does this correctly match the in game behavior?
            ShapeRenderData::new(
                device,
                p.name.0,
                crate::shader::swing::PerShape {
                    bone_indices: glam::IVec4::new(bone_index(skel, p.bonename), -1, -1, -1),
                    start_transform: glam::Mat4::from_quat(glam::Quat::from_rotation_arc(
                        glam::Vec3::Z,
                        glam::vec3(p.nx, p.ny, p.nz),
                    )),
                    color: PLANE_COLOR,
                },
            )
        })
        .collect()
}

fn bone_position(skel: Option<&SkelData>, name: Hash40) -> Option<usize> {
    skel.and_then(|skel| {
        skel.bones
            .iter()
            .position(|b| prc::hash40::hash40(&b.name.to_lowercase()) == name)
    })
}

fn bone_index(skel: Option<&SkelData>, name: Hash40) -> i32 {
    bone_position(skel, name).map(|i| i as i32).unwrap_or(-1)
}

// TODO: Is it worth making a SwingRenderer type?
pub fn draw_swing_collisions(
    render_data: &SwingRenderData,
    pass: &mut wgpu::RenderPass<'_>,
    swing_pipeline: &wgpu::RenderPipeline,
    swing_camera_bind_group: &crate::shader::swing::bind_groups::BindGroup0,
    hidden_collisions: &HashSet<u64>,
) {
    pass.set_pipeline(swing_pipeline);

    // Just draw most shapes as spheres for now.
    // TODO: Create vertex and index buffers for each shape.
    // TODO: Not all bind groups need to be set more than once.
    // TODO: Allow toggling rendering of certain shapes or shape types.
    draw_shapes(
        pass,
        &render_data.sphere_buffers,
        &render_data.collisions.spheres,
        &render_data.bind_group1,
        swing_camera_bind_group,
        hidden_collisions,
    );

    draw_shapes_with_buffers(
        pass,
        &render_data.collisions.ovals,
        &render_data.bind_group1,
        swing_camera_bind_group,
        hidden_collisions,
    );

    // Ellipsoids use the sphere geometry.
    draw_shapes(
        pass,
        &render_data.sphere_buffers,
        &render_data.collisions.ellipsoids,
        &render_data.bind_group1,
        swing_camera_bind_group,
        hidden_collisions,
    );

    draw_shapes_with_buffers(
        pass,
        &render_data.collisions.capsules,
        &render_data.bind_group1,
        swing_camera_bind_group,
        hidden_collisions,
    );

    draw_shapes(
        pass,
        &render_data.plane_buffers,
        &render_data.collisions.planes,
        &render_data.bind_group1,
        swing_camera_bind_group,
        hidden_collisions,
    );
}

fn draw_shapes(
    pass: &mut wgpu::RenderPass<'_>,
    buffers: &IndexedMeshBuffers,
    shapes: &[ShapeRenderData],
    bind_group1: &crate::shader::swing::bind_groups::BindGroup1,
    swing_camera_bind_group: &crate::shader::swing::bind_groups::BindGroup0,
    hidden_collisions: &HashSet<u64>,
) {
    buffers.set(pass);
    for shape in shapes
        .iter()
        .filter(|s| !hidden_collisions.contains(&s.hash))
    {
        draw_shape(pass, swing_camera_bind_group, bind_group1, shape, buffers);
    }
}

fn draw_shapes_with_buffers(
    pass: &mut wgpu::RenderPass<'_>,
    shapes: &[(IndexedMeshBuffers, ShapeRenderData)],
    bind_group1: &crate::shader::swing::bind_groups::BindGroup1,
    swing_camera_bind_group: &crate::shader::swing::bind_groups::BindGroup0,
    hidden_collisions: &HashSet<u64>,
) {
    for (buffers, shape) in shapes
        .iter()
        .filter(|s| !hidden_collisions.contains(&s.1.hash))
    {
        buffers.set(pass);
        draw_shape(pass, swing_camera_bind_group, bind_group1, shape, buffers);
    }
}

fn draw_shape(
    pass: &mut wgpu::RenderPass<'_>,
    swing_camera_bind_group: &crate::shader::swing::bind_groups::BindGroup0,
    bind_group1: &crate::shader::swing::bind_groups::BindGroup1,
    shape: &ShapeRenderData,
    buffers: &IndexedMeshBuffers,
) {
    crate::shader::swing::set_bind_groups(
        pass,
        swing_camera_bind_group,
        bind_group1,
        &shape.bind_group,
    );
    pass.draw_indexed(0..buffers.index_count, 0, 0..1);
}

pub fn swing_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = crate::shader::swing::create_shader_module(device);
    let layout = crate::shader::swing::create_pipeline_layout(device);

    // TODO: Get the stride using encase.
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[crate::shader::swing::VertexInput::vertex_buffer_layout(
                wgpu::VertexStepMode::Vertex,
            )],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}
