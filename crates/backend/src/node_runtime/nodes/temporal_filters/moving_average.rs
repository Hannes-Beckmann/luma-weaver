use std::collections::VecDeque;

use anyhow::Result;
use shared::{ColorFrame, FloatTensor, InputValue, LedLayout, RgbaColor};

use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, RuntimeOutputs,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct MovingAverageNode {
    history: VecDeque<InputValue>,
    running_sum: Option<RunningSum>,
}

impl RuntimeNodeFromParameters for MovingAverageNode {}

pub(crate) struct MovingAverageInputs {
    value: AnyInputValue,
    window_size: f32,
}

crate::node_runtime::impl_runtime_inputs!(MovingAverageInputs {
    value = AnyInputValue(InputValue::Float(0.0)),
    window_size = 4.0,
});

pub(crate) struct MovingAverageOutputs {
    value: InputValue,
}

impl RuntimeOutputs for MovingAverageOutputs {
    fn into_runtime_outputs(self) -> anyhow::Result<std::collections::HashMap<String, InputValue>> {
        let mut outputs = std::collections::HashMap::new();
        outputs.insert("value".to_owned(), self.value);
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
        let value = inputs.value.0;
        let window_size = inputs.window_size.round().clamp(1.0, 240.0) as usize;

        self.ensure_compatible_sum(&value);
        self.push_value(&value);
        while self.history.len() > window_size {
            self.pop_oldest();
        }

        let divisor = self.history.len().max(1) as f32;
        let averaged = self
            .running_sum
            .as_ref()
            .map(|sum| sum.average(divisor))
            .unwrap_or_else(|| value.clone());

        Ok(TypedNodeEvaluation::from_outputs(MovingAverageOutputs {
            value: averaged,
        }))
    }
}

impl MovingAverageNode {
    fn ensure_compatible_sum(&mut self, value: &InputValue) {
        let is_compatible = self
            .running_sum
            .as_ref()
            .is_some_and(|sum| sum.is_compatible(value));
        if is_compatible {
            return;
        }

        self.history.clear();
        self.running_sum = RunningSum::zero_for_value(value);
    }

    fn push_value(&mut self, value: &InputValue) {
        let Some(sum) = self.running_sum.as_mut() else {
            self.running_sum = RunningSum::from_value(value);
            self.history.push_back(value.clone());
            return;
        };
        sum.add(value);
        self.history.push_back(value.clone());
    }

    fn pop_oldest(&mut self) {
        let Some(oldest) = self.history.pop_front() else {
            return;
        };
        let Some(sum) = self.running_sum.as_mut() else {
            return;
        };
        sum.subtract(&oldest);
    }
}

enum RunningSum {
    Float(f32),
    Color([f32; 4]),
    Tensor {
        shape: Vec<usize>,
        values: Vec<f32>,
    },
    Frame {
        layout: LedLayout,
        values: Vec<[f32; 4]>,
    },
}

impl RunningSum {
    fn from_value(value: &InputValue) -> Option<Self> {
        match value {
            InputValue::Float(number) => Some(Self::Float(*number)),
            InputValue::String(_) => None,
            InputValue::Color(color) => Some(Self::Color([color.r, color.g, color.b, color.a])),
            InputValue::FloatTensor(tensor) => Some(Self::Tensor {
                shape: tensor.shape.clone(),
                values: tensor.values.clone(),
            }),
            InputValue::ColorFrame(frame) => Some(Self::Frame {
                layout: frame.layout.clone(),
                values: frame
                    .pixels
                    .iter()
                    .map(|pixel| [pixel.r, pixel.g, pixel.b, pixel.a])
                    .collect(),
            }),
            InputValue::LedLayout(_) => None,
        }
    }

    fn zero_for_value(value: &InputValue) -> Option<Self> {
        match value {
            InputValue::Float(_) => Some(Self::Float(0.0)),
            InputValue::String(_) => None,
            InputValue::Color(_) => Some(Self::Color([0.0; 4])),
            InputValue::FloatTensor(tensor) => Some(Self::Tensor {
                shape: tensor.shape.clone(),
                values: vec![0.0; tensor.values.len()],
            }),
            InputValue::ColorFrame(frame) => Some(Self::Frame {
                layout: frame.layout.clone(),
                values: vec![[0.0; 4]; frame.pixels.len()],
            }),
            InputValue::LedLayout(_) => None,
        }
    }

    fn is_compatible(&self, value: &InputValue) -> bool {
        match (self, value) {
            (Self::Float(_), InputValue::Float(_)) => true,
            (Self::Color(_), InputValue::Color(_)) => true,
            (Self::Tensor { shape, values }, InputValue::FloatTensor(tensor)) => {
                shape == &tensor.shape && values.len() == tensor.values.len()
            }
            (Self::Frame { layout, values }, InputValue::ColorFrame(frame)) => {
                layout.id == frame.layout.id
                    && layout.pixel_count == frame.layout.pixel_count
                    && layout.width == frame.layout.width
                    && layout.height == frame.layout.height
                    && values.len() == frame.pixels.len()
            }
            _ => false,
        }
    }

    fn add(&mut self, value: &InputValue) {
        match (self, value) {
            (Self::Float(sum), InputValue::Float(number)) => *sum += number,
            (Self::Color(sum), InputValue::Color(color)) => {
                sum[0] += color.r;
                sum[1] += color.g;
                sum[2] += color.b;
                sum[3] += color.a;
            }
            (Self::Tensor { values, .. }, InputValue::FloatTensor(tensor)) => {
                for (sum, value) in values.iter_mut().zip(tensor.values.iter().copied()) {
                    *sum += value;
                }
            }
            (Self::Frame { values, .. }, InputValue::ColorFrame(frame)) => {
                for (sum, pixel) in values.iter_mut().zip(frame.pixels.iter()) {
                    sum[0] += pixel.r;
                    sum[1] += pixel.g;
                    sum[2] += pixel.b;
                    sum[3] += pixel.a;
                }
            }
            _ => {}
        }
    }

    fn subtract(&mut self, value: &InputValue) {
        match (self, value) {
            (Self::Float(sum), InputValue::Float(number)) => *sum -= number,
            (Self::Color(sum), InputValue::Color(color)) => {
                sum[0] -= color.r;
                sum[1] -= color.g;
                sum[2] -= color.b;
                sum[3] -= color.a;
            }
            (Self::Tensor { values, .. }, InputValue::FloatTensor(tensor)) => {
                for (sum, value) in values.iter_mut().zip(tensor.values.iter().copied()) {
                    *sum -= value;
                }
            }
            (Self::Frame { values, .. }, InputValue::ColorFrame(frame)) => {
                for (sum, pixel) in values.iter_mut().zip(frame.pixels.iter()) {
                    sum[0] -= pixel.r;
                    sum[1] -= pixel.g;
                    sum[2] -= pixel.b;
                    sum[3] -= pixel.a;
                }
            }
            _ => {}
        }
    }

    fn average(&self, divisor: f32) -> InputValue {
        match self {
            Self::Float(sum) => InputValue::Float(*sum / divisor),
            Self::Color(sum) => InputValue::Color(RgbaColor {
                r: (sum[0] / divisor).clamp(0.0, 1.0),
                g: (sum[1] / divisor).clamp(0.0, 1.0),
                b: (sum[2] / divisor).clamp(0.0, 1.0),
                a: (sum[3] / divisor).clamp(0.0, 1.0),
            }),
            Self::Tensor { shape, values } => InputValue::FloatTensor(FloatTensor {
                shape: shape.clone(),
                values: values.iter().map(|value| *value / divisor).collect(),
            }),
            Self::Frame { layout, values } => InputValue::ColorFrame(ColorFrame {
                layout: layout.clone(),
                pixels: values
                    .iter()
                    .map(|sum| RgbaColor {
                        r: (sum[0] / divisor).clamp(0.0, 1.0),
                        g: (sum[1] / divisor).clamp(0.0, 1.0),
                        b: (sum[2] / divisor).clamp(0.0, 1.0),
                        a: (sum[3] / divisor).clamp(0.0, 1.0),
                    })
                    .collect(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> NodeEvaluationContext {
        NodeEvaluationContext {
            graph_id: "test-graph".to_owned(),
            graph_name: "Test Graph".to_owned(),
            elapsed_seconds: 0.0,
            render_layout: None,
        }
    }

    #[test]
    fn averages_float_values() {
        let mut node = MovingAverageNode::default();

        let first = node
            .evaluate(
                &context(),
                MovingAverageInputs {
                    value: AnyInputValue(InputValue::Float(2.0)),
                    window_size: 4.0,
                },
            )
            .expect("first float average");
        let second = node
            .evaluate(
                &context(),
                MovingAverageInputs {
                    value: AnyInputValue(InputValue::Float(4.0)),
                    window_size: 4.0,
                },
            )
            .expect("second float average");

        assert_eq!(first.outputs.value, InputValue::Float(2.0));
        assert_eq!(second.outputs.value, InputValue::Float(3.0));
    }

    #[test]
    fn averages_colors() {
        let mut node = MovingAverageNode::default();

        let output = node
            .evaluate(
                &context(),
                MovingAverageInputs {
                    value: AnyInputValue(InputValue::Color(RgbaColor {
                        r: 0.2,
                        g: 0.4,
                        b: 0.6,
                        a: 1.0,
                    })),
                    window_size: 2.0,
                },
            )
            .and_then(|_| {
                node.evaluate(
                    &context(),
                    MovingAverageInputs {
                        value: AnyInputValue(InputValue::Color(RgbaColor {
                            r: 0.6,
                            g: 0.4,
                            b: 0.2,
                            a: 0.5,
                        })),
                        window_size: 2.0,
                    },
                )
            })
            .expect("color average");

        assert_eq!(
            output.outputs.value,
            InputValue::Color(RgbaColor {
                r: 0.4,
                g: 0.4,
                b: 0.4,
                a: 0.75,
            })
        );
    }

    #[test]
    fn averages_tensors() {
        let mut node = MovingAverageNode::default();

        let output = node
            .evaluate(
                &context(),
                MovingAverageInputs {
                    value: AnyInputValue(InputValue::FloatTensor(FloatTensor {
                        shape: vec![2],
                        values: vec![1.0, 3.0],
                    })),
                    window_size: 2.0,
                },
            )
            .and_then(|_| {
                node.evaluate(
                    &context(),
                    MovingAverageInputs {
                        value: AnyInputValue(InputValue::FloatTensor(FloatTensor {
                            shape: vec![2],
                            values: vec![3.0, 5.0],
                        })),
                        window_size: 2.0,
                    },
                )
            })
            .expect("tensor average");

        assert_eq!(
            output.outputs.value,
            InputValue::FloatTensor(FloatTensor {
                shape: vec![2],
                values: vec![2.0, 4.0],
            })
        );
    }

    #[test]
    fn averages_frames() {
        let mut node = MovingAverageNode::default();
        let layout = LedLayout {
            id: "layout".to_owned(),
            pixel_count: 1,
            width: Some(1),
            height: Some(1),
        };

        let output = node
            .evaluate(
                &context(),
                MovingAverageInputs {
                    value: AnyInputValue(InputValue::ColorFrame(ColorFrame {
                        layout: layout.clone(),
                        pixels: vec![RgbaColor {
                            r: 0.2,
                            g: 0.4,
                            b: 0.6,
                            a: 1.0,
                        }],
                    })),
                    window_size: 2.0,
                },
            )
            .and_then(|_| {
                node.evaluate(
                    &context(),
                    MovingAverageInputs {
                        value: AnyInputValue(InputValue::ColorFrame(ColorFrame {
                            layout: layout.clone(),
                            pixels: vec![RgbaColor {
                                r: 0.6,
                                g: 0.2,
                                b: 0.4,
                                a: 0.5,
                            }],
                        })),
                        window_size: 2.0,
                    },
                )
            })
            .expect("frame average");

        assert_eq!(
            output.outputs.value,
            InputValue::ColorFrame(ColorFrame {
                layout,
                pixels: vec![RgbaColor {
                    r: 0.4,
                    g: 0.3,
                    b: 0.5,
                    a: 0.75,
                }],
            })
        );
    }
}
