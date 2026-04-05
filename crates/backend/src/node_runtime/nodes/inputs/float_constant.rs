use anyhow::Result;

use crate::node_runtime::{NodeEvaluationContext, RuntimeNode, TypedNodeEvaluation};

#[derive(Default)]
pub(crate) struct FloatConstantNode {
    value: f32,
}

crate::node_runtime::impl_runtime_parameters!(FloatConstantNode { value: f32 = 0.0 });

pub(crate) struct FloatConstantOutputs {
    value: f32,
}

crate::node_runtime::impl_runtime_outputs!(FloatConstantOutputs { value });

impl RuntimeNode for FloatConstantNode {
    type Inputs = ();
    type Outputs = FloatConstantOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        _inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        Ok(TypedNodeEvaluation {
            outputs: FloatConstantOutputs { value: self.value },
            frontend_updates: Vec::new(),
            diagnostics: Vec::new(),
        })
    }
}
