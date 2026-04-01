use anyhow::Result;
use shared::{NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct MultiplyFloatNode;

impl RuntimeNodeFromParameters for MultiplyFloatNode {}

pub(crate) struct MultiplyFloatInputs {
    a: f32,
    b: f32,
}

crate::node_runtime::impl_runtime_inputs!(MultiplyFloatInputs {
    a = 1.0,
    b = 1.0,
});

pub(crate) struct MultiplyFloatOutputs {
    product: f32,
}

crate::node_runtime::impl_runtime_outputs!(MultiplyFloatOutputs { product });

impl RuntimeNode for MultiplyFloatNode {
    type Inputs = MultiplyFloatInputs;
    type Outputs = MultiplyFloatOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let product = inputs.a * inputs.b;
        let diagnostics = if product.is_finite() {
            Vec::new()
        } else {
            vec![NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Error,
                code: Some("multiply_float_non_finite".to_owned()),
                message: "Multiply Float produced a non-finite result.".to_owned(),
            }]
        };

        Ok(TypedNodeEvaluation {
            outputs: MultiplyFloatOutputs { product },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}
