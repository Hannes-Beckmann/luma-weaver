use anyhow::Result;
use shared::{FloatTensor, InputValue, NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, RuntimeOutputs,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct ScaleTensorNode;

impl RuntimeNodeFromParameters for ScaleTensorNode {}

pub(crate) struct ScaleTensorInputs {
    tensor: FloatTensor,
    factor: f32,
}

crate::node_runtime::impl_runtime_inputs!(ScaleTensorInputs {
    tensor = FloatTensor {
        shape: vec![1],
        values: vec![0.0],
    },
    factor = 1.0,
});

pub(crate) struct ScaleTensorOutputs {
    tensor: FloatTensor,
}

impl RuntimeOutputs for ScaleTensorOutputs {
    fn into_runtime_outputs(self) -> anyhow::Result<std::collections::HashMap<String, InputValue>> {
        let mut outputs = std::collections::HashMap::new();
        outputs.insert("tensor".to_owned(), InputValue::FloatTensor(self.tensor));
        Ok(outputs)
    }
}

impl RuntimeNode for ScaleTensorNode {
    type Inputs = ScaleTensorInputs;
    type Outputs = ScaleTensorOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let values = inputs
            .tensor
            .values
            .iter()
            .map(|value| *value * inputs.factor)
            .collect::<Vec<_>>();
        let diagnostics =
            if inputs.factor.is_finite() && values.iter().all(|value| value.is_finite()) {
                Vec::new()
            } else {
                vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("scale_tensor_non_finite".to_owned()),
                    message: "Scale Tensor produced non-finite values.".to_owned(),
                }]
            };

        Ok(TypedNodeEvaluation {
            outputs: ScaleTensorOutputs {
                tensor: FloatTensor {
                    shape: inputs.tensor.shape,
                    values,
                },
            },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}
