use anyhow::Result;
use shared::{NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct ExponentialFloatNode;

impl RuntimeNodeFromParameters for ExponentialFloatNode {}

pub(crate) struct ExponentialFloatInputs {
    value: f32,
}

crate::node_runtime::impl_runtime_inputs!(ExponentialFloatInputs {
    value = 0.0,
});

pub(crate) struct ExponentialFloatOutputs {
    value: f32,
}

crate::node_runtime::impl_runtime_outputs!(ExponentialFloatOutputs { value });

impl RuntimeNode for ExponentialFloatNode {
    type Inputs = ExponentialFloatInputs;
    type Outputs = ExponentialFloatOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let value = inputs.value.exp();
        if !value.is_finite() {
            return Ok(TypedNodeEvaluation {
                outputs: ExponentialFloatOutputs { value: 0.0 },
                frontend_updates: Vec::new(),
                diagnostics: vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("exponential_float_non_finite".to_owned()),
                    message: "Exponential Float produced a non-finite result.".to_owned(),
                }],
            });
        }

        Ok(TypedNodeEvaluation::from_outputs(ExponentialFloatOutputs {
            value,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::{ExponentialFloatInputs, ExponentialFloatNode};
    use crate::node_runtime::{NodeEvaluationContext, RuntimeNode};

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
        let mut node = ExponentialFloatNode;
        let evaluation = node
            .evaluate(&context(), ExponentialFloatInputs { value: 1.0 })
            .expect("exponential float evaluation should succeed");

        assert!((evaluation.outputs.value - std::f32::consts::E).abs() < 1e-5);
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn reports_non_finite_results() {
        let mut node = ExponentialFloatNode;
        let evaluation = node
            .evaluate(&context(), ExponentialFloatInputs { value: 1000.0 })
            .expect("exponential float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, 0.0);
        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("exponential_float_non_finite")
        );
    }
}
