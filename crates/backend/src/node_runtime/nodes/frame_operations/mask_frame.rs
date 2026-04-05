use anyhow::Result;
use shared::{ColorFrame, InputValue, NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, RuntimeOutputs,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct MaskFrameNode;

impl RuntimeNodeFromParameters for MaskFrameNode {}

pub(crate) struct MaskFrameInputs {
    frame: Option<ColorFrame>,
    mask: Option<ColorFrame>,
}

crate::node_runtime::impl_runtime_inputs!(MaskFrameInputs {
    frame = None,
    mask = None,
});

pub(crate) struct MaskFrameOutputs {
    frame: Option<ColorFrame>,
}

impl RuntimeOutputs for MaskFrameOutputs {
    fn into_runtime_outputs(self) -> anyhow::Result<std::collections::HashMap<String, InputValue>> {
        let mut outputs = std::collections::HashMap::new();
        if let Some(frame) = self.frame {
            outputs.insert("frame".to_owned(), InputValue::ColorFrame(frame));
        }
        Ok(outputs)
    }
}

impl RuntimeNode for MaskFrameNode {
    type Inputs = MaskFrameInputs;
    type Outputs = MaskFrameOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let Some(mut frame) = inputs.frame else {
            return Ok(TypedNodeEvaluation::from_outputs(MaskFrameOutputs {
                frame: None,
            }));
        };
        let Some(mask) = inputs.mask else {
            return Ok(TypedNodeEvaluation::from_outputs(MaskFrameOutputs {
                frame: Some(frame),
            }));
        };

        let diagnostics;
        match mask_alphas_for_frame(&mask, frame.layout.pixel_count) {
            Ok(mask_alphas) => {
                for (pixel, alpha) in frame.pixels.iter_mut().zip(mask_alphas.iter().copied()) {
                    let factor = alpha.clamp(0.0, 1.0);
                    pixel.r = (pixel.r * factor).clamp(0.0, 1.0);
                    pixel.g = (pixel.g * factor).clamp(0.0, 1.0);
                    pixel.b = (pixel.b * factor).clamp(0.0, 1.0);
                    pixel.a = (pixel.a * factor).clamp(0.0, 1.0);
                }
                diagnostics = Vec::new();
            }
            Err(message) => {
                diagnostics = vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("mask_frame_mask_shape_mismatch".to_owned()),
                    message,
                }];
            }
        }

        Ok(TypedNodeEvaluation {
            outputs: MaskFrameOutputs { frame: Some(frame) },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}

fn mask_alphas_for_frame(mask: &ColorFrame, pixel_count: usize) -> Result<Vec<f32>, String> {
    if mask.pixels.len() == 1 {
        return Ok(vec![mask.pixels[0].a; pixel_count]);
    }
    if mask.pixels.len() == pixel_count {
        return Ok(mask.pixels.iter().map(|pixel| pixel.a).collect());
    }
    Err(format!(
        "Mask Frame expected a mask frame with 1 or {} pixels, but got {}.",
        pixel_count,
        mask.pixels.len()
    ))
}

#[cfg(test)]
mod tests {
    use shared::{LedLayout, RgbaColor};

    use super::*;

    #[test]
    fn uses_mask_frame_alpha_channel() {
        let mut node = MaskFrameNode;
        let layout = LedLayout {
            id: "layout".to_owned(),
            pixel_count: 2,
            width: Some(2),
            height: Some(1),
        };
        let result = node
            .evaluate(
                &NodeEvaluationContext {
                    graph_id: "test-graph".to_owned(),
                    graph_name: "Test Graph".to_owned(),
                    elapsed_seconds: 0.0,
                    render_layout: None,
                },
                MaskFrameInputs {
                    frame: Some(ColorFrame {
                        layout: layout.clone(),
                        pixels: vec![
                            RgbaColor {
                                r: 1.0,
                                g: 0.5,
                                b: 0.25,
                                a: 1.0,
                            },
                            RgbaColor {
                                r: 0.2,
                                g: 0.4,
                                b: 0.8,
                                a: 0.5,
                            },
                        ],
                    }),
                    mask: Some(ColorFrame {
                        layout,
                        pixels: vec![
                            RgbaColor {
                                r: 0.0,
                                g: 0.0,
                                b: 0.0,
                                a: 0.25,
                            },
                            RgbaColor {
                                r: 1.0,
                                g: 1.0,
                                b: 1.0,
                                a: 1.0,
                            },
                        ],
                    }),
                },
            )
            .expect("evaluate mask frame");

        let output = result.outputs.frame.expect("masked frame output");
        assert_eq!(
            output.pixels,
            vec![
                RgbaColor {
                    r: 0.25,
                    g: 0.125,
                    b: 0.0625,
                    a: 0.25,
                },
                RgbaColor {
                    r: 0.2,
                    g: 0.4,
                    b: 0.8,
                    a: 0.5,
                },
            ]
        );
    }
}
