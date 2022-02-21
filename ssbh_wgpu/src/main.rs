use std::{iter, path::Path};

use ssbh_wgpu::shader::model::bind_groups::CameraTransforms;
use ssbh_wgpu::{
    camera::create_camera_bind_group, load_model_folders, load_models, RenderMesh, SsbhRenderer,
};

use winit::{
    dpi::PhysicalPosition,
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

fn calculate_mvp(
    size: winit::dpi::PhysicalSize<u32>,
    translation: glam::Vec3,
    rotation: glam::Vec3,
) -> (glam::Vec4, glam::Mat4) {
    let aspect = size.width as f32 / size.height as f32;
    let model_view_matrix = glam::Mat4::from_translation(translation)
        * glam::Mat4::from_rotation_x(rotation.x)
        * glam::Mat4::from_rotation_y(rotation.y);
    let perspective_matrix = glam::Mat4::perspective_rh_gl(0.5, aspect, 1.0, 100000.0);

    let camera_pos = model_view_matrix.inverse().col(3);

    (camera_pos, perspective_matrix * model_view_matrix)
}

struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    render_meshes: Vec<RenderMesh>,
    camera_buffer: wgpu::Buffer,
    // TODO: Make this part of the public API?
    // TODO: Organize the passes somehow?
    renderer: SsbhRenderer,
    camera_bind_group: ssbh_wgpu::shader::model::bind_groups::BindGroup0,
    // TODO: Separate camera/window state struct?
    size: winit::dpi::PhysicalSize<u32>,

    // Camera stuff.
    previous_cursor_position: PhysicalPosition<f64>,
    is_mouse_left_clicked: bool,
    is_mouse_right_clicked: bool,
    translation_xyz: glam::Vec3,
    rotation_xyz: glam::Vec3,
}

impl State {
    async fn new(window: &Window, folder: &Path) -> Self {
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
                    features: wgpu::Features::TEXTURE_COMPRESSION_BC,
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

        let camera_position = glam::Vec3::new(0.0, -8.0, -60.0);
        let (model_view, mvp) =
            calculate_mvp(size, camera_position, glam::Vec3::new(0.0, 0.0, 0.0));
        let (camera_buffer, camera_bind_group) =
            create_camera_bind_group(size, model_view, mvp, &device);

        let models = load_models(folder);
        let render_meshes = load_model_folders(&device, &queue, surface_format, &models);

        // TODO: Move this to the lib?
        let renderer = SsbhRenderer::new(&device, &queue, size.width, size.height);

        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_meshes,
            camera_buffer,
            camera_bind_group,
            renderer,
            previous_cursor_position: PhysicalPosition { x: 0.0, y: 0.0 },
            is_mouse_left_clicked: false,
            is_mouse_right_clicked: false,
            translation_xyz: camera_position,
            rotation_xyz: glam::Vec3::new(0.0, 0.0, 0.0),
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);

            let (camera_pos, mvp_matrix) =
                calculate_mvp(new_size, self.translation_xyz, self.rotation_xyz);
            self.queue.write_buffer(
                &self.camera_buffer,
                0,
                bytemuck::cast_slice(&[CameraTransforms {
                    mvp_matrix,
                    camera_pos,
                }]),
            );

            // We also need to recreate the attachments if the size changes.
            self.renderer
                .resize(&self.device, &self.queue, new_size.width, new_size.height);
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
        let (camera_pos, mvp_matrix) =
            calculate_mvp(self.size, self.translation_xyz, self.rotation_xyz);
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[CameraTransforms {
                mvp_matrix,
                camera_pos,
            }]),
        );
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        // Bind groups are preconfigured outside the render loop for performance.
        // This means only the output view needs to be set for each pass.
        let _start = std::time::Instant::now();

        let output = self.surface.get_current_texture()?;
        let output_view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        self.renderer.render_ssbh_passes(
            &mut encoder,
            &output_view,
            &self.render_meshes,
            &self.camera_bind_group,
        );

        self.queue.submit(iter::once(encoder.finish()));
        // Actually draw the frame.
        output.present();

        Ok(())
    }
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let folder = std::path::Path::new(&args[1]);

    // TODO: Load the mesh and skel data.
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("ssbh_wgpu")
        .build(&event_loop)
        .unwrap();

    // Since main can't be async, we're going to need to block
    let mut state = futures::executor::block_on(State::new(&window, folder));

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
                        // new_inner_size is &mut so w have to dereference it twice
                        state.resize(**new_inner_size);
                    }
                    _ => {}
                }

                if state.handle_input(event) {
                    let (camera_pos, mvp_matrix) =
                        calculate_mvp(state.size, state.translation_xyz, state.rotation_xyz);
                    // TODO: Avoid requiring bytemuck in the application itself?
                    state.queue.write_buffer(
                        &state.camera_buffer,
                        0,
                        bytemuck::cast_slice(&[CameraTransforms {
                            mvp_matrix,
                            camera_pos,
                        }]),
                    );
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
