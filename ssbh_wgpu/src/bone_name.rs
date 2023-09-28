use crate::{RenderModel, RGBA_COLOR_FORMAT};
use glyph_brush::{ab_glyph::FontRef, DefaultSectionHasher};
use wgpu_text::{BrushBuilder, TextBrush};

// TODO: use glyphon?
// TODO: each bone uses a glyphon::Buffer for its name text?
// TODO: Don't require a static lifetime?
pub struct BoneNameRenderer {
    // TODO: Find a way to simplify this?
    brush: Option<TextBrush<FontRef<'static>, DefaultSectionHasher>>,
}

impl BoneNameRenderer {
    /// Initializes the renderer for the given dimensions and font data.
    ///
    /// The `font_bytes` should be the file contents of a `.ttf` font file.
    /// If `font_bytes` is empty or is not a valid font, text rendering will be disabled.
    pub fn new(device: &wgpu::Device, font_bytes: &'static [u8], width: u32, height: u32) -> Self {
        // TODO: Log errors?
        let brush = BrushBuilder::using_font_bytes(font_bytes)
            .ok()
            .map(|b| b.build(device, width, height, RGBA_COLOR_FORMAT));
        Self { brush }
    }

    /// Renders the bone names for skeleton in `skels` for each model in `render_models` to `output_view`.
    ///
    /// The `output_view` should have the format [RGBA_COLOR_FORMAT].
    /// The output is not cleared before drawing.
    pub fn render_skeleton_names<'a>(
        &'a mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass<'a>,
        models: &'a [RenderModel],
        width: u32,
        height: u32,
        mvp: glam::Mat4,
        font_size: f32,
    ) {
        if let Some(brush) = self.brush.as_mut() {
            // TODO: Optimize this?
            for model in models {
                model.queue_bone_names(device, queue, brush, width, height, mvp, font_size);
            }

            brush.draw(render_pass);
        }
    }

    /// A faster alternative to creating a new [BoneNameRenderer] with the desired size.
    pub fn resize(&mut self, queue: &wgpu::Queue, width: u32, height: u32) {
        if let Some(brush) = self.brush.as_mut() {
            brush.resize_view(width as f32, height as f32, queue);
        }
    }
}
