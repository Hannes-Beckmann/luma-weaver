use std::collections::HashMap;

use anyhow::Result;
use serde_json::Value as JsonValue;
use shared::{InputValue, NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::{
    NodeEvaluation, NodeEvaluationContext, RuntimeNodeEvaluator, TypedNodeEvaluation,
};
use crate::spatial_layout::SpatialTransform;

pub(crate) fn build_transform_evaluator(
    parameters: &HashMap<String, JsonValue>,
) -> Box<dyn RuntimeNodeEvaluator> {
    Box::new(TransformNodeEvaluator {
        transform: SpatialTransform::from_parameters(parameters),
    })
}

pub(crate) fn transform_construction_diagnostics(
    _parameters: &HashMap<String, JsonValue>,
) -> Vec<NodeDiagnostic> {
    Vec::new()
}

struct TransformNodeEvaluator {
    transform: SpatialTransform,
}

impl RuntimeNodeEvaluator for TransformNodeEvaluator {
    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: &HashMap<String, InputValue>,
    ) -> Result<NodeEvaluation> {
        let Some(frame) = inputs.get("frame") else {
            return Ok(TypedNodeEvaluation::from_outputs(TransformOutputs { frame: None }).into());
        };

        let (frame, diagnostics) = match frame {
            InputValue::ColorFrame(frame) => {
                (Some(InputValue::ColorFrame(frame.clone())), Vec::new())
            }
            InputValue::MappedFrame(frame) => {
                let mut transformed = frame.clone();
                let diagnostics = match transformed.layout.points_3d.as_mut() {
                    Some(points) => {
                        for point in points.iter_mut() {
                            *point = self.transform.transform_point(*point);
                        }
                        Vec::new()
                    }
                    None => vec![NodeDiagnostic {
                        severity: NodeDiagnosticSeverity::Warning,
                        code: Some("transform_missing_spatial_layout".to_owned()),
                        message: "Transform received a mapped frame without spatial points, so it passed the frame through unchanged."
                            .to_owned(),
                    }],
                };
                (Some(InputValue::MappedFrame(transformed)), diagnostics)
            }
            other => {
                anyhow::bail!(
                    "transform expects ColorFrame or MappedFrame input, got {:?}",
                    other.value_kind()
                );
            }
        };

        Ok(TypedNodeEvaluation {
            outputs: TransformOutputs { frame },
            frontend_updates: Vec::new(),
            diagnostics,
        }
        .into())
    }
}

struct TransformOutputs {
    frame: Option<InputValue>,
}

impl TransformOutputs {
    fn into_runtime_outputs(self) -> HashMap<String, InputValue> {
        let mut outputs = HashMap::new();
        if let Some(frame) = self.frame {
            outputs.insert("frame".to_owned(), frame);
        }
        outputs
    }
}

impl From<TypedNodeEvaluation<TransformOutputs>> for NodeEvaluation {
    fn from(value: TypedNodeEvaluation<TransformOutputs>) -> Self {
        Self {
            outputs: value.outputs.into_runtime_outputs(),
            frontend_updates: value.frontend_updates,
            diagnostics: value.diagnostics,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;
    use shared::{ColorFrame, InputValue, LedLayout, LedLayoutRole, RgbaColor, Vec3};

    use super::{TransformNodeEvaluator, build_transform_evaluator};
    use crate::node_runtime::{NodeEvaluationContext, RuntimeNodeEvaluator};

    fn test_layout(points_3d: Option<Vec<Vec3>>) -> LedLayout {
        LedLayout {
            id: "source".to_owned(),
            role: LedLayoutRole::Source,
            pixel_count: 2,
            width: Some(2),
            height: Some(1),
            points_3d,
        }
    }

    fn test_frame(points_3d: Option<Vec<Vec3>>) -> ColorFrame {
        ColorFrame {
            layout: test_layout(points_3d),
            pixels: vec![
                RgbaColor {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                },
                RgbaColor {
                    r: 0.0,
                    g: 0.0,
                    b: 1.0,
                    a: 1.0,
                },
            ],
        }
    }

    fn context() -> NodeEvaluationContext {
        NodeEvaluationContext {
            graph_id: "graph".to_owned(),
            graph_name: "Graph".to_owned(),
            elapsed_seconds: 0.0,
            render_layout: None,
            graph_layout_assets: Default::default(),
        }
    }

    #[test]
    fn color_frame_passes_through_unchanged() {
        let mut evaluator =
            build_transform_evaluator(&HashMap::from([("translation_x".to_owned(), json!(5.0))]));
        let frame = test_frame(Some(vec![
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
        ]));

        let evaluation = evaluator
            .evaluate(
                &context(),
                &HashMap::from([("frame".to_owned(), InputValue::ColorFrame(frame.clone()))]),
            )
            .expect("evaluate");

        assert_eq!(
            evaluation.outputs.get("frame"),
            Some(&InputValue::ColorFrame(frame))
        );
    }

    #[test]
    fn mapped_frame_is_forward_transformed() {
        let mut evaluator = TransformNodeEvaluator {
            transform: crate::spatial_layout::SpatialTransform {
                translation: Vec3 {
                    x: 2.0,
                    y: -1.0,
                    z: 0.5,
                },
                roll_degrees: 0.0,
                pitch_degrees: 0.0,
                yaw_degrees: 90.0,
            },
        };

        let evaluation = evaluator
            .evaluate(
                &context(),
                &HashMap::from([(
                    "frame".to_owned(),
                    InputValue::MappedFrame(test_frame(Some(vec![
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
                    ]))),
                )]),
            )
            .expect("evaluate");

        let InputValue::MappedFrame(frame) = evaluation
            .outputs
            .get("frame")
            .expect("frame output")
            .clone()
        else {
            panic!("expected mapped frame output");
        };

        let points = frame.layout.points_3d.expect("spatial points");
        assert!((points[0].x - 2.0).abs() < 1e-5);
        assert!((points[0].y + 1.0).abs() < 1e-5);
        assert!((points[0].z - 0.5).abs() < 1e-5);
        assert!((points[1].x - 2.0).abs() < 1e-5);
        assert!((points[1].y - 0.0).abs() < 1e-5);
        assert!((points[1].z - 0.5).abs() < 1e-5);
    }
}
