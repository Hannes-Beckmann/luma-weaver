use anyhow::Result;
use shared::{InputValue, NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::tensor::{apply_binary_numeric_op, input_value_is_finite};
use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct SubtractNode;

impl RuntimeNodeFromParameters for SubtractNode {}

pub(crate) struct SubtractInputs {
    a: AnyInputValue,
    b: AnyInputValue,
}

crate::node_runtime::impl_runtime_inputs!(SubtractInputs {
    a = AnyInputValue(InputValue::Float(0.0)),
    b = AnyInputValue(InputValue::Float(0.0)),
});

pub(crate) struct SubtractOutputs {
    difference: InputValue,
}

crate::node_runtime::impl_runtime_outputs!(SubtractOutputs { difference });

impl RuntimeNode for SubtractNode {
    type Inputs = SubtractInputs;
    type Outputs = SubtractOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let difference =
            apply_binary_numeric_op(&inputs.a.0, &inputs.b.0, "subtract", |a, b| a - b)?;
        let diagnostics = if input_value_is_finite(&difference) {
            Vec::new()
        } else {
            vec![NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Error,
                code: Some("subtract_non_finite".to_owned()),
                message: "Subtract produced a non-finite result.".to_owned(),
            }]
        };

        Ok(TypedNodeEvaluation {
            outputs: SubtractOutputs { difference },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}

#[cfg(test)]
mod tests {
    use shared::{ColorFrame, FloatTensor, InputValue, LedLayout, RgbaColor};

    use super::{SubtractInputs, SubtractNode};
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
    fn subtracts_inputs() {
        let mut node = SubtractNode;

        let evaluation = node
            .evaluate(
                &context(),
                SubtractInputs {
                    a: AnyInputValue(InputValue::Float(7.5)),
                    b: AnyInputValue(InputValue::Float(2.0)),
                },
            )
            .expect("subtract float evaluation should succeed");

        assert_eq!(evaluation.outputs.difference, InputValue::Float(5.5));
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn subtracts_tensors_element_wise() {
        let mut node = SubtractNode;

        let evaluation = node
            .evaluate(
                &context(),
                SubtractInputs {
                    a: AnyInputValue(InputValue::FloatTensor(FloatTensor {
                        shape: vec![2],
                        values: vec![3.0, 5.0],
                    })),
                    b: AnyInputValue(InputValue::Float(1.0)),
                },
            )
            .expect("subtract float evaluation should succeed");

        assert_eq!(
            evaluation.outputs.difference,
            InputValue::FloatTensor(FloatTensor {
                shape: vec![2],
                values: vec![2.0, 4.0],
            })
        );
    }

    #[test]
    fn subtracts_frames_channel_wise() {
        let mut node = SubtractNode;
        let layout = LedLayout {
            id: "frame".to_owned(),
            pixel_count: 1,
            width: Some(1),
            height: Some(1),
        };

        let evaluation = node
            .evaluate(
                &context(),
                SubtractInputs {
                    a: AnyInputValue(InputValue::ColorFrame(ColorFrame {
                        layout: layout.clone(),
                        pixels: vec![RgbaColor {
                            r: 0.9,
                            g: 0.8,
                            b: 0.7,
                            a: 0.6,
                        }],
                    })),
                    b: AnyInputValue(InputValue::FloatTensor(FloatTensor {
                        shape: vec![1, 1],
                        values: vec![0.1],
                    })),
                },
            )
            .expect("subtract float evaluation should succeed");

        assert_eq!(
            evaluation.outputs.difference,
            InputValue::ColorFrame(ColorFrame {
                layout,
                pixels: vec![RgbaColor {
                    r: 0.799_999_95,
                    g: 0.7,
                    b: 0.599_999_96,
                    a: 0.5,
                }],
            })
        );
    }

    #[test]
    fn reports_non_finite_results() {
        let mut node = SubtractNode;

        let evaluation = node
            .evaluate(
                &context(),
                SubtractInputs {
                    a: AnyInputValue(InputValue::Float(f32::INFINITY)),
                    b: AnyInputValue(InputValue::Float(1.0)),
                },
            )
            .expect("subtract float evaluation should succeed");

        assert!(!evaluation.diagnostics.is_empty());
        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("subtract_non_finite")
        );
    }
}
