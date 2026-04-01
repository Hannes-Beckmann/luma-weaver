use anyhow::Result;

use crate::node_runtime::{NodeEvaluationContext, RuntimeNode, TypedNodeEvaluation};

#[derive(Default)]
pub(crate) struct SinusNode {
    frequency: f64,
    amplitude: f64,
    last_value: f32,
}

crate::node_runtime::impl_runtime_parameters!(SinusNode {
    frequency: f64 = 1.0,
    amplitude: f64 = 1.0,
    ..Self::default()
});

pub(crate) struct SinusOutputs {
    value: f32,
}

crate::node_runtime::impl_runtime_outputs!(SinusOutputs { value });

impl RuntimeNode for SinusNode {
    type Inputs = ();
    type Outputs = SinusOutputs;

    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        _inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let value = (std::f64::consts::TAU * self.frequency * context.elapsed_seconds).sin()
            * self.amplitude;
        let value = value as f32;
        self.last_value = value;

        Ok(TypedNodeEvaluation::from_outputs(SinusOutputs { value }))
    }
}
