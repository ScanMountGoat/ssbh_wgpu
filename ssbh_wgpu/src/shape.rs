// TODO: Generate the sphere using code.
// TODO: Make the position coordinates have a w coordinate.
// TODO: Use separate buffers for normals and positions.
// TODO: Create a function for each shape returning (VertexBuffer, IndexBuffer)

use std::f32::consts::PI;

use wgpu::util::DeviceExt;

// TODO: Create a type that groups vertex, index buffers, and index count?
// This would reduce the number of fields for shapes, RenderMesh, etc.
// This should also make index count mismatches less frequent.
pub struct IndexedMeshBuffers {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

impl IndexedMeshBuffers {
    pub fn set<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
    }

    fn from_vertices(device: &wgpu::Device, vertices: &[[f32; 4]], indices: &[u32]) -> Self {
        // Add COPY_DST so we can animate swing shapes without allocating new buffers.
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        IndexedMeshBuffers {
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
        }
    }
}

pub fn sphere_mesh_buffers(device: &wgpu::Device) -> IndexedMeshBuffers {
    IndexedMeshBuffers::from_vertices(
        device,
        &sphere_vertices(8, 8, SphereRange::Full),
        &sphere_indices(8, 8, SphereRange::Full),
    )
}

#[derive(PartialEq)]
pub enum SphereRange {
    Full,
    TopHemisphere,
    BottomHemisphere,
}

pub fn sphere_vertices(sector_count: u32, stack_count: u32, range: SphereRange) -> Vec<[f32; 4]> {
    // http://www.songho.ca/opengl/gl_sphere.html
    let mut vertices = Vec::new();

    let radius = 1.0;
    let sector_step = 2.0 * PI / sector_count as f32;
    let stack_step = PI / stack_count as f32;

    let stack_range = match range {
        SphereRange::Full => 0..=stack_count,
        SphereRange::TopHemisphere => 0..=stack_count / 2,
        SphereRange::BottomHemisphere => stack_count / 2..=stack_count,
    };

    for i in stack_range {
        let stack_angle = PI / 2.0 - i as f32 * stack_step;
        let xy = radius * stack_angle.cos();
        let z = radius * stack_angle.sin();

        for j in 0..=sector_count {
            let sector_angle = j as f32 * sector_step;

            // Vertex position.
            let x = xy * sector_angle.cos();
            let y = xy * sector_angle.sin();
            vertices.push([x, y, z, 1.0]);

            // Vertex normal.
            vertices.push([x / radius, y / radius, z / radius, 1.0])
        }
    }

    vertices
}

pub fn sphere_indices(sector_count: u32, stack_count: u32, range: SphereRange) -> Vec<u32> {
    // http://www.songho.ca/opengl/gl_sphere.html
    // Generate a counterclockwise index list of sphere triangles.
    // k1--k1+1
    // |  / |
    // | /  |
    // k2--k2+1
    let mut indices = Vec::new();

    let stack_range = match range {
        SphereRange::Full => 0..stack_count,
        SphereRange::TopHemisphere | SphereRange::BottomHemisphere => 0..stack_count / 2,
    };

    for i in stack_range {
        let mut k1 = i * (sector_count + 1);
        let mut k2 = k1 + sector_count + 1;

        for _ in 0..sector_count {
            // 2 triangles per sector excluding first and last stacks
            // k1 => k2 => k1+1
            // The top sphere should still have its bottom stack.
            // TODO: Create proper hemisphere functions.
            if i != 0 || range == SphereRange::BottomHemisphere {
                indices.push(k1);
                indices.push(k2);
                indices.push(k1 + 1);
            }

            // k1+1 => k2 => k2+1
            if i != (stack_count - 1) {
                indices.push(k1 + 1);
                indices.push(k2);
                indices.push(k2 + 1);
            }

            k1 += 1;
            k2 += 1;
        }
    }

    indices
}

pub fn capsule_mesh_buffers(
    device: &wgpu::Device,
    height: f32,
    radius1: f32,
    radius2: f32,
) -> IndexedMeshBuffers {
    IndexedMeshBuffers::from_vertices(
        device,
        &capsule_vertices(8, 8, height, radius1, radius2),
        &capsule_indices(8, 8),
    )
}

pub fn capsule_vertices(
    sector_count: u32,
    stack_count: u32,
    height: f32,
    radius1: f32,
    radius2: f32,
) -> Vec<[f32; 4]> {
    // Combine two spheres and a cylinder to create a capsule.
    // TODO: Optimize this to use hemispheres.
    // TODO: This should allow a different radius for top and bottom.
    sphere_vertices(sector_count, stack_count, SphereRange::BottomHemisphere)
        .into_iter()
        .map(|v| {
            [
                v[0] * radius1,
                v[1] * radius1,
                v[2] * radius1 - height / 2.0,
                1.0,
            ]
        })
        .chain(
            sphere_vertices(sector_count, stack_count, SphereRange::TopHemisphere)
                .into_iter()
                .map(|v| {
                    [
                        v[0] * radius2,
                        v[1] * radius2,
                        v[2] * radius2 + height / 2.0,
                        1.0,
                    ]
                }),
        )
        .chain(cylinder_vertices(sector_count, height, [radius1, radius2]))
        .collect()
}

fn capsule_indices(sector_count: u32, stack_count: u32) -> Vec<u32> {
    // Combine two spheres and a cylinder to create a capsule.
    // TODO: Optimize this to use hemispheres.
    // TODO: This should be more efficient if vertices and indices are generated in the same function.
    let n1 = sphere_vertices(sector_count, stack_count, SphereRange::BottomHemisphere).len() / 2;
    let n2 = sphere_vertices(sector_count, stack_count, SphereRange::TopHemisphere).len() / 2;

    sphere_indices(sector_count, stack_count, SphereRange::BottomHemisphere)
        .into_iter()
        .chain(
            sphere_indices(sector_count, stack_count, SphereRange::TopHemisphere)
                .into_iter()
                .map(|i| i + n1 as u32),
        )
        .chain(
            cylinder_indices(sector_count)
                .into_iter()
                .map(|i| i + (n1 + n2) as u32),
        )
        .collect()
}

fn unit_circle_vertices(sector_count: u32) -> Vec<[f32; 4]> {
    let mut vertices = Vec::new();
    let sector_step = 2.0 * PI / sector_count as f32;

    for i in 0..=sector_count {
        let sector_angle = i as f32 * sector_step;
        vertices.push([sector_angle.cos(), sector_angle.sin(), 0.0, 1.0])
    }

    vertices
}

fn cylinder_vertices(sector_count: u32, height: f32, radii: [f32; 2]) -> Vec<[f32; 4]> {
    // http://www.songho.ca/opengl/gl_cylinder.html
    // TODO: Add half spheres for the caps on each end.
    let mut vertices = Vec::new();

    // Unit circle on the XY plane to avoid repetitive trig functions.
    let unit_vertices = unit_circle_vertices(sector_count);

    // Generate the side vertices.
    // Omit the caps on the top and bottom of the cylinder for now.
    for (i, radius) in radii.into_iter().enumerate() {
        let h = -height / 2.0 + i as f32 * height;

        for k in 0..=sector_count {
            let [ux, uy, uz, _] = unit_vertices[k as usize];

            // Position vector.
            vertices.push([ux * radius, uy * radius, h, 1.0]);

            // Normal vector.
            vertices.push([ux, uy, uz, 1.0]);
        }
    }

    vertices
}

fn cylinder_indices(sector_count: u32) -> Vec<u32> {
    // http://www.songho.ca/opengl/gl_cylinder.html
    let mut indices = Vec::new();

    for k1 in 0..sector_count {
        let k2 = k1 + sector_count + 1;

        indices.push(k1);
        indices.push(k1 + 1);
        indices.push(k2);

        indices.push(k2);
        indices.push(k1 + 1);
        indices.push(k2 + 1);
    }
    indices
}

pub fn plane_mesh_buffers(device: &wgpu::Device) -> IndexedMeshBuffers {
    IndexedMeshBuffers::from_vertices(device, &plane_vertices(), &plane_indices())
}

fn plane_vertices() -> Vec<[f32; 4]> {
    // The XY plane.
    // Pos0 Nrm0 Pos1 Nrm1 ...
    vec![
        [-1.0, 1.0, 0.0, 1.0],
        [0.0, 0.0, 1.0, 1.0],
        [1.0, 1.0, 0.0, 1.0],
        [0.0, 0.0, 1.0, 1.0],
        [-1.0, -1.0, 0.0, 1.0],
        [0.0, 0.0, 1.0, 1.0],
        [1.0, -1.0, 0.0, 1.0],
        [0.0, 0.0, 1.0, 1.0],
    ]
}

fn plane_indices() -> Vec<u32> {
    // TODO: Is this the correct winding order?
    vec![0, 1, 2, 2, 1, 3]
}
