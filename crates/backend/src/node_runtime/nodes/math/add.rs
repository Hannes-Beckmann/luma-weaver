use anyhow::Result;
use shared::InputValue;

use crate::node_runtime::tensor::apply_binary_numeric_op;
use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct AddNode;

impl RuntimeNodeFromParameters for AddNode {}

pub(crate) struct AddInputs {
    a: AnyInputValue,
    b: AnyInputValue,
}

crate::node_runtime::impl_runtime_inputs!(AddInputs {
    a = AnyInputValue(InputValue::Float(0.0)),
    b = AnyInputValue(InputValue::Float(0.0)),
});

pub(crate) struct AddOutputs {
    sum: InputValue,
}

crate::node_runtime::impl_runtime_outputs!(AddOutputs { sum });

impl RuntimeNode for AddNode {
    type Inputs = AddInputs;
    type Outputs = AddOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let sum = apply_binary_numeric_op(&inputs.a.0, &inputs.b.0, "add", |a, b| a + b)?;
        Ok(TypedNodeEvaluation::from_outputs(AddOutputs { sum }))
    }
}

#[cfg(test)]
mod tests {
    use shared::{ColorFrame, FloatTensor, InputValue, LedLayout, RgbaColor};

    use super::{AddInputs, AddNode};
    use crate::node_runtime::{AnyInputValue, NodeEvaluationContext, RuntimeNode};

    fn context() -> NodeEvaluationContext {
        NodeEvaluationContext {
            graph_id: "test-graph".to_owned(),
            graph_name: "Test Graph".to_owned(),
            elapsed_seconds: 0.0,
            render_layout: None,
        }
    }

    #[test]
    fn adds_scalars() {
        let mut node = AddNode;

        let evaluation = node
            .evaluate(
                &context(),
                AddInputs {
                    a: AnyInputValue(InputValue::Float(1.5)),
                    b: AnyInputValue(InputValue::Float(2.0)),
                },
            )
            .expect("add float evaluation should succeed");

        assert_eq!(evaluation.outputs.sum, InputValue::Float(3.5));
    }

    #[test]
    fn broadcasts_scalars_into_tensors() {
        let mut node = AddNode;

        let evaluation = node
            .evaluate(
                &context(),
                AddInputs {
                    a: AnyInputValue(InputValue::FloatTensor(FloatTensor {
                        shape: vec![3],
                        values: vec![1.0, 2.0, 3.0],
                    })),
                    b: AnyInputValue(InputValue::Float(0.5)),
                },
            )
            .expect("add float evaluation should succeed");

        assert_eq!(
            evaluation.outputs.sum,
            InputValue::FloatTensor(FloatTensor {
                shape: vec![3],
                values: vec![1.5, 2.5, 3.5],
            })
        );
    }

    #[test]
    fn adds_frames_channel_wise() {
        let mut node = AddNode;
        let layout = LedLayout {
            id: "frame".to_owned(),
            pixel_count: 1,
            width: Some(1),
            height: Some(1),
        };

        let evaluation = node
            .evaluate(
                &context(),
                AddInputs {
                    a: AnyInputValue(InputValue::ColorFrame(ColorFrame {
                        layout: layout.clone(),
                        pixels: vec![RgbaColor {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 0.4,
                        }],
                    })),
                    b: AnyInputValue(InputValue::Float(0.5)),
                },
            )
            .expect("add float evaluation should succeed");

        assert_eq!(
            evaluation.outputs.sum,
            InputValue::ColorFrame(ColorFrame {
                layout,
                pixels: vec![RgbaColor {
                    r: 0.6,
                    g: 0.7,
                    b: 0.8,
                    a: 0.9,
                }],
            })
        );
    }
}
