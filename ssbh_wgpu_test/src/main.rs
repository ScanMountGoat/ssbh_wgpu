use futures::executor::block_on;
use ssbh_wgpu::{
    create_database, create_default_textures, load_default_cube, load_model_folders,
    load_render_models, PipelineData,
};
use wgpu::TextureFormat;

fn main() {
    // Check for any errors.
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Warn)
        .with_module_level("ssbh_wgpu", log::LevelFilter::Info)
        .init()
        .unwrap();

    // Load models in headless mode without a surface.
    // This simplifies testing for stability and performance.
    let instance = wgpu::Instance::new(wgpu::Backends::all());
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .unwrap();
    let (device, queue) = block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: None,
            features: wgpu::Features::TEXTURE_COMPRESSION_BC,
            limits: wgpu::Limits::default(),
        },
        None,
    ))
    .unwrap();

    // TODO: Find a way to simplify initialization.
    let default_textures = create_default_textures(&device, &queue);
    let stage_cube = load_default_cube(&device, &queue);
    let shader_database = create_database();
    let pipeline_data = PipelineData::new(&device, TextureFormat::Rgba8UnormSrgb);

    // TODO: Avoid loading all folders in one call to save on memory.
    let args: Vec<_> = std::env::args().collect();
    let models = load_model_folders(&args[1]);
    for model in models {
        load_render_models(
            &device,
            &queue,
            &pipeline_data,
            &[model],
            &default_textures,
            &stage_cube,
            &shader_database,
        );
    }
}
