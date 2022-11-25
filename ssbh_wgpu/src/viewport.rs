use glam::Vec4Swizzles;

// TODO: Document what the input and output value ranges should be.
// TODO: Add tests.
pub fn world_to_screen(point: glam::Vec3, mvp: glam::Mat4, width: u32, height: u32) -> (f32, f32) {
    let position = (mvp) * glam::Vec4::new(point.x, point.y, point.z, 1.0);
    // Account for perspective correction.
    let position_clip = position.xyz() / position.w;
    // Convert from clip space [-1,1] to screen space [0,width] or [0,height].
    // Flip y vertically to match wgpu conventions.
    let position_x_screen = width as f32 * (position_clip.x * 0.5 + 0.5);
    let position_y_screen = height as f32 * (1.0 - (position_clip.y * 0.5 + 0.5));
    (position_x_screen, position_y_screen)
}

pub fn screen_to_world(point: (f32, f32), mvp: glam::Mat4, width: u32, height: u32) -> (f32, f32) {
    // The translation input is in pixels.
    let (x_pixels, y_pixels) = point;
    // We want a world translation to move the scene origin that many pixels.
    // Map from screen space to clip space in the range [-1,1].
    let x_clip = 2.0 * x_pixels / width as f32 - 1.0;
    let y_clip = 2.0 * y_pixels / height as f32 - 1.0;
    // Map to world space using the model, view, and projection matrix.

    let world = mvp.inverse() * glam::Vec4::new(x_clip, y_clip, 0.0, 1.0);
    let world_x = world.x * world.z;
    let world_y = world.y * world.z;
    (world_x, world_y)
}
