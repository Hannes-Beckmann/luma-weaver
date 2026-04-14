use anyhow::Result;
use shared::{InputValue, NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::tensor::{
    apply_unary_float_tensor_op, infer_float_tensor_target_shape, input_value_is_finite,
    zero_like_float_output,
};
use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct ExponentialNode;

impl RuntimeNodeFromParameters for ExponentialNode {}

pub(crate) struct ExponentialInputs {
    value: AnyInputValue,
}

crate::node_runtime::impl_runtime_inputs!(ExponentialInputs {
    value = AnyInputValue(InputValue::Float(0.0)),
});

pub(crate) struct ExponentialOutputs {
    value: InputValue,
}

crate::node_runtime::impl_runtime_outputs!(ExponentialOutputs { value });

impl RuntimeNode for ExponentialNode {
    type Inputs = ExponentialInputs;
    type Outputs = ExponentialOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let output_shape = infer_float_tensor_target_shape(&[&inputs.value.0])?;
        let value = apply_unary_float_tensor_op(&inputs.value.0, f32::exp)?;
        if !input_value_is_finite(&value) {
            return Ok(TypedNodeEvaluation {
                outputs: ExponentialOutputs {
                    value: zero_like_float_output(output_shape.as_deref()),
                },
                frontend_updates: Vec::new(),
                diagnostics: vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("exponential_non_finite".to_owned()),
                    message: "Exponential produced a non-finite result.".to_owned(),
                }],
            });
        }

        Ok(TypedNodeEvaluation::from_outputs(ExponentialOutputs {
            value,
        }))
    }
}

#[cfg(test)]
mod tests {
    use shared::{FloatTensor, InputValue};

    use super::{ExponentialInputs, ExponentialNode};
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
    fn computes_e_to_the_input() {
        let mut node = ExponentialNode;
        let evaluation = node
            .evaluate(
                &context(),
                ExponentialInputs {
                    value: AnyInputValue(InputValue::Float(1.0)),
                },
            )
            .expect("exponential float evaluation should succeed");

        match evaluation.outputs.value {
            InputValue::Float(value) => assert!((value - std::f32::consts::E).abs() < 1e-5),
            other => panic!("expected float output, got {:?}", other),
        }
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn supports_float_tensors() {
        let mut node = ExponentialNode;
        let evaluation = node
            .evaluate(
                &context(),
                ExponentialInputs {
                    value: AnyInputValue(InputValue::FloatTensor(FloatTensor {
                        shape: vec![2],
                        values: vec![0.0, 1.0],
                    })),
                },
            )
            .expect("exponential float evaluation should succeed");

        assert_eq!(
            evaluation.outputs.value,
            InputValue::FloatTensor(FloatTensor {
                shape: vec![2],
                values: vec![1.0, std::f32::consts::E],
            })
        );
    }

    #[test]
    fn reports_non_finite_results() {
        let mut node = ExponentialNode;
        let evaluation = node
            .evaluate(
                &context(),
                ExponentialInputs {
                    value: AnyInputValue(InputValue::Float(1000.0)),
                },
            )
            .expect("exponential float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, InputValue::Float(0.0));
        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("exponential_non_finite")
        );
    }
}
