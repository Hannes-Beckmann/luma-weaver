use anyhow::Result;
use shared::{InputValue, NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, RuntimeOutputs,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct FrameBrightnessNode;

impl RuntimeNodeFromParameters for FrameBrightnessNode {}

pub(crate) struct FrameBrightnessInputs {
    frame: Option<AnyInputValue>,
    factor: f32,
}

crate::node_runtime::impl_runtime_inputs!(FrameBrightnessInputs {
    frame = None,
    factor = 1.0,
});

pub(crate) struct FrameBrightnessOutputs {
    frame: Option<InputValue>,
}

impl RuntimeOutputs for FrameBrightnessOutputs {
    fn into_runtime_outputs(
        self,
    ) -> anyhow::Result<std::collections::HashMap<String, shared::InputValue>> {
        let mut outputs = std::collections::HashMap::new();
        if let Some(frame) = self.frame {
            outputs.insert("frame".to_owned(), frame);
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
        let Some(mut frame) = inputs.frame.map(|value| value.0) else {
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
        for pixel in &mut frame
            .as_frame_mut()
            .expect("frame brightness only accepts frame values")
            .pixels
        {
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

#[cfg(test)]
mod tests {
    use super::{FrameBrightnessInputs, FrameBrightnessNode};
    use crate::node_runtime::{AnyInputValue, NodeEvaluationContext, RuntimeNode};
    use shared::{ColorFrame, LedLayout, RgbaColor};

    #[test]
    fn default_factor_preserves_pixel_values() {
        let mut node = FrameBrightnessNode;
        let input_frame = ColorFrame {
            layout: LedLayout {
                id: "frame-brightness-default".to_owned(),

                role: ::shared::LedLayoutRole::RenderTarget,
                pixel_count: 1,
                width: Some(1),
                height: Some(1),
                points_3d: None,
            },
            pixels: vec![RgbaColor {
                r: 0.25,
                g: 0.5,
                b: 0.75,
                a: 1.0,
            }],
        };

        let evaluation = node
            .evaluate(
                &NodeEvaluationContext {
                    graph_id: "test-graph".to_owned(),
                    graph_name: "Test Graph".to_owned(),
                    elapsed_seconds: 0.0,
                    render_layout: None,
                    graph_layout_assets: Default::default(),
                },
                FrameBrightnessInputs {
                    frame: Some(AnyInputValue(shared::InputValue::ColorFrame(
                        input_frame.clone(),
                    ))),
                    factor: 1.0,
                },
            )
            .expect("frame brightness evaluation should succeed");

        assert_eq!(
            evaluation
                .outputs
                .frame
                .as_ref()
                .and_then(shared::InputValue::as_frame),
            Some(&input_frame),
            "default frame brightness factor should be neutral"
        );
        assert!(
            evaluation.diagnostics.is_empty(),
            "neutral default should not emit diagnostics"
        );
    }
}
