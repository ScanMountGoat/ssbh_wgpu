use crate::{
    shader::{
        model::{VertexInput0, VertexInput1},
        skinning::VertexWeight,
    },
    DeviceBufferExt,
};
use log::warn;
use ssbh_data::{
    mesh_data::{error::Error, MeshObjectData},
    skel_data::SkelData,
};
use wgpu::Device;

// TODO: Create a function and tests that groups attributes into two buffers
pub fn buffer0(mesh_data: &MeshObjectData) -> Result<Vec<VertexInput0>, Error> {
    // Use ssbh_data's vertex count validation.
    let vertex_count = mesh_data.vertex_count()?;

    // TODO: Refactor this to be cleaner.
    let mut vertices = Vec::new();

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
            ssbh_data::mesh_data::VectorData::Vector2(v) => {
                v.iter().map(|[x, y]| [*x, *y, 0.0, 1.0]).collect()
            }
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
            position0: position.into(),
            normal0: normal.into(),
            tangent0: tangent.into(),
        })
    }

    Ok(vertices)
}

// TODO: Support and test other lengths?
macro_rules! set_uv_attribute {
    ($v:ident, $data:expr, $field:ident, $dst1: literal, $dst2:literal) => {
        match $data {
            ssbh_data::mesh_data::VectorData::Vector2(values) => {
                for (i, value) in values.iter().enumerate() {
                    $v[i].$field[$dst1] = value[0];
                    $v[i].$field[$dst2] = value[1];
                }
            }
            ssbh_data::mesh_data::VectorData::Vector3(values) => {
                for (i, value) in values.iter().enumerate() {
                    $v[i].$field[$dst1] = value[0];
                    $v[i].$field[$dst2] = value[1];
                }
            }
            ssbh_data::mesh_data::VectorData::Vector4(values) => {
                for (i, value) in values.iter().enumerate() {
                    $v[i].$field[$dst1] = value[0];
                    $v[i].$field[$dst2] = value[1];
                }
            }
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
                    $v[i].$field[$dst] = value[0];
                    $v[i].$field[$dst + 1] = value[1];
                    $v[i].$field[$dst + 2] = value[2];
                    $v[i].$field[$dst + 3] = value[3];
                }
            }
        }
    };
}

pub fn buffer1(mesh_data: &MeshObjectData) -> Result<Vec<VertexInput1>, Error> {
    // TODO: How to assign attributes efficiently?
    // Use ssbh_data's vertex count validation.
    let vertex_count = mesh_data.vertex_count()?;

    // TODO: This could be done by zeroing memory but probably isn't worth it.
    let mut vertices = vec![
        VertexInput1 {
            map1_uvset: glam::Vec4::ZERO,
            uv_set1_uv_set2: glam::Vec4::ZERO,
            bake1: glam::Vec4::ZERO,
            color_set1: glam::Vec4::ZERO,
            color_set2_combined: glam::Vec4::ZERO,
            color_set3: glam::Vec4::ZERO,
            color_set4: glam::Vec4::ZERO,
            color_set5: glam::Vec4::ZERO,
            color_set6: glam::Vec4::ZERO,
            color_set7: glam::Vec4::ZERO
        };
        vertex_count
    ];

    for attribute in &mesh_data.texture_coordinates {
        match attribute.name.as_str() {
            "map1" => set_uv_attribute!(vertices, &attribute.data, map1_uvset, 0, 1),
            "uvSet" => set_uv_attribute!(vertices, &attribute.data, map1_uvset, 2, 3),
            "uvSet1" => set_uv_attribute!(vertices, &attribute.data, uv_set1_uv_set2, 0, 1),
            "uvSet2" => set_uv_attribute!(vertices, &attribute.data, uv_set1_uv_set2, 2, 3),
            "bake1" => set_uv_attribute!(vertices, &attribute.data, bake1, 0, 1),
            _ => (),
        }
    }

    for attribute in &mesh_data.color_sets {
        // TODO: Investigate how the game combines colorSet2, colorSet2_1, colorSet2_2.
        match attribute.name.as_str() {
            "colorSet1" => set_color_attribute!(vertices, &attribute.data, color_set1, 0),
            "colorSet2" => set_color_attribute!(vertices, &attribute.data, color_set2_combined, 0),
            "colorSet3" => set_color_attribute!(vertices, &attribute.data, color_set3, 0),
            "colorSet4" => set_color_attribute!(vertices, &attribute.data, color_set4, 0),
            "colorSet5" => set_color_attribute!(vertices, &attribute.data, color_set5, 0),
            "colorSet6" => set_color_attribute!(vertices, &attribute.data, color_set6, 0),
            "colorSet7" => set_color_attribute!(vertices, &attribute.data, color_set7, 0),
            _ => (),
        }
    }

    Ok(vertices)
}

impl VertexWeight {
    fn add_weight(&mut self, index: i32, weight: f32) -> bool {
        // Assume unitialized indices have an index of -1.
        // TODO: How does in game ignore more than 4 influences?
        for i in 0..4 {
            if self.bone_indices[i] < 0 {
                self.bone_indices[i] = index;
                self.weights[i] = weight;
                return true;
            }
        }

        false
    }
}

impl Default for VertexWeight {
    fn default() -> Self {
        Self {
            bone_indices: glam::IVec4::splat(-1),
            weights: glam::Vec4::ZERO,
        }
    }
}

pub fn skin_weights(
    mesh: &MeshObjectData,
    skel: Option<&SkelData>,
) -> Result<Vec<VertexWeight>, Error> {
    let vertex_count = mesh.vertex_count()?;

    // Use the default weight to represent no influences.
    // TODO: What is the in game behavior?
    let mut weights = vec![VertexWeight::default(); vertex_count];

    if let Some(skel) = skel {
        for influence in &mesh.bone_influences {
            if let Some(bone_index) = skel
                .bones
                .iter()
                .position(|b| b.name == influence.bone_name)
            {
                // Collect influences per vertex.
                // TODO: How to handle meshes with no influences but a parent bone?
                for w in &influence.vertex_weights {
                    if let Some(weight) = weights.get_mut(w.vertex_index as usize) {
                        if !weight.add_weight(bone_index as i32, w.vertex_weight) {
                            warn!(
                                "Vertex {} for mesh {} has more than 4 weights. Additional weights will be ignored.", 
                                w.vertex_index,
                                mesh.name,
                            );
                        }
                    } else {
                        warn!(
                            "Vertex weight assigns to vertex {}, which is out of range for mesh {} with {} vertices.",
                            w.vertex_index,
                            mesh.name,
                            vertex_count,
                        );
                    }
                }
            }
        }
    }

    // TODO: Log if there are any unweighted vertices but no parent bone.
    Ok(weights)
}

pub struct MeshObjectBufferData {
    pub vertex_buffer0_source: wgpu::Buffer,
    pub vertex_buffer0: wgpu::Buffer,
    pub vertex_buffer1: wgpu::Buffer,
    pub skinning_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
}

pub fn mesh_object_buffers(
    device: &Device,
    // TODO: Is this the best way to allow for aligning the buffers?
    buffer0: &[u8],
    buffer1: &[u8],
    skin_weights: &[u8],
    vertex_indices: &[u32],
) -> MeshObjectBufferData {
    // TODO: Clean this up.
    // TODO: Validate the vertex count and indices?
    // Keep a separate copy of the non transformed data.
    let vertex_buffer0_source =
        device.create_buffer_from_bytes("Vertex Buffer 0", buffer0, wgpu::BufferUsages::STORAGE);

    // This buffer will be filled by the compute shader later.
    // The buffer is transformed in a compute shader later, so it must support STORAGE.
    // Assume buffer0 is already padded/aligned to the requirements of a storage buffer.
    let vertex_buffer0 = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Vertex Storage Buffer 0"),
        size: buffer0.len() as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
        mapped_at_creation: false,
    });

    let vertex_buffer1 =
        device.create_buffer_from_bytes("Vertex Buffer 1", buffer1, wgpu::BufferUsages::VERTEX);

    let skinning_buffer = device.create_buffer_from_bytes(
        "Skinning Buffer",
        skin_weights,
        wgpu::BufferUsages::STORAGE,
    );

    let index_buffer = device.create_index_buffer("Index Buffer", vertex_indices);

    MeshObjectBufferData {
        vertex_buffer0_source,
        vertex_buffer0,
        vertex_buffer1,
        skinning_buffer,
        index_buffer,
    }
}

#[cfg(test)]
mod tests {
    use ssbh_data::{
        mesh_data::{AttributeData, BoneInfluence},
        skel_data::BoneData,
    };

    use super::*;

    // TODO: Test vertex buffer creation
    // Just test the vertices themselves.
    // Assign each attribute
    // Additional attributes should be ignored.
    // Missing attributes set to 0.
    // TODO: Test invalid vertex counts.

    fn identity_bone(name: &str) -> BoneData {
        BoneData {
            name: name.to_string(),
            transform: [[0.0; 4]; 4],
            parent_index: None,
            billboard_type: ssbh_data::skel_data::BillboardType::Disabled,
        }
    }

    fn bone_influence(name: &str, vertex_index: u32, vertex_weight: f32) -> BoneInfluence {
        BoneInfluence {
            bone_name: name.to_string(),
            vertex_weights: vec![ssbh_data::mesh_data::VertexWeight {
                vertex_index,
                vertex_weight,
            }],
        }
    }

    #[test]
    fn buffer0_empty() {
        assert!(buffer0(&MeshObjectData::default()).unwrap().is_empty());
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
        })
        .unwrap();

        assert_eq!(
            vec![VertexInput0 {
                position0: glam::Vec4::ZERO,
                normal0: glam::Vec4::ZERO,
                tangent0: glam::vec4(0.0, 0.0, 0.0, 1.0)
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
        })
        .unwrap();

        assert_eq!(
            vec![VertexInput0 {
                position0: glam::vec4(0.0, 1.0, 2.0, 1.0),
                normal0: glam::vec4(2.0, 3.0, 0.0, 1.0),
                tangent0: glam::vec4(4.0, 5.0, 6.0, 1.0)
            }],
            vertices
        );
    }

    #[test]
    fn buffer1_empty() {
        assert!(buffer1(&MeshObjectData::default()).unwrap().is_empty());
    }

    #[test]
    fn add_vertex_weights() {
        let mut weight = VertexWeight::default();
        assert!(weight.add_weight(1, 1.0));
        assert!(weight.add_weight(2, 2.0));
        assert!(weight.add_weight(3, 3.0));
        assert!(weight.add_weight(4, 4.0));
        // The final weight should be ignored.
        assert!(!weight.add_weight(5, 5.0));

        assert_eq!([1, 2, 3, 4], weight.bone_indices.to_array());
        assert_eq!([1.0, 2.0, 3.0, 4.0], weight.weights.to_array());
    }

    #[test]
    fn skin_weights_no_vertices_no_skel() {
        let weights = skin_weights(&MeshObjectData::default(), None).unwrap();
        assert!(weights.is_empty());
    }

    #[test]
    fn skin_weights_no_vertices() {
        let weights = skin_weights(
            &MeshObjectData::default(),
            Some(&SkelData {
                major_version: 1,
                minor_version: 0,
                bones: vec![identity_bone("a")],
            }),
        )
        .unwrap();
        assert!(weights.is_empty());
    }

    #[test]
    fn skin_weights_single_vertex_no_skel() {
        // TODO: Function to create a mesh with a single vertex?
        let weights = skin_weights(
            &MeshObjectData {
                positions: vec![AttributeData {
                    name: "a".to_string(),
                    data: ssbh_data::mesh_data::VectorData::Vector4(vec![[0.0, 1.0, 2.0, 3.0]]),
                }],
                ..Default::default()
            },
            None,
        )
        .unwrap();

        assert_eq!(vec![VertexWeight::default()], weights);
    }

    #[test]
    fn skin_weights_single_vertex() {
        let weights = skin_weights(
            &MeshObjectData {
                positions: vec![AttributeData {
                    name: "a".to_string(),
                    data: ssbh_data::mesh_data::VectorData::Vector4(vec![[0.0, 1.0, 2.0, 3.0]]),
                }],
                bone_influences: vec![
                    bone_influence("a", 0, 1.0),
                    bone_influence("b", 0, 0.75),
                    bone_influence("c", 0, 0.5),
                    bone_influence("d", 0, 0.25),
                    bone_influence("ignored", 0, 0.0),
                ],
                ..Default::default()
            },
            Some(&SkelData {
                major_version: 1,
                minor_version: 0,
                bones: vec![
                    identity_bone("a"),
                    identity_bone("b"),
                    identity_bone("c"),
                    identity_bone("d"),
                    identity_bone("ignored"),
                ],
            }),
        )
        .unwrap();

        // Only keep the first four influences.
        // TODO: Does in game keep the first or last four influences?
        assert_eq!(
            vec![VertexWeight {
                bone_indices: glam::ivec4(0, 1, 2, 3),
                weights: glam::vec4(1.0, 0.75, 0.5, 0.25)
            }],
            weights
        );
    }

    #[test]
    fn skin_weights_single_vertex_invalid_bone() {
        let weights = skin_weights(
            &MeshObjectData {
                positions: vec![AttributeData {
                    name: "a".to_string(),
                    data: ssbh_data::mesh_data::VectorData::Vector4(vec![[0.0, 1.0, 2.0, 3.0]]),
                }],
                bone_influences: vec![bone_influence("invalid", 0, 1.0)],
                ..Default::default()
            },
            Some(&SkelData {
                major_version: 1,
                minor_version: 0,
                bones: vec![identity_bone("a")],
            }),
        )
        .unwrap();

        assert_eq!(vec![VertexWeight::default()], weights);
    }

    #[test]
    fn skin_weights_single_vertex_out_of_range() {
        let weights = skin_weights(
            &MeshObjectData {
                positions: vec![AttributeData {
                    name: "a".to_string(),
                    data: ssbh_data::mesh_data::VectorData::Vector4(vec![[0.0, 1.0, 2.0, 3.0]]),
                }],
                bone_influences: vec![bone_influence("a", 2, 1.0)],
                ..Default::default()
            },
            Some(&SkelData {
                major_version: 1,
                minor_version: 0,
                bones: vec![identity_bone("a")],
            }),
        )
        .unwrap();

        assert_eq!(vec![VertexWeight::default()], weights);
    }
}
