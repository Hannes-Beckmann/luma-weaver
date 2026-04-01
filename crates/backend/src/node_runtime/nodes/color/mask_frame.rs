use anyhow::Result;
use shared::{ColorFrame, FloatTensor, InputValue, NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, RuntimeOutputs,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct MaskFrameNode;

impl RuntimeNodeFromParameters for MaskFrameNode {}

pub(crate) struct MaskFrameInputs {
    frame: Option<ColorFrame>,
    mask: FloatTensor,
}

crate::node_runtime::impl_runtime_inputs!(MaskFrameInputs {
    frame = None,
    mask = FloatTensor {
        shape: vec![1],
        values: vec![0.0],
    },
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

        let diagnostics;
        match mask_values_for_frame(&inputs.mask, frame.layout.pixel_count) {
            Ok(mask_values) => {
                let mut clamped = false;
                for (pixel, factor) in frame.pixels.iter_mut().zip(mask_values.iter().copied()) {
                    let factor = if (0.0..=1.0).contains(&factor) {
                        factor
                    } else {
                        clamped = true;
                        factor.clamp(0.0, 1.0)
                    };
                    pixel.r = (pixel.r * factor).clamp(0.0, 1.0);
                    pixel.g = (pixel.g * factor).clamp(0.0, 1.0);
                    pixel.b = (pixel.b * factor).clamp(0.0, 1.0);
                }
                diagnostics = if clamped {
                    vec![NodeDiagnostic {
                        severity: NodeDiagnosticSeverity::Warning,
                        code: Some("mask_frame_mask_clamped".to_owned()),
                        message: "Mask Frame clamped mask values into the [0, 1] range.".to_owned(),
                    }]
                } else {
                    Vec::new()
                };
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

fn mask_values_for_frame(mask: &FloatTensor, pixel_count: usize) -> Result<Vec<f32>, String> {
    if mask.values.len() == 1 {
        return Ok(vec![mask.values[0]; pixel_count]);
    }
    if mask.values.len() == pixel_count {
        return Ok(mask.values.clone());
    }
    Err(format!(
        "Mask Frame expected a mask with 1 or {} values, but got {}.",
        pixel_count,
        mask.values.len()
    ))
}
