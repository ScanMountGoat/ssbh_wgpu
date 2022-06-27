use std::{iter, path::Path};

use futures::executor::block_on;
use nutexb_wgpu::{NutexbFile, TextureRenderer};
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
    renderer: TextureRenderer,
    rgba_texture_bind_group: wgpu::BindGroup,
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
        let nutexb = NutexbFile::read_from_file(path).unwrap();
        println!("Load Nutexb: {:?}", start.elapsed());

        let texture = nutexb_wgpu::create_texture(&nutexb, &device, &queue);

        let renderer = TextureRenderer::new(&device, surface_format);

        // Use the full texture width and height.
        // Some use cases benefit from custom dimensions like texture thumbnails.
        let start = std::time::Instant::now();
        let rgba_texture = renderer.render_to_texture_rgba(
            &device,
            &queue,
            &texture,
            nutexb.footer.width,
            nutexb.footer.height,
        );
        println!("Render to RGBA: {:?}", start.elapsed());

        let rgba_texture_bind_group = renderer.create_texture_bind_group(&device, &rgba_texture);

        Self {
            surface,
            device,
            queue,
            size,
            renderer,
            config,
            rgba_texture_bind_group,
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

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: &output_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        });

        self.renderer
            .render(&mut render_pass, &self.rgba_texture_bind_group);

        drop(render_pass);

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
