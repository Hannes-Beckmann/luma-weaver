use anyhow::Result;
use shared::{NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::{NodeEvaluationContext, RuntimeNode, TypedNodeEvaluation};

#[derive(Clone, Copy)]
enum MinMaxMode {
    Min,
    Max,
}

pub(crate) struct MinMaxFloatNode {
    mode: MinMaxMode,
}

impl Default for MinMaxFloatNode {
    fn default() -> Self {
        Self {
            mode: MinMaxMode::Min,
        }
    }
}

crate::node_runtime::impl_runtime_parameters!(MinMaxFloatNode {
    mode: String => |value| MinMaxMode::from_id(&value), default MinMaxMode::Min,
});

pub(crate) struct MinMaxFloatInputs {
    a: f32,
    b: f32,
}

crate::node_runtime::impl_runtime_inputs!(MinMaxFloatInputs {
    a = 0.0,
    b = 0.0,
});

pub(crate) struct MinMaxFloatOutputs {
    value: f32,
}

crate::node_runtime::impl_runtime_outputs!(MinMaxFloatOutputs { value });

impl RuntimeNode for MinMaxFloatNode {
    type Inputs = MinMaxFloatInputs;
    type Outputs = MinMaxFloatOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let value = match self.mode {
            MinMaxMode::Min => inputs.a.min(inputs.b),
            MinMaxMode::Max => inputs.a.max(inputs.b),
        };
        let diagnostics = if value.is_finite() {
            Vec::new()
        } else {
            vec![NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Error,
                code: Some("min_max_float_non_finite".to_owned()),
                message: "Min/Max Float produced a non-finite result.".to_owned(),
            }]
        };

        Ok(TypedNodeEvaluation {
            outputs: MinMaxFloatOutputs { value },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}

impl MinMaxMode {
    fn from_id(id: &str) -> Self {
        match id {
            "max" => Self::Max,
            _ => Self::Min,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::Value as JsonValue;

    use super::{MinMaxFloatInputs, MinMaxFloatNode};
    use crate::node_runtime::{NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters};

    fn context() -> NodeEvaluationContext {
        NodeEvaluationContext {
            graph_id: "test-graph".to_owned(),
            graph_name: "Test Graph".to_owned(),
            elapsed_seconds: 0.0,
            render_layout: None,
        }
    }

    #[test]
    fn defaults_to_min_mode() {
        let mut node = MinMaxFloatNode::default();

        let evaluation = node
            .evaluate(&context(), MinMaxFloatInputs { a: 3.0, b: -2.0 })
            .expect("min/max float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, -2.0);
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn supports_max_mode_from_parameters() {
        let mut parameters = HashMap::new();
        parameters.insert("mode".to_owned(), JsonValue::from("max"));
        let mut node = MinMaxFloatNode::from_parameters(&parameters).node;

        let evaluation = node
            .evaluate(&context(), MinMaxFloatInputs { a: 3.0, b: -2.0 })
            .expect("min/max float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, 3.0);
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn reports_non_finite_results() {
        let mut parameters = HashMap::new();
        parameters.insert("mode".to_owned(), JsonValue::from("max"));
        let mut node = MinMaxFloatNode::from_parameters(&parameters).node;

        let evaluation = node
            .evaluate(
                &context(),
                MinMaxFloatInputs {
                    a: f32::INFINITY,
                    b: 1.0,
                },
            )
            .expect("min/max float evaluation should succeed");

        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("min_max_float_non_finite")
        );
    }
}
