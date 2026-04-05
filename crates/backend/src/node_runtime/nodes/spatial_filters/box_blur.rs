use anyhow::Result;
use shared::{ColorFrame, InputValue, NodeDiagnostic, NodeDiagnosticSeverity, RgbaColor};

use crate::node_runtime::nodes::color::filter_utils::{clamped_index, layout_dimensions};
use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, RuntimeOutputs,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct BoxBlurNode;

impl RuntimeNodeFromParameters for BoxBlurNode {}

pub(crate) struct BoxBlurInputs {
    frame: Option<ColorFrame>,
    radius: f32,
}

crate::node_runtime::impl_runtime_inputs!(BoxBlurInputs {
    frame = None,
    radius = 1.0,
});

pub(crate) struct BoxBlurOutputs {
    frame: Option<ColorFrame>,
}

impl RuntimeOutputs for BoxBlurOutputs {
    fn into_runtime_outputs(self) -> anyhow::Result<std::collections::HashMap<String, InputValue>> {
        let mut outputs = std::collections::HashMap::new();
        if let Some(frame) = self.frame {
            outputs.insert("frame".to_owned(), InputValue::ColorFrame(frame));
        }
        Ok(outputs)
    }
}

impl RuntimeNode for BoxBlurNode {
    type Inputs = BoxBlurInputs;
    type Outputs = BoxBlurOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let mut diagnostics = Vec::new();
        let Some(frame) = inputs.frame else {
            return Ok(TypedNodeEvaluation::from_outputs(BoxBlurOutputs {
                frame: None,
            }));
        };

        let radius = inputs.radius.round().clamp(0.0, 32.0) as usize;
        if (radius as f32 - inputs.radius).abs() > f32::EPSILON {
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Warning,
                code: Some("box_blur_radius_clamped".to_owned()),
                message: format!(
                    "Blur radius {} is out of range; using {} instead.",
                    inputs.radius, radius
                ),
            });
        }
        if radius == 0 || frame.pixels.is_empty() {
            return Ok(TypedNodeEvaluation {
                outputs: BoxBlurOutputs { frame: Some(frame) },
                frontend_updates: Vec::new(),
                diagnostics,
            });
        }

        let (width, height) = layout_dimensions(&frame.layout);
        let mut pixels = Vec::with_capacity(frame.pixels.len());
        for y in 0..height {
            for x in 0..width {
                let mut sum = [0.0; 4];
                let mut count = 0.0;
                for sample_y in y.saturating_sub(radius)..=(y + radius).min(height - 1) {
                    for sample_x in x.saturating_sub(radius)..=(x + radius).min(width - 1) {
                        let pixel = frame.pixels
                            [clamped_index(sample_x, sample_y, width, height, frame.pixels.len())];
                        sum[0] += pixel.r;
                        sum[1] += pixel.g;
                        sum[2] += pixel.b;
                        sum[3] += pixel.a;
                        count += 1.0;
                    }
                }
                pixels.push(RgbaColor {
                    r: sum[0] / count,
                    g: sum[1] / count,
                    b: sum[2] / count,
                    a: sum[3] / count,
                });
                if pixels.len() == frame.pixels.len() {
                    break;
                }
            }
            if pixels.len() == frame.pixels.len() {
                break;
            }
        }

        Ok(TypedNodeEvaluation {
            outputs: BoxBlurOutputs {
                frame: Some(ColorFrame {
                    layout: frame.layout,
                    pixels,
                }),
            },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}
