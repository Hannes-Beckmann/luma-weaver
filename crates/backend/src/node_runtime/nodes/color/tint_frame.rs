use anyhow::Result;
use shared::{ColorFrame, InputValue, RgbaColor};

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, RuntimeOutputs,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct TintFrameNode;

impl RuntimeNodeFromParameters for TintFrameNode {}

pub(crate) struct TintFrameInputs {
    frame: Option<ColorFrame>,
    tint: RgbaColor,
}

crate::node_runtime::impl_runtime_inputs!(TintFrameInputs {
    frame = None,
    tint = RgbaColor {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    },
});

pub(crate) struct TintFrameOutputs {
    frame: Option<ColorFrame>,
}

impl RuntimeOutputs for TintFrameOutputs {
    fn into_runtime_outputs(self) -> anyhow::Result<std::collections::HashMap<String, InputValue>> {
        let mut outputs = std::collections::HashMap::new();
        if let Some(frame) = self.frame {
            outputs.insert("frame".to_owned(), InputValue::ColorFrame(frame));
        }
        Ok(outputs)
    }
}

impl RuntimeNode for TintFrameNode {
    type Inputs = TintFrameInputs;
    type Outputs = TintFrameOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let frame = inputs.frame.map(|mut frame| {
            for pixel in &mut frame.pixels {
                pixel.r = (pixel.r * inputs.tint.r).clamp(0.0, 1.0);
                pixel.g = (pixel.g * inputs.tint.g).clamp(0.0, 1.0);
                pixel.b = (pixel.b * inputs.tint.b).clamp(0.0, 1.0);
                pixel.a = (pixel.a * inputs.tint.a).clamp(0.0, 1.0);
            }
            frame
        });

        Ok(TypedNodeEvaluation::from_outputs(TintFrameOutputs {
            frame,
        }))
    }
}
