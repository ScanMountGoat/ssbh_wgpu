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

fn float_to_u8(f: f32) -> u8 {
    (f * 255.0).clamp(0.0, 255.0) as u8
}

fn floats_to_u32(f: &[f32; 4]) -> u32 {
    // TODO: Does gpu memory enforce an endianness?
    u32::from_le_bytes([float_to_u8(f[0]), float_to_u8(f[1]), float_to_u8(f[2]), float_to_u8(f[3])])
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

macro_rules! set_color_attribute {
    ($v:ident, $data:expr, $field:ident, $dst: literal) => {
        match $data {
            ssbh_data::mesh_data::VectorData::Vector2(_) => todo!(),
            ssbh_data::mesh_data::VectorData::Vector3(_) => todo!(),
            ssbh_data::mesh_data::VectorData::Vector4(values) => {
                for (i, value) in values.iter().enumerate() {
                    $v[i].$field[$dst] = floats_to_u32(&value);
                }
            },
        }
    };
}

fn buffer1(mesh_data: &MeshObjectData) -> Vec<VertexInput1> {
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
            _ => (),
        }
    }

    for attribute in &mesh_data.color_sets {
        match attribute.name.as_str() {
            "colorSet1" => set_color_attribute!(vertices, &attribute.data, color_set1345_packed, 0),
            "colorSet3" => set_color_attribute!(vertices, &attribute.data, color_set1345_packed, 1),
            "colorSet4" => set_color_attribute!(vertices, &attribute.data, color_set1345_packed, 2),
            "colorSet5" => set_color_attribute!(vertices, &attribute.data, color_set1345_packed, 3),
            "colorSet2" => set_color_attribute!(vertices, &attribute.data, color_set2_packed, 0),
            "colorSet2_1" => set_color_attribute!(vertices, &attribute.data, color_set2_packed, 1),
            "colorSet2_2" => set_color_attribute!(vertices, &attribute.data, color_set2_packed, 2),
            "colorSet2_3" => set_color_attribute!(vertices, &attribute.data, color_set2_packed, 3),
            "colorSet6" => set_color_attribute!(vertices, &attribute.data, color_set67_packed, 0),
            "colorSet7" => set_color_attribute!(vertices, &attribute.data, color_set67_packed, 1),
            _ => (),
        }
    }

    vertices
}

// TODO: Return a struct.
pub fn mesh_object_buffers(
    mesh_object: &MeshObjectData,
    device: &Device,
) -> (Buffer, Buffer, Buffer, Buffer, u32, u32) {
    // TODO: Clean this up.
    // TODO: Validate the vertex count and indices?

    // The buffer0 is skinned in a compute shader later, so it must support STORAGE.
    // Keep a separate copy of the non transformed data.
    let buffer0_vertices = buffer0(mesh_object);
    let buffer0_data = bytemuck::cast_slice(&buffer0_vertices);
    let vertex_buffer0_source = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("vertex buffer 0"),
        contents: buffer0_data,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
    });

    // This buffer will be filled by the compute shader later.
    let vertex_buffer0 = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("vertex buffer 0"),
        size: buffer0_data.len() as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
        mapped_at_creation: false
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
        vertex_buffer0_source,
        vertex_buffer0,
        vertex_buffer1,
        index_buffer,
        buffer0_vertices.len() as u32,
        mesh_object.vertex_indices.len() as u32,
    )
}
