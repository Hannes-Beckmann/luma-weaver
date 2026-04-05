use std::collections::VecDeque;

use anyhow::Result;
use shared::{FloatTensor, InputValue};

use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, RuntimeOutputs,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct MovingMedianNode {
    history: VecDeque<MedianSample>,
}

impl RuntimeNodeFromParameters for MovingMedianNode {}

pub(crate) struct MovingMedianInputs {
    value: AnyInputValue,
    window_size: f32,
}

crate::node_runtime::impl_runtime_inputs!(MovingMedianInputs {
    value = AnyInputValue(InputValue::Float(0.0)),
    window_size = 4.0,
});

pub(crate) struct MovingMedianOutputs {
    value: InputValue,
}

impl RuntimeOutputs for MovingMedianOutputs {
    fn into_runtime_outputs(self) -> anyhow::Result<std::collections::HashMap<String, InputValue>> {
        let mut outputs = std::collections::HashMap::new();
        outputs.insert("value".to_owned(), self.value);
        Ok(outputs)
    }
}

impl RuntimeNode for MovingMedianNode {
    type Inputs = MovingMedianInputs;
    type Outputs = MovingMedianOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let value = inputs.value.0;
        let window_size = inputs.window_size.round().clamp(1.0, 240.0) as usize;

        let Some(sample) = MedianSample::from_input_value(&value) else {
            return Ok(TypedNodeEvaluation::from_outputs(MovingMedianOutputs { value }));
        };

        if self
            .history
            .front()
            .is_some_and(|existing| !existing.is_compatible(&sample))
        {
            self.history.clear();
        }

        self.history.push_back(sample);
        while self.history.len() > window_size {
            self.history.pop_front();
        }

        let median = self
            .history
            .front()
            .map(|sample| sample.median(&self.history))
            .unwrap_or(value);

        Ok(TypedNodeEvaluation::from_outputs(MovingMedianOutputs {
            value: median,
        }))
    }
}

#[derive(Clone)]
enum MedianSample {
    Float(f32),
    Tensor(FloatTensor),
}

impl MedianSample {
    fn from_input_value(value: &InputValue) -> Option<Self> {
        match value {
            InputValue::Float(number) => Some(Self::Float(*number)),
            InputValue::FloatTensor(tensor) => Some(Self::Tensor(tensor.clone())),
            _ => None,
        }
    }

    fn is_compatible(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Float(_), Self::Float(_)) => true,
            (Self::Tensor(left), Self::Tensor(right)) => {
                left.shape == right.shape && left.values.len() == right.values.len()
            }
            _ => false,
        }
    }

    fn median(&self, history: &VecDeque<Self>) -> InputValue {
        match self {
            Self::Float(_) => InputValue::Float(median_scalar(
                &history
                    .iter()
                    .filter_map(|sample| match sample {
                        Self::Float(value) => Some(*value),
                        Self::Tensor(_) => None,
                    })
                    .collect::<Vec<_>>(),
            )),
            Self::Tensor(tensor) => {
                let mut medians = vec![0.0; tensor.values.len()];
                for (index, median) in medians.iter_mut().enumerate() {
                    let values = history
                        .iter()
                        .filter_map(|sample| match sample {
                            Self::Tensor(sample_tensor) => sample_tensor.values.get(index).copied(),
                            Self::Float(_) => None,
                        })
                        .collect::<Vec<_>>();
                    *median = median_scalar(&values);
                }

                InputValue::FloatTensor(FloatTensor {
                    shape: tensor.shape.clone(),
                    values: medians,
                })
            }
        }
    }
}

fn median_scalar(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }

    let mut sorted = values.to_vec();
    sorted.sort_by(|left, right| left.total_cmp(right));
    let middle = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        sorted[middle]
    } else {
        (sorted[middle - 1] + sorted[middle]) * 0.5
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
    fn filters_float_outliers() {
        let mut node = MovingMedianNode::default();
        let samples = [1.0, 1.0, 100.0, 1.0, 1.0];
        let mut last = InputValue::Float(0.0);

        for sample in samples {
            last = node
                .evaluate(
                    &context(),
                    MovingMedianInputs {
                        value: AnyInputValue(InputValue::Float(sample)),
                        window_size: 5.0,
                    },
                )
                .expect("float median")
                .outputs
                .value;
        }

        assert_eq!(last, InputValue::Float(1.0));
    }

    #[test]
    fn averages_middle_pair_for_even_float_windows() {
        let mut node = MovingMedianNode::default();
        let mut last = InputValue::Float(0.0);

        for sample in [1.0, 3.0, 5.0, 7.0] {
            last = node
                .evaluate(
                    &context(),
                    MovingMedianInputs {
                        value: AnyInputValue(InputValue::Float(sample)),
                        window_size: 4.0,
                    },
                )
                .expect("even float median")
                .outputs
                .value;
        }

        assert_eq!(last, InputValue::Float(4.0));
    }

    #[test]
    fn filters_tensor_outliers_per_index() {
        let mut node = MovingMedianNode::default();
        let samples = [
            vec![1.0, 10.0],
            vec![2.0, 11.0],
            vec![100.0, -50.0],
            vec![3.0, 12.0],
            vec![4.0, 13.0],
        ];
        let mut last = InputValue::Float(0.0);

        for values in samples {
            last = node
                .evaluate(
                    &context(),
                    MovingMedianInputs {
                        value: AnyInputValue(InputValue::FloatTensor(FloatTensor {
                            shape: vec![2],
                            values,
                        })),
                        window_size: 5.0,
                    },
                )
                .expect("tensor median")
                .outputs
                .value;
        }

        assert_eq!(
            last,
            InputValue::FloatTensor(FloatTensor {
                shape: vec![2],
                values: vec![3.0, 11.0],
            })
        );
    }

    #[test]
    fn resets_history_when_tensor_shape_changes() {
        let mut node = MovingMedianNode::default();

        let _ = node
            .evaluate(
                &context(),
                MovingMedianInputs {
                    value: AnyInputValue(InputValue::FloatTensor(FloatTensor {
                        shape: vec![2],
                        values: vec![1.0, 3.0],
                    })),
                    window_size: 4.0,
                },
            )
            .expect("first tensor median");

        let output = node
            .evaluate(
                &context(),
                MovingMedianInputs {
                    value: AnyInputValue(InputValue::FloatTensor(FloatTensor {
                        shape: vec![3],
                        values: vec![8.0, 6.0, 4.0],
                    })),
                    window_size: 4.0,
                },
            )
            .expect("reset tensor median");

        assert_eq!(
            output.outputs.value,
            InputValue::FloatTensor(FloatTensor {
                shape: vec![3],
                values: vec![8.0, 6.0, 4.0],
            })
        );
    }
}
