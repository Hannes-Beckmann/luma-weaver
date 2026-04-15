use anyhow::Result;
use shared::{InputValue, NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::tensor::{apply_unary_float_tensor_op, input_value_is_finite};
use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct AbsNode;

impl RuntimeNodeFromParameters for AbsNode {}

pub(crate) struct AbsInputs {
    value: AnyInputValue,
}

crate::node_runtime::impl_runtime_inputs!(AbsInputs {
    value = AnyInputValue(InputValue::Float(0.0)),
});

pub(crate) struct AbsOutputs {
    value: InputValue,
}

crate::node_runtime::impl_runtime_outputs!(AbsOutputs { value });

impl RuntimeNode for AbsNode {
    type Inputs = AbsInputs;
    type Outputs = AbsOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let value = apply_unary_float_tensor_op(&inputs.value.0, |value| value.abs())?;
        let diagnostics = if input_value_is_finite(&value) {
            Vec::new()
        } else {
            vec![NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Error,
                code: Some("abs_non_finite".to_owned()),
                message: "Abs produced a non-finite result.".to_owned(),
            }]
        };

        Ok(TypedNodeEvaluation {
            outputs: AbsOutputs { value },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}

#[cfg(test)]
mod tests {
    use shared::{FloatTensor, InputValue};

    use super::{AbsInputs, AbsNode};
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
    fn returns_absolute_value() {
        let mut node = AbsNode;

        let evaluation = node
            .evaluate(
                &context(),
                AbsInputs {
                    value: AnyInputValue(InputValue::Float(-4.25)),
                },
            )
            .expect("abs float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, InputValue::Float(4.25));
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn supports_float_tensors() {
        let mut node = AbsNode;

        let evaluation = node
            .evaluate(
                &context(),
                AbsInputs {
                    value: AnyInputValue(InputValue::FloatTensor(FloatTensor {
                        shape: vec![3],
                        values: vec![-1.0, 2.5, -3.25],
                    })),
                },
            )
            .expect("abs float evaluation should succeed");

        assert_eq!(
            evaluation.outputs.value,
            InputValue::FloatTensor(FloatTensor {
                shape: vec![3],
                values: vec![1.0, 2.5, 3.25],
            })
        );
    }

    #[test]
    fn reports_non_finite_results() {
        let mut node = AbsNode;

        let evaluation = node
            .evaluate(
                &context(),
                AbsInputs {
                    value: AnyInputValue(InputValue::Float(f32::NEG_INFINITY)),
                },
            )
            .expect("abs float evaluation should succeed");

        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("abs_non_finite")
        );
    }
}
