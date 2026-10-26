use crate::RenderModel;

/// Frame the region defined by `min_xyz` and `max_xyz`
/// assuming a camera pointing toward the positive z-axis.
pub fn frame_bounds_translation(
    min_xyz: glam::Vec3,
    max_xyz: glam::Vec3,
    fov_y: f32,
    aspect: f32,
) -> glam::Vec3 {
    let center = (min_xyz + max_xyz) / 2.0;
    let bounds_size = max_xyz - min_xyz;

    // Find the base of the triangle based on FOV and model width or height.
    // Take the max to frame both horizontally and vertically.
    // TODO: Take near clip into account.
    let fov_x = 2.0 * (aspect * (fov_y * 0.5).tan()).atan();
    let distance_y = (bounds_size.y * 0.5) / (fov_y * 0.5).tan();
    let distance_x = (bounds_size.x * 0.5) / (fov_x * 0.5).tan();
    let distance = distance_x.max(distance_y);

    // This distance should be from the closest object depth.
    glam::vec3(-center.x, -center.y, min_xyz.z - distance)
}

/// Calculate the rotation and translation for a camera to frame `models`.
pub fn frame_models_translation_rotation(
    models: &[RenderModel],
    fov_y: f32,
    aspect: f32,
) -> (glam::Vec3, glam::Vec3) {
    let rotation = glam::Vec3::ZERO;

    let min_xyz = models
        .iter()
        .map(|m| m.min_xyz)
        .reduce(glam::Vec3::min)
        .unwrap_or_default();
    let max_xyz = models
        .iter()
        .map(|m| m.max_xyz)
        .reduce(glam::Vec3::max)
        .unwrap_or_default();
    let translation = frame_bounds_translation(min_xyz, max_xyz, fov_y, aspect);

    (translation, rotation)
}
