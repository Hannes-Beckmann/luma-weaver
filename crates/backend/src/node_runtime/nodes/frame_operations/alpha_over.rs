use anyhow::Result;
use shared::{ColorFrame, InputValue, RgbaColor};

use crate::node_runtime::tensor::{coerce_color_frame, infer_broadcast_shape, layout_from_shape};
use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct AlphaOverNode;

impl RuntimeNodeFromParameters for AlphaOverNode {}

#[derive(Clone)]
pub(crate) struct AlphaOverInputs {
    foreground: AnyInputValue,
    background: AnyInputValue,
}

crate::node_runtime::impl_runtime_inputs!(AlphaOverInputs {
    foreground = default_transparent_value(),
    background = default_transparent_value(),
});

pub(crate) struct AlphaOverOutputs {
    color: ColorFrame,
}

crate::node_runtime::impl_runtime_outputs!(AlphaOverOutputs { color });

impl RuntimeNode for AlphaOverNode {
    type Inputs = AlphaOverInputs;
    type Outputs = AlphaOverOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let foreground = inputs.foreground.0;
        let background = inputs.background.0;

        let shape = infer_broadcast_shape(&[&foreground, &background])?;
        let fallback_layout = layout_from_shape(&shape, "alpha_over");
        let foreground = coerce_color_frame(&foreground, &shape, &fallback_layout.id)?;
        let background = coerce_color_frame(&background, &shape, &fallback_layout.id)?;

        let pixels = foreground
            .pixels
            .iter()
            .zip(&background.pixels)
            .map(|(foreground, background)| alpha_over(*foreground, *background))
            .collect();

        Ok(TypedNodeEvaluation::from_outputs(AlphaOverOutputs {
            color: ColorFrame {
                layout: foreground.layout,
                pixels,
            },
        }))
    }
}

fn default_transparent_value() -> AnyInputValue {
    AnyInputValue(InputValue::Color(RgbaColor {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    }))
}

fn alpha_over(foreground: RgbaColor, background: RgbaColor) -> RgbaColor {
    let foreground_alpha = foreground.a.clamp(0.0, 1.0);
    let background_alpha = background.a.clamp(0.0, 1.0);
    let out_alpha = foreground_alpha + background_alpha * (1.0 - foreground_alpha);

    if out_alpha <= f32::EPSILON {
        return RgbaColor {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };
    }

    let foreground_weight = foreground_alpha;
    let background_weight = background_alpha * (1.0 - foreground_alpha);

    RgbaColor {
        r: ((foreground.r * foreground_weight) + (background.r * background_weight)) / out_alpha,
        g: ((foreground.g * foreground_weight) + (background.g * background_weight)) / out_alpha,
        b: ((foreground.b * foreground_weight) + (background.b * background_weight)) / out_alpha,
        a: out_alpha,
    }
}

#[cfg(test)]
mod tests {
    use shared::{InputValue, RgbaColor};

    use crate::node_runtime::{AnyInputValue, NodeEvaluationContext, RuntimeNode};

    use super::{AlphaOverInputs, AlphaOverNode};

    #[test]
    fn composites_foreground_over_background() {
        let mut node = AlphaOverNode;
        let evaluation = RuntimeNode::evaluate(
            &mut node,
            &NodeEvaluationContext {
                graph_id: "test-graph".to_owned(),
                graph_name: "Test Graph".to_owned(),
                elapsed_seconds: 0.0,
                render_layout: None,
            },
            AlphaOverInputs {
                foreground: AnyInputValue(InputValue::Color(RgbaColor {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.5,
                })),
                background: AnyInputValue(InputValue::Color(RgbaColor {
                    r: 0.0,
                    g: 0.0,
                    b: 1.0,
                    a: 1.0,
                })),
            },
        )
        .expect("evaluate alpha over");

        assert_eq!(
            evaluation.outputs.color.pixels[0],
            RgbaColor {
                r: 0.5,
                g: 0.0,
                b: 0.5,
                a: 1.0,
            }
        );
    }
}
