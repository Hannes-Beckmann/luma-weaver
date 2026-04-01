use anyhow::Result;

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct AddFloatNode;

impl RuntimeNodeFromParameters for AddFloatNode {}

pub(crate) struct AddFloatInputs {
    a: f32,
    b: f32,
}

crate::node_runtime::impl_runtime_inputs!(AddFloatInputs {
    a = 0.0,
    b = 0.0,
});

pub(crate) struct AddFloatOutputs {
    sum: f32,
}

crate::node_runtime::impl_runtime_outputs!(AddFloatOutputs { sum });

impl RuntimeNode for AddFloatNode {
    type Inputs = AddFloatInputs;
    type Outputs = AddFloatOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        Ok(TypedNodeEvaluation::from_outputs(AddFloatOutputs {
            sum: inputs.a + inputs.b,
        }))
    }
}
