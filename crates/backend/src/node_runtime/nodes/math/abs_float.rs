use anyhow::Result;
use shared::{NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct AbsFloatNode;

impl RuntimeNodeFromParameters for AbsFloatNode {}

pub(crate) struct AbsFloatInputs {
    value: f32,
}

crate::node_runtime::impl_runtime_inputs!(AbsFloatInputs {
    value = 0.0,
});

pub(crate) struct AbsFloatOutputs {
    value: f32,
}

crate::node_runtime::impl_runtime_outputs!(AbsFloatOutputs { value });

impl RuntimeNode for AbsFloatNode {
    type Inputs = AbsFloatInputs;
    type Outputs = AbsFloatOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let value = inputs.value.abs();
        let diagnostics = if value.is_finite() {
            Vec::new()
        } else {
            vec![NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Error,
                code: Some("abs_float_non_finite".to_owned()),
                message: "Abs Float produced a non-finite result.".to_owned(),
            }]
        };

        Ok(TypedNodeEvaluation {
            outputs: AbsFloatOutputs { value },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{AbsFloatInputs, AbsFloatNode};
    use crate::node_runtime::{NodeEvaluationContext, RuntimeNode};

    fn context() -> NodeEvaluationContext {
        NodeEvaluationContext {
            elapsed_seconds: 0.0,
            render_layout: None,
        }
    }

    #[test]
    fn returns_absolute_value() {
        let mut node = AbsFloatNode;

        let evaluation = node
            .evaluate(&context(), AbsFloatInputs { value: -4.25 })
            .expect("abs float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, 4.25);
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn reports_non_finite_results() {
        let mut node = AbsFloatNode;

        let evaluation = node
            .evaluate(
                &context(),
                AbsFloatInputs {
                    value: f32::NEG_INFINITY,
                },
            )
            .expect("abs float evaluation should succeed");

        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("abs_float_non_finite")
        );
    }
}
