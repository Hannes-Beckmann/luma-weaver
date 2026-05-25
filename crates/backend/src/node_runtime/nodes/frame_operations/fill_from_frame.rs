use std::collections::HashMap;

use anyhow::Result;
use serde_json::Value as JsonValue;
use shared::{
    ColorFrame, InputValue, LedLayout, NodeDiagnostic, NodeDiagnosticSeverity, RgbaColor, Vec3,
};

use crate::node_runtime::{
    NodeConstruction, NodeEvaluationContext, NodeFrontendUpdate, ParameterStatus, RuntimeNode,
    RuntimeNodeFromParameters, RuntimeOutputs, TypedNodeEvaluation, parameter_status,
};

const EPSILON: f32 = 1e-6;

pub(crate) struct FillFromFrameNode {
    method: FillFromFrameMethod,
    sample_count: usize,
    distance_power: f32,
    radius: f32,
    fallback_color: RgbaColor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FillFromFrameMethod {
    Nearest,
    SmoothDistance,
    Radius,
    IndexStretch,
}

impl Default for FillFromFrameNode {
    fn default() -> Self {
        Self {
            method: FillFromFrameMethod::Nearest,
            sample_count: 4,
            distance_power: 2.0,
            radius: 2.0,
            fallback_color: RgbaColor {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
        }
    }
}

impl RuntimeNodeFromParameters for FillFromFrameNode {
    fn from_parameters(parameters: &HashMap<String, JsonValue>) -> NodeConstruction<Self> {
        let defaults = Self::default();
        let sample_count =
            usize_parameter(parameters, "sample_count", defaults.sample_count).clamp(1, 64);
        let distance_power =
            f32_parameter(parameters, "distance_power", defaults.distance_power).clamp(0.1, 8.0);
        let radius = f32_parameter(parameters, "radius", defaults.radius)
            .clamp(0.0, 1000.0)
            .max(0.0);

        NodeConstruction {
            node: Self {
                method: FillFromFrameMethod::from_id(&string_parameter(
                    parameters, "method", "nearest",
                )),
                sample_count,
                distance_power,
                radius,
                fallback_color: color_parameter(
                    parameters,
                    "fallback_color",
                    defaults.fallback_color,
                ),
            },
            diagnostics: Vec::new(),
        }
    }
}

pub(crate) struct FillFromFrameInputs {
    frame: Option<InputValue>,
}

crate::node_runtime::impl_runtime_inputs!(FillFromFrameInputs {
    frame = None,
});

pub(crate) struct FillFromFrameOutputs {
    frame: Option<ColorFrame>,
}

impl RuntimeOutputs for FillFromFrameOutputs {
    fn into_runtime_outputs(self) -> anyhow::Result<HashMap<String, InputValue>> {
        let mut outputs = HashMap::new();
        if let Some(frame) = self.frame {
            outputs.insert("frame".to_owned(), InputValue::ColorFrame(frame));
        }
        Ok(outputs)
    }
}

impl RuntimeNode for FillFromFrameNode {
    type Inputs = FillFromFrameInputs;
    type Outputs = FillFromFrameOutputs;

    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let Some(source_frame) = inputs.frame.and_then(|value| match value {
            InputValue::MappedFrame(frame) => Some(frame),
            _ => None,
        }) else {
            return Ok(TypedNodeEvaluation::from_outputs(FillFromFrameOutputs {
                frame: None,
            }));
        };

        let Some(destination_layout) = context.render_layout.clone() else {
            return Ok(TypedNodeEvaluation {
                outputs: FillFromFrameOutputs { frame: None },
                frontend_updates: Vec::new(),
                diagnostics: vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Warning,
                    code: Some("fill_from_frame_missing_render_layout".to_owned()),
                    message: "Fill From Frame has no render layout, so it cannot emit a frame yet."
                        .to_owned(),
                }],
            });
        };

        let source_layout = source_frame.layout;
        let source_pixels = resample_index(&source_frame.pixels, source_layout.pixel_count);
        let (pixels, diagnostics) =
            self.fill_destination(&source_pixels, &source_layout, &destination_layout);
        let output_frame = ColorFrame {
            layout: destination_layout.clone(),
            pixels,
        };
        let frame_name = format!("frame ({})", destination_layout.id);

        Ok(TypedNodeEvaluation {
            outputs: FillFromFrameOutputs {
                frame: Some(output_frame.clone()),
            },
            frontend_updates: vec![
                NodeFrontendUpdate {
                    name: "source_frame".to_owned(),
                    value: InputValue::MappedFrame(ColorFrame {
                        layout: source_layout,
                        pixels: source_frame.pixels,
                    }),
                },
                NodeFrontendUpdate {
                    name: frame_name,
                    value: InputValue::ColorFrame(output_frame),
                },
            ],
            diagnostics,
        })
    }
}

impl FillFromFrameNode {
    fn fill_destination(
        &self,
        source_pixels: &[RgbaColor],
        source_layout: &LedLayout,
        destination_layout: &LedLayout,
    ) -> (Vec<RgbaColor>, Vec<NodeDiagnostic>) {
        let destination_points = destination_layout
            .points_3d
            .as_ref()
            .filter(|points| points.len() >= destination_layout.pixel_count);
        let source_points = source_layout
            .points_3d
            .as_ref()
            .filter(|points| points.len() >= source_layout.pixel_count);

        if matches!(self.method, FillFromFrameMethod::IndexStretch)
            || destination_points.is_none()
            || source_points.is_none()
        {
            let diagnostics = if !matches!(self.method, FillFromFrameMethod::IndexStretch)
                && destination_points.is_none()
            {
                vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Warning,
                    code: Some("fill_from_frame_non_spatial_destination".to_owned()),
                    message: "Fill From Frame is using index stretch because the destination layout has no spatial coordinates."
                        .to_owned(),
                }]
            } else {
                Vec::new()
            };
            return (
                resample_index(source_pixels, destination_layout.pixel_count),
                diagnostics,
            );
        }

        let source_points = source_points.expect("checked source points");
        let destination_points = destination_points.expect("checked destination points");
        let pixels = match self.method {
            FillFromFrameMethod::Nearest => destination_points
                .iter()
                .take(destination_layout.pixel_count)
                .map(|point| nearest_color(*point, source_points, source_pixels))
                .collect(),
            FillFromFrameMethod::SmoothDistance => destination_points
                .iter()
                .take(destination_layout.pixel_count)
                .map(|point| {
                    smooth_distance_color(
                        *point,
                        source_points,
                        source_pixels,
                        self.sample_count,
                        self.distance_power,
                    )
                })
                .collect(),
            FillFromFrameMethod::Radius => destination_points
                .iter()
                .take(destination_layout.pixel_count)
                .map(|point| {
                    radius_color(
                        *point,
                        source_points,
                        source_pixels,
                        self.radius,
                        self.fallback_color,
                    )
                })
                .collect(),
            FillFromFrameMethod::IndexStretch => unreachable!("handled before spatial branch"),
        };

        (pixels, Vec::new())
    }
}

impl FillFromFrameMethod {
    fn from_id(id: &str) -> Self {
        match id {
            "smooth_distance" => Self::SmoothDistance,
            "radius" => Self::Radius,
            "index_stretch" => Self::IndexStretch,
            _ => Self::Nearest,
        }
    }
}

fn nearest_color(point: Vec3, source_points: &[Vec3], source_pixels: &[RgbaColor]) -> RgbaColor {
    source_points
        .iter()
        .take(source_pixels.len())
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            distance_squared(point, **a).total_cmp(&distance_squared(point, **b))
        })
        .map(|(index, _)| source_pixels[index])
        .unwrap_or_else(black)
}

fn smooth_distance_color(
    point: Vec3,
    source_points: &[Vec3],
    source_pixels: &[RgbaColor],
    sample_count: usize,
    distance_power: f32,
) -> RgbaColor {
    let mut samples = source_points
        .iter()
        .take(source_pixels.len())
        .enumerate()
        .map(|(index, source_point)| (index, distance_squared(point, *source_point)))
        .collect::<Vec<_>>();
    samples.sort_by(|(_, a), (_, b)| a.total_cmp(b));

    let mut weighted = Vec::new();
    for (index, distance_sq) in samples.into_iter().take(sample_count.max(1)) {
        if distance_sq <= EPSILON {
            return source_pixels[index];
        }
        let distance = distance_sq.sqrt().max(EPSILON);
        weighted.push((source_pixels[index], 1.0 / distance.powf(distance_power)));
    }

    weighted_average(&weighted).unwrap_or_else(black)
}

fn radius_color(
    point: Vec3,
    source_points: &[Vec3],
    source_pixels: &[RgbaColor],
    radius: f32,
    fallback: RgbaColor,
) -> RgbaColor {
    if radius <= EPSILON {
        return source_points
            .iter()
            .take(source_pixels.len())
            .enumerate()
            .find(|(_, source_point)| distance_squared(point, **source_point) <= EPSILON)
            .map(|(index, _)| source_pixels[index])
            .unwrap_or(fallback);
    }

    let mut weighted = Vec::new();
    for (index, source_point) in source_points.iter().take(source_pixels.len()).enumerate() {
        let distance = distance_squared(point, *source_point).sqrt();
        if distance <= radius {
            weighted.push((source_pixels[index], (1.0 - distance / radius).max(EPSILON)));
        }
    }
    weighted_average(&weighted).unwrap_or(fallback)
}

fn weighted_average(samples: &[(RgbaColor, f32)]) -> Option<RgbaColor> {
    let total_weight = samples.iter().map(|(_, weight)| *weight).sum::<f32>();
    if total_weight <= EPSILON {
        return None;
    }

    let mut r = 0.0;
    let mut g = 0.0;
    let mut b = 0.0;
    let mut a = 0.0;
    for (color, weight) in samples {
        r += color.r * weight;
        g += color.g * weight;
        b += color.b * weight;
        a += color.a * weight;
    }

    Some(RgbaColor {
        r: (r / total_weight).clamp(0.0, 1.0),
        g: (g / total_weight).clamp(0.0, 1.0),
        b: (b / total_weight).clamp(0.0, 1.0),
        a: (a / total_weight).clamp(0.0, 1.0),
    })
}

fn resample_index(source_pixels: &[RgbaColor], destination_count: usize) -> Vec<RgbaColor> {
    if destination_count == 0 {
        return Vec::new();
    }
    if source_pixels.is_empty() {
        return vec![black(); destination_count];
    }
    if destination_count == 1 {
        return vec![source_pixels[0]];
    }
    if source_pixels.len() == 1 {
        return vec![source_pixels[0]; destination_count];
    }

    let source_last = source_pixels.len() - 1;
    let destination_last = destination_count - 1;
    (0..destination_count)
        .map(|index| {
            let source_index = ((index * source_last) + destination_last / 2) / destination_last;
            source_pixels[source_index.min(source_last)]
        })
        .collect()
}

fn distance_squared(a: Vec3, b: Vec3) -> f32 {
    (a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)
}

fn black() -> RgbaColor {
    RgbaColor {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    }
}

fn string_parameter(parameters: &HashMap<String, JsonValue>, name: &str, default: &str) -> String {
    parameters
        .get(name)
        .and_then(|value| value.as_str())
        .unwrap_or(default)
        .to_owned()
}

fn usize_parameter(parameters: &HashMap<String, JsonValue>, name: &str, default: usize) -> usize {
    parameters
        .get(name)
        .and_then(|value| value.as_u64())
        .map(|value| value as usize)
        .unwrap_or(default)
}

fn f32_parameter(parameters: &HashMap<String, JsonValue>, name: &str, default: f32) -> f32 {
    parameters
        .get(name)
        .and_then(|value| value.as_f64())
        .filter(|value| value.is_finite())
        .map(|value| value as f32)
        .unwrap_or(default)
}

fn color_parameter(
    parameters: &HashMap<String, JsonValue>,
    name: &str,
    default: RgbaColor,
) -> RgbaColor {
    match parameter_status::<RgbaColor>(parameters, name) {
        ParameterStatus::Present(color) => color,
        ParameterStatus::Missing | ParameterStatus::Invalid => default,
    }
}

#[cfg(test)]
mod tests {
    use shared::{LedLayout, LedLayoutRole};

    use super::*;

    fn color(r: f32, g: f32, b: f32) -> RgbaColor {
        RgbaColor { r, g, b, a: 1.0 }
    }

    fn context(layout: LedLayout) -> NodeEvaluationContext {
        NodeEvaluationContext {
            graph_id: "test-graph".to_owned(),
            graph_name: "Test Graph".to_owned(),
            elapsed_seconds: 0.0,
            render_layout: Some(layout),
        }
    }

    fn spatial_layout(points: Vec<Vec3>) -> LedLayout {
        LedLayout {
            id: "destination".to_owned(),
            role: LedLayoutRole::RenderTarget,
            pixel_count: points.len(),
            width: Some(points.len()),
            height: Some(1),
            points_3d: Some(points),
        }
    }

    #[test]
    fn nearest_uses_source_layout_positions() {
        let mut node = FillFromFrameNode::default();
        let source = ColorFrame {
            layout: LedLayout {
                id: "source".to_owned(),
                role: LedLayoutRole::Source,
                pixel_count: 2,
                width: Some(2),
                height: Some(1),
                points_3d: Some(vec![
                    Vec3 {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    Vec3 {
                        x: 1.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ]),
            },
            pixels: vec![color(1.0, 0.0, 0.0), color(0.0, 0.0, 1.0)],
        };
        let destination = spatial_layout(vec![
            Vec3 {
                x: 0.1,
                y: 0.0,
                z: 0.0,
            },
            Vec3 {
                x: 0.9,
                y: 0.0,
                z: 0.0,
            },
        ]);

        let output = node
            .evaluate(
                &context(destination),
                FillFromFrameInputs {
                    frame: Some(InputValue::MappedFrame(source)),
                },
            )
            .expect("evaluate")
            .outputs
            .frame
            .expect("frame output");

        assert_eq!(
            output.pixels,
            vec![color(1.0, 0.0, 0.0), color(0.0, 0.0, 1.0)]
        );
    }

    #[test]
    fn index_stretch_fills_non_spatial_destinations() {
        let mut node = FillFromFrameNode {
            method: FillFromFrameMethod::IndexStretch,
            ..FillFromFrameNode::default()
        };
        let source = ColorFrame {
            layout: LedLayout {
                id: "source".to_owned(),
                role: LedLayoutRole::Source,
                pixel_count: 2,
                width: Some(2),
                height: Some(1),
                points_3d: None,
            },
            pixels: vec![color(1.0, 0.0, 0.0), color(0.0, 0.0, 1.0)],
        };
        let destination = LedLayout {
            id: "destination".to_owned(),
            role: LedLayoutRole::RenderTarget,
            pixel_count: 3,
            width: Some(3),
            height: Some(1),
            points_3d: None,
        };

        let output = node
            .evaluate(
                &context(destination),
                FillFromFrameInputs {
                    frame: Some(InputValue::MappedFrame(source)),
                },
            )
            .expect("evaluate")
            .outputs
            .frame
            .expect("frame output");

        assert_eq!(
            output.pixels,
            vec![
                color(1.0, 0.0, 0.0),
                color(0.0, 0.0, 1.0),
                color(0.0, 0.0, 1.0)
            ]
        );
    }

    #[test]
    fn radius_uses_fallback_when_no_source_point_is_close() {
        let mut node = FillFromFrameNode {
            method: FillFromFrameMethod::Radius,
            radius: 0.25,
            fallback_color: color(0.2, 0.3, 0.4),
            ..FillFromFrameNode::default()
        };
        let source = ColorFrame {
            layout: LedLayout {
                id: "source".to_owned(),
                role: LedLayoutRole::Source,
                pixel_count: 1,
                width: Some(1),
                height: Some(1),
                points_3d: Some(vec![Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                }]),
            },
            pixels: vec![color(1.0, 0.0, 0.0)],
        };
        let destination = spatial_layout(vec![Vec3 {
            x: 10.0,
            y: 0.0,
            z: 0.0,
        }]);

        let output = node
            .evaluate(
                &context(destination),
                FillFromFrameInputs {
                    frame: Some(InputValue::MappedFrame(source)),
                },
            )
            .expect("evaluate")
            .outputs
            .frame
            .expect("frame output");

        assert_eq!(output.pixels, vec![color(0.2, 0.3, 0.4)]);
    }

    #[test]
    fn parses_method_parameters_without_source_layout() {
        let parameters =
            HashMap::from([("method".to_owned(), serde_json::json!("smooth_distance"))]);

        let node = FillFromFrameNode::from_parameters(&parameters).node;

        assert_eq!(node.method, FillFromFrameMethod::SmoothDistance);
    }

    #[test]
    fn emits_frontend_updates_for_source_and_render_layout_frame() {
        let mut node = FillFromFrameNode::default();
        let source = ColorFrame {
            layout: LedLayout {
                id: "source".to_owned(),
                role: LedLayoutRole::Source,
                pixel_count: 2,
                width: Some(2),
                height: Some(1),
                points_3d: Some(vec![
                    Vec3 {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    Vec3 {
                        x: 1.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ]),
            },
            pixels: vec![color(1.0, 0.0, 0.0), color(0.0, 0.0, 1.0)],
        };
        let destination = spatial_layout(vec![
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            Vec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
        ]);

        let evaluation = node
            .evaluate(
                &context(destination),
                FillFromFrameInputs {
                    frame: Some(InputValue::MappedFrame(source)),
                },
            )
            .expect("evaluate");

        assert_eq!(evaluation.frontend_updates.len(), 2);
        assert_eq!(evaluation.frontend_updates[0].name, "source_frame");
        assert_eq!(evaluation.frontend_updates[1].name, "frame (destination)");
        assert!(matches!(
            evaluation.frontend_updates[0].value,
            InputValue::MappedFrame(_)
        ));
        assert!(matches!(
            evaluation.frontend_updates[1].value,
            InputValue::ColorFrame(_)
        ));
    }
}
