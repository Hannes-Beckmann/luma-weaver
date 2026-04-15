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
pub(crate) struct LogNode;

impl RuntimeNodeFromParameters for LogNode {}

pub(crate) struct LogInputs {
    value: AnyInputValue,
    base: AnyInputValue,
}

crate::node_runtime::impl_runtime_inputs!(LogInputs {
    value = AnyInputValue(InputValue::Float(1.0)),
    base = AnyInputValue(InputValue::Float(std::f32::consts::E)),
});

pub(crate) struct LogOutputs {
    value: InputValue,
}

crate::node_runtime::impl_runtime_outputs!(LogOutputs { value });

impl RuntimeNode for LogNode {
    type Inputs = LogInputs;
    type Outputs = LogOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let output_shape = infer_float_tensor_target_shape(&[&inputs.value.0, &inputs.base.0])?;
        if !input_value_is_finite(&inputs.value.0) || !input_value_is_finite(&inputs.base.0) {
            return invalid_log(
                output_shape.as_deref(),
                "log_non_finite_input",
                "Log received a non-finite input.",
            );
        }
        if any_value_leq_zero(&inputs.value.0)? {
            return invalid_log(
                output_shape.as_deref(),
                "log_invalid_value",
                "Log requires a value greater than zero.",
            );
        }
        if any_invalid_base(&inputs.base.0)? {
            return invalid_log(
                output_shape.as_deref(),
                "log_invalid_base",
                "Log requires a positive base other than one.",
            );
        }

        let value =
            apply_binary_float_tensor_op(&inputs.value.0, &inputs.base.0, |value, base| {
                value.ln() / base.ln()
            })?;
        if !input_value_is_finite(&value) {
            return invalid_log(
                output_shape.as_deref(),
                "log_non_finite",
                "Log produced a non-finite result.",
            );
        }

        Ok(TypedNodeEvaluation::from_outputs(LogOutputs { value }))
    }
}

fn any_value_leq_zero(value: &InputValue) -> Result<bool> {
    match value {
        InputValue::Float(value) => Ok(*value <= 0.0),
        InputValue::FloatTensor(tensor) => {
            let tensor = coerce_float_tensor(value, &tensor.shape)?;
            Ok(tensor.values.iter().any(|value| *value <= 0.0))
        }
        _ => Ok(false),
    }
}

fn any_invalid_base(value: &InputValue) -> Result<bool> {
    match value {
        InputValue::Float(value) => Ok(*value <= 0.0 || *value == 1.0),
        InputValue::FloatTensor(tensor) => {
            let tensor = coerce_float_tensor(value, &tensor.shape)?;
            Ok(tensor
                .values
                .iter()
                .any(|value| *value <= 0.0 || *value == 1.0))
        }
        _ => Ok(false),
    }
}

fn invalid_log(
    shape: Option<&[usize]>,
    code: &str,
    message: &str,
) -> Result<TypedNodeEvaluation<LogOutputs>> {
    Ok(TypedNodeEvaluation {
        outputs: LogOutputs {
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

    use super::{LogInputs, LogNode};
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
    fn computes_logarithms_for_valid_inputs() {
        let mut node = LogNode;
        let evaluation = node
            .evaluate(
                &context(),
                LogInputs {
                    value: AnyInputValue(InputValue::Float(100.0)),
                    base: AnyInputValue(InputValue::Float(10.0)),
                },
            )
            .expect("log float evaluation should succeed");

        match evaluation.outputs.value {
            InputValue::Float(value) => assert!((value - 2.0).abs() < 1e-5),
            other => panic!("expected float output, got {:?}", other),
        }
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn rejects_non_positive_values() {
        let mut node = LogNode;
        let evaluation = node
            .evaluate(
                &context(),
                LogInputs {
                    value: AnyInputValue(InputValue::Float(0.0)),
                    base: AnyInputValue(InputValue::Float(10.0)),
                },
            )
            .expect("log float evaluation should succeed");

        assert_eq!(evaluation.outputs.value, InputValue::Float(0.0));
        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("log_invalid_value")
        );
    }

    #[test]
    fn supports_float_tensors() {
        let mut node = LogNode;
        let evaluation = node
            .evaluate(
                &context(),
                LogInputs {
                    value: AnyInputValue(InputValue::FloatTensor(FloatTensor {
                        shape: vec![2],
                        values: vec![1.0, 100.0],
                    })),
                    base: AnyInputValue(InputValue::Float(10.0)),
                },
            )
            .expect("log float evaluation should succeed");

        assert_eq!(
            evaluation.outputs.value,
            InputValue::FloatTensor(FloatTensor {
                shape: vec![2],
                values: vec![0.0, 2.0],
            })
        );
    }

    #[test]
    fn rejects_invalid_bases() {
        let mut node = LogNode;
        let evaluation = node
            .evaluate(
                &context(),
                LogInputs {
                    value: AnyInputValue(InputValue::Float(10.0)),
                    base: AnyInputValue(InputValue::Float(1.0)),
                },
            )
            .expect("log float evaluation should succeed");

        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("log_invalid_base")
        );
    }
}
