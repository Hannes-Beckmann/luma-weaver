use anyhow::Result;
use shared::{InputValue, NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::tensor::{apply_binary_numeric_op, input_value_is_finite};
use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct MultiplyNode;

impl RuntimeNodeFromParameters for MultiplyNode {}

pub(crate) struct MultiplyInputs {
    a: AnyInputValue,
    b: AnyInputValue,
}

crate::node_runtime::impl_runtime_inputs!(MultiplyInputs {
    a = AnyInputValue(InputValue::Float(1.0)),
    b = AnyInputValue(InputValue::Float(1.0)),
});

pub(crate) struct MultiplyOutputs {
    product: InputValue,
}

crate::node_runtime::impl_runtime_outputs!(MultiplyOutputs { product });

impl RuntimeNode for MultiplyNode {
    type Inputs = MultiplyInputs;
    type Outputs = MultiplyOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let product = apply_binary_numeric_op(&inputs.a.0, &inputs.b.0, "multiply", |a, b| a * b)?;
        let diagnostics = if input_value_is_finite(&product) {
            Vec::new()
        } else {
            vec![NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Error,
                code: Some("multiply_non_finite".to_owned()),
                message: "Multiply produced a non-finite result.".to_owned(),
            }]
        };

        Ok(TypedNodeEvaluation {
            outputs: MultiplyOutputs { product },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}

#[cfg(test)]
mod tests {
    use shared::{ColorFrame, FloatTensor, InputValue, LedLayout, RgbaColor};

    use super::{MultiplyInputs, MultiplyNode};
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
    fn multiplies_scalars() {
        let mut node = MultiplyNode;

        let evaluation = node
            .evaluate(
                &context(),
                MultiplyInputs {
                    a: AnyInputValue(InputValue::Float(3.0)),
                    b: AnyInputValue(InputValue::Float(2.0)),
                },
            )
            .expect("multiply float evaluation should succeed");

        assert_eq!(evaluation.outputs.product, InputValue::Float(6.0));
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn multiplies_tensors_element_wise() {
        let mut node = MultiplyNode;

        let evaluation = node
            .evaluate(
                &context(),
                MultiplyInputs {
                    a: AnyInputValue(InputValue::FloatTensor(FloatTensor {
                        shape: vec![2],
                        values: vec![2.0, 4.0],
                    })),
                    b: AnyInputValue(InputValue::Float(0.5)),
                },
            )
            .expect("multiply float evaluation should succeed");

        assert_eq!(
            evaluation.outputs.product,
            InputValue::FloatTensor(FloatTensor {
                shape: vec![2],
                values: vec![1.0, 2.0],
            })
        );
    }

    #[test]
    fn multiplies_frames_channel_wise() {
        let mut node = MultiplyNode;
        let layout = LedLayout {
            id: "frame".to_owned(),
            pixel_count: 1,
            width: Some(1),
            height: Some(1),
        };

        let evaluation = node
            .evaluate(
                &context(),
                MultiplyInputs {
                    a: AnyInputValue(InputValue::ColorFrame(ColorFrame {
                        layout: layout.clone(),
                        pixels: vec![RgbaColor {
                            r: 0.2,
                            g: 0.4,
                            b: 0.6,
                            a: 0.8,
                        }],
                    })),
                    b: AnyInputValue(InputValue::Float(0.5)),
                },
            )
            .expect("multiply float evaluation should succeed");

        assert_eq!(
            evaluation.outputs.product,
            InputValue::ColorFrame(ColorFrame {
                layout,
                pixels: vec![RgbaColor {
                    r: 0.1,
                    g: 0.2,
                    b: 0.3,
                    a: 0.4,
                }],
            })
        );
    }

    #[test]
    fn reports_non_finite_results() {
        let mut node = MultiplyNode;

        let evaluation = node
            .evaluate(
                &context(),
                MultiplyInputs {
                    a: AnyInputValue(InputValue::Float(f32::INFINITY)),
                    b: AnyInputValue(InputValue::Float(1.0)),
                },
            )
            .expect("multiply float evaluation should succeed");

        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("multiply_non_finite")
        );
    }
}
