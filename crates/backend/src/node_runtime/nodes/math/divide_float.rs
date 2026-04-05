use anyhow::Result;
use shared::{NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct DivideFloatNode;

impl RuntimeNodeFromParameters for DivideFloatNode {}

pub(crate) struct DivideFloatInputs {
    a: f32,
    b: f32,
}

crate::node_runtime::impl_runtime_inputs!(DivideFloatInputs {
    a = 0.0,
    b = 1.0,
});

pub(crate) struct DivideFloatOutputs {
    quotient: f32,
}

crate::node_runtime::impl_runtime_outputs!(DivideFloatOutputs { quotient });

impl RuntimeNode for DivideFloatNode {
    type Inputs = DivideFloatInputs;
    type Outputs = DivideFloatOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        if inputs.b == 0.0 {
            return Ok(TypedNodeEvaluation {
                outputs: DivideFloatOutputs { quotient: 0.0 },
                frontend_updates: Vec::new(),
                diagnostics: vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("divide_float_division_by_zero".to_owned()),
                    message: "Divide Float cannot divide by zero.".to_owned(),
                }],
            });
        }

        let quotient = inputs.a / inputs.b;
        let diagnostics = if quotient.is_finite() {
            Vec::new()
        } else {
            vec![NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Error,
                code: Some("divide_float_non_finite".to_owned()),
                message: "Divide Float produced a non-finite result.".to_owned(),
            }]
        };

        Ok(TypedNodeEvaluation {
            outputs: DivideFloatOutputs { quotient },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{DivideFloatInputs, DivideFloatNode};
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
    fn divides_inputs() {
        let mut node = DivideFloatNode;

        let evaluation = node
            .evaluate(&context(), DivideFloatInputs { a: 9.0, b: 3.0 })
            .expect("divide float evaluation should succeed");

        assert_eq!(evaluation.outputs.quotient, 3.0);
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn reports_division_by_zero_with_safe_fallback() {
        let mut node = DivideFloatNode;

        let evaluation = node
            .evaluate(&context(), DivideFloatInputs { a: 9.0, b: 0.0 })
            .expect("divide float evaluation should succeed");

        assert_eq!(evaluation.outputs.quotient, 0.0);
        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("divide_float_division_by_zero")
        );
    }

    #[test]
    fn reports_non_finite_results() {
        let mut node = DivideFloatNode;

        let evaluation = node
            .evaluate(
                &context(),
                DivideFloatInputs {
                    a: f32::INFINITY,
                    b: 2.0,
                },
            )
            .expect("divide float evaluation should succeed");

        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("divide_float_non_finite")
        );
    }
}
