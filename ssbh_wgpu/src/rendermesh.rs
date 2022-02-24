use crate::{
    pipeline::create_pipeline,
    texture::{load_texture_sampler_cube_or_default, load_texture_sampler_or_default},
    uniforms::create_uniforms_buffer,
    vertex::mesh_object_buffers,
};
use ssbh_data::{
    matl_data::{MatlData, ParamId},
    mesh_data::MeshData,
    modl_data::ModlData,
    skel_data::SkelData,
};
use std::sync::Arc;
use wgpu::util::DeviceExt;

pub fn create_render_meshes(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    surface_format: wgpu::TextureFormat,
    folder: &str,
    mesh: &MeshData,
    skel: &Option<SkelData>,
    matl: &Option<MatlData>,
    modl: &Option<ModlData>,
) -> Vec<RenderMesh> {
    let mut meshes = get_render_meshes_and_shader_tags(
        device,
        queue,
        layout,
        shader,
        surface_format,
        folder,
        mesh,
        skel,
        matl,
        modl,
    );
    // Sort by the tag.
    // TODO: Specify a custom ordering?
    meshes.sort_by(|a, b| render_order(&a.1).cmp(&render_order(&b.1)));
    meshes.into_iter().map(|(m, _)| m).collect()
}

fn get_render_meshes_and_shader_tags(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    surface_format: wgpu::TextureFormat,
    folder: &str,
    mesh: &MeshData,
    skel: &Option<SkelData>,
    matl: &Option<MatlData>,
    modl: &Option<ModlData>,
) -> Vec<(RenderMesh, String)> {
    // TODO: Find a way to organize this.
    // TODO: Find a way to derive the constants for the strides, alignments, etc.
    mesh.objects
        .iter() // TODO: par_iter?
        .map(|mesh_object| {
            // TODO: These could be cleaner as functions.
            let material_label = modl
                .as_ref()
                .map(|m| {
                    m.entries
                        .iter()
                        .find(|e| {
                            e.mesh_object_name == mesh_object.name
                                && e.mesh_object_sub_index == mesh_object.sub_index
                        })
                        .map(|e| &e.material_label)
                })
                .flatten();

            let material = material_label
                .map(|material_label| {
                    matl.as_ref().map(|matl| {
                        matl.entries
                            .iter()
                            .find(|e| &e.material_label == material_label)
                    })
                })
                .flatten()
                .flatten();

            // TODO: Initialize the pipeline here?
            let pipeline = create_pipeline(
                device,
                layout,
                shader,
                surface_format,
                mesh_object,
                material,
            );

            // TODO: Ideally some of these will be shared for performance reasons.
            // Create a pipeline per mesh object for now to test the worse case performance.
            let pipeline = Arc::new(pipeline);
            let (vertex_buffer0, vertex_buffer1, index_buffer, vertex_index_count) =
                mesh_object_buffers(mesh_object, device);

            let load_texture_sampler = |texture_id, sampler_id, default| {
                load_texture_sampler_or_default(
                    device, queue, material, folder, texture_id, sampler_id, default,
                )
            };

            let load_texture_sampler_cube = |texture_id, sampler_id, default| {
                load_texture_sampler_cube_or_default(
                    device, queue, material, folder, texture_id, sampler_id, default,
                )
            };

            // TODO: Have accurate defaults but also accurate texture blending?
            // TODO: Generate this using a macro?
            let (texture0, sampler0) =
                load_texture_sampler(ParamId::Texture0, ParamId::Sampler0, [0, 0, 0, 255]);
            let (texture1, sampler1) =
                load_texture_sampler(ParamId::Texture1, ParamId::Sampler1, [0, 0, 0, 0]);
            let (texture2, sampler2) =
                load_texture_sampler_cube(ParamId::Texture2, ParamId::Sampler2, [0, 0, 0, 0]);
            let (texture3, sampler3) =
                load_texture_sampler(ParamId::Texture3, ParamId::Sampler3, [0, 0, 0, 0]);
            let (texture4, sampler4) =
                load_texture_sampler(ParamId::Texture4, ParamId::Sampler4, [0, 0, 0, 0]);
            let (texture5, sampler5) =
                load_texture_sampler(ParamId::Texture5, ParamId::Sampler5, [0, 0, 0, 255]);
            let (texture6, sampler6) =
                load_texture_sampler(ParamId::Texture6, ParamId::Sampler6, [0, 0, 0, 0]);

            // TODO: Avoid loading texture files more than once.
            let (texture7, sampler7) =
                load_texture_sampler_cube(ParamId::Texture7, ParamId::Sampler7, [0, 128, 255, 255]);

            let (texture8, sampler8) =
                load_texture_sampler_cube(ParamId::Texture8, ParamId::Sampler8, [0, 0, 0, 255]);
            let (texture9, sampler9) =
                load_texture_sampler(ParamId::Texture9, ParamId::Sampler9, [0, 0, 0, 255]);
            let (texture10, sampler10) =
                load_texture_sampler(ParamId::Texture10, ParamId::Sampler10, [0, 0, 0, 255]);
            let (texture11, sampler11) =
                load_texture_sampler(ParamId::Texture11, ParamId::Sampler11, [0, 0, 0, 255]);
            let (texture12, sampler12) =
                load_texture_sampler(ParamId::Texture12, ParamId::Sampler12, [0, 0, 0, 255]);
            let (texture13, sampler13) =
                load_texture_sampler(ParamId::Texture13, ParamId::Sampler13, [0, 0, 0, 255]);

            let (texture14, sampler14) =
                load_texture_sampler(ParamId::Texture14, ParamId::Sampler14, [0, 0, 0, 0]);

            let transforms_buffer = create_transforms_buffer(mesh_object, skel, device);
            let uniforms_buffer = create_uniforms_buffer(material, device);

            let mesh = RenderMesh {
                pipeline,
                vertex_buffer0,
                vertex_buffer1,
                index_buffer,
                vertex_index_count,
                transforms_bind_group: crate::shader::model::bind_groups::BindGroup1::from_bindings(
                    device,
                    crate::shader::model::bind_groups::BindGroupLayout1 {
                        transforms: &transforms_buffer,
                    },
                ),
                textures_bind_group: crate::shader::model::bind_groups::BindGroup2::from_bindings(
                    device,
                    crate::shader::model::bind_groups::BindGroupLayout2 {
                        texture0: &texture0,
                        sampler0: &sampler0,
                        texture1: &texture1,
                        sampler1: &sampler1,
                        texture2: &texture2,
                        sampler2: &sampler2,
                        texture3: &texture3,
                        sampler3: &sampler3,
                        texture4: &texture4,
                        sampler4: &sampler4,
                        texture5: &texture5,
                        sampler5: &sampler5,
                        texture6: &texture6,
                        sampler6: &sampler6,
                        texture7: &texture7,
                        sampler7: &sampler7,
                        texture8: &texture8,
                        sampler8: &sampler8,
                        texture9: &texture9,
                        sampler9: &sampler9,
                        texture10: &texture10,
                        sampler10: &sampler10,
                        texture11: &texture11,
                        sampler11: &sampler11,
                        texture12: &texture12,
                        sampler12: &sampler12,
                        texture13: &texture13,
                        sampler13: &sampler13,
                        texture14: &texture14,
                        sampler14: &sampler14,
                    },
                ),
                material_uniforms_bind_group:
                    crate::shader::model::bind_groups::BindGroup3::from_bindings(
                        device,
                        crate::shader::model::bind_groups::BindGroupLayout3 {
                            uniforms: &uniforms_buffer,
                        },
                    ),
            };

            // The end of the shader label is used to determine draw order.
            // ex: "SFX_PBS_0101000008018278_sort" has a tag of "sort".
            // The render order is opaque -> far -> sort -> near.
            // TODO: How to handle missing tags?
            let shader_tag = material
                .map(|m| m.shader_label.get(25..))
                .flatten()
                .unwrap_or("")
                .to_string();

            (mesh, shader_tag)
        })
        .collect()
}

fn create_transforms_buffer(
    mesh_object: &ssbh_data::mesh_data::MeshObjectData,
    skel: &Option<SkelData>,
    device: &wgpu::Device,
) -> wgpu::Buffer {
    // TODO: Store animation data as well?
    let parent_transform = find_parent_transform(mesh_object, skel).unwrap_or(glam::Mat4::IDENTITY);

    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Transforms Buffer"),
        contents: bytemuck::cast_slice(&[crate::shader::model::bind_groups::Transforms {
            parent_transform,
        }]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    })
}

fn find_parent_transform(
    mesh_object: &ssbh_data::mesh_data::MeshObjectData,
    skel: &Option<SkelData>,
) -> Option<glam::Mat4> {
    if mesh_object.bone_influences.is_empty() {
        if let Some(skel_data) = skel {
            if let Some(parent_bone) = skel_data
                .bones
                .iter()
                .find(|b| b.name == mesh_object.parent_bone_name)
            {
                // TODO: Why do we not transpose here?
                return Some(glam::Mat4::from_cols_array_2d(
                    &skel_data.calculate_world_transform(parent_bone).unwrap(),
                ));
            }
        }
    }

    None
}

fn render_order(tag: &str) -> usize {
    match tag {
        "opaque" => 0,
        "far" => 1,
        "sort" => 2,
        "near" => 3,
        _ => 0, // TODO: How to handle invalid tags?
    }
}

pub fn draw_render_meshes<'a>(
    meshes: &'a [RenderMesh],
    render_pass: &mut wgpu::RenderPass<'a>,
    camera_bind_group: &'a crate::shader::model::bind_groups::BindGroup0,
) {
    // TODO: A future optimization is to reuse pipelines.
    // This requires testing to ensure state is correctly set.
    for mesh in meshes {
        // let start = std::time::Instant::now();
        render_pass.set_pipeline(mesh.pipeline.as_ref());

        crate::shader::model::bind_groups::set_bind_groups(
            render_pass,
            crate::shader::model::bind_groups::BindGroups::<'a> {
                bind_group0: camera_bind_group,
                bind_group1: &mesh.transforms_bind_group,
                bind_group2: &mesh.textures_bind_group,
                bind_group3: &mesh.material_uniforms_bind_group,
            },
        );

        mesh.set_vertex_buffers(render_pass);
        mesh.set_index_buffer(render_pass);

        // println!("Set Render State: {:?}", start.elapsed());
        render_pass.draw_indexed(0..mesh.vertex_index_count, 0, 0..1);
    }
}

pub struct RenderMesh {
    // TODO: It may be worth sharing buffers in the future.
    vertex_buffer0: wgpu::Buffer,
    vertex_buffer1: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    vertex_index_count: u32,
    // Use Arc so that meshes can share a pipeline.
    // Comparing arc pointers can be used to reduce set_pipeline calls later.
    pipeline: Arc<wgpu::RenderPipeline>,
    transforms_bind_group: crate::shader::model::bind_groups::BindGroup1,
    textures_bind_group: crate::shader::model::bind_groups::BindGroup2,
    material_uniforms_bind_group: crate::shader::model::bind_groups::BindGroup3,
}

impl RenderMesh {
    pub fn set_vertex_buffers<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        // TODO: Store the start/end indices in a tuple to avoid having to clone the range?
        render_pass.set_vertex_buffer(0, self.vertex_buffer0.slice(..));
        render_pass.set_vertex_buffer(1, self.vertex_buffer1.slice(..));
    }

    pub fn set_index_buffer<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        // TODO: Store the buffer and type together?
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
    }
}
