use std::collections::HashMap;

#[cfg(not(target_arch = "wasm32"))]
use crate::services::layout_asset_store::global_layout_asset_store;
use serde_json::Value as JsonValue;
use shared::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SpatialTransform {
    pub(crate) translation: Vec3,
    pub(crate) roll_degrees: f32,
    pub(crate) pitch_degrees: f32,
    pub(crate) yaw_degrees: f32,
}

impl Default for SpatialTransform {
    fn default() -> Self {
        Self {
            translation: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            roll_degrees: 0.0,
            pitch_degrees: 0.0,
            yaw_degrees: 0.0,
        }
    }
}

impl SpatialTransform {
    pub(crate) fn from_parameters(parameters: &HashMap<String, JsonValue>) -> Self {
        Self {
            translation: Vec3 {
                x: float_parameter(parameters, "translation_x", 0.0),
                y: float_parameter(parameters, "translation_y", 0.0),
                z: float_parameter(parameters, "translation_z", 0.0),
            },
            roll_degrees: float_parameter(parameters, "rotation_roll", 0.0),
            pitch_degrees: float_parameter(parameters, "rotation_pitch", 0.0),
            yaw_degrees: float_parameter(parameters, "rotation_yaw", 0.0),
        }
    }

    pub(crate) fn transform_point(&self, point: Vec3) -> Vec3 {
        let rotated = rotate_xyz(
            point,
            self.roll_degrees.to_radians(),
            self.pitch_degrees.to_radians(),
            self.yaw_degrees.to_radians(),
        );

        Vec3 {
            x: rotated.x + self.translation.x,
            y: rotated.y + self.translation.y,
            z: rotated.z + self.translation.z,
        }
    }

    pub(crate) fn inverse_transform_point(&self, point: Vec3) -> Vec3 {
        let translated = Vec3 {
            x: point.x - self.translation.x,
            y: point.y - self.translation.y,
            z: point.z - self.translation.z,
        };
        inverse_rotate_xyz(
            translated,
            self.roll_degrees.to_radians(),
            self.pitch_degrees.to_radians(),
            self.yaw_degrees.to_radians(),
        )
    }
}

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
    pub(crate) fn from_parameters_with_prefix(
        parameters: &HashMap<String, JsonValue>,
        prefix: &str,
    ) -> Self {
        Self {
            origin: Vec3 {
                x: float_parameter(parameters, &format!("{prefix}layout_origin_x"), 0.0),
                y: float_parameter(parameters, &format!("{prefix}layout_origin_y"), 0.0),
                z: float_parameter(parameters, &format!("{prefix}layout_origin_z"), 0.0),
            },
            roll_degrees: float_parameter(
                parameters,
                &format!("{prefix}layout_rotation_roll"),
                0.0,
            ),
            pitch_degrees: float_parameter(
                parameters,
                &format!("{prefix}layout_rotation_pitch"),
                0.0,
            ),
            yaw_degrees: float_parameter(parameters, &format!("{prefix}layout_rotation_yaw"), 0.0),
            spacing: float_parameter(parameters, &format!("{prefix}layout_spacing"), 1.0)
                .max(0.0001),
        }
    }

    pub(crate) fn transform_point(&self, local: Vec3) -> Vec3 {
        let scaled = Vec3 {
            x: local.x * self.spacing,
            y: local.y * self.spacing,
            z: local.z * self.spacing,
        };
        self.transform_unscaled_point(scaled)
    }

    pub(crate) fn transform_unscaled_point(&self, local: Vec3) -> Vec3 {
        let rotated = rotate_xyz(
            local,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MatrixStripMode {
    Auto { width: usize, height: usize },
    Strip,
    Matrix { width: usize, height: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpatialLayoutPattern {
    MatrixStrip,
    CircleArc,
    Rectangle,
    Imported,
}

pub(crate) fn spatial_points_for_mode(
    parameters: &HashMap<String, JsonValue>,
    prefix: &str,
    pixel_count: usize,
    mode: MatrixStripMode,
) -> Vec<Vec3> {
    let placement = SpatialPlacement::from_parameters_with_prefix(parameters, prefix);
    let matrix_strip_points = matrix_strip_points(pixel_count, mode, placement);
    match layout_pattern(parameters, prefix) {
        SpatialLayoutPattern::MatrixStrip => matrix_strip_points,
        SpatialLayoutPattern::CircleArc => arc_points(
            pixel_count,
            float_parameter(parameters, &format!("{prefix}layout_circle_radius"), 1.0).max(0.0),
            float_parameter(
                parameters,
                &format!("{prefix}layout_circle_start_degrees"),
                0.0,
            ),
            float_parameter(
                parameters,
                &format!("{prefix}layout_circle_sweep_degrees"),
                360.0,
            ),
            placement,
        ),
        SpatialLayoutPattern::Rectangle => rectangle_perimeter_points(
            pixel_count,
            float_parameter(parameters, &format!("{prefix}layout_rectangle_width"), 8.0).max(0.0),
            float_parameter(parameters, &format!("{prefix}layout_rectangle_height"), 8.0).max(0.0),
            placement,
        ),
        SpatialLayoutPattern::Imported => imported_points(
            imported_layout_points(parameters, prefix)
                .unwrap_or_else(|| matrix_strip_points.clone()),
            pixel_count,
            placement,
        ),
    }
}

pub(crate) fn spatial_layout_pixel_count(
    parameters: &HashMap<String, JsonValue>,
    prefix: &str,
    width: usize,
    height: usize,
    use_spatial: bool,
) -> usize {
    if !use_spatial {
        return width.max(1) * height.max(1);
    }

    match layout_pattern(parameters, prefix) {
        SpatialLayoutPattern::MatrixStrip => width.max(1) * height.max(1),
        SpatialLayoutPattern::CircleArc | SpatialLayoutPattern::Rectangle => width.max(1),
        SpatialLayoutPattern::Imported => imported_layout_points(parameters, prefix)
            .map(|points| points.len().max(1))
            .unwrap_or_else(|| width.max(1) * height.max(1)),
    }
}

pub(crate) fn spatial_layout_dimensions(
    parameters: &HashMap<String, JsonValue>,
    prefix: &str,
    width: usize,
    height: usize,
    use_spatial: bool,
) -> (Option<usize>, Option<usize>) {
    if !use_spatial {
        return (Some(width.max(1)), Some(height.max(1)));
    }

    match layout_pattern(parameters, prefix) {
        SpatialLayoutPattern::MatrixStrip => (Some(width.max(1)), Some(height.max(1))),
        SpatialLayoutPattern::CircleArc
        | SpatialLayoutPattern::Rectangle
        | SpatialLayoutPattern::Imported => (None, None),
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

fn matrix_strip_points(
    pixel_count: usize,
    mode: MatrixStripMode,
    placement: SpatialPlacement,
) -> Vec<Vec3> {
    match mode {
        MatrixStripMode::Strip => strip_points(pixel_count, placement),
        MatrixStripMode::Matrix { width, height } => {
            matrix_points_for_count(width, height, pixel_count, placement)
        }
        MatrixStripMode::Auto { width, height } if width > 1 && height > 1 => {
            matrix_points_for_count(width, height, pixel_count, placement)
        }
        MatrixStripMode::Auto { .. } => strip_points(pixel_count, placement),
    }
}

fn matrix_points_for_count(
    width: usize,
    height: usize,
    pixel_count: usize,
    placement: SpatialPlacement,
) -> Vec<Vec3> {
    let mut points = matrix_points(width.max(1), height.max(1), placement);
    points.truncate(pixel_count);
    if points.len() < pixel_count {
        points.extend(resample_points(&points, pixel_count - points.len()));
        points.truncate(pixel_count);
    }
    points
}

fn arc_points(
    pixel_count: usize,
    radius: f32,
    start_degrees: f32,
    sweep_degrees: f32,
    placement: SpatialPlacement,
) -> Vec<Vec3> {
    if pixel_count == 0 {
        return Vec::new();
    }

    let closed_loop = sweep_degrees.abs() >= 360.0;
    (0..pixel_count)
        .map(|index| {
            let t = if pixel_count <= 1 {
                0.0
            } else if closed_loop {
                index as f32 / pixel_count as f32
            } else {
                index as f32 / (pixel_count - 1) as f32
            };
            let angle = (start_degrees + sweep_degrees * t).to_radians();
            let (sin, cos) = angle.sin_cos();
            let local = Vec3 {
                x: cos * radius,
                y: sin * radius,
                z: 0.0,
            };
            placement.transform_unscaled_point(local)
        })
        .collect()
}

fn rectangle_perimeter_points(
    pixel_count: usize,
    width: f32,
    height: f32,
    placement: SpatialPlacement,
) -> Vec<Vec3> {
    if pixel_count == 0 {
        return Vec::new();
    }
    let half_width = width * 0.5;
    let half_height = height * 0.5;
    let perimeter = (2.0 * (width + height)).max(0.0001);

    (0..pixel_count)
        .map(|index| {
            let distance = perimeter * index as f32 / pixel_count as f32;
            let local = if distance < width {
                Vec3 {
                    x: -half_width + distance,
                    y: -half_height,
                    z: 0.0,
                }
            } else if distance < width + height {
                Vec3 {
                    x: half_width,
                    y: -half_height + (distance - width),
                    z: 0.0,
                }
            } else if distance < (2.0 * width) + height {
                Vec3 {
                    x: half_width - (distance - width - height),
                    y: half_height,
                    z: 0.0,
                }
            } else {
                Vec3 {
                    x: -half_width,
                    y: half_height - (distance - (2.0 * width) - height),
                    z: 0.0,
                }
            };
            placement.transform_unscaled_point(local)
        })
        .collect()
}

fn imported_points(
    points: Vec<Vec3>,
    pixel_count: usize,
    placement: SpatialPlacement,
) -> Vec<Vec3> {
    adapt_points_to_count(&points, pixel_count)
        .into_iter()
        .map(|point| placement.transform_unscaled_point(point))
        .collect()
}

fn adapt_points_to_count(points: &[Vec3], pixel_count: usize) -> Vec<Vec3> {
    if pixel_count == 0 {
        return Vec::new();
    }
    if points.is_empty() {
        return Vec::new();
    }
    if points.len() == pixel_count {
        return points.to_vec();
    }
    if pixel_count == 1 {
        return vec![points[0]];
    }
    if points.len() == 1 {
        return vec![points[0]; pixel_count];
    }

    let source_last = points.len() - 1;
    let destination_last = pixel_count - 1;
    (0..pixel_count)
        .map(|index| {
            let source_index = ((index * source_last) + destination_last / 2) / destination_last;
            points[source_index.min(source_last)]
        })
        .collect()
}

fn resample_points(points: &[Vec3], count: usize) -> Vec<Vec3> {
    if count == 0 {
        return Vec::new();
    }
    if points.is_empty() {
        return vec![
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            };
            count
        ];
    }
    vec![*points.last().expect("points not empty"); count]
}

fn imported_layout_points(
    parameters: &HashMap<String, JsonValue>,
    prefix: &str,
) -> Option<Vec<Vec3>> {
    let asset_id = parameters
        .get(&format!("{prefix}layout_asset_id"))
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim();
    if asset_id.is_empty() {
        return None;
    }
    load_imported_layout_points(asset_id)
}

#[cfg(not(target_arch = "wasm32"))]
fn load_imported_layout_points(asset_id: &str) -> Option<Vec<Vec3>> {
    global_layout_asset_store().and_then(|store| store.load_layout_points(asset_id).ok())
}

#[cfg(target_arch = "wasm32")]
fn load_imported_layout_points(_asset_id: &str) -> Option<Vec<Vec3>> {
    None
}

fn layout_pattern(parameters: &HashMap<String, JsonValue>, prefix: &str) -> SpatialLayoutPattern {
    match parameters
        .get(&format!("{prefix}layout_pattern"))
        .and_then(|value| value.as_str())
        .unwrap_or("matrix_strip")
    {
        "circle_arc" => SpatialLayoutPattern::CircleArc,
        "rectangle" => SpatialLayoutPattern::Rectangle,
        "import" => SpatialLayoutPattern::Imported,
        _ => SpatialLayoutPattern::MatrixStrip,
    }
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

fn inverse_rotate_xyz(point: Vec3, roll: f32, pitch: f32, yaw: f32) -> Vec3 {
    let (yaw_sin, yaw_cos) = (-yaw).sin_cos();
    let after_yaw = Vec3 {
        x: point.x * yaw_cos - point.y * yaw_sin,
        y: point.x * yaw_sin + point.y * yaw_cos,
        z: point.z,
    };

    let (pitch_sin, pitch_cos) = (-pitch).sin_cos();
    let after_pitch = Vec3 {
        x: after_yaw.x * pitch_cos + after_yaw.z * pitch_sin,
        y: after_yaw.y,
        z: -after_yaw.x * pitch_sin + after_yaw.z * pitch_cos,
    };

    let (roll_sin, roll_cos) = (-roll).sin_cos();
    Vec3 {
        x: after_pitch.x,
        y: after_pitch.y * roll_cos - after_pitch.z * roll_sin,
        z: after_pitch.y * roll_sin + after_pitch.z * roll_cos,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use super::{
        MatrixStripMode, SpatialPlacement, SpatialTransform, arc_points, imported_points,
        matrix_points, rectangle_perimeter_points, spatial_points_for_mode, strip_points,
    };
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

    #[test]
    fn arc_points_generate_on_xy_plane() {
        let points = arc_points(4, 1.0, 0.0, 360.0, SpatialPlacement::default());

        assert_vec3(points[0], 1.0, 0.0, 0.0);
        assert!((points[1].z - 0.0).abs() < 1e-5, "{:?}", points[1]);
    }

    #[test]
    fn rectangle_points_walk_the_perimeter_in_order() {
        let points = rectangle_perimeter_points(4, 2.0, 2.0, SpatialPlacement::default());

        assert_vec3(points[0], -1.0, -1.0, 0.0);
        assert_vec3(points[1], 1.0, -1.0, 0.0);
        assert_vec3(points[2], 1.0, 1.0, 0.0);
        assert_vec3(points[3], -1.0, 1.0, 0.0);
    }

    #[test]
    fn imported_points_apply_shared_rotation_and_origin_without_spacing() {
        let points = imported_points(
            vec![Vec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            }],
            1,
            SpatialPlacement {
                origin: Vec3 {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                },
                yaw_degrees: 90.0,
                spacing: 10.0,
                ..SpatialPlacement::default()
            },
        );

        assert_vec3(points[0], 1.0, 3.0, 3.0);
    }

    #[test]
    fn spatial_transform_forward_and_inverse_round_trip() {
        let transform = SpatialTransform {
            translation: Vec3 {
                x: 3.0,
                y: -2.0,
                z: 1.0,
            },
            roll_degrees: 15.0,
            pitch_degrees: 25.0,
            yaw_degrees: -35.0,
        };
        let original = Vec3 {
            x: 1.5,
            y: -0.25,
            z: 2.0,
        };

        let transformed = transform.transform_point(original);
        let restored = transform.inverse_transform_point(transformed);

        assert_vec3(restored, original.x, original.y, original.z);
    }

    #[test]
    fn import_pattern_falls_back_to_matrix_strip_when_asset_is_missing() {
        let parameters = HashMap::from([
            ("layout_pattern".to_owned(), json!("import")),
            ("layout_asset_id".to_owned(), json!("")),
        ]);

        let points = spatial_points_for_mode(
            &parameters,
            "",
            4,
            MatrixStripMode::Auto {
                width: 2,
                height: 2,
            },
        );

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
