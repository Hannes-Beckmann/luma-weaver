use anyhow::Result;
use shared::{ColorFrame, InputValue, LedLayout, RgbaColor};

use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, NodeFrontendUpdate, RuntimeNode, TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct WledDummyDisplayNode {
    width: usize,
    height: usize,
}

crate::node_runtime::impl_runtime_parameters!(WledDummyDisplayNode {
    width: u64 => |value| crate::node_runtime::max_u64_to_usize(value, 1), default 8usize,
    height: u64 => |value| crate::node_runtime::max_u64_to_usize(value, 1), default 8usize,
});

pub(crate) struct WledDummyDisplayInputs {
    value: Option<AnyInputValue>,
}

crate::node_runtime::impl_runtime_inputs!(WledDummyDisplayInputs {
    value = None,
});

impl RuntimeNode for WledDummyDisplayNode {
    type Inputs = WledDummyDisplayInputs;
    type Outputs = ();

    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let pixel_count = self.width * self.height;
        let layout = context.render_layout.clone().unwrap_or(LedLayout {
            id: format!("dummy:{}x{}", self.width, self.height),
            pixel_count,
            width: Some(self.width),
            height: Some(self.height),
        });

        let frame = match inputs.value.map(|value| value.0) {
            Some(InputValue::ColorFrame(frame)) => normalize_frame(frame, &layout),
            Some(InputValue::Color(color)) => ColorFrame {
                layout: layout.clone(),
                pixels: vec![color; layout.pixel_count],
            },
            _ => ColorFrame {
                layout: layout.clone(),
                pixels: vec![
                    RgbaColor {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    };
                    layout.pixel_count
                ],
            },
        };

        Ok(TypedNodeEvaluation::with_frontend_updates(
            (),
            vec![NodeFrontendUpdate {
                name: "frame".to_owned(),
                value: InputValue::ColorFrame(frame),
            }],
        ))
    }
}

fn normalize_frame(mut frame: ColorFrame, layout: &LedLayout) -> ColorFrame {
    frame.layout = layout.clone();
    if frame.pixels.len() > layout.pixel_count {
        frame.pixels.truncate(layout.pixel_count);
    } else if frame.pixels.len() < layout.pixel_count {
        frame.pixels.extend(std::iter::repeat_n(
            RgbaColor {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            layout.pixel_count - frame.pixels.len(),
        ));
    }
    frame
}
