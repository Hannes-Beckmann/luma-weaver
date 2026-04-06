use anyhow::Result;
use shared::{ColorFrame, InputValue, RgbaColor};

use crate::node_runtime::nodes::filter_utils::{clamped_index, layout_dimensions};
use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeOutputs, TypedNodeEvaluation,
};

pub(crate) struct LaplacianFilterNode {
    strength: f32,
    absolute_value: bool,
    filter_alpha: bool,
}

impl Default for LaplacianFilterNode {
    fn default() -> Self {
        Self {
            strength: 1.0,
            absolute_value: true,
            filter_alpha: false,
        }
    }
}

crate::node_runtime::impl_runtime_parameters!(LaplacianFilterNode {
    strength: f64 => |value| crate::node_runtime::clamp_f64_to_f32(value, 0.0, 8.0), default 1.0f32,
    absolute_value: bool = true,
    filter_alpha: bool = false,
});

pub(crate) struct LaplacianFilterInputs {
    frame: Option<ColorFrame>,
}

crate::node_runtime::impl_runtime_inputs!(LaplacianFilterInputs {
    frame = None,
});

pub(crate) struct LaplacianFilterOutputs {
    frame: Option<ColorFrame>,
}

impl RuntimeOutputs for LaplacianFilterOutputs {
    fn into_runtime_outputs(self) -> anyhow::Result<std::collections::HashMap<String, InputValue>> {
        let mut outputs = std::collections::HashMap::new();
        if let Some(frame) = self.frame {
            outputs.insert("frame".to_owned(), InputValue::ColorFrame(frame));
        }
        Ok(outputs)
    }
}

impl RuntimeNode for LaplacianFilterNode {
    type Inputs = LaplacianFilterInputs;
    type Outputs = LaplacianFilterOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let Some(frame) = inputs.frame else {
            return Ok(TypedNodeEvaluation::from_outputs(LaplacianFilterOutputs {
                frame: None,
            }));
        };

        if frame.pixels.is_empty() {
            return Ok(TypedNodeEvaluation::from_outputs(LaplacianFilterOutputs {
                frame: Some(frame),
            }));
        }

        let (width, height) = layout_dimensions(&frame.layout);
        let mut pixels = Vec::with_capacity(frame.pixels.len());
        for y in 0..height {
            for x in 0..width {
                let center = sample_pixel(&frame, x, y, width, height);
                let left = sample_pixel(&frame, x.saturating_sub(1), y, width, height);
                let right = sample_pixel(&frame, x.saturating_add(1), y, width, height);
                let up = sample_pixel(&frame, x, y.saturating_sub(1), width, height);
                let down = sample_pixel(&frame, x, y.saturating_add(1), width, height);

                pixels.push(RgbaColor {
                    r: shape_channel(
                        laplacian_response(center.r, left.r, right.r, up.r, down.r),
                        self.strength,
                        self.absolute_value,
                    ),
                    g: shape_channel(
                        laplacian_response(center.g, left.g, right.g, up.g, down.g),
                        self.strength,
                        self.absolute_value,
                    ),
                    b: shape_channel(
                        laplacian_response(center.b, left.b, right.b, up.b, down.b),
                        self.strength,
                        self.absolute_value,
                    ),
                    a: if self.filter_alpha {
                        shape_channel(
                            laplacian_response(center.a, left.a, right.a, up.a, down.a),
                            self.strength,
                            self.absolute_value,
                        )
                    } else {
                        center.a
                    },
                });

                if pixels.len() == frame.pixels.len() {
                    break;
                }
            }
            if pixels.len() == frame.pixels.len() {
                break;
            }
        }

        Ok(TypedNodeEvaluation {
            outputs: LaplacianFilterOutputs {
                frame: Some(ColorFrame {
                    layout: frame.layout,
                    pixels,
                }),
            },
            frontend_updates: Vec::new(),
            diagnostics: Vec::new(),
        })
    }
}

fn sample_pixel(frame: &ColorFrame, x: usize, y: usize, width: usize, height: usize) -> RgbaColor {
    frame.pixels[clamped_index(x, y, width, height, frame.pixels.len())]
}

fn laplacian_response(center: f32, left: f32, right: f32, up: f32, down: f32) -> f32 {
    (4.0 * center) - left - right - up - down
}

fn shape_channel(response: f32, strength: f32, absolute_value: bool) -> f32 {
    let shaped = if absolute_value {
        response.abs() * strength
    } else {
        0.5 + response * strength
    };
    shaped.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::Value as JsonValue;
    use shared::{ColorFrame, LedLayout, RgbaColor};

    use super::{LaplacianFilterInputs, LaplacianFilterNode};
    use crate::node_runtime::{NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters};

    fn context() -> NodeEvaluationContext {
        NodeEvaluationContext {
            graph_id: "test-graph".to_owned(),
            graph_name: "Test Graph".to_owned(),
            elapsed_seconds: 0.0,
            render_layout: None,
        }
    }

    fn layout(width: usize, height: usize) -> LedLayout {
        LedLayout {
            id: "laplacian-test".to_owned(),
            pixel_count: width * height,
            width: Some(width),
            height: Some(height),
        }
    }

    fn grayscale(value: f32) -> RgbaColor {
        RgbaColor {
            r: value,
            g: value,
            b: value,
            a: 1.0,
        }
    }

    #[test]
    fn missing_input_frame_produces_no_output() {
        let mut node = LaplacianFilterNode::default();
        let evaluation = node
            .evaluate(&context(), LaplacianFilterInputs { frame: None })
            .expect("laplacian filter evaluation should succeed");

        assert!(evaluation.outputs.frame.is_none());
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn flat_frame_produces_zero_response_in_absolute_mode() {
        let mut node = LaplacianFilterNode::default();
        let frame = ColorFrame {
            layout: layout(3, 1),
            pixels: vec![grayscale(0.4), grayscale(0.4), grayscale(0.4)],
        };

        let evaluation = node
            .evaluate(&context(), LaplacianFilterInputs { frame: Some(frame) })
            .expect("laplacian filter evaluation should succeed");

        let pixels = &evaluation.outputs.frame.expect("filtered frame").pixels;
        assert!(
            pixels
                .iter()
                .all(|pixel| pixel.r.abs() < 1e-6 && pixel.g.abs() < 1e-6 && pixel.b.abs() < 1e-6)
        );
        assert!(pixels.iter().all(|pixel| (pixel.a - 1.0).abs() < 1e-6));
    }

    #[test]
    fn bright_center_pixel_creates_edge_response() {
        let mut node = LaplacianFilterNode::default();
        let frame = ColorFrame {
            layout: layout(3, 1),
            pixels: vec![grayscale(0.0), grayscale(1.0), grayscale(0.0)],
        };

        let evaluation = node
            .evaluate(&context(), LaplacianFilterInputs { frame: Some(frame) })
            .expect("laplacian filter evaluation should succeed");

        let pixels = &evaluation.outputs.frame.expect("filtered frame").pixels;
        assert_eq!(pixels[1].r, 1.0);
        assert_eq!(pixels[0].r, 1.0);
        assert_eq!(pixels[2].r, 1.0);
    }

    #[test]
    fn signed_mode_biases_zero_response_to_midgray() {
        let mut parameters = HashMap::new();
        parameters.insert("absolute_value".to_owned(), JsonValue::from(false));
        let mut node = LaplacianFilterNode::from_parameters(&parameters).node;
        let frame = ColorFrame {
            layout: layout(2, 1),
            pixels: vec![grayscale(0.4), grayscale(0.4)],
        };

        let evaluation = node
            .evaluate(&context(), LaplacianFilterInputs { frame: Some(frame) })
            .expect("laplacian filter evaluation should succeed");

        let pixels = &evaluation.outputs.frame.expect("filtered frame").pixels;
        assert!(pixels.iter().all(|pixel| (pixel.r - 0.5).abs() < 1e-6));
        assert!(pixels.iter().all(|pixel| (pixel.a - 1.0).abs() < 1e-6));
    }

    #[test]
    fn clamped_borders_match_one_dimensional_strip_behavior() {
        let mut node = LaplacianFilterNode::default();
        let frame = ColorFrame {
            layout: layout(4, 1),
            pixels: vec![
                grayscale(0.0),
                grayscale(0.25),
                grayscale(0.75),
                grayscale(1.0),
            ],
        };

        let evaluation = node
            .evaluate(&context(), LaplacianFilterInputs { frame: Some(frame) })
            .expect("laplacian filter evaluation should succeed");

        let pixels = &evaluation.outputs.frame.expect("filtered frame").pixels;
        assert!((pixels[0].r - 0.25).abs() < 1e-6);
        assert!((pixels[1].r - 0.25).abs() < 1e-6);
        assert!((pixels[2].r - 0.25).abs() < 1e-6);
        assert!((pixels[3].r - 0.25).abs() < 1e-6);
    }

    #[test]
    fn alpha_is_only_filtered_when_enabled() {
        let mut parameters = HashMap::new();
        parameters.insert("filter_alpha".to_owned(), JsonValue::from(true));
        let mut node = LaplacianFilterNode::from_parameters(&parameters).node;
        let frame = ColorFrame {
            layout: layout(3, 1),
            pixels: vec![
                RgbaColor {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.0,
                },
                RgbaColor {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                },
                RgbaColor {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.0,
                },
            ],
        };

        let evaluation = node
            .evaluate(&context(), LaplacianFilterInputs { frame: Some(frame) })
            .expect("laplacian filter evaluation should succeed");

        let pixels = &evaluation.outputs.frame.expect("filtered frame").pixels;
        assert_eq!(pixels[0].a, 1.0);
        assert_eq!(pixels[1].a, 1.0);
        assert_eq!(pixels[2].a, 1.0);
    }
}
