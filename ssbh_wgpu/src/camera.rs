use wgpu::util::DeviceExt;

pub fn create_camera_bind_group(
    device: &wgpu::Device,
    camera_pos: glam::Vec4,
    mvp_matrix: glam::Mat4,
) -> (wgpu::Buffer, crate::shader::model::bind_groups::BindGroup0) {
    let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Camera Buffer"),
        contents: bytemuck::cast_slice(&[crate::shader::model::CameraTransforms {
            mvp_matrix,
            camera_pos: camera_pos.to_array(),
        }]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });
    let camera_bind_group = crate::shader::model::bind_groups::BindGroup0::from_bindings(
        device,
        crate::shader::model::bind_groups::BindGroupLayout0 {
            camera: &camera_buffer,
        },
    );
    (camera_buffer, camera_bind_group)
}
