use std::{iter, path::Path};

use futures::executor::block_on;
use nutexb_wgpu::{NutexbFile, RenderSettings, TextureRenderer};
use winit::{
    event::*,
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

struct State<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    size: winit::dpi::PhysicalSize<u32>,
    config: wgpu::SurfaceConfiguration,
    renderer: TextureRenderer,
    _layer: u32,
    _mipmap: f32,
}

impl<'a> State<'a> {
    async fn new<P: AsRef<Path>>(window: &'a Window, path: P, layer: u32, mipmap: f32) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let surface = instance.create_surface(window).unwrap();
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
                    required_features: wgpu::Features::TEXTURE_COMPRESSION_BC,
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await
            .unwrap();

        let size = window.inner_size();
        let surface_format = wgpu::TextureFormat::Rgba8Unorm;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: Vec::new(),
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let start = std::time::Instant::now();
        let nutexb = NutexbFile::read_from_file(path).unwrap();
        println!("Load Nutexb: {:?}", start.elapsed());

        // TODO: Use the dim to handle rendering 3d and cube map textures.
        let (texture, dim) = nutexb_wgpu::create_texture(&nutexb, &device, &queue).unwrap();

        let mut renderer = TextureRenderer::new(&device, &queue, surface_format);
        let settings = RenderSettings {
            render_rgba: [true; 4],
            mipmap,
            layer,
        };

        // Use the full texture width and height.
        // Some use cases benefit from custom dimensions like texture thumbnails.
        // This is just for documenting how to use the API.
        // In a real application, the renderer could render the texture directly.
        let start = std::time::Instant::now();
        let rgba_texture = renderer.render_to_texture_2d_rgba(
            &device,
            &queue,
            &texture,
            dim,
            (
                nutexb.footer.width,
                nutexb.footer.height,
                nutexb.footer.depth,
            ),
            nutexb.footer.width,
            nutexb.footer.height,
            &settings,
        );
        println!("Render to RGBA: {:?}", start.elapsed());

        // The RGBA texture is always 2D.
        renderer.update(
            &device,
            &queue,
            &rgba_texture,
            wgpu::TextureViewDimension::D2,
            (nutexb.footer.width, nutexb.footer.height, 1),
            &settings,
        );

        Self {
            surface,
            device,
            queue,
            size,
            renderer,
            config,
            _layer: layer,
            _mipmap: mipmap,
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
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &output_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        self.renderer.render(&mut render_pass);

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

    let layer: u32 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(0);
    let mipmap: f32 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(0.0);

    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new()
        .with_title(image_path.file_name().unwrap().to_string_lossy())
        .build(&event_loop)
        .unwrap();

    // TODO: change the mipmap or layer using keyboard shortcuts.
    let mut state = block_on(State::new(&window, &image_path, layer, mipmap));
    event_loop
        .run(|event, target| match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => match event {
                WindowEvent::CloseRequested => target.exit(),
                WindowEvent::Resized(physical_size) => {
                    state.resize(*physical_size);
                    window.request_redraw();
                }
                WindowEvent::ScaleFactorChanged { .. } => {}
                WindowEvent::RedrawRequested => {
                    match state.render() {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                        Err(wgpu::SurfaceError::OutOfMemory) => target.exit(),
                        Err(e) => eprintln!("{e:?}"),
                    }
                    window.request_redraw();
                }
                _ => {
                    window.request_redraw();
                }
            },
            _ => (),
        })
        .unwrap();
}
