use anyhow::Result;
use shared::{NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct MapRangeFloatNode;

impl RuntimeNodeFromParameters for MapRangeFloatNode {}

pub(crate) struct MapRangeFloatInputs {
    value: f32,
    source_min: f32,
    source_max: f32,
    target_min: f32,
    target_max: f32,
}

crate::node_runtime::impl_runtime_inputs!(MapRangeFloatInputs {
    value = 0.0,
    source_min = 0.0,
    source_max = 1.0,
    target_min = 0.0,
    target_max = 1.0,
});

pub(crate) struct MapRangeFloatOutputs {
    value: f32,
}

crate::node_runtime::impl_runtime_outputs!(MapRangeFloatOutputs { value });

impl RuntimeNode for MapRangeFloatNode {
    type Inputs = MapRangeFloatInputs;
    type Outputs = MapRangeFloatOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        if !inputs.value.is_finite()
            || !inputs.source_min.is_finite()
            || !inputs.source_max.is_finite()
            || !inputs.target_min.is_finite()
            || !inputs.target_max.is_finite()
        {
            return Ok(TypedNodeEvaluation {
                outputs: MapRangeFloatOutputs { value: 0.0 },
                frontend_updates: Vec::new(),
                diagnostics: vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("map_range_float_non_finite_input".to_owned()),
                    message: "Map Range Float received a non-finite input.".to_owned(),
                }],
            });
        }

        let source_width = inputs.source_max - inputs.source_min;
        if source_width == 0.0 {
            return Ok(TypedNodeEvaluation {
                outputs: MapRangeFloatOutputs {
                    value: inputs.target_min,
                },
                frontend_updates: Vec::new(),
                diagnostics: vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("map_range_float_zero_source_width".to_owned()),
                    message: "Map Range Float requires a non-zero source range width.".to_owned(),
                }],
            });
        }

        let t = (inputs.value - inputs.source_min) / source_width;
        let value = inputs.target_min + t * (inputs.target_max - inputs.target_min);
        if !value.is_finite() {
            return Ok(TypedNodeEvaluation {
                outputs: MapRangeFloatOutputs { value: 0.0 },
                frontend_updates: Vec::new(),
                diagnostics: vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("map_range_float_non_finite".to_owned()),
                    message: "Map Range Float produced a non-finite result.".to_owned(),
                }],
            });
        }

        Ok(TypedNodeEvaluation::from_outputs(MapRangeFloatOutputs {
            value,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::{MapRangeFloatInputs, MapRangeFloatNode};
    use crate::node_runtime::{NodeEvaluationContext, RuntimeNode};

    fn context() -> NodeEvaluationContext {
        NodeEvaluationContext {
            elapsed_seconds: 0.0,
            render_layout: None,
        }
    }

    #[test]
    fn remaps_linearly_between_ranges() {
        let mut node = MapRangeFloatNode;
        let evaluation = node
            .evaluate(
                &context(),
                MapRangeFloatInputs {
                    value: 0.5,
                    source_min: 0.0,
                    source_max: 1.0,
                    target_min: 10.0,
                    target_max: 20.0,
                },
            )
            .expect("map range float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, 15.0);
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn rejects_zero_width_source_range() {
        let mut node = MapRangeFloatNode;
        let evaluation = node
            .evaluate(
                &context(),
                MapRangeFloatInputs {
                    value: 0.5,
                    source_min: 1.0,
                    source_max: 1.0,
                    target_min: 10.0,
                    target_max: 20.0,
                },
            )
            .expect("map range float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, 10.0);
        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("map_range_float_zero_source_width")
        );
    }
}
