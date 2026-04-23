use std::{iter, path::PathBuf, sync::Arc};

use anyhow::Context;
use futures::executor::block_on;
use nutexb_wgpu::{NutexbFile, RenderSettings, TextureRenderer};
use winit::{application::ApplicationHandler, event::*, event_loop::EventLoop, window::Window};

struct State {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    size: winit::dpi::PhysicalSize<u32>,
    config: wgpu::SurfaceConfiguration,
    renderer: TextureRenderer,
    _layer: u32,
    _mipmap: f32,
}

impl State {
    async fn new(
        window: Window,
        path: PathBuf,
        layer: u32,
        mipmap: f32,
        event_loop: &winit::event_loop::ActiveEventLoop,
    ) -> anyhow::Result<Self> {
        let window = Arc::new(window);

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_with_display_handle(Box::new(
                event_loop.owned_display_handle(),
            ))
        });
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: wgpu::Features::TEXTURE_COMPRESSION_BC,
                ..Default::default()
            })
            .await?;

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
        let nutexb = NutexbFile::read_from_file(path)?;
        println!("Load Nutexb: {:?}", start.elapsed());

        // TODO: Use the dim to handle rendering 3d and cube map textures.
        let (texture, dim) = nutexb_wgpu::create_texture(&nutexb, &device, &queue)?;

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

        Ok(Self {
            window,
            surface,
            device,
            queue,
            size,
            renderer,
            config,
            _layer: layer,
            _mipmap: mipmap,
        })
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn render(&mut self, output: wgpu::SurfaceTexture) {
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
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        self.renderer.render(&mut render_pass);

        drop(render_pass);

        self.queue.submit(iter::once(encoder.finish()));

        // Actually draw the frame.
        output.present();
    }
}

struct App {
    state: Option<State>,

    // TODO: cli struct with clap
    image_path: PathBuf,
    layer: u32,
    mipmap: f32,
}

impl ApplicationHandler<()> for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window = event_loop
            .create_window(Window::default_attributes().with_title("nutexb_wgpu_viewer"))
            .unwrap();

        self.state = block_on(State::new(
            window,
            self.image_path.clone(),
            self.layer,
            self.mipmap,
            event_loop,
        ))
        .ok();
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        if event == WindowEvent::CloseRequested {
            event_loop.exit();
            return;
        };

        // Window specific event handling.
        if let Some(state) = self.state.as_mut() {
            if window_id != state.window.id() {
                return;
            }

            match event {
                WindowEvent::Resized(physical_size) => {
                    state.resize(physical_size);
                    state.window.request_redraw();
                }
                WindowEvent::ScaleFactorChanged { .. } => {}
                WindowEvent::RedrawRequested => {
                    match state.surface.get_current_texture() {
                        wgpu::CurrentSurfaceTexture::Success(output) => state.render(output),
                        wgpu::CurrentSurfaceTexture::Suboptimal(_) => state.resize(state.size),
                        wgpu::CurrentSurfaceTexture::Timeout => {}
                        wgpu::CurrentSurfaceTexture::Occluded => {}
                        wgpu::CurrentSurfaceTexture::Outdated => state.resize(state.size),
                        wgpu::CurrentSurfaceTexture::Lost => state.resize(state.size),
                        wgpu::CurrentSurfaceTexture::Validation => {}
                    }
                    state.window.request_redraw();
                }
                _ => {}
            }
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let image_path = PathBuf::from(&args[1]);

    let layer: u32 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(0);
    let mipmap: f32 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(0.0);

    // TODO: change the mipmap or layer using keyboard shortcuts.
    let event_loop = EventLoop::new()?;
    let mut app = App {
        state: None,
        image_path,
        layer,
        mipmap,
    };
    event_loop
        .run_app(&mut app)
        .with_context(|| "failed to complete event loop")?;
    Ok(())
}
