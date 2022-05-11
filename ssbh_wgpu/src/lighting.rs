use glam::{Mat4, Quat, Vec3, Vec4};

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

pub fn light_direction(rotation: Quat) -> Vec4 {
    Mat4::from_quat(rotation) * Vec4::Z
}

impl crate::shader::model::StageUniforms {
    // TODO: Make a function to initialize this from a nuanmb.
    pub fn training() -> Self {
        // TODO: Is it important to split into light and attribute sections?
        let custom_boolean = [[0.0; 4]; 20];

        let mut custom_vector = [[0.0; 4]; 64];
        custom_vector[0] = [1.0; 4];

        let mut custom_float = [[0.0; 4]; 20];
        custom_float[0] = [4.0, 0.0, 0.0, 0.0];

        Self {
            chr_light_dir: light_direction(glam::Quat::from_xyzw(
                -0.453154, -0.365998, -0.211309, 0.784886,
            ))
            .to_array(),
            custom_boolean,
            custom_vector,
            custom_float,
        }
    }
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;

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

    #[test]
    fn light_direction_light_chr_training() {
        let dir = light_direction(Quat::from_xyzw(-0.453154, -0.365998, -0.211309, 0.784886));
        assert_relative_eq!(-0.38302213, dir.x, epsilon = 0.0001f32);
        assert_relative_eq!(0.86602527, dir.y, epsilon = 0.0001f32);
        assert_relative_eq!(0.32139426, dir.z, epsilon = 0.0001f32);
        assert_eq!(0.0, dir.w);
    }
}
