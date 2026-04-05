use anyhow::Result;
use shared::{ColorFrame, NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, RuntimeOutputs,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct FrameBrightnessNode;

impl RuntimeNodeFromParameters for FrameBrightnessNode {}

pub(crate) struct FrameBrightnessInputs {
    frame: Option<ColorFrame>,
    factor: f32,
}

crate::node_runtime::impl_runtime_inputs!(FrameBrightnessInputs {
    frame = None,
    factor = 0.0,
});

pub(crate) struct FrameBrightnessOutputs {
    frame: Option<ColorFrame>,
}

impl RuntimeOutputs for FrameBrightnessOutputs {
    fn into_runtime_outputs(
        self,
    ) -> anyhow::Result<std::collections::HashMap<String, shared::InputValue>> {
        let mut outputs = std::collections::HashMap::new();
        if let Some(frame) = self.frame {
            outputs.insert("frame".to_owned(), shared::InputValue::ColorFrame(frame));
        }
        Ok(outputs)
    }
}

impl RuntimeNode for FrameBrightnessNode {
    type Inputs = FrameBrightnessInputs;
    type Outputs = FrameBrightnessOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let mut diagnostics = Vec::new();
        let Some(mut frame) = inputs.frame else {
            return Ok(TypedNodeEvaluation::from_outputs(FrameBrightnessOutputs {
                frame: None,
            }));
        };

        let factor = inputs.factor.max(0.0);
        if (factor - inputs.factor).abs() > f32::EPSILON {
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Warning,
                code: Some("frame_brightness_factor_clamped".to_owned()),
                message: format!(
                    "Brightness factor {} is too small; using {} instead.",
                    inputs.factor, factor
                ),
            });
        }
        for pixel in &mut frame.pixels {
            pixel.r = (pixel.r * factor).clamp(0.0, 1.0);
            pixel.g = (pixel.g * factor).clamp(0.0, 1.0);
            pixel.b = (pixel.b * factor).clamp(0.0, 1.0);
        }

        Ok(TypedNodeEvaluation {
            outputs: FrameBrightnessOutputs { frame: Some(frame) },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}
