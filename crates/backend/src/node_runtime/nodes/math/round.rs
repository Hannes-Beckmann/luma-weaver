use anyhow::Result;
use shared::{InputValue, NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::tensor::{
    apply_unary_float_tensor_op, infer_float_tensor_target_shape, zero_like_float_output,
};
use crate::node_runtime::{AnyInputValue, NodeEvaluationContext, RuntimeNode, TypedNodeEvaluation};

#[derive(Clone, Copy)]
enum RoundMode {
    Floor,
    Round,
    Ceil,
}

pub(crate) struct RoundNode {
    mode: RoundMode,
}

impl Default for RoundNode {
    fn default() -> Self {
        Self {
            mode: RoundMode::Round,
        }
    }
}

crate::node_runtime::impl_runtime_parameters!(RoundNode {
    mode: String => |value| RoundMode::from_id(&value), default RoundMode::Round,
});

pub(crate) struct RoundInputs {
    value: AnyInputValue,
}

crate::node_runtime::impl_runtime_inputs!(RoundInputs {
    value = AnyInputValue(InputValue::Float(0.0)),
});

pub(crate) struct RoundOutputs {
    value: InputValue,
}

crate::node_runtime::impl_runtime_outputs!(RoundOutputs { value });

impl RuntimeNode for RoundNode {
    type Inputs = RoundInputs;
    type Outputs = RoundOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let output_shape = infer_float_tensor_target_shape(&[&inputs.value.0])?;
        if !crate::node_runtime::tensor::input_value_is_finite(&inputs.value.0) {
            return Ok(TypedNodeEvaluation {
                outputs: RoundOutputs {
                    value: zero_like_float_output(output_shape.as_deref()),
                },
                frontend_updates: Vec::new(),
                diagnostics: vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("round_non_finite_input".to_owned()),
                    message: "Round received a non-finite input.".to_owned(),
                }],
            });
        }

        let value = apply_unary_float_tensor_op(&inputs.value.0, |value| match self.mode {
            RoundMode::Floor => value.floor(),
            RoundMode::Round => value.round(),
            RoundMode::Ceil => value.ceil(),
        })?;

        Ok(TypedNodeEvaluation::from_outputs(RoundOutputs { value }))
    }
}

impl RoundMode {
    fn from_id(id: &str) -> Self {
        match id {
            "floor" => Self::Floor,
            "ceil" => Self::Ceil,
            _ => Self::Round,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use shared::{FloatTensor, InputValue};

    use super::{RoundInputs, RoundNode};
    use crate::node_runtime::{
        AnyInputValue, NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters,
    };
    use serde_json::Value as JsonValue;

    fn context() -> NodeEvaluationContext {
        NodeEvaluationContext {
            graph_id: "test-graph".to_owned(),
            graph_name: "Test Graph".to_owned(),
            elapsed_seconds: 0.0,
            render_layout: None,
        }
    }

    #[test]
    fn defaults_to_round_mode() {
        let mut node = RoundNode::default();
        let evaluation = node
            .evaluate(
                &context(),
                RoundInputs {
                    value: AnyInputValue(InputValue::Float(1.6)),
                },
            )
            .expect("round float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, InputValue::Float(2.0));
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn supports_floor_mode() {
        let mut parameters = HashMap::new();
        parameters.insert("mode".to_owned(), JsonValue::from("floor"));
        let mut node = RoundNode::from_parameters(&parameters).node;

        let evaluation = node
            .evaluate(
                &context(),
                RoundInputs {
                    value: AnyInputValue(InputValue::Float(1.6)),
                },
            )
            .expect("round float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, InputValue::Float(1.0));
    }

    #[test]
    fn supports_ceil_mode() {
        let mut parameters = HashMap::new();
        parameters.insert("mode".to_owned(), JsonValue::from("ceil"));
        let mut node = RoundNode::from_parameters(&parameters).node;

        let evaluation = node
            .evaluate(
                &context(),
                RoundInputs {
                    value: AnyInputValue(InputValue::Float(1.2)),
                },
            )
            .expect("round float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, InputValue::Float(2.0));
    }

    #[test]
    fn supports_float_tensors() {
        let mut node = RoundNode::default();
        let evaluation = node
            .evaluate(
                &context(),
                RoundInputs {
                    value: AnyInputValue(InputValue::FloatTensor(FloatTensor {
                        shape: vec![3],
                        values: vec![1.2, 1.6, -0.6],
                    })),
                },
            )
            .expect("round float evaluation should succeed");

        assert_eq!(
            evaluation.outputs.value,
            InputValue::FloatTensor(FloatTensor {
                shape: vec![3],
                values: vec![1.0, 2.0, -1.0],
            })
        );
    }

    #[test]
    fn rejects_non_finite_input() {
        let mut node = RoundNode::default();
        let evaluation = node
            .evaluate(
                &context(),
                RoundInputs {
                    value: AnyInputValue(InputValue::Float(f32::INFINITY)),
                },
            )
            .expect("round float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, InputValue::Float(0.0));
        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("round_non_finite_input")
        );
    }
}
