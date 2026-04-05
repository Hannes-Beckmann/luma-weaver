use anyhow::Result;
use shared::{NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct PowerFloatNode;

impl RuntimeNodeFromParameters for PowerFloatNode {}

pub(crate) struct PowerFloatInputs {
    base: f32,
    exponent: f32,
}

crate::node_runtime::impl_runtime_inputs!(PowerFloatInputs {
    base = 1.0,
    exponent = 1.0,
});

pub(crate) struct PowerFloatOutputs {
    value: f32,
}

crate::node_runtime::impl_runtime_outputs!(PowerFloatOutputs { value });

impl RuntimeNode for PowerFloatNode {
    type Inputs = PowerFloatInputs;
    type Outputs = PowerFloatOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let value = inputs.base.powf(inputs.exponent);
        if !value.is_finite() {
            return Ok(TypedNodeEvaluation {
                outputs: PowerFloatOutputs { value: 0.0 },
                frontend_updates: Vec::new(),
                diagnostics: vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("power_float_non_finite".to_owned()),
                    message: "Power Float produced a non-finite result.".to_owned(),
                }],
            });
        }

        Ok(TypedNodeEvaluation::from_outputs(PowerFloatOutputs {
            value,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::{PowerFloatInputs, PowerFloatNode};
    use crate::node_runtime::{NodeEvaluationContext, RuntimeNode};

    fn context() -> NodeEvaluationContext {
        NodeEvaluationContext {
            elapsed_seconds: 0.0,
            render_layout: None,
        }
    }

    #[test]
    fn raises_base_to_exponent() {
        let mut node = PowerFloatNode;
        let evaluation = node
            .evaluate(
                &context(),
                PowerFloatInputs {
                    base: 2.0,
                    exponent: 3.0,
                },
            )
            .expect("power float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, 8.0);
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn reports_non_finite_results() {
        let mut node = PowerFloatNode;
        let evaluation = node
            .evaluate(
                &context(),
                PowerFloatInputs {
                    base: f32::INFINITY,
                    exponent: 2.0,
                },
            )
            .expect("power float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, 0.0);
        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("power_float_non_finite")
        );
    }
}
