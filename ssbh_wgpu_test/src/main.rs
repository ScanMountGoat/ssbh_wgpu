use std::path::{Path, PathBuf};

use futures::executor::block_on;
use image::ImageBuffer;
use rayon::prelude::*;
use ssbh_data::prelude::*;
use ssbh_wgpu::{
    load_render_models, CameraTransforms, ModelFolder, ModelRenderOptions, SharedRenderData,
    SsbhRenderer, REQUIRED_FEATURES,
};
use wgpu::{
    DeviceDescriptor, Extent3d, PowerPreference, RequestAdapterOptions, TextureDescriptor,
    TextureDimension, TextureUsages,
};

// TODO: Just return camera transforms?
fn calculate_camera(
    translation: glam::Vec3,
    rotation: glam::Vec3,
) -> (glam::Vec4, glam::Mat4, glam::Mat4, glam::Mat4) {
    let aspect = 1.0;
    let model_view_matrix = glam::Mat4::from_translation(translation)
        * glam::Mat4::from_rotation_x(rotation.x)
        * glam::Mat4::from_rotation_y(rotation.y);
    // Use a large far clip distance to include stage skyboxes.
    let projection_matrix = glam::Mat4::perspective_rh(0.5, aspect, 1.0, 400000.0);

    let camera_pos = model_view_matrix.inverse().col(3);

    (
        camera_pos,
        model_view_matrix,
        projection_matrix,
        projection_matrix * model_view_matrix,
    )
}

fn main() {
    // TODO: use clap for this.
    let args: Vec<_> = std::env::args().collect();
    let source_folder = &args[1];
    let fighter_anim = args.get(2).map(|s| s.as_str()) == Some("--fighter-anim");

    // Check for any errors.
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Warn)
        .with_module_level("ssbh_wgpu", log::LevelFilter::Warn)
        .init()
        .unwrap();

    // Load models in headless mode without a surface.
    // This simplifies testing for stability and performance.
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });
    let adapter = block_on(instance.request_adapter(&RequestAdapterOptions {
        power_preference: PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .unwrap();
    let (device, queue) = block_on(adapter.request_device(&DeviceDescriptor {
        required_features: REQUIRED_FEATURES,
        ..Default::default()
    }))
    .unwrap();

    // TODO: Find a way to simplify initialization.
    let surface_format = wgpu::TextureFormat::Bgra8UnormSrgb;
    let shared_data = SharedRenderData::new(&device, &queue);
    let mut renderer = SsbhRenderer::new(&device, &queue, 512, 512, 1.0, [0.0; 4], surface_format);

    // TODO: Share camera code with ssbh_wgpu?
    // TODO: Document the screen_dimensions struct.
    // TODO: Frame each model individually?

    let rotation = if fighter_anim {
        // Match the in game orientation.
        glam::vec3(0.0, 50.0f32.to_radians(), 0.0)
    } else {
        glam::Vec3::ZERO
    };

    let (camera_pos, model_view_matrix, projection_matrix, mvp_matrix) =
        calculate_camera(glam::vec3(0.0, -8.0, -60.0), rotation);
    let transforms = CameraTransforms {
        model_view_matrix,
        projection_matrix,
        mvp_matrix,
        mvp_inv_matrix: mvp_matrix.inverse(),
        camera_pos,
        screen_dimensions: glam::vec4(512.0, 512.0, 1.0, 0.0),
    };
    renderer.update_camera(&queue, transforms);

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
        view_formats: &[],
    };
    let output = device.create_texture(&texture_desc);
    let output_view = output.create_view(&Default::default());

    // Load and render folders individually to save on memory.
    let source_folder = Path::new(source_folder);

    // Render each model folder.
    let start = std::time::Instant::now();
    globwalk::GlobWalkerBuilder::from_patterns(source_folder, &["*.{numshb}"])
        .build()
        .unwrap()
        .par_bridge()
        .filter_map(Result::ok)
        .filter_map(|p| {
            let parent = p.path().parent()?;
            if fighter_anim && !parent.components().any(|c| c.as_os_str() == "body") {
                // Only folders like /fighter/mario/body/c00 will have a wait animation.
                None
            } else {
                Some(parent.to_owned())
            }
        })
        .for_each(|folder_path| {
            // Create a unique buffer to avoid mapping a buffer from multiple threads.
            let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                size: 512 * 512 * 4,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                label: None,
                mapped_at_creation: false,
            });

            // Convert fighter/mario/model/body/c00 to mario_model_body_c00.
            let output_path = folder_path
                .strip_prefix(source_folder)
                .unwrap()
                .components()
                .into_iter()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("_");
            let output_path = source_folder.join(output_path).with_extension("png");

            let model = ModelFolder::load_folder(&folder_path);

            let models = [model];
            let mut render_models = load_render_models(&device, &queue, &models, &shared_data);

            if fighter_anim {
                // Try and load an idle animation if possible.
                // TODO: Make this an optional argument.
                let anim_folder =
                    PathBuf::from(folder_path.to_string_lossy().replace("model", "motion"));
                if let Ok(anim) = AnimData::from_file(anim_folder.join("a00wait2.nuanmb"))
                    .or_else(|_| AnimData::from_file(anim_folder.join("a00wait3.nuanmb")))
                {
                    for render_model in &mut render_models {
                        render_model.apply_anims(
                            &queue,
                            std::iter::once(&anim),
                            models[0].find_skel(),
                            models[0].find_matl(),
                            models[0].find_hlpb(),
                            &shared_data,
                            0.0,
                        );
                    }
                }
            }

            render_screenshot(
                &device,
                &renderer,
                &output_view,
                &render_models,
                &shared_data,
                &output,
                &output_buffer,
                texture_desc.size,
                &queue,
                output_path,
            );
        });

    println!("Completed in {:?}", start.elapsed());
}

fn render_screenshot(
    device: &wgpu::Device,
    renderer: &SsbhRenderer,
    output_view: &wgpu::TextureView,
    render_models: &[ssbh_wgpu::RenderModel],
    shared_data: &SharedRenderData,
    output: &wgpu::Texture,
    output_buffer: &wgpu::Buffer,
    size: wgpu::Extent3d,
    queue: &wgpu::Queue,
    output_path: std::path::PathBuf,
) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Render Encoder"),
    });
    renderer.render_models(
        &mut encoder,
        output_view,
        render_models,
        shared_data.database(),
        &ModelRenderOptions::default(),
    );
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            aspect: wgpu::TextureAspect::All,
            texture: output,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: output_buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(512 * 4),
                rows_per_image: Some(512),
            },
        },
        size,
    );
    queue.submit([encoder.finish()]);
    // TODO: Move this functionality to ssbh_wgpu for taking screenshots?
    // Save the output texture.
    // Adapted from WGPU Example https://github.com/gfx-rs/wgpu/tree/master/wgpu/examples/capture
    {
        // TODO: Find ways to optimize this?
        let buffer_slice = output_buffer.slice(..);

        // TODO: Reuse the channel?
        let (tx, rx) = futures_intrusive::channel::shared::oneshot_channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });
        device.poll(wgpu::PollType::Wait).unwrap();
        block_on(rx.receive()).unwrap().unwrap();

        let data = buffer_slice.get_mapped_range();
        let mut buffer =
            ImageBuffer::<image::Rgba<u8>, _>::from_raw(512, 512, data.to_owned()).unwrap();
        // Convert BGRA to RGBA.
        buffer.pixels_mut().for_each(|p| p.0.swap(0, 2));

        buffer.save(output_path).unwrap();
    }
    output_buffer.unmap();
}
