use ssbh_data::mesh_data::MeshObjectData;
use wgpu::{util::DeviceExt, Buffer, Device};

// TODO: Generate structs for the vertex inputs in wgsl_to_wgpu

// TODO: Create a function and tests that groups attributes into two buffers
// TODO: Crevice for std140/430 layout to avoid alignment issues?
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct VertexBuffer0 {
    position0: [f32; 4],
    normal0: [f32; 4],
    tangent0: [f32; 4],
}

// TODO: Add remaining attributes.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct VertexBuffer1 {
    map1_uv_set: glam::Vec4,
    uv_set1_uv_set2: glam::Vec4,
    bake1_color_set67: glam::Vec4,
    color_set1345: glam::Vec4,     // TODO: use [u32; 4]?
    color_set2_packed: glam::Vec4, // TODO: use [u32; 4]?
}

fn buffer0(mesh_data: &MeshObjectData) -> Vec<VertexBuffer0> {
    let mut vertices = Vec::new();

    // TODO: Refactor this to be cleaner.

    // Always pad to the same size to reuse the program pipeline.
    // TODO: Handle this case by returning no vertices?
    // TODO: Make sure everything has the same length.
    let positions: Vec<_> = mesh_data.positions[0].data.to_vec4_with_w(1.0);

    let normals: Vec<_> = mesh_data.normals[0].data.to_vec4_with_w(1.0);

    // TODO: Add a padding function that preserves w?
    let tangents: Vec<_> = match &mesh_data.tangents[0].data {
        ssbh_data::mesh_data::VectorData::Vector2(_) => todo!(),
        ssbh_data::mesh_data::VectorData::Vector3(v) => {
            v.iter().map(|[x, y, z]| [*x, *y, *z, 1.0]).collect()
        }
        ssbh_data::mesh_data::VectorData::Vector4(v) => {
            v.iter().map(|[x, y, z, w]| [*x, *y, *z, *w]).collect()
        }
    };

    for ((position, normal), tangent) in positions
        .into_iter()
        .zip(normals.into_iter())
        .zip(tangents.into_iter())
    {
        vertices.push(VertexBuffer0 {
            position0: position,
            normal0: normal,
            tangent0: tangent,
        })
    }

    vertices
}

fn buffer1(mesh_data: &MeshObjectData) -> Vec<VertexBuffer1> {
    // TODO: Actually check the attribute names.
    // TODO: How to assign attributes efficiently?
    match &mesh_data.texture_coordinates[0].data {
        ssbh_data::mesh_data::VectorData::Vector2(uvs) => uvs
            .iter()
            .map(|uv| VertexBuffer1 {
                map1_uv_set: glam::Vec4::new(uv[0], uv[1], 0.0, 0.0),
                uv_set1_uv_set2: glam::Vec4::ZERO,
                bake1_color_set67: glam::Vec4::ZERO,
                color_set1345: glam::Vec4::ZERO,
                color_set2_packed: glam::Vec4::ZERO,
            })
            .collect(),
        ssbh_data::mesh_data::VectorData::Vector3(_) => todo!(),
        ssbh_data::mesh_data::VectorData::Vector4(_) => todo!(),
    }
}

pub fn mesh_object_buffers(
    mesh_object: &MeshObjectData,
    device: &Device,
) -> (Buffer, Buffer, Buffer, u32) {
    // TODO: Clean this up.
    let buffer0_vertices = buffer0(mesh_object);
    let vertex_buffer0 = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("vertex buffer 0"),
        contents: bytemuck::cast_slice(&buffer0_vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let buffer1_vertices = buffer1(mesh_object);
    let vertex_buffer1 = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("vertex buffer 1"),
        contents: bytemuck::cast_slice(&buffer1_vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Index Buffer"),
        contents: bytemuck::cast_slice(&mesh_object.vertex_indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    (
        vertex_buffer0,
        vertex_buffer1,
        index_buffer,
        mesh_object.vertex_indices.len() as u32,
    )
}
