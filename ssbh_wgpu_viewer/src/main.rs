use anyhow::Context;
use clap::Parser;
use futures::executor::block_on;
use ssbh_data::prelude::*;
use ssbh_wgpu::{
    animation::camera::animate_camera, load_model_folders, load_render_models, next_frame,
    swing::SwingPrc, BoneNameRenderer, CameraTransforms, DebugMode, ModelFolder,
    ModelRenderOptions, NutexbFile, RenderModel, RenderSettings, SharedRenderData, SsbhRenderer,
    TransitionMaterial, REQUIRED_FEATURES, REQUIRED_LIMITS,
};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::*,
    event_loop::EventLoop,
    keyboard::{KeyCode, NamedKey},
    window::Window,
};

const FOV_Y: f32 = 0.5;
const NEAR_CLIP: f32 = 1.0;
const FAR_CLIP: f32 = 400000.0;

// TODO: Just return camera transforms?
fn calculate_camera(
    size: winit::dpi::PhysicalSize<u32>,
    translation: glam::Vec3,
    rotation: glam::Vec3,
) -> (glam::Vec4, glam::Mat4, glam::Mat4, glam::Mat4) {
    let aspect = size.width as f32 / size.height as f32;
    let model_view_matrix = glam::Mat4::from_translation(translation)
        * glam::Mat4::from_rotation_x(rotation.x)
        * glam::Mat4::from_rotation_y(rotation.y);
    // Use a large far clip distance to include stage skyboxes.
    let projection_matrix = glam::Mat4::perspective_rh(FOV_Y, aspect, NEAR_CLIP, FAR_CLIP);

    let camera_pos = model_view_matrix.inverse().col(3);

    (
        camera_pos,
        model_view_matrix,
        projection_matrix,
        projection_matrix * model_view_matrix,
    )
}

struct State {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,

    // Parallel lists for models and renderable models.
    models: Vec<(PathBuf, ModelFolder)>,
    render_models: Vec<RenderModel>,

    renderer: SsbhRenderer,
    name_renderer: BoneNameRenderer,

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
    camera_animation: Option<AnimData>,
    light_animation: Option<AnimData>,

    // TODO: How to handle overflow if left running too long?
    current_frame: f32,
    previous_frame_start: std::time::Instant,

    // TODO: Should this be part of the renderer?
    shared_data: SharedRenderData,

    is_playing: bool,

    render: RenderSettings,

    draw_bones: bool,
    draw_bone_names: bool,
    draw_bone_axes: bool,
}

impl State {
    async fn new(
        window: Window,
        cli: &Cli,
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
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: REQUIRED_FEATURES,
                required_limits: REQUIRED_LIMITS,
                ..Default::default()
            })
            .await?;

        let size = window.inner_size();

        let surface_format = wgpu::TextureFormat::Bgra8UnormSrgb;
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

        // TODO: Frame bounding spheres?
        let animation = cli
            .anim
            .as_ref()
            .map(|anim_path| AnimData::from_file(anim_path).unwrap());
        let swing_prc = cli.swing.as_ref().and_then(SwingPrc::from_file);
        let camera_animation = cli
            .camera_anim
            .as_ref()
            .map(|camera_anim_path| AnimData::from_file(camera_anim_path).unwrap());

        // Try different possible paths.
        let light_animation = cli
            .render_folder
            .as_ref()
            .and_then(|f| {
                AnimData::from_file(Path::new(f).join("light").join("light00.nuanmb")).ok()
            })
            .or_else(|| {
                cli.render_folder.as_ref().and_then(|f| {
                    AnimData::from_file(Path::new(f).join("light").join("light_00.nuanmb")).ok()
                })
            });

        let mut shared_data = SharedRenderData::new(&device, &queue);

        // Update the cube map first since it's used in model loading for texture assignments.
        if let Some(nutexb) = cli.render_folder.as_ref().and_then(|f| {
            NutexbFile::read_from_file(Path::new(f).join("reflection_cubemap.nutexb")).ok()
        }) {
            shared_data.update_stage_cube_map(&device, &queue, &nutexb);
        }

        let models = load_model_folders(&cli.folder);
        let mut render_models =
            load_render_models(&device, &queue, models.iter().map(|(_, m)| m), &shared_data);

        // Assume only one folder is loaded and apply the swing prc to every folder.
        if let Some(swing_prc) = &swing_prc {
            for (render_model, (_, model)) in render_models.iter_mut().zip(models.iter()) {
                render_model.recreate_swing_collisions(&device, swing_prc, model.find_skel());
            }
        }

        let mut renderer = SsbhRenderer::new(
            &device,
            &queue,
            size.width,
            size.height,
            window.scale_factor() as f32,
            [0.0, 0.0, 0.0, 1.0],
            surface_format,
        );

        if let Some(nutexb) = cli.render_folder.as_ref().and_then(|f| {
            NutexbFile::read_from_file(
                Path::new(f)
                    .parent()?
                    .join("lut")
                    .join("color_grading_lut.nutexb"),
            )
            .ok()
        }) {
            renderer.update_color_lut(&device, &queue, &nutexb);
        }

        let font_bytes = cli.font.as_ref().map(std::fs::read).transpose()?;

        let name_renderer = BoneNameRenderer::new(&device, &queue, font_bytes, surface_format);

        Ok(Self {
            window,
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
            translation_xyz: glam::vec3(0.0, -8.0, -60.0),
            rotation_xyz: glam::vec3(0.0, 0.0, 0.0),
            animation,
            camera_animation,
            light_animation,
            current_frame: 0.0,
            previous_frame_start: std::time::Instant::now(),
            shared_data,
            is_playing: false,
            render: RenderSettings::default(),
            name_renderer,
            draw_bones: cli.bones,
            draw_bone_names: cli.bone_names,
            draw_bone_axes: cli.bone_axes,
        })
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>, scale_factor: f32) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);

            self.update_camera(scale_factor);

            // We also need to recreate the attachments if the size changes.
            self.renderer
                .resize(&self.device, new_size.width, new_size.height, scale_factor);
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
                    let delta_x = position.x - self.previous_cursor_position.x;
                    let delta_y = position.y - self.previous_cursor_position.y;

                    // Translate an equivalent distance in screen space based on the camera.
                    // The viewport height and vertical field of view define the conversion.
                    let fac = FOV_Y.sin() * self.translation_xyz.z.abs() / self.size.height as f32;

                    // Negate y so that dragging up "drags" the model up.
                    self.translation_xyz.x += delta_x as f32 * fac;
                    self.translation_xyz.y -= delta_y as f32 * fac;
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
            WindowEvent::KeyboardInput { event, .. } => {
                match &event.physical_key {
                    winit::keyboard::PhysicalKey::Code(code) => match code {
                        // TODO: Add more steps?
                        KeyCode::Numpad0 => self.render.transition_factor = 0.0,
                        KeyCode::Numpad1 => self.render.transition_factor = 1.0 / 3.0,
                        KeyCode::Numpad2 => self.render.transition_factor = 2.0 / 3.0,
                        KeyCode::Numpad3 => self.render.transition_factor = 1.0,
                        KeyCode::Numpad4 => {
                            self.render.transition_material = TransitionMaterial::Ink
                        }
                        KeyCode::Numpad5 => {
                            self.render.transition_material = TransitionMaterial::MetalBox
                        }
                        KeyCode::Numpad6 => {
                            self.render.transition_material = TransitionMaterial::Gold
                        }
                        KeyCode::Numpad7 => {
                            self.render.transition_material = TransitionMaterial::Ditto
                        }
                        _ => (),
                    },
                    winit::keyboard::PhysicalKey::Unidentified(_) => todo!(),
                }
                match &event.logical_key {
                    winit::keyboard::Key::Named(named) => match named {
                        NamedKey::ArrowUp => self.translation_xyz.z += 10.0,
                        NamedKey::ArrowDown => self.translation_xyz.z -= 10.0,
                        NamedKey::Space if event.state == ElementState::Released => {
                            self.is_playing = !self.is_playing;
                        }
                        _ => (),
                    },
                    winit::keyboard::Key::Character(c) => match c.as_str() {
                        "1" => self.render.debug_mode = DebugMode::Shaded,
                        "2" => self.render.debug_mode = DebugMode::ColorSet1,
                        "3" => self.render.debug_mode = DebugMode::ColorSet2,
                        "4" => self.render.debug_mode = DebugMode::ColorSet3,
                        "5" => self.render.debug_mode = DebugMode::ColorSet4,
                        "6" => self.render.debug_mode = DebugMode::ColorSet5,
                        "7" => self.render.debug_mode = DebugMode::ColorSet6,
                        "8" => self.render.debug_mode = DebugMode::ColorSet7,
                        "q" => self.render.debug_mode = DebugMode::Texture0,
                        "w" => self.render.debug_mode = DebugMode::Texture1,
                        "e" => self.render.debug_mode = DebugMode::Texture2,
                        "r" => self.render.debug_mode = DebugMode::Texture3,
                        "t" => self.render.debug_mode = DebugMode::Texture4,
                        "y" => self.render.debug_mode = DebugMode::Texture5,
                        "u" => self.render.debug_mode = DebugMode::Texture6,
                        "i" => self.render.debug_mode = DebugMode::Texture7,
                        "o" => self.render.debug_mode = DebugMode::Texture8,
                        "p" => self.render.debug_mode = DebugMode::Texture9,
                        "a" => self.render.debug_mode = DebugMode::Texture10,
                        "s" => self.render.debug_mode = DebugMode::Texture11,
                        "d" => self.render.debug_mode = DebugMode::Texture12,
                        "f" => self.render.debug_mode = DebugMode::Texture13,
                        "g" => self.render.debug_mode = DebugMode::Texture14,
                        "h" => self.render.debug_mode = DebugMode::Texture16,
                        "j" => self.render.debug_mode = DebugMode::Position0,
                        "k" => self.render.debug_mode = DebugMode::Normal0,
                        "l" => self.render.debug_mode = DebugMode::Tangent0,
                        "z" => self.render.debug_mode = DebugMode::Map1,
                        "x" => self.render.debug_mode = DebugMode::Bake1,
                        "c" => self.render.debug_mode = DebugMode::UvSet,
                        "v" => self.render.debug_mode = DebugMode::UvSet1,
                        "b" => self.render.debug_mode = DebugMode::UvSet2,
                        "n" => self.render.debug_mode = DebugMode::Basic,
                        "m" => self.render.debug_mode = DebugMode::Normals,
                        "," => self.render.debug_mode = DebugMode::Bitangents,
                        "." => self.render.debug_mode = DebugMode::Unlit,
                        "/" => self.render.debug_mode = DebugMode::ShaderComplexity,
                        _ => (),
                    },
                    winit::keyboard::Key::Unidentified(_) => (),
                    winit::keyboard::Key::Dead(_) => (),
                }

                true
            }
            _ => false,
        }
    }

    // TODO: Module and tests for a viewport camera.

    fn update_camera(&mut self, scale_factor: f32) {
        let (camera_pos, model_view_matrix, projection_matrix, mvp_matrix) =
            calculate_camera(self.size, self.translation_xyz, self.rotation_xyz);
        let transforms = CameraTransforms {
            model_view_matrix,
            projection_matrix,
            mvp_matrix,
            mvp_inv_matrix: mvp_matrix.inverse(),
            camera_pos,
            screen_dimensions: glam::vec4(
                self.size.width as f32,
                self.size.height as f32,
                scale_factor,
                0.0,
            ),
        };
        self.renderer.update_camera(&self.queue, transforms);
    }

    fn update_render_settings(&mut self) {
        self.renderer
            .update_render_settings(&self.queue, &self.render);
    }

    fn render(&mut self, output: wgpu::SurfaceTexture, scale_factor: f32) {
        let current_frame_start = std::time::Instant::now();
        if self.is_playing {
            self.current_frame = next_frame(
                self.current_frame,
                current_frame_start.duration_since(self.previous_frame_start),
                self.animation
                    .as_ref()
                    .map(|a| a.final_frame_index)
                    .unwrap_or_default()
                    .max(
                        self.camera_animation
                            .as_ref()
                            .map(|a| a.final_frame_index)
                            .unwrap_or_default(),
                    )
                    .max(
                        self.light_animation
                            .as_ref()
                            .map(|a| a.final_frame_index)
                            .unwrap_or_default(),
                    ),
                1.0,
                true,
            );
        }
        self.previous_frame_start = current_frame_start;

        // Bind groups are preconfigured outside the render loop for performance.
        // This means only the output view needs to be set for each pass.
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
            self.renderer
                .update_current_frame(&self.queue, self.current_frame);

            // TODO: Combine these into one list?
            for (i, model) in self.render_models.iter_mut().enumerate() {
                model.apply_anims(
                    &self.queue,
                    self.animation.iter(),
                    self.models[i].1.find_skel(),
                    self.models[i].1.find_matl(),
                    self.models[i].1.find_hlpb(),
                    &self.shared_data,
                    self.current_frame,
                );
            }

            if let Some(anim) = &self.camera_animation {
                if let Some(values) =
                    animate_camera(anim, self.current_frame, FOV_Y, NEAR_CLIP, FAR_CLIP)
                {
                    let transforms =
                        values.to_transforms(self.size.width, self.size.height, scale_factor);
                    self.renderer.update_camera(&self.queue, transforms);
                }
            }

            if let Some(anim) = &self.light_animation {
                self.renderer
                    .update_stage_uniforms(&self.queue, anim, self.current_frame);
            }
        }

        let mut final_pass = self.renderer.render_models(
            &mut encoder,
            &output_view,
            &self.render_models,
            self.shared_data.database(),
            &ModelRenderOptions {
                draw_bones: self.draw_bones,
                draw_bone_axes: self.draw_bone_axes,
                draw_floor_grid: true,
                draw_wireframe: true,
                ..Default::default()
            },
        );

        for model in &self.render_models {
            // Use an empty set to show all collisions.
            self.renderer
                .render_swing(&mut final_pass, model, &HashSet::new());
        }

        if self.draw_bone_names {
            // TODO: This doesn't work properly with camera animations.
            // TODO: Avoid recalculating this?
            let (_, _, _, mvp) =
                calculate_camera(self.size, self.translation_xyz, self.rotation_xyz);

            self.name_renderer.render_bone_names(
                &self.device,
                &self.queue,
                &mut final_pass,
                &self.render_models,
                self.size.width,
                self.size.height,
                mvp,
                18.0,
            );
        }

        drop(final_pass);

        self.queue.submit([encoder.finish()]);

        // Actually draw the frame.
        output.present();
    }
}

struct App {
    state: Option<State>,
    cli: Cli,
}

impl ApplicationHandler<()> for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window = event_loop
            .create_window(Window::default_attributes().with_title("ssbh_wgpu_viewer"))
            .unwrap();

        self.state = block_on(State::new(window, &self.cli, event_loop)).ok();
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

            let scale_factor = state.window.scale_factor() as f32;
            match event {
                WindowEvent::Resized(physical_size) => {
                    state.resize(physical_size, scale_factor);
                    state.update_camera(scale_factor);
                    state.window.request_redraw();
                }
                WindowEvent::ScaleFactorChanged { .. } => {}
                WindowEvent::RedrawRequested => {
                    match state.surface.get_current_texture() {
                        wgpu::CurrentSurfaceTexture::Success(output) => {
                            state.render(output, scale_factor)
                        }
                        wgpu::CurrentSurfaceTexture::Suboptimal(_) => {
                            state.resize(state.size, scale_factor)
                        }
                        wgpu::CurrentSurfaceTexture::Timeout => {}
                        wgpu::CurrentSurfaceTexture::Occluded => {}
                        wgpu::CurrentSurfaceTexture::Outdated => {
                            state.resize(state.size, scale_factor)
                        }
                        wgpu::CurrentSurfaceTexture::Lost => state.resize(state.size, scale_factor),
                        wgpu::CurrentSurfaceTexture::Validation => {}
                    }
                    state.window.request_redraw();
                }
                _ => {
                    state.handle_input(&event);
                    state.update_camera(state.window.scale_factor() as f32);
                    state.update_render_settings();
                }
            }
        }
    }
}

#[derive(Parser)]
#[command(author, version, about)]
#[command(propagate_version = true)]
struct Cli {
    folder: String,

    #[arg(long)]
    anim: Option<String>,

    #[arg(long)]
    swing: Option<String>,

    #[arg(long)]
    camera_anim: Option<String>,

    #[arg(long)]
    render_folder: Option<String>,

    #[arg(long)]
    font: Option<String>,

    #[arg(long)]
    bones: bool,

    #[arg(long)]
    bone_axes: bool,

    #[arg(long)]
    bone_names: bool,
}

fn main() -> anyhow::Result<()> {
    // Ignore most wgpu logs to avoid flooding the console.
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Warn)
        .with_module_level("ssbh_wgpu", log::LevelFilter::Info)
        .init()
        .unwrap();

    let cli = Cli::parse();

    // TODO: Support loading multiple folders.

    // TODO: move model loading here for better error reporting.

    let event_loop = EventLoop::new()?;
    let mut app = App { state: None, cli };

    event_loop
        .run_app(&mut app)
        .with_context(|| "failed to complete event loop")?;
    Ok(())
}
