use anyhow::Result;
use shared::{InputValue, NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::tensor::{apply_binary_float_tensor_op, input_value_is_finite};
use crate::node_runtime::{AnyInputValue, NodeEvaluationContext, RuntimeNode, TypedNodeEvaluation};

#[derive(Clone, Copy)]
enum MinMaxMode {
    Min,
    Max,
}

pub(crate) struct MinMaxNode {
    mode: MinMaxMode,
}

impl Default for MinMaxNode {
    fn default() -> Self {
        Self {
            mode: MinMaxMode::Min,
        }
    }
}

crate::node_runtime::impl_runtime_parameters!(MinMaxNode {
    mode: String => |value| MinMaxMode::from_id(&value), default MinMaxMode::Min,
});

pub(crate) struct MinMaxInputs {
    a: AnyInputValue,
    b: AnyInputValue,
}

crate::node_runtime::impl_runtime_inputs!(MinMaxInputs {
    a = AnyInputValue(InputValue::Float(0.0)),
    b = AnyInputValue(InputValue::Float(0.0)),
});

pub(crate) struct MinMaxOutputs {
    value: InputValue,
}

crate::node_runtime::impl_runtime_outputs!(MinMaxOutputs { value });

impl RuntimeNode for MinMaxNode {
    type Inputs = MinMaxInputs;
    type Outputs = MinMaxOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let value = match self.mode {
            MinMaxMode::Min => apply_binary_float_tensor_op(&inputs.a.0, &inputs.b.0, f32::min)?,
            MinMaxMode::Max => apply_binary_float_tensor_op(&inputs.a.0, &inputs.b.0, f32::max)?,
        };
        let diagnostics = if input_value_is_finite(&value) {
            Vec::new()
        } else {
            vec![NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Error,
                code: Some("min_max_non_finite".to_owned()),
                message: "Min/Max produced a non-finite result.".to_owned(),
            }]
        };

        Ok(TypedNodeEvaluation {
            outputs: MinMaxOutputs { value },
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

    use shared::{FloatTensor, InputValue};

    use super::{MinMaxInputs, MinMaxNode};
    use crate::node_runtime::{
        AnyInputValue, NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters,
    };

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
        let mut node = MinMaxNode::default();

        let evaluation = node
            .evaluate(
                &context(),
                MinMaxInputs {
                    a: AnyInputValue(InputValue::Float(3.0)),
                    b: AnyInputValue(InputValue::Float(-2.0)),
                },
            )
            .expect("min/max float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, InputValue::Float(-2.0));
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn supports_max_mode_from_parameters() {
        let mut parameters = HashMap::new();
        parameters.insert("mode".to_owned(), JsonValue::from("max"));
        let mut node = MinMaxNode::from_parameters(&parameters).node;

        let evaluation = node
            .evaluate(
                &context(),
                MinMaxInputs {
                    a: AnyInputValue(InputValue::Float(3.0)),
                    b: AnyInputValue(InputValue::Float(-2.0)),
                },
            )
            .expect("min/max float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, InputValue::Float(3.0));
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn supports_tensors() {
        let mut parameters = HashMap::new();
        parameters.insert("mode".to_owned(), JsonValue::from("max"));
        let mut node = MinMaxNode::from_parameters(&parameters).node;

        let evaluation = node
            .evaluate(
                &context(),
                MinMaxInputs {
                    a: AnyInputValue(InputValue::FloatTensor(FloatTensor {
                        shape: vec![2],
                        values: vec![1.0, -2.0],
                    })),
                    b: AnyInputValue(InputValue::Float(0.5)),
                },
            )
            .expect("min/max float evaluation should succeed");

        assert_eq!(
            evaluation.outputs.value,
            InputValue::FloatTensor(FloatTensor {
                shape: vec![2],
                values: vec![1.0, 0.5],
            })
        );
    }

    #[test]
    fn reports_non_finite_results() {
        let mut parameters = HashMap::new();
        parameters.insert("mode".to_owned(), JsonValue::from("max"));
        let mut node = MinMaxNode::from_parameters(&parameters).node;

        let evaluation = node
            .evaluate(
                &context(),
                MinMaxInputs {
                    a: AnyInputValue(InputValue::Float(f32::INFINITY)),
                    b: AnyInputValue(InputValue::Float(1.0)),
                },
            )
            .expect("min/max float evaluation should succeed");

        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("min_max_non_finite")
        );
    }
}
