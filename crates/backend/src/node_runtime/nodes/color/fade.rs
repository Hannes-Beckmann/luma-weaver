use anyhow::{Result, bail};
use shared::{ColorFrame, FloatTensor, InputValue, LedLayout, RgbaColor};

use crate::node_runtime::tensor::coerce_float_tensor;
use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct FadeNode {
    last_elapsed_seconds: Option<f64>,
}

impl RuntimeNodeFromParameters for FadeNode {}

#[derive(Clone)]
pub(crate) struct FadeInputs {
    value: AnyInputValue,
    decay: AnyInputValue,
}

crate::node_runtime::impl_runtime_inputs!(FadeInputs {
    value = AnyInputValue(InputValue::Color(RgbaColor {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    })),
    decay = AnyInputValue(InputValue::Float(0.0)),
});

pub(crate) struct FadeOutputs {
    value: InputValue,
}

crate::node_runtime::impl_runtime_outputs!(FadeOutputs { value });

impl RuntimeNode for FadeNode {
    type Inputs = FadeInputs;
    type Outputs = FadeOutputs;

    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let value = inputs.value.0;
        let decay = inputs.decay.0;
        let dt = self.delta_seconds(context.elapsed_seconds);

        let output = match value {
            InputValue::Color(mut color) => {
                color.a = (color.a * scalar_decay_multiplier(&decay, dt)?).clamp(0.0, 1.0);
                InputValue::Color(color)
            }
            InputValue::ColorFrame(mut frame) => {
                let tensor = coerce_float_tensor(&decay, &shape_from_layout(&frame.layout))?;
                apply_frame_fade(&mut frame, &tensor, dt)?;
                InputValue::ColorFrame(frame)
            }
            other => bail!(
                "fade expects Color or ColorFrame input, got {:?}",
                other.value_kind()
            ),
        };

        Ok(TypedNodeEvaluation::from_outputs(FadeOutputs {
            value: output,
        }))
    }
}

impl FadeNode {
    fn delta_seconds(&mut self, elapsed_seconds: f64) -> f32 {
        let dt = match self.last_elapsed_seconds {
            Some(last_elapsed_seconds) if elapsed_seconds >= last_elapsed_seconds => {
                (elapsed_seconds - last_elapsed_seconds) as f32
            }
            _ => 0.0,
        };
        self.last_elapsed_seconds = Some(elapsed_seconds);
        dt
    }
}

fn scalar_decay_multiplier(value: &InputValue, dt: f32) -> Result<f32> {
    match value {
        InputValue::Float(value) => Ok((-(value.max(0.0) * dt)).exp()),
        InputValue::FloatTensor(tensor) => {
            Ok((-(tensor.values.first().copied().unwrap_or(0.0)).max(0.0) * dt).exp())
        }
        _ => bail!("fade decay expects Float or FloatTensor"),
    }
}

fn apply_frame_fade(frame: &mut ColorFrame, decay: &FloatTensor, dt: f32) -> Result<()> {
    if frame.pixels.len() != decay.values.len() {
        bail!("fade decay length does not match frame pixels");
    }

    for (pixel, decay) in frame.pixels.iter_mut().zip(&decay.values) {
        pixel.a = (pixel.a * (-(decay.max(0.0) * dt)).exp()).clamp(0.0, 1.0);
    }
    Ok(())
}

fn shape_from_layout(layout: &LedLayout) -> Vec<usize> {
    match (layout.width, layout.height) {
        (Some(width), Some(height)) => vec![height, width],
        _ if layout.pixel_count > 0 => vec![layout.pixel_count],
        _ => vec![0],
    }
}

#[cfg(test)]
mod tests {
    use shared::{InputValue, RgbaColor};

    use crate::node_runtime::{AnyInputValue, NodeEvaluationContext, RuntimeNode};

    use super::{FadeInputs, FadeNode};

    #[test]
    fn applies_exponential_decay_to_alpha_only_using_dt() {
        let mut node = FadeNode::default();

        let first = RuntimeNode::evaluate(
            &mut node,
            &NodeEvaluationContext {
                elapsed_seconds: 0.0,
                render_layout: None,
            },
            FadeInputs {
                value: AnyInputValue(InputValue::Color(RgbaColor {
                    r: 0.4,
                    g: 0.5,
                    b: 0.6,
                    a: 1.0,
                })),
                decay: AnyInputValue(InputValue::Float(std::f32::consts::LN_2)),
            },
        )
        .expect("evaluate fade");
        assert_eq!(
            first.outputs.value,
            InputValue::Color(RgbaColor {
                r: 0.4,
                g: 0.5,
                b: 0.6,
                a: 1.0,
            })
        );

        let second = RuntimeNode::evaluate(
            &mut node,
            &NodeEvaluationContext {
                elapsed_seconds: 1.0,
                render_layout: None,
            },
            FadeInputs {
                value: AnyInputValue(InputValue::Color(RgbaColor {
                    r: 0.4,
                    g: 0.5,
                    b: 0.6,
                    a: 1.0,
                })),
                decay: AnyInputValue(InputValue::Float(std::f32::consts::LN_2)),
            },
        )
        .expect("evaluate fade");

        assert_eq!(
            second.outputs.value,
            InputValue::Color(RgbaColor {
                r: 0.4,
                g: 0.5,
                b: 0.6,
                a: 0.5,
            })
        );
    }
}
