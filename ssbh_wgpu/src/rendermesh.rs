use crate::{
    pipeline::create_pipeline, texture::load_texture_sampler, uniforms::create_uniforms_buffer,
    vertex::mesh_object_buffers,
};
use nutexb_wgpu::NutexbFile;
use ssbh_data::{
    matl_data::{MatlData, ParamId},
    mesh_data::MeshData,
    modl_data::ModlData,
    skel_data::SkelData,
};
use std::{collections::HashMap, sync::Arc};
use wgpu::{util::DeviceExt, SamplerDescriptor, TextureViewDescriptor, TextureViewDimension};

pub struct RenderMesh {
    // TODO: It may be worth sharing buffers in the future.
    vertex_buffer0: wgpu::Buffer,
    vertex_buffer1: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    vertex_index_count: u32,
    sort_bias: i32,
    transforms_bind_group: crate::shader::model::bind_groups::BindGroup1,
    // Use Arc so that meshes can share a pipeline.
    // TODO: Comparing arc pointers to reduce set_pipeline calls?
    pipeline: Arc<PipelineData>,
}

pub struct PipelineData {
    pub pipeline: wgpu::RenderPipeline,
    pub textures_bind_group: crate::shader::model::bind_groups::BindGroup2,
    pub material_uniforms_bind_group: crate::shader::model::bind_groups::BindGroup3,
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
    textures: &[(String, NutexbFile)],
    default_textures: &[(&'static str, wgpu::Texture)],
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
        textures,
        default_textures,
    );
    // TODO: Does the order the sorting is applied matter here?
    meshes.sort_by(|a, b| {
        (render_pass_index(&a.1) as i32 + a.0.sort_bias)
            .cmp(&(render_pass_index(&b.1) as i32 + b.0.sort_bias))
    });
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
    textures: &[(String, NutexbFile)],
    default_textures: &[(&'static str, wgpu::Texture)],
) -> Vec<(RenderMesh, String)> {
    // TODO: Find a way to organize this.

    // Ideally we only create one pipeline per used shader.
    // Mesh objects control the depth state of the pipeline.
    // In practice, each material/mesh pair may need a unique pipeline.
    // TODO: How to test this optimization?
    // TODO: Does it matter that these are cloned one more time than necessary?
    let create_pipeline_data = |material, depth_write, depth_test| {
        let pipeline = create_pipeline(
            device,
            layout,
            shader,
            surface_format,
            material,
            depth_write,
            depth_test,
        );

        let textures_bind_group =
            create_textures_bind_group(material, device, queue, folder, textures, default_textures);
        let material_uniforms_bind_group = create_uniforms_bind_group(material, device);

        Arc::new(PipelineData {
            pipeline: pipeline,
            textures_bind_group,
            material_uniforms_bind_group,
        })
    };

    // TODO: Split into PerMaterial, PerObject, etc in the shaders?

    let mut pipelines = HashMap::new();

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

            // Pipeline creation is expensive.
            // Lazily initialize pipelines and share pipelines when possible.
            // TODO: Handle missing materials?
            let pipeline = pipelines
                .entry((
                    material.unwrap().material_label.clone(),
                    !mesh_object.disable_depth_write,
                    !mesh_object.disable_depth_test,
                ))
                .or_insert_with(|| {
                    create_pipeline_data(
                        material,
                        !mesh_object.disable_depth_write,
                        !mesh_object.disable_depth_test,
                    )
                });

            let (vertex_buffer0, vertex_buffer1, index_buffer, vertex_index_count) =
                mesh_object_buffers(mesh_object, device);

            let transforms_buffer = create_transforms_buffer(mesh_object, skel, device);

            let mesh = RenderMesh {
                pipeline: pipeline.clone(),
                vertex_buffer0,
                vertex_buffer1,
                index_buffer,
                vertex_index_count,
                sort_bias: mesh_object.sort_bias,
                transforms_bind_group: crate::shader::model::bind_groups::BindGroup1::from_bindings(
                    device,
                    crate::shader::model::bind_groups::BindGroupLayout1 {
                        transforms: &transforms_buffer,
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

fn create_uniforms_bind_group(
    material: Option<&ssbh_data::matl_data::MatlEntryData>,
    device: &wgpu::Device,
) -> crate::shader::model::bind_groups::BindGroup3 {
    let uniforms_buffer = create_uniforms_buffer(material, device);
    crate::shader::model::bind_groups::BindGroup3::from_bindings(
        device,
        crate::shader::model::bind_groups::BindGroupLayout3 {
            uniforms: &uniforms_buffer,
        },
    )
}

fn create_textures_bind_group(
    material: Option<&ssbh_data::matl_data::MatlEntryData>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    folder: &str,
    textures: &[(String, NutexbFile)],
    default_textures: &[(&'static str, wgpu::Texture)],
) -> crate::shader::model::bind_groups::BindGroup2 {
    // TODO: Avoid creating defaults more than once?
    let load_texture_sampler = |texture_id, sampler_id| {
        load_texture_sampler(
            material,
            device,
            queue,
            folder,
            texture_id,
            sampler_id,
            textures,
            default_textures,
        )
    };
    // TODO: Do all textures default to white if the path isn't correct?
    // TODO: Default cube map?
    let (_, default_white) = default_textures
        .iter()
        .find(|d| d.0 == "/common/shader/sfxpbs/default_white")
        .unwrap();
    let default_white = (
        default_white.create_view(&TextureViewDescriptor::default()),
        device.create_sampler(&SamplerDescriptor::default()),
    );
    // TODO: Better cube map handling.
    // TODO: This should be part of the default textures?
    // let stage_cube = load_default_cube(device, queue).unwrap();
    // TODO: Default texture for other cube maps?
    let (_, stage_cube) = default_textures
        .iter()
        .find(|d| d.0 == "#replace_cubemap")
        .unwrap();
    let stage_cube = (
        stage_cube.create_view(&TextureViewDescriptor {
            dimension: Some(TextureViewDimension::Cube),
            ..Default::default()
        }),
        device.create_sampler(&SamplerDescriptor::default()),
    );
    // TODO: Avoid loading texture files more than once.
    // This can be done by creating a HashMap<Path, Texture>.
    // Most textures will be used, so it doesn't make sense to lazy load them.
    // TODO: Generate this using a macro?
    let texture0 = load_texture_sampler(ParamId::Texture0, ParamId::Sampler0);
    let texture1 = load_texture_sampler(ParamId::Texture1, ParamId::Sampler1);
    // let texture2 = load_texture_sampler_cube(ParamId::Texture2, ParamId::Sampler2).unwrap();
    let texture3 = load_texture_sampler(ParamId::Texture3, ParamId::Sampler3);
    let texture4 = load_texture_sampler(ParamId::Texture4, ParamId::Sampler4);
    let texture5 = load_texture_sampler(ParamId::Texture5, ParamId::Sampler5);
    let texture6 = load_texture_sampler(ParamId::Texture6, ParamId::Sampler6);
    // let texture7 = load_texture_sampler_cube(ParamId::Texture7, ParamId::Sampler7).unwrap();
    // let texture8 = load_texture_sampler_cube(ParamId::Texture8, ParamId::Sampler8).unwrap();
    let texture9 = load_texture_sampler(ParamId::Texture9, ParamId::Sampler9);
    let texture10 = load_texture_sampler(ParamId::Texture10, ParamId::Sampler10);
    let texture11 = load_texture_sampler(ParamId::Texture11, ParamId::Sampler11);
    let texture12 = load_texture_sampler(ParamId::Texture12, ParamId::Sampler12);
    let texture13 = load_texture_sampler(ParamId::Texture13, ParamId::Sampler13);
    let texture14 = load_texture_sampler(ParamId::Texture14, ParamId::Sampler14);

    crate::shader::model::bind_groups::BindGroup2::from_bindings(
        device,
        crate::shader::model::bind_groups::BindGroupLayout2 {
            texture0: &texture0.as_ref().unwrap_or(&default_white).0,
            sampler0: &texture0.as_ref().unwrap_or(&default_white).1,
            texture1: &texture1.as_ref().unwrap_or(&default_white).0,
            sampler1: &texture1.as_ref().unwrap_or(&default_white).1,
            texture2: &stage_cube.0,
            sampler2: &stage_cube.1,
            texture3: &texture3.as_ref().unwrap_or(&default_white).0,
            sampler3: &texture3.as_ref().unwrap_or(&default_white).1,
            texture4: &texture4.as_ref().unwrap_or(&default_white).0,
            sampler4: &texture4.as_ref().unwrap_or(&default_white).1,
            texture5: &texture5.as_ref().unwrap_or(&default_white).0,
            sampler5: &texture5.as_ref().unwrap_or(&default_white).1,
            texture6: &texture6.as_ref().unwrap_or(&default_white).0,
            sampler6: &texture6.as_ref().unwrap_or(&default_white).1,
            texture7: &stage_cube.0,
            sampler7: &stage_cube.1,
            texture8: &stage_cube.0,
            sampler8: &stage_cube.1,
            texture9: &texture9.as_ref().unwrap_or(&default_white).0,
            sampler9: &texture9.as_ref().unwrap_or(&default_white).1,
            texture10: &texture10.as_ref().unwrap_or(&default_white).0,
            sampler10: &texture10.as_ref().unwrap_or(&default_white).1,
            texture11: &texture11.as_ref().unwrap_or(&default_white).0,
            sampler11: &texture11.as_ref().unwrap_or(&default_white).1,
            texture12: &texture12.as_ref().unwrap_or(&default_white).0,
            sampler12: &texture12.as_ref().unwrap_or(&default_white).1,
            texture13: &texture13.as_ref().unwrap_or(&default_white).0,
            sampler13: &texture13.as_ref().unwrap_or(&default_white).1,
            texture14: &texture14.as_ref().unwrap_or(&default_white).0,
            sampler14: &texture14.as_ref().unwrap_or(&default_white).1,
        },
    )
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

fn render_pass_index(tag: &str) -> usize {
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
        render_pass.set_pipeline(&mesh.pipeline.as_ref().pipeline);

        crate::shader::model::bind_groups::set_bind_groups(
            render_pass,
            crate::shader::model::bind_groups::BindGroups::<'a> {
                bind_group0: camera_bind_group,
                bind_group1: &mesh.transforms_bind_group,
                bind_group2: &mesh.pipeline.as_ref().textures_bind_group,
                bind_group3: &mesh.pipeline.as_ref().material_uniforms_bind_group,
            },
        );

        mesh.set_vertex_buffers(render_pass);
        mesh.set_index_buffer(render_pass);

        // println!("Set Render State: {:?}", start.elapsed());
        render_pass.draw_indexed(0..mesh.vertex_index_count, 0, 0..1);
    }
}
