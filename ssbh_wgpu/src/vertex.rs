use ssbh_data::mesh_data::MeshObjectData;
use wgpu::{util::DeviceExt, Buffer, Device};
use crate::shader::model::vertex::{VertexInput0, VertexInput1};

// TODO: Create a function and tests that groups attributes into two buffers
// TODO: Crevice for std140/430 layout to avoid alignment issues?
fn buffer0(mesh_data: &MeshObjectData) -> Vec<VertexInput0> {
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
        vertices.push(VertexInput0 {
            position0: position,
            normal0: normal,
            tangent0: tangent,
        })
    }

    vertices
}

// TODO: Support other lengths?
macro_rules! set_attribute {
    ($v:ident, $data:expr, $field:ident, $dst1: literal, $dst2:literal) => {
        match $data {
            ssbh_data::mesh_data::VectorData::Vector2(values) => {
                for (i, value) in values.iter().enumerate() {
                    $v[i].$field[$dst1] = value[0];
                    $v[i].$field[$dst2] = value[1];
                }
            }
            ssbh_data::mesh_data::VectorData::Vector3(_) => todo!(),
            ssbh_data::mesh_data::VectorData::Vector4(_) => todo!(),
        }
    };
}

fn buffer1(mesh_data: &MeshObjectData) -> Vec<VertexInput1> {
    // TODO: Actually check the attribute names.
    // TODO: How to assign attributes efficiently?
    // TODO: More robustly determine vertex count?
    let vertex_count = mesh_data.positions[0].data.len();

    // TODO: This could be done by zeroing memory but probably isn't worth it.
    let mut vertices = vec![VertexInput1::default(); vertex_count];
    
    for attribute in &mesh_data.texture_coordinates {
        match attribute.name.as_str() {
            "map1" => set_attribute!(vertices, &attribute.data, map1_uvset, 0, 1),
            "uvSet" => set_attribute!(vertices, &attribute.data, map1_uvset, 2, 3),
            "uvSet1" => set_attribute!(vertices, &attribute.data, uv_set1_uv_set2, 0, 1),
            "uvSet2" => set_attribute!(vertices, &attribute.data, uv_set1_uv_set2, 2, 3),
            "bake1" => set_attribute!(vertices, &attribute.data, bake1, 0, 1),
            // TODO: color sets
            _ => (),
        }
    }

    vertices
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
