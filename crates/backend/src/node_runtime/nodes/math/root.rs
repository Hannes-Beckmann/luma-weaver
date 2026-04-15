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
pub(crate) struct RootNode;

impl RuntimeNodeFromParameters for RootNode {}

pub(crate) struct RootInputs {
    value: AnyInputValue,
    degree: AnyInputValue,
}

crate::node_runtime::impl_runtime_inputs!(RootInputs {
    value = AnyInputValue(InputValue::Float(0.0)),
    degree = AnyInputValue(InputValue::Float(2.0)),
});

pub(crate) struct RootOutputs {
    value: InputValue,
}

crate::node_runtime::impl_runtime_outputs!(RootOutputs { value });

impl RuntimeNode for RootNode {
    type Inputs = RootInputs;
    type Outputs = RootOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let output_shape = infer_float_tensor_target_shape(&[&inputs.value.0, &inputs.degree.0])?;
        if !input_value_is_finite(&inputs.value.0) || !input_value_is_finite(&inputs.degree.0) {
            return invalid_root(
                output_shape.as_deref(),
                "root_non_finite_input",
                "Root received a non-finite input.",
            );
        }
        if any_zero_degree(&inputs.degree.0)? {
            return invalid_root(
                output_shape.as_deref(),
                "root_zero_degree",
                "Root cannot use a degree of zero.",
            );
        }

        if any_invalid_negative_root(&inputs.value.0, &inputs.degree.0, output_shape.as_deref())? {
            return invalid_root(
                output_shape.as_deref(),
                "root_negative_value",
                "Root cannot evaluate a negative value for a non-odd degree.",
            );
        }

        let value = apply_binary_float_tensor_op(&inputs.value.0, &inputs.degree.0, root_value)?;

        if !input_value_is_finite(&value) {
            return invalid_root(
                output_shape.as_deref(),
                "root_non_finite",
                "Root produced a non-finite result.",
            );
        }

        Ok(TypedNodeEvaluation::from_outputs(RootOutputs { value }))
    }
}

fn root_value(value: f32, degree: f32) -> f32 {
    if value < 0.0 {
        let integer_degree = odd_integer_degree(degree).expect("validated odd degree");
        let magnitude = (-value).powf(1.0 / integer_degree as f32);
        -magnitude
    } else {
        value.powf(1.0 / degree)
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

fn any_zero_degree(degree: &InputValue) -> Result<bool> {
    match degree {
        InputValue::Float(value) => Ok(*value == 0.0),
        InputValue::FloatTensor(tensor) => {
            let tensor = coerce_float_tensor(degree, &tensor.shape)?;
            Ok(tensor.values.iter().any(|value| *value == 0.0))
        }
        _ => Ok(false),
    }
}

fn any_invalid_negative_root(
    value: &InputValue,
    degree: &InputValue,
    shape: Option<&[usize]>,
) -> Result<bool> {
    let Some(shape) = shape else {
        return Ok(match (value, degree) {
            (InputValue::Float(value), InputValue::Float(degree)) => {
                *value < 0.0 && odd_integer_degree(*degree).is_none()
            }
            _ => false,
        });
    };

    let values = coerce_float_tensor(value, shape)?;
    let degrees = coerce_float_tensor(degree, shape)?;
    Ok(values
        .values
        .iter()
        .zip(&degrees.values)
        .any(|(value, degree)| *value < 0.0 && odd_integer_degree(*degree).is_none()))
}

fn invalid_root(
    shape: Option<&[usize]>,
    code: &str,
    message: &str,
) -> Result<TypedNodeEvaluation<RootOutputs>> {
    Ok(TypedNodeEvaluation {
        outputs: RootOutputs {
            value: zero_like_float_output(shape),
        },
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
    use shared::{FloatTensor, InputValue};

    use super::{RootInputs, RootNode};
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
    fn computes_square_root() {
        let mut node = RootNode;
        let evaluation = node
            .evaluate(
                &context(),
                RootInputs {
                    value: AnyInputValue(InputValue::Float(9.0)),
                    degree: AnyInputValue(InputValue::Float(2.0)),
                },
            )
            .expect("root float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, InputValue::Float(3.0));
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn supports_negative_values_for_odd_integer_degrees() {
        let mut node = RootNode;
        let evaluation = node
            .evaluate(
                &context(),
                RootInputs {
                    value: AnyInputValue(InputValue::Float(-27.0)),
                    degree: AnyInputValue(InputValue::Float(3.0)),
                },
            )
            .expect("root float evaluation should succeed");

        match evaluation.outputs.value {
            InputValue::Float(value) => assert!((value + 3.0).abs() < 1e-5),
            other => panic!("expected float output, got {:?}", other),
        }
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn supports_float_tensors() {
        let mut node = RootNode;
        let evaluation = node
            .evaluate(
                &context(),
                RootInputs {
                    value: AnyInputValue(InputValue::FloatTensor(FloatTensor {
                        shape: vec![2],
                        values: vec![9.0, 27.0],
                    })),
                    degree: AnyInputValue(InputValue::FloatTensor(FloatTensor {
                        shape: vec![2],
                        values: vec![2.0, 3.0],
                    })),
                },
            )
            .expect("root float evaluation should succeed");

        assert_eq!(
            evaluation.outputs.value,
            InputValue::FloatTensor(FloatTensor {
                shape: vec![2],
                values: vec![3.0, 3.0],
            })
        );
    }

    #[test]
    fn rejects_negative_values_for_even_degrees() {
        let mut node = RootNode;
        let evaluation = node
            .evaluate(
                &context(),
                RootInputs {
                    value: AnyInputValue(InputValue::Float(-16.0)),
                    degree: AnyInputValue(InputValue::Float(2.0)),
                },
            )
            .expect("root float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, InputValue::Float(0.0));
        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("root_negative_value")
        );
    }

    #[test]
    fn rejects_zero_degree() {
        let mut node = RootNode;
        let evaluation = node
            .evaluate(
                &context(),
                RootInputs {
                    value: AnyInputValue(InputValue::Float(16.0)),
                    degree: AnyInputValue(InputValue::Float(0.0)),
                },
            )
            .expect("root float evaluation should succeed");

        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("root_zero_degree")
        );
    }
}
