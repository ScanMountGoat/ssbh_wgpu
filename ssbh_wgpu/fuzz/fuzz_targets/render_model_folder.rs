#![no_main]
use futures::executor::block_on;
use libfuzzer_sys::fuzz_target;
use once_cell::sync::Lazy;
use ssbh_wgpu::{
    load_render_models, ModelFolder, ModelRenderOptions, RenderModel, ShaderDatabase,
    SharedRenderData, SsbhRenderer, REQUIRED_FEATURES, RGBA_COLOR_FORMAT,
};
use wgpu::{
    Device, DeviceDescriptor, Extent3d, Limits, PowerPreference, Queue, RequestAdapterOptions,
    TextureDescriptor, TextureDimension, TextureUsages, TextureView,
};

static SHARED: Lazy<(Device, Queue, SharedRenderData, SsbhRenderer, TextureView)> =
    Lazy::new(|| {
        // Load models in headless mode without a surface.
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let adapter = block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .unwrap();

        let (device, queue) = block_on(adapter.request_device(
            &DeviceDescriptor {
                label: None,
                features: REQUIRED_FEATURES,
                limits: Limits::default(),
            },
            None,
        ))
        .unwrap();

        let shared_data = SharedRenderData::new(&device, &queue);

        let renderer = SsbhRenderer::new(&device, &queue, 8, 8, 1.0, [0.0; 3], &[]);

        let texture_desc = TextureDescriptor {
            size: Extent3d {
                width: 8,
                height: 8,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: RGBA_COLOR_FORMAT,
            usage: TextureUsages::COPY_SRC | TextureUsages::RENDER_ATTACHMENT,
            label: None,
            view_formats: &[],
        };
        let output = device.create_texture(&texture_desc);
        let output_view = output.create_view(&Default::default());

        (device, queue, shared_data, renderer, output_view)
    });

fn render(
    device: &Device,
    queue: &Queue,
    renderer: &SsbhRenderer,
    output_view: &TextureView,
    render_models: &[RenderModel],
    shader_database: &ShaderDatabase,
) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Render Encoder"),
    });

    renderer.render_models(
        &mut encoder,
        &output_view,
        &render_models,
        &shader_database,
        &ModelRenderOptions::default(),
    );

    queue.submit([encoder.finish()]);
}

fuzz_target!(|model: ModelFolder| {
    let device = &SHARED.0;
    let queue = &SHARED.1;
    let shared_data = &SHARED.2;
    let renderer = &SHARED.3;
    let output_view = &SHARED.4;

    // Check for errors when loading and rendering models.
    // This helps check for validation errors and WGPU panics.
    let render_models = load_render_models(&device, &queue, &[model], &shared_data);

    // TODO: Apply animations as well?
    render(
        &device,
        &queue,
        &renderer,
        &output_view,
        &render_models,
        &shared_data.database(),
    );
});
