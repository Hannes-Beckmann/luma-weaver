use anyhow::Result;
use shared::{InputValue, NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::tensor::{
    apply_binary_float_tensor_op, infer_float_tensor_target_shape, input_value_is_finite,
    zero_like_float_output,
};
use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct PowerNode;

impl RuntimeNodeFromParameters for PowerNode {}

pub(crate) struct PowerInputs {
    base: AnyInputValue,
    exponent: AnyInputValue,
}

crate::node_runtime::impl_runtime_inputs!(PowerInputs {
    base = AnyInputValue(InputValue::Float(1.0)),
    exponent = AnyInputValue(InputValue::Float(1.0)),
});

pub(crate) struct PowerOutputs {
    value: InputValue,
}

crate::node_runtime::impl_runtime_outputs!(PowerOutputs { value });

impl RuntimeNode for PowerNode {
    type Inputs = PowerInputs;
    type Outputs = PowerOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let output_shape = infer_float_tensor_target_shape(&[&inputs.base.0, &inputs.exponent.0])?;
        let value = apply_binary_float_tensor_op(&inputs.base.0, &inputs.exponent.0, f32::powf)?;
        if !input_value_is_finite(&value) {
            return Ok(TypedNodeEvaluation {
                outputs: PowerOutputs {
                    value: zero_like_float_output(output_shape.as_deref()),
                },
                frontend_updates: Vec::new(),
                diagnostics: vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("power_non_finite".to_owned()),
                    message: "Power produced a non-finite result.".to_owned(),
                }],
            });
        }

        Ok(TypedNodeEvaluation::from_outputs(PowerOutputs { value }))
    }
}

#[cfg(test)]
mod tests {
    use shared::{FloatTensor, InputValue};

    use super::{PowerInputs, PowerNode};
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
    fn raises_base_to_exponent() {
        let mut node = PowerNode;
        let evaluation = node
            .evaluate(
                &context(),
                PowerInputs {
                    base: AnyInputValue(InputValue::Float(2.0)),
                    exponent: AnyInputValue(InputValue::Float(3.0)),
                },
            )
            .expect("power float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, InputValue::Float(8.0));
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn supports_float_tensors() {
        let mut node = PowerNode;
        let evaluation = node
            .evaluate(
                &context(),
                PowerInputs {
                    base: AnyInputValue(InputValue::FloatTensor(FloatTensor {
                        shape: vec![2],
                        values: vec![2.0, 3.0],
                    })),
                    exponent: AnyInputValue(InputValue::Float(2.0)),
                },
            )
            .expect("power float evaluation should succeed");

        assert_eq!(
            evaluation.outputs.value,
            InputValue::FloatTensor(FloatTensor {
                shape: vec![2],
                values: vec![4.0, 9.0],
            })
        );
    }

    #[test]
    fn reports_non_finite_results() {
        let mut node = PowerNode;
        let evaluation = node
            .evaluate(
                &context(),
                PowerInputs {
                    base: AnyInputValue(InputValue::Float(f32::INFINITY)),
                    exponent: AnyInputValue(InputValue::Float(2.0)),
                },
            )
            .expect("power float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, InputValue::Float(0.0));
        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("power_non_finite")
        );
    }
}
