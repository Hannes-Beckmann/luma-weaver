use std::collections::HashMap;

use serde_json::Value as JsonValue;
use shared::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SpatialPlacement {
    pub(crate) origin: Vec3,
    pub(crate) roll_degrees: f32,
    pub(crate) pitch_degrees: f32,
    pub(crate) yaw_degrees: f32,
    pub(crate) spacing: f32,
}

impl Default for SpatialPlacement {
    fn default() -> Self {
        Self {
            origin: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            roll_degrees: 0.0,
            pitch_degrees: 0.0,
            yaw_degrees: 0.0,
            spacing: 1.0,
        }
    }
}

impl SpatialPlacement {
    pub(crate) fn from_parameters(parameters: &HashMap<String, JsonValue>) -> Self {
        Self {
            origin: Vec3 {
                x: float_parameter(parameters, "layout_origin_x", 0.0),
                y: float_parameter(parameters, "layout_origin_y", 0.0),
                z: float_parameter(parameters, "layout_origin_z", 0.0),
            },
            roll_degrees: float_parameter(parameters, "layout_rotation_roll", 0.0),
            pitch_degrees: float_parameter(parameters, "layout_rotation_pitch", 0.0),
            yaw_degrees: float_parameter(parameters, "layout_rotation_yaw", 0.0),
            spacing: float_parameter(parameters, "layout_spacing", 1.0).max(0.0001),
        }
    }

    pub(crate) fn transform_point(&self, local: Vec3) -> Vec3 {
        let scaled = Vec3 {
            x: local.x * self.spacing,
            y: local.y * self.spacing,
            z: local.z * self.spacing,
        };
        let rotated = rotate_xyz(
            scaled,
            self.roll_degrees.to_radians(),
            self.pitch_degrees.to_radians(),
            self.yaw_degrees.to_radians(),
        );

        Vec3 {
            x: rotated.x + self.origin.x,
            y: rotated.y + self.origin.y,
            z: rotated.z + self.origin.z,
        }
    }
}

pub(crate) fn strip_points(pixel_count: usize, placement: SpatialPlacement) -> Vec<Vec3> {
    (0..pixel_count)
        .map(|index| {
            placement.transform_point(Vec3 {
                x: index as f32,
                y: 0.0,
                z: 0.0,
            })
        })
        .collect()
}

pub(crate) fn matrix_points(width: usize, height: usize, placement: SpatialPlacement) -> Vec<Vec3> {
    (0..height)
        .flat_map(|y| {
            (0..width).map(move |x| {
                placement.transform_point(Vec3 {
                    x: x as f32,
                    y: y as f32,
                    z: 0.0,
                })
            })
        })
        .collect()
}

fn float_parameter(parameters: &HashMap<String, JsonValue>, name: &str, default: f32) -> f32 {
    parameters
        .get(name)
        .and_then(|value| value.as_f64())
        .filter(|value| value.is_finite())
        .map(|value| value as f32)
        .unwrap_or(default)
}

fn rotate_xyz(point: Vec3, roll: f32, pitch: f32, yaw: f32) -> Vec3 {
    let (roll_sin, roll_cos) = roll.sin_cos();
    let after_roll = Vec3 {
        x: point.x,
        y: point.y * roll_cos - point.z * roll_sin,
        z: point.y * roll_sin + point.z * roll_cos,
    };

    let (pitch_sin, pitch_cos) = pitch.sin_cos();
    let after_pitch = Vec3 {
        x: after_roll.x * pitch_cos + after_roll.z * pitch_sin,
        y: after_roll.y,
        z: -after_roll.x * pitch_sin + after_roll.z * pitch_cos,
    };

    let (yaw_sin, yaw_cos) = yaw.sin_cos();
    Vec3 {
        x: after_pitch.x * yaw_cos - after_pitch.y * yaw_sin,
        y: after_pitch.x * yaw_sin + after_pitch.y * yaw_cos,
        z: after_pitch.z,
    }
}

#[cfg(test)]
mod tests {
    use super::{SpatialPlacement, matrix_points, strip_points};
    use shared::Vec3;

    #[test]
    fn strip_points_apply_origin_spacing_and_yaw() {
        let points = strip_points(
            2,
            SpatialPlacement {
                origin: Vec3 {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                },
                yaw_degrees: 90.0,
                spacing: 2.0,
                ..SpatialPlacement::default()
            },
        );

        assert_vec3(points[0], 1.0, 2.0, 3.0);
        assert_vec3(points[1], 1.0, 4.0, 3.0);
    }

    #[test]
    fn matrix_points_keep_physical_index_order() {
        let points = matrix_points(2, 2, SpatialPlacement::default());

        assert_vec3(points[0], 0.0, 0.0, 0.0);
        assert_vec3(points[1], 1.0, 0.0, 0.0);
        assert_vec3(points[2], 0.0, 1.0, 0.0);
        assert_vec3(points[3], 1.0, 1.0, 0.0);
    }

    fn assert_vec3(actual: Vec3, x: f32, y: f32, z: f32) {
        assert!((actual.x - x).abs() < 1e-5, "{actual:?}");
        assert!((actual.y - y).abs() < 1e-5, "{actual:?}");
        assert!((actual.z - z).abs() < 1e-5, "{actual:?}");
    }
}
