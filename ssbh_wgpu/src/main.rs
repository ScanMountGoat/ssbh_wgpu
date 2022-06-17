use std::path::Path;

use ssbh_wgpu::create_default_textures;
use ssbh_wgpu::load_default_cube;
use ssbh_wgpu::CameraTransforms;
use ssbh_wgpu::ModelFolder;
use ssbh_wgpu::PipelineData;
use ssbh_wgpu::RenderModel;
use ssbh_wgpu::RenderSettings;
use ssbh_wgpu::TransitionMaterial;
use ssbh_wgpu::REQUIRED_FEATURES;
use ssbh_wgpu::{create_database, DebugMode, ShaderDatabase};
use ssbh_wgpu::{load_model_folders, load_render_models, SsbhRenderer};

use wgpu_text::BrushBuilder;

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
) -> (glam::Vec4, glam::Mat4, glam::Mat4) {
    let aspect = size.width as f32 / size.height as f32;
    let model_view_matrix = glam::Mat4::from_translation(translation)
        * glam::Mat4::from_rotation_x(rotation.x)
        * glam::Mat4::from_rotation_y(rotation.y);
    // Use a large far clip distance to include stage skyboxes.
    let perspective_matrix = glam::Mat4::perspective_rh(0.5, aspect, 1.0, 400000.0);

    let camera_pos = model_view_matrix.inverse().col(3);

    (
        camera_pos,
        model_view_matrix,
        perspective_matrix * model_view_matrix,
    )
}

struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,

    // Parallel lists for models and renderable models.
    models: Vec<ModelFolder>,
    render_models: Vec<RenderModel>,

    renderer: SsbhRenderer,
    shader_database: ShaderDatabase,

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

    is_playing: bool,

    render: RenderSettings,
}

impl State {
    async fn new(window: &Window, folder: &Path, anim_path: Option<&Path>) -> Self {
        // The instance is a handle to our GPU
        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(&window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
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
            present_mode: wgpu::PresentMode::Fifo,
        };
        surface.configure(&device, &config);

        // TODO: Frame bounding spheres?

        // TODO: Where to store/load anim files?
        let animation = anim_path.map(|anim_path| AnimData::from_file(anim_path).unwrap());

        // TODO: Combine these into a single global textures struct?
        let default_textures = create_default_textures(&device, &queue);
        let stage_cube = load_default_cube(&device, &queue);

        let shader_database = create_database();

        let pipeline_data = PipelineData::new(&device, surface_format);

        let models = load_model_folders(folder);
        let render_models = load_render_models(
            &device,
            &queue,
            &pipeline_data,
            &models,
            &default_textures,
            &stage_cube,
            &shader_database,
        );

        let renderer = SsbhRenderer::new(
            &device,
            &queue,
            size.width,
            size.height,
            window.scale_factor(),
            wgpu::Color::BLACK,
        );

        Self {
            surface,
            device,
            queue,
            config,
            size,
            models,
            render_models,
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
            is_playing: false,
            shader_database,
            render: RenderSettings::default(),
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>, scale_factor: f64) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);

            self.update_camera();

            // We also need to recreate the attachments if the size changes.
            self.renderer.resize(
                &self.device,
                &self.queue,
                new_size.width,
                new_size.height,
                scale_factor,
            );
        }
    }

    fn handle_input(&mut self, event: &WindowEvent) -> bool {
        // Return true if this function handled the event.
        // TODO: Input handling can be it's own module with proper tests.
        // Just test if the WindowEvent object is handled correctly.
        // Test that some_fn(event, state) returns new state?
        match event {
            WindowEvent::MouseInput { button, state, .. } => {
                // Track mouse clicks to only rotate when dragging while clicked.
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
                    let (current_x_world, current_y_world) = self.screen_to_world(*position);
                    let (previous_x_world, previous_y_world) =
                        self.screen_to_world(self.previous_cursor_position);

                    let delta_x_world = current_x_world - previous_x_world;
                    let delta_y_world = current_y_world - previous_y_world;

                    // Negate y so that dragging up "drags" the model up.
                    self.translation_xyz.x += delta_x_world;
                    self.translation_xyz.y -= delta_y_world;
                }
                // Always update the position to avoid jumps when moving between clicks.
                self.previous_cursor_position = *position;

                true
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // TODO: Add tests for handling scroll events properly?
                // Scale zoom speed with distance to make it easier to zoom out large scenes.
                let delta_z = match delta {
                    MouseScrollDelta::LineDelta(_x, y) => *y * self.translation_xyz.z.abs() * 0.1,
                    MouseScrollDelta::PixelDelta(p) => {
                        p.y as f32 * self.translation_xyz.z.abs() * 0.005
                    }
                };

                // Clamp to prevent the user from zooming through the origin.
                self.translation_xyz.z = (self.translation_xyz.z + delta_z).min(-1.0);
                true
            }
            WindowEvent::KeyboardInput { input, .. } => {
                if let Some(keycode) = input.virtual_keycode {
                    // Don't handle the release event to avoid duplicate events.
                    if matches!(input.state, winit::event::ElementState::Pressed) {
                        match keycode {
                            VirtualKeyCode::Up => self.translation_xyz.z += 10.0,
                            VirtualKeyCode::Down => self.translation_xyz.z -= 10.0,
                            VirtualKeyCode::Space => self.is_playing = !self.is_playing,
                            VirtualKeyCode::Key1 => self.render.debug_mode = DebugMode::Shaded,
                            VirtualKeyCode::Key2 => self.render.debug_mode = DebugMode::ColorSet1,
                            VirtualKeyCode::Key3 => self.render.debug_mode = DebugMode::ColorSet2,
                            VirtualKeyCode::Key4 => self.render.debug_mode = DebugMode::ColorSet3,
                            VirtualKeyCode::Key5 => self.render.debug_mode = DebugMode::ColorSet4,
                            VirtualKeyCode::Key6 => self.render.debug_mode = DebugMode::ColorSet5,
                            VirtualKeyCode::Key7 => self.render.debug_mode = DebugMode::ColorSet6,
                            VirtualKeyCode::Key8 => self.render.debug_mode = DebugMode::ColorSet7,
                            VirtualKeyCode::Q => self.render.debug_mode = DebugMode::Texture0,
                            VirtualKeyCode::W => self.render.debug_mode = DebugMode::Texture1,
                            VirtualKeyCode::E => self.render.debug_mode = DebugMode::Texture2,
                            VirtualKeyCode::R => self.render.debug_mode = DebugMode::Texture3,
                            VirtualKeyCode::T => self.render.debug_mode = DebugMode::Texture4,
                            VirtualKeyCode::Y => self.render.debug_mode = DebugMode::Texture5,
                            VirtualKeyCode::U => self.render.debug_mode = DebugMode::Texture6,
                            VirtualKeyCode::I => self.render.debug_mode = DebugMode::Texture7,
                            VirtualKeyCode::O => self.render.debug_mode = DebugMode::Texture8,
                            VirtualKeyCode::P => self.render.debug_mode = DebugMode::Texture9,
                            VirtualKeyCode::A => self.render.debug_mode = DebugMode::Texture10,
                            VirtualKeyCode::S => self.render.debug_mode = DebugMode::Texture11,
                            VirtualKeyCode::D => self.render.debug_mode = DebugMode::Texture12,
                            VirtualKeyCode::F => self.render.debug_mode = DebugMode::Texture13,
                            VirtualKeyCode::G => self.render.debug_mode = DebugMode::Texture14,
                            VirtualKeyCode::H => self.render.debug_mode = DebugMode::Texture16,
                            VirtualKeyCode::J => self.render.debug_mode = DebugMode::Position0,
                            VirtualKeyCode::K => self.render.debug_mode = DebugMode::Normal0,
                            VirtualKeyCode::L => self.render.debug_mode = DebugMode::Tangent0,
                            VirtualKeyCode::Z => self.render.debug_mode = DebugMode::Map1,
                            VirtualKeyCode::X => self.render.debug_mode = DebugMode::Bake1,
                            VirtualKeyCode::C => self.render.debug_mode = DebugMode::UvSet,
                            VirtualKeyCode::V => self.render.debug_mode = DebugMode::UvSet1,
                            VirtualKeyCode::B => self.render.debug_mode = DebugMode::UvSet2,
                            // TODO: Add more steps?
                            VirtualKeyCode::Numpad0 => self.render.transition_factor = 0.0,
                            VirtualKeyCode::Numpad1 => self.render.transition_factor = 1.0 / 3.0,
                            VirtualKeyCode::Numpad2 => self.render.transition_factor = 2.0 / 3.0,
                            VirtualKeyCode::Numpad3 => self.render.transition_factor = 1.0,
                            VirtualKeyCode::Numpad4 => {
                                self.render.transition_material = TransitionMaterial::Ink
                            }
                            VirtualKeyCode::Numpad5 => {
                                self.render.transition_material = TransitionMaterial::MetalBox
                            }
                            VirtualKeyCode::Numpad6 => {
                                self.render.transition_material = TransitionMaterial::Gold
                            }
                            VirtualKeyCode::Numpad7 => {
                                self.render.transition_material = TransitionMaterial::Ditto
                            }
                            _ => (),
                        }
                    }
                }

                true
            }
            _ => false,
        }
    }

    // TODO: Module and tests for a viewport camera.
    fn screen_to_world(&mut self, position: PhysicalPosition<f64>) -> (f32, f32) {
        // The translation input is in pixels.
        let x_pixels = position.x;
        let y_pixels = position.y;
        // dbg!(x_pixels, y_pixels);
        // We want a world translation to move the scene origin that many pixels.
        // Map from screen space to clip space in the range [-1,1].
        let x_clip = 2.0 * x_pixels / self.size.width as f64 - 1.0;
        let y_clip = 2.0 * y_pixels / self.size.height as f64 - 1.0;
        // dbg!(x_clip, y_clip);
        // Map to world space using the model, view, and projection matrix.
        // TODO: Avoid recalculating the matrix?
        // Rotation is applied first, so always translate in XY.
        // TODO: Does ignoring rotation like this work in general?
        let (_, _, mvp) =
            calculate_camera_pos_mvp(self.size, self.translation_xyz, self.rotation_xyz * 0.0);
        // TODO: This doesn't work properly when the camera rotates?
        let world = mvp.inverse() * glam::Vec4::new(x_clip as f32, y_clip as f32, 0.0, 1.0);
        // dbg!(world);
        // TODO: What's the correct scale for this step?
        let world_x = world.x * world.z;
        let world_y = world.y * world.z;
        (world_x, world_y)
    }

    fn update(&mut self) {
        self.update_camera();
    }

    fn update_camera(&mut self) {
        let (camera_pos, model_view_matrix, mvp_matrix) =
            calculate_camera_pos_mvp(self.size, self.translation_xyz, self.rotation_xyz);
        let transforms = CameraTransforms {
            model_view_matrix,
            mvp_matrix,
            camera_pos: camera_pos.to_array(),
        };
        self.renderer.update_camera(&self.queue, transforms);
    }

    fn update_render_settings(&mut self) {
        self.renderer
            .update_render_settings(&self.queue, &self.render);
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let current_frame_start = std::time::Instant::now();
        if self.is_playing {
            self.current_frame = next_frame(
                self.current_frame,
                self.previous_frame_start,
                current_frame_start,
                self.animation.as_ref(),
            );
        }
        self.previous_frame_start = current_frame_start;

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
        // This is more efficient than animating per mesh since state is shared between render meshes.
        if self.is_playing {
            // TODO: Combine these into one list?
            for (i, model) in self.render_models.iter_mut().enumerate() {
                model.apply_anim(
                    &self.device,
                    &self.queue,
                    self.animation.as_ref(),
                    self.models[i].find_skel(),
                    self.models[i].find_matl(),
                    self.models[i].find_hlpb(),
                    self.current_frame,
                    &self.pipeline_data,
                    &self.default_textures,
                    &self.stage_cube,
                    &self.shader_database,
                );
            }
        }

        self.renderer.render_ssbh_passes(
            &mut encoder,
            &output_view,
            &self.render_models,
            &self.shader_database,
        );

        let skel = self.models.get(0).and_then(|m| m.find_skel());
        let (_, _, mvp) =
            calculate_camera_pos_mvp(self.size, self.translation_xyz, self.rotation_xyz);

        if let Some(text_commands) = self.renderer.render_skeleton(
            &self.device,
            &self.queue,
            &mut encoder,
            &output_view,
            &self.render_models,
            skel,
            self.size.width,
            self.size.height,
            mvp,
            false,
        ) {
            self.queue.submit([encoder.finish(), text_commands]);
        } else {
            self.queue.submit([encoder.finish()]);
        }

        // Actually draw the frame.
        output.present();

        Ok(())
    }
}

pub fn next_frame(
    current_frame: f32,
    previous: std::time::Instant,
    current: std::time::Instant,
    animation: Option<&AnimData>,
) -> f32 {
    // Animate at 60 fps regardless of the rendering framerate.
    // This relies on interpolation or frame skipping.
    // TODO: How robust is this timing implementation?
    // TODO: Create a module/tests for this?
    let delta_t = current.duration_since(previous);

    let millis_per_frame = 1000.0f64 / 60.0f64;
    let delta_t_frames = delta_t.as_millis() as f64 / millis_per_frame;
    let playback_speed = 1.0;

    let mut next_frame = current_frame + (delta_t_frames * playback_speed) as f32;

    if let Some(animation) = animation {
        if next_frame > animation.final_frame_index {
            next_frame = 0.0;
        }
    }

    next_frame
}

fn main() {
    // Ignore most wgpu logs to avoid flooding the console.
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Warn)
        .with_module_level("ssbh_wgpu", log::LevelFilter::Info)
        .init()
        .unwrap();

    let args: Vec<_> = std::env::args().collect();
    let folder = Path::new(&args[1]);
    let anim_path = args.get(2).map(Path::new);

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("ssbh_wgpu")
        .build(&event_loop)
        .unwrap();

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
                        state.resize(*physical_size, window.scale_factor());
                    }
                    WindowEvent::ScaleFactorChanged {
                        scale_factor,
                        new_inner_size,
                    } => {
                        // new_inner_size is &mut so we have to dereference it twice
                        state.resize(**new_inner_size, *scale_factor);
                    }
                    _ => {}
                }

                if state.handle_input(event) {
                    state.update_camera();
                    state.update_render_settings();
                }
            }
            Event::RedrawRequested(_) => {
                // let start = std::time::Instant::now();
                state.update();
                match state.render() {
                    Ok(_) => {}
                    // Recreate the swap_chain if lost
                    Err(wgpu::SurfaceError::Lost) => {
                        state.resize(state.size, window.scale_factor())
                    }
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
