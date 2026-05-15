use anyhow::Result;
use shared::{ColorFrame, InputValue, RgbaColor};

use crate::node_runtime::nodes::filter_utils::{clamped_index, layout_dimensions};
use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, RuntimeOutputs,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct MedianFilterNode;

impl RuntimeNodeFromParameters for MedianFilterNode {}

pub(crate) struct MedianFilterInputs {
    frame: Option<AnyInputValue>,
    radius: f32,
}

crate::node_runtime::impl_runtime_inputs!(MedianFilterInputs {
    frame = None,
    radius = 1.0,
});

pub(crate) struct MedianFilterOutputs {
    frame: Option<InputValue>,
}

impl RuntimeOutputs for MedianFilterOutputs {
    fn into_runtime_outputs(self) -> anyhow::Result<std::collections::HashMap<String, InputValue>> {
        let mut outputs = std::collections::HashMap::new();
        if let Some(frame) = self.frame {
            outputs.insert("frame".to_owned(), frame);
        }
        Ok(outputs)
    }
}

impl RuntimeNode for MedianFilterNode {
    type Inputs = MedianFilterInputs;
    type Outputs = MedianFilterOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let Some(frame) = inputs.frame.map(|value| value.0) else {
            return Ok(TypedNodeEvaluation::from_outputs(MedianFilterOutputs {
                frame: None,
            }));
        };
        let kind = frame.value_kind();
        let frame = frame
            .as_frame()
            .expect("median filter only accepts frame values")
            .clone();

        let radius = inputs.radius.round().clamp(0.0, 16.0) as usize;
        if radius == 0 || frame.pixels.is_empty() {
            return Ok(TypedNodeEvaluation::from_outputs(MedianFilterOutputs {
                frame: Some(InputValue::from_frame_kind(kind, frame).expect("frame kind")),
            }));
        }

        let (width, height) = layout_dimensions(&frame.layout);
        let window_capacity = (radius * 2 + 1) * (radius * 2 + 1);
        let mut reds = Vec::with_capacity(window_capacity);
        let mut greens = Vec::with_capacity(window_capacity);
        let mut blues = Vec::with_capacity(window_capacity);
        let mut alphas = Vec::with_capacity(window_capacity);
        let mut pixels = Vec::with_capacity(frame.pixels.len());

        for y in 0..height {
            for x in 0..width {
                reds.clear();
                greens.clear();
                blues.clear();
                alphas.clear();

                for sample_y in y.saturating_sub(radius)..=(y + radius).min(height - 1) {
                    for sample_x in x.saturating_sub(radius)..=(x + radius).min(width - 1) {
                        let pixel = frame.pixels
                            [clamped_index(sample_x, sample_y, width, height, frame.pixels.len())];
                        reds.push(pixel.r);
                        greens.push(pixel.g);
                        blues.push(pixel.b);
                        alphas.push(pixel.a);
                    }
                }

                pixels.push(RgbaColor {
                    r: median_channel(&mut reds),
                    g: median_channel(&mut greens),
                    b: median_channel(&mut blues),
                    a: median_channel(&mut alphas),
                });
                if pixels.len() == frame.pixels.len() {
                    break;
                }
            }
            if pixels.len() == frame.pixels.len() {
                break;
            }
        }

        Ok(TypedNodeEvaluation::from_outputs(MedianFilterOutputs {
            frame: Some(
                InputValue::from_frame_kind(
                    kind,
                    ColorFrame {
                        layout: frame.layout,
                        pixels,
                    },
                )
                .expect("frame kind"),
            ),
        }))
    }
}

fn median_channel(values: &mut [f32]) -> f32 {
    values.sort_by(|a, b| a.total_cmp(b));
    let middle = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[middle - 1] + values[middle]) * 0.5
    } else {
        values[middle]
    }
}
