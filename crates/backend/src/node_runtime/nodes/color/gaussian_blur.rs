use anyhow::{Context, Result};
use libblur::{
    AnisotropicRadius, BlurImageMut, EdgeMode, EdgeMode2D, FastBlurChannels, ThreadingPolicy,
    fast_gaussian_f32,
};
use shared::{ColorFrame, InputValue, RgbaColor};

use crate::node_runtime::nodes::color::filter_utils::layout_dimensions;
use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, RuntimeOutputs,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct GaussianBlurNode;

impl RuntimeNodeFromParameters for GaussianBlurNode {}

pub(crate) struct GaussianBlurInputs {
    frame: Option<ColorFrame>,
    radius: f32,
}

crate::node_runtime::impl_runtime_inputs!(GaussianBlurInputs {
    frame = None,
    radius = 2.0,
});

pub(crate) struct GaussianBlurOutputs {
    frame: Option<ColorFrame>,
}

impl RuntimeOutputs for GaussianBlurOutputs {
    fn into_runtime_outputs(self) -> anyhow::Result<std::collections::HashMap<String, InputValue>> {
        let mut outputs = std::collections::HashMap::new();
        if let Some(frame) = self.frame {
            outputs.insert("frame".to_owned(), InputValue::ColorFrame(frame));
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
        let Some(frame) = inputs.frame else {
            return Ok(TypedNodeEvaluation::from_outputs(GaussianBlurOutputs {
                frame: None,
            }));
        };

        let radius = inputs.radius.round().clamp(0.0, 255.0) as u32;
        if radius == 0 || frame.pixels.is_empty() {
            return Ok(TypedNodeEvaluation::from_outputs(GaussianBlurOutputs {
                frame: Some(frame),
            }));
        }
        let (width, height) = layout_dimensions(&frame.layout);
        let mut pixels = frame_to_rgba32f(&frame);
        let mut image = BlurImageMut::borrow(
            &mut pixels,
            width as u32,
            height as u32,
            FastBlurChannels::Channels4,
        );
        fast_gaussian_f32(
            &mut image,
            AnisotropicRadius::new(radius),
            ThreadingPolicy::Adaptive,
            EdgeMode2D::new(EdgeMode::Clamp),
        )
        .context("apply gaussian blur")?;
        let pixels = rgba32f_to_frame_pixels(image.data.borrow())
            .context("convert blurred rgba output back into frame pixels")?;

        Ok(TypedNodeEvaluation::from_outputs(GaussianBlurOutputs {
            frame: Some(ColorFrame {
                layout: frame.layout,
                pixels,
            }),
        }))
    }
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
