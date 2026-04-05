use anyhow::Result;
use shared::{NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct LogFloatNode;

impl RuntimeNodeFromParameters for LogFloatNode {}

pub(crate) struct LogFloatInputs {
    value: f32,
    base: f32,
}

crate::node_runtime::impl_runtime_inputs!(LogFloatInputs {
    value = 1.0,
    base = std::f32::consts::E,
});

pub(crate) struct LogFloatOutputs {
    value: f32,
}

crate::node_runtime::impl_runtime_outputs!(LogFloatOutputs { value });

impl RuntimeNode for LogFloatNode {
    type Inputs = LogFloatInputs;
    type Outputs = LogFloatOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        if !inputs.value.is_finite() || !inputs.base.is_finite() {
            return invalid_log(
                "log_float_non_finite_input",
                "Log Float received a non-finite input.",
            );
        }
        if inputs.value <= 0.0 {
            return invalid_log(
                "log_float_invalid_value",
                "Log Float requires a value greater than zero.",
            );
        }
        if inputs.base <= 0.0 || inputs.base == 1.0 {
            return invalid_log(
                "log_float_invalid_base",
                "Log Float requires a positive base other than one.",
            );
        }

        let value = inputs.value.ln() / inputs.base.ln();
        if !value.is_finite() {
            return invalid_log(
                "log_float_non_finite",
                "Log Float produced a non-finite result.",
            );
        }

        Ok(TypedNodeEvaluation::from_outputs(LogFloatOutputs { value }))
    }
}

fn invalid_log(code: &str, message: &str) -> Result<TypedNodeEvaluation<LogFloatOutputs>> {
    Ok(TypedNodeEvaluation {
        outputs: LogFloatOutputs { value: 0.0 },
        frontend_updates: Vec::new(),
        diagnostics: vec![NodeDiagnostic {
            severity: NodeDiagnosticSeverity::Error,
            code: Some(code.to_owned()),
            message: message.to_owned(),
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::{LogFloatInputs, LogFloatNode};
    use crate::node_runtime::{NodeEvaluationContext, RuntimeNode};

    fn context() -> NodeEvaluationContext {
        NodeEvaluationContext {
            elapsed_seconds: 0.0,
            render_layout: None,
        }
    }

    #[test]
    fn computes_logarithms_for_valid_inputs() {
        let mut node = LogFloatNode;
        let evaluation = node
            .evaluate(
                &context(),
                LogFloatInputs {
                    value: 100.0,
                    base: 10.0,
                },
            )
            .expect("log float evaluation should succeed");

        assert!((evaluation.outputs.value - 2.0).abs() < 1e-5);
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn rejects_non_positive_values() {
        let mut node = LogFloatNode;
        let evaluation = node
            .evaluate(
                &context(),
                LogFloatInputs {
                    value: 0.0,
                    base: 10.0,
                },
            )
            .expect("log float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, 0.0);
        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("log_float_invalid_value")
        );
    }

    #[test]
    fn rejects_invalid_bases() {
        let mut node = LogFloatNode;
        let evaluation = node
            .evaluate(
                &context(),
                LogFloatInputs {
                    value: 10.0,
                    base: 1.0,
                },
            )
            .expect("log float evaluation should succeed");

        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("log_float_invalid_base")
        );
    }
}
