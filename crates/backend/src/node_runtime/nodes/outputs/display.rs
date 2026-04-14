use anyhow::Result;
use shared::{InputValue, NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, NodeFrontendUpdate, RuntimeNode,
    RuntimeNodeFromParameters, TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct DisplayNode;

impl RuntimeNodeFromParameters for DisplayNode {}

pub(crate) struct DisplayInputs {
    value: AnyInputValue,
}

crate::node_runtime::impl_runtime_inputs!(DisplayInputs {
    value = default_display_value(),
});

impl RuntimeNode for DisplayNode {
    type Inputs = DisplayInputs;
    type Outputs = ();

    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        if let InputValue::FloatTensor(tensor) = &inputs.value.0 {
            if tensor.shape.len() != 2 {
                return Ok(TypedNodeEvaluation {
                    outputs: (),
                    frontend_updates: Vec::new(),
                    diagnostics: vec![NodeDiagnostic {
                        severity: NodeDiagnosticSeverity::Error,
                        code: Some("display_tensor_requires_two_dimensions".to_owned()),
                        message: format!(
                            "Display only supports FloatTensor values with exactly two dimensions; received shape {:?}.",
                            tensor.shape
                        ),
                    }],
                });
            }
        }
        let value_name = context
            .render_layout
            .as_ref()
            .map(|layout| format!("value ({})", layout.id))
            .unwrap_or_else(|| "value".to_owned());
        Ok(TypedNodeEvaluation::with_frontend_updates(
            (),
            vec![NodeFrontendUpdate {
                name: value_name,
                value: inputs.value.0,
            }],
        ))
    }
}

fn default_display_value() -> AnyInputValue {
    AnyInputValue(InputValue::Float(0.0))
}

#[cfg(test)]
mod tests {
    use shared::{FloatTensor, InputValue, NodeDiagnosticSeverity};

    use super::{DisplayInputs, DisplayNode};
    use crate::node_runtime::{AnyInputValue, NodeEvaluationContext, RuntimeNode};

    fn evaluation_context() -> NodeEvaluationContext {
        NodeEvaluationContext {
            graph_id: "graph".to_owned(),
            graph_name: "Graph".to_owned(),
            elapsed_seconds: 0.0,
            render_layout: None,
        }
    }

    #[test]
    fn accepts_two_dimensional_float_tensors() {
        let mut node = DisplayNode;
        let evaluation = node
            .evaluate(
                &evaluation_context(),
                DisplayInputs {
                    value: AnyInputValue(InputValue::FloatTensor(FloatTensor {
                        shape: vec![2, 3],
                        values: vec![0.0; 6],
                    })),
                },
            )
            .expect("evaluate display node");

        assert_eq!(evaluation.frontend_updates.len(), 1);
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn rejects_non_two_dimensional_float_tensors() {
        let mut node = DisplayNode;
        let evaluation = node
            .evaluate(
                &evaluation_context(),
                DisplayInputs {
                    value: AnyInputValue(InputValue::FloatTensor(FloatTensor {
                        shape: vec![4],
                        values: vec![0.0; 4],
                    })),
                },
            )
            .expect("evaluate display node");

        assert!(evaluation.frontend_updates.is_empty());
        assert_eq!(evaluation.diagnostics.len(), 1);
        assert_eq!(
            evaluation.diagnostics[0].severity,
            NodeDiagnosticSeverity::Error
        );
    }
}
