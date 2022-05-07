use glam::{Mat4, Quat, Vec3};

pub fn calculate_light_transform(rotation: Quat, scale: Vec3) -> Mat4 {
    // TODO: This should be editable when changing stages.
    // TODO: Why do we negate w?
    // TODO: Read this value from the transform for LightStg0 from light00_set.nuanmb.
    // TODO: Do translation and scale matter?

    // TODO: What controls the "scale" of the lighting region?
    let perspective_matrix =
        Mat4::orthographic_rh(-scale.x, scale.x, -scale.y, scale.y, -scale.z, scale.z);
    let model_view = Mat4::from_quat(rotation);

    perspective_matrix * model_view
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::assert_matrix_relative_eq;

    #[test]
    fn rotation_zero() {
        assert_matrix_relative_eq!(
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, -0.5, 0.5],
                [0.0, 0.0, 0.0, 1.0],
            ],
            calculate_light_transform(
                Quat::from_xyzw(0.0, 0.0, 0.0, 1.0),
                Vec3::new(1.0, 1.0, 1.0)
            )
            .transpose()
            .to_cols_array_2d()
        )
    }

    #[test]
    fn rotation_x_90_degrees() {
        assert_matrix_relative_eq!(
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, -1.0, 0.0],
                [0.0, -0.5, 0.0, 0.5],
                [0.0, 0.0, 0.0, 1.0],
            ],
            calculate_light_transform(
                Quat::from_xyzw(1.0, 0.0, 0.0, 1.0).normalize(),
                Vec3::new(1.0, 1.0, 1.0)
            )
            .transpose()
            .to_cols_array_2d()
        )
    }

    #[test]
    fn rotation_y_90_degrees() {
        assert_matrix_relative_eq!(
            [
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.5, 0.0, 0.0, 0.5],
                [0.0, 0.0, 0.0, 1.0],
            ],
            calculate_light_transform(
                Quat::from_xyzw(0.0, 1.0, 0.0, 1.0).normalize(),
                Vec3::new(1.0, 1.0, 1.0)
            )
            .transpose()
            .to_cols_array_2d()
        )
    }

    #[test]
    fn rotation_z_90_degrees() {
        assert_matrix_relative_eq!(
            [
                [0.0, -1.0, 0.0, 0.0],
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, -0.5, 0.5],
                [0.0, 0.0, 0.0, 1.0],
            ],
            calculate_light_transform(
                Quat::from_xyzw(0.0, 0.0, 1.0, 1.0).normalize(),
                Vec3::new(1.0, 1.0, 1.0)
            )
            .transpose()
            .to_cols_array_2d()
        )
    }
}
