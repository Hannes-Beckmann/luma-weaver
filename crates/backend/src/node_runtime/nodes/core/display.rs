use anyhow::Result;
use shared::InputValue;

use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, NodeFrontendUpdate, RuntimeNode,
    RuntimeNodeFromParameters, TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct DisplayNode;

impl RuntimeNodeFromParameters for DisplayNode {}

pub(crate) struct DisplayInputs {
    value: AnyInputValue,
}

crate::node_runtime::impl_runtime_inputs!(DisplayInputs {
    value = default_display_value(),
});

impl RuntimeNode for DisplayNode {
    type Inputs = DisplayInputs;
    type Outputs = ();

    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let value_name = context
            .render_layout
            .as_ref()
            .map(|layout| format!("value ({})", layout.id))
            .unwrap_or_else(|| "value".to_owned());
        Ok(TypedNodeEvaluation::with_frontend_updates(
            (),
            vec![NodeFrontendUpdate {
                name: value_name,
                value: inputs.value.0,
            }],
        ))
    }
}

fn default_display_value() -> AnyInputValue {
    AnyInputValue(InputValue::Float(0.0))
}
