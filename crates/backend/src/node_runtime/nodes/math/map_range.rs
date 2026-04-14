use anyhow::Result;
use shared::{InputValue, NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::tensor::{
    apply_binary_float_tensor_op, coerce_float_tensor, infer_float_tensor_target_shape,
    input_value_is_finite, zero_like_float_output,
};
use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct MapRangeNode;

impl RuntimeNodeFromParameters for MapRangeNode {}

pub(crate) struct MapRangeInputs {
    value: AnyInputValue,
    source_min: AnyInputValue,
    source_max: AnyInputValue,
    target_min: AnyInputValue,
    target_max: AnyInputValue,
}

crate::node_runtime::impl_runtime_inputs!(MapRangeInputs {
    value = AnyInputValue(InputValue::Float(0.0)),
    source_min = AnyInputValue(InputValue::Float(0.0)),
    source_max = AnyInputValue(InputValue::Float(1.0)),
    target_min = AnyInputValue(InputValue::Float(0.0)),
    target_max = AnyInputValue(InputValue::Float(1.0)),
});

pub(crate) struct MapRangeOutputs {
    value: InputValue,
}

crate::node_runtime::impl_runtime_outputs!(MapRangeOutputs { value });

impl RuntimeNode for MapRangeNode {
    type Inputs = MapRangeInputs;
    type Outputs = MapRangeOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let output_shape = infer_float_tensor_target_shape(&[
            &inputs.value.0,
            &inputs.source_min.0,
            &inputs.source_max.0,
            &inputs.target_min.0,
            &inputs.target_max.0,
        ])?;
        if !input_value_is_finite(&inputs.value.0)
            || !input_value_is_finite(&inputs.source_min.0)
            || !input_value_is_finite(&inputs.source_max.0)
            || !input_value_is_finite(&inputs.target_min.0)
            || !input_value_is_finite(&inputs.target_max.0)
        {
            return Ok(TypedNodeEvaluation {
                outputs: MapRangeOutputs {
                    value: zero_like_float_output(output_shape.as_deref()),
                },
                frontend_updates: Vec::new(),
                diagnostics: vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("map_range_non_finite_input".to_owned()),
                    message: "Map Range received a non-finite input.".to_owned(),
                }],
            });
        }

        let source_width =
            apply_binary_float_tensor_op(&inputs.source_max.0, &inputs.source_min.0, |a, b| a - b)?;
        if has_zero_width(&source_width) {
            return Ok(TypedNodeEvaluation {
                outputs: MapRangeOutputs {
                    value: inputs.target_min.0,
                },
                frontend_updates: Vec::new(),
                diagnostics: vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("map_range_zero_source_width".to_owned()),
                    message: "Map Range requires a non-zero source range width.".to_owned(),
                }],
            });
        }

        let value_delta =
            apply_binary_float_tensor_op(&inputs.value.0, &inputs.source_min.0, |a, b| a - b)?;
        let t = apply_binary_float_tensor_op(&value_delta, &source_width, |a, b| a / b)?;
        let target_width =
            apply_binary_float_tensor_op(&inputs.target_max.0, &inputs.target_min.0, |a, b| a - b)?;
        let scaled = apply_binary_float_tensor_op(&t, &target_width, |a, b| a * b)?;
        let value = apply_binary_float_tensor_op(&inputs.target_min.0, &scaled, |a, b| a + b)?;
        if !input_value_is_finite(&value) {
            return Ok(TypedNodeEvaluation {
                outputs: MapRangeOutputs {
                    value: zero_like_float_output(output_shape.as_deref()),
                },
                frontend_updates: Vec::new(),
                diagnostics: vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("map_range_non_finite".to_owned()),
                    message: "Map Range produced a non-finite result.".to_owned(),
                }],
            });
        }

        Ok(TypedNodeEvaluation::from_outputs(MapRangeOutputs { value }))
    }
}

fn has_zero_width(value: &InputValue) -> bool {
    match value {
        InputValue::Float(value) => *value == 0.0,
        InputValue::FloatTensor(tensor) => {
            let tensor = coerce_float_tensor(value, &tensor.shape).expect("normalized tensor");
            tensor.values.iter().any(|value| *value == 0.0)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use shared::{FloatTensor, InputValue};

    use super::{MapRangeInputs, MapRangeNode};
    use crate::node_runtime::{AnyInputValue, NodeEvaluationContext, RuntimeNode};

    fn context() -> NodeEvaluationContext {
        NodeEvaluationContext {
            graph_id: "test-graph".to_owned(),
            graph_name: "Test Graph".to_owned(),
            elapsed_seconds: 0.0,
            render_layout: None,
        }
    }

    #[test]
    fn remaps_linearly_between_ranges() {
        let mut node = MapRangeNode;
        let evaluation = node
            .evaluate(
                &context(),
                MapRangeInputs {
                    value: AnyInputValue(InputValue::Float(0.5)),
                    source_min: AnyInputValue(InputValue::Float(0.0)),
                    source_max: AnyInputValue(InputValue::Float(1.0)),
                    target_min: AnyInputValue(InputValue::Float(10.0)),
                    target_max: AnyInputValue(InputValue::Float(20.0)),
                },
            )
            .expect("map range float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, InputValue::Float(15.0));
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn supports_float_tensors() {
        let mut node = MapRangeNode;
        let evaluation = node
            .evaluate(
                &context(),
                MapRangeInputs {
                    value: AnyInputValue(InputValue::FloatTensor(FloatTensor {
                        shape: vec![2],
                        values: vec![0.0, 0.5],
                    })),
                    source_min: AnyInputValue(InputValue::Float(0.0)),
                    source_max: AnyInputValue(InputValue::Float(1.0)),
                    target_min: AnyInputValue(InputValue::Float(10.0)),
                    target_max: AnyInputValue(InputValue::Float(20.0)),
                },
            )
            .expect("map range float evaluation should succeed");

        assert_eq!(
            evaluation.outputs.value,
            InputValue::FloatTensor(FloatTensor {
                shape: vec![2],
                values: vec![10.0, 15.0],
            })
        );
    }

    #[test]
    fn rejects_zero_width_source_range() {
        let mut node = MapRangeNode;
        let evaluation = node
            .evaluate(
                &context(),
                MapRangeInputs {
                    value: AnyInputValue(InputValue::Float(0.5)),
                    source_min: AnyInputValue(InputValue::Float(1.0)),
                    source_max: AnyInputValue(InputValue::Float(1.0)),
                    target_min: AnyInputValue(InputValue::Float(10.0)),
                    target_max: AnyInputValue(InputValue::Float(20.0)),
                },
            )
            .expect("map range float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, InputValue::Float(10.0));
        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("map_range_zero_source_width")
        );
    }
}
