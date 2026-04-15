use anyhow::Result;
use shared::{InputValue, NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::tensor::{clamp_numeric_value, input_value_is_finite};
use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct ClampNode;

impl RuntimeNodeFromParameters for ClampNode {}

pub(crate) struct ClampInputs {
    value: AnyInputValue,
    min: AnyInputValue,
    max: AnyInputValue,
}

crate::node_runtime::impl_runtime_inputs!(ClampInputs {
    value = AnyInputValue(InputValue::Float(0.0)),
    min = AnyInputValue(InputValue::Float(0.0)),
    max = AnyInputValue(InputValue::Float(1.0)),
});

pub(crate) struct ClampOutputs {
    value: InputValue,
}

crate::node_runtime::impl_runtime_outputs!(ClampOutputs { value });

impl RuntimeNode for ClampNode {
    type Inputs = ClampInputs;
    type Outputs = ClampOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        if !input_value_is_finite(&inputs.value.0)
            || !input_value_is_finite(&inputs.min.0)
            || !input_value_is_finite(&inputs.max.0)
        {
            return Ok(TypedNodeEvaluation {
                outputs: ClampOutputs {
                    value: InputValue::Float(0.0),
                },
                frontend_updates: Vec::new(),
                diagnostics: vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("clamp_non_finite_input".to_owned()),
                    message: "Clamp received a non-finite input.".to_owned(),
                }],
            });
        }

        let mut diagnostics = Vec::new();
        let min_value = match &inputs.min.0 {
            InputValue::Float(value) => Some(*value),
            InputValue::FloatTensor(tensor) => tensor.values.first().copied(),
            _ => None,
        };
        let max_value = match &inputs.max.0 {
            InputValue::Float(value) => Some(*value),
            InputValue::FloatTensor(tensor) => tensor.values.first().copied(),
            _ => None,
        };

        let (min, max) = if min_value.unwrap_or(0.0) <= max_value.unwrap_or(0.0) {
            (inputs.min.0, inputs.max.0)
        } else {
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Warning,
                code: Some("clamp_bounds_swapped".to_owned()),
                message: "Clamp received min greater than max and swapped the bounds.".to_owned(),
            });
            (inputs.max.0, inputs.min.0)
        };

        let value = clamp_numeric_value(&inputs.value.0, &min, &max, "clamp")?;

        Ok(TypedNodeEvaluation {
            outputs: ClampOutputs { value },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}

#[cfg(test)]
mod tests {
    use shared::{ColorFrame, FloatTensor, InputValue, LedLayout, RgbaColor};

    use super::{ClampInputs, ClampNode};
    use crate::node_runtime::{AnyInputValue, NodeEvaluationContext, RuntimeNode};

    fn context() -> NodeEvaluationContext {
        NodeEvaluationContext {
            graph_id: "test-graph".to_owned(),
            graph_name: "Test Graph".to_owned(),
            elapsed_seconds: 0.0,
            render_layout: None,
        }
    }

    #[test]
    fn clamps_into_range() {
        let mut node = ClampNode;

        let evaluation = node
            .evaluate(
                &context(),
                ClampInputs {
                    value: AnyInputValue(InputValue::Float(5.0)),
                    min: AnyInputValue(InputValue::Float(1.0)),
                    max: AnyInputValue(InputValue::Float(3.0)),
                },
            )
            .expect("clamp float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, InputValue::Float(3.0));
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn swaps_reversed_bounds_with_warning() {
        let mut node = ClampNode;

        let evaluation = node
            .evaluate(
                &context(),
                ClampInputs {
                    value: AnyInputValue(InputValue::Float(2.0)),
                    min: AnyInputValue(InputValue::Float(5.0)),
                    max: AnyInputValue(InputValue::Float(1.0)),
                },
            )
            .expect("clamp float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, InputValue::Float(2.0));
        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("clamp_bounds_swapped")
        );
    }

    #[test]
    fn clamps_tensors_element_wise() {
        let mut node = ClampNode;

        let evaluation = node
            .evaluate(
                &context(),
                ClampInputs {
                    value: AnyInputValue(InputValue::FloatTensor(FloatTensor {
                        shape: vec![3],
                        values: vec![-1.0, 0.5, 2.0],
                    })),
                    min: AnyInputValue(InputValue::Float(0.0)),
                    max: AnyInputValue(InputValue::FloatTensor(FloatTensor {
                        shape: vec![3],
                        values: vec![0.25, 0.75, 1.5],
                    })),
                },
            )
            .expect("clamp float evaluation should succeed");

        assert_eq!(
            evaluation.outputs.value,
            InputValue::FloatTensor(FloatTensor {
                shape: vec![3],
                values: vec![0.0, 0.5, 1.5],
            })
        );
    }

    #[test]
    fn clamps_frames_channel_wise() {
        let mut node = ClampNode;
        let layout = LedLayout {
            id: "frame".to_owned(),
            pixel_count: 1,
            width: Some(1),
            height: Some(1),
        };

        let evaluation = node
            .evaluate(
                &context(),
                ClampInputs {
                    value: AnyInputValue(InputValue::ColorFrame(ColorFrame {
                        layout: layout.clone(),
                        pixels: vec![RgbaColor {
                            r: -0.5,
                            g: 0.3,
                            b: 1.7,
                            a: 0.8,
                        }],
                    })),
                    min: AnyInputValue(InputValue::Float(0.0)),
                    max: AnyInputValue(InputValue::Float(1.0)),
                },
            )
            .expect("clamp float evaluation should succeed");

        assert_eq!(
            evaluation.outputs.value,
            InputValue::ColorFrame(ColorFrame {
                layout,
                pixels: vec![RgbaColor {
                    r: 0.0,
                    g: 0.3,
                    b: 1.0,
                    a: 0.8,
                }],
            })
        );
    }

    #[test]
    fn reports_non_finite_results() {
        let mut node = ClampNode;

        let evaluation = node
            .evaluate(
                &context(),
                ClampInputs {
                    value: AnyInputValue(InputValue::Float(f32::INFINITY)),
                    min: AnyInputValue(InputValue::Float(0.0)),
                    max: AnyInputValue(InputValue::Float(1.0)),
                },
            )
            .expect("clamp float evaluation should succeed");

        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("clamp_non_finite_input")
        );
    }
}
