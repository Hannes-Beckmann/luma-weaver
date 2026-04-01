use std::collections::VecDeque;

use anyhow::Result;
use shared::{ColorFrame, InputValue, RgbaColor};

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, RuntimeOutputs,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct MovingAverageNode {
    history: VecDeque<Vec<RgbaColor>>,
    running_sum: Vec<[f32; 4]>,
    cached_layout_id: Option<String>,
    cached_pixel_count: usize,
}

impl RuntimeNodeFromParameters for MovingAverageNode {}

pub(crate) struct MovingAverageInputs {
    frame: Option<ColorFrame>,
    window_size: f32,
}

crate::node_runtime::impl_runtime_inputs!(MovingAverageInputs {
    frame = None,
    window_size = 4.0,
});

pub(crate) struct MovingAverageOutputs {
    frame: Option<ColorFrame>,
}

impl RuntimeOutputs for MovingAverageOutputs {
    fn into_runtime_outputs(self) -> anyhow::Result<std::collections::HashMap<String, InputValue>> {
        let mut outputs = std::collections::HashMap::new();
        if let Some(frame) = self.frame {
            outputs.insert("frame".to_owned(), InputValue::ColorFrame(frame));
        }
        Ok(outputs)
    }
}

impl RuntimeNode for MovingAverageNode {
    type Inputs = MovingAverageInputs;
    type Outputs = MovingAverageOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let Some(frame) = inputs.frame else {
            self.history.clear();
            self.running_sum.clear();
            self.cached_layout_id = None;
            self.cached_pixel_count = 0;
            return Ok(TypedNodeEvaluation::from_outputs(MovingAverageOutputs {
                frame: None,
            }));
        };

        let window_size = inputs.window_size.round().clamp(1.0, 240.0) as usize;
        self.ensure_layout(&frame);
        self.push_frame(&frame.pixels);
        while self.history.len() > window_size {
            self.pop_oldest();
        }

        let divisor = self.history.len().max(1) as f32;
        let mut pixels = Vec::with_capacity(frame.pixels.len());
        for sum in &self.running_sum {
            pixels.push(RgbaColor {
                r: (sum[0] / divisor).clamp(0.0, 1.0),
                g: (sum[1] / divisor).clamp(0.0, 1.0),
                b: (sum[2] / divisor).clamp(0.0, 1.0),
                a: (sum[3] / divisor).clamp(0.0, 1.0),
            });
        }

        Ok(TypedNodeEvaluation::from_outputs(MovingAverageOutputs {
            frame: Some(ColorFrame {
                layout: frame.layout,
                pixels,
            }),
        }))
    }
}

impl MovingAverageNode {
    fn ensure_layout(&mut self, frame: &ColorFrame) {
        if self.cached_layout_id.as_deref() == Some(frame.layout.id.as_str())
            && self.cached_pixel_count == frame.pixels.len()
        {
            return;
        }

        self.history.clear();
        self.running_sum = vec![[0.0; 4]; frame.pixels.len()];
        self.cached_layout_id = Some(frame.layout.id.clone());
        self.cached_pixel_count = frame.pixels.len();
    }

    fn push_frame(&mut self, pixels: &[RgbaColor]) {
        for (sum, pixel) in self.running_sum.iter_mut().zip(pixels.iter()) {
            sum[0] += pixel.r;
            sum[1] += pixel.g;
            sum[2] += pixel.b;
            sum[3] += pixel.a;
        }
        self.history.push_back(pixels.to_vec());
    }

    fn pop_oldest(&mut self) {
        let Some(old_pixels) = self.history.pop_front() else {
            return;
        };
        for (sum, pixel) in self.running_sum.iter_mut().zip(old_pixels.iter()) {
            sum[0] -= pixel.r;
            sum[1] -= pixel.g;
            sum[2] -= pixel.b;
            sum[3] -= pixel.a;
        }
    }
}
