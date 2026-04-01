use std::collections::HashMap;

use ::shared::InputValue;
use anyhow::Result;
use serde::{Serialize, de::DeserializeOwned};

use super::{NodeEvaluation, NodeEvaluationContext, TypedNodeEvaluation, deserialize_inputs};

/// Erases the concrete input and output types of a runtime node so the executor can call it uniformly.
pub(crate) trait RuntimeNodeEvaluator: Send {
    /// Evaluates the node using generic named inputs and returns generic named outputs.
    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        inputs: &HashMap<String, InputValue>,
    ) -> Result<NodeEvaluation>;
}

/// Converts generic runtime inputs into a node-specific typed input struct.
pub(crate) trait RuntimeInputs: Sized {
    /// Builds the typed input value expected by a node from named runtime inputs.
    fn from_runtime_inputs(inputs: &HashMap<String, InputValue>) -> Result<Self>;
}

/// Converts a node-specific typed output value into generic named runtime outputs.
pub(crate) trait RuntimeOutputs {
    /// Serializes the typed output into the executor's generic output map.
    fn into_runtime_outputs(self) -> Result<HashMap<String, InputValue>>;
}

/// Core trait implemented by concrete runtime nodes.
///
/// Nodes operate on typed inputs and outputs, while the executor handles conversion to and from the
/// generic runtime representation.
pub(crate) trait RuntimeNode: Send {
    type Inputs;
    type Outputs;

    /// Evaluates the node for the current tick and render context.
    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>>;
}

impl<T> RuntimeNodeEvaluator for T
where
    T: RuntimeNode,
    T::Inputs: DeserializeOwned,
    T::Outputs: Serialize,
{
    /// Deserializes generic inputs, evaluates the typed node, and serializes the result again.
    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        inputs: &HashMap<String, InputValue>,
    ) -> Result<NodeEvaluation> {
        let typed_inputs = deserialize_inputs::<T::Inputs>(inputs)?;
        Ok(RuntimeNode::evaluate(self, context, typed_inputs)?.into())
    }
}

impl<T> RuntimeInputs for T
where
    T: DeserializeOwned,
{
    /// Deserializes a typed input value from the generic runtime input map.
    fn from_runtime_inputs(inputs: &HashMap<String, InputValue>) -> Result<Self> {
        deserialize_inputs(inputs)
    }
}

impl<T> RuntimeOutputs for T
where
    T: Serialize,
{
    /// Serializes a typed output value into the generic runtime output map.
    fn into_runtime_outputs(self) -> Result<HashMap<String, InputValue>> {
        super::serialize_outputs(self)
    }
}
