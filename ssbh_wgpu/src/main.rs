use std::{iter, path::Path};

use ssbh_wgpu::create_default_textures;
use ssbh_wgpu::load_default_cube;
use ssbh_wgpu::CameraTransforms;
use ssbh_wgpu::PipelineData;
use ssbh_wgpu::RenderModel;
use ssbh_wgpu::REQUIRED_FEATURES;
use ssbh_wgpu::{load_model_folders, load_render_models, SsbhRenderer};

use ssbh_data::prelude::*;

use winit::{
    dpi::PhysicalPosition,
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

fn calculate_camera_pos_mvp(
    size: winit::dpi::PhysicalSize<u32>,
    translation: glam::Vec3,
    rotation: glam::Vec3,
) -> (glam::Vec4, glam::Mat4) {
    let aspect = size.width as f32 / size.height as f32;
    let model_view_matrix = glam::Mat4::from_translation(translation)
        * glam::Mat4::from_rotation_x(rotation.x)
        * glam::Mat4::from_rotation_y(rotation.y);
    // Use a large far clip distance to include stage skyboxes.
    let perspective_matrix = glam::Mat4::perspective_rh_gl(0.5, aspect, 1.0, 400000.0);

    let camera_pos = model_view_matrix.inverse().col(3);

    (camera_pos, perspective_matrix * model_view_matrix)
}

struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    render_models: Vec<RenderModel>,
    renderer: SsbhRenderer,
    // TODO: Separate camera/window state struct?
    size: winit::dpi::PhysicalSize<u32>,

    // Camera input stuff.
    previous_cursor_position: PhysicalPosition<f64>,
    is_mouse_left_clicked: bool,
    is_mouse_right_clicked: bool,
    translation_xyz: glam::Vec3,
    rotation_xyz: glam::Vec3,

    // Animations
    animation: Option<AnimData>,
    // TODO: How to handle overflow if left running too long?
    current_frame: f32,
    previous_frame_start: std::time::Instant,

    // TODO: Should this be part of the renderer?
    default_textures: Vec<(String, wgpu::Texture)>,
    stage_cube: (wgpu::TextureView, wgpu::Sampler),
    pipeline_data: PipelineData,
}

impl State {
    async fn new(window: &Window, folder: &Path, anim_path: Option<&Path>) -> Self {
        // The instance is a handle to our GPU
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
                    features: wgpu::Features::default() | REQUIRED_FEATURES,
                    limits: wgpu::Limits::default(),
                },
                None, // Trace path
            )
            .await
            .unwrap();

        let size = window.inner_size();

        let surface_format = ssbh_wgpu::RGBA_COLOR_FORMAT;

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width as u32,
            height: size.height as u32,
            present_mode: wgpu::PresentMode::Mailbox,
        };
        surface.configure(&device, &config);

        // TODO: Frame bounding spheres?

        // TODO: Where to store/load anim files?
        let animation = anim_path.map(|anim_path| AnimData::from_file(anim_path).unwrap());

        // TODO: Combine these into a single global textures struct?
        let default_textures = create_default_textures(&device, &queue);
        let stage_cube = load_default_cube(&device, &queue).unwrap();
        let stage_cube = (stage_cube.0, stage_cube.1);

        let pipeline_data = PipelineData::new(&device, surface_format);

        let models = load_model_folders(folder);
        let render_meshes = load_render_models(
            &device,
            &queue,
            &pipeline_data,
            &models,
            &default_textures,
            &stage_cube,
        );

        let renderer =
            SsbhRenderer::new(&device, &queue, size.width, size.height, wgpu::Color::BLACK);

        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_models: render_meshes,
            renderer,
            previous_cursor_position: PhysicalPosition { x: 0.0, y: 0.0 },
            is_mouse_left_clicked: false,
            is_mouse_right_clicked: false,
            translation_xyz: glam::Vec3::new(0.0, -8.0, -60.0),
            rotation_xyz: glam::Vec3::new(0.0, 0.0, 0.0),
            animation,
            current_frame: 0.0,
            previous_frame_start: std::time::Instant::now(),
            default_textures,
            stage_cube,
            pipeline_data,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);

            self.update_camera();

            // We also need to recreate the attachments if the size changes.
            self.renderer
                .resize(&self.device, new_size.width, new_size.height);
        }
    }

    fn handle_input(&mut self, event: &WindowEvent) -> bool {
        // Return true if this function handled the event.
        // TODO: Input handling can be it's own module with proper tests.
        // Just test if the WindowEvent object is handled correctly.
        // Test that some_fn(event, state) returns new state?
        match event {
            WindowEvent::MouseInput { button, state, .. } => {
                // Keep track mouse clicks to only rotate when dragging while clicked.
                match (button, state) {
                    (MouseButton::Left, ElementState::Pressed) => self.is_mouse_left_clicked = true,
                    (MouseButton::Left, ElementState::Released) => {
                        self.is_mouse_left_clicked = false
                    }
                    (MouseButton::Right, ElementState::Pressed) => {
                        self.is_mouse_right_clicked = true
                    }
                    (MouseButton::Right, ElementState::Released) => {
                        self.is_mouse_right_clicked = false
                    }
                    _ => (),
                }
                true
            }
            WindowEvent::CursorMoved { position, .. } => {
                if self.is_mouse_left_clicked {
                    let delta_x = position.x - self.previous_cursor_position.x;
                    let delta_y = position.y - self.previous_cursor_position.y;

                    // Swap XY so that dragging left right rotates left right.
                    self.rotation_xyz.x += (delta_y * 0.01) as f32;
                    self.rotation_xyz.y += (delta_x * 0.01) as f32;
                } else if self.is_mouse_right_clicked {
                    // TODO: Adjust speed based on camera distance and handle 0 distance.
                    let delta_x = position.x - self.previous_cursor_position.x;
                    let delta_y = position.y - self.previous_cursor_position.y;

                    // Negate y so that dragging up "drags" the model up.
                    self.translation_xyz.x += (delta_x * 0.1) as f32;
                    self.translation_xyz.y -= (delta_y * 0.1) as f32;
                }
                // Always update the position to avoid jumps when moving between clicks.
                self.previous_cursor_position = *position;

                true
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // TODO: Add tests for handling scroll events properly?
                self.translation_xyz.z += match delta {
                    MouseScrollDelta::LineDelta(_x, y) => *y * 5.0,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                true
            }
            WindowEvent::KeyboardInput { input, .. } => {
                if let Some(keycode) = input.virtual_keycode {
                    match keycode {
                        VirtualKeyCode::Up => self.translation_xyz.z += 10.0,
                        VirtualKeyCode::Down => self.translation_xyz.z -= 10.0,
                        _ => (),
                    }
                }

                true
            }
            _ => false,
        }
    }

    fn update(&mut self) {
        self.update_camera();
    }

    fn update_camera(&mut self) {
        let (camera_pos, mvp_matrix) =
            calculate_camera_pos_mvp(self.size, self.translation_xyz, self.rotation_xyz);
        let transforms = CameraTransforms {
            mvp_matrix,
            camera_pos: camera_pos.to_array(),
        };
        self.renderer.update_camera(&self.queue, transforms);
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        self.update_current_frame();

        // Bind groups are preconfigured outside the render loop for performance.
        // This means only the output view needs to be set for each pass.
        let output = self.surface.get_current_texture()?;
        let output_view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // Apply animations for each model.
        // This is more efficient since state is shared between render meshes.
        for model in &mut self.render_models {
            model.apply_anim(
                &self.device,
                &self.queue,
                self.animation.as_ref(),
                self.current_frame,
                &self.pipeline_data,
                &self.default_textures,
                &self.stage_cube,
            );
        }

        self.renderer
            .render_ssbh_passes(&mut encoder, &output_view, &self.render_models);

        self.queue.submit(iter::once(encoder.finish()));
        // Actually draw the frame.
        output.present();

        Ok(())
    }

    fn update_current_frame(&mut self) {
        // Animate at 60 fps regardless of the rendering framerate.
        // This relies on interpolation or frame skipping.
        // TODO: How robust is this timing implementation?
        // TODO: Create a module/tests for this?
        let current_frame_start = std::time::Instant::now();
        let delta_t = current_frame_start.duration_since(self.previous_frame_start);
        self.previous_frame_start = current_frame_start;

        let millis_per_frame = 1000.0f64 / 60.0f64;
        let delta_t_frames = delta_t.as_millis() as f64 / millis_per_frame;
        let playback_speed = 1.0;

        self.current_frame += (delta_t_frames * playback_speed) as f32;

        if let Some(animation) = &self.animation {
            if self.current_frame > animation.final_frame_index {
                self.current_frame = 0.0;
            }
        }
    }
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let folder = Path::new(&args[1]);
    let anim_path = args.get(2).map(Path::new);

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("ssbh_wgpu")
        .build(&event_loop)
        .unwrap();

    // Since main can't be async, we're going to need to block
    let mut state = futures::executor::block_on(State::new(&window, folder, anim_path));

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => {
                match event {
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
                        // new_inner_size is &mut so we have to dereference it twice
                        state.resize(**new_inner_size);
                    }
                    _ => {}
                }

                if state.handle_input(event) {
                    state.update_camera();
                }
            }
            Event::RedrawRequested(_) => {
                // let start = std::time::Instant::now();
                state.update();
                match state.render() {
                    Ok(_) => {}
                    // Recreate the swap_chain if lost
                    Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    // All other errors (Outdated, Timeout) should be resolved by the next frame
                    Err(e) => eprintln!("{:?}", e),
                }
                // eprintln!("{:?}", start.elapsed());
            }
            Event::MainEventsCleared => {
                // RedrawRequested will only trigger once, unless we manually
                // request it.
                window.request_redraw();
            }
            _ => {}
        }
    });
}
