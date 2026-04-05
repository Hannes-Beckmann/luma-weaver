use std::collections::{HashMap, VecDeque};

use anyhow::Result;
use shared::{
    ColorFrame, FloatTensor, InputValue, LedLayout, NodeDiagnostic, NodeDiagnosticSeverity,
    RgbaColor,
};

use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, RuntimeInputs, RuntimeNode, TypedNodeEvaluation,
};

pub(crate) struct DelayNode {
    ticks: usize,
    history: VecDeque<InputValue>,
}

impl Default for DelayNode {
    fn default() -> Self {
        Self {
            ticks: 1,
            history: VecDeque::new(),
        }
    }
}

crate::node_runtime::impl_runtime_parameters!(DelayNode {
    ticks: u64 => |value| crate::node_runtime::max_u64_to_usize(value, 1), default 1usize,
    ..Self::default()
});

pub(crate) struct DelayInputs {
    value: AnyInputValue,
}

impl RuntimeInputs for DelayInputs {
    fn from_runtime_inputs(inputs: &HashMap<String, InputValue>) -> Result<Self> {
        Ok(Self {
            value: AnyInputValue(
                inputs
                    .get("value")
                    .cloned()
                    .unwrap_or(InputValue::Float(0.0)),
            ),
        })
    }
}

pub(crate) struct DelayOutputs {
    value: InputValue,
}

crate::node_runtime::impl_runtime_outputs!(DelayOutputs { value });

impl RuntimeNode for DelayNode {
    type Inputs = DelayInputs;
    type Outputs = DelayOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let mut diagnostics = Vec::new();
        let current = inputs.value.0;
        if self.ticks == 0 {
            return Ok(TypedNodeEvaluation {
                outputs: DelayOutputs { value: current },
                frontend_updates: Vec::new(),
                diagnostics,
            });
        }

        if self.history.is_empty() {
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Info,
                code: Some("delay_not_primed".to_owned()),
                message: format!(
                    "Delay is not primed yet; outputting a zero-like value for the first {} tick(s).",
                    self.ticks
                ),
            });
        }

        self.history.push_back(current.clone());
        let value = if self.history.len() > self.ticks {
            self.history
                .pop_front()
                .unwrap_or_else(|| zero_like(&current))
        } else {
            zero_like(&current)
        };

        Ok(TypedNodeEvaluation {
            outputs: DelayOutputs { value },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}

fn zero_like(value: &InputValue) -> InputValue {
    match value {
        InputValue::Float(_) => InputValue::Float(0.0),
        InputValue::FloatTensor(tensor) => InputValue::FloatTensor(FloatTensor {
            shape: tensor.shape.clone(),
            values: vec![0.0; tensor.values.len()],
        }),
        InputValue::Color(_) => InputValue::Color(RgbaColor {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        }),
        InputValue::LedLayout(layout) => InputValue::LedLayout(LedLayout {
            id: layout.id.clone(),
            pixel_count: layout.pixel_count,
            width: layout.width,
            height: layout.height,
        }),
        InputValue::ColorFrame(frame) => InputValue::ColorFrame(ColorFrame {
            layout: frame.layout.clone(),
            pixels: vec![
                RgbaColor {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.0,
                };
                frame.pixels.len()
            ],
        }),
    }
}

#[cfg(test)]
mod tests {
    use shared::{InputValue, LedLayout, RgbaColor};

    use crate::node_runtime::{AnyInputValue, NodeEvaluationContext, RuntimeNode};

    use super::{DelayInputs, DelayNode};

    #[test]
    fn first_frame_output_is_zero_like_input() {
        let mut node = DelayNode::default();
        let layout = LedLayout {
            id: "test".to_owned(),
            pixel_count: 2,
            width: None,
            height: None,
        };
        let input = InputValue::ColorFrame(shared::ColorFrame {
            layout: layout.clone(),
            pixels: vec![
                RgbaColor {
                    r: 1.0,
                    g: 0.5,
                    b: 0.25,
                    a: 1.0,
                };
                2
            ],
        });

        let evaluation = RuntimeNode::evaluate(
            &mut node,
            &NodeEvaluationContext {
                graph_id: "test-graph".to_owned(),
                graph_name: "Test Graph".to_owned(),
                elapsed_seconds: 0.0,
                render_layout: None,
            },
            DelayInputs {
                value: AnyInputValue(input),
            },
        )
        .expect("evaluate delay");

        assert_eq!(
            evaluation.outputs.value,
            InputValue::ColorFrame(shared::ColorFrame {
                layout,
                pixels: vec![
                    RgbaColor {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.0,
                    };
                    2
                ],
            })
        );
    }
}
