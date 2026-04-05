use anyhow::Result;
use shared::{NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct ClampFloatNode;

impl RuntimeNodeFromParameters for ClampFloatNode {}

pub(crate) struct ClampFloatInputs {
    value: f32,
    min: f32,
    max: f32,
}

crate::node_runtime::impl_runtime_inputs!(ClampFloatInputs {
    value = 0.0,
    min = 0.0,
    max = 1.0,
});

pub(crate) struct ClampFloatOutputs {
    value: f32,
}

crate::node_runtime::impl_runtime_outputs!(ClampFloatOutputs { value });

impl RuntimeNode for ClampFloatNode {
    type Inputs = ClampFloatInputs;
    type Outputs = ClampFloatOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        if !inputs.value.is_finite() || !inputs.min.is_finite() || !inputs.max.is_finite() {
            return Ok(TypedNodeEvaluation {
                outputs: ClampFloatOutputs { value: 0.0 },
                frontend_updates: Vec::new(),
                diagnostics: vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("clamp_float_non_finite_input".to_owned()),
                    message: "Clamp Float received a non-finite input.".to_owned(),
                }],
            });
        }

        let mut diagnostics = Vec::new();
        let (min, max) = if inputs.min <= inputs.max {
            (inputs.min, inputs.max)
        } else {
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Warning,
                code: Some("clamp_float_bounds_swapped".to_owned()),
                message: "Clamp Float received min greater than max and swapped the bounds."
                    .to_owned(),
            });
            (inputs.max, inputs.min)
        };

        let value = inputs.value.clamp(min, max);

        Ok(TypedNodeEvaluation {
            outputs: ClampFloatOutputs { value },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{ClampFloatInputs, ClampFloatNode};
    use crate::node_runtime::{NodeEvaluationContext, RuntimeNode};

    fn context() -> NodeEvaluationContext {
        NodeEvaluationContext {
            elapsed_seconds: 0.0,
            render_layout: None,
        }
    }

    #[test]
    fn clamps_into_range() {
        let mut node = ClampFloatNode;

        let evaluation = node
            .evaluate(
                &context(),
                ClampFloatInputs {
                    value: 5.0,
                    min: 1.0,
                    max: 3.0,
                },
            )
            .expect("clamp float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, 3.0);
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn swaps_reversed_bounds_with_warning() {
        let mut node = ClampFloatNode;

        let evaluation = node
            .evaluate(
                &context(),
                ClampFloatInputs {
                    value: 2.0,
                    min: 5.0,
                    max: 1.0,
                },
            )
            .expect("clamp float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, 2.0);
        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("clamp_float_bounds_swapped")
        );
    }

    #[test]
    fn reports_non_finite_results() {
        let mut node = ClampFloatNode;

        let evaluation = node
            .evaluate(
                &context(),
                ClampFloatInputs {
                    value: f32::INFINITY,
                    min: 0.0,
                    max: 1.0,
                },
            )
            .expect("clamp float evaluation should succeed");

        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("clamp_float_non_finite_input")
        );
    }
}
