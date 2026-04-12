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
    disable: f32,
}

crate::node_runtime::impl_runtime_inputs!(WledDummyDisplayInputs {
    value = None,
    disable = 0.0,
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

        let frame = if is_disabled(inputs.disable) {
            black_frame(&layout)
        } else {
            match inputs.value.map(|value| value.0) {
                Some(InputValue::ColorFrame(frame)) => normalize_frame(frame, &layout),
                Some(InputValue::Color(color)) => ColorFrame {
                    layout: layout.clone(),
                    pixels: vec![color; layout.pixel_count],
                },
                _ => black_frame(&layout),
            }
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

fn is_disabled(value: f32) -> bool {
    value >= 0.5
}

fn black_frame(layout: &LedLayout) -> ColorFrame {
    ColorFrame {
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

#[cfg(test)]
mod tests {
    use shared::{InputValue, RgbaColor};

    use super::{WledDummyDisplayInputs, WledDummyDisplayNode, is_disabled};
    use crate::node_runtime::{AnyInputValue, NodeEvaluationContext, RuntimeNode};

    fn evaluation_context() -> NodeEvaluationContext {
        NodeEvaluationContext {
            graph_id: "graph".to_owned(),
            graph_name: "Graph".to_owned(),
            elapsed_seconds: 0.0,
            render_layout: None,
        }
    }

    #[test]
    fn disable_threshold_matches_issue_contract() {
        assert!(!is_disabled(0.49));
        assert!(is_disabled(0.5));
        assert!(is_disabled(1.0));
    }

    #[test]
    fn disabled_dummy_display_emits_black_frame_update() {
        let mut node = WledDummyDisplayNode {
            width: 8,
            height: 8,
        };
        let evaluation = node
            .evaluate(
                &evaluation_context(),
                WledDummyDisplayInputs {
                    value: Some(AnyInputValue(InputValue::Color(RgbaColor {
                        r: 1.0,
                        g: 0.2,
                        b: 0.1,
                        a: 1.0,
                    }))),
                    disable: 1.0,
                },
            )
            .expect("evaluate disabled display");

        let frame = match &evaluation.frontend_updates[0].value {
            InputValue::ColorFrame(frame) => frame,
            value => panic!("expected color frame update, got {value:?}"),
        };

        assert_eq!(frame.layout.width, Some(8));
        assert_eq!(frame.layout.height, Some(8));
        assert_eq!(frame.pixels.len(), 64);
        assert!(frame.pixels.iter().all(|pixel| {
            *pixel
                == RgbaColor {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                }
        }));
    }
}
