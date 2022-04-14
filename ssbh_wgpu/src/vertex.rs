use crate::shader::model::{VertexInput0, VertexInput1};
use ssbh_data::{mesh_data::MeshObjectData, skel_data::SkelData};
use wgpu::{util::DeviceExt, Device};

// TODO: Create a function and tests that groups attributes into two buffers
fn buffer0(mesh_data: &MeshObjectData) -> Vec<VertexInput0> {
    // Use ssbh_data's vertex count validation.
    // TODO: Return an error if vertex count is ambiguous?
    let vertex_count = mesh_data.vertex_count().unwrap();
    if vertex_count == 0 {
        return Vec::new();
    }

    let mut vertices = Vec::new();

    // TODO: Refactor this to be cleaner.

    // Pad to vec4 to avoid needing separate pipelines for different meshes.
    let positions: Vec<_> = mesh_data
        .positions
        .first()
        .map(|a| a.data.to_vec4_with_w(1.0))
        .unwrap_or_else(|| vec![[0.0; 4]; vertex_count]);

    let normals: Vec<_> = mesh_data
        .normals
        .first()
        .map(|a| a.data.to_vec4_with_w(1.0))
        .unwrap_or_else(|| vec![[0.0; 4]; vertex_count]);

    // TODO: Add a padding function that preserves w?
    let tangents: Vec<_> = mesh_data
        .tangents
        .first()
        .map(|a| match &a.data {
            ssbh_data::mesh_data::VectorData::Vector2(_) => todo!(),
            ssbh_data::mesh_data::VectorData::Vector3(v) => {
                v.iter().map(|[x, y, z]| [*x, *y, *z, 1.0]).collect()
            }
            ssbh_data::mesh_data::VectorData::Vector4(v) => {
                v.iter().map(|[x, y, z, w]| [*x, *y, *z, *w]).collect()
            }
        })
        .unwrap_or_else(|| vec![[0.0, 0.0, 0.0, 1.0]; vertex_count]);

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
    u32::from_le_bytes([
        float_to_u8(f[0]),
        float_to_u8(f[1]),
        float_to_u8(f[2]),
        float_to_u8(f[3]),
    ])
}

// TODO: Support and test other lengths?
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
            }
        }
    };
}

fn buffer1(mesh_data: &MeshObjectData) -> Vec<VertexInput1> {
    // TODO: How to assign attributes efficiently?
    // TODO: More robustly determine vertex count?
    let vertex_count = mesh_data.vertex_count().unwrap();

    // TODO: This could be done by zeroing memory but probably isn't worth it.
    let mut vertices = vec![
        VertexInput1 {
            map1_uvset: [0.0; 4],
            uv_set1_uv_set2: [0.0; 4],
            bake1: [0.0; 4],
            color_set1345_packed: [0; 4],
            color_set2_packed: [0; 4],
            color_set67_packed: [0; 4]
        };
        vertex_count
    ];

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

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct VertexWeight {
    bone_indices: [i32; 4],
    weights: [f32; 4],
}

impl VertexWeight {
    fn add_weight(&mut self, index: i32, weight: f32) {
        // Assume unitialized indices have an index of -1.
        // TODO: How does in game ignore more than 4 influences?
        for i in 0..4 {
            if self.bone_indices[i] < 0 {
                self.bone_indices[i] = index;
                self.weights[i] = weight;
                break;
            }
        }
    }
}

impl Default for VertexWeight {
    fn default() -> Self {
        Self {
            bone_indices: [-1; 4],
            weights: [0.0; 4],
        }
    }
}

fn skin_weights(
    mesh_data: &MeshObjectData,
    skel: &Option<SkelData>,
    vertex_count: usize,
) -> Vec<VertexWeight> {
    match skel {
        Some(skel) => {
            if mesh_data.bone_influences.is_empty() {
                vec![VertexWeight::default(); vertex_count]
            } else {
                // Collect influences per vertex.
                let mut weights = vec![VertexWeight::default(); vertex_count];

                for influence in &mesh_data.bone_influences {
                    if let Some(bone_index) = skel
                        .bones
                        .iter()
                        .position(|b| b.name == influence.bone_name)
                    {
                        // TODO: How to handle meshes with no influences but a parent bone?
                        for w in &influence.vertex_weights {
                            weights[w.vertex_index as usize]
                                .add_weight(bone_index as i32, w.vertex_weight)
                        }
                    }
                }

                weights
            }
        }
        None => {
            // TODO: How to handle a missing skel?
            vec![VertexWeight::default(); vertex_count]
        }
    }
}

pub struct MeshObjectBufferData {
    pub vertex_buffer0_source: wgpu::Buffer,
    pub vertex_buffer0: wgpu::Buffer,
    pub vertex_buffer1: wgpu::Buffer,
    pub skinning_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub vertex_count: usize,
    pub vertex_index_count: usize,
}

pub fn mesh_object_buffers(
    device: &Device,
    mesh_object: &MeshObjectData,
    skel: &Option<SkelData>,
) -> MeshObjectBufferData {
    // TODO: Clean this up.
    // TODO: Validate the vertex count and indices?
    let vertex_count = mesh_object.vertex_count().unwrap();

    // The buffer0 is skinned in a compute shader later, so it must support STORAGE.
    // Keep a separate copy of the non transformed data.
    let buffer0_vertices = buffer0(mesh_object);
    let buffer0_data = bytemuck::cast_slice(&buffer0_vertices);
    let vertex_buffer0_source = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("vertex buffer 0"),
        contents: buffer0_data,
        usage: wgpu::BufferUsages::STORAGE,
    });

    // This buffer will be filled by the compute shader later.
    let vertex_buffer0 = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Vertex Buffer 0"),
        size: buffer0_data.len() as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
        mapped_at_creation: false,
    });

    let buffer1_vertices = buffer1(mesh_object);
    let vertex_buffer1 = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Vertex Buffer 1"),
        contents: bytemuck::cast_slice(&buffer1_vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let skin_weights = skin_weights(mesh_object, skel, vertex_count);

    let skinning_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Skinning Buffer"),
        contents: bytemuck::cast_slice(&skin_weights),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Index Buffer"),
        contents: bytemuck::cast_slice(&mesh_object.vertex_indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    MeshObjectBufferData {
        vertex_buffer0_source,
        vertex_buffer0,
        vertex_buffer1,
        skinning_buffer,
        index_buffer,
        vertex_count,
        vertex_index_count: mesh_object.vertex_indices.len(),
    }
}

#[cfg(test)]
mod tests {
    use ssbh_data::mesh_data::AttributeData;

    use super::*;

    // TODO: Test vertex buffer creation
    // Just test the vertices themselves.
    // Assign each attribute
    // Additional attributes should be ignored.
    // Missing attributes set to 0.
    // TODO: Test invalid vertex counts.

    #[test]
    fn buffer0_empty() {
        assert!(buffer0(&MeshObjectData::default()).is_empty());
    }

    #[test]
    fn buffer0_missing_attributes() {
        // TODO: How to handle this case?
        // Some meshes may be missing positions, normals, or tangents.
        // TODO: Is this an attribute error (yellow checkerboard) in Smash Ultimate?
        let vertices = buffer0(&MeshObjectData {
            texture_coordinates: vec![AttributeData {
                name: "a".to_string(),
                data: ssbh_data::mesh_data::VectorData::Vector2(vec![[0.0, 1.0]]),
            }],
            ..Default::default()
        });

        assert_eq!(
            vec![VertexInput0 {
                position0: [0.0; 4],
                normal0: [0.0; 4],
                tangent0: [0.0, 0.0, 0.0, 1.0]
            }],
            vertices
        );
    }

    #[test]
    fn buffer0_single_vertex() {
        // Test padding of vectors.
        let vertices = buffer0(&MeshObjectData {
            positions: vec![AttributeData {
                name: "a".to_string(),
                data: ssbh_data::mesh_data::VectorData::Vector4(vec![[0.0, 1.0, 2.0, 3.0]]),
            }],
            normals: vec![AttributeData {
                name: "a".to_string(),
                data: ssbh_data::mesh_data::VectorData::Vector2(vec![[2.0, 3.0]]),
            }],
            tangents: vec![AttributeData {
                name: "a".to_string(),
                data: ssbh_data::mesh_data::VectorData::Vector3(vec![[4.0, 5.0, 6.0]]),
            }],
            ..Default::default()
        });

        assert_eq!(
            vec![VertexInput0 {
                position0: [0.0, 1.0, 2.0, 1.0],
                normal0: [2.0, 3.0, 0.0, 1.0],
                tangent0: [4.0, 5.0, 6.0, 1.0]
            }],
            vertices
        );
    }

    #[test]
    fn buffer1_empty() {
        assert!(buffer1(&MeshObjectData::default()).is_empty());
    }
}
