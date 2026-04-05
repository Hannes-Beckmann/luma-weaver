use anyhow::Result;
use shared::{NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::{NodeEvaluationContext, RuntimeNode, TypedNodeEvaluation};

#[derive(Clone, Copy)]
enum RoundMode {
    Floor,
    Round,
    Ceil,
}

pub(crate) struct RoundFloatNode {
    mode: RoundMode,
}

impl Default for RoundFloatNode {
    fn default() -> Self {
        Self {
            mode: RoundMode::Round,
        }
    }
}

crate::node_runtime::impl_runtime_parameters!(RoundFloatNode {
    mode: String => |value| RoundMode::from_id(&value), default RoundMode::Round,
});

pub(crate) struct RoundFloatInputs {
    value: f32,
}

crate::node_runtime::impl_runtime_inputs!(RoundFloatInputs {
    value = 0.0,
});

pub(crate) struct RoundFloatOutputs {
    value: f32,
}

crate::node_runtime::impl_runtime_outputs!(RoundFloatOutputs { value });

impl RuntimeNode for RoundFloatNode {
    type Inputs = RoundFloatInputs;
    type Outputs = RoundFloatOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        if !inputs.value.is_finite() {
            return Ok(TypedNodeEvaluation {
                outputs: RoundFloatOutputs { value: 0.0 },
                frontend_updates: Vec::new(),
                diagnostics: vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("round_float_non_finite_input".to_owned()),
                    message: "Round Float received a non-finite input.".to_owned(),
                }],
            });
        }

        let value = match self.mode {
            RoundMode::Floor => inputs.value.floor(),
            RoundMode::Round => inputs.value.round(),
            RoundMode::Ceil => inputs.value.ceil(),
        };

        Ok(TypedNodeEvaluation::from_outputs(RoundFloatOutputs {
            value,
        }))
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

    use super::{RoundFloatInputs, RoundFloatNode};
    use crate::node_runtime::{NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters};
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
        let mut node = RoundFloatNode::default();
        let evaluation = node
            .evaluate(&context(), RoundFloatInputs { value: 1.6 })
            .expect("round float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, 2.0);
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn supports_floor_mode() {
        let mut parameters = HashMap::new();
        parameters.insert("mode".to_owned(), JsonValue::from("floor"));
        let mut node = RoundFloatNode::from_parameters(&parameters).node;

        let evaluation = node
            .evaluate(&context(), RoundFloatInputs { value: 1.6 })
            .expect("round float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, 1.0);
    }

    #[test]
    fn supports_ceil_mode() {
        let mut parameters = HashMap::new();
        parameters.insert("mode".to_owned(), JsonValue::from("ceil"));
        let mut node = RoundFloatNode::from_parameters(&parameters).node;

        let evaluation = node
            .evaluate(&context(), RoundFloatInputs { value: 1.2 })
            .expect("round float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, 2.0);
    }

    #[test]
    fn rejects_non_finite_input() {
        let mut node = RoundFloatNode::default();
        let evaluation = node
            .evaluate(
                &context(),
                RoundFloatInputs {
                    value: f32::INFINITY,
                },
            )
            .expect("round float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, 0.0);
        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("round_float_non_finite_input")
        );
    }
}
