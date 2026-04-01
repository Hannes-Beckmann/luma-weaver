use anyhow::Result;
use shared::{InputValue, NodeDiagnostic, NodeDiagnosticSeverity, RgbaColor};

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, RuntimeOutputs,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct ScaleColorNode;

impl RuntimeNodeFromParameters for ScaleColorNode {}

pub(crate) struct ScaleColorInputs {
    color: RgbaColor,
    factor: f32,
}

crate::node_runtime::impl_runtime_inputs!(ScaleColorInputs {
    color = RgbaColor {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    },
    factor = 1.0,
});

pub(crate) struct ScaleColorOutputs {
    color: RgbaColor,
}

impl RuntimeOutputs for ScaleColorOutputs {
    fn into_runtime_outputs(self) -> anyhow::Result<std::collections::HashMap<String, InputValue>> {
        let mut outputs = std::collections::HashMap::new();
        outputs.insert("color".to_owned(), InputValue::Color(self.color));
        Ok(outputs)
    }
}

impl RuntimeNode for ScaleColorNode {
    type Inputs = ScaleColorInputs;
    type Outputs = ScaleColorOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let factor = inputs.factor.max(0.0);
        let diagnostics = if (factor - inputs.factor).abs() > f32::EPSILON {
            vec![NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Warning,
                code: Some("scale_color_factor_clamped".to_owned()),
                message: format!(
                    "Scale Color factor {} is too small; using {} instead.",
                    inputs.factor, factor
                ),
            }]
        } else {
            Vec::new()
        };

        Ok(TypedNodeEvaluation {
            outputs: ScaleColorOutputs {
                color: RgbaColor {
                    r: (inputs.color.r * factor).clamp(0.0, 1.0),
                    g: (inputs.color.g * factor).clamp(0.0, 1.0),
                    b: (inputs.color.b * factor).clamp(0.0, 1.0),
                    a: inputs.color.a,
                },
            },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}
