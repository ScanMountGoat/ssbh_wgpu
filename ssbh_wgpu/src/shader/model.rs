// File automatically generated by build.rs.
// Changes made to this file will not be saved.
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraTransforms {
    pub mvp_matrix: glam::Mat4,
    pub camera_pos: [f32; 4],
}
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialUniforms {
    pub custom_vector: [[f32; 4]; 64],
    pub custom_boolean: [[f32; 4]; 20],
    pub custom_float: [[f32; 4]; 20],
    pub has_float: [[f32; 4]; 20],
    pub has_texture: [[f32; 4]; 19],
    pub has_vector: [[f32; 4]; 64],
}
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexInput0 {
    pub position0: [f32; 4],
    pub normal0: [f32; 4],
    pub tangent0: [f32; 4],
}
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexInput1 {
    pub map1_uvset: [f32; 4],
    pub uv_set1_uv_set2: [f32; 4],
    pub bake1: [f32; 4],
    pub color_set1345_packed: [u32; 4],
    pub color_set2_packed: [u32; 4],
    pub color_set67_packed: [u32; 4],
}
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexOutput {
    pub clip_position: [f32; 4],
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tangent: [f32; 4],
    pub map1_uvset: [f32; 4],
    pub uv_set1_uv_set2: [f32; 4],
    pub bake1: [f32; 4],
    pub color_set1345_packed: [u32; 4],
    pub color_set2_packed: [u32; 4],
    pub color_set67_packed: [u32; 4],
}
pub mod bind_groups {
    pub struct BindGroup0(wgpu::BindGroup);
    pub struct BindGroupLayout0<'a> {
        pub camera: &'a wgpu::Buffer,
    }
    const LAYOUT_DESCRIPTOR0: wgpu::BindGroupLayoutDescriptor = wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ]
    };
    impl BindGroup0 {
        pub fn get_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
            device.create_bind_group_layout(&LAYOUT_DESCRIPTOR0)
        }
    
        pub fn from_bindings(device: &wgpu::Device, bindings: BindGroupLayout0) -> Self {
            let bind_group_layout = device.create_bind_group_layout(&LAYOUT_DESCRIPTOR0);
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0u32,
                        resource: bindings.camera.as_entire_binding(),
                    },
                ],
                label: None,
            });
            Self(bind_group)
        }
    
        pub fn set<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
            render_pass.set_bind_group(0u32, &self.0, &[]);
        }
    }
    pub struct BindGroup1(wgpu::BindGroup);
    pub struct BindGroupLayout1<'a> {
        pub texture0: &'a wgpu::TextureView,
        pub sampler0: &'a wgpu::Sampler,
        pub texture1: &'a wgpu::TextureView,
        pub sampler1: &'a wgpu::Sampler,
        pub texture2: &'a wgpu::TextureView,
        pub sampler2: &'a wgpu::Sampler,
        pub texture3: &'a wgpu::TextureView,
        pub sampler3: &'a wgpu::Sampler,
        pub texture4: &'a wgpu::TextureView,
        pub sampler4: &'a wgpu::Sampler,
        pub texture5: &'a wgpu::TextureView,
        pub sampler5: &'a wgpu::Sampler,
        pub texture6: &'a wgpu::TextureView,
        pub sampler6: &'a wgpu::Sampler,
        pub texture7: &'a wgpu::TextureView,
        pub sampler7: &'a wgpu::Sampler,
        pub texture8: &'a wgpu::TextureView,
        pub sampler8: &'a wgpu::Sampler,
        pub texture9: &'a wgpu::TextureView,
        pub sampler9: &'a wgpu::Sampler,
        pub texture10: &'a wgpu::TextureView,
        pub sampler10: &'a wgpu::Sampler,
        pub texture11: &'a wgpu::TextureView,
        pub sampler11: &'a wgpu::Sampler,
        pub texture12: &'a wgpu::TextureView,
        pub sampler12: &'a wgpu::Sampler,
        pub texture13: &'a wgpu::TextureView,
        pub sampler13: &'a wgpu::Sampler,
        pub texture14: &'a wgpu::TextureView,
        pub sampler14: &'a wgpu::Sampler,
    }
    const LAYOUT_DESCRIPTOR1: wgpu::BindGroupLayoutDescriptor = wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::Cube,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 5u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 6u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 7u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 8u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 9u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 10u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 11u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 12u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 13u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 14u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::Cube,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 15u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 16u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::Cube,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 17u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 18u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 19u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 20u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 21u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 22u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 23u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 24u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 25u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 26u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 27u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 28u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 29u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ]
    };
    impl BindGroup1 {
        pub fn get_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
            device.create_bind_group_layout(&LAYOUT_DESCRIPTOR1)
        }
    
        pub fn from_bindings(device: &wgpu::Device, bindings: BindGroupLayout1) -> Self {
            let bind_group_layout = device.create_bind_group_layout(&LAYOUT_DESCRIPTOR1);
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0u32,
                        resource: wgpu::BindingResource::TextureView(bindings.texture0),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1u32,
                        resource: wgpu::BindingResource::Sampler(bindings.sampler0),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2u32,
                        resource: wgpu::BindingResource::TextureView(bindings.texture1),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3u32,
                        resource: wgpu::BindingResource::Sampler(bindings.sampler1),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4u32,
                        resource: wgpu::BindingResource::TextureView(bindings.texture2),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5u32,
                        resource: wgpu::BindingResource::Sampler(bindings.sampler2),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6u32,
                        resource: wgpu::BindingResource::TextureView(bindings.texture3),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7u32,
                        resource: wgpu::BindingResource::Sampler(bindings.sampler3),
                    },
                    wgpu::BindGroupEntry {
                        binding: 8u32,
                        resource: wgpu::BindingResource::TextureView(bindings.texture4),
                    },
                    wgpu::BindGroupEntry {
                        binding: 9u32,
                        resource: wgpu::BindingResource::Sampler(bindings.sampler4),
                    },
                    wgpu::BindGroupEntry {
                        binding: 10u32,
                        resource: wgpu::BindingResource::TextureView(bindings.texture5),
                    },
                    wgpu::BindGroupEntry {
                        binding: 11u32,
                        resource: wgpu::BindingResource::Sampler(bindings.sampler5),
                    },
                    wgpu::BindGroupEntry {
                        binding: 12u32,
                        resource: wgpu::BindingResource::TextureView(bindings.texture6),
                    },
                    wgpu::BindGroupEntry {
                        binding: 13u32,
                        resource: wgpu::BindingResource::Sampler(bindings.sampler6),
                    },
                    wgpu::BindGroupEntry {
                        binding: 14u32,
                        resource: wgpu::BindingResource::TextureView(bindings.texture7),
                    },
                    wgpu::BindGroupEntry {
                        binding: 15u32,
                        resource: wgpu::BindingResource::Sampler(bindings.sampler7),
                    },
                    wgpu::BindGroupEntry {
                        binding: 16u32,
                        resource: wgpu::BindingResource::TextureView(bindings.texture8),
                    },
                    wgpu::BindGroupEntry {
                        binding: 17u32,
                        resource: wgpu::BindingResource::Sampler(bindings.sampler8),
                    },
                    wgpu::BindGroupEntry {
                        binding: 18u32,
                        resource: wgpu::BindingResource::TextureView(bindings.texture9),
                    },
                    wgpu::BindGroupEntry {
                        binding: 19u32,
                        resource: wgpu::BindingResource::Sampler(bindings.sampler9),
                    },
                    wgpu::BindGroupEntry {
                        binding: 20u32,
                        resource: wgpu::BindingResource::TextureView(bindings.texture10),
                    },
                    wgpu::BindGroupEntry {
                        binding: 21u32,
                        resource: wgpu::BindingResource::Sampler(bindings.sampler10),
                    },
                    wgpu::BindGroupEntry {
                        binding: 22u32,
                        resource: wgpu::BindingResource::TextureView(bindings.texture11),
                    },
                    wgpu::BindGroupEntry {
                        binding: 23u32,
                        resource: wgpu::BindingResource::Sampler(bindings.sampler11),
                    },
                    wgpu::BindGroupEntry {
                        binding: 24u32,
                        resource: wgpu::BindingResource::TextureView(bindings.texture12),
                    },
                    wgpu::BindGroupEntry {
                        binding: 25u32,
                        resource: wgpu::BindingResource::Sampler(bindings.sampler12),
                    },
                    wgpu::BindGroupEntry {
                        binding: 26u32,
                        resource: wgpu::BindingResource::TextureView(bindings.texture13),
                    },
                    wgpu::BindGroupEntry {
                        binding: 27u32,
                        resource: wgpu::BindingResource::Sampler(bindings.sampler13),
                    },
                    wgpu::BindGroupEntry {
                        binding: 28u32,
                        resource: wgpu::BindingResource::TextureView(bindings.texture14),
                    },
                    wgpu::BindGroupEntry {
                        binding: 29u32,
                        resource: wgpu::BindingResource::Sampler(bindings.sampler14),
                    },
                ],
                label: None,
            });
            Self(bind_group)
        }
    
        pub fn set<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
            render_pass.set_bind_group(1u32, &self.0, &[]);
        }
    }
    pub struct BindGroup2(wgpu::BindGroup);
    pub struct BindGroupLayout2<'a> {
        pub uniforms: &'a wgpu::Buffer,
    }
    const LAYOUT_DESCRIPTOR2: wgpu::BindGroupLayoutDescriptor = wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0u32,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ]
    };
    impl BindGroup2 {
        pub fn get_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
            device.create_bind_group_layout(&LAYOUT_DESCRIPTOR2)
        }
    
        pub fn from_bindings(device: &wgpu::Device, bindings: BindGroupLayout2) -> Self {
            let bind_group_layout = device.create_bind_group_layout(&LAYOUT_DESCRIPTOR2);
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0u32,
                        resource: bindings.uniforms.as_entire_binding(),
                    },
                ],
                label: None,
            });
            Self(bind_group)
        }
    
        pub fn set<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
            render_pass.set_bind_group(2u32, &self.0, &[]);
        }
    }
    pub struct BindGroups<'a> {
        pub bind_group0: &'a BindGroup0,
        pub bind_group1: &'a BindGroup1,
        pub bind_group2: &'a BindGroup2,
    }
    pub fn set_bind_groups<'a>(
        pass: &mut wgpu::RenderPass<'a>,
        bind_groups: BindGroups<'a>,
    ) {
        pass.set_bind_group(0u32, &bind_groups.bind_group0.0, &[]);
        pass.set_bind_group(1u32, &bind_groups.bind_group1.0, &[]);
        pass.set_bind_group(2u32, &bind_groups.bind_group2.0, &[]);
    }
}
pub mod vertex {
    pub const POSITION0_LOCATION: u32 = 0u32;
    pub const NORMAL0_LOCATION: u32 = 1u32;
    pub const TANGENT0_LOCATION: u32 = 2u32;
    pub const MAP1_UVSET_LOCATION: u32 = 3u32;
    pub const UV_SET1_UV_SET2_LOCATION: u32 = 4u32;
    pub const BAKE1_LOCATION: u32 = 5u32;
    pub const COLOR_SET1345_PACKED_LOCATION: u32 = 6u32;
    pub const COLOR_SET2_PACKED_LOCATION: u32 = 7u32;
    pub const COLOR_SET67_PACKED_LOCATION: u32 = 8u32;
    impl super::VertexInput0 {
        pub const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4];
        /// The total size in bytes of all fields without considering padding or alignment.
        pub const SIZE_IN_BYTES: u64 = 48;
    }
    impl super::VertexInput1 {
        pub const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Uint32x4, 7 => Uint32x4, 8 => Uint32x4];
        /// The total size in bytes of all fields without considering padding or alignment.
        pub const SIZE_IN_BYTES: u64 = 96;
    }
}
pub fn create_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(&wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!("model.wgsl")))
    })
}
pub fn create_pipeline_layout(device: &wgpu::Device) -> wgpu::PipelineLayout {
    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[
            &bind_groups::BindGroup0::get_bind_group_layout(device),
            &bind_groups::BindGroup1::get_bind_group_layout(device),
            &bind_groups::BindGroup2::get_bind_group_layout(device),
        ],
        push_constant_ranges: &[],
    })
}
