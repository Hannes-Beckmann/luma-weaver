use anyhow::Result;
use shared::{NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct RootFloatNode;

impl RuntimeNodeFromParameters for RootFloatNode {}

pub(crate) struct RootFloatInputs {
    value: f32,
    degree: f32,
}

crate::node_runtime::impl_runtime_inputs!(RootFloatInputs {
    value = 0.0,
    degree = 2.0,
});

pub(crate) struct RootFloatOutputs {
    value: f32,
}

crate::node_runtime::impl_runtime_outputs!(RootFloatOutputs { value });

impl RuntimeNode for RootFloatNode {
    type Inputs = RootFloatInputs;
    type Outputs = RootFloatOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        if !inputs.value.is_finite() || !inputs.degree.is_finite() {
            return invalid_root(
                "root_float_non_finite_input",
                "Root Float received a non-finite input.",
            );
        }
        if inputs.degree == 0.0 {
            return invalid_root(
                "root_float_zero_degree",
                "Root Float cannot use a degree of zero.",
            );
        }

        let value = if inputs.value < 0.0 {
            if let Some(integer_degree) = odd_integer_degree(inputs.degree) {
                let magnitude = (-inputs.value).powf(1.0 / integer_degree as f32);
                -magnitude
            } else {
                return invalid_root(
                    "root_float_negative_value",
                    "Root Float cannot evaluate a negative value for a non-odd degree.",
                );
            }
        } else {
            inputs.value.powf(1.0 / inputs.degree)
        };

        if !value.is_finite() {
            return invalid_root(
                "root_float_non_finite",
                "Root Float produced a non-finite result.",
            );
        }

        Ok(TypedNodeEvaluation::from_outputs(RootFloatOutputs {
            value,
        }))
    }
}

fn odd_integer_degree(degree: f32) -> Option<i32> {
    let rounded = degree.round();
    if (degree - rounded).abs() > 1e-6 {
        return None;
    }
    let integer = rounded as i32;
    if integer == 0 || integer % 2 == 0 {
        None
    } else {
        Some(integer)
    }
}

fn invalid_root(code: &str, message: &str) -> Result<TypedNodeEvaluation<RootFloatOutputs>> {
    Ok(TypedNodeEvaluation {
        outputs: RootFloatOutputs { value: 0.0 },
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
    use super::{RootFloatInputs, RootFloatNode};
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
    fn computes_square_root() {
        let mut node = RootFloatNode;
        let evaluation = node
            .evaluate(
                &context(),
                RootFloatInputs {
                    value: 9.0,
                    degree: 2.0,
                },
            )
            .expect("root float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, 3.0);
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn supports_negative_values_for_odd_integer_degrees() {
        let mut node = RootFloatNode;
        let evaluation = node
            .evaluate(
                &context(),
                RootFloatInputs {
                    value: -27.0,
                    degree: 3.0,
                },
            )
            .expect("root float evaluation should succeed");

        assert!((evaluation.outputs.value + 3.0).abs() < 1e-5);
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn rejects_negative_values_for_even_degrees() {
        let mut node = RootFloatNode;
        let evaluation = node
            .evaluate(
                &context(),
                RootFloatInputs {
                    value: -16.0,
                    degree: 2.0,
                },
            )
            .expect("root float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, 0.0);
        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("root_float_negative_value")
        );
    }

    #[test]
    fn rejects_zero_degree() {
        let mut node = RootFloatNode;
        let evaluation = node
            .evaluate(
                &context(),
                RootFloatInputs {
                    value: 16.0,
                    degree: 0.0,
                },
            )
            .expect("root float evaluation should succeed");

        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("root_float_zero_degree")
        );
    }
}
