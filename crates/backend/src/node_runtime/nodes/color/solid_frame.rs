use anyhow::Result;
use shared::{ColorFrame, LedLayout, RgbaColor};

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct SolidFrameNode;

impl RuntimeNodeFromParameters for SolidFrameNode {}

pub(crate) struct SolidFrameInputs {
    color: RgbaColor,
}

crate::node_runtime::impl_runtime_inputs!(SolidFrameInputs {
    color = RgbaColor {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    },
});

pub(crate) struct SolidFrameOutputs {
    frame: ColorFrame,
}

crate::node_runtime::impl_runtime_outputs!(SolidFrameOutputs { frame });

impl RuntimeNode for SolidFrameNode {
    type Inputs = SolidFrameInputs;
    type Outputs = SolidFrameOutputs;

    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let Some(layout) = context.render_layout.clone() else {
            return Ok(TypedNodeEvaluation::from_outputs(SolidFrameOutputs {
                frame: ColorFrame {
                    layout: LedLayout {
                        id: "solid_frame:unbound".to_owned(),
                        pixel_count: 0,
                        width: None,
                        height: None,
                    },
                    pixels: Vec::new(),
                },
            }));
        };

        Ok(TypedNodeEvaluation::from_outputs(SolidFrameOutputs {
            frame: ColorFrame {
                pixels: vec![inputs.color; layout.pixel_count],
                layout,
            },
        }))
    }
}
