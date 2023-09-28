use std::sync::Arc;

use crate::{viewport::world_to_screen, RenderModel, RGBA_COLOR_FORMAT};
use glam::Vec4Swizzles;
use glyphon::{
    Attrs, Buffer, Color, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea,
    TextAtlas, TextBounds, TextRenderer,
};

pub struct BoneNameRenderer {
    font_system: FontSystem,
    cache: SwashCache,
    atlas: TextAtlas,
    renderer: TextRenderer,
}

struct BoneText {
    buffer: Buffer,
    left: f32,
    top: f32,
}

impl BoneNameRenderer {
    /// Initializes the renderer from the given `font_bytes` or tries to use system fonts if `None`.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, font_bytes: Option<Vec<u8>>) -> Self {
        let font_system = font_bytes
            .map(|font_bytes| {
                FontSystem::new_with_fonts(std::iter::once(glyphon::fontdb::Source::Binary(
                    Arc::new(font_bytes),
                )))
            })
            .unwrap_or_else(|| FontSystem::new());

        let cache = SwashCache::new();
        let mut atlas = TextAtlas::new(device, queue, RGBA_COLOR_FORMAT);
        let renderer =
            TextRenderer::new(&mut atlas, device, wgpu::MultisampleState::default(), None);

        Self {
            font_system,
            cache,
            atlas,
            renderer,
        }
    }

    /// Renders the bone names for skeleton in `skels` for each model in `render_models` to `output_view`.
    ///
    /// The `render_pass` should have the format [RGBA_COLOR_FORMAT].
    /// The pass is not cleared before drawing.
    pub fn render_bone_names<'a>(
        &'a mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass<'a>,
        models: &[RenderModel],
        width: u32,
        height: u32,
        mvp: glam::Mat4,
        font_size: f32,
    ) {
        // TODO: create buffers ahead of time to avoid per frame allocations?
        let mut bone_texts = Vec::new();
        for model in models {
            for (name, transform) in model.bone_names_animated_world_transforms() {
                let bone_text =
                    self.create_bone_text(name, transform, mvp, width, height, font_size);
                bone_texts.push(bone_text);
            }
        }

        let text_areas = bone_texts.iter().map(|b| TextArea {
            buffer: &b.buffer,
            left: b.left,
            top: b.top,
            scale: 1.0,
            bounds: TextBounds {
                left: 0,
                top: 0,
                right: width as i32,
                bottom: height as i32,
            },
            default_color: Color::rgb(255, 255, 255),
        });

        // TODO: Is it worth only calling prepare when something changes?
        self.renderer
            .prepare(
                device,
                queue,
                &mut self.font_system,
                &mut self.atlas,
                Resolution { width, height },
                text_areas,
                &mut self.cache,
            )
            .unwrap();

        self.renderer.render(&self.atlas, render_pass).unwrap();
    }

    // TODO: Should these be cached and stored?
    fn create_bone_text(
        &mut self,
        text: &str,
        transform: glam::Mat4,
        mvp: glam::Mat4,
        width: u32,
        height: u32,
        font_size: f32,
    ) -> BoneText {
        let mut buffer = Buffer::new(
            &mut self.font_system,
            Metrics {
                font_size,
                line_height: font_size,
            },
        );
        // TODO: Account for window scale factor?
        buffer.set_size(&mut self.font_system, width as f32, height as f32);
        buffer.set_text(&mut self.font_system, text, Attrs::new(), Shaping::Advanced);
        buffer.shape_until_scroll(&mut self.font_system);

        let position = transform * glam::vec4(0.0, 0.0, 0.0, 1.0);
        let (left, top) = world_to_screen(position.xyz(), mvp, width, height);

        BoneText { buffer, left, top }
    }
}
