use anyhow::Result;
use shared::{ColorFrame, FloatTensor, InputValue};

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeOutputs, TypedNodeEvaluation,
};

#[derive(Clone, Copy)]
enum Channel {
    Red,
    Green,
    Blue,
    Alpha,
}

pub(crate) struct ExtractChannelsNode {
    channel: Channel,
}

impl Default for ExtractChannelsNode {
    fn default() -> Self {
        Self {
            channel: Channel::Red,
        }
    }
}

crate::node_runtime::impl_runtime_parameters!(ExtractChannelsNode {
    channel: String => |value| Channel::from_id(&value), default Channel::Red,
});

pub(crate) struct ExtractChannelsInputs {
    frame: Option<ColorFrame>,
}

crate::node_runtime::impl_runtime_inputs!(ExtractChannelsInputs {
    frame = None,
});

pub(crate) struct ExtractChannelsOutputs {
    tensor: Option<FloatTensor>,
}

impl RuntimeOutputs for ExtractChannelsOutputs {
    fn into_runtime_outputs(self) -> anyhow::Result<std::collections::HashMap<String, InputValue>> {
        let mut outputs = std::collections::HashMap::new();
        if let Some(tensor) = self.tensor {
            outputs.insert("tensor".to_owned(), InputValue::FloatTensor(tensor));
        }
        Ok(outputs)
    }
}

impl RuntimeNode for ExtractChannelsNode {
    type Inputs = ExtractChannelsInputs;
    type Outputs = ExtractChannelsOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let tensor = inputs.frame.map(|frame| {
            let shape = match (frame.layout.width, frame.layout.height) {
                (Some(width), Some(height)) => vec![height, width],
                _ => vec![frame.layout.pixel_count],
            };

            FloatTensor {
                shape,
                values: frame
                    .pixels
                    .iter()
                    .map(|pixel| match self.channel {
                        Channel::Red => pixel.r,
                        Channel::Green => pixel.g,
                        Channel::Blue => pixel.b,
                        Channel::Alpha => pixel.a,
                    })
                    .collect(),
            }
        });

        Ok(TypedNodeEvaluation::from_outputs(ExtractChannelsOutputs {
            tensor,
        }))
    }
}

impl Channel {
    fn from_id(id: &str) -> Self {
        match id {
            "g" => Self::Green,
            "b" => Self::Blue,
            "a" => Self::Alpha,
            _ => Self::Red,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::Value as JsonValue;
    use shared::{ColorFrame, FloatTensor, LedLayout, RgbaColor};

    use super::{ExtractChannelsInputs, ExtractChannelsNode};
    use crate::node_runtime::{NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters};

    fn context() -> NodeEvaluationContext {
        NodeEvaluationContext {
            graph_id: "test-graph".to_owned(),
            graph_name: "Test Graph".to_owned(),
            elapsed_seconds: 0.0,
            render_layout: None,
        }
    }

    #[test]
    fn defaults_to_red_channel() {
        let mut node = ExtractChannelsNode::default();
        let evaluation = node
            .evaluate(
                &context(),
                ExtractChannelsInputs {
                    frame: Some(ColorFrame {
                        layout: LedLayout {
                            id: "frame".to_owned(),
                            pixel_count: 2,
                            width: Some(2),
                            height: Some(1),
                        },
                        pixels: vec![
                            RgbaColor {
                                r: 0.1,
                                g: 0.2,
                                b: 0.3,
                                a: 0.4,
                            },
                            RgbaColor {
                                r: 0.5,
                                g: 0.6,
                                b: 0.7,
                                a: 0.8,
                            },
                        ],
                    }),
                },
            )
            .expect("extract channels evaluation should succeed");

        assert_eq!(
            evaluation.outputs.tensor,
            Some(FloatTensor {
                shape: vec![1, 2],
                values: vec![0.1, 0.5],
            })
        );
    }

    #[test]
    fn supports_alpha_channel_from_parameters() {
        let mut parameters = HashMap::new();
        parameters.insert("channel".to_owned(), JsonValue::from("a"));
        let mut node = ExtractChannelsNode::from_parameters(&parameters).node;

        let evaluation = node
            .evaluate(
                &context(),
                ExtractChannelsInputs {
                    frame: Some(ColorFrame {
                        layout: LedLayout {
                            id: "frame".to_owned(),
                            pixel_count: 3,
                            width: None,
                            height: None,
                        },
                        pixels: vec![
                            RgbaColor {
                                r: 0.1,
                                g: 0.2,
                                b: 0.3,
                                a: 0.4,
                            },
                            RgbaColor {
                                r: 0.5,
                                g: 0.6,
                                b: 0.7,
                                a: 0.8,
                            },
                            RgbaColor {
                                r: 0.9,
                                g: 1.0,
                                b: 0.2,
                                a: 0.25,
                            },
                        ],
                    }),
                },
            )
            .expect("extract channels evaluation should succeed");

        assert_eq!(
            evaluation.outputs.tensor,
            Some(FloatTensor {
                shape: vec![3],
                values: vec![0.4, 0.8, 0.25],
            })
        );
    }

    #[test]
    fn omits_output_when_frame_is_missing() {
        let mut node = ExtractChannelsNode::default();
        let evaluation = node
            .evaluate(&context(), ExtractChannelsInputs { frame: None })
            .expect("extract channels evaluation should succeed");

        assert_eq!(evaluation.outputs.tensor, None);
    }
}
