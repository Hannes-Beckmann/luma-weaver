use anyhow::{Context, Result, bail};
use libblur::{
    AnisotropicRadius, BlurImageMut, EdgeMode, EdgeMode2D, FastBlurChannels, ThreadingPolicy,
    fast_gaussian_f32,
};
use shared::{ColorFrame, FloatTensor, InputValue, RgbaColor};

use crate::node_runtime::nodes::filter_utils::layout_dimensions;
use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, RuntimeOutputs,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct GaussianBlurNode;

impl RuntimeNodeFromParameters for GaussianBlurNode {}

pub(crate) struct GaussianBlurInputs {
    frame: Option<AnyInputValue>,
    radius: f32,
}

crate::node_runtime::impl_runtime_inputs!(GaussianBlurInputs {
    frame = None,
    radius = 2.0,
});

pub(crate) struct GaussianBlurOutputs {
    frame: Option<InputValue>,
}

impl RuntimeOutputs for GaussianBlurOutputs {
    fn into_runtime_outputs(self) -> anyhow::Result<std::collections::HashMap<String, InputValue>> {
        let mut outputs = std::collections::HashMap::new();
        if let Some(value) = self.frame {
            outputs.insert("frame".to_owned(), value);
        }
        Ok(outputs)
    }
}

impl RuntimeNode for GaussianBlurNode {
    type Inputs = GaussianBlurInputs;
    type Outputs = GaussianBlurOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let Some(value) = inputs.frame else {
            return Ok(TypedNodeEvaluation::from_outputs(GaussianBlurOutputs {
                frame: None,
            }));
        };

        let radius = inputs.radius.round().clamp(0.0, 255.0) as u32;
        let output = match value.0 {
            InputValue::ColorFrame(frame) => InputValue::ColorFrame(blur_frame(frame, radius)?),
            InputValue::MappedFrame(frame) => InputValue::MappedFrame(blur_frame(frame, radius)?),
            InputValue::FloatTensor(tensor) => {
                InputValue::FloatTensor(blur_tensor(tensor, radius)?)
            }
            other => bail!(
                "gaussian blur expects ColorFrame, MappedFrame, or FloatTensor input, got {:?}",
                other.value_kind()
            ),
        };

        Ok(TypedNodeEvaluation::from_outputs(GaussianBlurOutputs {
            frame: Some(output),
        }))
    }
}

fn blur_frame(frame: ColorFrame, radius: u32) -> Result<ColorFrame> {
    if radius == 0 || frame.pixels.is_empty() {
        return Ok(frame);
    }

    let (width, height) = layout_dimensions(&frame.layout);
    let mut pixels = frame_to_rgba32f(&frame);
    blur_plane(
        &mut pixels,
        width,
        height,
        FastBlurChannels::Channels4,
        radius,
    )
    .context("apply gaussian blur")?;
    let pixels = rgba32f_to_frame_pixels(&pixels)
        .context("convert blurred rgba output back into frame pixels")?;

    Ok(ColorFrame {
        layout: frame.layout,
        pixels,
    })
}

fn blur_tensor(tensor: FloatTensor, radius: u32) -> Result<FloatTensor> {
    let (width, height) = tensor_dimensions(&tensor)?;
    if radius == 0 || tensor.values.is_empty() {
        return Ok(tensor);
    }

    let mut values = normalized_tensor_values(&tensor, width * height);
    blur_plane(&mut values, width, height, FastBlurChannels::Plane, radius)
        .context("apply gaussian blur to tensor")?;

    Ok(FloatTensor {
        shape: tensor.shape,
        values,
    })
}

fn blur_plane(
    values: &mut [f32],
    width: usize,
    height: usize,
    channels: FastBlurChannels,
    radius: u32,
) -> Result<()> {
    let mut image = BlurImageMut::borrow(values, width as u32, height as u32, channels);
    fast_gaussian_f32(
        &mut image,
        AnisotropicRadius::new(radius),
        ThreadingPolicy::Adaptive,
        EdgeMode2D::new(EdgeMode::Clamp),
    )?;
    Ok(())
}

fn frame_to_rgba32f(frame: &ColorFrame) -> Vec<f32> {
    let mut bytes = Vec::with_capacity(frame.pixels.len() * 4);
    for pixel in &frame.pixels {
        bytes.push(pixel.r);
        bytes.push(pixel.g);
        bytes.push(pixel.b);
        bytes.push(pixel.a);
    }
    bytes
}

fn rgba32f_to_frame_pixels(bytes: &[f32]) -> Result<Vec<RgbaColor>> {
    anyhow::ensure!(
        bytes.len() % 4 == 0,
        "expected rgba float output, got {} values",
        bytes.len()
    );

    Ok(bytes
        .chunks_exact(4)
        .map(|rgba| RgbaColor {
            r: rgba[0].clamp(0.0, 1.0),
            g: rgba[1].clamp(0.0, 1.0),
            b: rgba[2].clamp(0.0, 1.0),
            a: rgba[3].clamp(0.0, 1.0),
        })
        .collect())
}

fn tensor_dimensions(tensor: &FloatTensor) -> Result<(usize, usize)> {
    match tensor.shape.as_slice() {
        [] => Ok((1, 1)),
        [width] => Ok(((*width).max(1), 1)),
        [height, width] => Ok(((*width).max(1), (*height).max(1))),
        shape => bail!(
            "gaussian blur only supports 1D or 2D tensors, got shape {:?}",
            shape
        ),
    }
}

fn normalized_tensor_values(tensor: &FloatTensor, expected_len: usize) -> Vec<f32> {
    let mut values = tensor.values.clone();
    values.resize(expected_len, 0.0);
    values.truncate(expected_len);
    values
}

#[cfg(test)]
mod tests {
    use shared::{FloatTensor, InputValue};

    use super::{GaussianBlurInputs, GaussianBlurNode};
    use crate::node_runtime::{AnyInputValue, NodeEvaluationContext, RuntimeNode};

    fn context() -> NodeEvaluationContext {
        NodeEvaluationContext {
            graph_id: "test-graph".to_owned(),
            graph_name: "Test Graph".to_owned(),
            elapsed_seconds: 0.0,
            render_layout: None,
            graph_layout_assets: Default::default(),
        }
    }

    #[test]
    fn tensor_input_produces_tensor_output() {
        let mut node = GaussianBlurNode;

        let evaluation = node
            .evaluate(
                &context(),
                GaussianBlurInputs {
                    frame: Some(AnyInputValue(InputValue::FloatTensor(FloatTensor {
                        shape: vec![5, 5],
                        values: vec![
                            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0,
                            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                        ],
                    }))),
                    radius: 3.0,
                },
            )
            .expect("gaussian blur evaluation should succeed");

        let Some(InputValue::FloatTensor(tensor)) = evaluation.outputs.frame else {
            panic!("expected float tensor output");
        };
        assert_eq!(tensor.shape, vec![5, 5]);
        assert_eq!(tensor.values.len(), 25);
        assert!(tensor.values.iter().all(|value| value.is_finite()));
        assert!(tensor.values[12] < 1.0);
        assert!(
            tensor
                .values
                .iter()
                .any(|value| *value > 0.0 && *value < 1.0)
        );
    }

    #[test]
    fn zero_radius_preserves_tensor_values() {
        let mut node = GaussianBlurNode;
        let tensor = FloatTensor {
            shape: vec![2, 2],
            values: vec![0.0, 1.0, 0.5, 0.25],
        };

        let evaluation = node
            .evaluate(
                &context(),
                GaussianBlurInputs {
                    frame: Some(AnyInputValue(InputValue::FloatTensor(tensor.clone()))),
                    radius: 0.0,
                },
            )
            .expect("gaussian blur evaluation should succeed");

        assert_eq!(
            evaluation.outputs.frame,
            Some(InputValue::FloatTensor(tensor))
        );
    }
}
