use futures::executor::block_on;
use ssbh_wgpu::{
    load_render_models, ModelFolder, RenderModel, ShaderDatabase, SharedRenderData, SsbhRenderer,
    REQUIRED_FEATURES, RGBA_COLOR_FORMAT,
};
use wgpu::{
    Backends, Device, DeviceDescriptor, Extent3d, Instance, Limits, PowerPreference, Queue,
    RequestAdapterOptions, TextureDescriptor, TextureDimension, TextureUsages, TextureView,
};

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

    renderer.render_ssbh_passes(&mut encoder, &output_view, &render_models, &shader_database);

    queue.submit([encoder.finish()]);
}

fn main() {
    // Check for any errors.
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Warn)
        .with_module_level("ssbh_wgpu", log::LevelFilter::Info)
        .init()
        .unwrap();

    // Load models in headless mode without a surface.
    // This simplifies testing for stability and performance.
    let instance = Instance::new(Backends::all());
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

    // TODO: Find a way to simplify initialization.
    let surface_format = RGBA_COLOR_FORMAT;
    let shared_data = SharedRenderData::new(&device, &queue, surface_format);
    let renderer = SsbhRenderer::new(&device, &queue, 512, 512, 1.0, [0.0; 3], &[]);

    let texture_desc = TextureDescriptor {
        size: Extent3d {
            width: 512,
            height: 512,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: surface_format,
        usage: TextureUsages::COPY_SRC | TextureUsages::RENDER_ATTACHMENT,
        label: None,
    };
    let output = device.create_texture(&texture_desc);
    let output_view = output.create_view(&Default::default());

    let args: Vec<_> = std::env::args().collect();

    // Load and render folders individually to save on memory.
    let model_paths = globwalk::GlobWalkerBuilder::from_patterns(&args[1], &["*.{numshb}"])
        .build()
        .unwrap()
        .into_iter()
        .filter_map(Result::ok);

    for model in model_paths.into_iter().filter_map(|p| {
        let parent = p.path().parent()?;
        Some(ModelFolder::load_folder(parent))
    }) {
        let render_models = load_render_models(&device, &queue, &[model], &shared_data);

        render(
            &device,
            &queue,
            &renderer,
            &output_view,
            &render_models,
            &shared_data.database,
        );

        // TODO: Save the output texture.
    }
}
