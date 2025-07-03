use pico_args::Arguments;
use ssbh_data::prelude::*;
use ssbh_wgpu::animation::camera::animate_camera;
use ssbh_wgpu::next_frame;
use ssbh_wgpu::swing::SwingPrc;
use ssbh_wgpu::BoneNameRenderer;
use ssbh_wgpu::CameraTransforms;
use ssbh_wgpu::DebugMode;
use ssbh_wgpu::ModelFolder;
use ssbh_wgpu::ModelRenderOptions;
use ssbh_wgpu::NutexbFile;
use ssbh_wgpu::RenderModel;
use ssbh_wgpu::RenderSettings;
use ssbh_wgpu::SharedRenderData;
use ssbh_wgpu::TransitionMaterial;
use ssbh_wgpu::REQUIRED_FEATURES;
use ssbh_wgpu::{load_model_folders, load_render_models, SsbhRenderer};
use std::collections::HashSet;
use std::path::PathBuf;
use winit::keyboard::KeyCode;
use winit::keyboard::NamedKey;
use winit::{
    dpi::PhysicalPosition,
    event::*,
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
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

struct State<'a> {
    surface: wgpu::Surface<'a>,
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
}

impl<'a> State<'a> {
    async fn new(
        window: &'a Window,
        folder: PathBuf,
        anim: Option<PathBuf>,
        prc: Option<PathBuf>,
        camera_anim: Option<PathBuf>,
        render_folder: Option<PathBuf>,
        font_path: Option<PathBuf>,
    ) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let surface = instance.create_surface(window).unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: REQUIRED_FEATURES,
                ..Default::default()
            })
            .await
            .unwrap();

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
        let animation = anim.map(|anim_path| AnimData::from_file(anim_path).unwrap());
        let swing_prc = prc.and_then(|prc_path| SwingPrc::from_file(prc_path));
        let camera_animation =
            camera_anim.map(|camera_anim_path| AnimData::from_file(camera_anim_path).unwrap());

        // Try different possible paths.
        let light_animation = render_folder
            .as_ref()
            .and_then(|f| AnimData::from_file(f.join("light").join("light00.nuanmb")).ok())
            .or_else(|| {
                render_folder
                    .as_ref()
                    .and_then(|f| AnimData::from_file(f.join("light").join("light_00.nuanmb")).ok())
            });

        let mut shared_data = SharedRenderData::new(&device, &queue);

        // Update the cube map first since it's used in model loading for texture assignments.
        if let Some(nutexb) = render_folder
            .as_ref()
            .and_then(|f| NutexbFile::read_from_file(f.join("reflection_cubemap.nutexb")).ok())
        {
            shared_data.update_stage_cube_map(&device, &queue, &nutexb);
        }

        let models = load_model_folders(folder);
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

        if let Some(nutexb) = render_folder.as_ref().and_then(|f| {
            NutexbFile::read_from_file(
                f.parent()
                    .unwrap()
                    .join("lut")
                    .join("color_grading_lut.nutexb"),
            )
            .ok()
        }) {
            renderer.update_color_lut(&device, &queue, &nutexb);
        }

        let font_bytes = font_path.map(|font_path| std::fs::read(font_path).unwrap());

        let name_renderer = BoneNameRenderer::new(&device, &queue, font_bytes, surface_format);

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
        }
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
                        NamedKey::Space => {
                            if event.state == ElementState::Released {
                                self.is_playing = !self.is_playing;
                            }
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

    fn render(&mut self, scale_factor: f64) -> Result<(), wgpu::SurfaceError> {
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
                draw_bones: false,
                draw_bone_axes: false,
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

        // TODO: make name rendering optional.
        // TODO: Avoid recalculating this?
        let (_, _, _, mvp) = calculate_camera(self.size, self.translation_xyz, self.rotation_xyz);

        // TODO: This doesn't work properly with camera animations.
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

        drop(final_pass);

        self.queue.submit([encoder.finish()]);

        // Actually draw the frame.
        output.present();

        Ok(())
    }
}

fn main() {
    // Ignore most wgpu logs to avoid flooding the console.
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Warn)
        .with_module_level("ssbh_wgpu", log::LevelFilter::Info)
        .init()
        .unwrap();

    let mut args = Arguments::from_env();
    // TODO: Support loading multiple folders.
    let folder: PathBuf = args.free_from_str().unwrap();
    let anim_path: Option<PathBuf> = args.opt_value_from_str("--anim").unwrap();
    let prc_path: Option<PathBuf> = args.opt_value_from_str("--swing").unwrap();
    let camera_anim_path: Option<PathBuf> = args.opt_value_from_str("--camera-anim").unwrap();
    let render_folder_path: Option<PathBuf> = args.opt_value_from_str("--render-folder").unwrap();
    let font_path: Option<PathBuf> = args.opt_value_from_str("--font").unwrap();

    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new()
        .with_title("ssbh_wgpu")
        .build(&event_loop)
        .unwrap();

    let mut state = futures::executor::block_on(State::new(
        &window,
        folder,
        anim_path,
        prc_path,
        camera_anim_path,
        render_folder_path,
        font_path,
    ));

    // Initialize the camera buffer.
    state.update_camera(window.scale_factor() as f32);

    event_loop
        .run(|event, target| match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => match event {
                WindowEvent::CloseRequested => target.exit(),
                WindowEvent::Resized(physical_size) => {
                    state.resize(*physical_size, window.scale_factor() as f32);
                    window.request_redraw();
                }
                WindowEvent::ScaleFactorChanged { .. } => {}
                WindowEvent::RedrawRequested => {
                    match state.render(window.scale_factor()) {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost) => {
                            state.resize(state.size, window.scale_factor() as f32)
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => target.exit(),
                        Err(e) => eprintln!("{e:?}"),
                    }
                    window.request_redraw();
                }
                _ => {
                    if state.handle_input(event) {
                        // TODO: Avoid overriding the camera values when pausing?
                        state.update_camera(window.scale_factor() as f32);

                        state.update_render_settings();
                    }
                    window.request_redraw();
                }
            },
            _ => (),
        })
        .unwrap();
}
