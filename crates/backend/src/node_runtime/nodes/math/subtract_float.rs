use anyhow::Result;
use shared::{NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct SubtractFloatNode;

impl RuntimeNodeFromParameters for SubtractFloatNode {}

pub(crate) struct SubtractFloatInputs {
    a: f32,
    b: f32,
}

crate::node_runtime::impl_runtime_inputs!(SubtractFloatInputs {
    a = 0.0,
    b = 0.0,
});

pub(crate) struct SubtractFloatOutputs {
    difference: f32,
}

crate::node_runtime::impl_runtime_outputs!(SubtractFloatOutputs { difference });

impl RuntimeNode for SubtractFloatNode {
    type Inputs = SubtractFloatInputs;
    type Outputs = SubtractFloatOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let difference = inputs.a - inputs.b;
        let diagnostics = if difference.is_finite() {
            Vec::new()
        } else {
            vec![NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Error,
                code: Some("subtract_float_non_finite".to_owned()),
                message: "Subtract Float produced a non-finite result.".to_owned(),
            }]
        };

        Ok(TypedNodeEvaluation {
            outputs: SubtractFloatOutputs { difference },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{SubtractFloatInputs, SubtractFloatNode};
    use crate::node_runtime::{NodeEvaluationContext, RuntimeNode};

    fn context() -> NodeEvaluationContext {
        NodeEvaluationContext {
            elapsed_seconds: 0.0,
            render_layout: None,
        }
    }

    #[test]
    fn subtracts_inputs() {
        let mut node = SubtractFloatNode;

        let evaluation = node
            .evaluate(&context(), SubtractFloatInputs { a: 7.5, b: 2.0 })
            .expect("subtract float evaluation should succeed");

        assert_eq!(evaluation.outputs.difference, 5.5);
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn reports_non_finite_results() {
        let mut node = SubtractFloatNode;

        let evaluation = node
            .evaluate(
                &context(),
                SubtractFloatInputs {
                    a: f32::INFINITY,
                    b: 1.0,
                },
            )
            .expect("subtract float evaluation should succeed");

        assert!(!evaluation.diagnostics.is_empty());
        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("subtract_float_non_finite")
        );
    }
}
