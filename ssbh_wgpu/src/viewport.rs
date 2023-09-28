use glam::Vec4Swizzles;

// TODO: Document what the input and output value ranges should be.
// TODO: Add tests.
pub fn world_to_screen(point: glam::Vec3, mvp: glam::Mat4, width: u32, height: u32) -> (f32, f32) {
    let position = (mvp) * glam::vec4(point.x, point.y, point.z, 1.0);
    // Account for perspective correction.
    let position_clip = position.xyz() / position.w;
    // Convert from clip space [-1,1] to screen space [0,width] or [0,height].
    // Flip y vertically to match wgpu conventions.
    let position_x_screen = width as f32 * (position_clip.x * 0.5 + 0.5);
    let position_y_screen = height as f32 * (1.0 - (position_clip.y * 0.5 + 0.5));
    (position_x_screen, position_y_screen)
}
