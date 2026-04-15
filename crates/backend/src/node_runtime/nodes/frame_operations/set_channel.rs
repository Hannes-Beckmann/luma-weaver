use std::collections::HashMap;

use anyhow::Result;
use palette::{FromColor, Hsv, RgbHue, Srgb};
use shared::{
    ColorFrame, FloatTensor, InputValue, NodeDiagnostic, NodeDiagnosticSeverity, RgbaColor,
};

use crate::node_runtime::tensor::coerce_float_tensor;
use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeOutputs, TypedNodeEvaluation,
};

#[derive(Clone, Copy)]
enum Channel {
    Red,
    Green,
    Blue,
    Alpha,
    Hue,
    Saturation,
    Value,
}

pub(crate) struct SetChannelNode {
    channel: Channel,
}

impl Default for SetChannelNode {
    fn default() -> Self {
        Self {
            channel: Channel::Red,
        }
    }
}

crate::node_runtime::impl_runtime_parameters!(SetChannelNode {
    channel: String => |value| Channel::from_id(&value), default Channel::Red,
});

pub(crate) struct SetChannelInputs {
    frame: Option<ColorFrame>,
    tensor: Option<FloatTensor>,
}

crate::node_runtime::impl_runtime_inputs!(SetChannelInputs {
    frame = None,
    tensor = None,
});

pub(crate) struct SetChannelOutputs {
    frame: Option<ColorFrame>,
}

impl RuntimeOutputs for SetChannelOutputs {
    fn into_runtime_outputs(self) -> anyhow::Result<HashMap<String, InputValue>> {
        let mut outputs = HashMap::new();
        if let Some(frame) = self.frame {
            outputs.insert("frame".to_owned(), InputValue::ColorFrame(frame));
        }
        Ok(outputs)
    }
}

impl RuntimeNode for SetChannelNode {
    type Inputs = SetChannelInputs;
    type Outputs = SetChannelOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let Some(mut frame) = inputs.frame else {
            return Ok(TypedNodeEvaluation::from_outputs(SetChannelOutputs {
                frame: None,
            }));
        };
        let Some(tensor) = inputs.tensor else {
            return Ok(TypedNodeEvaluation::from_outputs(SetChannelOutputs {
                frame: Some(frame),
            }));
        };

        let target_shape = match (frame.layout.width, frame.layout.height) {
            (Some(width), Some(height)) => vec![height, width],
            _ => vec![frame.layout.pixel_count],
        };

        let diagnostics;
        match coerce_float_tensor(&InputValue::FloatTensor(tensor), &target_shape) {
            Ok(tensor) => {
                for (pixel, value) in frame.pixels.iter_mut().zip(tensor.values.iter().copied()) {
                    *pixel = self.channel.apply(*pixel, value);
                }
                diagnostics = Vec::new();
            }
            Err(error) => {
                diagnostics = vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("set_channel_tensor_shape_mismatch".to_owned()),
                    message: format!(
                        "Set Channel expected a tensor matching shape {:?}: {}.",
                        target_shape, error
                    ),
                }];
            }
        }

        Ok(TypedNodeEvaluation {
            outputs: SetChannelOutputs { frame: Some(frame) },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}

impl Channel {
    fn from_id(id: &str) -> Self {
        match id {
            "g" => Self::Green,
            "b" => Self::Blue,
            "a" => Self::Alpha,
            "h" => Self::Hue,
            "s" => Self::Saturation,
            "v" => Self::Value,
            _ => Self::Red,
        }
    }

    fn apply(self, pixel: RgbaColor, value: f32) -> RgbaColor {
        match self {
            Self::Red => RgbaColor {
                r: value.clamp(0.0, 1.0),
                ..pixel
            },
            Self::Green => RgbaColor {
                g: value.clamp(0.0, 1.0),
                ..pixel
            },
            Self::Blue => RgbaColor {
                b: value.clamp(0.0, 1.0),
                ..pixel
            },
            Self::Alpha => RgbaColor {
                a: value.clamp(0.0, 1.0),
                ..pixel
            },
            Self::Hue | Self::Saturation | Self::Value => apply_hsv_channel(pixel, self, value),
        }
    }
}

fn apply_hsv_channel(pixel: RgbaColor, channel: Channel, value: f32) -> RgbaColor {
    let mut hsv = Hsv::from_color(Srgb::new(pixel.r, pixel.g, pixel.b));
    match channel {
        Channel::Hue => hsv.hue = RgbHue::from_degrees(value.rem_euclid(1.0) * 360.0),
        Channel::Saturation => hsv.saturation = value.clamp(0.0, 1.0),
        Channel::Value => hsv.value = value.clamp(0.0, 1.0),
        _ => {}
    }

    let rgb: Srgb<f32> = Srgb::from_color(hsv);
    RgbaColor {
        r: rgb.red.clamp(0.0, 1.0),
        g: rgb.green.clamp(0.0, 1.0),
        b: rgb.blue.clamp(0.0, 1.0),
        a: pixel.a,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value as JsonValue;

    use super::*;
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
    fn sets_red_channel_from_tensor() {
        let mut node = SetChannelNode::default();
        let layout = shared::LedLayout {
            id: "frame".to_owned(),
            pixel_count: 2,
            width: Some(2),
            height: Some(1),
        };
        let evaluation = node
            .evaluate(
                &context(),
                SetChannelInputs {
                    frame: Some(ColorFrame {
                        layout: layout.clone(),
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
                    tensor: Some(FloatTensor {
                        shape: vec![1, 2],
                        values: vec![0.9, 0.25],
                    }),
                },
            )
            .expect("set channel evaluation should succeed");

        assert_eq!(
            evaluation.outputs.frame,
            Some(ColorFrame {
                layout,
                pixels: vec![
                    RgbaColor {
                        r: 0.9,
                        g: 0.2,
                        b: 0.3,
                        a: 0.4,
                    },
                    RgbaColor {
                        r: 0.25,
                        g: 0.6,
                        b: 0.7,
                        a: 0.8,
                    },
                ],
            })
        );
    }

    #[test]
    fn sets_hue_channel_from_tensor() {
        let mut parameters = HashMap::new();
        parameters.insert("channel".to_owned(), JsonValue::from("h"));
        let mut node = SetChannelNode::from_parameters(&parameters).node;

        let evaluation = node
            .evaluate(
                &context(),
                SetChannelInputs {
                    frame: Some(ColorFrame {
                        layout: shared::LedLayout {
                            id: "frame".to_owned(),
                            pixel_count: 1,
                            width: Some(1),
                            height: Some(1),
                        },
                        pixels: vec![RgbaColor {
                            r: 1.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.75,
                        }],
                    }),
                    tensor: Some(FloatTensor {
                        shape: vec![1, 1],
                        values: vec![1.0 / 3.0],
                    }),
                },
            )
            .expect("set channel evaluation should succeed");

        let output = evaluation.outputs.frame.expect("frame output");
        let pixel = output.pixels[0];
        assert!(pixel.r < 0.1);
        assert!(pixel.g > 0.95);
        assert!(pixel.b < 0.1);
        assert!((pixel.a - 0.75).abs() < 1e-6);
    }
}
