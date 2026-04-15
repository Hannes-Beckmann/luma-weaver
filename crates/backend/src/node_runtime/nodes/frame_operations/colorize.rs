use anyhow::Result;
use shared::{ColorFrame, ColorGradient, FloatTensor, InputValue, RgbaColor};

use crate::color_math::sample_gradient_hsv;
use crate::node_runtime::tensor::layout_from_shape;
use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, RuntimeNode, RuntimeOutputs, TypedNodeEvaluation,
};

pub(crate) struct ColorizeNode {
    gradient: ColorGradient,
}

impl Default for ColorizeNode {
    fn default() -> Self {
        Self {
            gradient: default_gradient(),
        }
    }
}

crate::node_runtime::impl_runtime_parameters!(ColorizeNode {
    gradient: ColorGradient => |value| crate::node_runtime::non_empty_gradient(value, default_gradient()), default default_gradient(),
});

pub(crate) struct ColorizeInputs {
    value: Option<AnyInputValue>,
}

crate::node_runtime::impl_runtime_inputs!(ColorizeInputs {
    value = None,
});

pub(crate) struct ColorizeOutputs {
    frame: Option<ColorFrame>,
}

impl RuntimeOutputs for ColorizeOutputs {
    fn into_runtime_outputs(self) -> anyhow::Result<std::collections::HashMap<String, InputValue>> {
        let mut outputs = std::collections::HashMap::new();
        if let Some(frame) = self.frame {
            outputs.insert("frame".to_owned(), InputValue::ColorFrame(frame));
        }
        Ok(outputs)
    }
}

impl RuntimeNode for ColorizeNode {
    type Inputs = ColorizeInputs;
    type Outputs = ColorizeOutputs;

    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let frame = match inputs.value.map(|value| value.0) {
            Some(InputValue::Float(value)) => {
                let layout = context
                    .render_layout
                    .clone()
                    .unwrap_or_else(|| layout_from_shape(&[1], "colorize"));
                let color = sample_gradient_hsv(&self.gradient, value.clamp(0.0, 1.0));
                Some(ColorFrame {
                    pixels: vec![color; layout.pixel_count],
                    layout,
                })
            }
            Some(InputValue::FloatTensor(tensor)) => Some(colorize_tensor(&tensor, &self.gradient)),
            Some(_) | None => None,
        };

        Ok(TypedNodeEvaluation::from_outputs(ColorizeOutputs { frame }))
    }
}

fn colorize_tensor(tensor: &FloatTensor, gradient: &ColorGradient) -> ColorFrame {
    let layout = layout_from_shape(&tensor.shape, "colorize");
    ColorFrame {
        layout,
        pixels: tensor
            .values
            .iter()
            .map(|value| sample_gradient_hsv(gradient, value.clamp(0.0, 1.0)))
            .collect(),
    }
}

fn default_gradient() -> ColorGradient {
    ColorGradient {
        stops: vec![
            stop(0.0, 1.0, 0.0, 0.0),
            stop(0.2, 1.0, 0.5, 0.0),
            stop(0.4, 1.0, 1.0, 0.0),
            stop(0.6, 0.0, 1.0, 0.0),
            stop(0.8, 0.0, 0.4, 1.0),
            stop(1.0, 0.7, 0.0, 1.0),
        ],
    }
}

fn stop(position: f32, r: f32, g: f32, b: f32) -> shared::ColorGradientStop {
    shared::ColorGradientStop {
        position,
        color: RgbaColor { r, g, b, a: 1.0 },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_runtime::{NodeEvaluationContext, RuntimeNode};

    fn context() -> NodeEvaluationContext {
        NodeEvaluationContext {
            graph_id: "test-graph".to_owned(),
            graph_name: "Test Graph".to_owned(),
            elapsed_seconds: 0.0,
            render_layout: None,
        }
    }

    #[test]
    fn colorizes_tensor_element_wise() {
        let mut node = ColorizeNode::default();
        let evaluation = node
            .evaluate(
                &context(),
                ColorizeInputs {
                    value: Some(AnyInputValue(InputValue::FloatTensor(FloatTensor {
                        shape: vec![1, 2],
                        values: vec![0.0, 1.0],
                    }))),
                },
            )
            .expect("colorize evaluation should succeed");

        let output = evaluation.outputs.frame.expect("frame output");
        assert_eq!(output.layout.width, Some(2));
        assert_eq!(output.layout.height, Some(1));
        assert_eq!(output.pixels.len(), 2);
        assert!(output.pixels[0].r > 0.95);
        assert!(output.pixels[0].g < 0.05);
        assert!(output.pixels[1].b > 0.95);
    }

    #[test]
    fn colorizes_scalar_across_render_layout() {
        let mut node = ColorizeNode::default();
        let evaluation = node
            .evaluate(
                &NodeEvaluationContext {
                    render_layout: Some(shared::LedLayout {
                        id: "panel".to_owned(),
                        pixel_count: 4,
                        width: Some(2),
                        height: Some(2),
                    }),
                    ..context()
                },
                ColorizeInputs {
                    value: Some(AnyInputValue(InputValue::Float(0.4))),
                },
            )
            .expect("colorize evaluation should succeed");

        let output = evaluation.outputs.frame.expect("frame output");
        assert_eq!(output.pixels.len(), 4);
        assert!(
            output
                .pixels
                .windows(2)
                .all(|window| window[0] == window[1])
        );
    }
}
