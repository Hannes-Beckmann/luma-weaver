use std::collections::HashMap;

use ::shared::{ColorFrame, FloatTensor, InputValue, LedLayout, RgbaColor};
use anyhow::Result;

use super::{
    AnyInputValue, NodeEvaluation, NodeEvaluationContext, RuntimeInputs, RuntimeNode,
    RuntimeNodeEvaluator, RuntimeOutputs,
};

/// Converts a generic runtime input value into a concrete Rust input type.
pub(crate) trait FromInputValue: Sized {
    /// Attempts to extract the expected typed value from a generic `InputValue`.
    fn from_input_value(value: &InputValue) -> Result<Self>;
}

/// Converts a concrete Rust output type into a generic runtime output value.
pub(crate) trait IntoInputValue {
    /// Serializes the value into the generic `InputValue` representation.
    fn into_input_value(self) -> Result<InputValue>;
}

/// Adapts nodes that implement `RuntimeInputs` and `RuntimeOutputs` without serde-based conversion.
pub(crate) struct FastNodeEvaluator<T>(pub(crate) T);

impl<T> RuntimeNodeEvaluator for FastNodeEvaluator<T>
where
    T: RuntimeNode,
    T::Inputs: RuntimeInputs,
    T::Outputs: RuntimeOutputs,
{
    /// Converts generic inputs with the fast typed conversion traits and evaluates the node.
    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        inputs: &HashMap<String, InputValue>,
    ) -> Result<NodeEvaluation> {
        let typed_inputs = T::Inputs::from_runtime_inputs(inputs)?;
        let evaluation = RuntimeNode::evaluate(&mut self.0, context, typed_inputs)?;
        Ok(NodeEvaluation {
            outputs: evaluation.outputs.into_runtime_outputs()?,
            frontend_updates: evaluation.frontend_updates,
            diagnostics: evaluation.diagnostics,
        })
    }
}

impl FromInputValue for f32 {
    /// Extracts a scalar float input.
    fn from_input_value(value: &InputValue) -> Result<Self> {
        match value {
            InputValue::Float(value) => Ok(*value),
            _ => anyhow::bail!("expected Float input"),
        }
    }
}

impl FromInputValue for String {
    /// Extracts a string input.
    fn from_input_value(value: &InputValue) -> Result<Self> {
        match value {
            InputValue::String(value) => Ok(value.clone()),
            _ => anyhow::bail!("expected String input"),
        }
    }
}

impl FromInputValue for AnyInputValue {
    /// Preserves the original input value without type narrowing.
    fn from_input_value(value: &InputValue) -> Result<Self> {
        Ok(AnyInputValue(value.clone()))
    }
}

impl<T> FromInputValue for Option<T>
where
    T: FromInputValue,
{
    /// Wraps a successfully converted value in `Some`.
    fn from_input_value(value: &InputValue) -> Result<Self> {
        Ok(Some(T::from_input_value(value)?))
    }
}

impl FromInputValue for InputValue {
    /// Clones the generic input value directly.
    fn from_input_value(value: &InputValue) -> Result<Self> {
        Ok(value.clone())
    }
}

impl FromInputValue for ColorFrame {
    /// Extracts a color-frame input.
    fn from_input_value(value: &InputValue) -> Result<Self> {
        match value {
            InputValue::ColorFrame(value) => Ok(value.clone()),
            _ => anyhow::bail!("expected ColorFrame input"),
        }
    }
}

impl FromInputValue for FloatTensor {
    /// Extracts a float-tensor input.
    fn from_input_value(value: &InputValue) -> Result<Self> {
        match value {
            InputValue::FloatTensor(value) => Ok(value.clone()),
            _ => anyhow::bail!("expected FloatTensor input"),
        }
    }
}

impl FromInputValue for RgbaColor {
    /// Extracts a scalar color input.
    fn from_input_value(value: &InputValue) -> Result<Self> {
        match value {
            InputValue::Color(value) => Ok(*value),
            _ => anyhow::bail!("expected Color input"),
        }
    }
}

impl FromInputValue for LedLayout {
    /// Extracts a LED layout input.
    fn from_input_value(value: &InputValue) -> Result<Self> {
        match value {
            InputValue::LedLayout(value) => Ok(value.clone()),
            _ => anyhow::bail!("expected LedLayout input"),
        }
    }
}

impl IntoInputValue for f32 {
    /// Serializes a scalar float output.
    fn into_input_value(self) -> Result<InputValue> {
        Ok(InputValue::Float(self))
    }
}

impl IntoInputValue for String {
    /// Serializes a string output.
    fn into_input_value(self) -> Result<InputValue> {
        Ok(InputValue::String(self))
    }
}

impl IntoInputValue for InputValue {
    /// Reuses an already generic output value.
    fn into_input_value(self) -> Result<InputValue> {
        Ok(self)
    }
}

impl IntoInputValue for ColorFrame {
    /// Serializes a color-frame output.
    fn into_input_value(self) -> Result<InputValue> {
        Ok(InputValue::ColorFrame(self))
    }
}

impl IntoInputValue for FloatTensor {
    /// Serializes a float-tensor output.
    fn into_input_value(self) -> Result<InputValue> {
        Ok(InputValue::FloatTensor(self))
    }
}

impl IntoInputValue for RgbaColor {
    /// Serializes a scalar color output.
    fn into_input_value(self) -> Result<InputValue> {
        Ok(InputValue::Color(self))
    }
}

impl IntoInputValue for LedLayout {
    /// Serializes a LED layout output.
    fn into_input_value(self) -> Result<InputValue> {
        Ok(InputValue::LedLayout(self))
    }
}

impl<T> IntoInputValue for Option<T>
where
    T: IntoInputValue,
{
    /// Serializes the contained value, rejecting `None` because runtime outputs must be concrete.
    fn into_input_value(self) -> Result<InputValue> {
        match self {
            Some(value) => value.into_input_value(),
            None => anyhow::bail!("cannot serialize missing optional output"),
        }
    }
}
