use anyhow::Result;
use shared::{InputValue, NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::tensor::{
    apply_binary_float_tensor_op, coerce_float_tensor, infer_float_tensor_target_shape,
    input_value_is_finite, zero_like_float_output,
};
use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct DivideNode;

impl RuntimeNodeFromParameters for DivideNode {}

pub(crate) struct DivideInputs {
    a: AnyInputValue,
    b: AnyInputValue,
}

crate::node_runtime::impl_runtime_inputs!(DivideInputs {
    a = AnyInputValue(InputValue::Float(0.0)),
    b = AnyInputValue(InputValue::Float(1.0)),
});

pub(crate) struct DivideOutputs {
    quotient: InputValue,
}

crate::node_runtime::impl_runtime_outputs!(DivideOutputs { quotient });

impl RuntimeNode for DivideNode {
    type Inputs = DivideInputs;
    type Outputs = DivideOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let output_shape = infer_float_tensor_target_shape(&[&inputs.a.0, &inputs.b.0])?;

        if has_division_by_zero(&inputs.b.0) {
            return Ok(TypedNodeEvaluation {
                outputs: DivideOutputs {
                    quotient: zero_like_float_output(output_shape.as_deref()),
                },
                frontend_updates: Vec::new(),
                diagnostics: vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("divide_division_by_zero".to_owned()),
                    message: "Divide cannot divide by zero.".to_owned(),
                }],
            });
        }

        let quotient = apply_binary_float_tensor_op(&inputs.a.0, &inputs.b.0, |a, b| a / b)?;
        let diagnostics = if input_value_is_finite(&quotient) {
            Vec::new()
        } else {
            return Ok(TypedNodeEvaluation {
                outputs: DivideOutputs {
                    quotient: zero_like_float_output(output_shape.as_deref()),
                },
                frontend_updates: Vec::new(),
                diagnostics: vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("divide_non_finite".to_owned()),
                    message: "Divide produced a non-finite result.".to_owned(),
                }],
            });
        };

        Ok(TypedNodeEvaluation {
            outputs: DivideOutputs { quotient },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}

fn has_division_by_zero(value: &InputValue) -> bool {
    match value {
        InputValue::Float(value) => *value == 0.0,
        InputValue::FloatTensor(tensor) => coerce_float_tensor(value, &tensor.shape)
            .map(|tensor| tensor.values.iter().any(|value| *value == 0.0))
            .unwrap_or(false),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use shared::{FloatTensor, InputValue};

    use super::{DivideInputs, DivideNode};
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
    fn divides_inputs() {
        let mut node = DivideNode;

        let evaluation = node
            .evaluate(
                &context(),
                DivideInputs {
                    a: AnyInputValue(InputValue::Float(9.0)),
                    b: AnyInputValue(InputValue::Float(3.0)),
                },
            )
            .expect("divide float evaluation should succeed");

        assert_eq!(evaluation.outputs.quotient, InputValue::Float(3.0));
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn reports_division_by_zero_with_safe_fallback() {
        let mut node = DivideNode;

        let evaluation = node
            .evaluate(
                &context(),
                DivideInputs {
                    a: AnyInputValue(InputValue::Float(9.0)),
                    b: AnyInputValue(InputValue::Float(0.0)),
                },
            )
            .expect("divide float evaluation should succeed");

        assert_eq!(evaluation.outputs.quotient, InputValue::Float(0.0));
        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("divide_division_by_zero")
        );
    }

    #[test]
    fn supports_float_tensors() {
        let mut node = DivideNode;

        let evaluation = node
            .evaluate(
                &context(),
                DivideInputs {
                    a: AnyInputValue(InputValue::FloatTensor(FloatTensor {
                        shape: vec![2],
                        values: vec![9.0, 6.0],
                    })),
                    b: AnyInputValue(InputValue::Float(3.0)),
                },
            )
            .expect("divide float evaluation should succeed");

        assert_eq!(
            evaluation.outputs.quotient,
            InputValue::FloatTensor(FloatTensor {
                shape: vec![2],
                values: vec![3.0, 2.0],
            })
        );
    }

    #[test]
    fn reports_non_finite_results() {
        let mut node = DivideNode;

        let evaluation = node
            .evaluate(
                &context(),
                DivideInputs {
                    a: AnyInputValue(InputValue::Float(f32::INFINITY)),
                    b: AnyInputValue(InputValue::Float(2.0)),
                },
            )
            .expect("divide float evaluation should succeed");

        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("divide_non_finite")
        );
    }
}
