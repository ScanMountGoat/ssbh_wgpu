use std::{iter, path::Path};

use futures::executor::block_on;
use nutexb_wgpu::{
    create_render_pipeline, create_texture_bind_group, draw_textured_triangle, NutexbFile,
    NutexbImage,
};
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    size: winit::dpi::PhysicalSize<u32>,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    texture_bind_group: wgpu::BindGroup,
    // Test conversions to RGBA.
    rgba_pipeline: wgpu::RenderPipeline,
    rgba_texture_bind_group: wgpu::BindGroup,
    rgba_texture_view: wgpu::TextureView,
}

impl State {
    async fn new<P: AsRef<Path>>(window: &Window, path: P) -> Self {
        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::TEXTURE_COMPRESSION_BC,
                    limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .unwrap();

        let size = window.inner_size();
        let surface_format = surface.get_preferred_format(&adapter).unwrap();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width as u32,
            height: size.height as u32,
            present_mode: wgpu::PresentMode::Fifo,
        };
        surface.configure(&device, &config);

        let start = std::time::Instant::now();
        let nutexb = NutexbImage::from(&NutexbFile::read_from_file(path).unwrap());
        println!("Load Nutexb: {:?}", start.elapsed());

        // TODO: How to make this into a function that returns the rgba texture?
        // TODO: Create a texture renderer similar to SsbhRenderer to cache pipelines?
        let texture = nutexb.create_texture(&device, &queue);
        let (texture_bind_group_layout, texture_bind_group) =
            create_texture_bind_group(&texture, &device);

        // Create a separate texture to test the rgba conversion.
        // The conversion to RGBA is well supported by GPUs.
        // TODO: This may be more efficient using compute shaders.
        // Some applications like egui require RGBA textures.
        let rgba_format = wgpu::TextureFormat::Rgba8UnormSrgb;
        // Disable mipmaps or arrsys since this will be a color attachment.
        let rgba_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: nutexb.width,
                height: nutexb.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: if nutexb.depth > 1 {
                wgpu::TextureDimension::D3
            } else {
                wgpu::TextureDimension::D2
            },
            format: rgba_format,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
        });

        let render_pipeline =
            create_render_pipeline(&device, texture_bind_group_layout, surface_format);

        let rgba_texture_view = rgba_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let (rgba_texture_bind_group_layout, rgba_texture_bind_group) =
            create_texture_bind_group(&rgba_texture, &device);
        let rgba_pipeline =
            create_render_pipeline(&device, rgba_texture_bind_group_layout, rgba_format);

        Self {
            surface,
            device,
            queue,
            size,
            pipeline: render_pipeline,
            texture_bind_group,
            rgba_texture_bind_group,
            rgba_texture_view,
            rgba_pipeline,
            config,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let output_view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // Draw the texture to a second RGBA texture.
        // This is inefficient but tests the RGBA conversion.
        draw_textured_triangle(
            &mut encoder,
            &self.rgba_texture_view,
            &self.rgba_pipeline,
            &self.texture_bind_group,
        );

        // Now draw the RGBA texture to the screen.
        draw_textured_triangle(
            &mut encoder,
            &output_view,
            &self.pipeline,
            &self.rgba_texture_bind_group,
        );

        self.queue.submit(iter::once(encoder.finish()));

        // Actually draw the frame.
        output.present();

        Ok(())
    }
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let image_path = std::path::Path::new(&args[1]);

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(image_path.file_name().unwrap().to_string_lossy())
        .build(&event_loop)
        .unwrap();

    let mut state = block_on(State::new(&window, &image_path));
    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == window.id() => match event {
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state: ElementState::Pressed,
                        virtual_keycode: Some(VirtualKeyCode::Escape),
                        ..
                    },
                ..
            } => *control_flow = ControlFlow::Exit,
            WindowEvent::Resized(physical_size) => {
                state.resize(*physical_size);
            }
            WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                state.resize(**new_inner_size);
            }
            _ => {}
        },
        Event::RedrawRequested(_) => match state.render() {
            Ok(_) => {}
            Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
            Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
            Err(e) => eprintln!("{:?}", e),
        },
        Event::MainEventsCleared => {
            window.request_redraw();
        }
        _ => {}
    });
}
