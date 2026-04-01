use anyhow::Result;
use shared::InputValue;

use crate::node_runtime::{
    NodeEvaluationContext, NodeFrontendUpdate, RuntimeNode, RuntimeNodeFromParameters,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct PlotNode;

impl RuntimeNodeFromParameters for PlotNode {}

pub(crate) struct PlotInputs {
    value: f32,
}

crate::node_runtime::impl_runtime_inputs!(PlotInputs {
    value = 0.0,
});

impl RuntimeNode for PlotNode {
    type Inputs = PlotInputs;
    type Outputs = ();

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        Ok(TypedNodeEvaluation::with_frontend_updates(
            (),
            vec![NodeFrontendUpdate {
                name: "value".to_owned(),
                value: InputValue::Float(inputs.value),
            }],
        ))
    }
}
