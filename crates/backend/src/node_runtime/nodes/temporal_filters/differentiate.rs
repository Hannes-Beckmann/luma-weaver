use anyhow::Result;

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct DifferentiateNode {
    last_elapsed_seconds: Option<f64>,
    last_value: Option<f32>,
}

impl RuntimeNodeFromParameters for DifferentiateNode {}

pub(crate) struct DifferentiateInputs {
    value: f32,
}

crate::node_runtime::impl_runtime_inputs!(DifferentiateInputs {
    value = 0.0,
});

pub(crate) struct DifferentiateOutputs {
    value: f32,
}

crate::node_runtime::impl_runtime_outputs!(DifferentiateOutputs { value });

impl RuntimeNode for DifferentiateNode {
    type Inputs = DifferentiateInputs;
    type Outputs = DifferentiateOutputs;

    /// Differentiates the input against elapsed runtime seconds.
    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let output = match (self.last_value, self.last_elapsed_seconds) {
            (Some(previous_value), Some(previous_time))
                if context.elapsed_seconds >= previous_time =>
            {
                let dt = (context.elapsed_seconds - previous_time) as f32;
                if dt > 0.0 {
                    (inputs.value - previous_value) / dt
                } else {
                    0.0
                }
            }
            _ => 0.0,
        };

        self.last_value = Some(inputs.value);
        self.last_elapsed_seconds = Some(context.elapsed_seconds);

        Ok(TypedNodeEvaluation::from_outputs(DifferentiateOutputs {
            value: output,
        }))
    }
}

#[cfg(test)]
mod tests {
    use crate::node_runtime::{NodeEvaluationContext, RuntimeNode};

    use super::{DifferentiateInputs, DifferentiateNode};

    fn context(elapsed_seconds: f64) -> NodeEvaluationContext {
        NodeEvaluationContext {
            graph_id: "test-graph".to_owned(),
            graph_name: "Test Graph".to_owned(),
            elapsed_seconds,
            render_layout: None,
        }
    }

    #[test]
    fn first_sample_outputs_zero() {
        let mut node = DifferentiateNode::default();

        let evaluation = node
            .evaluate(&context(0.0), DifferentiateInputs { value: 4.0 })
            .expect("evaluate first sample");

        assert_eq!(evaluation.outputs.value, 0.0);
    }

    #[test]
    fn computes_rate_of_change_from_elapsed_seconds() {
        let mut node = DifferentiateNode::default();

        node.evaluate(&context(0.0), DifferentiateInputs { value: 1.0 })
            .expect("prime differentiate");
        let evaluation = node
            .evaluate(&context(0.5), DifferentiateInputs { value: 2.0 })
            .expect("differentiate ramp");

        assert_eq!(evaluation.outputs.value, 2.0);
    }

    #[test]
    fn returns_zero_when_time_does_not_advance() {
        let mut node = DifferentiateNode::default();

        node.evaluate(&context(1.0), DifferentiateInputs { value: 1.0 })
            .expect("prime differentiate");
        let evaluation = node
            .evaluate(&context(1.0), DifferentiateInputs { value: 3.0 })
            .expect("differentiate repeated timestamp");

        assert_eq!(evaluation.outputs.value, 0.0);
    }
}
